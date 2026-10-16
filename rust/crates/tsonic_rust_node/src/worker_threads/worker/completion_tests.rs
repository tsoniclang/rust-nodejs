use super::*;
use std::future::Future;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};

pub(super) fn completed_process_with_pending_transport(
) -> (WorkerState, SyncSender<WorkerFrame>, TcpStream) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let status = child.wait().unwrap();
    assert!(status.success(), "native child completed successfully");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (socket, _) = listener.accept().unwrap();
    let (transport, unused) = WorkerTransport::start(socket).unwrap();
    drop(unused);
    let (sender, incoming) = sync_channel(MAXIMUM_QUEUED_SIGNALS + 1);
    (
        WorkerState {
            reservation: super::super::resources::reserve_resource().unwrap(),
            physical: Arc::new(WorkerPhysical::new(WorkerChild::finished(child, status)).unwrap()),
            transport,
            incoming,
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

#[test]
fn process_exit_waits_for_every_tail_frame_and_the_reader_terminal_boundary() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    state.ingest_signals(false).unwrap();
    assert!(
        !state.delivery.borrow().exit_queued,
        "process status alone cannot close an undrained native transport"
    );
    assert!(
        state.delivery.borrow().pending.is_empty(),
        "no premature exit callback"
    );
    for index in 0..64 {
        sender.send(native_message(index)).unwrap();
        state.ingest_signals(false).unwrap();
        assert!(
            !state.delivery.borrow().exit_queued,
            "native reader has not reached its terminal boundary"
        );
    }
    state.physical.finish_reader(None);
    state.ingest_signals(false).unwrap();
    let mut delivery = state.delivery.borrow_mut();
    assert!(
        delivery.exit_queued,
        "joined native process and transport complete once"
    );
    assert_eq!(delivery.pending.len(), 65);
    for index in 0..64 {
        let (_, WorkerSignal::Message(value)) = delivery.pending.pop_front().unwrap() else {
            panic!("every final message precedes the exit callback");
        };
        assert_eq!(value.to_js(), JsValue::Integer(index));
    }
    assert!(matches!(
        delivery.pending.pop_front(),
        Some((_, WorkerSignal::Exit(0)))
    ));
    drop(delivery);
    state.ingest_signals(false).unwrap();
    assert!(
        state.delivery.borrow().pending.is_empty(),
        "one exit callback only"
    );
}

#[test]
fn full_native_signal_queue_drains_before_admitting_the_one_exit_callback() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    for index in 0..MAXIMUM_QUEUED_SIGNALS {
        sender.send(native_message(index as i64)).unwrap();
    }
    state.physical.finish_reader(None);
    state.ingest_signals(false).unwrap();
    let mut delivery = state.delivery.borrow_mut();
    assert_eq!(delivery.pending.len(), MAXIMUM_QUEUED_SIGNALS);
    assert!(
        !delivery.exit_queued,
        "full signal queue cannot admit or bypass exit"
    );
    for index in 0..MAXIMUM_QUEUED_SIGNALS {
        let (_, WorkerSignal::Message(value)) = delivery.pending.pop_front().unwrap() else {
            panic!("bounded native messages are retained in full");
        };
        assert_eq!(value.to_js(), JsValue::Integer(index as i64));
    }
    drop(delivery);
    state.ingest_signals(false).unwrap();
    let mut delivery = state.delivery.borrow_mut();
    assert!(delivery.exit_queued);
    assert_eq!(delivery.pending.len(), 1);
    assert!(matches!(
        delivery.pending.pop_front(),
        Some((_, WorkerSignal::Exit(0)))
    ));
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
            worker.state.physical.finish_reader(None);
            assert!(matches!(
                completion
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop())),
                Poll::Ready(Ok(0))
            ));
            let mut delivery = worker.state.delivery.borrow_mut();
            let (_, WorkerSignal::Message(value)) = delivery.pending.pop_front().unwrap() else {
                panic!("owner-bound termination retains final native message");
            };
            assert_eq!(value.to_js(), JsValue::Integer(i64::MAX));
            assert!(matches!(
                delivery.pending.pop_front(),
                Some((_, WorkerSignal::Exit(0)))
            ));
            assert!(delivery.pending.is_empty());
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
fn native_completion_cancels_replaces_and_wakes_concurrent_waiters_once() {
    let (state, _sender, _peer) = completed_process_with_pending_transport();
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
    let (state, _sender, _peer) = completed_process_with_pending_transport();
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
        let (state, _sender, _peer) = completed_process_with_pending_transport();
        state
            .physical
            .finish_reader(Some(NodeError::new(code, "exact native reason")));
        state.ingest_signals(false).unwrap();
        state.ingest_signals(false).unwrap();
        let delivery = state.delivery.borrow();
        assert!(
            delivery.exit_queued,
            "reader failure never strands native completion"
        );
        let errors = delivery
            .pending
            .iter()
            .filter(|(_, signal)| matches!(signal, WorkerSignal::Error(_)))
            .count();
        assert_eq!(
            errors, expected_errors,
            "ordinary representation excludes native IO; delivery tested separately"
        );
        if expected_errors != 0 {
            assert!(matches!(&delivery.pending.front().unwrap().1,
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
    state.physical.finish_reader(None);
    state.ingest_signals(false).unwrap();
    let delivery = state.delivery.borrow();
    assert!(delivery.exit_queued);
    assert!(
        matches!(&delivery.pending.front().unwrap().1,
        WorkerSignal::Error(error) if error.message().contains("version is unsupported")),
        "malformed clone is an error signal, not a loop failure"
    );
    assert!(matches!(
        delivery.pending.back(),
        Some((_, WorkerSignal::Exit(0)))
    ));
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
                    worker.state.physical.finish_reader(None);
                } else {
                    worker.state.physical.finish_reader(Some(NodeError::new(
                        if kind == "io" {
                            "ERR_WORKER_TRANSPORT"
                        } else {
                            "ERR_WORKER_PROTOCOL"
                        },
                        reason,
                    )));
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
        assert!(worker.state.delivery.borrow().pending.is_empty());
    }
}

#[test]
fn native_worker_process_fixture() {
    let Ok(mode) = std::env::var("TSONIC_P05_NATIVE_WORKER_MODE") else {
        return;
    };
    let mut socket =
        TcpStream::connect(std::env::var("TSONIC_P05_NATIVE_WORKER_ADDRESS").unwrap()).unwrap();
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
    let published = Arc::clone(&physical);
    let terminal = Arc::clone(&physical);
    let (transport, incoming) = WorkerTransport::start_observed(
        socket,
        move || published.published(),
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
                incoming,
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
