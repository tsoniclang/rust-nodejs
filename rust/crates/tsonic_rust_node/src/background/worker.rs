use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex, OnceLock};

const BACKGROUND_WORKER_COUNT: usize = 4;

struct WorkRequest {
    work: Box<dyn FnOnce() + Send>,
}

struct BackgroundRuntime {
    work_sender: SyncSender<WorkRequest>,
}

static RUNTIME: OnceLock<BackgroundRuntime> = OnceLock::new();
static RUNTIME_INITIALIZATION: Mutex<()> = Mutex::new(());

pub(super) fn submit(work: impl FnOnce() + Send + 'static) -> crate::NodeResult<()> {
    runtime()?
        .work_sender
        .try_send(WorkRequest {
            work: Box::new(work),
        })
        .map_err(|error| {
            crate::NodeError::new(
                "ERR_NODE_BACKGROUND_WORK_LIMIT",
                match error {
                    std::sync::mpsc::TrySendError::Full(_) => {
                        "background work queue exceeds the finite limit"
                    }
                    std::sync::mpsc::TrySendError::Disconnected(_) => {
                        "background worker pool is unavailable"
                    }
                },
            )
        })
}

pub(super) fn execute<TResult>(
    work: impl FnOnce() -> crate::NodeResult<TResult>,
) -> crate::NodeResult<TResult> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).unwrap_or_else(|_| {
        Err(crate::NodeError::new(
            "ERR_NODE_BACKGROUND_PANIC",
            "background provider work panicked",
        ))
    })
}

pub(crate) async fn run<TResult>(
    work: impl FnOnce() -> crate::NodeResult<TResult> + Send + 'static,
) -> crate::NodeResult<TResult>
where
    TResult: Send + 'static,
{
    struct Completion<TResult> {
        result: Option<crate::NodeResult<TResult>>,
        waker: Option<std::task::Waker>,
    }
    let completion = Arc::new(Mutex::new(Completion {
        result: None,
        waker: None,
    }));
    let worker_completion = Arc::clone(&completion);
    submit(move || {
        let result = execute(work);
        let waker = {
            let mut completion = crate::sync::lock(&worker_completion);
            completion.result = Some(result);
            completion.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    })?;
    std::future::poll_fn(|context| {
        let mut completion = crate::sync::lock(&completion);
        match completion.result.take() {
            Some(result) => std::task::Poll::Ready(result),
            None => {
                if !completion
                    .waker
                    .as_ref()
                    .is_some_and(|waker| waker.will_wake(context.waker()))
                {
                    completion.waker = Some(context.waker().clone());
                }
                std::task::Poll::Pending
            }
        }
    })
    .await
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
        std::sync::mpsc::sync_channel::<WorkRequest>(super::MAX_PENDING_BACKGROUND_WORK);
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
    let _ = RUNTIME.set(BackgroundRuntime { work_sender });
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
    }
}
