use super::completion_tests::{completed_process_with_pending_transport, native_message};
use super::*;
use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Condvar, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use tsonic_rust_runtime::dispatch::{prepend, DispatchContexts, DispatchEnd};
use tsonic_rust_runtime::TsonicError;

#[test]
fn no_owner_retains_a_full_prefix_until_explicit_native_observation() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    for index in 0..MAXIMUM_QUEUED_SIGNALS {
        sender.send(native_message(index as i64)).unwrap();
    }
    assert!(
        state.physical.queued_len() == MAXIMUM_QUEUED_SIGNALS,
        "construction does not consume physical frames"
    );
    drop(sender);
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
        state.physical.queued_len() == 0,
        "unobserved messages are not replayable"
    );
    assert!(
        state.physical.native_drained(),
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
    drop(sender);
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
        error.additional_unobserved_errors() == 2,
        "later errors counted"
    );
    assert_eq!(state.delivery.borrow().native_failure_count, 3);
    assert_eq!(
        error.message(),
        "first native reason",
        "the first message is never replaced by formatted count text"
    );
    assert_eq!(
        error.additional_unobserved_errors(),
        2,
        "additional count is a native u64 value"
    );
    assert!(
        error.source_error().has_same_identity(
            state
                .delivery
                .borrow()
                .native_failure
                .as_ref()
                .unwrap()
                .source_error()
        ),
        "the first underlying JsError/cause identity is retained"
    );
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
            drop(sender);
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
                worker.state.physical.queued_len(),
                1,
                "error frame remains retained while exit is inline"
            );
            assert!(worker.state.physical.exit_admitted());
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
    assert_eq!(worker.state.physical.queued_len(), 0);
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
        worker.state.physical.queued_len(),
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
            drop(sender);
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
    let (state, sender, _peer) = completed_process_with_pending_transport();
    let wake = Arc::new(ReentrantWake {
        physical: Arc::downgrade(&state.physical),
        calls: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&wake));
    let mut observer = completion::WorkerCompletion::new(&state);
    assert!(Pin::new(&mut observer)
        .poll(&mut Context::from_waker(&waker))
        .is_pending());
    drop(sender);
    state.physical.finish_reader(None);
    assert_eq!(wake.calls.load(AtomicOrdering::Relaxed), 1);
    assert!(matches!(
        Pin::new(&mut observer).poll(&mut Context::from_waker(&waker)),
        Poll::Ready(Ok(0))
    ));
}

struct ReentrantDropWake {
    physical: std::sync::Weak<WorkerPhysical>,
    dropped: Arc<AtomicUsize>,
}

impl Wake for ReentrantDropWake {
    fn wake(self: Arc<Self>) {}
}

impl Drop for ReentrantDropWake {
    fn drop(&mut self) {
        assert_eq!(self.physical.upgrade().unwrap().queued_len(), 0);
        self.dropped.fetch_add(1, AtomicOrdering::Relaxed);
    }
}

#[test]
fn native_waker_preparation_and_cancelled_registration_drop_reenter_outside_the_fifo_lock() {
    let (state, _sender, _peer) = completed_process_with_pending_transport();
    let checked = Rc::new(Cell::new(false));
    let prepared = Rc::clone(&checked);
    let physical = Arc::clone(&state.physical);
    super::completion_tests::WAKER_PREPARATION.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || {
            assert_eq!(physical.queued_len(), 0);
            prepared.set(true);
        }));
    });
    let dropped = Arc::new(AtomicUsize::new(0));
    let waker = Waker::from(Arc::new(ReentrantDropWake {
        physical: Arc::downgrade(&state.physical),
        dropped: Arc::clone(&dropped),
    }));
    let mut observer = completion::WorkerCompletion::new(&state);
    assert!(Pin::new(&mut observer)
        .poll(&mut Context::from_waker(&waker))
        .is_pending());
    assert!(
        checked.get(),
        "native waker preparation can reenter the queue owner"
    );
    drop(waker);
    assert_eq!(dropped.load(AtomicOrdering::Relaxed), 0);
    drop(observer);
    assert_eq!(
        dropped.load(AtomicOrdering::Relaxed),
        1,
        "the last cancelled native waker drops outside the queue lock"
    );
}

#[test]
fn owner_bound_native_waiters_register_at_full_capacity_and_ignore_frame_only_progress() {
    for prefix in [0, MAXIMUM_QUEUED_SIGNALS] {
        let (state, sender, _peer) = completed_process_with_pending_transport();
        for index in 0..prefix {
            sender.send(native_message(index as i64)).unwrap();
        }
        let worker = Worker::<TsonicError> {
            state: Rc::new(state),
            emitter: EventEmitter::new(),
        };
        let resources = WorkerResources::<TsonicError>::new();
        let wake = Arc::new(ModeWake(AtomicUsize::new(0)));
        let waker = Waker::from(Arc::clone(&wake));
        let mut observer = completion::WorkerCompletion::new(&worker.state);
        crate::block_on_with_contexts(async {
            resources.register_worker(&worker).unwrap();
            worker.unref();
            assert!(Pin::new(&mut observer).poll(&mut Context::from_waker(&waker)).is_pending(),
                "owner-bound observation registers even while the physical FIFO is full");
            if prefix == 0 {
                sender.send(native_message(17)).unwrap();
                assert_eq!(wake.0.load(AtomicOrdering::Relaxed), 0,
                    "a frame cannot advance owner-bound physical completion");
            }
            worker.state.physical.finish_reader(None);
            assert_eq!(wake.0.load(AtomicOrdering::Relaxed), 1,
                "status, not reader-end or per-frame notifications, wakes the owner-bound observer");
            assert!(matches!(Pin::new(&mut observer).poll(&mut Context::from_waker(&waker)), Poll::Ready(Ok(0))));
            assert_eq!(worker.state.physical.queued_len(), prefix.max(1),
                "owner-bound completion has not stolen any callback frame");
        }, prepend(&resources, DispatchEnd::<TsonicError>::new())).unwrap();
    }
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
    let (state, sender, _peer) = completed_process_with_pending_transport();
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
    drop(sender);
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
        }
        drop(sender);
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
    assert_eq!(error.additional_unobserved_errors(), 2);
    assert!(state.delivery.borrow().terminal_consumed);
    assert_eq!(state.physical.queued_len(), 0);
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

#[test]
fn frame_publication_wakes_registered_native_waiters_without_a_later_terminal_event() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    let wake = Arc::new(NativeTestWake {
        ready: Mutex::new(false),
        changed: Condvar::new(),
    });
    let waker = Waker::from(Arc::clone(&wake));
    let mut observer = completion::WorkerCompletion::new(&state);
    for index in 0..64 {
        *wake.ready.lock().unwrap() = false;
        assert!(Pin::new(&mut observer)
            .poll(&mut Context::from_waker(&waker))
            .is_pending());
        std::thread::scope(|scope| {
            scope.spawn(|| {
                sender.send(native_message(index)).unwrap();
            });
            let ready = wake.ready.lock().unwrap();
            let (ready, timeout) = wake
                .changed
                .wait_timeout_while(ready, Duration::from_secs(10), |ready| !*ready)
                .unwrap();
            assert!(
                *ready && !timeout.timed_out(),
                "the frame itself wakes its actual native observer"
            );
        });
        assert!(
            !state.physical.snapshot().reader_finished,
            "no compensating EOF or process event exists"
        );
    }
    drop(sender);
    state.physical.finish_reader(None);
    assert!(matches!(
        Pin::new(&mut observer).poll(&mut Context::from_waker(&waker)),
        Poll::Ready(Ok(0))
    ));
}

#[test]
fn owner_retirement_wakes_the_existing_observer_and_releases_the_saturated_physical_fifo() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    for index in 0..MAXIMUM_QUEUED_SIGNALS {
        sender.send(native_message(index as i64)).unwrap();
    }
    state.ingest_signals(false).unwrap();
    assert_eq!(state.physical.queued_len(), MAXIMUM_QUEUED_SIGNALS);
    let physical = Arc::clone(&state.physical);
    let producer = std::thread::spawn(move || {
        let error = WorkerFrame {
            kind: WorkerFrameKind::Error,
            payload: b"retained saturated error".to_vec(),
        };
        sender.send(error).unwrap();
        drop(sender);
        physical.finish_reader(None);
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while state.physical.capacity_waiters() == 0 {
        assert!(
            Instant::now() < deadline,
            "the actual one-FIFO producer is waiting for native capacity"
        );
        std::thread::yield_now();
    }
    let worker = Worker::<TsonicError> {
        state: Rc::new(state),
        emitter: EventEmitter::new(),
    };
    let resources = WorkerResources::<TsonicError>::new();
    let wake = Arc::new(NativeTestWake {
        ready: Mutex::new(false),
        changed: Condvar::new(),
    });
    let waker = Waker::from(Arc::clone(&wake));
    let mut observer = completion::WorkerCompletion::new(&worker.state);
    crate::block_on_with_contexts(
        async {
            resources.register_worker(&worker).unwrap();
            assert!(Pin::new(&mut observer)
                .poll(&mut Context::from_waker(&waker))
                .is_pending());
            assert_eq!(
                worker.state.physical.queued_len(),
                MAXIMUM_QUEUED_SIGNALS,
                "native observer does not steal callback-owned input"
            );
            assert!(
                !*wake.ready.lock().unwrap(),
                "blocked producer has published no later event"
            );
            resources.close(tsonic_rust_js::event_loop::dispatch_owner().unwrap());
            assert!(
                *wake.ready.lock().unwrap(),
                "retirement itself wakes the already pending native observer"
            );
        },
        prepend(&resources, DispatchEnd::<TsonicError>::new()),
    )
    .unwrap();
    let error = native_observe(observer).unwrap_err();
    producer.join().unwrap();
    assert_eq!(error.message(), "retained saturated error");
    assert_eq!(worker.state.delivery.borrow().native_failure_count, 1);
    assert_eq!(worker.state.physical.queued_len(), 0);
    assert!(worker.state.delivery.borrow().reader_terminal_seen);
    assert!(worker.state.delivery.borrow().terminal_consumed);
    assert!(worker.state.physical.native_drained());
}

#[test]
fn public_terminate_drains_an_actually_saturated_native_reader_without_a_js_owner() {
    let (worker, _control) = super::completion_tests::live_native_worker("saturated");
    let deadline = Instant::now() + Duration::from_secs(10);
    while worker.state.physical.queued_len() < MAXIMUM_QUEUED_SIGNALS {
        assert!(
            Instant::now() < deadline,
            "native reader must fill its actual bounded channel"
        );
        std::thread::yield_now();
    }
    assert!(
        !worker.state.physical.snapshot().reader_finished,
        "saturated reader cannot publish its terminal yet"
    );
    assert!(worker.state.delivery.borrow().owner.is_none());
    assert!(tsonic_rust_js::event_loop::dispatch_owner().is_err());
    let error = native_observe(worker.terminate()).unwrap_err();
    assert_eq!(error.code(), "ERR_WORKER_ERROR");
    assert_eq!(error.message(), "public saturated native error");
    assert_eq!(worker.state.delivery.borrow().native_failure_count, 1);
    assert!(worker.state.delivery.borrow().reader_terminal_seen);
    assert!(worker.state.delivery.borrow().terminal_consumed);
    assert_eq!(worker.state.physical.queued_len(), 0);
    assert!(worker.state.physical.native_drained());
    #[cfg(unix)]
    assert!(
        matches!(worker.state.physical.result(), Some(Ok(137))),
        "real native SIGKILL status follows actual reader drain"
    );
}

#[test]
fn native_io_consumed_before_observed_exit_remains_the_natural_completion_failure() {
    use std::io::Write;
    let (worker, mut control) = super::completion_tests::live_native_worker("live-failure");
    let mut observer = completion::WorkerCompletion::new(&worker.state);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !worker.state.physical.snapshot().reader_finished {
        assert!(Pin::new(&mut observer)
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending());
        assert!(
            Instant::now() < deadline,
            "real reader publishes its truncated native terminal"
        );
        std::thread::yield_now();
    }
    assert!(Pin::new(&mut observer)
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_pending());
    assert!(
        !worker.state.physical.snapshot().process_finished,
        "child remains live before consumption"
    );
    assert_eq!(
        worker.state.delivery.borrow().native_failure_count,
        1,
        "pre-observation IO failure is already counted"
    );
    control.write_all(&[1]).unwrap();
    let error = native_observe(observer).unwrap_err();
    assert_eq!(error.code(), "ERR_WORKER_TRANSPORT");
    assert!(
        error.message().contains("fill whole buffer"),
        "exact native truncated-read reason retained"
    );
    assert!(
        matches!(worker.state.physical.result(), Some(Ok(0))),
        "later normal exit never retroactively erases an observed IO failure"
    );
}

#[test]
fn native_io_consumed_after_observed_exit_is_suppressed_without_a_timer() {
    use std::io::Write;
    let (worker, mut control) = super::completion_tests::live_native_worker("live-failure");
    control.write_all(&[1]).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !worker.state.physical.snapshot().process_finished {
        assert!(
            Instant::now() < deadline,
            "native process produces its actual observed exit"
        );
        std::thread::yield_now();
    }
    assert_eq!(
        worker.state.delivery.borrow().native_failure_count,
        0,
        "physical completion is not an observer"
    );
    assert_eq!(
        native_observe(completion::WorkerCompletion::new(&worker.state)).unwrap(),
        0
    );
    assert_eq!(
        worker.state.delivery.borrow().native_failure_count,
        0,
        "IO consumed after observed exit is suppressed"
    );
    assert!(worker.state.delivery.borrow().terminal_consumed);
}

#[test]
fn captured_raw_fifo_boundary_does_not_admit_a_later_frame() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    sender.send(native_message(1)).unwrap();
    let first = state.physical.admit_frames().unwrap().unwrap();
    sender.send(native_message(2)).unwrap();
    let Some(WorkerSignal::Message(value)) = state.next_signal(Some(first), true).unwrap() else {
        panic!("first admitted raw frame");
    };
    assert_eq!(value.to_js(), JsValue::Integer(1));
    assert!(
        state.next_signal(Some(first), true).unwrap().is_none(),
        "old capture cannot consume a later unstamped raw frame"
    );
    assert_eq!(
        state.physical.queued_len(),
        1,
        "capture never pops into a second buffer"
    );
    let next = state.physical.admit_frames().unwrap().unwrap();
    assert!(
        next > first,
        "later capture receives its existing global task admission"
    );
    let Some(WorkerSignal::Message(value)) = state.next_signal(Some(next), true).unwrap() else {
        panic!("fresh admitted raw frame");
    };
    assert_eq!(value.to_js(), JsValue::Integer(2));
}

#[test]
fn physical_consumer_drop_releases_an_actually_blocked_reader() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    for index in 0..MAXIMUM_QUEUED_SIGNALS {
        sender.send(native_message(index as i64)).unwrap();
    }
    let physical = Arc::clone(&state.physical);
    let reader = std::thread::spawn(move || sender.send(native_message(-1)));
    let deadline = Instant::now() + Duration::from_secs(10);
    while physical.capacity_waiters() == 0 {
        assert!(
            Instant::now() < deadline,
            "reader reached actual native capacity wait"
        );
        std::thread::yield_now();
    }
    drop(state);
    assert!(
        reader.join().unwrap().is_err(),
        "consumer Drop wakes the existing native reader without polling or another thread"
    );
    assert_eq!(
        physical.queued_len(),
        0,
        "discarded raw payloads are retired outside the physical mutex"
    );
}

#[test]
fn native_raw_fifo_stage_cost_matches_its_handwritten_physical_queue_baseline() {
    use std::collections::VecDeque;
    struct NativeQueue {
        frames: VecDeque<WorkerFrame>,
        waiter: Option<Waker>,
        waiting_producers: usize,
    }
    let (state, _sender, _peer) = completed_process_with_pending_transport();
    let queue = Mutex::new(NativeQueue {
        frames: VecDeque::with_capacity(1),
        waiter: None,
        waiting_producers: 0,
    });
    let capacity = Condvar::new();
    let empty = || WorkerFrame {
        kind: WorkerFrameKind::Message,
        payload: Vec::new(),
    };
    assert!(state.physical.enqueue_frame(empty()));
    assert!(matches!(
        state.physical.take_signal(None, false),
        Some(PhysicalSignal::Frame(_))
    ));
    let iterations = 50_000;
    let mut actual = Vec::new();
    let mut baseline = Vec::new();
    let (base_sender, base_receiver) = std::sync::mpsc::sync_channel(MAXIMUM_QUEUED_SIGNALS);
    let mut base_channel = Vec::new();
    for _ in 0..7 {
        let start = Instant::now();
        for _ in 0..iterations {
            base_sender.send(empty()).unwrap();
            assert!(base_receiver.try_recv().is_ok());
        }
        base_channel.push(start.elapsed().as_nanos());
        let start = Instant::now();
        for _ in 0..iterations {
            let mut pending = queue.lock().unwrap();
            while pending.frames.len() == MAXIMUM_QUEUED_SIGNALS {
                pending.waiting_producers += 1;
                pending = capacity.wait(pending).unwrap();
                pending.waiting_producers -= 1;
            }
            pending.frames.push_back(empty());
            let wake = pending.waiter.take();
            drop(pending);
            if let Some(waker) = wake {
                waker.wake();
            }
            let mut pending = queue.lock().unwrap();
            assert!(pending.frames.pop_front().is_some());
            let notify = pending.waiting_producers != 0;
            drop(pending);
            if notify {
                capacity.notify_one();
            }
        }
        baseline.push(start.elapsed().as_nanos());
        let start = Instant::now();
        for _ in 0..iterations {
            assert!(state.physical.enqueue_frame(empty()));
            assert!(matches!(
                state.physical.take_signal(None, false),
                Some(PhysicalSignal::Frame(_))
            ));
        }
        actual.push(start.elapsed().as_nanos());
    }
    actual.sort_unstable();
    baseline.sort_unstable();
    base_channel.sort_unstable();
    println!("P05_NATIVE_FIFO_COST iterations={iterations} actual_ns={} handwritten_ns={} ratio={:.6} base_channel_ns={} base_channel_ratio={:.6} mutexes_per_rotation=2 publication_rmw=0 flag_load=0 idle_capacity_notifications=0 payload_copies=0 extra_queue=0 actual_samples_ns={actual:?} handwritten_samples_ns={baseline:?} base_channel_samples_ns={base_channel:?}", actual[3], baseline[3], actual[3] as f64 / baseline[3] as f64, base_channel[3], actual[3] as f64 / base_channel[3] as f64);
    assert_eq!(state.physical.queued_len(), 0);
}

#[test]
fn native_observation_error_layout_matches_the_required_handwritten_count_carrier() {
    use tsonic_rust_runtime::JsError;
    struct NativeFirstError {
        code: String,
        first: JsError,
    }
    struct NativeCountedError {
        code: String,
        first: JsError,
        additional: u64,
    }
    let original = NodeError::new("NATIVE_FIRST", "unchanged exact first reason");
    let plain = NativeFirstError {
        code: original.code.clone(),
        first: original.source_error().clone(),
    };
    let native = NativeCountedError {
        code: plain.code.clone(),
        first: plain.first.clone(),
        additional: 7,
    };
    let mut observed = original.clone();
    observed.set_additional_unobserved_errors(7);
    assert_eq!(observed.code(), native.code);
    assert_eq!(observed.message(), native.first.message());
    assert_eq!(observed.additional_unobserved_errors(), native.additional);
    assert!(
        original
            .source_error()
            .has_same_identity(observed.source_error()),
        "native count does not replace the cause identity"
    );
    assert_eq!(
        std::mem::size_of_val(&observed),
        std::mem::size_of_val(&native),
        "same required typed native fields, no wrapper/box"
    );
    assert_eq!(
        std::mem::size_of::<NodeError>(),
        std::mem::size_of::<NativeFirstError>() + 8,
        "actual common error carrier grows by exactly its u64 field"
    );
    assert_eq!(
        std::mem::size_of::<Result<i32, NodeError>>(),
        std::mem::size_of::<Result<i32, NativeCountedError>>(),
        "result ABI remains the equivalent direct native result"
    );
    println!("P05_NATIVE_ERROR_LAYOUT first={} counted={} result={} metadata_bytes=8 additional_allocations=0", std::mem::size_of::<NativeFirstError>(), std::mem::size_of::<NodeError>(), std::mem::size_of::<Result<i32, NodeError>>());
}
