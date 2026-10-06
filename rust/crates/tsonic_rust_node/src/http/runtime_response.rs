fn begin_response<E: From<NodeError> + 'static>(
    connection: &mut RuntimeConnection<E>,
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

fn queue_body_chunk<E: From<NodeError> + 'static>(
    connection: &mut RuntimeConnection<E>,
    chunk: Buffer,
) -> NodeResult<()> {
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

fn finish_response_wire<E: From<NodeError> + 'static>(
    connection: &mut RuntimeConnection<E>,
) -> NodeResult<()> {
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

fn finalize_response<E: From<NodeError> + 'static>(
    connection: &Rc<RefCell<RuntimeConnection<E>>>,
) -> NodeResult<()> {
    let mut state = connection.borrow_mut();
    let server_closing = state
        .server
        .upgrade()
        .is_some_and(|server| server.borrow().closing);
    if !state.body_complete
        || !state.keep_alive
        || state.close_after_response
        || state.peer_eof
        || server_closing
    {
        state.close();
        return Ok(());
    }
    let request = state.request.take();
    let response = state.response.take();
    state.body = RequestBody::None;
    state.body_complete = false;
    state.body_paused = false;
    state.keep_alive = false;
    state.wire = ResponseWireState::default();
    let result = state.refresh_interest();
    drop(state);
    drop(request);
    drop(response);
    result
}

fn queue_bad_request<E: From<NodeError> + 'static>(
    connection: &mut RuntimeConnection<E>,
) -> NodeResult<()> {
    connection.output.clear();
    connection.output_bytes = 0;
    connection.queue(Buffer::from_bytes(
        b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
    ))
}
