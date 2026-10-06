pub(crate) struct ServerResponseState {
    pub(crate) status_code: i32,
    pub(crate) status_message: String,
    pub(crate) headers: HeaderStore,
    pub(crate) headers_sent: bool,
    pub(crate) retained_reservation: Option<tsonic_rust_runtime::dispatch_queue::TaskReservation>,
}

pub struct ServerResponse<E: 'static = NodeError> {
    state: Rc<RefCell<ServerResponseState>>,
    writable: Writable<E>,
}

impl<E: From<NodeError> + 'static> std::fmt::Debug for ServerResponse<E> {
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

impl<E: From<NodeError> + 'static> Default for ServerResponse<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: From<NodeError> + 'static> ServerResponse<E> {
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(default_response_state())),
            writable: Writable::<E>::new(),
        }
    }

    pub(crate) fn streaming(
        high_water_mark: usize,
        backend: impl FnOnce(
            std::rc::Weak<RefCell<ServerResponseState>>,
        ) -> Rc<dyn crate::stream::WritableBackend<E>>,
    ) -> Self {
        let state = Rc::new(RefCell::new(default_response_state()));
        let writable = Writable::<E>::with_backend(
            crate::stream::StreamOptions {
                high_water_mark,
                ..Default::default()
            },
            backend(Rc::downgrade(&state)),
            None,
        );
        Self { state, writable }
    }

    pub fn status_code(&self) -> i32 {
        self.state.borrow().status_code
    }

    pub fn set_status_code(&self, value: i32) -> Result<(), E> {
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

    pub fn set_status_message(&self, value: &str) -> Result<(), E> {
        if value.bytes().any(|byte| matches!(byte, 0..=31 | 127)) {
            return Err(NodeError::new(
                "ERR_INVALID_CHAR",
                "status message contains invalid characters",
            )
            .into());
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

    pub fn set_header(&self, name: &str, value: &str) -> Result<Self, E> {
        let mut state = self.state.borrow_mut();
        ensure_headers_mutable(&state)?;
        state.headers.set(name, [value.to_string()])?;
        Ok(self.clone())
    }

    pub fn set_header_values(
        &self,
        name: &str,
        values: &tsonic_rust_js::JsArray<String>,
    ) -> Result<Self, E> {
        let mut state = self.state.borrow_mut();
        ensure_headers_mutable(&state)?;
        state.headers.set(name, values.values())?;
        Ok(self.clone())
    }

    pub fn append_header(&self, name: &str, value: &str) -> Result<Self, E> {
        let mut state = self.state.borrow_mut();
        ensure_headers_mutable(&state)?;
        state.headers.append(name, [value.to_string()])?;
        Ok(self.clone())
    }

    pub fn append_header_values(
        &self,
        name: &str,
        values: &tsonic_rust_js::JsArray<String>,
    ) -> Result<Self, E> {
        let mut state = self.state.borrow_mut();
        ensure_headers_mutable(&state)?;
        state.headers.append(name, values.values())?;
        Ok(self.clone())
    }

    pub fn get_header(&self, name: &str) -> Result<Option<String>, E> {
        self.state.borrow().headers.get(name).map_err(E::from)
    }

    pub fn get_header_values(&self, name: &str) -> Result<tsonic_rust_js::JsArray<String>, E> {
        self.state
            .borrow()
            .headers
            .get_all(name)
            .map(tsonic_rust_js::JsArray::from_dense)
            .map_err(E::from)
    }

    pub fn get_header_names(&self) -> tsonic_rust_js::JsArray<String> {
        tsonic_rust_js::JsArray::from_dense(self.state.borrow().headers.names.clone())
    }

    pub fn get_headers(&self) -> OutgoingHttpHeaders {
        OutgoingHttpHeaders::snapshot(&self.state.borrow().headers)
    }

    pub fn has_header(&self, name: &str) -> Result<bool, E> {
        self.state.borrow().headers.contains(name).map_err(E::from)
    }

    pub fn remove_header(&self, name: &str) -> Result<(), E> {
        let mut state = self.state.borrow_mut();
        ensure_headers_mutable(&state)?;
        state.headers.remove(name).map_err(E::from)
    }

    pub fn flush_headers(&self) -> Result<(), E> {
        self.writable.flush_backend()
    }

    pub fn write_head(&self, status_code: i32) -> Result<Self, E> {
        self.set_status_code(status_code)?;
        self.flush_headers()?;
        Ok(self.clone())
    }

    pub fn write_head_headers(
        &self,
        status_code: i32,
        headers: &OutgoingHttpHeaders,
    ) -> Result<Self, E> {
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
    ) -> Result<Self, E> {
        self.set_status_code(status_code)?;
        self.set_status_message(status_message)?;
        if let Some(headers) = headers {
            self.replace_headers(headers)?;
        }
        self.flush_headers()?;
        Ok(self.clone())
    }

    fn replace_headers(&self, headers: &OutgoingHttpHeaders) -> Result<(), E> {
        let mut state = self.state.borrow_mut();
        ensure_headers_mutable(&state)?;
        state.headers = headers.store.clone();
        Ok(())
    }

    pub fn write_buffer(&self, chunk: &Buffer) -> Result<bool, E> {
        self.writable.write_buffer(chunk)
    }

    pub fn write_string(&self, chunk: &str) -> Result<bool, E> {
        self.writable.write_string(chunk)
    }

    pub fn end_empty(&self) -> Result<Self, E> {
        self.writable.end()?;
        Ok(self.clone())
    }

    pub fn end_string(&self, chunk: &str) -> Result<Self, E> {
        let buffer = Buffer::from_string(chunk, Some("utf8"))?;
        self.end_buffer(&buffer)
    }

    pub fn end_buffer(&self, chunk: &Buffer) -> Result<Self, E> {
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
        self.writable.end()?;
        Ok(self.clone())
    }

    pub fn destroy_chain(
        &self,
        error: Option<tsonic_rust_runtime::RetainedError>,
    ) -> Result<Self, E> {
        self.writable.destroy_chain(error)?;
        Ok(self.clone())
    }

    pub fn on_drain(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.on_drain(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_drain(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.once_drain(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_drain(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.off_drain(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_finish(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.on_finish(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_finish(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.once_finish(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_finish(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.off_finish(event, listener)?;
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
        self.writable.on_error(event, listener)?;
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
        self.writable.once_error(event, listener)?;
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
        self.writable.off_error(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_close(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.on_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_close(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.once_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_close(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.off_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn writable_handle(&self) -> Writable<E> {
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
            body: Buffer::concat_dense(&self.writable.chunks())
                .as_bytes()
                .to_vec(),
        }
    }
}

impl<E: From<NodeError> + 'static> crate::stream::WritableTarget<E> for ServerResponse<E> {
    fn writable_handle(&self) -> Writable<E> {
        self.writable.clone()
    }
}

pub fn server_response_as_writable<E: From<NodeError> + 'static>(
    value: &ServerResponse<E>,
) -> Writable<E> {
    value.writable_handle()
}

pub fn server_response_as_stream<E: From<NodeError> + 'static>(
    value: &ServerResponse<E>,
) -> crate::stream::Stream<E> {
    crate::stream::writable_as_stream(&value.writable_handle())
}

fn default_response_state() -> ServerResponseState {
    ServerResponseState {
        status_code: 200,
        status_message: "OK".to_string(),
        headers: HeaderStore::default(),
        headers_sent: false,
        retained_reservation: None,
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

pub type OutgoingMessage<E = NodeError> = ServerResponse<E>;

impl<E: 'static> Clone for ServerResponse<E> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
            writable: self.writable.clone(),
        }
    }
}
