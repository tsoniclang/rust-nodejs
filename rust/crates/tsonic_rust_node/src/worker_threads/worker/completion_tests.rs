use super::*;
use std::future::Future;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};

thread_local! {
    static INGESTION_GAP: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
    static OBSERVATION_GAP: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
    pub(super) static WAKER_PREPARATION: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
}

pub(super) fn at_ingestion_gap() {
    let hook = INGESTION_GAP.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook();
    }
}

pub(super) fn at_observation_gap() {
    let hook = OBSERVATION_GAP.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook();
    }
}

pub(super) fn at_waker_preparation() {
    let hook = WAKER_PREPARATION.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook();
    }
}

pub(super) struct NativeFrameProducer(Arc<WorkerPhysical>);

impl NativeFrameProducer {
    pub(super) fn send(&self, frame: WorkerFrame) -> Result<(), ()> {
        self.0.enqueue_frame(frame).then_some(()).ok_or(())
    }
}

pub(super) fn completed_process_with_pending_transport(
) -> (WorkerState, NativeFrameProducer, TcpStream) {
    let child = WorkerChild::spawn(
        Command::new(std::env::current_exe().unwrap())
            .arg("--help")
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )
    .unwrap();
    let status = child.wait().unwrap();
    assert!(status.success(), "native child completed successfully");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (socket, _) = listener.accept().unwrap();
    let (transport, unused) = WorkerTransport::start(socket).unwrap();
    drop(unused);
    let physical = Arc::new(WorkerPhysical::new(child).unwrap());
    let sender = NativeFrameProducer(Arc::clone(&physical));
    (
        WorkerState {
            reservation: super::super::resources::reserve_resource().unwrap(),
            physical,
            transport,
            thread_id: 1,
            delivery: RefCell::new(WorkerDelivery::new()),
        },
        sender,
        peer,
    )
}

pub(super) fn native_message(value: i64) -> WorkerFrame {
    WorkerFrame {
        kind: WorkerFrameKind::Message,
        payload: encode(&ClonedValue::from_js(&JsValue::Integer(value)).unwrap()).unwrap(),
    }
}

fn take_admitted(state: &WorkerState) -> Option<WorkerSignal> {
    let boundary = state.physical.admit_frames().unwrap()?;
    state.next_signal(Some(boundary), true).unwrap()
}

#[test]
fn boundary_peer_stop_is_published_before_completion_wakes_and_native_wait() {
    struct ObserveStop {
        physical: std::sync::Weak<WorkerPhysical>,
        observed: Arc<Mutex<Vec<(bool, bool)>>>,
    }
    impl Wake for ObserveStop {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            let physical = self.physical.upgrade().unwrap();
            let snapshot = physical.snapshot();
            self.observed
                .lock()
                .unwrap()
                .push((snapshot.peer_stopped, snapshot.process_finished));
            assert!(physical.suppresses_native_io());
        }
    }
    for kind in [
        None,
        Some(std::io::ErrorKind::ConnectionReset),
        Some(std::io::ErrorKind::BrokenPipe),
    ] {
        let (state, _sender, _peer) = completed_process_with_pending_transport();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let waker = Waker::from(Arc::new(ObserveStop {
            physical: Arc::downgrade(&state.physical),
            observed: Arc::clone(&observed),
        }));
        let mut completion = completion::WorkerCompletion::new(&state);
        assert!(Pin::new(&mut completion)
            .poll(&mut Context::from_waker(&waker))
            .is_pending());
        observed.lock().unwrap().clear();
        state.physical.finish_reader(kind.map(|kind| {
            super::super::protocol::WorkerTransportError::Io {
                error: std::io::Error::from(kind),
                phase: super::super::protocol::WorkerFramePhase::Boundary,
            }
        }));
        let snapshot = state.physical.snapshot();
        assert_eq!(snapshot.reader_io_kind, kind);
        assert!(snapshot.peer_stopped);
        assert_eq!(observed.lock().unwrap().first(), Some(&(true, false)));
        assert!(matches!(
            state.physical.take_signal(None, true),
            Some(completion::PhysicalSignal::ReaderEnd(None))
        ));
        assert!(matches!(
            state.physical.take_signal(None, true),
            Some(completion::PhysicalSignal::Exit(Ok(0)))
        ));
    }
}

#[test]
fn only_proven_epipe_posts_increment_the_native_unread_count() {
    let (state, _sender, _peer) = completed_process_with_pending_transport();
    let worker = Worker::<tsonic_rust_runtime::TsonicError> {
        state: Rc::new(state),
        emitter: EventEmitter::new(),
    };
    worker.post_message(JsValue::Int32(1)).unwrap();
    assert_eq!(
        worker.state.physical.snapshot().unread_posts,
        0,
        "successful kernel writes are not claimed unread"
    );
    worker.state.transport.shutdown_write();
    for expected in 1..=128 {
        worker.post_message(JsValue::Int32(1)).unwrap();
        assert_eq!(
            worker.state.physical.snapshot().unread_posts,
            expected,
            "each proven EPIPE is one unread drop"
        );
    }
    worker.state.physical.finish_reader(None);
    assert_eq!(worker.state.physical.snapshot().unread_posts, 128);
}

#[test]
fn terminal_publication_after_empty_never_overtakes_the_final_frame() {
    for prefix in [0, MAXIMUM_QUEUED_SIGNALS - 1, MAXIMUM_QUEUED_SIGNALS] {
        for error in [false, true] {
            let (state, sender, _peer) = completed_process_with_pending_transport();
            for index in 0..prefix {
                sender.send(native_message(index as i64)).unwrap();
            }
            let physical = Arc::clone(&state.physical);
            let joined = Rc::new(RefCell::new(None));
            let producer_join = Rc::clone(&joined);
            INGESTION_GAP.with(|slot| {
                *slot.borrow_mut() = Some(Box::new(move || {
                    let tail = if error {
                        WorkerFrame {
                            kind: WorkerFrameKind::Error,
                            payload: b"final precise error".to_vec(),
                        }
                    } else {
                        native_message(prefix as i64)
                    };
                    if prefix == MAXIMUM_QUEUED_SIGNALS {
                        let waiting = Arc::clone(&physical);
                        *producer_join.borrow_mut() = Some(std::thread::spawn(move || {
                            sender.send(tail).unwrap();
                            drop(sender);
                            physical.finish_reader(None);
                        }));
                        let deadline = Instant::now() + Duration::from_secs(10);
                        while waiting.capacity_waiters() == 0 {
                            assert!(
                                Instant::now() < deadline,
                                "capacity-plus-one waits on the one native FIFO"
                            );
                            std::thread::yield_now();
                        }
                    } else {
                        sender.send(tail).unwrap();
                        drop(sender);
                        physical.finish_reader(None);
                    }
                }));
            });
            state.ingest_signals(false).unwrap();
            assert!(
                !state.physical.exit_admitted(),
                "temporary Empty or full is not terminal"
            );
            let mut received = 0;
            let mut errors = 0;
            let mut exits = 0;
            loop {
                let next = take_admitted(&state);
                match next {
                    Some(WorkerSignal::Message(value)) => {
                        assert_eq!(value.to_js(), JsValue::Integer(received));
                        received += 1;
                    }
                    Some(WorkerSignal::RemoteError(reason)) => {
                        assert_eq!(received, prefix as i64, "full prefix precedes final error");
                        assert_eq!(reason, "final precise error");
                        errors += 1;
                    }
                    Some(WorkerSignal::Exit(code)) => {
                        assert_eq!(code, 0);
                        assert_eq!(received, (prefix + usize::from(!error)) as i64);
                        assert_eq!(errors, usize::from(error));
                        exits += 1;
                    }
                    Some(_) => panic!("only the exact message/error/exit signals are admitted"),
                    None if exits == 1 => break,
                    None => {}
                }
                state.ingest_signals(false).unwrap();
            }
            if let Some(producer) = joined.borrow_mut().take() {
                producer.join().unwrap();
            }
            assert_eq!(exits, 1, "one terminal after the complete prefix");
        }
    }
}

#[test]
fn native_ready_after_ingestion_rechecks_the_final_error_and_terminal() {
    for at_read_gap in [false, true] {
        let (state, sender, _peer) = completed_process_with_pending_transport();
        let physical = Arc::clone(&state.physical);
        let hook: Box<dyn FnOnce()> = Box::new(move || {
            sender.send(native_message(42)).unwrap();
            sender
                .send(WorkerFrame {
                    kind: WorkerFrameKind::Error,
                    payload: b"tail failure".to_vec(),
                })
                .unwrap();
            drop(sender);
            physical.finish_reader(None);
        });
        if at_read_gap {
            INGESTION_GAP.with(|slot| *slot.borrow_mut() = Some(hook));
        } else {
            OBSERVATION_GAP.with(|slot| *slot.borrow_mut() = Some(hook));
        }
        let mut observer = completion::WorkerCompletion::new(&state);
        let Poll::Ready(Err(error)) =
            Pin::new(&mut observer).poll(&mut Context::from_waker(Waker::noop()))
        else {
            panic!(
                "native Ready must include the final error even when join is published in the gap"
            );
        };
        assert_eq!(error.first().code(), "ERR_WORKER_ERROR");
        assert_eq!(error.first().message(), "tail failure");
        assert_eq!(state.delivery.borrow().native_failure_count(), 1);
        assert!(state.delivery.borrow().reader_terminal_seen);
        assert!(state.delivery.borrow().terminal_consumed);
        assert!(
            state.physical.native_drained(),
            "the single physical owner proves drained terminal"
        );
    }
}

#[test]
fn owner_bound_native_ready_leaves_gap_published_frames_for_actual_callbacks() {
    use std::cell::Cell;
    use tsonic_rust_runtime::dispatch::{prepend, DispatchEnd};
    use tsonic_rust_runtime::TsonicError;
    let (state, sender, _peer) = completed_process_with_pending_transport();
    let worker = Worker::<TsonicError> {
        state: Rc::new(state),
        emitter: EventEmitter::new(),
    };
    let resources = WorkerResources::<TsonicError>::new();
    let messages = Rc::new(Cell::new(0));
    let errors = Rc::new(Cell::new(0));
    let exits = Rc::new(Cell::new(0));
    let observed_messages = Rc::clone(&messages);
    let observed_errors = Rc::clone(&errors);
    let observed_exits = Rc::clone(&exits);
    let before_exit_messages = Rc::clone(&messages);
    let before_exit_errors = Rc::clone(&errors);
    crate::block_on_with_contexts(async {
        resources.register_worker(&worker).unwrap();
        worker.on_callable1(&JsValue::String("message".to_owned()), &Callable::new(move |(value,): (JsValue,)| {
            assert_eq!(value, JsValue::Integer(42));
            observed_messages.set(observed_messages.get() + 1);
            Ok::<(), TsonicError>(())
        })).unwrap();
        worker.on_callable1(&JsValue::String("error".to_owned()), &Callable::new(move |(value,): (JsValue,)| {
            let JsValue::String(reason) = value else { panic!("exact error callback payload"); };
            assert!(reason.ends_with("tail failure"));
            observed_errors.set(observed_errors.get() + 1);
            Ok::<(), TsonicError>(())
        })).unwrap();
        worker.on_callable1(&JsValue::String("exit".to_owned()), &Callable::new(move |(value,): (JsValue,)| {
            assert_eq!(value, JsValue::Int32(0));
            assert_eq!(before_exit_messages.get(), 1);
            assert_eq!(before_exit_errors.get(), 1);
            observed_exits.set(observed_exits.get() + 1);
            Ok::<(), TsonicError>(())
        })).unwrap();
        let physical = Arc::clone(&worker.state.physical);
        OBSERVATION_GAP.with(|slot| *slot.borrow_mut() = Some(Box::new(move || {
            sender.send(native_message(42)).unwrap();
            sender.send(WorkerFrame { kind: WorkerFrameKind::Error, payload: b"tail failure".to_vec() }).unwrap();
            drop(sender);
            physical.finish_reader(None);
        })));
        let mut observer = completion::WorkerCompletion::new(&worker.state);
        assert!(matches!(Pin::new(&mut observer).poll(&mut Context::from_waker(Waker::noop())), Poll::Ready(Ok(0))),
            "owner-bound native observation reports physical status without stealing callback data");
        assert_eq!(worker.state.delivery.borrow().native_failure_count(), 0);
        assert!(!worker.state.delivery.borrow().terminal_consumed);
    }, prepend(&resources, DispatchEnd::<TsonicError>::new())).unwrap();
    assert_eq!(messages.get(), 1);
    assert_eq!(errors.get(), 1);
    assert_eq!(exits.get(), 1);
}

#[test]
fn process_exit_waits_for_every_tail_frame_and_the_reader_terminal_boundary() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    state.ingest_signals(false).unwrap();
    assert!(
        !state.physical.exit_admitted(),
        "process status alone cannot close an undrained native transport"
    );
    assert!(
        state.physical.queued_len() == 0,
        "no premature exit callback"
    );
    for index in 0..64 {
        sender.send(native_message(index)).unwrap();
        state.ingest_signals(false).unwrap();
        assert!(
            !state.physical.exit_admitted(),
            "native reader has not reached its terminal boundary"
        );
    }
    drop(sender);
    state.physical.finish_reader(None);
    state.ingest_signals(false).unwrap();
    assert!(
        state.physical.exit_admitted(),
        "joined native process and transport complete once"
    );
    assert_eq!(
        state.physical.queued_len(),
        64,
        "64 physical frames plus inline exit"
    );
    for index in 0..64 {
        let WorkerSignal::Message(value) = take_admitted(&state).unwrap() else {
            panic!("every final message precedes the exit callback");
        };
        assert_eq!(value.to_js(), JsValue::Integer(index));
    }
    assert!(matches!(take_admitted(&state), Some(WorkerSignal::Exit(0))));
    state.ingest_signals(false).unwrap();
    assert!(take_admitted(&state).is_none(), "one exit callback only");
}

#[test]
fn full_native_signal_queue_drains_before_admitting_the_one_exit_callback() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    for index in 0..MAXIMUM_QUEUED_SIGNALS {
        sender.send(native_message(index as i64)).unwrap();
    }
    drop(sender);
    state.physical.finish_reader(None);
    state.ingest_signals(false).unwrap();
    assert_eq!(state.physical.queued_len(), MAXIMUM_QUEUED_SIGNALS);
    assert!(
        !state.delivery.borrow().terminal_consumed,
        "inline exit cannot bypass the full physical prefix"
    );
    for index in 0..MAXIMUM_QUEUED_SIGNALS {
        let WorkerSignal::Message(value) = take_admitted(&state).unwrap() else {
            panic!("bounded native messages are retained in full");
        };
        assert_eq!(value.to_js(), JsValue::Integer(index as i64));
    }
    state.ingest_signals(false).unwrap();
    assert!(state.physical.exit_admitted());
    assert_eq!(
        state.physical.queued_len(),
        0,
        "one terminal does not consume another frame slot"
    );
    assert!(matches!(take_admitted(&state), Some(WorkerSignal::Exit(0))));
}

#[test]
fn explicit_termination_does_not_bypass_final_transport_admission() {
    use tsonic_rust_runtime::dispatch::{prepend, DispatchEnd};
    use tsonic_rust_runtime::TsonicError;
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
            let mut completion = std::pin::pin!(worker.terminate());
            assert!(completion
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending());
            assert!(
                !worker.state.physical.snapshot().process_finished,
                "termination status is not transport completion"
            );
            sender.send(native_message(i64::MAX)).unwrap();
            drop(sender);
            worker.state.physical.finish_reader(None);
            assert!(matches!(
                completion
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop())),
                Poll::Ready(Ok(0))
            ));
            let WorkerSignal::Message(value) = take_admitted(&worker.state).unwrap() else {
                panic!("owner-bound termination retains final native message");
            };
            assert_eq!(value.to_js(), JsValue::Integer(i64::MAX));
            assert!(matches!(
                take_admitted(&worker.state),
                Some(WorkerSignal::Exit(0))
            ));
            assert!(take_admitted(&worker.state).is_none());
        },
        prepend(&resources, DispatchEnd::<TsonicError>::new()),
    )
    .unwrap();
    assert!(result.is_some(), "explicit owner completed");
}

struct CompletionWake {
    identity: usize,
    invoked: Arc<Mutex<Vec<usize>>>,
}

impl Wake for CompletionWake {
    fn wake(self: Arc<Self>) {
        self.invoked.lock().unwrap().push(self.identity);
    }
}

fn observed_waker(identity: usize, invoked: &Arc<Mutex<Vec<usize>>>) -> Waker {
    Waker::from(Arc::new(CompletionWake {
        identity,
        invoked: Arc::clone(invoked),
    }))
}

#[test]
fn publication_before_native_registration_is_consumed_before_waiting_for_the_next_frame() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    let invoked = Arc::new(Mutex::new(Vec::new()));
    let waker = observed_waker(1, &invoked);
    let first = Arc::clone(&state.physical);
    OBSERVATION_GAP.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || {
            assert!(first.enqueue_frame(native_message(17)));
        }));
    });
    let mut observer = completion::WorkerCompletion::new(&state);
    assert!(Pin::new(&mut observer)
        .poll(&mut Context::from_waker(&waker))
        .is_pending());
    assert_eq!(
        state.physical.queued_len(),
        0,
        "the publication before registration is observed under the queue mutex"
    );
    assert!(
        invoked.lock().unwrap().is_empty(),
        "catch-up consumption needs no artificial self-wake"
    );
    sender.send(native_message(18)).unwrap();
    assert_eq!(
        *invoked.lock().unwrap(),
        [1],
        "the next publication sees the registration under the same queue mutex"
    );
    assert!(Pin::new(&mut observer)
        .poll(&mut Context::from_waker(&waker))
        .is_pending());
    assert_eq!(state.physical.queued_len(), 0);
    assert!(!state.physical.snapshot().reader_finished);
    drop(sender);
    state.physical.finish_reader(None);
    assert!(matches!(
        Pin::new(&mut observer).poll(&mut Context::from_waker(&waker)),
        Poll::Ready(Ok(0))
    ));
}

#[test]
fn native_completion_cancels_replaces_and_wakes_concurrent_waiters_once() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    let invoked = Arc::new(Mutex::new(Vec::new()));
    let first_waker = observed_waker(1, &invoked);
    let second_waker = observed_waker(2, &invoked);
    let replacement = observed_waker(3, &invoked);
    let mut first = completion::WorkerCompletion::new(&state);
    let mut second = completion::WorkerCompletion::new(&state);
    let mut third = completion::WorkerCompletion::new(&state);
    assert!(Pin::new(&mut first)
        .poll(&mut Context::from_waker(&first_waker))
        .is_pending());
    assert!(Pin::new(&mut second)
        .poll(&mut Context::from_waker(&second_waker))
        .is_pending());
    assert!(Pin::new(&mut second)
        .poll(&mut Context::from_waker(&replacement))
        .is_pending());
    assert!(Pin::new(&mut third)
        .poll(&mut Context::from_waker(&second_waker))
        .is_pending());
    drop(first);
    assert!(
        invoked.lock().unwrap().is_empty(),
        "no poll-based or premature completion wake"
    );
    drop(sender);
    state.physical.finish_reader(None);
    assert_eq!(
        *invoked.lock().unwrap(),
        [3, 2],
        "only retained registrations wake in registration order"
    );
    assert!(matches!(
        Pin::new(&mut second).poll(&mut Context::from_waker(&replacement)),
        Poll::Ready(Ok(0))
    ));
    assert!(matches!(
        Pin::new(&mut third).poll(&mut Context::from_waker(&second_waker)),
        Poll::Ready(Ok(0))
    ));
    state.ingest_signals(true).unwrap();
    assert_eq!(
        invoked.lock().unwrap().len(),
        2,
        "native completion wakes once"
    );
}

#[test]
fn native_completion_drops_the_inline_registration_before_reuse() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    let invoked = Arc::new(Mutex::new(Vec::new()));
    let waker = observed_waker(1, &invoked);
    let mut cancelled = completion::WorkerCompletion::new(&state);
    assert!(Pin::new(&mut cancelled)
        .poll(&mut Context::from_waker(&waker))
        .is_pending());
    drop(cancelled);
    let mut current = completion::WorkerCompletion::new(&state);
    assert!(Pin::new(&mut current)
        .poll(&mut Context::from_waker(&waker))
        .is_pending());
    drop(sender);
    state.physical.finish_reader(None);
    assert_eq!(
        *invoked.lock().unwrap(),
        [1],
        "cancelled inline waiter is not retained or reawakened"
    );
    assert!(matches!(
        Pin::new(&mut current).poll(&mut Context::from_waker(&waker)),
        Poll::Ready(Ok(0))
    ));
}

#[test]
fn native_io_after_exit_is_not_a_worker_error_but_protocol_and_clone_failures_remain_errors() {
    for (code, expected_errors) in [("ERR_WORKER_TRANSPORT", 0), ("ERR_WORKER_PROTOCOL", 1)] {
        let (state, sender, _peer) = completed_process_with_pending_transport();
        drop(sender);
        state
            .physical
            .finish_reader(Some(NodeError::new(code, "exact native reason").into()));
        state.ingest_signals(false).unwrap();
        state.ingest_signals(false).unwrap();
        assert!(
            state.physical.exit_admitted(),
            "reader failure never strands native completion"
        );
        let signals: Vec<_> = std::iter::from_fn(|| take_admitted(&state)).collect();
        let errors = signals
            .iter()
            .filter(|signal| matches!(signal, WorkerSignal::Error(_)))
            .count();
        assert_eq!(
            errors, expected_errors,
            "ordinary representation excludes native IO; delivery tested separately"
        );
        if expected_errors != 0 {
            assert!(matches!(signals.first().unwrap(),
                WorkerSignal::Error(error) if error.message().ends_with("exact native reason")));
        }
    }
    let (state, sender, _peer) = completed_process_with_pending_transport();
    sender
        .send(WorkerFrame {
            kind: WorkerFrameKind::Message,
            payload: vec![255],
        })
        .unwrap();
    drop(sender);
    state.physical.finish_reader(None);
    state.ingest_signals(false).unwrap();
    assert!(state.physical.exit_admitted());
    let signals: Vec<_> = std::iter::from_fn(|| take_admitted(&state)).collect();
    assert!(
        matches!(signals.first().unwrap(),
        WorkerSignal::Error(error) if error.message().contains("version is unsupported")),
        "malformed clone is an error signal, not a loop failure"
    );
    assert!(matches!(signals.last(), Some(WorkerSignal::Exit(0))));
}

#[test]
fn native_worker_error_delivery_suppresses_observed_exit_io_but_keeps_protocol_and_clone_errors() {
    use std::cell::Cell;
    use tsonic_rust_runtime::dispatch::{prepend, DispatchEnd};
    use tsonic_rust_runtime::TsonicError;
    for (kind, expected_errors, reason) in [
        ("io", 0, "exact native IO reason"),
        ("protocol", 1, "exact native protocol reason"),
        ("clone", 1, "version is unsupported"),
    ] {
        let (state, sender, _peer) = completed_process_with_pending_transport();
        let worker = Worker::<TsonicError> {
            state: Rc::new(state),
            emitter: EventEmitter::new(),
        };
        let resources = WorkerResources::<TsonicError>::new();
        let errors = Rc::new(RefCell::new(Vec::<String>::new()));
        let exits = Rc::new(Cell::new(0));
        let delivered = Rc::clone(&errors);
        let completed = Rc::clone(&exits);
        let errors_before_exit = Rc::clone(&errors);
        let (result, _) = crate::block_on_with_contexts(
            async {
                resources.register_worker(&worker).unwrap();
                worker
                    .on_callable1(
                        &JsValue::String("error".to_owned()),
                        &Callable::new(move |(value,): (JsValue,)| {
                            let JsValue::String(message) = value else {
                                panic!("precise native error payload");
                            };
                            delivered.borrow_mut().push(message);
                            Ok::<(), TsonicError>(())
                        }),
                    )
                    .unwrap();
                worker
                    .on_callable1(
                        &JsValue::String("exit".to_owned()),
                        &Callable::new(move |(value,): (JsValue,)| {
                            assert!(
                                matches!(value, JsValue::Int32(0)),
                                "{kind}: exact native exit"
                            );
                            assert_eq!(
                                errors_before_exit.borrow().len(),
                                expected_errors,
                                "{kind}: errors precede exit"
                            );
                            completed.set(completed.get() + 1);
                            Ok::<(), TsonicError>(())
                        }),
                    )
                    .unwrap();
                if kind == "clone" {
                    sender
                        .send(WorkerFrame {
                            kind: WorkerFrameKind::Message,
                            payload: vec![255],
                        })
                        .unwrap();
                    drop(sender);
                    worker.state.physical.finish_reader(None);
                } else {
                    drop(sender);
                    worker.state.physical.finish_reader(Some(
                        NodeError::new(
                            if kind == "io" {
                                "ERR_WORKER_TRANSPORT"
                            } else {
                                "ERR_WORKER_PROTOCOL"
                            },
                            reason,
                        )
                        .into(),
                    ));
                }
            },
            prepend(&resources, DispatchEnd::<TsonicError>::new()),
        )
        .unwrap();
        assert!(result.is_some(), "{kind}: selected owner drained");
        assert_eq!(
            errors.borrow().len(),
            expected_errors,
            "{kind}: actual error-listener count"
        );
        if expected_errors != 0 {
            assert!(
                errors.borrow()[0].contains(reason),
                "{kind}: exact reason survives exit"
            );
        }
        assert_eq!(exits.get(), 1, "{kind}: one terminal delivery");
        assert_eq!(worker.state.physical.queued_len(), 0);
    }
}

#[test]
fn native_worker_process_fixture() {
    let Ok(mode) = std::env::var("TSONIC_P05_NATIVE_WORKER_MODE") else {
        return;
    };
    let mut socket =
        TcpStream::connect(std::env::var("TSONIC_P05_NATIVE_WORKER_ADDRESS").unwrap()).unwrap();
    if mode == "saturated" {
        let mut control =
            TcpStream::connect(std::env::var("TSONIC_P05_NATIVE_WORKER_CONTROL").unwrap()).unwrap();
        control.write_all(&[1]).unwrap();
        write_frame(
            &mut socket,
            WorkerFrameKind::Error,
            b"public saturated native error",
        )
        .unwrap();
        for index in 0..MAXIMUM_QUEUED_SIGNALS * 4 {
            write_frame(
                &mut socket,
                WorkerFrameKind::Message,
                &native_message(index as i64).payload,
            )
            .unwrap();
        }
        let mut released = [0];
        control.read_exact(&mut released).unwrap();
        return;
    }
    for index in 0..10 {
        write_frame(
            &mut socket,
            WorkerFrameKind::Message,
            &native_message(index).payload,
        )
        .unwrap();
    }
    if mode == "partial" || mode == "live-failure" {
        socket
            .write_all(&[WorkerFrameKind::Message as u8, 0, 0, 0, 8, 7])
            .unwrap();
        if mode == "live-failure" {
            socket.shutdown(std::net::Shutdown::Write).unwrap();
        }
    }
    let mut control =
        TcpStream::connect(std::env::var("TSONIC_P05_NATIVE_WORKER_CONTROL").unwrap()).unwrap();
    control.write_all(&[1]).unwrap();
    let mut released = [0];
    control.read_exact(&mut released).unwrap();
    if mode == "unread" {
        std::process::exit(0);
    }
}

pub(super) fn live_native_worker(mode: &str) -> (Worker, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let control_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let child = WorkerChild::spawn(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "worker_threads::worker::completion_tests::native_worker_process_fixture",
                "--nocapture",
            ])
            .env("TSONIC_P05_NATIVE_WORKER_MODE", mode)
            .env(
                "TSONIC_P05_NATIVE_WORKER_ADDRESS",
                listener.local_addr().unwrap().to_string(),
            )
            .env(
                "TSONIC_P05_NATIVE_WORKER_CONTROL",
                control_listener.local_addr().unwrap().to_string(),
            )
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )
    .unwrap();
    let physical = Arc::new(WorkerPhysical::new(child).unwrap());
    let (socket, _) = listener.accept().unwrap();
    let frames = Arc::clone(&physical);
    let terminal = Arc::clone(&physical);
    let transport = WorkerTransport::start_observed(
        socket,
        move |frame| frames.enqueue_frame(frame),
        move |error| terminal.finish_reader(error),
    )
    .unwrap();
    let (mut control, _) = control_listener.accept().unwrap();
    control
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut ready = [0];
    control.read_exact(&mut ready).unwrap();
    assert_eq!(
        ready,
        [1],
        "native child wrote retained prefix before ready"
    );
    (
        Worker {
            state: Rc::new(WorkerState {
                reservation: super::super::resources::reserve_resource().unwrap(),
                physical,
                transport,
                thread_id: 1,
                delivery: RefCell::new(WorkerDelivery::new()),
            }),
            emitter: EventEmitter::new(),
        },
        control,
    )
}

#[test]
fn live_native_termination_and_unread_posts_preserve_the_prefix_without_transport_errors() {
    use std::cell::Cell;
    use tsonic_rust_runtime::dispatch::{prepend, DispatchEnd};
    use tsonic_rust_runtime::TsonicError;
    for mode in ["partial", "unread", "live-failure"] {
        let (worker, mut control) = live_native_worker(mode);
        let resources = WorkerResources::<TsonicError>::new();
        let messages = Rc::new(RefCell::new(Vec::new()));
        let errors = Rc::new(RefCell::new(Vec::new()));
        let exits = Rc::new(Cell::new(0));
        let exit_code = Rc::new(Cell::new(None));
        let delivered = Rc::clone(&messages);
        let failures = Rc::clone(&errors);
        let release = control.try_clone().unwrap();
        let observed = Rc::clone(&exit_code);
        let completed = Rc::clone(&exits);
        let prefix = Rc::clone(&messages);
        let (result, _) = crate::block_on_with_contexts(
            async {
                resources.register_worker(&worker).unwrap();
                worker
                    .on_callable1(
                        &JsValue::String("message".to_owned()),
                        &Callable::new(move |(value,): (JsValue,)| {
                            let JsValue::Integer(value) = value else {
                                panic!("native int64 remains exact");
                            };
                            delivered.borrow_mut().push(value);
                            Ok::<(), TsonicError>(())
                        }),
                    )
                    .unwrap();
                worker
                    .on_callable1(
                        &JsValue::String("error".to_owned()),
                        &Callable::new(move |(value,): (JsValue,)| {
                            let JsValue::String(message) = value else {
                                panic!("exact native error message");
                            };
                            failures.borrow_mut().push(message);
                            if mode == "live-failure" {
                                (&release).write_all(&[1]).unwrap();
                            }
                            Ok::<(), TsonicError>(())
                        }),
                    )
                    .unwrap();
                worker
                    .on_callable1(
                        &JsValue::String("exit".to_owned()),
                        &Callable::new(move |(value,): (JsValue,)| {
                            let JsValue::Int32(code) = value else {
                                panic!("native int32 exit");
                            };
                            assert_eq!(
                                *prefix.borrow(),
                                (0..10).collect::<Vec<_>>(),
                                "complete frames precede native exit"
                            );
                            observed.set(Some(code));
                            completed.set(completed.get() + 1);
                            Ok::<(), TsonicError>(())
                        }),
                    )
                    .unwrap();
                if mode == "partial" {
                    let code = worker.terminate().await.unwrap();
                    #[cfg(unix)]
                    assert_eq!(code, 137, "SIGKILL preserves native Unix status");
                    println!("P05 native Rust terminate exit: {code}");
                } else if mode == "unread" {
                    for _ in 0..128 {
                        worker
                            .post_message(JsValue::String("x".repeat(1024)))
                            .unwrap();
                    }
                    control.write_all(&[1]).unwrap();
                }
            },
            prepend(&resources, DispatchEnd::<TsonicError>::new()),
        )
        .unwrap();
        assert!(
            result.is_some(),
            "native root completed without polling fallback"
        );
        assert_eq!(
            errors.borrow().len(),
            usize::from(mode == "live-failure"),
            "{mode}: actual IO delivery"
        );
        assert_eq!(exits.get(), 1, "one exit after complete prefix");
        assert!(exit_code.get().is_some());
        assert!(
            worker.state.delivery.borrow().terminal_consumed,
            "one process/reader terminal owner"
        );
    }
}

#[test]
fn unread_native_peer_exit_reports_only_observable_failed_writes() {
    use std::cell::Cell;
    use tsonic_rust_runtime::dispatch::{prepend, DispatchEnd};
    use tsonic_rust_runtime::TsonicError;
    for failed_writes in [false, true] {
        let (worker, mut control) = live_native_worker("unread");
        let resources = WorkerResources::<TsonicError>::new();
        let messages = Rc::new(RefCell::new(Vec::new()));
        let delivered = Rc::clone(&messages);
        let errors = Rc::new(Cell::new(0));
        let failures = Rc::clone(&errors);
        let exits = Rc::new(Cell::new(0));
        let completed = Rc::clone(&exits);
        let prefix = Rc::clone(&messages);
        crate::block_on_with_contexts(
            async {
                resources.register_worker(&worker).unwrap();
                worker
                    .on_callable1(
                        &JsValue::String("message".to_owned()),
                        &Callable::new(move |(value,): (JsValue,)| {
                            let JsValue::Integer(value) = value else {
                                panic!("exact native int64 prefix");
                            };
                            delivered.borrow_mut().push(value);
                            Ok::<(), TsonicError>(())
                        }),
                    )
                    .unwrap();
                worker
                    .on_callable1(
                        &JsValue::String("error".to_owned()),
                        &Callable::new(move |(_value,): (JsValue,)| {
                            failures.set(failures.get() + 1);
                            Ok::<(), TsonicError>(())
                        }),
                    )
                    .unwrap();
                worker
                    .on_callable1(
                        &JsValue::String("exit".to_owned()),
                        &Callable::new(move |(value,): (JsValue,)| {
                            assert!(
                                matches!(value, JsValue::Int32(0)),
                                "actual child exit status"
                            );
                            assert_eq!(
                                *prefix.borrow(),
                                (0..10).collect::<Vec<_>>(),
                                "completed prefix precedes exit"
                            );
                            completed.set(completed.get() + 1);
                            Ok::<(), TsonicError>(())
                        }),
                    )
                    .unwrap();
                if failed_writes {
                    worker.state.transport.shutdown_write();
                }
                for _ in 0..128 {
                    worker
                        .post_message(JsValue::String("x".repeat(1024)))
                        .unwrap();
                }
                control.write_all(&[1]).unwrap();
            },
            prepend(&resources, DispatchEnd::<TsonicError>::new()),
        )
        .unwrap();
        let terminal = worker.state.physical.snapshot();
        assert_eq!(terminal.unread_posts, if failed_writes { 128 } else { 0 });
        assert!(
            terminal.unread_posts <= 128,
            "fixture ground truth bounds observable unread posts"
        );
        assert_eq!(errors.get(), 0);
        assert_eq!(exits.get(), 1);
        assert!(terminal.peer_stopped);
        println!("CH08 native terminal failed_writes={failed_writes}: kind={:?}, peer-stopped={}, unread={}", terminal.reader_io_kind, terminal.peer_stopped, terminal.unread_posts);
    }
}
