use super::*;
use std::future::Future;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};

fn completed_process_with_pending_transport() -> (WorkerState, SyncSender<TransportEvent>, TcpStream)
{
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    assert!(
        child.wait().unwrap().success(),
        "native child completed successfully"
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (socket, _) = listener.accept().unwrap();
    let (transport, unused) = WorkerTransport::start(socket).unwrap();
    drop(unused);
    let (sender, incoming) = sync_channel(MAXIMUM_QUEUED_SIGNALS + 1);
    (
        WorkerState {
            reservation: super::super::resources::reserve_resource().unwrap(),
            pending: VecDeque::new(),
            child,
            transport,
            incoming,
            thread_id: 1,
            refed: true,
            complete: false,
            transport_ended: false,
            exit_code: None,
            termination_requested: false,
            completion: WorkerCompletionWaiters::default(),
        },
        sender,
        peer,
    )
}

fn native_message(value: i64) -> TransportEvent {
    TransportEvent::Frame(super::super::protocol::WorkerFrame {
        kind: WorkerFrameKind::Message,
        payload: encode(&ClonedValue::from_js(&JsValue::Integer(value)).unwrap()).unwrap(),
    })
}

#[test]
fn process_exit_waits_for_every_tail_frame_and_the_reader_terminal_boundary() {
    let (mut state, sender, _peer) = completed_process_with_pending_transport();
    state.ingest_signals().unwrap();
    assert!(
        !state.complete,
        "process status alone cannot close an undrained native transport"
    );
    assert!(state.pending.is_empty(), "no premature exit callback");
    for index in 0..64 {
        sender.send(native_message(index)).unwrap();
        state.ingest_signals().unwrap();
        assert!(
            !state.complete,
            "native reader has not reached its terminal boundary"
        );
    }
    sender.send(TransportEvent::End).unwrap();
    state.ingest_signals().unwrap();
    assert!(
        state.complete,
        "joined native process and transport complete once"
    );
    assert_eq!(state.pending.len(), 65);
    for index in 0..64 {
        let (_, WorkerSignal::Message(value)) = state.pending.pop_front().unwrap() else {
            panic!("every final message precedes the exit callback");
        };
        assert_eq!(value.to_js(), JsValue::Integer(index));
    }
    assert!(matches!(
        state.pending.pop_front(),
        Some((_, WorkerSignal::Exit(0)))
    ));
    state.ingest_signals().unwrap();
    assert!(state.pending.is_empty(), "one exit callback only");
}

#[test]
fn full_native_signal_queue_drains_before_admitting_the_one_exit_callback() {
    let (mut state, sender, _peer) = completed_process_with_pending_transport();
    for index in 0..MAXIMUM_QUEUED_SIGNALS {
        sender.send(native_message(index as i64)).unwrap();
    }
    sender.send(TransportEvent::End).unwrap();
    state.ingest_signals().unwrap();
    assert_eq!(state.pending.len(), MAXIMUM_QUEUED_SIGNALS);
    assert!(
        !state.complete,
        "full signal queue cannot admit or bypass exit"
    );
    for index in 0..MAXIMUM_QUEUED_SIGNALS {
        let (_, WorkerSignal::Message(value)) = state.pending.pop_front().unwrap() else {
            panic!("bounded native messages are retained in full");
        };
        assert_eq!(value.to_js(), JsValue::Integer(index as i64));
    }
    state.ingest_signals().unwrap();
    assert!(state.complete);
    assert_eq!(state.pending.len(), 1);
    assert!(matches!(
        state.pending.pop_front(),
        Some((_, WorkerSignal::Exit(0)))
    ));
}

#[test]
fn explicit_termination_does_not_bypass_final_transport_admission() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    let worker = Worker::<NodeError> {
        state: Rc::new(RefCell::new(state)),
        emitter: EventEmitter::new(),
    };
    let mut completion = std::pin::pin!(worker.terminate());
    assert!(matches!(
        completion
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    assert!(
        !worker.state.borrow().complete,
        "termination status is not transport completion"
    );
    sender.send(native_message(i64::MAX)).unwrap();
    sender.send(TransportEvent::End).unwrap();
    worker.capture().unwrap();
    assert!(matches!(
        completion
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Ok(0))
    ));
    let mut state = worker.state.borrow_mut();
    let (_, WorkerSignal::Message(value)) = state.pending.pop_front().unwrap() else {
        panic!("termination retains final native message");
    };
    assert_eq!(value.to_js(), JsValue::Integer(i64::MAX));
    assert!(matches!(
        state.pending.pop_front(),
        Some((_, WorkerSignal::Exit(0)))
    ));
    assert!(state.pending.is_empty());
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
    let (state, sender, _peer) = completed_process_with_pending_transport();
    let worker = Worker::<NodeError> {
        state: Rc::new(RefCell::new(state)),
        emitter: EventEmitter::new(),
    };
    let invoked = Arc::new(Mutex::new(Vec::new()));
    let first_waker = observed_waker(1, &invoked);
    let second_waker = observed_waker(2, &invoked);
    let replacement = observed_waker(3, &invoked);
    let mut first = completion::WorkerCompletion::new(&worker.state);
    let mut second = completion::WorkerCompletion::new(&worker.state);
    let mut third = completion::WorkerCompletion::new(&worker.state);
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
    worker.capture().unwrap();
    assert!(
        invoked.lock().unwrap().is_empty(),
        "no poll-based or premature completion wake"
    );
    sender.send(TransportEvent::End).unwrap();
    worker.capture().unwrap();
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
    worker.capture().unwrap();
    assert_eq!(
        invoked.lock().unwrap().len(),
        2,
        "native completion wakes once"
    );
}

#[test]
fn native_completion_drops_the_inline_registration_before_reuse() {
    let (state, sender, _peer) = completed_process_with_pending_transport();
    let worker = Worker::<NodeError> {
        state: Rc::new(RefCell::new(state)),
        emitter: EventEmitter::new(),
    };
    let invoked = Arc::new(Mutex::new(Vec::new()));
    let waker = observed_waker(1, &invoked);
    let mut cancelled = completion::WorkerCompletion::new(&worker.state);
    assert!(Pin::new(&mut cancelled)
        .poll(&mut Context::from_waker(&waker))
        .is_pending());
    drop(cancelled);
    let mut current = completion::WorkerCompletion::new(&worker.state);
    assert!(Pin::new(&mut current)
        .poll(&mut Context::from_waker(&waker))
        .is_pending());
    sender.send(TransportEvent::End).unwrap();
    worker.capture().unwrap();
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
        let (mut state, sender, _peer) = completed_process_with_pending_transport();
        sender
            .send(TransportEvent::Failure(NodeError::new(
                code,
                "exact native reason",
            )))
            .unwrap();
        sender.send(TransportEvent::End).unwrap();
        state.ingest_signals().unwrap();
        state.ingest_signals().unwrap();
        let errors = state
            .pending
            .iter()
            .filter(|(_, signal)| matches!(signal, WorkerSignal::Error(_)))
            .count();
        assert_eq!(
            errors, expected_errors,
            "observed native process exit suppresses only IO failure"
        );
        if expected_errors != 0 {
            assert!(
                matches!(&state.pending.front().unwrap().1, WorkerSignal::Error(message) if message.ends_with("exact native reason"))
            );
        }
        assert!(
            state.complete,
            "reader failure never strands the native completion"
        );
    }
    let (mut state, sender, _peer) = completed_process_with_pending_transport();
    sender
        .send(TransportEvent::Frame(super::super::protocol::WorkerFrame {
            kind: WorkerFrameKind::Message,
            payload: vec![255],
        }))
        .unwrap();
    sender.send(TransportEvent::End).unwrap();
    state.ingest_signals().unwrap();
    assert!(state.complete);
    assert!(
        matches!(&state.pending.front().unwrap().1, WorkerSignal::Error(message) if message.contains("version is unsupported")),
        "malformed clone is an error signal, not a loop failure"
    );
    assert!(matches!(
        state.pending.back(),
        Some((_, WorkerSignal::Exit(0)))
    ));
}

#[test]
fn native_worker_process_fixture() {
    let Ok(mode) = std::env::var("TSONIC_P05_NATIVE_WORKER_MODE") else {
        return;
    };
    let mut socket =
        TcpStream::connect(std::env::var("TSONIC_P05_NATIVE_WORKER_ADDRESS").unwrap()).unwrap();
    for index in 0..10 {
        let payload = encode(&ClonedValue::from_js(&JsValue::Integer(index)).unwrap()).unwrap();
        write_frame(&mut socket, WorkerFrameKind::Message, &payload).unwrap();
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
}

fn live_native_worker(mode: &str) -> (Worker, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let control_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
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
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let (socket, _) = listener.accept().unwrap();
    let (transport, incoming) = WorkerTransport::start(socket).unwrap();
    let (mut control, _) = control_listener.accept().unwrap();
    control
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut ready = [0];
    control.read_exact(&mut ready).unwrap();
    assert_eq!(
        ready,
        [1],
        "live native child wrote its retained prefix before signalling ready"
    );
    (
        Worker {
            state: Rc::new(RefCell::new(WorkerState {
                reservation: super::super::resources::reserve_resource().unwrap(),
                pending: VecDeque::new(),
                child,
                transport,
                incoming,
                thread_id: 1,
                refed: true,
                complete: false,
                transport_ended: false,
                exit_code: None,
                termination_requested: false,
                completion: WorkerCompletionWaiters::default(),
            })),
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
        let exit_code = Rc::new(Cell::new(None));
        let exits = Rc::new(Cell::new(0));
        let delivered = Rc::clone(&messages);
        worker
            .on_callable1(
                &JsValue::String("message".to_owned()),
                &Callable::new(move |(value,): (JsValue,)| {
                    let JsValue::Integer(value) = value else {
                        panic!("worker native int64 remains exact");
                    };
                    delivered.borrow_mut().push(value);
                    Ok::<(), TsonicError>(())
                }),
            )
            .unwrap();
        let failures = Rc::clone(&errors);
        let release = control.try_clone().unwrap();
        worker
            .on_callable1(
                &JsValue::String("error".to_owned()),
                &Callable::new(move |(value,): (JsValue,)| {
                    let JsValue::String(message) = value else {
                        panic!("worker native error retains its message");
                    };
                    failures.borrow_mut().push(message);
                    if mode == "live-failure" {
                        (&release).write_all(&[1]).unwrap();
                    }
                    Ok::<(), TsonicError>(())
                }),
            )
            .unwrap();
        let observed = Rc::clone(&exit_code);
        let completed = Rc::clone(&exits);
        let prefix = Rc::clone(&messages);
        worker
            .on_callable1(
                &JsValue::String("exit".to_owned()),
                &Callable::new(move |(value,): (JsValue,)| {
                    let JsValue::Int32(code) = value else {
                        panic!("worker exit retains native int32");
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
        let (result, _) = crate::block_on_with_contexts(
            async {
                resources.register_worker(&worker).unwrap();
                if mode == "partial" {
                    let code = worker.terminate().await.unwrap();
                    #[cfg(unix)]
                    assert_eq!(code, 137, "SIGKILL preserves the native Unix signal status");
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
            "native root completed without a polling fallback"
        );
        assert_eq!(
            errors.borrow().len(),
            usize::from(mode == "live-failure"),
            "only failure while the native worker is live emits a transport error"
        );
        assert_eq!(
            exits.get(),
            1,
            "native completion admits one exit after its prefix"
        );
        assert!(exit_code.get().is_some());
        assert!(
            worker.state.borrow().complete,
            "one terminal owner completes process and reader"
        );
    }
}
