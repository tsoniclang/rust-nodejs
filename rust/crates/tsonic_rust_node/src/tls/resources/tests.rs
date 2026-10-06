use super::{admit_signal, TlsServers};
use crate::background::BackgroundTasks;
use crate::tls::server::{TlsServer, TlsServerState, TlsSignal};
use crate::{NodeError, NodeResult};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
use tsonic_rust_runtime::dispatch::{poll_phase, poll_prepared, DispatchContexts, DispatchPhase};
use tsonic_rust_runtime::{Callable, TsonicError};

enum Failure {
    Original(Rc<Cell<u64>>),
    Native(NodeError),
    Runtime(TsonicError),
}
impl From<NodeError> for Failure {
    fn from(value: NodeError) -> Self {
        Self::Native(value)
    }
}
impl From<TsonicError> for Failure {
    fn from(value: TsonicError) -> Self {
        Self::Runtime(value)
    }
}

fn server(
    roots: &TlsServers<Failure>,
    background: &BackgroundTasks<Failure>,
) -> NodeResult<TlsServer<Failure>> {
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(rustls::server::ResolvesServerCertUsingSni::new()));
    let value = TlsServer {
        state: Rc::new(RefCell::new(TlsServerState {
            reservation: super::reserve_resource()?,
            options: Default::default(),
            config: Arc::new(config),
            background: background.handle(),
            listener: None,
            callback: Callable::new(|_| Ok(())),
            pending: VecDeque::new(),
            listening: false,
            refed: true,
        })),
    };
    roots.register(&value);
    Ok(value)
}

fn enqueue(server: &TlsServer<Failure>, callback: Callable<(), Result<(), Failure>>) {
    server
        .state
        .borrow_mut()
        .pending
        .push_back((admit_signal().unwrap(), TlsSignal::Listening(callback)));
}

#[test]
fn native_tls_callbacks_keep_original_failure_and_later_work() {
    let roots = TlsServers::<Failure>::new();
    let background = BackgroundTasks::<Failure>::new();
    let value = server(&roots, &background).unwrap();
    let original = Rc::new(Cell::new(9_007_199_254_740_993_u64));
    let retained = Rc::clone(&original);
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    enqueue(
        &value,
        Callable::new(move |()| Err(Failure::Original(Rc::clone(&retained)))),
    );
    enqueue(
        &value,
        Callable::new(move |()| {
            observed.set(observed.get() + 1);
            Ok(())
        }),
    );
    match poll_phase(&roots, DispatchPhase::Tls) {
        Err(Failure::Original(returned)) => {
            assert!(Rc::ptr_eq(&returned, &original));
            assert_eq!(returned.get(), 9_007_199_254_740_993);
        }
        Err(Failure::Native(error)) => panic!("unexpected native guard: {}", error.code()),
        Err(Failure::Runtime(error)) => panic!("unexpected runtime guard: {error}"),
        Ok(_) => panic!("original TLS failure lost"),
    }
    assert_eq!(calls.get(), 0);
    assert_eq!(value.state.borrow().pending.len(), 1);
    assert!(matches!(poll_phase(&roots, DispatchPhase::Tls), Ok(true)));
    assert_eq!(calls.get(), 1);
}

#[test]
fn reentrant_tls_callbacks_wait_for_the_next_native_frontier() {
    let roots = TlsServers::<Failure>::new();
    let background = BackgroundTasks::<Failure>::new();
    let value = server(&roots, &background).unwrap();
    let alias = value.clone();
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    enqueue(
        &value,
        Callable::new(move |()| {
            let observed = Rc::clone(&observed);
            enqueue(
                &alias,
                Callable::new(move |()| {
                    observed.set(observed.get() + 1);
                    Ok(())
                }),
            );
            Ok(())
        }),
    );
    let frontier = match roots.prepare(DispatchPhase::Tls) {
        Ok(value) => value,
        Err(_) => panic!("valid native TLS capture failed"),
    };
    assert!(matches!(poll_prepared(&roots, &frontier), Ok(true)));
    assert_eq!(calls.get(), 0);
    assert!(matches!(poll_phase(&roots, DispatchPhase::Tls), Ok(true)));
    assert_eq!(calls.get(), 1);
}

#[test]
fn weak_tls_roots_release_the_actual_source_capture() {
    struct Probe(Rc<Cell<usize>>);
    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let roots = TlsServers::<Failure>::new();
    let background = BackgroundTasks::<Failure>::new();
    let value = server(&roots, &background).unwrap();
    let drops = Rc::new(Cell::new(0));
    let probe = Probe(Rc::clone(&drops));
    let listener = Callable::new(move |_| {
        std::hint::black_box(&probe);
        Ok(())
    });
    let identity = listener.identity_key();
    value.state.borrow_mut().callback = listener;
    assert_eq!(value.state.borrow().callback.identity_key(), identity);
    let alias = value.clone();
    drop(value);
    assert_eq!(drops.get(), 0);
    drop(alias);
    assert_eq!(drops.get(), 1);
    assert!(!roots.has_work());
    assert!(matches!(poll_phase(&roots, DispatchPhase::Tls), Ok(false)));
}

#[test]
fn tls_native_configuration_and_port_guards_remain_exact() {
    let roots = TlsServers::<Failure>::new();
    let background = BackgroundTasks::<Failure>::new();
    let invalid = TlsServer::create(
        &roots,
        &background,
        Default::default(),
        Callable::new(|_| Ok(())),
    );
    assert_eq!(invalid.err().unwrap().code(), "ERR_TLS_CERT_REQUIRED");
    let value = server(&roots, &background).unwrap();
    assert_eq!(
        value.listen(65_536_u64, "127.0.0.1").err().unwrap().code(),
        "ERR_SOCKET_BAD_PORT"
    );
    assert!(!value.listening());
    assert!(!roots.has_work());
}
