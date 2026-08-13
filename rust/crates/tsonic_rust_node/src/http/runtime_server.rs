const RUNTIME_MAX_BODY_SIZE: usize = 64 * 1024 * 1024;
const RUNTIME_IO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

static NEXT_RUNTIME_SERVER_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

type RuntimeRequestArguments = (IncomingMessage, ServerResponseHandle);
type RuntimeRequestHandler =
    tsonic_rust_runtime::Callable<RuntimeRequestArguments, tsonic_rust_runtime::TsonicResult<()>>;
type RuntimeListenCallback =
    tsonic_rust_runtime::Callable<(), tsonic_rust_runtime::TsonicResult<()>>;
type InfallibleRuntimeRequestHandler =
    tsonic_rust_runtime::Callable<RuntimeRequestArguments, ()>;
type InfallibleRuntimeListenCallback = tsonic_rust_runtime::Callable<(), ()>;

struct RuntimeServer {
    listener: std::net::TcpListener,
    handler: RuntimeRequestHandler,
    listening_callback: Option<RuntimeListenCallback>,
}

struct AcceptedConnection {
    stream: std::net::TcpStream,
    handler: RuntimeRequestHandler,
}

struct PendingResponse {
    stream: std::net::TcpStream,
    response: ServerResponseHandle,
    omit_body: bool,
}

thread_local! {
    static RUNTIME_SERVERS: std::cell::RefCell<std::collections::BTreeMap<u64, RuntimeServer>> =
        std::cell::RefCell::new(std::collections::BTreeMap::new());
    static PENDING_RESPONSES: std::cell::RefCell<Vec<PendingResponse>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

#[derive(Clone)]
pub struct ServerHandle {
    id: u64,
    handler: RuntimeRequestHandler,
}

impl ServerHandle {
    pub fn listen(
        &self,
        port: i32,
        host: &str,
        callback: InfallibleRuntimeListenCallback,
    ) -> NodeResult<Self> {
        self.listen_fallible(
            port,
            host,
            tsonic_rust_runtime::Callable::new(move |()| {
                callback.call(());
                Ok(())
            }),
        )
    }

    pub fn listen_fallible(
        &self,
        port: i32,
        host: &str,
        callback: RuntimeListenCallback,
    ) -> NodeResult<Self> {
        let port = u16::try_from(port)
            .map_err(|_| NodeError::new("ERR_SOCKET_BAD_PORT", "port must be between 0 and 65535"))?;
        let listener = std::net::TcpListener::bind((host, port))
            .map_err(runtime_http_io_error)?;
        listener.set_nonblocking(true).map_err(runtime_http_io_error)?;
        RUNTIME_SERVERS.with(|servers| {
            let mut servers = servers.borrow_mut();
            if servers.contains_key(&self.id) {
                return Err(NodeError::new(
                    "ERR_SERVER_ALREADY_LISTEN",
                    "server is already listening",
                ));
            }
            servers.insert(
                self.id,
                RuntimeServer {
                    listener,
                    handler: self.handler.clone(),
                    listening_callback: Some(callback),
                },
            );
            Ok(())
        })?;
        Ok(self.clone())
    }

    pub fn listen_default_host(
        &self,
        port: i32,
        callback: InfallibleRuntimeListenCallback,
    ) -> NodeResult<Self> {
        self.listen(port, "0.0.0.0", callback)
    }

    pub fn listen_default_host_fallible(
        &self,
        port: i32,
        callback: RuntimeListenCallback,
    ) -> NodeResult<Self> {
        self.listen_fallible(port, "0.0.0.0", callback)
    }

    pub fn close(&self) {
        RUNTIME_SERVERS.with(|servers| {
            servers.borrow_mut().remove(&self.id);
        });
    }

    pub fn local_port(&self) -> NodeResult<u16> {
        RUNTIME_SERVERS.with(|servers| {
            servers
                .borrow()
                .get(&self.id)
                .ok_or_else(|| NodeError::new("ERR_SERVER_NOT_RUNNING", "server is not listening"))?
                .listener
                .local_addr()
                .map(|address| address.port())
                .map_err(runtime_http_io_error)
        })
    }
}

#[derive(Clone)]
pub struct ServerResponseHandle {
    state: std::rc::Rc<std::cell::RefCell<RuntimeResponseState>>,
}

struct RuntimeResponseState {
    status_code: i32,
    headers: std::collections::BTreeMap<String, String>,
    body: Vec<u8>,
    finished: bool,
}

impl ServerResponseHandle {
    fn new() -> Self {
        Self {
            state: std::rc::Rc::new(std::cell::RefCell::new(RuntimeResponseState {
                status_code: 200,
                headers: std::collections::BTreeMap::new(),
                body: Vec::new(),
                finished: false,
            })),
        }
    }

    pub fn status_code(&self) -> i32 {
        self.state.borrow().status_code
    }

    pub fn set_status_code(&self, status_code: i32) {
        self.state.borrow_mut().status_code = status_code;
    }

    pub fn set_header(&self, name: &str, value: &str) -> NodeResult<()> {
        validate_header_value(name, value)?;
        self.state
            .borrow_mut()
            .headers
            .insert(name.to_ascii_lowercase(), value.to_string());
        Ok(())
    }

    pub fn end_empty(&self) {
        self.finish(Vec::new());
    }

    pub fn end_string(&self, chunk: &str) {
        self.finish(chunk.as_bytes().to_vec());
    }

    pub fn end_buffer(&self, chunk: Buffer) {
        self.finish(chunk.as_bytes());
    }

    fn finish(&self, body: Vec<u8>) {
        let mut state = self.state.borrow_mut();
        state.body = body;
        state.finished = true;
    }

    fn is_finished(&self) -> bool {
        self.state.borrow().finished
    }
}

pub fn create_server_callable(handler: InfallibleRuntimeRequestHandler) -> ServerHandle {
    create_runtime_server(tsonic_rust_runtime::Callable::new(move |arguments| {
        handler.call(arguments);
        Ok(())
    }))
}

pub fn create_server_fallible_callable(
    handler: RuntimeRequestHandler,
) -> tsonic_rust_runtime::TsonicResult<ServerHandle> {
    Ok(create_runtime_server(handler))
}

fn create_runtime_server(handler: RuntimeRequestHandler) -> ServerHandle {
    ServerHandle {
        id: NEXT_RUNTIME_SERVER_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        handler,
    }
}

pub(crate) fn has_active_runtime_servers() -> bool {
    let servers = RUNTIME_SERVERS.with(|servers| !servers.borrow().is_empty());
    servers || PENDING_RESPONSES.with(|responses| !responses.borrow().is_empty())
}

pub(crate) fn poll_runtime_servers() -> tsonic_rust_runtime::TsonicResult<bool> {
    let (listen_callbacks, accepted) = RUNTIME_SERVERS.with(|servers| {
        let mut servers = servers.borrow_mut();
        let mut listen_callbacks = Vec::new();
        let mut accepted = Vec::new();
        for server in servers.values_mut() {
            if let Some(callback) = server.listening_callback.take() {
                listen_callbacks.push(callback);
            }
            match server.listener.accept() {
                Ok((stream, _)) => accepted.push(AcceptedConnection {
                    stream,
                    handler: server.handler.clone(),
                }),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(runtime_http_io_error(error)),
            }
        }
        Ok((listen_callbacks, accepted))
    })?;

    let mut did_work = !listen_callbacks.is_empty() || !accepted.is_empty();
    for callback in listen_callbacks {
        callback.call(())?;
    }
    for accepted in accepted {
        if let Some(pending) = handle_runtime_connection(accepted)? {
            PENDING_RESPONSES.with(|responses| responses.borrow_mut().push(pending));
        }
    }

    let ready = PENDING_RESPONSES.with(|responses| {
        let mut responses = responses.borrow_mut();
        let mut ready = Vec::new();
        let mut index = 0;
        while index < responses.len() {
            if responses[index].response.is_finished() {
                ready.push(responses.swap_remove(index));
            } else {
                index += 1;
            }
        }
        ready
    });
    did_work |= !ready.is_empty();
    for pending in ready {
        write_runtime_response(pending)?;
    }
    Ok(did_work)
}

fn handle_runtime_connection(
    mut accepted: AcceptedConnection,
) -> tsonic_rust_runtime::TsonicResult<Option<PendingResponse>> {
    use std::io::Write;

    let _ = accepted.stream.set_nonblocking(false);
    let _ = accepted.stream.set_read_timeout(Some(RUNTIME_IO_TIMEOUT));
    let parsed = read_runtime_request(&mut accepted.stream);
    let (request, omit_body) = match parsed {
        Ok(request) => {
            let omit_body = request.method == "HEAD";
            (request, omit_body)
        }
        Err(_) => {
            accepted
                .stream
                .write_all(
                    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .map_err(runtime_http_io_error)?;
            return Ok(None);
        }
    };
    let response = ServerResponseHandle::new();
    accepted.handler.call((request, response.clone()))?;
    Ok(Some(PendingResponse {
        stream: accepted.stream,
        response,
        omit_body,
    }))
}

fn read_runtime_request(stream: &mut std::net::TcpStream) -> NodeResult<IncomingMessage> {
    use std::io::Read;

    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let header_end = loop {
        let count = stream.read(&mut buffer).map_err(runtime_http_io_error)?;
        if count == 0 {
            return Err(NodeError::new("HPE_INVALID_EOF_STATE", "request ended before headers"));
        }
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(index) = find_header_end(&bytes) {
            break index;
        }
        if bytes.len() > MAX_HEADER_SIZE {
            return Err(NodeError::new("HPE_HEADER_OVERFLOW", "request headers exceed limit"));
        }
    };

    let head = std::str::from_utf8(&bytes[..header_end])
        .map_err(|error| NodeError::new("HPE_INVALID_HEADER_TOKEN", error.to_string()))?;
    let mut lines = head.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| NodeError::new("HPE_INVALID_REQUEST", "missing request line"))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| NodeError::new("HPE_INVALID_METHOD", "missing method"))?
        .to_string();
    let url = request_parts
        .next()
        .ok_or_else(|| NodeError::new("HPE_INVALID_URL", "missing request target"))?
        .to_string();
    let version = request_parts
        .next()
        .and_then(|value| value.strip_prefix("HTTP/"))
        .ok_or_else(|| NodeError::new("HPE_INVALID_VERSION", "missing HTTP version"))?
        .to_string();
    if request_parts.next().is_some() {
        return Err(NodeError::new("HPE_INVALID_REQUEST", "invalid request line"));
    }

    let mut headers = Vec::new();
    let mut content_length = 0_usize;
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| NodeError::new("HPE_INVALID_HEADER_TOKEN", "invalid header"))?;
        let value = value.trim();
        validate_header_value(name, value)?;
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(NodeError::new(
                "HPE_UNSUPPORTED_TRANSFER_ENCODING",
                "chunked request bodies are not supported",
            ));
        }
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value
                .parse::<usize>()
                .map_err(|error| NodeError::new("HPE_INVALID_CONTENT_LENGTH", error.to_string()))?;
            if content_length > RUNTIME_MAX_BODY_SIZE {
                return Err(NodeError::new("HPE_BODY_OVERFLOW", "request body exceeds limit"));
            }
        }
        headers.push((name.to_string(), value.to_string()));
    }

    let body_start = header_end + 4;
    while bytes.len().saturating_sub(body_start) < content_length {
        let count = stream.read(&mut buffer).map_err(runtime_http_io_error)?;
        if count == 0 {
            return Err(NodeError::new("HPE_INVALID_EOF_STATE", "request body ended early"));
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    let mut request = IncomingMessage::new(
        method,
        url,
        bytes[body_start..body_start + content_length].to_vec(),
    );
    request.http_version = version.clone();
    let mut version_parts = version.split('.');
    request.http_version_major = version_parts.next().and_then(|v| v.parse().ok()).unwrap_or(1);
    request.http_version_minor = version_parts.next().and_then(|v| v.parse().ok()).unwrap_or(1);
    if let Ok(peer) = stream.peer_addr() {
        request.socket = Some(net::SocketAddress::new(&peer.ip().to_string(), peer.port())?);
    }
    for (name, value) in headers {
        request.set_header(&name, &value);
    }
    Ok(request)
}

fn write_runtime_response(mut pending: PendingResponse) -> NodeResult<()> {
    use std::io::Write;

    let state = pending.response.state.borrow();
    let status_code = u16::try_from(state.status_code)
        .ok()
        .filter(|code| (100..=999).contains(code))
        .ok_or_else(|| NodeError::new("ERR_HTTP_INVALID_STATUS_CODE", "status code must be 100 through 999"))?;
    let omit_body = pending.omit_body || matches!(status_code, 204 | 304);
    let body = if omit_body { &[][..] } else { state.body.as_slice() };
    let mut response = format!(
        "HTTP/1.1 {status_code} {}\r\n",
        canonical_status_message(status_code)
    );
    for (name, value) in &state.headers {
        response.push_str(name);
        response.push_str(": ");
        response.push_str(value);
        response.push_str("\r\n");
    }
    if !state.headers.contains_key("content-length") {
        response.push_str(&format!("Content-Length: {}\r\n", state.body.len()));
    }
    if !state.headers.contains_key("connection") {
        response.push_str("Connection: close\r\n");
    }
    response.push_str("\r\n");
    pending
        .stream
        .write_all(response.as_bytes())
        .and_then(|_| pending.stream.write_all(body))
        .and_then(|_| pending.stream.flush())
        .map_err(runtime_http_io_error)
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn runtime_http_io_error(error: std::io::Error) -> NodeError {
    NodeError::new("ERR_HTTP_IO", error.to_string())
}
