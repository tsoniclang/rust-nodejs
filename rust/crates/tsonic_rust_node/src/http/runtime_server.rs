use std::io::Write as _;

const RUNTIME_MAX_BODY_SIZE: usize = 64 * 1024 * 1024;
const RUNTIME_IO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

static NEXT_RUNTIME_SERVER_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

pub(crate) type RuntimeRequestArguments = (IncomingMessage, ServerResponseHandle);
pub(crate) type RuntimeRequestHandler =
    tsonic_rust_runtime::Callable<RuntimeRequestArguments, tsonic_rust_runtime::TsonicResult<()>>;
type RuntimeListenCallback =
    tsonic_rust_runtime::Callable<(), tsonic_rust_runtime::TsonicResult<()>>;

struct RuntimeServer {
    listener: std::net::TcpListener,
    handler: RuntimeRequestHandler,
    listening_callback: Option<RuntimeListenCallback>,
}

pub(crate) trait RuntimeTransport: std::io::Read + std::io::Write {
    fn peer_addr(&self) -> std::io::Result<std::net::SocketAddr>;
}

impl RuntimeTransport for std::net::TcpStream {
    fn peer_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        std::net::TcpStream::peer_addr(self)
    }
}

struct AcceptedConnection {
    stream: Box<dyn RuntimeTransport>,
    handler: RuntimeRequestHandler,
}

struct PendingResponse {
    response: ServerResponseHandle,
}

thread_local! {
    static RUNTIME_SERVERS: std::cell::RefCell<std::collections::BTreeMap<u64, RuntimeServer>> =
        const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
    static PENDING_RESPONSES: std::cell::RefCell<Vec<PendingResponse>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

#[derive(Clone)]
pub struct ServerHandle {
    id: u64,
    handler: RuntimeRequestHandler,
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
                    listening_callback: Some(adapt_runtime_callback(callback)),
                },
            );
            Ok(())
        })?;
        Ok(self.clone())
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
    stream: Option<Box<dyn RuntimeTransport>>,
    omit_body: bool,
    status_code: i32,
    headers: std::collections::BTreeMap<String, String>,
    headers_sent: bool,
    chunked: bool,
    finished: bool,
}

impl ServerResponseHandle {
    fn new(stream: Box<dyn RuntimeTransport>, omit_body: bool) -> Self {
        Self {
            state: std::rc::Rc::new(std::cell::RefCell::new(RuntimeResponseState {
                stream: Some(stream),
                omit_body,
                status_code: 200,
                headers: std::collections::BTreeMap::new(),
                headers_sent: false,
                chunked: false,
                finished: false,
            })),
        }
    }

    pub fn status_code(&self) -> i32 {
        self.state.borrow().status_code
    }

    pub fn set_status_code(&self, status_code: i32) {
        let mut state = self.state.borrow_mut();
        if !state.headers_sent {
            state.status_code = status_code;
        }
    }

    pub fn set_header(&self, name: &str, value: &str) -> NodeResult<()> {
        validate_header_value(name, value)?;
        let mut state = self.state.borrow_mut();
        if state.headers_sent {
            return Err(NodeError::new(
                "ERR_HTTP_HEADERS_SENT",
                "response headers have already been sent",
            ));
        }
        state.headers.insert(name.to_ascii_lowercase(), value.to_string());
        Ok(())
    }

    pub fn write_buffer(&self, chunk: Buffer) -> NodeResult<bool> {
        chunk.with_bytes(|bytes| self.write_bytes(bytes))?;
        Ok(true)
    }

    pub fn end_empty(&self) -> NodeResult<()> {
        self.finish(&[])
    }

    pub fn end_string(&self, chunk: &str) -> NodeResult<()> {
        self.finish(chunk.as_bytes())
    }

    pub fn end_buffer(&self, chunk: Buffer) -> NodeResult<()> {
        chunk.with_bytes(|bytes| self.finish(bytes))
    }

    fn write_bytes(&self, bytes: &[u8]) -> NodeResult<()> {
        let mut state = self.state.borrow_mut();
        if state.finished {
            return Err(NodeError::new(
                "ERR_STREAM_WRITE_AFTER_END",
                "response has already ended",
            ));
        }
        begin_runtime_response(&mut state, None)?;
        write_runtime_body_chunk(&mut state, bytes)
    }

    fn finish(&self, final_bytes: &[u8]) -> NodeResult<()> {
        let mut state = self.state.borrow_mut();
        if state.finished {
            return Err(NodeError::new(
                "ERR_STREAM_WRITE_AFTER_END",
                "response has already ended",
            ));
        }
        if state.headers_sent {
            write_runtime_body_chunk(&mut state, final_bytes)?;
        } else {
            begin_runtime_response(&mut state, Some(final_bytes.len()))?;
            write_runtime_body_bytes(&mut state, final_bytes)?;
        }
        if state.chunked && !runtime_response_omits_body(&state)? {
            state
                .stream
                .as_mut()
                .ok_or_else(runtime_response_closed)?
                .write_all(b"0\r\n\r\n")
                .map_err(runtime_http_io_error)?;
        }
        state
            .stream
            .as_mut()
            .ok_or_else(runtime_response_closed)?
            .flush()
            .map_err(runtime_http_io_error)?;
        state.stream = None;
        state.finished = true;
        Ok(())
    }

    fn is_finished(&self) -> bool {
        self.state.borrow().finished
    }
}

impl crate::stream::WritableTarget for ServerResponseHandle {
    fn write_target_chunk(&mut self, chunk: Buffer) -> NodeResult<bool> {
        self.write_buffer(chunk)
    }

    fn drain_target(&mut self) -> NodeResult<()> {
        Ok(())
    }

    fn finish_target(&mut self) -> NodeResult<()> {
        self.end_empty()
    }
}

fn begin_runtime_response(
    state: &mut RuntimeResponseState,
    known_body_length: Option<usize>,
) -> NodeResult<()> {
    if state.headers_sent {
        return Ok(());
    }
    let status_code = runtime_response_status_code(state)?;
    let omit_body = state.omit_body || matches!(status_code, 204 | 304);
    let has_content_length = state.headers.contains_key("content-length");
    let has_explicit_transfer_encoding = match state.headers.get("transfer-encoding") {
        Some(value) if value.eq_ignore_ascii_case("chunked") => true,
        Some(_) => {
            return Err(NodeError::new(
                "ERR_HTTP_INVALID_HEADER_VALUE",
                "the only supported response transfer-encoding is chunked",
            ));
        }
        None => false,
    };
    if has_explicit_transfer_encoding && has_content_length {
        return Err(NodeError::new(
            "ERR_HTTP_CONTENT_LENGTH_MISMATCH",
            "content-length and transfer-encoding cannot be sent together",
        ));
    }
    state.chunked = !omit_body
        && (has_explicit_transfer_encoding
            || (known_body_length.is_none() && !has_content_length));

    let mut headers = format!(
        "HTTP/1.1 {status_code} {}\r\n",
        canonical_status_message(status_code)
    );
    for (name, value) in &state.headers {
        headers.push_str(name);
        headers.push_str(": ");
        headers.push_str(value);
        headers.push_str("\r\n");
    }
    if !has_content_length && !has_explicit_transfer_encoding && !omit_body {
        if let Some(length) = known_body_length {
            headers.push_str(&format!("Content-Length: {length}\r\n"));
        } else {
            headers.push_str("Transfer-Encoding: chunked\r\n");
        }
    }
    if !state.headers.contains_key("connection") {
        headers.push_str("Connection: close\r\n");
    }
    headers.push_str("\r\n");
    state
        .stream
        .as_mut()
        .ok_or_else(runtime_response_closed)?
        .write_all(headers.as_bytes())
        .map_err(runtime_http_io_error)?;
    state.headers_sent = true;
    Ok(())
}

fn write_runtime_body_chunk(
    state: &mut RuntimeResponseState,
    bytes: &[u8],
) -> NodeResult<()> {
    if bytes.is_empty() || runtime_response_omits_body(state)? {
        return Ok(());
    }
    if state.chunked {
        let stream = state.stream.as_mut().ok_or_else(runtime_response_closed)?;
        stream
            .write_all(format!("{:X}\r\n", bytes.len()).as_bytes())
            .and_then(|_| stream.write_all(bytes))
            .and_then(|_| stream.write_all(b"\r\n"))
            .and_then(|_| stream.flush())
            .map_err(runtime_http_io_error)
    } else {
        write_runtime_body_bytes(state, bytes)
    }
}

fn write_runtime_body_bytes(
    state: &mut RuntimeResponseState,
    bytes: &[u8],
) -> NodeResult<()> {
    if bytes.is_empty() || runtime_response_omits_body(state)? {
        return Ok(());
    }
    state
        .stream
        .as_mut()
        .ok_or_else(runtime_response_closed)?
        .write_all(bytes)
        .map_err(runtime_http_io_error)
}

fn runtime_response_omits_body(state: &RuntimeResponseState) -> NodeResult<bool> {
    Ok(state.omit_body || matches!(runtime_response_status_code(state)?, 204 | 304))
}

fn runtime_response_status_code(state: &RuntimeResponseState) -> NodeResult<u16> {
    u16::try_from(state.status_code)
        .ok()
        .filter(|code| (100..=999).contains(code))
        .ok_or_else(|| NodeError::new(
            "ERR_HTTP_INVALID_STATUS_CODE",
            "status code must be 100 through 999",
        ))
}

fn runtime_response_closed() -> NodeError {
    NodeError::new("ERR_STREAM_WRITE_AFTER_END", "response has already ended")
}

pub fn create_server_callable<E>(
    handler: tsonic_rust_runtime::Callable<RuntimeRequestArguments, Result<(), E>>,
) -> ServerHandle
where
    E: std::fmt::Display + 'static,
{
    create_runtime_server(adapt_runtime_callback(handler))
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
                Ok((stream, _)) => {
                    stream.set_nonblocking(false).map_err(runtime_http_io_error)?;
                    stream.set_read_timeout(Some(RUNTIME_IO_TIMEOUT)).map_err(runtime_http_io_error)?;
                    stream.set_write_timeout(Some(RUNTIME_IO_TIMEOUT)).map_err(runtime_http_io_error)?;
                    accepted.push(AcceptedConnection {
                    stream: Box::new(stream),
                    handler: server.handler.clone(),
                })},
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
    drop(ready);
    Ok(did_work)
}

fn handle_runtime_connection(
    mut accepted: AcceptedConnection,
) -> tsonic_rust_runtime::TsonicResult<Option<PendingResponse>> {
    use std::io::Write;

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
    let response = ServerResponseHandle::new(accepted.stream, omit_body);
    accepted.handler.call((request, response.clone()))?;
    Ok(Some(PendingResponse {
        response,
    }))
}

pub(crate) fn accept_runtime_transport(
    stream: Box<dyn RuntimeTransport>,
    handler: RuntimeRequestHandler,
) -> tsonic_rust_runtime::TsonicResult<()> {
    if let Some(pending) = handle_runtime_connection(AcceptedConnection { stream, handler })? {
        PENDING_RESPONSES.with(|responses| responses.borrow_mut().push(pending));
    }
    Ok(())
}

fn read_runtime_request(stream: &mut Box<dyn RuntimeTransport>) -> NodeResult<IncomingMessage> {
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

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn runtime_http_io_error(error: std::io::Error) -> NodeError {
    NodeError::new("ERR_HTTP_IO", error.to_string())
}
