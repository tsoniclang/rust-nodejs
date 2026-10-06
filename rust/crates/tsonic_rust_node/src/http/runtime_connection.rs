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

struct RuntimeConnection<E: 'static> {
    reservation: tsonic_rust_runtime::dispatch_queue::TaskReservation,
    io: RuntimeConnectionIo,
    server: Weak<RefCell<RuntimeServerState<E>>>,
    handler: RuntimeRequestHandler<E>,
    input: Vec<u8>,
    input_start: usize,
    output: VecDeque<OutputChunk>,
    output_bytes: usize,
    request: Option<IncomingMessage<E>>,
    response: Option<ServerResponse<E>>,
    body: RequestBody,
    body_complete: bool,
    body_paused: bool,
    pending: Option<(
        tsonic_rust_runtime::dispatch_queue::TaskTicket,
        ConnectionAction<E>,
    )>,
    keep_alive: bool,
    close_after_response: bool,
    peer_eof: bool,
    closed: bool,
    failure: Option<NodeError>,
    wire: ResponseWireState,
}

impl<E: From<NodeError> + 'static> RuntimeConnection<E> {
    fn new(
        io: RuntimeConnectionIo,
        server: &Rc<RefCell<RuntimeServerState<E>>>,
        handler: RuntimeRequestHandler<E>,
        reservation: tsonic_rust_runtime::dispatch_queue::TaskReservation,
    ) -> Self {
        Self {
            reservation,
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
            pending: None,
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
            NodeError::new(
                "ERR_HTTP_OUTPUT_OVERFLOW",
                "pending response bytes overflow",
            )
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

struct HttpWritableBackend<E: 'static> {
    connection: Weak<RefCell<RuntimeConnection<E>>>,
    response: Weak<RefCell<ServerResponseState>>,
}

impl<E: From<NodeError> + 'static> crate::stream::WritableBackend<E> for HttpWritableBackend<E> {
    fn bind(&self, _owner: crate::stream::WeakWritable<E>) {}

    fn write(&self, chunk: Buffer) -> crate::stream::StreamBackendResult<(), E> {
        let connection = self
            .connection
            .upgrade()
            .ok_or_else(runtime_response_closed)?;
        let response = self
            .response
            .upgrade()
            .ok_or_else(runtime_response_closed)?;
        let mut connection = connection.borrow_mut();
        begin_response(&mut connection, &mut response.borrow_mut(), None)?;
        queue_body_chunk(&mut connection, chunk)?;
        connection.refresh_interest()?;
        Ok(())
    }

    fn flush(&self) -> crate::stream::StreamBackendResult<(), E> {
        let connection = self
            .connection
            .upgrade()
            .ok_or_else(runtime_response_closed)?;
        let response = self
            .response
            .upgrade()
            .ok_or_else(runtime_response_closed)?;
        let mut connection = connection.borrow_mut();
        begin_response(&mut connection, &mut response.borrow_mut(), None)?;
        connection.refresh_interest()?;
        Ok(())
    }

    fn finish(&self) -> crate::stream::StreamBackendResult<bool, E> {
        let connection = self
            .connection
            .upgrade()
            .ok_or_else(runtime_response_closed)?;
        let response = self
            .response
            .upgrade()
            .ok_or_else(runtime_response_closed)?;
        let mut connection = connection.borrow_mut();
        begin_response(&mut connection, &mut response.borrow_mut(), Some(0))?;
        finish_response_wire(&mut connection)?;
        connection.refresh_interest()?;
        Ok(connection.output.is_empty())
    }

    fn destroy(&self) -> crate::stream::StreamBackendResult<(), E> {
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

enum ConnectionAction<E: 'static> {
    FramingProgress,
    Release,
    DestroyResponse {
        response: ServerResponse<E>,
        error: NodeError,
    },
    Dispatch {
        handler: RuntimeRequestHandler<E>,
        request: IncomingMessage<E>,
        response: ServerResponse<E>,
    },
    BodyChunk {
        request: IncomingMessage<E>,
        chunk: Buffer,
    },
    BodyEnd(IncomingMessage<E>),
    AbortRequest {
        request: IncomingMessage<E>,
        error: NodeError,
    },
    WritableProgress(Writable<E>),
    WritableComplete(Writable<E>),
}
