use super::resources::{self, TlsServers};
use super::{map_io_error, server_config, source_port, SourceServerOptions, TlsSocket};
use crate::background::{BackgroundHandle, BackgroundTasks};
use crate::{NodeError, NodeResult};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::net::{TcpListener, TcpStream};
use std::rc::Rc;
use std::sync::Arc;
use tsonic_rust_runtime::dispatch_queue::{TaskReservation, TaskTicket};
use tsonic_rust_runtime::{Callable, TsonicError};

pub(super) enum TlsSignal<E> {
    Listening(Callable<(), Result<(), E>>),
    Connection(
        TcpStream,
        Arc<rustls::ServerConfig>,
        Callable<(TlsSocket,), Result<(), E>>,
    ),
}

pub(super) struct TlsServerState<E> {
    pub(super) reservation: TaskReservation,
    pub(super) options: SourceServerOptions,
    pub(super) config: Arc<rustls::ServerConfig>,
    pub(super) background: BackgroundHandle<E>,
    pub(super) listener: Option<crate::readiness::Listener>,
    pub(super) callback: Callable<(TlsSocket,), Result<(), E>>,
    pub(super) pending: VecDeque<(TaskTicket, TlsSignal<E>)>,
    pub(super) listening: bool,
    pub(super) refed: bool,
}

pub struct TlsServer<E: 'static = TsonicError> {
    pub(super) state: Rc<RefCell<TlsServerState<E>>>,
}

impl<E: 'static> Clone for TlsServer<E> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
}

impl<E: From<NodeError> + 'static> TlsServer<E> {
    pub fn create(
        roots: &TlsServers<E>,
        background: &BackgroundTasks<E>,
        options: SourceServerOptions,
        callback: Callable<(TlsSocket,), Result<(), E>>,
    ) -> NodeResult<Self> {
        let reservation = resources::reserve_resource()?;
        let config = Arc::new(server_config(&options)?);
        let value = Self {
            state: Rc::new(RefCell::new(TlsServerState {
                reservation,
                options,
                config,
                background: background.handle(),
                listener: None,
                callback,
                pending: VecDeque::new(),
                listening: false,
                refed: true,
            })),
        };
        roots.register(&value);
        Ok(value)
    }

    pub fn listen(
        &self,
        port: impl tsonic_rust_runtime::conversions::IntegerInput<u16>,
        host: &str,
    ) -> NodeResult<&Self> {
        let listener = TcpListener::bind((host, source_port(port)?)).map_err(map_io_error)?;
        let listener = crate::readiness::Listener::new(listener)?;
        let mut state = self.state.borrow_mut();
        state.listener = Some(listener);
        state.listening = true;
        Ok(self)
    }

    pub fn listen_callable(
        &self,
        port: impl tsonic_rust_runtime::conversions::IntegerInput<u16>,
        host: &str,
        callback: Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        if self.state.borrow().pending.len() >= resources::MAXIMUM_PENDING_SIGNALS {
            return Err(resources::queue_limit());
        }
        let ticket = resources::admit_signal()?;
        self.listen(port, host)?;
        self.state
            .borrow_mut()
            .pending
            .push_back((ticket, TlsSignal::Listening(callback)));
        Ok(self)
    }

    pub fn listen_default_host_callable(
        &self,
        port: impl tsonic_rust_runtime::conversions::IntegerInput<u16>,
        callback: Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.listen_callable(port, "0.0.0.0", callback)
    }

    pub fn close(&self) {
        let mut state = self.state.borrow_mut();
        state.listener = None;
        state.listening = false;
    }

    pub fn listening(&self) -> bool {
        self.state.borrow().listening
    }

    pub fn ref_chain(&self) -> &Self {
        self.state.borrow_mut().refed = true;
        self
    }
    pub fn unref_chain(&self) -> &Self {
        self.state.borrow_mut().refed = false;
        self
    }
    pub fn options(&self) -> SourceServerOptions {
        self.state.borrow().options.clone()
    }
}

pub fn create_server<E: From<NodeError> + 'static>(
    roots: &TlsServers<E>,
    background: &BackgroundTasks<E>,
    options: SourceServerOptions,
    callback: Callable<(TlsSocket,), Result<(), E>>,
) -> NodeResult<TlsServer<E>> {
    TlsServer::create(roots, background, options, callback)
}
