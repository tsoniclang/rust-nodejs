struct DetachedHttpState {
    stream: Option<Box<dyn RuntimeTransport>>,
    response: Weak<RefCell<ServerResponseState>>,
    wire: ResponseWireState,
}

impl DetachedHttpState {
    fn start(&mut self, known_body_length: Option<usize>) -> NodeResult<()> {
        if self.wire.started {
            return Ok(());
        }
        let response = self
            .response
            .upgrade()
            .ok_or_else(runtime_response_closed)?;
        let mut response = response.borrow_mut();
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
        let omit_body = self.wire.omit_body || matches!(response.status_code, 204 | 304);
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
        let mut head = format!(
            "HTTP/1.1 {} {}\r\n",
            response.status_code, response.status_message
        );
        for (name, values) in response.headers.entries() {
            if name == "connection" {
                continue;
            }
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
        head.push_str("Connection: close\r\n\r\n");
        self.stream
            .as_mut()
            .ok_or_else(runtime_response_closed)?
            .write_all(head.as_bytes())
            .map_err(runtime_http_io_error)?;
        response.headers_sent = true;
        self.wire.started = true;
        self.wire.chunked = chunked;
        self.wire.omit_body = omit_body;
        self.wire.content_length = content_length.or(known_body_length);
        Ok(())
    }

    fn write(&mut self, chunk: Buffer) -> NodeResult<()> {
        self.start(None)?;
        self.wire.body_bytes = self
            .wire
            .body_bytes
            .checked_add(chunk.len())
            .ok_or_else(|| NodeError::new("ERR_HTTP_OUTPUT_OVERFLOW", "response size overflow"))?;
        if let Some(length) = self.wire.content_length {
            if self.wire.body_bytes > length {
                return Err(NodeError::new(
                    "ERR_HTTP_CONTENT_LENGTH_MISMATCH",
                    "response body exceeds content-length",
                ));
            }
        }
        if self.wire.omit_body || chunk.is_empty() {
            return Ok(());
        }
        let stream = self.stream.as_mut().ok_or_else(runtime_response_closed)?;
        if self.wire.chunked {
            stream
                .write_all(format!("{:X}\r\n", chunk.len()).as_bytes())
                .map_err(runtime_http_io_error)?;
        }
        chunk
            .with_bytes(|bytes| stream.write_all(bytes))
            .map_err(runtime_http_io_error)?;
        if self.wire.chunked {
            stream.write_all(b"\r\n").map_err(runtime_http_io_error)?;
        }
        Ok(())
    }

    fn finish(&mut self) -> NodeResult<()> {
        self.start(Some(0))?;
        if let Some(length) = self.wire.content_length {
            if !self.wire.omit_body && self.wire.body_bytes != length {
                return Err(NodeError::new(
                    "ERR_HTTP_CONTENT_LENGTH_MISMATCH",
                    "response body does not match content-length",
                ));
            }
        }
        let stream = self.stream.as_mut().ok_or_else(runtime_response_closed)?;
        if self.wire.chunked && !self.wire.omit_body {
            stream
                .write_all(b"0\r\n\r\n")
                .map_err(runtime_http_io_error)?;
        }
        stream.flush().map_err(runtime_http_io_error)?;
        self.wire.end_requested = true;
        self.stream.take();
        Ok(())
    }
}

struct DetachedHttpWritableBackend {
    state: RefCell<DetachedHttpState>,
}

impl DetachedHttpWritableBackend {
    fn new(
        stream: Box<dyn RuntimeTransport>,
        response: Weak<RefCell<ServerResponseState>>,
        omit_body: bool,
    ) -> Self {
        Self {
            state: RefCell::new(DetachedHttpState {
                stream: Some(stream),
                response,
                wire: ResponseWireState {
                    omit_body,
                    ..Default::default()
                },
            }),
        }
    }
}

impl crate::stream::WritableBackend for DetachedHttpWritableBackend {
    fn write(&self, chunk: Buffer) -> NodeResult<()> {
        self.state.borrow_mut().write(chunk)
    }

    fn flush(&self) -> NodeResult<()> {
        let mut state = self.state.borrow_mut();
        state.start(None)?;
        state
            .stream
            .as_mut()
            .ok_or_else(runtime_response_closed)?
            .flush()
            .map_err(runtime_http_io_error)
    }

    fn finish(&self) -> NodeResult<bool> {
        self.state.borrow_mut().finish()?;
        Ok(true)
    }

    fn destroy(&self) -> NodeResult<()> {
        self.state.borrow_mut().stream.take();
        Ok(())
    }

    fn buffered_bytes(&self) -> usize {
        0
    }
}

thread_local! {
    static PENDING_DETACHED_RESPONSES: RefCell<Vec<ServerResponse>> = const { RefCell::new(Vec::new()) };
}

fn retain_detached_response(response: ServerResponse) {
    if !response.writable_finished() && !response.destroyed() {
        PENDING_DETACHED_RESPONSES.with(|responses| responses.borrow_mut().push(response));
    }
}

fn has_pending_detached_responses() -> bool {
    PENDING_DETACHED_RESPONSES.with(|responses| !responses.borrow().is_empty())
}

fn poll_detached_responses() -> bool {
    PENDING_DETACHED_RESPONSES.with(|responses| {
        let mut responses = responses.borrow_mut();
        let before = responses.len();
        responses.retain(|response| !response.writable_finished() && !response.destroyed());
        responses.len() != before
    })
}

#[cfg(test)]
mod detached_response_tests {
    use super::{
        accept_runtime_transport, has_pending_detached_responses, poll_detached_responses,
        IncomingMessage, RuntimeTransport, ServerResponse,
    };
    use std::cell::RefCell;
    use std::io::{Cursor, Read, Write};
    use std::net::SocketAddr;
    use std::rc::Rc;
    use tsonic_rust_runtime::{Callable, TsonicError};

    struct MemoryTransport {
        input: Cursor<Vec<u8>>,
        output: Rc<RefCell<Vec<u8>>>,
    }

    impl Read for MemoryTransport {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            self.input.read(buffer)
        }
    }

    impl Write for MemoryTransport {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.output.borrow_mut().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl RuntimeTransport for MemoryTransport {
        fn peer_addr(&self) -> std::io::Result<SocketAddr> {
            Ok("127.0.0.1:12345".parse().unwrap())
        }
    }

    #[test]
    fn detached_response_can_finish_after_request_callback_returns() {
        let output = Rc::new(RefCell::new(Vec::new()));
        let retained = Rc::new(RefCell::new(None::<ServerResponse>));
        let retained_in_handler = Rc::clone(&retained);
        let transport = MemoryTransport {
            input: Cursor::new(b"GET /later HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec()),
            output: Rc::clone(&output),
        };
        accept_runtime_transport(
            Box::new(transport),
            Callable::new(
                move |(request, response): (IncomingMessage, ServerResponse)| {
                    assert_eq!(request.url(), Some("/later".to_string()));
                    *retained_in_handler.borrow_mut() = Some(response);
                    Ok::<(), TsonicError>(())
                },
            ),
        )
        .unwrap();

        assert!(has_pending_detached_responses());
        assert!(output.borrow().is_empty());
        let response = retained.borrow_mut().take().unwrap();
        response.write_string("a").unwrap();
        response.end_string("b").unwrap();
        assert!(poll_detached_responses());
        assert!(!has_pending_detached_responses());

        let wire = output.borrow();
        assert!(wire.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert!(wire
            .windows(b"Transfer-Encoding: chunked\r\n".len())
            .any(|window| window == b"Transfer-Encoding: chunked\r\n"));
        assert!(wire.ends_with(b"1\r\na\r\n1\r\nb\r\n0\r\n\r\n"));
    }
}
