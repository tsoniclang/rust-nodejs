use super::*;
use std::future::Future;
use std::net::{TcpListener, TcpStream};
use std::process::Stdio;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::task::{Context, Poll, Waker};

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
        Poll::Ready(Ok(0))
    ));
    assert!(
        !worker.state.borrow().complete,
        "termination status is not transport completion"
    );
    sender.send(native_message(i64::MAX)).unwrap();
    sender.send(TransportEvent::End).unwrap();
    worker.state.borrow_mut().ingest_signals().unwrap();
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
