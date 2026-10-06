use std::collections::VecDeque;
use std::io::{Read as _, Write as _};
use std::rc::Weak;
use tsonic_rust_runtime::dispatch_queue::{TaskReservation, TaskTicket};

const RUNTIME_MAX_MATERIALIZED_BODY_SIZE: usize = 64 * 1024 * 1024;
const RUNTIME_READ_HIGH_WATER_MARK: usize = 64 * 1024;
const RUNTIME_WRITE_HIGH_WATER_MARK: usize = 64 * 1024;
const RUNTIME_MAX_PENDING_INPUT: usize = 256 * 1024;
const RUNTIME_MAX_PENDING_OUTPUT: usize = 16 * 1024 * 1024;
const RUNTIME_IO_BUDGET: usize = 256 * 1024;
const HTTP_MAXIMUM_PENDING_SIGNALS: usize = 1 << 16;

pub(crate) type RuntimeRequestArguments<E> = (IncomingMessage<E>, ServerResponse<E>);
pub(crate) type RuntimeRequestHandler<E> =
    tsonic_rust_runtime::Callable<RuntimeRequestArguments<E>, Result<(), E>>;
type RuntimeListenCallback<E> = tsonic_rust_runtime::Callable<(), Result<(), E>>;
type RuntimeCloseCallback<E> = tsonic_rust_runtime::Callable<(Option<NodeError>,), Result<(), E>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressInfo {
    pub address: String,
    pub family: String,
    pub port: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerAddress {
    Address(AddressInfo),
    Path(String),
}

impl ServerAddress {
    pub fn address(&self) -> Option<AddressInfo> {
        match self {
            Self::Address(address) => Some(address.clone()),
            Self::Path(_) => None,
        }
    }

    pub fn path(&self) -> Option<String> {
        match self {
            Self::Address(_) => None,
            Self::Path(path) => Some(path.clone()),
        }
    }

    pub fn port(&self) -> Option<i32> {
        self.address().map(|address| address.port)
    }
}

enum RuntimeListener {
    Tcp(crate::readiness::Listener),
    #[cfg(unix)]
    Unix(crate::readiness::UnixListener),
}

struct RuntimeServerState<E: 'static> {
    _reservation: TaskReservation,
    listener: Option<RuntimeListener>,
    handler: RuntimeRequestHandler<E>,
    address: Option<ServerAddress>,
    pending_listen_callback: Option<RuntimeListenCallback<E>>,
    pending_close_callbacks: VecDeque<RuntimeCloseCallback<E>>,
    pending: VecDeque<(TaskTicket, ServerSignal<E>)>,
    connections: BTreeMap<u64, Rc<RefCell<RuntimeConnection<E>>>>,
    resources: HttpHandle<E>,
    listening_pending: bool,
    listening: bool,
    refed: bool,
    closing: bool,
    listening_event: crate::stream::StreamEvent<(), E>,
    close_event: crate::stream::StreamEvent<(), E>,
    error_event: crate::stream::StreamEvent<NodeError, E>,
}

enum ServerSignal<E: 'static> {
    ListeningCallback(RuntimeListenCallback<E>),
    CloseCallback(RuntimeCloseCallback<E>, Option<NodeError>),
    Event(crate::stream::StreamEmission<(), E>),
}

pub struct ServerHandle<E: 'static = NodeError> {
    state: Rc<RefCell<RuntimeServerState<E>>>,
}
impl<E: 'static> Clone for ServerHandle<E> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
}
impl<E: From<NodeError> + 'static> ServerHandle<E> {
    pub fn listen(
        &self,
        port: i32,
        host: &str,
        callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.listen_with_backlog(port, host, 511, callback)
    }

    pub fn listen_with_backlog(
        &self,
        port: i32,
        host: &str,
        backlog: i32,
        callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.listen_with_backlog_optional(port, host, backlog, Some(callback))
    }

    pub fn listen_optional(
        &self,
        port: i32,
        host: &str,
        callback: Option<tsonic_rust_runtime::Callable<(), Result<(), E>>>,
    ) -> NodeResult<Self> {
        self.listen_with_backlog_optional(port, host, 511, callback)
    }

    pub fn listen_default_host(
        &self,
        port: i32,
        callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.listen(port, "0.0.0.0", callback)
    }

    pub fn listen_default_host_optional(
        &self,
        port: i32,
        callback: Option<tsonic_rust_runtime::Callable<(), Result<(), E>>>,
    ) -> NodeResult<Self> {
        self.listen_optional(port, "0.0.0.0", callback)
    }

    pub fn listen_with_backlog_optional(
        &self,
        port: i32,
        host: &str,
        backlog: i32,
        callback: Option<tsonic_rust_runtime::Callable<(), Result<(), E>>>,
    ) -> NodeResult<Self> {
        let port = u16::try_from(port).map_err(|_| {
            NodeError::new("ERR_SOCKET_BAD_PORT", "port must be between 0 and 65535")
        })?;
        if backlog < 0 {
            return Err(NodeError::new(
                "ERR_OUT_OF_RANGE",
                "listen backlog must be non-negative",
            ));
        }
        let listener = bind_tcp_listener(host, port, backlog)?;
        let address = listener.local_addr().map_err(runtime_http_io_error)?;
        self.install_listener(
            RuntimeListener::Tcp(crate::readiness::Listener::new(listener)?),
            ServerAddress::Address(AddressInfo {
                address: address.ip().to_string(),
                family: if address.is_ipv4() { "IPv4" } else { "IPv6" }.to_string(),
                port: i32::from(address.port()),
            }),
            callback,
        )?;
        Ok(self.clone())
    }

    #[cfg(unix)]
    pub fn listen_path_optional(
        &self,
        path: &str,
        callback: Option<tsonic_rust_runtime::Callable<(), Result<(), E>>>,
    ) -> NodeResult<Self> {
        if path.is_empty() {
            return Err(NodeError::new(
                "ERR_INVALID_ARG_VALUE",
                "listen path must not be empty",
            ));
        }
        let listener = crate::readiness::UnixListener::bind(std::path::Path::new(path))?;
        self.install_listener(
            RuntimeListener::Unix(listener),
            ServerAddress::Path(path.to_string()),
            callback,
        )?;
        Ok(self.clone())
    }

    #[cfg(not(unix))]
    pub fn listen_path_optional(
        &self,
        _path: &str,
        _callback: Option<tsonic_rust_runtime::Callable<(), Result<(), E>>>,
    ) -> NodeResult<Self> {
        Err(NodeError::new(
            "ERR_UNSUPPORTED_PLATFORM",
            "Unix-domain HTTP listeners are unavailable on this platform",
        ))
    }

    fn install_listener(
        &self,
        listener: RuntimeListener,
        address: ServerAddress,
        callback: Option<RuntimeListenCallback<E>>,
    ) -> NodeResult<()> {
        let mut state = self.state.borrow_mut();
        if state.listening || state.closing {
            return Err(NodeError::new(
                "ERR_SERVER_ALREADY_LISTEN",
                "server is already listening or closing",
            ));
        }
        state.listener = Some(listener);
        state.address = Some(address);
        state.pending_listen_callback = callback;
        state.listening_pending = true;
        state.listening = true;
        Ok(())
    }

    pub fn listening(&self) -> bool {
        self.state.borrow().listening
    }

    pub fn address(&self) -> Option<ServerAddress> {
        self.state.borrow().address.clone()
    }

    pub fn local_port(&self) -> NodeResult<u16> {
        match self.address() {
            Some(ServerAddress::Address(address)) => u16::try_from(address.port).map_err(|_| {
                NodeError::new(
                    "ERR_SOCKET_BAD_PORT",
                    "bound port is outside the native range",
                )
            }),
            Some(ServerAddress::Path(_)) => Err(NodeError::new(
                "ERR_INVALID_ARG_VALUE",
                "a path listener does not have a TCP port",
            )),
            None => Err(NodeError::new(
                "ERR_SERVER_NOT_RUNNING",
                "server is not listening",
            )),
        }
    }

    pub fn close(&self) -> NodeResult<Self> {
        let mut state = self.state.borrow_mut();
        if state.listening {
            state.listener = None;
            state.listening = false;
            state.closing = true;
        }
        Ok(self.clone())
    }

    pub fn close_callback(&self, callback: RuntimeCloseCallback<E>) -> NodeResult<Self> {
        let mut state = self.state.borrow_mut();
        if state.pending.len() + state.pending_close_callbacks.len() >= HTTP_MAXIMUM_PENDING_SIGNALS
        {
            return Err(http_queue_limit());
        }
        if !state.listening && !state.closing {
            let ticket = admit_http_signal()?;
            state.pending.push_back((
                ticket,
                ServerSignal::CloseCallback(
                    callback,
                    Some(NodeError::new(
                        "ERR_SERVER_NOT_RUNNING",
                        "server is not listening",
                    )),
                ),
            ));
        } else {
            state.listener = None;
            state.listening = false;
            state.closing = true;
            state.pending_close_callbacks.push_back(callback);
        }
        Ok(self.clone())
    }

    pub fn close_optional(&self, callback: Option<RuntimeCloseCallback<E>>) -> NodeResult<Self> {
        match callback {
            Some(callback) => self.close_callback(callback),
            None => self.close(),
        }
    }

    pub fn ref_chain(&self) -> Self {
        self.state.borrow_mut().refed = true;
        self.clone()
    }

    pub fn unref_chain(&self) -> Self {
        self.state.borrow_mut().refed = false;
        self.clone()
    }

    pub fn on_listening(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        add_server_empty_listener(&self.state, event, "listening", listener, false)?;
        Ok(self.clone())
    }

    pub fn once_listening(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        add_server_empty_listener(&self.state, event, "listening", listener, true)?;
        Ok(self.clone())
    }

    pub fn off_listening(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        remove_server_empty_listener(&self.state, event, "listening", listener.identity_key())?;
        Ok(self.clone())
    }

    pub fn on_close(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        add_server_empty_listener(&self.state, event, "close", listener, false)?;
        Ok(self.clone())
    }

    pub fn once_close(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        add_server_empty_listener(&self.state, event, "close", listener, true)?;
        Ok(self.clone())
    }

    pub fn off_close(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        remove_server_empty_listener(&self.state, event, "close", listener.identity_key())?;
        Ok(self.clone())
    }

    pub fn on_error(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(NodeError,), Result<(), E>>,
    ) -> NodeResult<Self> {
        ensure_http_event(event, "error")?;
        self.state
            .borrow_mut()
            .error_event
            .add(crate::stream::value_listener(listener, false));
        Ok(self.clone())
    }

    pub fn once_error(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(NodeError,), Result<(), E>>,
    ) -> NodeResult<Self> {
        ensure_http_event(event, "error")?;
        self.state
            .borrow_mut()
            .error_event
            .add(crate::stream::value_listener(listener, true));
        Ok(self.clone())
    }

    pub fn off_error(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(NodeError,), Result<(), E>>,
    ) -> NodeResult<Self> {
        ensure_http_event(event, "error")?;
        self.state
            .borrow_mut()
            .error_event
            .remove(listener.identity_key());
        Ok(self.clone())
    }
}

pub fn create_server_callable<E: From<NodeError> + 'static>(
    roots: &HttpServers<E>,
    handler: RuntimeRequestHandler<E>,
) -> NodeResult<ServerHandle<E>> {
    let reservation = reserve_http_resource()?;
    let ticket = reservation.ticket();
    let resources = roots.handle();
    let server = ServerHandle {
        state: Rc::new(RefCell::new(RuntimeServerState {
            _reservation: reservation,
            listener: None,
            handler,
            address: None,
            pending_listen_callback: None,
            pending_close_callbacks: VecDeque::new(),
            pending: VecDeque::new(),
            connections: BTreeMap::new(),
            resources: resources.clone(),
            listening_pending: false,
            listening: false,
            refed: true,
            closing: false,
            listening_event: crate::stream::StreamEvent::default(),
            close_event: crate::stream::StreamEvent::default(),
            error_event: crate::stream::StreamEvent::default(),
        })),
    };
    resources.register(ticket, HttpResource::Server(Rc::downgrade(&server.state)))?;
    Ok(server)
}
pub fn create_server_optional<E: From<NodeError> + 'static>(
    roots: &HttpServers<E>,
    handler: Option<RuntimeRequestHandler<E>>,
) -> NodeResult<ServerHandle<E>> {
    create_server_callable(
        roots,
        handler.unwrap_or_else(|| {
            tsonic_rust_runtime::Callable::new(
                |(_request, response): RuntimeRequestArguments<E>| response.end_empty().map(|_| ()),
            )
        }),
    )
}
fn add_server_empty_listener<E: From<NodeError> + 'static>(
    server: &Rc<RefCell<RuntimeServerState<E>>>,
    event: &str,
    expected: &str,
    listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    once: bool,
) -> NodeResult<()> {
    ensure_http_event(event, expected)?;
    let entry = crate::stream::empty_listener(listener, once);
    let mut server = server.borrow_mut();
    match expected {
        "listening" => server.listening_event.add(entry),
        "close" => server.close_event.add(entry),
        _ => unreachable!("validated server event"),
    }
    Ok(())
}

fn remove_server_empty_listener<E: From<NodeError> + 'static>(
    server: &Rc<RefCell<RuntimeServerState<E>>>,
    event: &str,
    expected: &str,
    identity: usize,
) -> NodeResult<()> {
    ensure_http_event(event, expected)?;
    let mut server = server.borrow_mut();
    match expected {
        "listening" => server.listening_event.remove(identity),
        "close" => server.close_event.remove(identity),
        _ => unreachable!("validated server event"),
    }
    Ok(())
}
