use super::completion_tests::{completed_process_with_pending_transport, native_message};
use super::*;
use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Condvar, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use tsonic_rust_runtime::dispatch::{prepend, DispatchEnd};
use tsonic_rust_runtime::TsonicError;

#[test]
fn no_owner_retains_a_full_prefix_until_explicit_native_observation() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    for index in 0..MAXIMUM_QUEUED_SIGNALS {
        sender.send(native_message(index as i64)).unwrap();
    }
    assert!(
        state.delivery.borrow().pending.is_empty(),
        "construction does not consume physical frames"
    );
    state.physical.finish_reader(None);
    assert!(
        !state.delivery.borrow().terminal_consumed,
        "private physical completion is not an observer"
    );
    let mut observer = completion::WorkerCompletion::new(&state);
    assert!(matches!(
        Pin::new(&mut observer).poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Ok(0))
    ));
    assert!(
        state.delivery.borrow().terminal_consumed,
        "explicit native observer consumes terminal"
    );
    assert!(
        state.delivery.borrow().pending.is_empty(),
        "unobserved messages are not replayable"
    );
    assert!(
        state.incoming.try_recv().is_err(),
        "full prefix consumed by native observation"
    );
}

#[test]
fn ownerless_errors_preserve_the_first_reason_and_count_every_later_error() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    for reason in [
        "first native reason",
        "second native reason",
        "third native reason",
    ] {
        sender
            .send(WorkerFrame {
                kind: WorkerFrameKind::Error,
                payload: reason.as_bytes().to_vec(),
            })
            .unwrap();
    }
    state.physical.finish_reader(None);
    let mut observer = completion::WorkerCompletion::new(&state);
    let Poll::Ready(Err(error)) =
        Pin::new(&mut observer).poll(&mut Context::from_waker(Waker::noop()))
    else {
        panic!("explicit native observation reports unobserved worker errors after join");
    };
    assert_eq!(error.code(), "ERR_WORKER_ERROR");
    assert!(
        error.message().starts_with("first native reason"),
        "first error retained"
    );
    assert!(
        error.message().contains("2 additional"),
        "later errors counted"
    );
    assert_eq!(state.delivery.borrow().native_failure_count, 3);
    assert!(
        matches!(state.physical.result(), Some(Ok(0))),
        "physical join never retroactively faults"
    );
}

#[test]
fn owner_bound_success_does_not_hide_errors_from_a_later_ownerless_observer() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    let worker = Worker::<TsonicError> {
        state: Rc::new(state),
        emitter: EventEmitter::new(),
    };
    let resources = WorkerResources::<TsonicError>::new();
    let (result, _) = crate::block_on_with_contexts(
        async {
            resources.register_worker(&worker).unwrap();
            worker.unref();
            sender
                .send(WorkerFrame {
                    kind: WorkerFrameKind::Error,
                    payload: b"retired native reason".to_vec(),
                })
                .unwrap();
            worker.state.physical.finish_reader(None);
            let mut observer = completion::WorkerCompletion::new(&worker.state);
            assert!(
                matches!(
                    Pin::new(&mut observer).poll(&mut Context::from_waker(Waker::noop())),
                    Poll::Ready(Ok(0))
                ),
                "owner-bound observation leaves callback errors with their owner"
            );
            assert_eq!(
                worker.state.delivery.borrow().pending.len(),
                2,
                "error and exit remain admitted"
            );
        },
        prepend(&resources, DispatchEnd::<TsonicError>::new()),
    )
    .unwrap();
    assert!(result.is_some());
    assert!(
        worker.state.delivery.borrow().owner.is_none(),
        "callback epoch retired"
    );
    let mut later = completion::WorkerCompletion::new(&worker.state);
    let Poll::Ready(Err(error)) =
        Pin::new(&mut later).poll(&mut Context::from_waker(Waker::noop()))
    else {
        panic!("later native observer must consume the retired error");
    };
    assert!(error.message().contains("retired native reason"));
    assert!(worker.state.delivery.borrow().pending.is_empty());
    assert!(worker.state.delivery.borrow().terminal_consumed);
}

#[test]
fn retirement_keeps_uninvoked_frames_for_a_new_explicit_owner() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    let worker = Worker::<TsonicError> {
        state: Rc::new(state),
        emitter: EventEmitter::new(),
    };
    let resources = WorkerResources::<TsonicError>::new();
    let old_calls = Rc::new(Cell::new(0));
    let old = Rc::clone(&old_calls);
    crate::block_on_with_contexts(
        async {
            resources.register_worker(&worker).unwrap();
            worker.unref();
            worker
                .on_callable1(
                    &JsValue::String("message".to_owned()),
                    &Callable::new(move |_: (JsValue,)| {
                        old.set(old.get() + 1);
                        Ok::<(), TsonicError>(())
                    }),
                )
                .unwrap();
            sender.send(native_message(i64::MAX)).unwrap();
            worker.capture().unwrap();
        },
        prepend(&resources, DispatchEnd::<TsonicError>::new()),
    )
    .unwrap();
    assert_eq!(old_calls.get(), 0, "retired admitted callback never runs");
    assert_eq!(
        worker.state.delivery.borrow().pending.len(),
        1,
        "retirement did not pop physical data"
    );
    let new_calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&new_calls);
    crate::block_on_with_contexts(
        async {
            worker.ref_chain();
            worker
                .on_callable1(
                    &JsValue::String("message".to_owned()),
                    &Callable::new(move |(value,): (JsValue,)| {
                        assert_eq!(value, JsValue::Integer(i64::MAX));
                        observed.set(observed.get() + 1);
                        Ok::<(), TsonicError>(())
                    }),
                )
                .unwrap();
            worker.state.physical.finish_reader(None);
        },
        prepend(&resources, DispatchEnd::<TsonicError>::new()),
    )
    .unwrap();
    assert_eq!(old_calls.get(), 0);
    assert_eq!(
        new_calls.get(),
        1,
        "new owner sees complete retained prefix exactly once"
    );
}

struct ReentrantWake {
    physical: std::sync::Weak<WorkerPhysical>,
    calls: AtomicUsize,
}

impl Wake for ReentrantWake {
    fn wake(self: Arc<Self>) {
        assert!(
            self.physical.upgrade().unwrap().snapshot().reader_finished,
            "native waker can re-enter physical owner outside its mutex"
        );
        self.calls.fetch_add(1, AtomicOrdering::Relaxed);
    }
}

#[test]
fn native_readiness_invokes_reentrant_wakers_outside_the_physical_mutex() {
    let (state, _sender, _peer) = completed_process_with_pending_transport();
    let wake = Arc::new(ReentrantWake {
        physical: Arc::downgrade(&state.physical),
        calls: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&wake));
    let mut observer = completion::WorkerCompletion::new(&state);
    assert!(Pin::new(&mut observer)
        .poll(&mut Context::from_waker(&waker))
        .is_pending());
    state.physical.finish_reader(None);
    assert_eq!(wake.calls.load(AtomicOrdering::Relaxed), 1);
    assert!(matches!(
        Pin::new(&mut observer).poll(&mut Context::from_waker(&waker)),
        Poll::Ready(Ok(0))
    ));
}

struct NativeTestWake {
    ready: Mutex<bool>,
    changed: Condvar,
}

struct ModeWake(AtomicUsize);

impl Wake for ModeWake {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, AtomicOrdering::Relaxed);
    }
}

#[test]
fn retiring_callback_ownership_wakes_a_pending_native_observer_without_new_io() {
    let (state, _sender, _peer) = completed_process_with_pending_transport();
    let worker = Worker::<TsonicError> {
        state: Rc::new(state),
        emitter: EventEmitter::new(),
    };
    let resources = WorkerResources::<TsonicError>::new();
    let wake = Arc::new(ModeWake(AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&wake));
    let mut observer = completion::WorkerCompletion::new(&worker.state);
    crate::block_on_with_contexts(
        async {
            resources.register_worker(&worker).unwrap();
            worker.unref();
            assert!(Pin::new(&mut observer)
                .poll(&mut Context::from_waker(&waker))
                .is_pending());
            assert_eq!(
                wake.0.load(AtomicOrdering::Relaxed),
                0,
                "no premature native readiness"
            );
        },
        prepend(&resources, DispatchEnd::<TsonicError>::new()),
    )
    .unwrap();
    assert_eq!(
        wake.0.load(AtomicOrdering::Relaxed),
        1,
        "native progress must learn that callback delivery no longer owns its retained input"
    );
    assert!(
        !worker.state.physical.snapshot().reader_finished,
        "no transport event manufactured the mode wake"
    );
    assert!(Pin::new(&mut observer)
        .poll(&mut Context::from_waker(&waker))
        .is_pending());
    worker.state.physical.finish_reader(None);
    assert!(matches!(
        Pin::new(&mut observer).poll(&mut Context::from_waker(&waker)),
        Poll::Ready(Ok(0))
    ));
}

impl Wake for NativeTestWake {
    fn wake(self: Arc<Self>) {
        *self.ready.lock().unwrap() = true;
        self.changed.notify_one();
    }
}

fn native_observe<T>(future: impl Future<Output = T>) -> T {
    assert!(
        tsonic_rust_js::event_loop::dispatch_owner().is_err(),
        "ordinary executor has no JS owner"
    );
    let wake = Arc::new(NativeTestWake {
        ready: Mutex::new(false),
        changed: Condvar::new(),
    });
    let waker = Waker::from(Arc::clone(&wake));
    let mut future = std::pin::pin!(future);
    loop {
        *wake.ready.lock().unwrap() = false;
        if let Poll::Ready(result) = future.as_mut().poll(&mut Context::from_waker(&waker)) {
            return result;
        }
        let ready = wake.ready.lock().unwrap();
        let (ready, timeout) = wake
            .changed
            .wait_timeout_while(ready, Duration::from_secs(10), |ready| !*ready)
            .unwrap();
        assert!(
            *ready && !timeout.timed_out(),
            "actual native producer must wake the waiter"
        );
    }
}

#[test]
fn ownerless_native_progress_drains_more_than_twice_capacity_without_losing_errors() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    let physical = Arc::clone(&state.physical);
    let producer = std::thread::spawn(move || {
        for index in 0..(MAXIMUM_QUEUED_SIGNALS * 2 + 17) {
            if index == 0 || index == MAXIMUM_QUEUED_SIGNALS || index == MAXIMUM_QUEUED_SIGNALS * 2
            {
                sender
                    .send(WorkerFrame {
                        kind: WorkerFrameKind::Error,
                        payload: format!("native error at {index}").into_bytes(),
                    })
                    .unwrap();
            }
            sender.send(native_message(index as i64)).unwrap();
            physical.published();
        }
        physical.finish_reader(None);
    });
    let error = native_observe(completion::WorkerCompletion::new(&state)).unwrap_err();
    producer.join().unwrap();
    assert!(error.message().starts_with("native error at 0"));
    assert_eq!(
        state.delivery.borrow().native_failure_count,
        3,
        "errors at every pressure level retained"
    );
    assert!(error.message().contains("2 additional"));
    assert!(state.delivery.borrow().terminal_consumed);
    assert!(state.delivery.borrow().pending.is_empty());
    assert!(
        matches!(state.physical.result(), Some(Ok(0))),
        "native error is reported only after actual join"
    );
}

#[test]
fn reader_eof_before_process_exit_wakes_native_progress_but_is_not_completion() {
    use std::io::Write;
    let (worker, mut control) = super::completion_tests::live_native_worker("live-failure");
    let wake = Arc::new(NativeTestWake {
        ready: Mutex::new(false),
        changed: Condvar::new(),
    });
    let waker = Waker::from(Arc::clone(&wake));
    let mut observer = completion::WorkerCompletion::new(&worker.state);
    loop {
        *wake.ready.lock().unwrap() = false;
        assert!(
            Pin::new(&mut observer)
                .poll(&mut Context::from_waker(&waker))
                .is_pending(),
            "EOF does not pretend that a live process exited"
        );
        if worker.state.physical.snapshot().reader_finished {
            break;
        }
        let ready = wake.ready.lock().unwrap();
        let (ready, timeout) = wake
            .changed
            .wait_timeout_while(ready, Duration::from_secs(10), |ready| !*ready)
            .unwrap();
        assert!(
            *ready && !timeout.timed_out(),
            "reader EOF provides actual native notification"
        );
    }
    assert!(
        !worker.state.physical.snapshot().process_finished,
        "process held beyond real reader EOF"
    );
    control.write_all(&[1]).unwrap();
    let error = native_observe(observer).unwrap_err();
    assert_eq!(error.code(), "ERR_WORKER_TRANSPORT");
    assert!(
        worker.state.physical.snapshot().process_finished,
        "native process producer wakes final completion"
    );
}
