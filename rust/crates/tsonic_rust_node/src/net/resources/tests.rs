use super::{admit_signal, NetServers, Server, ServerSignal};
use crate::NodeError;
use std::cell::Cell;
use std::net::TcpStream;
use std::rc::Rc;
use tsonic_rust_runtime::dispatch::{poll_phase, poll_prepared, DispatchContexts, DispatchPhase};
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

fn enqueue(server: &Server<Failure>, callback: Callable<(), Result<(), Failure>>) {
    server
        .state
        .borrow_mut()
        .pending
        .push_back((admit_signal().unwrap(), ServerSignal::Listening(callback)));
}

#[test]
fn native_connections_retain_the_original_failure_and_later_accepted_connections() {
    let roots = NetServers::<Failure>::new();
    let server = Server::new(&roots).unwrap();
    let original = Rc::new(Cell::new(9_007_199_254_740_993_u64));
    let retained = Rc::clone(&original);
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    server.set_connection_callback(Callable::new(move |(_socket,)| {
        observed.set(observed.get() + 1);
        if observed.get() == 1 {
            Err(Failure::Original(Rc::clone(&retained)))
        } else {
            Ok(())
        }
    }));
    server.bind("127.0.0.1", 0).unwrap();
    let port = server.local_port().unwrap();
    let clients: Vec<_> = (0..4)
        .map(|_| TcpStream::connect(("127.0.0.1", port)).unwrap())
        .collect();
    match poll_phase(&roots, DispatchPhase::Net) {
        Err(Failure::Original(returned)) => {
            assert!(Rc::ptr_eq(&returned, &original));
            assert_eq!(returned.get(), 9_007_199_254_740_993_u64);
        }
        Err(Failure::Native(error)) => panic!("unexpected native guard: {}", error.code()),
        Ok(_) => panic!("original connection callback failure was lost"),
    }
    assert_eq!(calls.get(), 1);
    assert_eq!(server.state.borrow().pending.len(), 3);
    assert!(matches!(poll_phase(&roots, DispatchPhase::Net), Ok(true)));
    assert_eq!(calls.get(), 4);
    server.close();
    assert!(!roots.has_work());
    drop(clients);
}

#[test]
fn reentrant_listening_work_waits_for_the_next_captured_native_frontier() {
    let roots = NetServers::<Failure>::new();
    let server = Server::new(&roots).unwrap();
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let nested = server.clone();
    enqueue(
        &server,
        Callable::new(move |()| {
            let observed = Rc::clone(&observed);
            enqueue(
                &nested,
                Callable::new(move |()| {
                    observed.set(observed.get() + 1);
                    Ok(())
                }),
            );
            Ok(())
        }),
    );
    let captured = match roots.prepare(DispatchPhase::Net) {
        Ok(value) => value,
        Err(_) => panic!("valid native capture failed"),
    };
    assert!(matches!(poll_prepared(&roots, &captured), Ok(true)));
    assert_eq!(calls.get(), 0);
    assert!(matches!(poll_phase(&roots, DispatchPhase::Net), Ok(true)));
    assert_eq!(calls.get(), 1);
}

#[test]
fn weak_server_roots_release_listener_captures_when_the_last_native_owner_drops() {
    struct Probe(Rc<Cell<usize>>);
    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let roots = NetServers::<Failure>::new();
    let server = Server::new(&roots).unwrap();
    let drops = Rc::new(Cell::new(0));
    let probe = Probe(Rc::clone(&drops));
    let listener = Callable::new(move |(_socket,)| {
        std::hint::black_box(&probe);
        Ok(())
    });
    let identity = listener.identity_key();
    server.set_connection_callback(listener);
    assert_eq!(
        server
            .state
            .borrow()
            .connection_callback
            .as_ref()
            .unwrap()
            .identity_key(),
        identity
    );
    let alias = server.clone();
    drop(server);
    assert_eq!(drops.get(), 0);
    drop(alias);
    assert_eq!(drops.get(), 1);
    assert!(!roots.has_work());
    assert!(matches!(poll_phase(&roots, DispatchPhase::Net), Ok(false)));
}

#[test]
fn native_server_guards_keep_their_native_codes() {
    let roots = NetServers::<Failure>::new();
    let server = Server::new(&roots).unwrap();
    assert_eq!(
        server.listen_port(65_536_i64).err().unwrap().code(),
        "ERR_SOCKET_BAD_PORT"
    );
    assert_eq!(
        server.address().err().unwrap().code(),
        "ERR_SERVER_NOT_RUNNING"
    );
    assert_eq!(server.connections(), 0);
    assert!(!server.listening());
}
