use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status_code: u16,
    pub status_message: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn text(&self) -> NodeResult<String> {
        String::from_utf8(self.body.clone())
            .map_err(|error| NodeError::new("ERR_INVALID_ARG_VALUE", error.to_string()))
    }
}

struct IncomingMessageState {
    method: Option<String>,
    url: Option<String>,
    headers: IncomingHttpHeaders,
    http_version: String,
    aborted: bool,
    complete: bool,
    status_code: Option<u16>,
    status_message: Option<String>,
    destroyed: bool,
    timeout: Option<u64>,
    aborted_event: crate::stream::StreamEvent<()>,
}

#[derive(Clone)]
pub struct IncomingMessage {
    state: Rc<RefCell<IncomingMessageState>>,
    readable: Readable,
    socket: Rc<net::Socket>,
}

impl std::fmt::Debug for IncomingMessage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.borrow();
        formatter
            .debug_struct("IncomingMessage")
            .field("method", &state.method)
            .field("url", &state.url)
            .field("http_version", &state.http_version)
            .field("aborted", &state.aborted)
            .field("complete", &state.complete)
            .finish()
    }
}

impl IncomingMessage {
    pub fn new(method: impl Into<String>, url: impl Into<String>, body: Vec<u8>) -> Self {
        let readable = if body.is_empty() {
            Readable::from_chunks(Vec::new())
        } else {
            Readable::from_chunks(vec![Buffer::from_bytes(body)])
        };
        Self::from_parts(
            Some(method.into()),
            Some(url.into()),
            "1.1".to_string(),
            IncomingHttpHeaders::default(),
            net::Socket::from_http_endpoints(None, None),
            readable,
            true,
        )
    }

    pub(crate) fn streaming(
        method: String,
        url: String,
        http_version: String,
        header_pairs: Vec<(String, String)>,
        local_address: Option<std::net::SocketAddr>,
        remote_address: Option<std::net::SocketAddr>,
        high_water_mark: usize,
    ) -> NodeResult<Self> {
        let headers = IncomingHttpHeaders::from_pairs(header_pairs)?;
        let readable = Readable::open(crate::stream::StreamOptions {
            high_water_mark,
            ..Default::default()
        });
        Ok(Self::from_parts(
            Some(method),
            Some(url),
            http_version,
            headers,
            net::Socket::from_http_endpoints(local_address, remote_address),
            readable,
            false,
        ))
    }

    pub(crate) fn from_client_response(
        url: String,
        status_code: u16,
        status_message: String,
        http_version: String,
        header_pairs: Vec<(String, String)>,
        body: Vec<u8>,
    ) -> NodeResult<Self> {
        let headers = IncomingHttpHeaders::from_pairs(header_pairs)?;
        let readable = if body.is_empty() {
            Readable::from_chunks(Vec::new())
        } else {
            Readable::from_chunks(vec![Buffer::from_bytes(body)])
        };
        let message = Self::from_parts(
            None,
            Some(url),
            http_version,
            headers,
            net::Socket::from_http_endpoints(None, None),
            readable,
            true,
        );
        {
            let mut state = message.state.borrow_mut();
            state.status_code = Some(status_code);
            state.status_message = Some(status_message);
        }
        Ok(message)
    }

    fn from_parts(
        method: Option<String>,
        url: Option<String>,
        http_version: String,
        headers: IncomingHttpHeaders,
        socket: net::Socket,
        readable: Readable,
        complete: bool,
    ) -> Self {
        Self {
            state: Rc::new(RefCell::new(IncomingMessageState {
                method,
                url,
                headers,
                http_version,
                aborted: false,
                complete,
                status_code: None,
                status_message: None,
                destroyed: false,
                timeout: None,
                aborted_event: crate::stream::StreamEvent::default(),
            })),
            readable,
            socket: Rc::new(socket),
        }
    }

    pub fn method(&self) -> Option<String> {
        self.state.borrow().method.clone()
    }

    pub fn url(&self) -> Option<String> {
        self.state.borrow().url.clone()
    }

    pub fn http_version(&self) -> String {
        self.state.borrow().http_version.clone()
    }

    pub fn headers(&self) -> IncomingHttpHeaders {
        self.state.borrow().headers.clone()
    }

    pub fn headers_distinct(&self) -> IncomingHttpHeaders {
        self.headers()
    }

    pub fn complete(&self) -> bool {
        self.state.borrow().complete
    }

    pub fn aborted(&self) -> bool {
        self.state.borrow().aborted
    }

    pub fn destroyed(&self) -> bool {
        self.state.borrow().destroyed || self.readable.destroyed()
    }

    pub fn status_code(&self) -> Option<i32> {
        self.state.borrow().status_code.map(i32::from)
    }

    pub fn status_message(&self) -> Option<String> {
        self.state.borrow().status_message.clone()
    }

    pub fn socket(&self) -> &net::Socket {
        self.socket.as_ref()
    }

    pub fn readable_handle(&self) -> Readable {
        self.readable.clone()
    }

    pub fn read(&self) -> Option<Buffer> {
        self.readable.read()
    }

    pub fn read_buffer(&self, size: Option<i32>) -> NodeResult<Option<Buffer>> {
        self.readable.read_buffer(size)
    }

    pub fn pause_chain(&self) -> Self {
        self.readable.pause();
        self.clone()
    }

    pub fn resume_chain(&self) -> Self {
        self.readable.resume();
        self.clone()
    }

    pub fn is_paused(&self) -> bool {
        self.readable.is_paused()
    }

    pub fn destroy_chain(&self, error: Option<NodeError>) -> NodeResult<Self> {
        self.abort(error)?;
        Ok(self.clone())
    }

    pub fn on_data<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.on_data(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_data<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.once_data(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_data<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.off_data(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_end<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.on_end(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_end<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.once_end(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_end<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.off_end(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_error<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(NodeError,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.on_error(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_error<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(NodeError,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.once_error(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_error<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(NodeError,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.off_error(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_close<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.on_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_close<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.once_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_close<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.off_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn set_timeout(&self, milliseconds: u64) -> Self {
        self.state.borrow_mut().timeout = Some(milliseconds);
        self.clone()
    }

    pub fn timeout(&self) -> Option<u64> {
        self.state.borrow().timeout
    }

    pub fn on_aborted<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.add_aborted_listener(event, listener, false)
    }

    pub fn once_aborted<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.add_aborted_listener(event, listener, true)
    }

    pub fn off_aborted<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        ensure_http_event(event, "aborted")?;
        self.state
            .borrow_mut()
            .aborted_event
            .remove(listener.identity_key());
        Ok(self.clone())
    }

    fn add_aborted_listener<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
        once: bool,
    ) -> NodeResult<Self> {
        ensure_http_event(event, "aborted")?;
        self.state
            .borrow_mut()
            .aborted_event
            .add(crate::stream::empty_listener(listener, once));
        Ok(self.clone())
    }

    pub(crate) fn push_body(&self, chunk: Buffer) -> NodeResult<bool> {
        self.readable.enqueue(chunk)
    }

    pub(crate) fn finish_body(&self) -> NodeResult<()> {
        self.state.borrow_mut().complete = true;
        self.readable.finish_input()
    }

    pub(crate) fn body_pressured(&self) -> bool {
        self.readable.pressured()
    }

    pub(crate) fn set_capacity_handler(
        &self,
        callback: impl Fn() -> NodeResult<()> + 'static,
    ) {
        self.readable.set_capacity_handler(callback);
    }

    pub(crate) fn abort(&self, error: Option<NodeError>) -> NodeResult<()> {
        let callbacks = {
            let mut state = self.state.borrow_mut();
            if state.destroyed {
                return Ok(());
            }
            state.destroyed = true;
            state.aborted = !state.complete;
            if state.aborted {
                state.aborted_event.emission()
            } else {
                Vec::new()
            }
        };
        self.socket.mark_destroyed();
        crate::stream::invoke_event(callbacks, ())?;
        self.readable.destroy_chain(error)?;
        Ok(())
    }
}

pub fn incoming_message_as_readable(value: &IncomingMessage) -> Readable {
    value.readable_handle()
}

pub fn incoming_message_as_stream(value: &IncomingMessage) -> crate::stream::Stream {
    crate::stream::readable_as_stream(&value.readable_handle())
}

pub(crate) struct ServerResponseState {
    pub(crate) status_code: i32,
    pub(crate) status_message: String,
    pub(crate) headers: HeaderStore,
    pub(crate) headers_sent: bool,
}

#[derive(Clone)]
pub struct ServerResponse {
    state: Rc<RefCell<ServerResponseState>>,
    writable: Writable,
}

impl std::fmt::Debug for ServerResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.borrow();
        formatter
            .debug_struct("ServerResponse")
            .field("status_code", &state.status_code)
            .field("headers_sent", &state.headers_sent)
            .field("writable_ended", &self.writable.writable_ended())
            .field("writable_finished", &self.writable.writable_finished())
            .finish()
    }
}

impl Default for ServerResponse {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerResponse {
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(default_response_state())),
            writable: Writable::new(),
        }
    }

    pub(crate) fn streaming(
        high_water_mark: usize,
        backend: impl FnOnce(
            std::rc::Weak<RefCell<ServerResponseState>>,
        ) -> Rc<dyn crate::stream::WritableBackend>,
    ) -> Self {
        let state = Rc::new(RefCell::new(default_response_state()));
        let writable = Writable::with_backend(
            crate::stream::StreamOptions {
                high_water_mark,
                ..Default::default()
            },
            backend(Rc::downgrade(&state)),
        );
        Self { state, writable }
    }

    pub fn status_code(&self) -> i32 {
        self.state.borrow().status_code
    }

    pub fn set_status_code(&self, value: i32) -> NodeResult<()> {
        ensure_status_code(value)?;
        let mut state = self.state.borrow_mut();
        ensure_headers_mutable(&state)?;
        state.status_code = value;
        state.status_message = canonical_status_message(value as u16).to_string();
        Ok(())
    }

    pub fn status_message(&self) -> String {
        self.state.borrow().status_message.clone()
    }

    pub fn set_status_message(&self, value: &str) -> NodeResult<()> {
        if value.bytes().any(|byte| matches!(byte, 0..=31 | 127)) {
            return Err(NodeError::new(
                "ERR_INVALID_CHAR",
                "status message contains invalid characters",
            ));
        }
        let mut state = self.state.borrow_mut();
        ensure_headers_mutable(&state)?;
        state.status_message = value.to_string();
        Ok(())
    }

    pub fn headers_sent(&self) -> bool {
        self.state.borrow().headers_sent
    }

    pub fn writable_ended(&self) -> bool {
        self.writable.writable_ended()
    }

    pub fn writable_finished(&self) -> bool {
        self.writable.writable_finished()
    }

    pub fn writable_need_drain(&self) -> bool {
        self.writable.writable_need_drain()
    }

    pub fn destroyed(&self) -> bool {
        self.writable.destroyed()
    }

    pub fn set_header(&self, name: &str, value: &str) -> NodeResult<Self> {
        let mut state = self.state.borrow_mut();
        ensure_headers_mutable(&state)?;
        state.headers.set(name, [value.to_string()])?;
        Ok(self.clone())
    }

    pub fn set_header_values(
        &self,
        name: &str,
        values: &tsonic_rust_js::JsArray<String>,
    ) -> NodeResult<Self> {
        let mut state = self.state.borrow_mut();
        ensure_headers_mutable(&state)?;
        state.headers.set(name, values.values())?;
        Ok(self.clone())
    }

    pub fn append_header(&self, name: &str, value: &str) -> NodeResult<Self> {
        let mut state = self.state.borrow_mut();
        ensure_headers_mutable(&state)?;
        state.headers.append(name, [value.to_string()])?;
        Ok(self.clone())
    }

    pub fn append_header_values(
        &self,
        name: &str,
        values: &tsonic_rust_js::JsArray<String>,
    ) -> NodeResult<Self> {
        let mut state = self.state.borrow_mut();
        ensure_headers_mutable(&state)?;
        state.headers.append(name, values.values())?;
        Ok(self.clone())
    }

    pub fn get_header(&self, name: &str) -> NodeResult<Option<String>> {
        self.state.borrow().headers.get(name)
    }

    pub fn get_header_values(
        &self,
        name: &str,
    ) -> NodeResult<tsonic_rust_js::JsArray<String>> {
        self.state
            .borrow()
            .headers
            .get_all(name)
            .map(tsonic_rust_js::JsArray::from_dense)
    }

    pub fn get_header_names(&self) -> tsonic_rust_js::JsArray<String> {
        tsonic_rust_js::JsArray::from_dense(self.state.borrow().headers.names.clone())
    }

    pub fn get_headers(&self) -> OutgoingHttpHeaders {
        OutgoingHttpHeaders::snapshot(&self.state.borrow().headers)
    }

    pub fn has_header(&self, name: &str) -> NodeResult<bool> {
        self.state.borrow().headers.contains(name)
    }

    pub fn remove_header(&self, name: &str) -> NodeResult<()> {
        let mut state = self.state.borrow_mut();
        ensure_headers_mutable(&state)?;
        state.headers.remove(name)
    }

    pub fn flush_headers(&self) -> NodeResult<()> {
        self.writable.flush_backend()
    }

    pub fn write_head(&self, status_code: i32) -> NodeResult<Self> {
        self.set_status_code(status_code)?;
        self.flush_headers()?;
        Ok(self.clone())
    }

    pub fn write_head_headers(
        &self,
        status_code: i32,
        headers: &OutgoingHttpHeaders,
    ) -> NodeResult<Self> {
        self.set_status_code(status_code)?;
        self.replace_headers(headers)?;
        self.flush_headers()?;
        Ok(self.clone())
    }

    pub fn write_head_message(
        &self,
        status_code: i32,
        status_message: &str,
        headers: &Option<OutgoingHttpHeaders>,
    ) -> NodeResult<Self> {
        self.set_status_code(status_code)?;
        self.set_status_message(status_message)?;
        if let Some(headers) = headers {
            self.replace_headers(headers)?;
        }
        self.flush_headers()?;
        Ok(self.clone())
    }

    fn replace_headers(&self, headers: &OutgoingHttpHeaders) -> NodeResult<()> {
        let mut state = self.state.borrow_mut();
        ensure_headers_mutable(&state)?;
        state.headers = headers.store.clone();
        Ok(())
    }

    pub fn write_buffer(&self, chunk: &Buffer) -> NodeResult<bool> {
        self.writable.write_buffer(chunk)
    }

    pub fn write_string(&self, chunk: &str) -> NodeResult<bool> {
        self.writable.write_string(chunk)
    }

    pub fn end_empty(&self) -> NodeResult<Self> {
        self.writable.end_checked()?;
        Ok(self.clone())
    }

    pub fn end_string(&self, chunk: &str) -> NodeResult<Self> {
        let buffer = Buffer::from_string(chunk, Some("utf8"))?;
        self.end_buffer(&buffer)
    }

    pub fn end_buffer(&self, chunk: &Buffer) -> NodeResult<Self> {
        let state = self.state.borrow();
        let add_length = !state.headers_sent
            && !matches!(state.status_code, 204 | 304)
            && !state.headers.contains("content-length")?
            && !state.headers.contains("transfer-encoding")?;
        drop(state);
        if add_length {
            self.set_header("content-length", &chunk.len().to_string())?;
        }
        self.writable.write_buffer(chunk)?;
        self.writable.end_checked()?;
        Ok(self.clone())
    }

    pub fn destroy_chain(&self, error: Option<NodeError>) -> NodeResult<Self> {
        self.writable.destroy_chain(error)?;
        Ok(self.clone())
    }

    pub fn on_drain<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.on_drain(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_drain<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.once_drain(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_drain<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.off_drain(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_finish<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.on_finish(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_finish<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.once_finish(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_finish<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.off_finish(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_error<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(NodeError,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.on_error(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_error<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(NodeError,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.once_error(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_error<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(NodeError,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.off_error(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_close<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.on_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_close<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.once_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_close<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.off_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn writable_handle(&self) -> Writable {
        self.writable.clone()
    }

    pub fn to_response(&self) -> Response {
        let state = self.state.borrow();
        let headers = state
            .headers
            .entries()
            .into_iter()
            .filter_map(|(name, values)| values.first().cloned().map(|value| (name, value)))
            .collect();
        Response {
            status_code: state.status_code as u16,
            status_message: state.status_message.clone(),
            headers,
            body: Buffer::concat_dense(&self.writable.chunks()).as_bytes().to_vec(),
        }
    }

}

impl crate::stream::WritableTarget for ServerResponse {
    fn writable_handle(&self) -> Writable {
        self.writable.clone()
    }
}

pub fn server_response_as_writable(value: &ServerResponse) -> Writable {
    value.writable_handle()
}

pub fn server_response_as_stream(value: &ServerResponse) -> crate::stream::Stream {
    crate::stream::writable_as_stream(&value.writable_handle())
}

fn default_response_state() -> ServerResponseState {
    ServerResponseState {
        status_code: 200,
        status_message: "OK".to_string(),
        headers: HeaderStore::default(),
        headers_sent: false,
    }
}

fn ensure_status_code(value: i32) -> NodeResult<()> {
    if (100..=999).contains(&value) {
        Ok(())
    } else {
        Err(NodeError::new(
            "ERR_HTTP_INVALID_STATUS_CODE",
            "status code must be 100 through 999",
        ))
    }
}

fn ensure_headers_mutable(state: &ServerResponseState) -> NodeResult<()> {
    if state.headers_sent {
        Err(NodeError::new(
            "ERR_HTTP_HEADERS_SENT",
            "response headers have already been sent",
        ))
    } else {
        Ok(())
    }
}

fn ensure_http_event(actual: &str, expected: &str) -> NodeResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(NodeError::new(
            "ERR_INVALID_ARG_VALUE",
            format!("expected HTTP event {expected}, received {actual}"),
        ))
    }
}

pub type OutgoingMessage = ServerResponse;

#[cfg(test)]
mod http_message_events_tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use tsonic_rust_runtime::{Callable, TsonicError};

    use super::IncomingMessage;

    #[test]
    fn aborted_listener_retention_removal_and_single_fire() {
        let message = IncomingMessage::streaming(
            "POST".to_string(),
            "/upload".to_string(),
            "1.1".to_string(),
            Vec::new(),
            None,
            None,
            1024,
        )
        .unwrap();
        let fired = Rc::new(Cell::new(0));
        let removed_fired = Rc::clone(&fired);
        let removed = Callable::new(move |()| {
            removed_fired.set(removed_fired.get() + 1);
            Ok::<(), TsonicError>(())
        });
        message.on_aborted("aborted", &removed).unwrap();
        message.off_aborted("aborted", &removed).unwrap();

        let once_fired = Rc::clone(&fired);
        let once = Callable::new(move |()| {
            once_fired.set(once_fired.get() + 1);
            Ok::<(), TsonicError>(())
        });
        message.once_aborted("aborted", &once).unwrap();
        message.destroy_chain(None).unwrap();
        message.destroy_chain(None).unwrap();
        assert!(message.aborted());
        assert_eq!(fired.get(), 1);
    }
}
