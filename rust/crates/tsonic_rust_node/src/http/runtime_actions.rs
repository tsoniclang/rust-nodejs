fn next_connection_action<E: From<NodeError> + 'static>(
    connection: &Rc<RefCell<RuntimeConnection<E>>>,
) -> NodeResult<Option<ConnectionAction<E>>> {
    let mut state = connection.borrow_mut();
    if state.closed {
        return Ok(None);
    }
    if let Some(request) = state
        .request
        .clone()
        .filter(|_| !state.body_complete && matches!(state.body, RequestBody::None))
    {
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
                let resume = Rc::downgrade(connection);
                let wake = crate::readiness::waker()?;
                request.set_capacity_handler(move || {
                    if let Some(owner) = resume.upgrade() {
                        let result = {
                            let mut state = owner.borrow_mut();
                            state.body_paused = false;
                            state.refresh_interest()
                        };
                        result.map_err(E::from)?;
                    }
                    wake.wake().map_err(runtime_http_io_error).map_err(E::from)
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
                        Rc::new(HttpWritableBackend::<E> {
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

fn next_body_action<E: From<NodeError> + 'static>(
    state: &mut RuntimeConnection<E>,
    request: IncomingMessage<E>,
) -> NodeResult<Option<ConnectionAction<E>>> {
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
            let length = (*remaining)
                .min(state.available_input().len())
                .min(16 * 1024);
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

fn read_connection_input<E: From<NodeError> + 'static>(
    state: &mut RuntimeConnection<E>,
) -> NodeResult<()> {
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

fn flush_connection_output<E: From<NodeError> + 'static>(
    state: &mut RuntimeConnection<E>,
) -> NodeResult<()> {
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
