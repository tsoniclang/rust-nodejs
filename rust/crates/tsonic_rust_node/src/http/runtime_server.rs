use std::cell::Cell;
use std::collections::VecDeque;
use std::io::{Read as _, Write as _};
use std::rc::Weak;

const RUNTIME_MAX_MATERIALIZED_BODY_SIZE: usize = 64 * 1024 * 1024;
const RUNTIME_READ_HIGH_WATER_MARK: usize = 64 * 1024;
const RUNTIME_WRITE_HIGH_WATER_MARK: usize = 64 * 1024;
const RUNTIME_MAX_PENDING_INPUT: usize = 256 * 1024;
const RUNTIME_MAX_PENDING_OUTPUT: usize = 16 * 1024 * 1024;
const RUNTIME_IO_BUDGET: usize = 256 * 1024;

static NEXT_RUNTIME_SERVER_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static NEXT_RUNTIME_CONNECTION_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

pub(crate) type RuntimeRequestArguments = (IncomingMessage, ServerResponse);
pub(crate) type RuntimeRequestHandler =
    tsonic_rust_runtime::Callable<RuntimeRequestArguments, tsonic_rust_runtime::TsonicResult<()>>;
type RuntimeListenCallback =
    tsonic_rust_runtime::Callable<(), tsonic_rust_runtime::TsonicResult<()>>;
type RuntimeCloseCallback = tsonic_rust_runtime::Callable<
    (Option<NodeError>,),
    tsonic_rust_runtime::TsonicResult<()>,
>;

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

struct RuntimeServerState {
    id: u64,
    listener: Option<RuntimeListener>,
    handler: RuntimeRequestHandler,
    address: Option<ServerAddress>,
    pending_listen_callback: Option<RuntimeListenCallback>,
    pending_close_callbacks: Vec<RuntimeCloseCallback>,
    listening: bool,
    refed: bool,
    active_connections: usize,
    closing: bool,
    listening_event: crate::stream::StreamEvent<()>,
    close_event: crate::stream::StreamEvent<()>,
    error_event: crate::stream::StreamEvent<NodeError>,
}

#[derive(Clone)]
pub struct ServerHandle {
    state: Rc<RefCell<RuntimeServerState>>,
}

impl ServerHandle {
    pub fn listen<E>(
        &self,
        port: i32,
        host: &str,
        callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.listen_with_backlog(port, host, 511, callback)
    }

    pub fn listen_with_backlog<E>(
        &self,
        port: i32,
        host: &str,
        backlog: i32,
        callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.listen_with_backlog_optional(port, host, backlog, Some(callback))
    }

    pub fn listen_optional<E>(
        &self,
        port: i32,
        host: &str,
        callback: Option<tsonic_rust_runtime::Callable<(), Result<(), E>>>,
    ) -> NodeResult<Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.listen_with_backlog_optional(port, host, 511, callback)
    }

    pub fn listen_default_host<E>(
        &self,
        port: i32,
        callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.listen(port, "0.0.0.0", callback)
    }

    pub fn listen_default_host_optional<E>(
        &self,
        port: i32,
        callback: Option<tsonic_rust_runtime::Callable<(), Result<(), E>>>,
    ) -> NodeResult<Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.listen_optional(port, "0.0.0.0", callback)
    }

    pub fn listen_with_backlog_optional<E>(
        &self,
        port: i32,
        host: &str,
        backlog: i32,
        callback: Option<tsonic_rust_runtime::Callable<(), Result<(), E>>>,
    ) -> NodeResult<Self>
    where
        E: std::fmt::Display + 'static,
    {
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
            callback.map(adapt_runtime_callback),
        )?;
        Ok(self.clone())
    }

    #[cfg(unix)]
    pub fn listen_path_optional<E>(
        &self,
        path: &str,
        callback: Option<tsonic_rust_runtime::Callable<(), Result<(), E>>>,
    ) -> NodeResult<Self>
    where
        E: std::fmt::Display + 'static,
    {
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
            callback.map(adapt_runtime_callback),
        )?;
        Ok(self.clone())
    }

    #[cfg(not(unix))]
    pub fn listen_path_optional<E>(
        &self,
        _path: &str,
        _callback: Option<tsonic_rust_runtime::Callable<(), Result<(), E>>>,
    ) -> NodeResult<Self>
    where
        E: std::fmt::Display + 'static,
    {
        Err(NodeError::new(
            "ERR_UNSUPPORTED_PLATFORM",
            "Unix-domain HTTP listeners are unavailable on this platform",
        ))
    }

    fn install_listener(
        &self,
        listener: RuntimeListener,
        address: ServerAddress,
        callback: Option<RuntimeListenCallback>,
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
        state.listening = true;
        RUNTIME_SERVERS.with(|servers| {
            servers.borrow_mut().insert(state.id, Rc::clone(&self.state));
        });
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
                NodeError::new("ERR_SOCKET_BAD_PORT", "bound port is outside the native range")
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
        if !self.state.borrow().listening {
            return Ok(self.clone());
        }
        self.begin_close()?;
        Ok(self.clone())
    }

    pub fn close_callback<E>(
        &self,
        callback: tsonic_rust_runtime::Callable<(Option<NodeError>,), Result<(), E>>,
    ) -> NodeResult<Self>
    where
        E: std::fmt::Display + 'static,
    {
        let callback = adapt_runtime_callback(callback);
        if let Err(error) = self.begin_close() {
            callback
                .call((Some(error),))
                .map_err(|error| NodeError::new("ERR_CALLBACK", error.to_string()))?;
            return Ok(self.clone());
        }
        self.state
            .borrow_mut()
            .pending_close_callbacks
            .push(callback);
        if !self.state.borrow().closing {
            finish_server_close_callbacks(&self.state)?;
        }
        Ok(self.clone())
    }

    pub fn close_optional<E>(
        &self,
        callback: Option<
            tsonic_rust_runtime::Callable<(Option<NodeError>,), Result<(), E>>,
        >,
    ) -> NodeResult<Self>
    where
        E: std::fmt::Display + 'static,
    {
        match callback {
            Some(callback) => self.close_callback(callback),
            None => self.close(),
        }
    }

    fn begin_close(&self) -> NodeResult<()> {
        let should_emit = {
            let mut state = self.state.borrow_mut();
            if !state.listening && !state.closing {
                return Err(NodeError::new(
                    "ERR_SERVER_NOT_RUNNING",
                    "server is not listening",
                ));
            }
            state.listener = None;
            state.listening = false;
            state.closing = true;
            state.active_connections == 0
        };
        if should_emit {
            finish_server_close(&self.state)?;
        }
        Ok(())
    }

    pub fn ref_chain(&self) -> Self {
        self.state.borrow_mut().refed = true;
        self.clone()
    }

    pub fn unref_chain(&self) -> Self {
        self.state.borrow_mut().refed = false;
        self.clone()
    }

    pub fn on_listening<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        add_server_empty_listener(&self.state, event, "listening", listener, false)?;
        Ok(self.clone())
    }

    pub fn once_listening<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        add_server_empty_listener(&self.state, event, "listening", listener, true)?;
        Ok(self.clone())
    }

    pub fn off_listening<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        remove_server_empty_listener(&self.state, event, "listening", listener.identity_key())?;
        Ok(self.clone())
    }

    pub fn on_close<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        add_server_empty_listener(&self.state, event, "close", listener, false)?;
        Ok(self.clone())
    }

    pub fn once_close<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        add_server_empty_listener(&self.state, event, "close", listener, true)?;
        Ok(self.clone())
    }

    pub fn off_close<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        remove_server_empty_listener(&self.state, event, "close", listener.identity_key())?;
        Ok(self.clone())
    }

    pub fn on_error<E: std::fmt::Display + 'static>(
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

    pub fn once_error<E: std::fmt::Display + 'static>(
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

    pub fn off_error<E>(
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

enum RuntimeConnectionIo {
    Tcp(crate::readiness::Connection),
    #[cfg(unix)]
    Unix(crate::readiness::UnixConnection),
}

impl RuntimeConnectionIo {
    fn set_interest(&mut self, readable: bool, writable: bool) -> NodeResult<()> {
        match self {
            Self::Tcp(stream) => stream.set_interest(readable, writable),
            #[cfg(unix)]
            Self::Unix(stream) => stream.set_interest(readable, writable),
        }
    }

    fn local_addr(&self) -> Option<std::net::SocketAddr> {
        match self {
            Self::Tcp(stream) => stream.local_addr().ok(),
            #[cfg(unix)]
            Self::Unix(_) => None,
        }
    }

    fn peer_addr(&self) -> Option<std::net::SocketAddr> {
        match self {
            Self::Tcp(stream) => stream.peer_addr().ok(),
            #[cfg(unix)]
            Self::Unix(_) => None,
        }
    }

    fn shutdown(&self) {
        match self {
            Self::Tcp(stream) => {
                let _ = stream.shutdown();
            }
            #[cfg(unix)]
            Self::Unix(stream) => {
                let _ = stream.shutdown();
            }
        }
    }
}

impl std::io::Read for RuntimeConnectionIo {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.read(buffer),
            #[cfg(unix)]
            Self::Unix(stream) => stream.read(buffer),
        }
    }
}

impl std::io::Write for RuntimeConnectionIo {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.write(buffer),
            #[cfg(unix)]
            Self::Unix(stream) => stream.write(buffer),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Tcp(stream) => stream.flush(),
            #[cfg(unix)]
            Self::Unix(stream) => stream.flush(),
        }
    }
}

struct OutputChunk {
    buffer: Buffer,
    offset: usize,
}

#[derive(Default)]
struct ResponseWireState {
    started: bool,
    chunked: bool,
    omit_body: bool,
    body_bytes: usize,
    content_length: Option<usize>,
    end_requested: bool,
    completion_dispatched: bool,
}

struct RuntimeConnection {
    id: u64,
    io: RuntimeConnectionIo,
    server: Weak<RefCell<RuntimeServerState>>,
    handler: RuntimeRequestHandler,
    input: Vec<u8>,
    input_start: usize,
    output: VecDeque<OutputChunk>,
    output_bytes: usize,
    request: Option<IncomingMessage>,
    response: Option<ServerResponse>,
    body: RequestBody,
    body_complete: bool,
    body_paused: bool,
    body_resume_requested: Rc<Cell<bool>>,
    keep_alive: bool,
    close_after_response: bool,
    peer_eof: bool,
    closed: bool,
    failure: Option<NodeError>,
    wire: ResponseWireState,
}

impl RuntimeConnection {
    fn new(
        io: RuntimeConnectionIo,
        server: &Rc<RefCell<RuntimeServerState>>,
        handler: RuntimeRequestHandler,
    ) -> Self {
        Self {
            id: NEXT_RUNTIME_CONNECTION_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            io,
            server: Rc::downgrade(server),
            handler,
            input: Vec::new(),
            input_start: 0,
            output: VecDeque::new(),
            output_bytes: 0,
            request: None,
            response: None,
            body: RequestBody::None,
            body_complete: false,
            body_paused: false,
            body_resume_requested: Rc::new(Cell::new(false)),
            keep_alive: false,
            close_after_response: false,
            peer_eof: false,
            closed: false,
            failure: None,
            wire: ResponseWireState::default(),
        }
    }

    fn available_input(&self) -> &[u8] {
        &self.input[self.input_start..]
    }

    fn consume_input(&mut self, amount: usize) {
        self.input_start += amount;
        if self.input_start == self.input.len() {
            self.input.clear();
            self.input_start = 0;
        } else if self.input_start >= 64 * 1024 && self.input_start * 2 >= self.input.len() {
            self.input.drain(..self.input_start);
            self.input_start = 0;
        }
    }

    fn take_body_chunk(&mut self, length: usize) -> Buffer {
        if self.input_start == 0 && self.input.len() == length {
            return Buffer::from_bytes(std::mem::take(&mut self.input));
        }
        let chunk = Buffer::from_bytes(self.available_input()[..length].to_vec());
        self.consume_input(length);
        chunk
    }

    fn queue(&mut self, buffer: Buffer) -> NodeResult<()> {
        if buffer.is_empty() {
            return Ok(());
        }
        let next = self.output_bytes.checked_add(buffer.len()).ok_or_else(|| {
            NodeError::new("ERR_HTTP_OUTPUT_OVERFLOW", "pending response bytes overflow")
        })?;
        if next > RUNTIME_MAX_PENDING_OUTPUT {
            return Err(NodeError::new(
                "ERR_HTTP_OUTPUT_OVERFLOW",
                "pending response bytes exceed the finite connection limit",
            ));
        }
        self.output.push_back(OutputChunk { buffer, offset: 0 });
        self.output_bytes = next;
        Ok(())
    }

    fn refresh_interest(&mut self) -> NodeResult<()> {
        if self.body_resume_requested.replace(false) {
            self.body_paused = false;
        }
        let readable = !self.closed
            && !self.peer_eof
            && !self.body_paused
            && self.available_input().len() < RUNTIME_MAX_PENDING_INPUT;
        let writable = !self.closed && !self.output.is_empty();
        self.io.set_interest(readable, writable)
    }

    fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.io.shutdown();
            let _ = self.io.set_interest(false, false);
        }
    }
}

struct HttpWritableBackend {
    connection: Weak<RefCell<RuntimeConnection>>,
    response: Weak<RefCell<ServerResponseState>>,
}

impl crate::stream::WritableBackend for HttpWritableBackend {
    fn bind(&self, _owner: crate::stream::WeakWritable) {}

    fn write(&self, chunk: Buffer) -> NodeResult<()> {
        let connection = self.connection.upgrade().ok_or_else(runtime_response_closed)?;
        let response = self.response.upgrade().ok_or_else(runtime_response_closed)?;
        let mut connection = connection.borrow_mut();
        begin_response(&mut connection, &mut response.borrow_mut(), None)?;
        queue_body_chunk(&mut connection, chunk)?;
        connection.refresh_interest()
    }

    fn flush(&self) -> NodeResult<()> {
        let connection = self.connection.upgrade().ok_or_else(runtime_response_closed)?;
        let response = self.response.upgrade().ok_or_else(runtime_response_closed)?;
        let mut connection = connection.borrow_mut();
        begin_response(&mut connection, &mut response.borrow_mut(), None)?;
        connection.refresh_interest()
    }

    fn finish(&self) -> NodeResult<bool> {
        let connection = self.connection.upgrade().ok_or_else(runtime_response_closed)?;
        let response = self.response.upgrade().ok_or_else(runtime_response_closed)?;
        let mut connection = connection.borrow_mut();
        begin_response(&mut connection, &mut response.borrow_mut(), Some(0))?;
        finish_response_wire(&mut connection)?;
        connection.refresh_interest()?;
        Ok(connection.output.is_empty())
    }

    fn destroy(&self) -> NodeResult<()> {
        if let Some(connection) = self.connection.upgrade() {
            let mut connection = connection.borrow_mut();
            connection.output.clear();
            connection.output_bytes = 0;
            connection.close_after_response = true;
            connection.close();
        }
        Ok(())
    }

    fn buffered_bytes(&self) -> usize {
        self.connection
            .upgrade()
            .map(|connection| connection.borrow().output_bytes)
            .unwrap_or(0)
    }
}

enum ConnectionAction {
    FramingProgress,
    Dispatch {
        handler: RuntimeRequestHandler,
        request: IncomingMessage,
        response: ServerResponse,
    },
    BodyChunk {
        request: IncomingMessage,
        chunk: Buffer,
    },
    BodyEnd(IncomingMessage),
    AbortRequest {
        request: IncomingMessage,
        error: NodeError,
    },
    WritableProgress(Writable),
    WritableComplete(Writable),
}

thread_local! {
    static RUNTIME_SERVERS: RefCell<BTreeMap<u64, Rc<RefCell<RuntimeServerState>>>> =
        const { RefCell::new(BTreeMap::new()) };
    static RUNTIME_CONNECTIONS: RefCell<BTreeMap<u64, Rc<RefCell<RuntimeConnection>>>> =
        const { RefCell::new(BTreeMap::new()) };
}

pub fn create_server_callable<E>(
    handler: tsonic_rust_runtime::Callable<RuntimeRequestArguments, Result<(), E>>,
) -> ServerHandle
where
    E: std::fmt::Display + 'static,
{
    create_runtime_server(adapt_runtime_callback(handler))
}

pub fn create_server_optional<E>(
    handler: Option<tsonic_rust_runtime::Callable<RuntimeRequestArguments, Result<(), E>>>,
) -> ServerHandle
where
    E: std::fmt::Display + 'static,
{
    match handler {
        Some(handler) => create_runtime_server(adapt_runtime_callback(handler)),
        None => create_runtime_server(tsonic_rust_runtime::Callable::new(
            |(_request, response): RuntimeRequestArguments| {
                response
                    .end_empty()
                    .map(|_| ())
                    .map_err(tsonic_rust_runtime::TsonicError::from)
            },
        )),
    }
}

fn create_runtime_server(handler: RuntimeRequestHandler) -> ServerHandle {
    let id = NEXT_RUNTIME_SERVER_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    ServerHandle {
        state: Rc::new(RefCell::new(RuntimeServerState {
            id,
            listener: None,
            handler,
            address: None,
            pending_listen_callback: None,
            pending_close_callbacks: Vec::new(),
            listening: false,
            refed: true,
            active_connections: 0,
            closing: false,
            listening_event: crate::stream::StreamEvent::default(),
            close_event: crate::stream::StreamEvent::default(),
            error_event: crate::stream::StreamEvent::default(),
        })),
    }
}

pub(crate) fn has_active_runtime_servers() -> bool {
    has_pending_detached_responses() || RUNTIME_SERVERS.with(|servers| {
        servers.borrow().values().any(|server| {
            let server = server.borrow();
            server.refed && (server.listening || server.active_connections > 0)
        })
    })
}

pub(crate) fn poll_runtime_servers() -> tsonic_rust_runtime::TsonicResult<bool> {
    let servers = RUNTIME_SERVERS.with(|servers| servers.borrow().values().cloned().collect::<Vec<_>>());
    let mut did_work = false;
    for server in servers {
        did_work |= poll_server(&server)?;
    }

    let connections = RUNTIME_CONNECTIONS.with(|connections| {
        connections.borrow().values().cloned().collect::<Vec<_>>()
    });
    for connection in connections {
        did_work |= poll_connection(&connection)?;
    }

    let closed = RUNTIME_CONNECTIONS.with(|connections| {
        connections
            .borrow()
            .iter()
            .filter_map(|(id, connection)| connection.borrow().closed.then_some(*id))
            .collect::<Vec<_>>()
    });
    for id in closed {
        if let Some(connection) =
            RUNTIME_CONNECTIONS.with(|connections| connections.borrow_mut().remove(&id))
        {
            release_connection(&connection)?;
            did_work = true;
        }
    }
    Ok(did_work | poll_detached_responses())
}

fn poll_server(
    server: &Rc<RefCell<RuntimeServerState>>,
) -> tsonic_rust_runtime::TsonicResult<bool> {
    let (callback, listening_callbacks) = {
        let mut state = server.borrow_mut();
        let callback = state.pending_listen_callback.take();
        let listeners = callback
            .as_ref()
            .map(|_| state.listening_event.emission())
            .unwrap_or_default();
        (callback, listeners)
    };
    let mut did_work = callback.is_some();
    if let Some(callback) = callback {
        callback.call(())?;
        crate::stream::invoke_event(listening_callbacks, ())
            .map_err(tsonic_rust_runtime::TsonicError::from)?;
    }

    for _ in 0..128 {
        let accepted = {
            let state = server.borrow();
            let Some(listener) = state.listener.as_ref() else {
                break;
            };
            match listener {
                RuntimeListener::Tcp(listener) => match listener.accept() {
                    Ok((stream, _)) => Some(RuntimeConnectionIo::Tcp(
                        crate::readiness::Connection::new(stream)
                            .map_err(tsonic_rust_runtime::TsonicError::from)?,
                    )),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
                    Err(error) => {
                        return Err(tsonic_rust_runtime::TsonicError::from(
                            runtime_http_io_error(error),
                        ));
                    }
                },
                #[cfg(unix)]
                RuntimeListener::Unix(listener) => match listener.accept() {
                    Ok((stream, _)) => Some(RuntimeConnectionIo::Unix(
                        crate::readiness::UnixConnection::new(stream)
                            .map_err(tsonic_rust_runtime::TsonicError::from)?,
                    )),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
                    Err(error) => {
                        return Err(tsonic_rust_runtime::TsonicError::from(
                            runtime_http_io_error(error),
                        ));
                    }
                },
            }
        };
        let Some(io) = accepted else {
            break;
        };
        let handler = server.borrow().handler.clone();
        let connection = Rc::new(RefCell::new(RuntimeConnection::new(
            io,
            server,
            handler,
        )));
        let id = connection.borrow().id;
        server.borrow_mut().active_connections += 1;
        RUNTIME_CONNECTIONS.with(|connections| {
            connections.borrow_mut().insert(id, connection);
        });
        did_work = true;
    }
    Ok(did_work)
}

fn poll_connection(
    connection: &Rc<RefCell<RuntimeConnection>>,
) -> tsonic_rust_runtime::TsonicResult<bool> {
    let mut did_work = false;
    for _ in 0..128 {
        let action = match next_connection_action(connection) {
            Ok(action) => action,
            Err(error) => {
                let mut state = connection.borrow_mut();
                state.failure = Some(error);
                state.close();
                break;
            }
        };
        let Some(action) = action else {
            break;
        };
        did_work = true;
        match action {
            ConnectionAction::FramingProgress => {}
            ConnectionAction::Dispatch {
                handler,
                request,
                response,
            } => {
                if let Err(error) = handler.call((request.clone(), response.clone())) {
                    let node_error = NodeError::new("ERR_HTTP_HANDLER", error.to_string());
                    response
                        .destroy_chain(Some(node_error.clone()))
                        .map_err(tsonic_rust_runtime::TsonicError::from)?;
                    request
                        .abort(Some(node_error))
                        .map_err(tsonic_rust_runtime::TsonicError::from)?;
                    return Err(error);
                }
            }
            ConnectionAction::BodyChunk { request, chunk } => {
                let accepted = request
                    .push_body(chunk)
                    .map_err(tsonic_rust_runtime::TsonicError::from)?;
                let mut state = connection.borrow_mut();
                state.body_paused = !accepted || request.body_pressured();
                state
                    .refresh_interest()
                    .map_err(tsonic_rust_runtime::TsonicError::from)?;
            }
            ConnectionAction::BodyEnd(request) => {
                request
                    .finish_body()
                    .map_err(tsonic_rust_runtime::TsonicError::from)?;
            }
            ConnectionAction::AbortRequest { request, error } => {
                request
                    .abort(Some(error))
                    .map_err(tsonic_rust_runtime::TsonicError::from)?;
            }
            ConnectionAction::WritableProgress(writable) => writable
                .poll_progress()
                .map_err(tsonic_rust_runtime::TsonicError::from)?,
            ConnectionAction::WritableComplete(writable) => {
                writable
                    .poll_progress()
                    .map_err(tsonic_rust_runtime::TsonicError::from)?;
                writable
                    .complete_finish()
                    .map_err(tsonic_rust_runtime::TsonicError::from)?;
                finalize_response(connection)
                    .map_err(tsonic_rust_runtime::TsonicError::from)?;
            }
        }
    }
    Ok(did_work)
}

fn next_connection_action(
    connection: &Rc<RefCell<RuntimeConnection>>,
) -> NodeResult<Option<ConnectionAction>> {
    let mut state = connection.borrow_mut();
    if state.closed {
        return Ok(None);
    }
    if state.body_resume_requested.replace(false) {
        state.body_paused = false;
    }
    if let Some(request) = state.request.clone().filter(|_| !state.body_complete && matches!(state.body, RequestBody::None)) {
        if let Some(action) = next_body_action(&mut state, request)? {
            state.refresh_interest()?;
            return Ok(Some(action));
        }
    }

    let before_output = state.output_bytes;
    flush_connection_output(&mut state)?;
    if state.output_bytes < before_output {
        if let Some(response) = &state.response {
            let writable = response.writable_handle();
            if state.wire.end_requested
                && state.output.is_empty()
                && !state.wire.completion_dispatched
            {
                state.wire.completion_dispatched = true;
                state.refresh_interest()?;
                return Ok(Some(ConnectionAction::WritableComplete(writable)));
            }
            state.refresh_interest()?;
            return Ok(Some(ConnectionAction::WritableProgress(writable)));
        }
    }

    if state.close_after_response && state.request.is_none() {
        if state.output.is_empty() {
            state.close();
        } else {
            state.refresh_interest()?;
        }
        return Ok(None);
    }

    read_connection_input(&mut state)?;
    if state.request.is_none() {
        match parse_request_head(state.available_input()) {
            Ok(Some((consumed, parsed))) => {
                state.consume_input(consumed);
                let local = state.io.local_addr();
                let remote = state.io.peer_addr();
                let request = IncomingMessage::streaming(
                    parsed.method.clone(),
                    parsed.target,
                    parsed.version,
                    parsed.headers,
                    local,
                    remote,
                    RUNTIME_READ_HIGH_WATER_MARK,
                )?;
                let resume = Rc::clone(&state.body_resume_requested);
                let wake = crate::readiness::waker()?;
                request.set_capacity_handler(move || {
                    resume.set(true);
                    wake.wake().map_err(runtime_http_io_error)
                });
                state.body = parsed.body;
                state.body_complete = false;
                state.keep_alive = parsed.keep_alive;
                state.close_after_response = false;
                state.wire = ResponseWireState {
                    omit_body: parsed.method == "HEAD",
                    ..Default::default()
                };
                let weak_connection = Rc::downgrade(connection);
                let response = ServerResponse::streaming(
                    RUNTIME_WRITE_HIGH_WATER_MARK,
                    move |response_state| {
                        Rc::new(HttpWritableBackend {
                            connection: weak_connection,
                            response: response_state,
                        })
                    },
                );
                state.request = Some(request.clone());
                state.response = Some(response.clone());
                let handler = state.handler.clone();
                state.refresh_interest()?;
                return Ok(Some(ConnectionAction::Dispatch {
                    handler,
                    request,
                    response,
                }));
            }
            Ok(None) => {}
            Err(error) => {
                state.failure = Some(error);
                queue_bad_request(&mut state)?;
                state.close_after_response = true;
                state.peer_eof = true;
                state.refresh_interest()?;
                return Ok(None);
            }
        }
    }

    if let Some(request) = state.request.clone() {
        if !state.body_complete && !state.body_paused {
            if let Some(action) = next_body_action(&mut state, request.clone())? {
                state.refresh_interest()?;
                return Ok(Some(action));
            }
        }
        if state.peer_eof && !state.body_complete {
            state.close_after_response = true;
            state.close();
            return Ok(Some(ConnectionAction::AbortRequest {
                request,
                error: NodeError::new(
                    "HPE_INVALID_EOF_STATE",
                    "request body ended before its framing completed",
                ),
            }));
        }
    }

    if state.close_after_response && state.output.is_empty() {
        state.close();
    }
    state.refresh_interest()?;
    Ok(None)
}

fn next_body_action(
    state: &mut RuntimeConnection,
    request: IncomingMessage,
) -> NodeResult<Option<ConnectionAction>> {
    match &mut state.body {
        RequestBody::None => {
            state.body_complete = true;
            Ok(Some(ConnectionAction::BodyEnd(request)))
        }
        RequestBody::ContentLength { remaining } => {
            if *remaining == 0 {
                state.body_complete = true;
                return Ok(Some(ConnectionAction::BodyEnd(request)));
            }
            let length = (*remaining).min(state.available_input().len()).min(16 * 1024);
            if length == 0 {
                return Ok(None);
            }
            let chunk = state.take_body_chunk(length);
            let RequestBody::ContentLength { remaining } = &mut state.body else {
                unreachable!("content-length body remained selected")
            };
            *remaining -= length;
            Ok(Some(ConnectionAction::BodyChunk { request, chunk }))
        }
        RequestBody::Chunked(decoder) => {
            let step = decoder.step(&state.input[state.input_start..])?;
            match step {
                ChunkStep::NeedInput => Ok(None),
                ChunkStep::Consumed(consumed) => {
                    state.consume_input(consumed);
                    Ok(Some(ConnectionAction::FramingProgress))
                }
                ChunkStep::Data { consumed, .. } => {
                    let chunk = state.take_body_chunk(consumed);
                    Ok(Some(ConnectionAction::BodyChunk { request, chunk }))
                }
                ChunkStep::Complete(consumed) => {
                    state.consume_input(consumed);
                    state.body_complete = true;
                    Ok(Some(ConnectionAction::BodyEnd(request)))
                }
            }
        }
    }
}

fn read_connection_input(state: &mut RuntimeConnection) -> NodeResult<()> {
    if state.body_paused || state.peer_eof || state.closed {
        return Ok(());
    }
    let mut total = 0;
    loop {
        let pending = state.available_input().len();
        if pending >= RUNTIME_MAX_PENDING_INPUT {
            break;
        }
        let capacity = (RUNTIME_MAX_PENDING_INPUT - pending).min(16 * 1024);
        let start = state.input.len();
        state.input.resize(start + capacity, 0);
        match state.io.read(&mut state.input[start..]) {
            Ok(0) => {
                state.input.truncate(start);
                state.peer_eof = true;
                break;
            }
            Ok(read) => {
                state.input.truncate(start + read);
                total += read;
                if total >= RUNTIME_IO_BUDGET {
                    break;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                state.input.truncate(start);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                state.input.truncate(start);
            }
            Err(error) => {
                state.input.truncate(start);
                return Err(runtime_http_io_error(error));
            }
        }
    }
    Ok(())
}

fn flush_connection_output(state: &mut RuntimeConnection) -> NodeResult<()> {
    let mut written = 0;
    while written < RUNTIME_IO_BUDGET {
        let Some(mut chunk) = state.output.pop_front() else {
            break;
        };
        let result = chunk
            .buffer
            .with_bytes(|bytes| state.io.write(&bytes[chunk.offset..]));
        match result {
            Ok(0) => {
                state.output.push_front(chunk);
                return Err(NodeError::new(
                    "ERR_HTTP_WRITE_ZERO",
                    "HTTP transport accepted no response bytes",
                ));
            }
            Ok(count) => {
                chunk.offset += count;
                state.output_bytes = state.output_bytes.saturating_sub(count);
                written += count;
                if chunk.offset < chunk.buffer.len() {
                    state.output.push_front(chunk);
                    break;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                state.output.push_front(chunk);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                state.output.push_front(chunk);
            }
            Err(error) => return Err(runtime_http_io_error(error)),
        }
    }
    Ok(())
}

fn begin_response(
    connection: &mut RuntimeConnection,
    response: &mut ServerResponseState,
    known_body_length: Option<usize>,
) -> NodeResult<()> {
    if connection.wire.started {
        return Ok(());
    }
    ensure_status_code(response.status_code)?;
    let content_length = response
        .headers
        .get("content-length")?
        .map(|value| parse_response_content_length(&value))
        .transpose()?;
    let transfer_encoding = response.headers.get("transfer-encoding")?;
    if content_length.is_some() && transfer_encoding.is_some() {
        return Err(NodeError::new(
            "ERR_HTTP_CONTENT_LENGTH_MISMATCH",
            "content-length and transfer-encoding cannot be sent together",
        ));
    }
    let status_code = response.status_code as u16;
    let omit_body = connection.wire.omit_body || matches!(status_code, 204 | 304);
    let server_closing = connection
        .server
        .upgrade()
        .is_some_and(|server| server.borrow().closing);
    let chunked = match transfer_encoding.as_deref() {
        Some(value) if value.eq_ignore_ascii_case("chunked") => !omit_body,
        Some(_) => {
            return Err(NodeError::new(
                "ERR_HTTP_INVALID_HEADER_VALUE",
                "the only supported response transfer-encoding is chunked",
            ));
        }
        None => !omit_body && content_length.is_none() && known_body_length.is_none(),
    };
    connection.wire.started = true;
    connection.wire.chunked = chunked;
    connection.wire.omit_body = omit_body;
    connection.wire.content_length = content_length.or(known_body_length);
    response.headers_sent = true;

    let mut head = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status_code, response.status_message
    );
    for (name, values) in response.headers.entries() {
        for value in values {
            head.push_str(&name);
            head.push_str(": ");
            head.push_str(&value);
            head.push_str("\r\n");
        }
    }
    if content_length.is_none() && transfer_encoding.is_none() && !omit_body {
        if let Some(length) = known_body_length {
            head.push_str(&format!("Content-Length: {length}\r\n"));
        } else {
            head.push_str("Transfer-Encoding: chunked\r\n");
        }
    }
    if !response.headers.contains("connection")? {
        if connection.keep_alive && !connection.close_after_response && !server_closing {
            head.push_str("Connection: keep-alive\r\n");
        } else {
            head.push_str("Connection: close\r\n");
            connection.close_after_response = true;
        }
    } else if response
        .headers
        .get("connection")?
        .is_some_and(|value| value.eq_ignore_ascii_case("close"))
    {
        connection.close_after_response = true;
    }
    head.push_str("\r\n");
    connection.queue(Buffer::from_bytes(head.into_bytes()))
}

fn queue_body_chunk(connection: &mut RuntimeConnection, chunk: Buffer) -> NodeResult<()> {
    if chunk.is_empty() || connection.wire.omit_body {
        return Ok(());
    }
    connection.wire.body_bytes = connection
        .wire
        .body_bytes
        .checked_add(chunk.len())
        .ok_or_else(|| NodeError::new("ERR_HTTP_OUTPUT_OVERFLOW", "response size overflow"))?;
    if let Some(length) = connection.wire.content_length {
        if connection.wire.body_bytes > length {
            return Err(NodeError::new(
                "ERR_HTTP_CONTENT_LENGTH_MISMATCH",
                "response body exceeds content-length",
            ));
        }
    }
    if connection.wire.chunked {
        connection.queue(Buffer::from_bytes(
            format!("{:X}\r\n", chunk.len()).into_bytes(),
        ))?;
        connection.queue(chunk)?;
        connection.queue(Buffer::from_bytes(b"\r\n".to_vec()))
    } else {
        connection.queue(chunk)
    }
}

fn finish_response_wire(connection: &mut RuntimeConnection) -> NodeResult<()> {
    if connection.wire.end_requested {
        return Err(NodeError::new(
            "ERR_STREAM_WRITE_AFTER_END",
            "response has already ended",
        ));
    }
    if let Some(length) = connection.wire.content_length {
        if !connection.wire.omit_body && connection.wire.body_bytes != length {
            return Err(NodeError::new(
                "ERR_HTTP_CONTENT_LENGTH_MISMATCH",
                "response body does not match content-length",
            ));
        }
    }
    if connection.wire.chunked && !connection.wire.omit_body {
        connection.queue(Buffer::from_bytes(b"0\r\n\r\n".to_vec()))?;
    }
    connection.wire.end_requested = true;
    if !connection.body_complete && !matches!(connection.body, RequestBody::None) {
        connection.close_after_response = true;
    }
    Ok(())
}

fn finalize_response(connection: &Rc<RefCell<RuntimeConnection>>) -> NodeResult<()> {
    let mut state = connection.borrow_mut();
    let server_closing = state
        .server
        .upgrade()
        .is_some_and(|server| server.borrow().closing);
    if !state.body_complete || !state.keep_alive || state.close_after_response || state.peer_eof || server_closing {
        state.close();
        return Ok(());
    }
    state.request = None;
    state.response = None;
    state.body = RequestBody::None;
    state.body_complete = false;
    state.body_paused = false;
    state.keep_alive = false;
    state.wire = ResponseWireState::default();
    state.refresh_interest()
}

fn queue_bad_request(connection: &mut RuntimeConnection) -> NodeResult<()> {
    connection.output.clear();
    connection.output_bytes = 0;
    connection.queue(Buffer::from_bytes(
        b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .to_vec(),
    ))
}

fn release_connection(
    connection: &Rc<RefCell<RuntimeConnection>>,
) -> tsonic_rust_runtime::TsonicResult<()> {
    let (server, request, response, failure) = {
        let state = connection.borrow();
        (
            state.server.upgrade(),
            state.request.clone(),
            state.response.clone(),
            state.failure.clone(),
        )
    };
    if let Some(request) = request {
        if !request.complete() && !request.destroyed() {
            request
                .abort(Some(failure.clone().unwrap_or_else(|| NodeError::new(
                    "ECONNRESET",
                    "HTTP connection closed before request completion",
                ))))
                .map_err(tsonic_rust_runtime::TsonicError::from)?;
        }
    }
    if let Some(response) = response {
        if !response.writable_finished() && !response.destroyed() {
            response
                .destroy_chain(Some(failure.unwrap_or_else(|| NodeError::new(
                    "ECONNRESET",
                    "HTTP connection closed before response completion",
                ))))
                .map_err(tsonic_rust_runtime::TsonicError::from)?;
        }
    }
    if let Some(server) = server {
        let should_close = {
            let mut server = server.borrow_mut();
            server.active_connections = server.active_connections.saturating_sub(1);
            server.closing && server.active_connections == 0
        };
        if should_close {
            finish_server_close(&server).map_err(tsonic_rust_runtime::TsonicError::from)?;
        }
    }
    Ok(())
}

fn finish_server_close(server: &Rc<RefCell<RuntimeServerState>>) -> NodeResult<()> {
    let (id, callbacks) = {
        let mut state = server.borrow_mut();
        state.closing = false;
        state.address = None;
        (state.id, state.close_event.emission())
    };
    RUNTIME_SERVERS.with(|servers| {
        servers.borrow_mut().remove(&id);
    });
    crate::stream::invoke_event(callbacks, ())
        .and_then(|()| finish_server_close_callbacks(server))
}

fn finish_server_close_callbacks(server: &Rc<RefCell<RuntimeServerState>>) -> NodeResult<()> {
    let callbacks = std::mem::take(&mut server.borrow_mut().pending_close_callbacks);
    for callback in callbacks {
        callback
            .call((None,))
            .map_err(|error| NodeError::new("ERR_CALLBACK", error.to_string()))?;
    }
    Ok(())
}

fn add_server_empty_listener<E: std::fmt::Display + 'static>(
    server: &Rc<RefCell<RuntimeServerState>>,
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

fn remove_server_empty_listener(
    server: &Rc<RefCell<RuntimeServerState>>,
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

fn adapt_runtime_callback<TArguments, E>(
    callback: tsonic_rust_runtime::Callable<TArguments, Result<(), E>>,
) -> tsonic_rust_runtime::Callable<TArguments, tsonic_rust_runtime::TsonicResult<()>>
where
    TArguments: 'static,
    E: std::fmt::Display + 'static,
{
    tsonic_rust_runtime::Callable::new(move |arguments| {
        callback
            .call(arguments)
            .map_err(crate::error::callback_runtime_error)
    })
}

fn bind_tcp_listener(host: &str, port: u16, backlog: i32) -> NodeResult<std::net::TcpListener> {
    use std::net::ToSocketAddrs as _;
    let address = (host, port)
        .to_socket_addrs()
        .map_err(runtime_http_io_error)?
        .next()
        .ok_or_else(|| NodeError::new("EADDRNOTAVAIL", "listen host resolved to no address"))?;
    let domain = if address.is_ipv4() {
        socket2::Domain::IPV4
    } else {
        socket2::Domain::IPV6
    };
    let socket = socket2::Socket::new(domain, socket2::Type::STREAM, Some(socket2::Protocol::TCP))
        .map_err(runtime_http_io_error)?;
    socket.set_reuse_address(true).map_err(runtime_http_io_error)?;
    socket
        .bind(&socket2::SockAddr::from(address))
        .map_err(runtime_http_io_error)?;
    socket.listen(backlog).map_err(runtime_http_io_error)?;
    Ok(socket.into())
}

fn parse_response_content_length(value: &str) -> NodeResult<usize> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(NodeError::new(
            "ERR_HTTP_INVALID_HEADER_VALUE",
            "content-length must be an unsigned decimal integer",
        ));
    }
    value.parse().map_err(|_| {
        NodeError::new(
            "ERR_HTTP_INVALID_HEADER_VALUE",
            "content-length exceeds native limits",
        )
    })
}

fn runtime_response_closed() -> NodeError {
    NodeError::new("ERR_STREAM_WRITE_AFTER_END", "response is no longer writable")
}

fn runtime_http_io_error(error: std::io::Error) -> NodeError {
    NodeError::new("ERR_HTTP_IO", error.to_string())
}

pub(crate) trait RuntimeTransport: std::io::Read + std::io::Write {
    fn peer_addr(&self) -> std::io::Result<std::net::SocketAddr>;
}

impl RuntimeTransport for std::net::TcpStream {
    fn peer_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        std::net::TcpStream::peer_addr(self)
    }
}

pub(crate) fn accept_runtime_transport(
    mut stream: Box<dyn RuntimeTransport>,
    handler: RuntimeRequestHandler,
) -> tsonic_rust_runtime::TsonicResult<()> {
    let mut input = Vec::new();
    let parsed = loop {
        if let Some(parsed) = parse_request_head(&input).map_err(tsonic_rust_runtime::TsonicError::from)? {
            break parsed;
        }
        let start = input.len();
        input.resize(start + 16 * 1024, 0);
        let read = stream
            .read(&mut input[start..])
            .map_err(runtime_http_io_error)
            .map_err(tsonic_rust_runtime::TsonicError::from)?;
        input.truncate(start + read);
        if read == 0 {
            return Err(tsonic_rust_runtime::TsonicError::from(NodeError::new(
                "HPE_INVALID_EOF_STATE",
                "request ended before headers completed",
            )));
        }
    };
    let (consumed, head) = parsed;
    let body = read_blocking_body(&mut stream, &input[consumed..], head.body)
        .map_err(tsonic_rust_runtime::TsonicError::from)?;
    let remote = stream.peer_addr().ok();
    let request = IncomingMessage::streaming(
        head.method.clone(),
        head.target,
        head.version,
        head.headers,
        None,
        remote,
        RUNTIME_READ_HIGH_WATER_MARK,
    )
    .map_err(tsonic_rust_runtime::TsonicError::from)?;
    if !body.is_empty() {
        request
            .push_body(Buffer::from_bytes(body))
            .map_err(tsonic_rust_runtime::TsonicError::from)?;
    }
    request
        .finish_body()
        .map_err(tsonic_rust_runtime::TsonicError::from)?;
    let response = ServerResponse::streaming(RUNTIME_WRITE_HIGH_WATER_MARK, move |state| {
        Rc::new(DetachedHttpWritableBackend::new(stream, state, head.method == "HEAD"))
    });
    handler.call((request, response.clone()))?;
    retain_detached_response(response);
    Ok(())
}

fn read_blocking_body(
    stream: &mut Box<dyn RuntimeTransport>,
    initial: &[u8],
    body: RequestBody,
) -> NodeResult<Vec<u8>> {
    let mut input = initial.to_vec();
    match body {
        RequestBody::None => Ok(Vec::new()),
        RequestBody::ContentLength { remaining } => {
            if remaining > RUNTIME_MAX_MATERIALIZED_BODY_SIZE {
                return Err(NodeError::new(
                    "HPE_BODY_OVERFLOW",
                    "materialized request body exceeds the finite limit",
                ));
            }
            while input.len() < remaining {
                let start = input.len();
                input.resize(start + 16 * 1024, 0);
                let read = stream.read(&mut input[start..]).map_err(runtime_http_io_error)?;
                input.truncate(start + read);
                if read == 0 {
                    return Err(NodeError::new(
                        "HPE_INVALID_EOF_STATE",
                        "request body ended before content-length",
                    ));
                }
            }
            input.truncate(remaining);
            Ok(input)
        }
        RequestBody::Chunked(mut decoder) => {
            let mut decoded = Vec::new();
            let mut start = 0;
            loop {
                match decoder.step(&input[start..])? {
                    ChunkStep::NeedInput => {
                        if start > 0 {
                            input.drain(..start);
                            start = 0;
                        }
                        let prior = input.len();
                        input.resize(prior + 16 * 1024, 0);
                        let read = stream.read(&mut input[prior..]).map_err(runtime_http_io_error)?;
                        input.truncate(prior + read);
                        if read == 0 {
                            return Err(NodeError::new(
                                "HPE_INVALID_EOF_STATE",
                                "chunked request body ended early",
                            ));
                        }
                    }
                    ChunkStep::Consumed(count) => start += count,
                    ChunkStep::Data { consumed, length } => {
                        if decoded.len().saturating_add(length) > RUNTIME_MAX_MATERIALIZED_BODY_SIZE {
                            return Err(NodeError::new(
                                "HPE_BODY_OVERFLOW",
                                "materialized request body exceeds the finite limit",
                            ));
                        }
                        decoded.extend_from_slice(&input[start..start + length]);
                        start += consumed;
                    }
                    ChunkStep::Complete(_) => return Ok(decoded),
                }
            }
        }
    }
}
