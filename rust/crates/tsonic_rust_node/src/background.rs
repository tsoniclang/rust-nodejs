use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::{Arc, Mutex, OnceLock};

const BACKGROUND_WORKER_COUNT: usize = 4;
const MAX_PENDING_BACKGROUND_WORK: usize = 16 * 1024;

type BackgroundWork = Box<dyn FnOnce() + Send>;

trait BackgroundCompletion {
    fn complete(self: Box<Self>) -> tsonic_rust_runtime::TsonicResult<()>;
}

struct TypedBackgroundCompletion<T, F> {
    result_receiver: Receiver<crate::NodeResult<T>>,
    callback: F,
}

impl<T, F> BackgroundCompletion for TypedBackgroundCompletion<T, F>
where
    T: Send + 'static,
    F: FnOnce(crate::NodeResult<T>) -> tsonic_rust_runtime::TsonicResult<()> + 'static,
{
    fn complete(self: Box<Self>) -> tsonic_rust_runtime::TsonicResult<()> {
        let Self {
            result_receiver,
            callback,
        } = *self;
        let result = result_receiver.recv().map_err(|_| {
            tsonic_rust_runtime::TsonicError::from(crate::NodeError::new(
                "ERR_NODE_BACKGROUND_RESULT",
                "background work completed without its exact typed result",
            ))
        })?;
        callback(result)
    }
}

struct WorkRequest {
    id: u64,
    work: BackgroundWork,
    completion_sender: SyncSender<WorkCompletion>,
}

struct WorkCompletion {
    id: u64,
}

struct BackgroundRuntime {
    work_sender: SyncSender<WorkRequest>,
}

struct SourceThreadCompletions {
    sender: SyncSender<WorkCompletion>,
    receiver: Receiver<WorkCompletion>,
    callbacks: BTreeMap<u64, Box<dyn BackgroundCompletion>>,
    pending: usize,
}

impl SourceThreadCompletions {
    fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::sync_channel(MAX_PENDING_BACKGROUND_WORK);
        Self {
            sender,
            receiver,
            callbacks: BTreeMap::new(),
            pending: 0,
        }
    }
}

thread_local! {
    static SOURCE_THREAD_COMPLETIONS: std::cell::RefCell<SourceThreadCompletions> =
        std::cell::RefCell::new(SourceThreadCompletions::new());
}

static RUNTIME: OnceLock<BackgroundRuntime> = OnceLock::new();
static RUNTIME_INITIALIZATION: Mutex<()> = Mutex::new(());
static NEXT_WORK_ID: AtomicU64 = AtomicU64::new(1);

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
    let (result_sender, result_receiver) = std::sync::mpsc::sync_channel(1);
    let completion_sender = SOURCE_THREAD_COMPLETIONS.with(|source| {
        let mut source = source.borrow_mut();
        if source.callbacks.len() >= MAX_PENDING_BACKGROUND_WORK {
            return Err(crate::NodeError::new(
                "ERR_NODE_BACKGROUND_WORK_LIMIT",
                "pending background work exceeds the finite limit",
            ));
        }
        source.callbacks.insert(
            id,
            Box::new(TypedBackgroundCompletion {
                result_receiver,
                callback: completion,
            }),
        );
        Ok(source.sender.clone())
    })?;

    let request = WorkRequest {
        id,
        completion_sender,
        work: Box::new(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or_else(|_| {
                    Err(crate::NodeError::new(
                        "ERR_NODE_BACKGROUND_PANIC",
                        "background provider work panicked",
                    ))
                });
            let _ = result_sender.send(result);
        }),
    };
    match runtime.work_sender.try_send(request) {
        Ok(()) => {
            SOURCE_THREAD_COMPLETIONS.with(|source| {
                source.borrow_mut().pending += 1;
            });
            Ok(())
        }
        Err(error) => {
            SOURCE_THREAD_COMPLETIONS.with(|source| {
                source.borrow_mut().callbacks.remove(&id);
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
    let callbacks = SOURCE_THREAD_COMPLETIONS.with(|source| {
        let mut source = source.borrow_mut();
        let mut completed = Vec::new();
        loop {
            match source.receiver.try_recv() {
                Ok(value) => completed.push(value),
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
        if completed
            .iter()
            .any(|completion| !source.callbacks.contains_key(&completion.id))
            || source.pending < completed.len()
        {
            return Err(tsonic_rust_runtime::TsonicError::from(
                crate::NodeError::new(
                    "ERR_NODE_BACKGROUND_RESULT",
                    "background work completed without its exact callback",
                ),
            ));
        }
        source.pending -= completed.len();
        completed
            .into_iter()
            .map(|completion| {
                source.callbacks.remove(&completion.id).ok_or_else(|| {
                    tsonic_rust_runtime::TsonicError::from(crate::NodeError::new(
                        "ERR_NODE_BACKGROUND_RESULT",
                        "background work completed without its exact callback",
                    ))
                })
            })
            .collect::<tsonic_rust_runtime::TsonicResult<Vec<_>>>()
    })?;
    let did_work = !callbacks.is_empty();
    for callback in callbacks {
        callback.complete()?;
    }
    Ok(did_work)
}

pub(crate) fn has_pending_work() -> bool {
    SOURCE_THREAD_COMPLETIONS.with(|source| source.borrow().pending != 0)
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
    let shared_receiver = Arc::new(Mutex::new(work_receiver));
    for worker_index in 0..BACKGROUND_WORKER_COUNT {
        let work_receiver = Arc::clone(&shared_receiver);
        std::thread::Builder::new()
            .name(format!("tsonic-node-worker-{worker_index}"))
            .spawn(move || worker_loop(work_receiver))
            .map_err(|error| {
                crate::NodeError::new("ERR_NODE_BACKGROUND_WORKER", error.to_string())
            })?;
    }
    let created = BackgroundRuntime { work_sender };
    let _ = RUNTIME.set(created);
    RUNTIME.get().ok_or_else(|| {
        crate::NodeError::new(
            "ERR_NODE_BACKGROUND_WORKER",
            "background worker pool initialization failed",
        )
    })
}

fn worker_loop(work_receiver: Arc<Mutex<std::sync::mpsc::Receiver<WorkRequest>>>) {
    loop {
        let request = {
            let receiver = crate::sync::lock(&work_receiver);
            receiver.recv()
        };
        let Ok(request) = request else {
            return;
        };
        (request.work)();
        let _ = request
            .completion_sender
            .send(WorkCompletion { id: request.id });
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    #[test]
    fn completions_return_to_the_exact_source_thread() {
        let threads = [11_u32, 29_u32].map(|expected| {
            std::thread::spawn(move || {
                let observed = Rc::new(Cell::new(None));
                let completion_observed = Rc::clone(&observed);
                super::spawn(
                    move || Ok(expected),
                    move |result| {
                        completion_observed.set(Some(
                            result.map_err(tsonic_rust_runtime::TsonicError::from)?,
                        ));
                        Ok(())
                    },
                )
                .unwrap();

                let deadline = Instant::now() + Duration::from_secs(5);
                while super::has_pending_work() && Instant::now() < deadline {
                    super::poll().unwrap();
                    std::thread::yield_now();
                }
                assert!(!super::has_pending_work());
                assert_eq!(observed.get(), Some(expected));
            })
        });
        for thread in threads {
            thread.join().unwrap();
        }
    }
}
