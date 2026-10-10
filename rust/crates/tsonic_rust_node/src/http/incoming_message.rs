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

struct IncomingMessageMetadata {
    method: Option<String>,
    url: Option<String>,
    headers: IncomingHttpHeaders,
    http_version: String,
    status: Option<(u16, String)>,
}

struct IncomingMessageLifecycle<E: 'static> {
    aborted: bool,
    complete: bool,
    destroyed: bool,
    timeout: Option<u64>,
    aborted_event: crate::stream::StreamEvent<(), E>,
}

struct IncomingMessageState<E: 'static> {
    metadata: IncomingMessageMetadata,
    lifecycle: RefCell<IncomingMessageLifecycle<E>>,
}

pub struct IncomingMessage<E: 'static = NodeError> {
    state: Rc<IncomingMessageState<E>>,
    readable: Readable<E>,
    socket: Rc<net::Socket>,
}

impl<E: From<NodeError> + 'static> std::fmt::Debug for IncomingMessage<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.lifecycle.borrow();
        formatter
            .debug_struct("IncomingMessage")
            .field("method", &self.state.metadata.method)
            .field("url", &self.state.metadata.url)
            .field("http_version", &self.state.metadata.http_version)
            .field("aborted", &state.aborted)
            .field("complete", &state.complete)
            .finish()
    }
}

impl<E: From<NodeError> + 'static> IncomingMessage<E> {
    pub fn new(method: impl Into<String>, url: impl Into<String>, body: Vec<u8>) -> Self {
        let readable = if body.is_empty() {
            Readable::<E>::from_chunks(Vec::new())
        } else {
            Readable::<E>::from_chunks(vec![Buffer::from_bytes(body)])
        };
        Self::from_parts(
            IncomingMessageMetadata {
                method: Some(method.into()),
                url: Some(url.into()),
                http_version: "1.1".to_string(),
                headers: IncomingHttpHeaders::default(),
                status: None,
            },
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
        let readable = Readable::<E>::open(crate::stream::StreamOptions {
            high_water_mark,
            ..Default::default()
        });
        Ok(Self::from_parts(
            IncomingMessageMetadata {
                method: Some(method),
                url: Some(url),
                http_version,
                headers,
                status: None,
            },
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
            Readable::<E>::from_chunks(Vec::new())
        } else {
            Readable::<E>::from_chunks(vec![Buffer::from_bytes(body)])
        };
        Ok(Self::from_parts(
            IncomingMessageMetadata {
                method: None,
                url: Some(url),
                http_version,
                headers,
                status: Some((status_code, status_message)),
            },
            net::Socket::from_http_endpoints(None, None),
            readable,
            true,
        ))
    }

    fn from_parts(
        metadata: IncomingMessageMetadata,
        socket: net::Socket,
        readable: Readable<E>,
        complete: bool,
    ) -> Self {
        Self {
            state: Rc::new(IncomingMessageState {
                metadata,
                lifecycle: RefCell::new(IncomingMessageLifecycle::<E> {
                    aborted: false,
                    complete,
                    destroyed: false,
                    timeout: None,
                    aborted_event: crate::stream::StreamEvent::default(),
                }),
            }),
            readable,
            socket: Rc::new(socket),
        }
    }

    pub fn method(&self) -> Option<&str> {
        self.state.metadata.method.as_deref()
    }

    pub fn url(&self) -> Option<&str> {
        self.state.metadata.url.as_deref()
    }

    pub fn http_version(&self) -> &str {
        &self.state.metadata.http_version
    }

    pub fn headers(&self) -> &IncomingHttpHeaders {
        &self.state.metadata.headers
    }

    pub fn headers_distinct(&self) -> &IncomingHttpHeaders {
        self.headers()
    }

    pub fn complete(&self) -> bool {
        self.state.lifecycle.borrow().complete
    }

    pub fn aborted(&self) -> bool {
        self.state.lifecycle.borrow().aborted
    }

    pub fn destroyed(&self) -> bool {
        self.state.lifecycle.borrow().destroyed || self.readable.destroyed()
    }

    pub fn status_code(&self) -> Option<i32> {
        self.state.metadata.status.as_ref().map(|(code, _)| i32::from(*code))
    }

    pub fn status_message(&self) -> Option<&str> {
        self.state.metadata.status.as_ref().map(|(_, message)| message.as_str())
    }

    pub fn socket(&self) -> &net::Socket {
        self.socket.as_ref()
    }

    pub fn readable_handle(&self) -> Readable<E> {
        self.readable.clone()
    }

    pub fn read(&self) -> Result<Option<Buffer>, E> {
        self.readable.read()
    }

    pub fn read_buffer(&self, size: Option<i32>) -> Result<Option<Buffer>, E> {
        self.readable.read_buffer(size)
    }

    pub fn pause_chain(&self) -> Self {
        self.readable.pause();
        self.clone()
    }

    pub fn resume_chain(&self) -> Result<Self, E> {
        self.readable.resume()?;
        Ok(self.clone())
    }

    pub fn is_paused(&self) -> bool {
        self.readable.is_paused()
    }

    pub fn destroy_chain(
        &self,
        error: Option<tsonic_rust_runtime::RetainedError>,
    ) -> Result<Self, E> {
        self.abort(error)?;
        Ok(self.clone())
    }

    pub fn on_data(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> Result<Self, E> {
        self.readable.on_data(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_data(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> Result<Self, E> {
        self.readable.once_data(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_data(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> Result<Self, E> {
        self.readable.off_data(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_end(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.readable.on_end(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_end(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.readable.once_end(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_end(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.readable.off_end(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_error(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<
            (tsonic_rust_runtime::RetainedError,),
            Result<(), E>,
        >,
    ) -> Result<Self, E> {
        self.readable.on_error(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_error(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<
            (tsonic_rust_runtime::RetainedError,),
            Result<(), E>,
        >,
    ) -> Result<Self, E> {
        self.readable.once_error(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_error(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<
            (tsonic_rust_runtime::RetainedError,),
            Result<(), E>,
        >,
    ) -> Result<Self, E> {
        self.readable.off_error(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_close(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.readable.on_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_close(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.readable.once_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_close(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.readable.off_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn set_timeout(&self, milliseconds: u64) -> Self {
        self.state.lifecycle.borrow_mut().timeout = Some(milliseconds);
        self.clone()
    }

    pub fn timeout(&self) -> Option<u64> {
        self.state.lifecycle.borrow().timeout
    }

    pub fn on_aborted(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.add_aborted_listener(event, listener, false)
    }

    pub fn once_aborted(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.add_aborted_listener(event, listener, true)
    }

    pub fn off_aborted(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        ensure_http_event(event, "aborted")?;
        self.state
            .lifecycle
            .borrow_mut()
            .aborted_event
            .remove(listener.identity_key());
        Ok(self.clone())
    }

    fn add_aborted_listener(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
        once: bool,
    ) -> Result<Self, E> {
        ensure_http_event(event, "aborted")?;
        self.state
            .lifecycle
            .borrow_mut()
            .aborted_event
            .add(crate::stream::empty_listener(listener, once));
        Ok(self.clone())
    }

    pub(crate) fn push_body(&self, chunk: Buffer) -> Result<bool, E> {
        self.readable.enqueue(chunk)
    }

    pub(crate) fn finish_body(&self) -> Result<(), E> {
        self.state.lifecycle.borrow_mut().complete = true;
        self.readable.finish_input()
    }

    pub(crate) fn body_pressured(&self) -> bool {
        self.readable.pressured()
    }

    pub(crate) fn set_capacity_handler(&self, callback: impl Fn() -> Result<(), E> + 'static) {
        self.readable.set_capacity_handler(callback);
    }

    pub(crate) fn abort(&self, error: Option<tsonic_rust_runtime::RetainedError>) -> Result<(), E> {
        let callbacks = {
            let mut state = self.state.lifecycle.borrow_mut();
            let previously_destroyed = state.destroyed;
            state.destroyed = true;
            state.aborted = !state.complete;
            if state.aborted && !previously_destroyed {
                state.aborted_event.emission()
            } else {
                crate::stream::StreamEmission::default()
            }
        };
        self.socket.mark_destroyed();
        self.readable
            .destroy_before_terminal(error, || crate::stream::invoke_event(callbacks, ()))
    }
}

pub fn incoming_message_as_readable<E: From<NodeError> + 'static>(
    value: &IncomingMessage<E>,
) -> Readable<E> {
    value.readable_handle()
}

pub fn incoming_message_as_stream<E: From<NodeError> + 'static>(
    value: &IncomingMessage<E>,
) -> crate::stream::Stream<E> {
    crate::stream::readable_as_stream(&value.readable_handle())
}

impl<E: 'static> Clone for IncomingMessage<E> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
            readable: self.readable.clone(),
            socket: Rc::clone(&self.socket),
        }
    }
}
