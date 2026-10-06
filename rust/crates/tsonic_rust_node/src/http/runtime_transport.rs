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
    socket
        .set_reuse_address(true)
        .map_err(runtime_http_io_error)?;
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
    NodeError::new(
        "ERR_STREAM_WRITE_AFTER_END",
        "response is no longer writable",
    )
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

pub(crate) fn accept_runtime_transport<E: From<NodeError> + 'static>(
    resources: &HttpHandle<E>,
    mut stream: Box<dyn RuntimeTransport>,
    handler: RuntimeRequestHandler<E>,
) -> Result<(), E> {
    let mut input = Vec::new();
    let parsed = loop {
        if let Some(parsed) = parse_request_head(&input).map_err(E::from)? {
            break parsed;
        }
        let start = input.len();
        input.resize(start + 16 * 1024, 0);
        let read = stream
            .read(&mut input[start..])
            .map_err(runtime_http_io_error)
            .map_err(E::from)?;
        input.truncate(start + read);
        if read == 0 {
            return Err(E::from(NodeError::new(
                "HPE_INVALID_EOF_STATE",
                "request ended before headers completed",
            )));
        }
    };
    let (consumed, head) = parsed;
    let body = read_blocking_body(&mut stream, &input[consumed..], head.body).map_err(E::from)?;
    let remote = stream.peer_addr().ok();
    let request = IncomingMessage::<E>::streaming(
        head.method.clone(),
        head.target,
        head.version,
        head.headers,
        None,
        remote,
        RUNTIME_READ_HIGH_WATER_MARK,
    )
    .map_err(E::from)?;
    if !body.is_empty() {
        request.push_body(Buffer::from_bytes(body))?;
    }
    request.finish_body()?;
    let response = ServerResponse::streaming(RUNTIME_WRITE_HIGH_WATER_MARK, move |state| {
        Rc::new(DetachedHttpWritableBackend::new(
            stream,
            state,
            head.method == "HEAD",
        ))
    });
    resources
        .retain_response(response.clone())
        .map_err(E::from)?;
    handler.call((request, response))?;
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
                let read = stream
                    .read(&mut input[start..])
                    .map_err(runtime_http_io_error)?;
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
                        let read = stream
                            .read(&mut input[prior..])
                            .map_err(runtime_http_io_error)?;
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
                        if decoded.len().saturating_add(length) > RUNTIME_MAX_MATERIALIZED_BODY_SIZE
                        {
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
