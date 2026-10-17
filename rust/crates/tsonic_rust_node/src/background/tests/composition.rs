use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{register_ready, Failure, OtherFailure};
use crate::background::{background_task_budget, BackgroundTasks};
use tsonic_rust_runtime::TsonicError;

#[test]
fn loop_owner_shutdown_detaches_native_callbacks_and_channels_before_reuse() {
    use tsonic_rust_runtime::dispatch::{prepend, DispatchEnd};
    struct Capture {
        drops: Rc<Cell<usize>>,
        thread: std::thread::ThreadId,
    }
    impl Drop for Capture {
        fn drop(&mut self) {
            assert_eq!(std::thread::current().id(), self.thread);
            self.drops.set(self.drops.get() + 1);
        }
    }
    let tasks = BackgroundTasks::<TsonicError>::new();
    let old = tasks.handle();
    assert!(tasks.source.get().unwrap().borrow().channel.is_none());
    let (release, waiting) = std::sync::mpsc::sync_channel(1);
    let drops = Rc::new(Cell::new(0));
    let capture = Capture {
        drops: Rc::clone(&drops),
        thread: std::thread::current().id(),
    };
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::block_on_with_contexts(
            async {
                old.spawn(
                    move || {
                        waiting.recv().unwrap();
                        Ok(())
                    },
                    move |_| {
                        drop(capture);
                        panic!("retired background callback must never run")
                    },
                )
                .unwrap();
                assert!(tasks.has_pending_work());
                panic!("original root failure");
            },
            prepend(&tasks, DispatchEnd::<TsonicError>::new()),
        )
    }));
    assert!(failure.is_err());
    assert_eq!(drops.get(), 1);
    assert_eq!(Rc::strong_count(&drops), 1);
    assert!(!tasks.has_pending_work());
    assert!(tasks.source.get().unwrap().borrow().channel.is_none());
    assert_eq!(background_task_budget().pending(), 0);
    release.send(()).unwrap();
    let next = tasks.handle();
    let observed = Rc::new(Cell::new(0));
    let recorded = Rc::clone(&observed);
    crate::block_on_with_contexts(
        async {
            let error = old.spawn(|| Ok(()), |_| Ok(())).unwrap_err();
            assert_eq!(error.code(), "ERR_NODE_BACKGROUND_CLOSED");
            next.spawn(
                || Ok(7),
                move |result| {
                    recorded.set(result?);
                    Ok(())
                },
            )
            .unwrap();
        },
        prepend(&tasks, DispatchEnd::<TsonicError>::new()),
    )
    .unwrap();
    assert_eq!(observed.get(), 7);
    assert_eq!(Rc::strong_count(&observed), 1);
    assert_eq!(drops.get(), 1);
    assert!(tasks.source.get().unwrap().borrow().channel.is_none());
    assert_eq!(background_task_budget().pending(), 0);
}

#[test]
fn composed_background_domains_use_one_ready_order_and_retain_uninvoked_callbacks() {
    use tsonic_rust_runtime::dispatch::{DispatchContexts, DispatchEnd};
    let first = BackgroundTasks::<Failure>::new();
    let second = BackgroundTasks::<OtherFailure>::new();
    let observed = Rc::new(RefCell::new(Vec::new()));
    let recorded = observed.clone();
    register_ready(second.scheduling_source(), move || {
        recorded.borrow_mut().push(0);
        Ok(())
    });
    assert!(second.prepare(crate::phases::BACKGROUND).is_ok());
    let recorded = observed.clone();
    register_ready(first.scheduling_source(), move || {
        recorded.borrow_mut().push(1);
        Ok(())
    });
    assert!(first.prepare(crate::phases::BACKGROUND).is_ok());
    let expected = Rc::new(Cell::new(9_007_199_254_740_993));
    let failure = expected.clone();
    register_ready(second.scheduling_source(), move || {
        Err(OtherFailure(Failure::Payload(failure)))
    });
    assert!(second.prepare(crate::phases::BACKGROUND).is_ok());
    let recorded = observed.clone();
    register_ready(first.scheduling_source(), move || {
        recorded.borrow_mut().push(3);
        Ok(())
    });
    let contexts = tsonic_rust_runtime::dispatch::prepend(
        &first,
        tsonic_rust_runtime::dispatch::prepend(&second, DispatchEnd::<Failure>::new()),
    );
    let returned =
        match tsonic_rust_runtime::dispatch::poll_phase(&contexts, crate::phases::BACKGROUND) {
            Err(error) => error,
            Ok(_) => panic!("source callback must fail"),
        };
    let Failure::Payload(returned) = returned else {
        panic!("exact source error domain must survive");
    };
    assert!(Rc::ptr_eq(&returned, &expected));
    assert_eq!(returned.get(), 9_007_199_254_740_993);
    assert_eq!(&*observed.borrow(), &[0, 1]);
    assert!(first.has_pending_work());
    assert!(!second.has_pending_work());
    assert!(
        tsonic_rust_runtime::dispatch::poll_phase(&contexts, crate::phases::BACKGROUND).is_ok()
    );
    assert_eq!(&*observed.borrow(), &[0, 1, 3]);
    assert_eq!(background_task_budget().pending(), 0);
}

#[test]
fn composed_phase_prepares_every_domain_before_reentrant_dispatch() {
    use tsonic_rust_runtime::dispatch::DispatchEnd;
    let first = BackgroundTasks::<Failure>::new();
    let second = Rc::new(BackgroundTasks::<OtherFailure>::new());
    let reentrant = second.clone();
    let observed = Rc::new(Cell::new(0));
    let recorded = observed.clone();
    register_ready(first.scheduling_source(), move || {
        register_ready(reentrant.scheduling_source(), move || {
            recorded.set(2);
            Ok(())
        });
        assert!(reentrant.poll().is_err());
        Ok(())
    });
    register_ready(second.scheduling_source(), || {
        Err(OtherFailure(Failure::Payload(Rc::new(Cell::new(1)))))
    });
    let contexts = tsonic_rust_runtime::dispatch::prepend(
        &first,
        tsonic_rust_runtime::dispatch::prepend(&*second, DispatchEnd::<Failure>::new()),
    );
    assert!(
        tsonic_rust_runtime::dispatch::poll_phase(&contexts, crate::phases::BACKGROUND).is_ok()
    );
    assert_eq!(observed.get(), 0);
    assert!(second.has_pending_work());
    assert!(
        tsonic_rust_runtime::dispatch::poll_phase(&contexts, crate::phases::BACKGROUND).is_ok()
    );
    assert_eq!(observed.get(), 2);
    assert_eq!(background_task_budget().pending(), 0);
}

#[test]
fn native_node_driver_transports_exact_component_background_errors() {
    use tsonic_rust_runtime::dispatch::DispatchEnd;
    let first = BackgroundTasks::<Failure>::new();
    let second = BackgroundTasks::<OtherFailure>::new();
    let expected = Rc::new(Cell::new(9_007_199_254_740_993));
    let failure = expected.clone();
    let observed = Rc::new(Cell::new(0));
    let recorded = observed.clone();
    let (release, blocked) = std::sync::mpsc::sync_channel(1);
    let contexts = tsonic_rust_runtime::dispatch::prepend(
        &first,
        tsonic_rust_runtime::dispatch::prepend(&second, DispatchEnd::<Failure>::new()),
    );
    let returned = match crate::run_entry_with_contexts(
        || {
            first
                .spawn(
                    move || {
                        blocked
                            .recv_timeout(std::time::Duration::from_secs(5))
                            .unwrap();
                        Ok(())
                    },
                    move |_| Err(Failure::Payload(failure)),
                )
                .unwrap();
            second
                .spawn(
                    || Ok(()),
                    move |_| {
                        recorded.set(1);
                        release.send(()).unwrap();
                        Ok(())
                    },
                )
                .unwrap();
        },
        &contexts,
    ) {
        Err(error) => error,
        Ok(_) => panic!("actual native callback must fail"),
    };
    let Failure::Payload(returned) = returned else {
        panic!("native driver must not format source errors");
    };
    assert!(Rc::ptr_eq(&returned, &expected));
    assert_eq!(returned.get(), 9_007_199_254_740_993);
    assert!(crate::run_with_contexts(&contexts).is_ok());
    assert_eq!(observed.get(), 1);
    assert_eq!(background_task_budget().pending(), 0);
}

#[test]
fn native_async_driver_drops_its_root_future_on_exact_callback_failure() {
    use tsonic_rust_runtime::dispatch::DispatchEnd;
    struct PendingRoot(Rc<Cell<usize>>);
    impl std::future::Future for PendingRoot {
        type Output = usize;
        fn poll(
            self: std::pin::Pin<&mut Self>,
            _context: &mut std::task::Context<'_>,
        ) -> std::task::Poll<usize> {
            std::task::Poll::Pending
        }
    }
    impl Drop for PendingRoot {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let root = BackgroundTasks::<Failure>::new();
    let expected = Rc::new(Cell::new(9_007_199_254_740_993));
    let failure = expected.clone();
    let released = Rc::new(Cell::new(0));
    let contexts = tsonic_rust_runtime::dispatch::prepend(&root, DispatchEnd::<Failure>::new());
    let future = async {
        root.spawn(|| Ok(()), move |_| Err(Failure::Payload(failure)))
            .unwrap();
        PendingRoot(released.clone()).await
    };
    let returned = match crate::block_on_with_contexts(future, contexts) {
        Err(error) => error,
        Ok(_) => panic!("native background callback must interrupt the root future"),
    };
    let Failure::Payload(returned) = returned else {
        panic!("source error type must survive native async driving");
    };
    assert!(Rc::ptr_eq(&returned, &expected));
    assert_eq!(released.get(), 1);
    assert_eq!(background_task_budget().pending(), 0);
}

#[test]
fn background_epoch_admission_uses_weak_source_identity_and_the_active_owner() {
    use tsonic_rust_runtime::dispatch::{prepend, DispatchContexts, DispatchEnd, DispatchOwner};
    let tasks = BackgroundTasks::<TsonicError>::new();
    let other = BackgroundTasks::<TsonicError>::new();
    let old = tasks.handle();
    let foreign = other.handle();
    assert!(!old.source.ptr_eq(&foreign.source));
    assert_eq!(
        old.spawn(|| Ok(()), |_| Ok(())).unwrap_err().code(),
        "ERR_NODE_BACKGROUND_CLOSED"
    );
    let observed = Rc::new(Cell::new(0));
    let recorded = Rc::clone(&observed);
    crate::run_entry_with_contexts(
        || {
            let current = tsonic_rust_js::event_loop::require_dispatch_owner();
            let wrong = DispatchOwner::new().unwrap();
            assert_ne!(current, wrong);
            assert!(!tasks.enter(wrong));
            tasks.close(wrong);
            assert!(tasks
                .source
                .get()
                .unwrap()
                .borrow()
                .generation
                .owns(current));
            assert_eq!(
                foreign.spawn(|| Ok(()), |_| Ok(())).unwrap_err().code(),
                "ERR_NODE_BACKGROUND_CLOSED"
            );
            old.spawn(
                || Ok(7),
                move |result| {
                    recorded.set(result?);
                    Ok(())
                },
            )
            .unwrap();
        },
        prepend(&tasks, DispatchEnd::<TsonicError>::new()),
    )
    .unwrap();
    assert_eq!(observed.get(), 7);
    assert!(tasks.source.get().unwrap().borrow().channel.is_none());
    assert_eq!(background_task_budget().pending(), 0);
    let next = tasks.handle();
    assert!(old.source.ptr_eq(&next.source));
    assert_ne!(old.generation, next.generation);
    let recorded = Rc::clone(&observed);
    crate::run_entry_with_contexts(
        || {
            assert_eq!(
                old.spawn(|| Ok(()), |_| Ok(())).unwrap_err().code(),
                "ERR_NODE_BACKGROUND_CLOSED"
            );
            next.spawn(
                || Ok(11),
                move |result| {
                    recorded.set(result?);
                    Ok(())
                },
            )
            .unwrap();
        },
        prepend(&tasks, DispatchEnd::<TsonicError>::new()),
    )
    .unwrap();
    assert_eq!(observed.get(), 11);
    assert_eq!(Rc::strong_count(tasks.source.get().unwrap()), 1);
    drop(tasks);
    assert!(old.source.upgrade().is_none());
    assert!(next.source.upgrade().is_none());
    let replacement = BackgroundTasks::<TsonicError>::new();
    let replacement_handle = replacement.handle();
    assert!(!old.source.ptr_eq(&replacement_handle.source));
    assert_eq!(
        old.spawn(|| Ok(()), |_| Ok(())).unwrap_err().code(),
        "ERR_NODE_BACKGROUND_CLOSED"
    );
    assert_eq!(background_task_budget().pending(), 0);
}

#[test]
fn background_epoch_shutdown_drops_captures_once_outside_the_registry_borrow() {
    use tsonic_rust_runtime::dispatch::{prepend, DispatchContexts, DispatchEnd};
    struct Capture {
        handle: crate::background::BackgroundHandle<TsonicError>,
        drops: Rc<Cell<usize>>,
        thread: std::thread::ThreadId,
    }
    impl Drop for Capture {
        fn drop(&mut self) {
            assert_eq!(std::thread::current().id(), self.thread);
            assert_eq!(
                self.handle.spawn(|| Ok(()), |_| Ok(())).unwrap_err().code(),
                "ERR_NODE_BACKGROUND_CLOSED"
            );
            let source = self.handle.source.upgrade().unwrap();
            let source = source.borrow();
            assert_eq!(source.pending(), 0);
            assert!(source.channel.is_none());
            self.drops.set(self.drops.get() + 1);
        }
    }
    for unwind in [false, true] {
        let tasks = BackgroundTasks::<TsonicError>::new();
        let handle = tasks.handle();
        let (release, blocked) = std::sync::mpsc::sync_channel(1);
        let drops = Rc::new(Cell::new(0));
        let capture = Capture {
            handle: handle.clone(),
            drops: Rc::clone(&drops),
            thread: std::thread::current().id(),
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::run_entry_with_contexts(
                || {
                    handle
                        .spawn(
                            move || {
                                blocked
                                    .recv_timeout(std::time::Duration::from_secs(5))
                                    .unwrap();
                                Ok(())
                            },
                            move |_| {
                                drop(capture);
                                panic!("retired capture must never execute")
                            },
                        )
                        .unwrap();
                    assert!(tasks.has_pending_work());
                    if unwind {
                        panic!("native background owner unwind");
                    }
                    tasks.close(tsonic_rust_js::event_loop::require_dispatch_owner());
                },
                prepend(&tasks, DispatchEnd::<TsonicError>::new()),
            )
            .unwrap();
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(drops.get(), 1);
        assert_eq!(Rc::strong_count(&drops), 1);
        assert!(!tasks.has_pending_work());
        assert!(tasks.source.get().unwrap().borrow().channel.is_none());
        assert_eq!(background_task_budget().pending(), 0);
        release.send(()).unwrap();
    }
}

#[test]
fn missing_background_loop_unwinds_before_creating_a_source_or_reserving_work() {
    let tasks = BackgroundTasks::<TsonicError>::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = tasks.spawn(|| Ok(()), |_| Ok(()));
    }));
    let failure = result.expect_err("missing native event loop must unwind");
    let message = failure
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| failure.downcast_ref::<String>().map(String::as_str));
    assert_eq!(
        message,
        Some("No JavaScript event loop is running on this thread.")
    );
    assert!(tasks.source.get().is_none());
    assert_eq!(background_task_budget().pending(), 0);
}
