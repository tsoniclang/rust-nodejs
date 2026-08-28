use std::any::Any;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex, OnceLock};

const BACKGROUND_WORKER_COUNT: usize = 4;
const MAX_PENDING_BACKGROUND_WORK: usize = 16 * 1024;

type BackgroundValue = Box<dyn Any + Send>;
type BackgroundResult = Result<BackgroundValue, crate::NodeError>;
type BackgroundWork = Box<dyn FnOnce() -> BackgroundResult + Send>;
type BackgroundCompletion =
    Box<dyn FnOnce(BackgroundResult) -> tsonic_rust_runtime::TsonicResult<()>>;

struct WorkRequest {
    id: u64,
    work: BackgroundWork,
}

struct WorkCompletion {
    id: u64,
    result: BackgroundResult,
}

struct BackgroundRuntime {
    work_sender: SyncSender<WorkRequest>,
    completion_receiver: Mutex<std::sync::mpsc::Receiver<WorkCompletion>>,
}

thread_local! {
    static COMPLETIONS: std::cell::RefCell<BTreeMap<u64, BackgroundCompletion>> =
        std::cell::RefCell::new(BTreeMap::new());
}

static RUNTIME: OnceLock<BackgroundRuntime> = OnceLock::new();
static RUNTIME_INITIALIZATION: Mutex<()> = Mutex::new(());
static NEXT_WORK_ID: AtomicU64 = AtomicU64::new(1);
static PENDING_WORK_COUNT: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn spawn<T>(
    work: impl FnOnce() -> crate::NodeResult<T> + Send + 'static,
    completion: impl FnOnce(crate::NodeResult<T>) -> tsonic_rust_runtime::TsonicResult<()> + 'static,
) -> crate::NodeResult<()>
where
    T: Send + 'static,
{
    let runtime = runtime()?;
    let id = NEXT_WORK_ID.fetch_add(1, Ordering::Relaxed);
    if id == u64::MAX {
        return Err(crate::NodeError::new(
            "ERR_NODE_BACKGROUND_WORK_LIMIT",
            "background work identity space is exhausted",
        ));
    }
    COMPLETIONS.with(|completions| {
        let mut completions = completions.borrow_mut();
        if completions.len() >= MAX_PENDING_BACKGROUND_WORK {
            return Err(crate::NodeError::new(
                "ERR_NODE_BACKGROUND_WORK_LIMIT",
                "pending background work exceeds the finite limit",
            ));
        }
        completions.insert(
            id,
            Box::new(move |result| {
                let typed = result.and_then(|value| {
                    value.downcast::<T>().map(|value| *value).map_err(|_| {
                        crate::NodeError::new(
                            "ERR_NODE_BACKGROUND_RESULT",
                            "background work returned an incompatible result carrier",
                        )
                    })
                });
                completion(typed)
            }),
        );
        Ok(())
    })?;

    let request = WorkRequest {
        id,
        work: Box::new(move || work().map(|value| Box::new(value) as BackgroundValue)),
    };
    match runtime.work_sender.try_send(request) {
        Ok(()) => {
            PENDING_WORK_COUNT.fetch_add(1, Ordering::Release);
            Ok(())
        }
        Err(error) => {
            COMPLETIONS.with(|completions| {
                completions.borrow_mut().remove(&id);
            });
            Err(crate::NodeError::new(
                "ERR_NODE_BACKGROUND_WORK_LIMIT",
                match error {
                    std::sync::mpsc::TrySendError::Full(_) => {
                        "background work queue exceeds the finite limit"
                    }
                    std::sync::mpsc::TrySendError::Disconnected(_) => {
                        "background worker pool is unavailable"
                    }
                },
            ))
        }
    }
}

pub(crate) fn poll() -> tsonic_rust_runtime::TsonicResult<bool> {
    let Some(runtime) = RUNTIME.get() else {
        return Ok(false);
    };
    let completed = {
        let receiver = crate::sync::lock(&runtime.completion_receiver);
        let mut values = Vec::new();
        loop {
            match receiver.try_recv() {
                Ok(value) => values.push(value),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Err(tsonic_rust_runtime::TsonicError::from(
                        crate::NodeError::new(
                            "ERR_NODE_BACKGROUND_WORKER",
                            "background completion channel is unavailable",
                        ),
                    ));
                }
            }
        }
        values
    };
    let did_work = !completed.is_empty();
    for completed in completed {
        let callback = COMPLETIONS
            .with(|completions| completions.borrow_mut().remove(&completed.id))
            .ok_or_else(|| {
                tsonic_rust_runtime::TsonicError::from(crate::NodeError::new(
                    "ERR_NODE_BACKGROUND_RESULT",
                    "background work completed without its exact callback",
                ))
            })?;
        PENDING_WORK_COUNT.fetch_sub(1, Ordering::AcqRel);
        callback(completed.result)?;
    }
    Ok(did_work)
}

pub(crate) fn has_pending_work() -> bool {
    PENDING_WORK_COUNT.load(Ordering::Acquire) != 0
}

fn runtime() -> crate::NodeResult<&'static BackgroundRuntime> {
    if let Some(runtime) = RUNTIME.get() {
        return Ok(runtime);
    }
    let _initialization = crate::sync::lock(&RUNTIME_INITIALIZATION);
    if let Some(runtime) = RUNTIME.get() {
        return Ok(runtime);
    }
    let (work_sender, work_receiver) =
        std::sync::mpsc::sync_channel::<WorkRequest>(MAX_PENDING_BACKGROUND_WORK);
    let (completion_sender, completion_receiver) =
        std::sync::mpsc::sync_channel::<WorkCompletion>(MAX_PENDING_BACKGROUND_WORK);
    let shared_receiver = Arc::new(Mutex::new(work_receiver));
    for worker_index in 0..BACKGROUND_WORKER_COUNT {
        let work_receiver = Arc::clone(&shared_receiver);
        let completion_sender = completion_sender.clone();
        std::thread::Builder::new()
            .name(format!("tsonic-node-worker-{worker_index}"))
            .spawn(move || worker_loop(work_receiver, completion_sender))
            .map_err(|error| {
                crate::NodeError::new("ERR_NODE_BACKGROUND_WORKER", error.to_string())
            })?;
    }
    let created = BackgroundRuntime {
        work_sender,
        completion_receiver: Mutex::new(completion_receiver),
    };
    let _ = RUNTIME.set(created);
    RUNTIME.get().ok_or_else(|| {
        crate::NodeError::new(
            "ERR_NODE_BACKGROUND_WORKER",
            "background worker pool initialization failed",
        )
    })
}

fn worker_loop(
    work_receiver: Arc<Mutex<std::sync::mpsc::Receiver<WorkRequest>>>,
    completion_sender: SyncSender<WorkCompletion>,
) {
    loop {
        let request = {
            let receiver = crate::sync::lock(&work_receiver);
            receiver.recv()
        };
        let Ok(request) = request else {
            return;
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(request.work))
            .unwrap_or_else(|_| {
                Err(crate::NodeError::new(
                    "ERR_NODE_BACKGROUND_PANIC",
                    "background provider work panicked",
                ))
            });
        if completion_sender
            .send(WorkCompletion {
                id: request.id,
                result,
            })
            .is_err()
        {
            return;
        }
    }
}
