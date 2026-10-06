use std::cell::{OnceCell, RefCell};
use std::collections::{BTreeMap, VecDeque};
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
    work: BackgroundWork,
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
    in_flight: BTreeMap<u64, Box<dyn BackgroundCompletion>>,
    ready: VecDeque<(u64, Box<dyn BackgroundCompletion>)>,
    next_ready_ticket: u64,
}

impl SourceThreadCompletions {
    fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::sync_channel(MAX_PENDING_BACKGROUND_WORK);
        Self {
            sender,
            receiver,
            in_flight: BTreeMap::new(),
            ready: VecDeque::new(),
            next_ready_ticket: 0,
        }
    }

    fn pending(&self) -> usize {
        self.in_flight.len() + self.ready.len()
    }
}

thread_local! {
    static SOURCE_THREAD_COMPLETIONS: OnceCell<RefCell<SourceThreadCompletions>> =
        const { OnceCell::new() };
}

fn with_source<Output>(
    callback: impl FnOnce(&RefCell<SourceThreadCompletions>) -> Output,
) -> Output {
    SOURCE_THREAD_COMPLETIONS.with(|source| {
        callback(source.get_or_init(|| RefCell::new(SourceThreadCompletions::new())))
    })
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
    let wake = crate::readiness::waker()?;
    let id = NEXT_WORK_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map_err(|_| {
            crate::NodeError::new(
                "ERR_NODE_BACKGROUND_WORK_LIMIT",
                "background work identity space is exhausted",
            )
        })?;
    let (result_sender, result_receiver) = std::sync::mpsc::sync_channel(1);
    let completion_sender = with_source(|source| {
        let mut source = source.borrow_mut();
        if source.pending() >= MAX_PENDING_BACKGROUND_WORK {
            return Err(crate::NodeError::new(
                "ERR_NODE_BACKGROUND_WORK_LIMIT",
                "pending background work exceeds the finite limit",
            ));
        }
        source.in_flight.insert(
            id,
            Box::new(TypedBackgroundCompletion {
                result_receiver,
                callback: completion,
            }),
        );
        Ok(source.sender.clone())
    })?;

    let request = WorkRequest {
        work: Box::new(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or_else(|_| {
                    Err(crate::NodeError::new(
                        "ERR_NODE_BACKGROUND_PANIC",
                        "background provider work panicked",
                    ))
                });
            let _ = result_sender.send(result);
            let _ = completion_sender.send(WorkCompletion { id });
            let _ = wake.wake();
        }),
    };
    match runtime.work_sender.try_send(request) {
        Ok(()) => Ok(()),
        Err(error) => {
            with_source(|source| {
                source.borrow_mut().in_flight.remove(&id);
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

pub(crate) async fn run<T>(
    work: impl FnOnce() -> crate::NodeResult<T> + Send + 'static,
) -> crate::NodeResult<T>
where
    T: Send + 'static,
{
    struct Completion<T> {
        result: Option<crate::NodeResult<T>>,
        waker: Option<std::task::Waker>,
    }
    let completion = Arc::new(Mutex::new(Completion {
        result: None,
        waker: None,
    }));
    let worker_completion = Arc::clone(&completion);
    let request = WorkRequest {
        work: Box::new(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or_else(|_| {
                    Err(crate::NodeError::new(
                        "ERR_NODE_BACKGROUND_PANIC",
                        "background provider work panicked",
                    ))
                });
            let waker = {
                let mut completion = crate::sync::lock(&worker_completion);
                completion.result = Some(result);
                completion.waker.take()
            };
            if let Some(waker) = waker {
                waker.wake();
            }
        }),
    };
    runtime()?.work_sender.try_send(request).map_err(|error| {
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

pub(crate) fn poll() -> tsonic_rust_runtime::TsonicResult<bool> {
    SOURCE_THREAD_COMPLETIONS.with(|source| source.get().map_or(Ok(false), poll_completions))
}

fn poll_completions(
    source: &RefCell<SourceThreadCompletions>,
) -> tsonic_rust_runtime::TsonicResult<bool> {
    let boundary = {
        let mut source = source.borrow_mut();
        loop {
            match source.receiver.try_recv() {
                Ok(value) => {
                    let next = source.next_ready_ticket.checked_add(1).ok_or_else(|| {
                        crate::NodeError::new(
                            "ERR_NODE_BACKGROUND_WORK_LIMIT",
                            "background ready ticket range is exhausted",
                        )
                    })?;
                    let callback = source.in_flight.remove(&value.id).ok_or_else(|| {
                        crate::NodeError::new(
                            "ERR_NODE_BACKGROUND_RESULT",
                            "background work completed without its exact callback",
                        )
                    })?;
                    let ticket = source.next_ready_ticket;
                    source.next_ready_ticket = next;
                    source.ready.push_back((ticket, callback));
                }
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
        source.ready.back().map(|(ticket, _)| *ticket)
    };
    let Some(boundary) = boundary else {
        return Ok(false);
    };
    let mut did_work = false;
    loop {
        let callback = {
            let mut source = source.borrow_mut();
            if source
                .ready
                .front()
                .is_some_and(|(ticket, _)| *ticket <= boundary)
            {
                source.ready.pop_front().map(|(_, callback)| callback)
            } else {
                None
            }
        };
        let Some(callback) = callback else {
            break;
        };
        callback.complete()?;
        did_work = true;
    }
    Ok(did_work)
}

pub(crate) fn has_pending_work() -> bool {
    SOURCE_THREAD_COMPLETIONS.with(|source| {
        source
            .get()
            .is_some_and(|source| source.borrow().pending() != 0)
    })
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
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::{Duration, Instant};
    use tsonic_rust_runtime::ErrorObject;

    struct TestCompletion<Callback>(Callback);

    impl<Callback: FnOnce() -> tsonic_rust_runtime::TsonicResult<()>> super::BackgroundCompletion
        for TestCompletion<Callback>
    {
        fn complete(self: Box<Self>) -> tsonic_rust_runtime::TsonicResult<()> {
            (self.0)()
        }
    }

    fn register_ready(
        source: &RefCell<super::SourceThreadCompletions>,
        id: u64,
        callback: impl FnOnce() -> tsonic_rust_runtime::TsonicResult<()> + 'static,
    ) {
        let mut source = source.borrow_mut();
        assert!(source
            .in_flight
            .insert(id, Box::new(TestCompletion(callback)))
            .is_none());
        source
            .sender
            .try_send(super::WorkCompletion { id })
            .unwrap();
    }

    #[test]
    fn context_free_completion_queries_do_not_initialize_channels() {
        assert!(!super::has_pending_work());
        assert!(!super::poll().unwrap());
        assert!(super::SOURCE_THREAD_COMPLETIONS.with(|source| source.get().is_none()));
    }

    #[test]
    fn first_completion_failure_preserves_identity_and_uninvoked_work() {
        let source = RefCell::new(super::SourceThreadCompletions::new());
        let expected = tsonic_rust_runtime::JsError::error("original background failure");
        let failure = expected.clone();
        register_ready(&source, 1, move || Err(failure.into()));
        let observed = Rc::new(Cell::new(0));
        let recorded = observed.clone();
        register_ready(&source, 2, move || {
            recorded.set(1);
            Ok(())
        });
        let returned = super::poll_completions(&source).unwrap_err();
        assert_eq!(
            returned.source_error().error_identity_key(),
            expected.error_identity_key()
        );
        assert_eq!(observed.get(), 0);
        assert_eq!(source.borrow().pending(), 1);
        assert!(super::poll_completions(&source).unwrap());
        assert_eq!(observed.get(), 1);
        assert_eq!(source.borrow().pending(), 0);
        assert!(!super::poll_completions(&source).unwrap());
    }

    #[test]
    fn nested_failed_dispatch_does_not_extend_the_outer_ready_frontier() {
        let source = Rc::new(RefCell::new(super::SourceThreadCompletions::new()));
        let owner = Rc::downgrade(&source);
        let observed = Rc::new(RefCell::new(Vec::new()));
        let recorded = observed.clone();
        let expected = tsonic_rust_runtime::JsError::error("nested background failure");
        let nested_identity = expected.error_identity_key();
        register_ready(&source, 1, move || {
            let source = owner.upgrade().unwrap();
            recorded.borrow_mut().push(1);
            let later = recorded.clone();
            register_ready(&source, 3, move || {
                later.borrow_mut().push(3);
                Ok(())
            });
            let returned = super::poll_completions(&source).unwrap_err();
            assert_eq!(
                returned.source_error().error_identity_key(),
                nested_identity
            );
            Ok(())
        });
        let recorded = observed.clone();
        register_ready(&source, 2, move || {
            recorded.borrow_mut().push(2);
            Err(expected.into())
        });
        assert!(super::poll_completions(&source).unwrap());
        assert_eq!(*observed.borrow(), vec![1, 2]);
        assert_eq!(source.borrow().pending(), 1);
        assert!(super::poll_completions(&source).unwrap());
        assert_eq!(*observed.borrow(), vec![1, 2, 3]);
        assert_eq!(source.borrow().pending(), 0);
    }

    #[test]
    fn malformed_completion_and_ticket_exhaustion_fail_before_invocation() {
        for duplicate in [false, true] {
            let source = RefCell::new(super::SourceThreadCompletions::new());
            let invoked = Rc::new(Cell::new(0));
            let recorded = invoked.clone();
            register_ready(&source, 1, move || {
                recorded.set(1);
                Ok(())
            });
            source
                .borrow()
                .sender
                .try_send(super::WorkCompletion {
                    id: if duplicate { 1 } else { 2 },
                })
                .unwrap();
            assert!(super::poll_completions(&source).is_err());
            assert_eq!(invoked.get(), 0);
            assert_eq!(source.borrow().pending(), 1);
            assert!(super::poll_completions(&source).unwrap());
            assert_eq!(invoked.get(), 1);
        }
        let source = RefCell::new(super::SourceThreadCompletions::new());
        source.borrow_mut().next_ready_ticket = u64::MAX;
        register_ready(&source, 1, || panic!("overflow must not invoke callbacks"));
        assert!(super::poll_completions(&source).is_err());
        assert_eq!(source.borrow().pending(), 1);
        assert!(source.borrow().ready.is_empty());
    }

    #[test]
    fn asynchronous_work_does_not_block_the_polling_thread() {
        use std::future::Future;
        use std::task::{Context, Poll, Wake, Waker};
        struct Notification(std::sync::mpsc::SyncSender<()>);
        impl Wake for Notification {
            fn wake(self: std::sync::Arc<Self>) {
                let _ = self.0.try_send(());
            }
        }
        let (release, blocked) = std::sync::mpsc::sync_channel(1);
        let (notify, notified) = std::sync::mpsc::sync_channel(1);
        let wake = Waker::from(std::sync::Arc::new(Notification(notify)));
        let mut context = Context::from_waker(&wake);
        let mut future = std::pin::pin!(super::run(move || {
            blocked.recv_timeout(Duration::from_secs(3)).unwrap();
            Ok(42)
        }));
        assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
        release.send(()).unwrap();
        notified.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(matches!(
            future.as_mut().poll(&mut context),
            Poll::Ready(Ok(42))
        ));
    }

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
