use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};
use tsonic_rust_runtime::dispatch_queue::TaskTicket;
use tsonic_rust_runtime::{ErrorObject, TsonicError};

struct TestCompletion<TCallback>(TCallback);

impl<TCallback: FnOnce() -> Result<(), TError>, TError> super::BackgroundCompletion<TError>
    for TestCompletion<TCallback>
{
    fn complete(self: Box<Self>) -> Result<(), TError> {
        (self.0)()
    }
}

fn register_ready<TError>(
    source: &RefCell<super::SourceThreadCompletions<TError>>,
    callback: impl FnOnce() -> Result<(), TError> + 'static,
) -> TaskTicket {
    let reservation = super::background_task_budget().reserve().unwrap();
    let id = reservation.ticket();
    let mut source = source.borrow_mut();
    assert!(source
        .in_flight
        .insert(
            id,
            super::PendingCompletion {
                reservation,
                callback: Box::new(TestCompletion(callback)),
            },
        )
        .is_none());
    source
        .sender
        .try_send(super::WorkCompletion { id })
        .unwrap();
    id
}

#[test]
fn context_free_completion_queries_do_not_initialize_channels() {
    assert!(!super::has_pending_work());
    assert!(!super::poll().unwrap());
    assert!(super::SOURCE_THREAD_COMPLETIONS.with(|source| source.source.get().is_none()));
    let typed = super::BackgroundTasks::<Failure>::new();
    assert!(!typed.has_pending_work());
    assert!(matches!(typed.poll(), Ok(false)));
    assert!(typed.source.get().is_none());
    assert!(super::BACKGROUND_TASK_BUDGET.with(|budget| budget.get().is_none()));
}

#[test]
fn first_completion_failure_preserves_identity_and_uninvoked_work() {
    let source = RefCell::new(super::SourceThreadCompletions::<TsonicError>::new());
    let expected = tsonic_rust_runtime::JsError::error("original background failure");
    let failure = expected.clone();
    register_ready(&source, move || Err(failure.into()));
    let observed = Rc::new(Cell::new(0));
    let recorded = observed.clone();
    register_ready(&source, move || {
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
    assert_eq!(super::background_task_budget().pending(), 0);
}

#[test]
fn nested_failed_dispatch_does_not_extend_the_outer_ready_frontier() {
    let source = Rc::new(RefCell::new(
        super::SourceThreadCompletions::<TsonicError>::new(),
    ));
    let owner = Rc::downgrade(&source);
    let observed = Rc::new(RefCell::new(Vec::new()));
    let recorded = observed.clone();
    let expected = tsonic_rust_runtime::JsError::error("nested background failure");
    let nested_identity = expected.error_identity_key();
    register_ready(&source, move || {
        let source = owner.upgrade().unwrap();
        recorded.borrow_mut().push(1);
        let later = recorded.clone();
        register_ready(&source, move || {
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
    register_ready(&source, move || {
        recorded.borrow_mut().push(2);
        Err(expected.into())
    });
    assert!(super::poll_completions(&source).unwrap());
    assert_eq!(*observed.borrow(), vec![1, 2]);
    assert_eq!(source.borrow().pending(), 1);
    assert!(super::poll_completions(&source).unwrap());
    assert_eq!(*observed.borrow(), vec![1, 2, 3]);
    assert_eq!(source.borrow().pending(), 0);
    assert_eq!(super::background_task_budget().pending(), 0);
}

#[test]
fn malformed_completion_and_ticket_exhaustion_fail_before_invocation() {
    for duplicate in [false, true] {
        let source = RefCell::new(super::SourceThreadCompletions::<TsonicError>::new());
        let invoked = Rc::new(Cell::new(0));
        let recorded = invoked.clone();
        let id = register_ready(&source, move || {
            recorded.set(1);
            Ok(())
        });
        let unknown = super::background_task_budget().reserve().unwrap();
        source
            .borrow()
            .sender
            .try_send(super::WorkCompletion {
                id: if duplicate { id } else { unknown.ticket() },
            })
            .unwrap();
        assert!(super::poll_completions(&source).is_err());
        assert_eq!(invoked.get(), 0);
        assert_eq!(source.borrow().pending(), 1);
        assert!(super::poll_completions(&source).unwrap());
        assert_eq!(invoked.get(), 1);
    }
    let source = RefCell::new(super::SourceThreadCompletions::<TsonicError>::new());
    register_ready(&source, || panic!("overflow must not invoke callbacks"));
    super::NEXT_COMPLETION_TICKET.with(|sequence| {
        let previous = sequence.replace(u64::MAX);
        let failed = super::poll_completions(&source).is_err();
        sequence.set(previous);
        assert!(failed);
    });
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
                    completion_observed.set(Some(result.map_err(TsonicError::from)?));
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
            assert_eq!(super::background_task_budget().pending(), 0);
        })
    });
    for thread in threads {
        thread.join().unwrap();
    }
}

enum Failure {
    Payload(Rc<Cell<i64>>),
    Native(TsonicError),
}

impl From<TsonicError> for Failure {
    fn from(value: TsonicError) -> Self {
        Self::Native(value)
    }
}

struct OtherFailure(Failure);

impl From<OtherFailure> for Failure {
    fn from(value: OtherFailure) -> Self {
        value.0
    }
}

impl From<TsonicError> for OtherFailure {
    fn from(value: TsonicError) -> Self {
        Self(Failure::Native(value))
    }
}

#[test]
fn native_work_retains_non_send_non_clone_non_display_callback_failures() {
    let root = super::BackgroundTasks::<Failure>::new();
    let expected = Rc::new(Cell::new(9_007_199_254_740_993));
    let failure = expected.clone();
    let source_thread = std::thread::current().id();
    root.spawn(
        || Ok(std::thread::current().id()),
        move |result| {
            assert_ne!(result.unwrap(), source_thread);
            assert_eq!(std::thread::current().id(), source_thread);
            Err(Failure::Payload(failure))
        },
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let returned = loop {
        match root.poll() {
            Err(error) => break error,
            Ok(_) => {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
        }
    };
    let Failure::Payload(returned) = returned else {
        panic!("callback failure must retain its original native type");
    };
    assert!(Rc::ptr_eq(&returned, &expected));
    assert_eq!(returned.get(), 9_007_199_254_740_993);
    assert!(!root.has_pending_work());
    assert_eq!(super::background_task_budget().pending(), 0);
}

#[test]
fn distinct_callback_domains_share_capacity_without_initializing_rejected_roots() {
    let budget = super::background_task_budget();
    let reservations: Vec<_> = (0..super::MAX_PENDING_BACKGROUND_WORK - 1)
        .map(|_| budget.reserve().unwrap())
        .collect();
    let root = super::BackgroundTasks::<Failure>::new();
    let other = super::BackgroundTasks::<OtherFailure>::new();
    let (release, blocked) = std::sync::mpsc::sync_channel(1);
    root.spawn(
        move || {
            blocked.recv_timeout(Duration::from_secs(5)).unwrap();
            Ok(())
        },
        |_| Ok(()),
    )
    .unwrap();
    let rejected = other.spawn(
        || -> crate::NodeResult<()> { panic!("rejected work cannot execute") },
        |_| Ok(()),
    );
    assert_eq!(
        rejected.unwrap_err().code(),
        "ERR_NODE_BACKGROUND_WORK_LIMIT"
    );
    assert!(other.source.get().is_none());
    assert_eq!(budget.pending(), super::MAX_PENDING_BACKGROUND_WORK);
    drop(root);
    assert_eq!(budget.pending(), reservations.len());
    release.send(()).unwrap();
    drop(reservations);
    assert_eq!(budget.pending(), 0);
    let value = Rc::new(Cell::new(9_007_199_254_740_993));
    let failure = value.clone();
    other
        .spawn(
            || Ok(()),
            move |_| Err(OtherFailure(Failure::Payload(failure))),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let returned = loop {
        match other.poll() {
            Err(error) => break error,
            Ok(_) => {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
        }
    };
    let OtherFailure(Failure::Payload(returned)) = returned else {
        panic!("second domain must retain its exact payload");
    };
    assert!(Rc::ptr_eq(&returned, &value));
    assert_eq!(returned.get(), 9_007_199_254_740_993);
    assert_eq!(budget.pending(), 0);
}

#[test]
fn dropping_the_source_owner_releases_callbacks_and_capacity_before_native_work_finishes() {
    let budget = super::background_task_budget();
    let root = super::BackgroundTasks::<Failure>::new();
    let retained = Rc::new(Cell::new(17));
    let released = Rc::downgrade(&retained);
    let (release, blocked) = std::sync::mpsc::sync_channel(1);
    root.spawn(
        move || {
            blocked.recv_timeout(Duration::from_secs(5)).unwrap();
            Ok(())
        },
        move |_| {
            retained.set(29);
            panic!("released callback must not execute");
        },
    )
    .unwrap();
    assert!(root.has_pending_work());
    assert_eq!(budget.pending(), 1);
    assert!(released.upgrade().is_some());
    drop(root);
    assert!(released.upgrade().is_none());
    assert_eq!(budget.pending(), 0);
    release.send(()).unwrap();
}

#[test]
fn weak_scheduling_handles_do_not_retain_their_callback_owner() {
    let budget = super::background_task_budget();
    let root = super::BackgroundTasks::<Failure>::new();
    let handle = root.handle();
    let scheduled = handle.clone();
    assert!(handle.source.ptr_eq(&scheduled.source));
    let retained = Rc::new(Cell::new(17));
    let released = Rc::downgrade(&retained);
    let (release, blocked) = std::sync::mpsc::sync_channel(1);
    handle
        .spawn(
            move || {
                blocked.recv_timeout(Duration::from_secs(5)).unwrap();
                Ok(())
            },
            move |_| {
                retained.set(29);
                scheduled
                    .spawn(|| Ok(()), |_| Ok(()))
                    .map_err(TsonicError::from)?;
                panic!("released callback must not execute");
            },
        )
        .unwrap();
    assert!(released.upgrade().is_some());
    assert_eq!(budget.pending(), 1);
    drop(root);
    assert!(handle.source.upgrade().is_none());
    assert!(released.upgrade().is_none());
    assert_eq!(budget.pending(), 0);
    let rejected = handle.spawn(
        || -> crate::NodeResult<()> { panic!("closed owner cannot submit native work") },
        |_| Ok(()),
    );
    assert_eq!(rejected.unwrap_err().code(), "ERR_NODE_BACKGROUND_CLOSED");
    assert_eq!(budget.pending(), 0);
    release.send(()).unwrap();
}

#[test]
fn weak_reentrant_scheduling_retains_exact_failures_without_a_source_borrow() {
    let budget = super::background_task_budget();
    let root = super::BackgroundTasks::<Failure>::new();
    let handle = root.handle();
    let expected = Rc::new(Cell::new(9_007_199_254_740_993));
    let failure = expected.clone();
    let observed = Rc::new(Cell::new(0));
    let recorded = observed.clone();
    root.spawn(
        || Ok(()),
        move |_| {
            recorded.set(1);
            let recorded = recorded.clone();
            handle
                .spawn(
                    || Ok(()),
                    move |_| {
                        recorded.set(2);
                        Ok(())
                    },
                )
                .map_err(TsonicError::from)?;
            Err(Failure::Payload(failure))
        },
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let returned = loop {
        match root.poll() {
            Err(error) => break error,
            Ok(_) => {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
        }
    };
    let Failure::Payload(returned) = returned else {
        panic!("reentrant callback must retain its original failure");
    };
    assert!(Rc::ptr_eq(&returned, &expected));
    assert_eq!(returned.get(), 9_007_199_254_740_993);
    assert_eq!(observed.get(), 1);
    assert_eq!(budget.pending(), 1);
    while root.has_pending_work() && Instant::now() < deadline {
        assert!(root.poll().is_ok());
        std::thread::yield_now();
    }
    assert!(!root.has_pending_work());
    assert_eq!(observed.get(), 2);
    assert_eq!(budget.pending(), 0);
}

#[test]
fn native_failures_lift_once_without_replacing_the_source_identity() {
    let root = super::BackgroundTasks::<Failure>::new();
    let identity = Rc::new(Cell::new(None));
    let recorded = identity.clone();
    root.spawn(
        || {
            Err::<(), _>(crate::NodeError::new(
                "original-native-code",
                "original-native-message",
            ))
        },
        move |result| {
            let error = result.unwrap_err();
            recorded.set(Some((
                error.code().as_ptr() as usize,
                error.error_identity_key(),
            )));
            Err(Failure::Native(error.into()))
        },
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let returned = loop {
        match root.poll() {
            Err(error) => break error,
            Ok(_) => {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
        }
    };
    let Failure::Native(TsonicError::Node { code, source }) = returned else {
        panic!("native fault must retain its exact native boundary");
    };
    assert_eq!(code, "original-native-code");
    assert_eq!(
        Some((code.as_ptr() as usize, source.error_identity_key())),
        identity.get()
    );
    assert_eq!(source.message(), "original-native-message");
}

struct ReentrantFailure(TsonicError);

thread_local! {
    static REENTRANT_NATIVE_FAULT_HANDLE: RefCell<Option<super::BackgroundHandle<ReentrantFailure>>> =
        const { RefCell::new(None) };
}

impl From<TsonicError> for ReentrantFailure {
    fn from(value: TsonicError) -> Self {
        REENTRANT_NATIVE_FAULT_HANDLE.with_borrow(|handle| {
            if let Some(handle) = handle {
                handle.spawn(|| Ok(()), |_| Ok(())).unwrap();
            }
        });
        Self(value)
    }
}

#[test]
fn native_fault_conversion_releases_registry_borrows_before_user_from_code() {
    let root = super::BackgroundTasks::<ReentrantFailure>::new();
    let handle = root.handle();
    REENTRANT_NATIVE_FAULT_HANDLE.set(Some(handle));
    let unregistered = super::background_task_budget().reserve().unwrap();
    root.scheduling_source()
        .borrow()
        .sender
        .try_send(super::WorkCompletion {
            id: unregistered.ticket(),
        })
        .unwrap();
    let returned = root
        .poll()
        .err()
        .expect("unregistered native completion must fail");
    assert_eq!(
        returned.0.source_error().message(),
        "background work completed without its exact callback"
    );
    assert!(root.has_pending_work());
    REENTRANT_NATIVE_FAULT_HANDLE.set(None);
    drop(unregistered);
    let deadline = Instant::now() + Duration::from_secs(5);
    while root.has_pending_work() && Instant::now() < deadline {
        assert!(root.poll().is_ok());
        std::thread::yield_now();
    }
    assert!(!root.has_pending_work());
    assert_eq!(super::background_task_budget().pending(), 0);
}

mod composition;
