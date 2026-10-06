use super::{admit_event, Watchers};
use crate::fs::{
    empty_watch_stats, watch_file_with_options, FsWatchEvent, FsWatcher, Stats, WatchCallback,
    WatchFileOptions, WatchInput,
};
use crate::NodeError;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use tsonic_rust_runtime::dispatch::{
    poll_phase, poll_prepared, prepend, DispatchContexts, DispatchPhase,
};
use tsonic_rust_runtime::Callable;

enum Failure {
    Original(Rc<Cell<u64>>),
    Native(NodeError),
}

impl From<NodeError> for Failure {
    fn from(value: NodeError) -> Self {
        Self::Native(value)
    }
}

fn watcher(roots: &Watchers<Failure>, path: &str) -> FsWatcher<Failure> {
    watch_file_with_options(
        roots,
        path,
        WatchFileOptions {
            interval_ms: u64::MAX,
            ..Default::default()
        },
    )
    .unwrap()
}

fn enqueue_stat(watcher: &FsWatcher<Failure>, size: u64) {
    let mut current = empty_watch_stats();
    current.size = size;
    watcher.state.borrow_mut().pending_stat =
        Some((admit_event().unwrap(), current, empty_watch_stats()));
}

#[test]
fn typed_watchers_keep_original_errors_and_other_roots_pending_work() {
    let first = Watchers::<Failure>::new();
    let second = Watchers::<Failure>::new();
    let failing = watcher(&first, "native-original-watch");
    let pending = watcher(&second, "native-pending-watch");
    let original = Rc::new(Cell::new(9_007_199_254_740_993_u64));
    let retained = Rc::clone(&original);
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    failing.state.borrow_mut().callback = Some(WatchCallback::Stat(Callable::new(
        move |(_current, _previous): (Stats, Stats)| Err(Failure::Original(Rc::clone(&retained))),
    )));
    pending.state.borrow_mut().callback = Some(WatchCallback::Stat(Callable::new(
        move |(current, previous): (Stats, Stats)| {
            assert_eq!(current.size, 9_007_199_254_740_993_u64);
            assert_eq!(previous.size, 0);
            observed.set(observed.get() + 1);
            Ok(())
        },
    )));
    enqueue_stat(&failing, 1);
    enqueue_stat(&pending, 9_007_199_254_740_993);
    let contexts = prepend(&first, &second);
    match poll_phase(&contexts, DispatchPhase::Watchers) {
        Err(Failure::Original(returned)) => {
            assert!(Rc::ptr_eq(&original, &returned));
            assert_eq!(returned.get(), 9_007_199_254_740_993);
        }
        Err(Failure::Native(error)) => panic!("unexpected native error: {}", error.code()),
        Ok(_) => panic!("original stat failure was lost"),
    }
    assert_eq!(calls.get(), 0);
    assert!(pending.state.borrow().pending_stat.is_some());
    assert!(matches!(
        poll_phase(&contexts, DispatchPhase::Watchers),
        Ok(true)
    ));
    assert_eq!(calls.get(), 1);
    failing.close();
    pending.close();
}

#[test]
fn stat_callback_reentry_waits_for_the_next_captured_frontier() {
    let roots = Watchers::<Failure>::new();
    let value = watcher(&roots, "native-reentrant-watch");
    let alias = value.clone();
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    value.state.borrow_mut().callback = Some(WatchCallback::Stat(Callable::new(
        move |(_current, _previous)| {
            observed.set(observed.get() + 1);
            if observed.get() == 1 {
                enqueue_stat(&alias, 2);
            } else {
                alias.close();
            }
            Ok(())
        },
    )));
    enqueue_stat(&value, 1);
    let frontier = match roots.prepare(DispatchPhase::Watchers) {
        Ok(value) => value,
        Err(_) => panic!("valid capture failed"),
    };
    assert!(matches!(poll_prepared(&roots, &frontier), Ok(true)));
    assert_eq!(calls.get(), 1);
    assert!(value.state.borrow().pending_stat.is_some());
    assert!(matches!(
        poll_phase(&roots, DispatchPhase::Watchers),
        Ok(true)
    ));
    assert_eq!(calls.get(), 2);
    assert!(value.closed());
    assert!(!roots.has_work());
}

#[test]
fn unwatch_closes_registrations_across_different_native_error_roots() {
    let first = Watchers::<Failure>::new();
    let second = Watchers::<NodeError>::new();
    let left = watcher(&first, "native-shared-stat-path");
    let right = watch_file_with_options(
        &second,
        "native-shared-stat-path",
        WatchFileOptions::default(),
    )
    .unwrap();
    left.state.borrow_mut().callback = Some(WatchCallback::Stat(Callable::new(|_| Ok(()))));
    right.state.borrow_mut().callback = Some(WatchCallback::Stat(Callable::new(|_| Ok(()))));
    first.retain_stat(&left);
    second.retain_stat(&right);
    assert!(first.has_work());
    assert!(second.has_work());
    crate::fs::unwatch_file("native-shared-stat-path");
    assert!(left.closed());
    assert!(right.closed());
    assert!(!first.has_work());
    assert!(!second.has_work());
    assert!(left.state.borrow().reservation.is_none());
    assert!(right.state.borrow().reservation.is_none());
}

#[test]
fn closing_a_watcher_drops_its_original_capture_outside_native_borrows() {
    struct Probe {
        drops: Rc<Cell<usize>>,
        owner: Rc<RefCell<Option<FsWatcher<Failure>>>>,
    }
    impl Drop for Probe {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
            if let Some(owner) = self.owner.borrow().as_ref() {
                owner.close();
            }
        }
    }
    let roots = Watchers::<Failure>::new();
    let value = watcher(&roots, "native-watch-drop");
    let drops = Rc::new(Cell::new(0));
    let holder = Rc::new(RefCell::new(Some(value.clone())));
    let probe = Probe {
        drops: Rc::clone(&drops),
        owner: Rc::clone(&holder),
    };
    let callback = Callable::new(move |(_current, _previous)| {
        std::hint::black_box(&probe);
        Ok(())
    });
    let identity = callback.identity_key();
    value.state.borrow_mut().callback = Some(WatchCallback::Stat(callback));
    match &value.state.borrow().callback {
        Some(WatchCallback::Stat(callback)) => assert_eq!(callback.identity_key(), identity),
        _ => panic!("original callback missing"),
    }
    value.close();
    assert_eq!(drops.get(), 1);
    holder.borrow_mut().take();
    assert!(!roots.has_work());
}

#[test]
fn native_notifications_keep_every_path_across_a_finite_decoding_queue() {
    let roots = Watchers::<Failure>::new();
    let value = watcher(&roots, "native-watch-paths");
    let (sender, receiver) = mpsc::sync_channel(2);
    let names = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::clone(&names);
    {
        let mut state = value.state.borrow_mut();
        state.maximum_events = 2;
        state.input = Some(WatchInput {
            receiver,
            overflowed: Arc::new(AtomicBool::new(false)),
        });
        state.callback = Some(WatchCallback::Event(Callable::new(move |(kind, name)| {
            assert_eq!(kind, "change");
            observed.borrow_mut().push(name);
            Ok(())
        })));
    }
    let event = notify::Event::new(notify::EventKind::Any);
    let event = (0..5).fold(event, |event, index| {
        event.add_path(PathBuf::from(format!("path-{index}")))
    });
    sender.send(Ok(event)).unwrap();
    for count in [2, 4, 5] {
        assert!(matches!(
            poll_phase(&roots, DispatchPhase::Watchers),
            Ok(true)
        ));
        assert_eq!(names.borrow().len(), count);
    }
    assert_eq!(
        *names.borrow(),
        (0..5)
            .map(|index| format!("path-{index}"))
            .collect::<Vec<_>>()
    );
    assert!(value.state.borrow().pending_notification.is_none());
    value.close();
}

#[test]
fn overflow_is_an_explicit_native_guard_and_pending_events_survive() {
    let roots = Watchers::<Failure>::new();
    let value = watcher(&roots, "native-overflow-watch");
    let (_sender, receiver) = mpsc::sync_channel(1);
    let overflow = Arc::new(AtomicBool::new(true));
    let observed = Rc::new(Cell::new(0));
    let calls = Rc::clone(&observed);
    {
        let mut state = value.state.borrow_mut();
        state.input = Some(WatchInput {
            receiver,
            overflowed: Arc::clone(&overflow),
        });
        state.callback = Some(WatchCallback::Event(Callable::new(move |_| {
            calls.set(calls.get() + 1);
            Ok(())
        })));
        state.pending_events.push_back((
            admit_event().unwrap(),
            FsWatchEvent {
                event_type: "change".to_owned(),
                filename: "retained".to_owned(),
            },
        ));
    }
    match roots.prepare(DispatchPhase::Watchers) {
        Err(Failure::Native(error)) => assert_eq!(error.code(), "ERR_FS_WATCHER_QUEUE_LIMIT"),
        _ => panic!("native overflow guard was lost"),
    }
    assert!(!overflow.load(Ordering::Acquire));
    assert_eq!(observed.get(), 0);
    assert_eq!(value.state.borrow().pending_events.len(), 1);
    assert!(matches!(
        poll_phase(&roots, DispatchPhase::Watchers),
        Ok(true)
    ));
    assert_eq!(observed.get(), 1);
    value.close();
}

#[test]
fn weak_watcher_roots_do_not_extend_native_listener_lifetimes() {
    struct Probe(Rc<Cell<usize>>);
    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let roots = Watchers::<Failure>::new();
    let value = watcher(&roots, "native-weak-watch");
    let drops = Rc::new(Cell::new(0));
    let probe = Probe(Rc::clone(&drops));
    value.state.borrow_mut().callback = Some(WatchCallback::Stat(Callable::new(move |_| {
        std::hint::black_box(&probe);
        Ok(())
    })));
    let alias = value.clone();
    drop(value);
    assert_eq!(drops.get(), 0);
    drop(alias);
    assert_eq!(drops.get(), 1);
    assert!(!roots.has_work());
    assert!(matches!(
        poll_phase(&roots, DispatchPhase::Watchers),
        Ok(false)
    ));
}

#[test]
fn native_watch_queue_and_abort_guards_reject_before_installation() {
    let roots = Watchers::<Failure>::new();
    for limit in [0, super::MAXIMUM_PENDING_EVENTS + 1] {
        let result = crate::fs::watch_with_options(
            &roots,
            "invalid-watch-path",
            crate::fs::WatchOptions {
                max_queue: limit,
                ..Default::default()
            },
        );
        assert_eq!(result.err().unwrap().code(), "ERR_OUT_OF_RANGE");
    }
    let result = crate::fs::watch_with_options(
        &roots,
        "invalid-watch-path",
        crate::fs::WatchOptions {
            signal_aborted: true,
            ..Default::default()
        },
    );
    assert_eq!(result.err().unwrap().code(), "ABORT_ERR");
    assert!(!roots.has_work());
}
