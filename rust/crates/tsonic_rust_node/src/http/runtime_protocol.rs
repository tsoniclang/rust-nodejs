const MAX_RUNTIME_HEADER_COUNT: usize = 128;
const MAX_CHUNK_LINE_SIZE: usize = 8 * 1024;

pub(super) struct ParsedRequestHead {
    pub(super) method: String,
    pub(super) target: String,
    pub(super) version: String,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: RequestBody,
    pub(super) keep_alive: bool,
}

pub(super) enum RequestBody {
    None,
    ContentLength { remaining: usize },
    Chunked(ChunkDecoder),
}

pub(super) struct ChunkDecoder {
    phase: ChunkPhase,
}

enum ChunkPhase {
    Size,
    Data { remaining: usize },
    DataTerminator,
    Trailers,
    Complete,
}

pub(super) enum ChunkStep {
    NeedInput,
    Data { consumed: usize, length: usize },
    Consumed(usize),
    Complete(usize),
}

impl ChunkDecoder {
    fn new() -> Self {
        Self {
            phase: ChunkPhase::Size,
        }
    }

    pub(super) fn step(&mut self, input: &[u8]) -> NodeResult<ChunkStep> {
        match self.phase {
            ChunkPhase::Size => {
                let end = find_crlf(input);
                if has_malformed_line_endings(&input[..end.unwrap_or(input.len())]) {
                    return Err(NodeError::new(
                        "HPE_INVALID_CHUNK_SIZE",
                        "chunk-size line requires exact CRLF framing",
                    ));
                }
                let Some(end) = end else {
                    if input.len() > MAX_CHUNK_LINE_SIZE {
                        return Err(NodeError::new(
                            "HPE_INVALID_CHUNK_SIZE",
                            "chunk-size line exceeds the finite limit",
                        ));
                    }
                    return Ok(ChunkStep::NeedInput);
                };
                if end > MAX_CHUNK_LINE_SIZE {
                    return Err(NodeError::new(
                        "HPE_INVALID_CHUNK_SIZE",
                        "chunk-size line exceeds the finite limit",
                    ));
                }
                let line = std::str::from_utf8(&input[..end]).map_err(|_| {
                    NodeError::new("HPE_INVALID_CHUNK_SIZE", "chunk size is not ASCII")
                })?;
                let size = line.split(';').next().unwrap_or_default().trim();
                if size.is_empty() || !size.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return Err(NodeError::new(
                        "HPE_INVALID_CHUNK_SIZE",
                        "chunk size is not a hexadecimal integer",
                    ));
                }
                let size = usize::from_str_radix(size, 16).map_err(|_| {
                    NodeError::new("HPE_INVALID_CHUNK_SIZE", "chunk size exceeds native limits")
                })?;
                self.phase = if size == 0 {
                    ChunkPhase::Trailers
                } else {
                    ChunkPhase::Data { remaining: size }
                };
                Ok(ChunkStep::Consumed(end + 2))
            }
            ChunkPhase::Data { remaining } => {
                if input.is_empty() {
                    return Ok(ChunkStep::NeedInput);
                }
                let length = remaining.min(input.len()).min(16 * 1024);
                let next = remaining - length;
                self.phase = if next == 0 {
                    ChunkPhase::DataTerminator
                } else {
                    ChunkPhase::Data { remaining: next }
                };
                Ok(ChunkStep::Data {
                    consumed: length,
                    length,
                })
            }
            ChunkPhase::DataTerminator => {
                if input.len() < 2 {
                    return Ok(ChunkStep::NeedInput);
                }
                if &input[..2] != b"\r\n" {
                    return Err(NodeError::new(
                        "HPE_INVALID_CHUNK_SIZE",
                        "chunk data is missing its CRLF terminator",
                    ));
                }
                self.phase = ChunkPhase::Size;
                Ok(ChunkStep::Consumed(2))
            }
            ChunkPhase::Trailers => {
                if input.starts_with(b"\r\n") {
                    self.phase = ChunkPhase::Complete;
                    return Ok(ChunkStep::Complete(2));
                }
                let end = find_header_end(input);
                let trailer_prefix = &input[..end.map_or(input.len(), |position| position + 4)];
                if has_malformed_line_endings(trailer_prefix) {
                    return Err(NodeError::new(
                        "HPE_INVALID_HEADER_TOKEN",
                        "chunk trailers require exact CRLF framing",
                    ));
                }
                let Some(end) = end else {
                    if input.len() > MAX_HEADER_SIZE {
                        return Err(NodeError::new(
                            "HPE_HEADER_OVERFLOW",
                            "chunk trailers exceed the finite header limit",
                        ));
                    }
                    return Ok(ChunkStep::NeedInput);
                };
                if end + 4 > MAX_HEADER_SIZE {
                    return Err(NodeError::new(
                        "HPE_HEADER_OVERFLOW",
                        "chunk trailers exceed the finite header limit",
                    ));
                }
                validate_trailers(&input[..end])?;
                self.phase = ChunkPhase::Complete;
                Ok(ChunkStep::Complete(end + 4))
            }
            ChunkPhase::Complete => Ok(ChunkStep::Complete(0)),
        }
    }
}

pub(super) fn parse_request_head(input: &[u8]) -> NodeResult<Option<(usize, ParsedRequestHead)>> {
    if input.len() > MAX_HEADER_SIZE && find_header_end(input).is_none() {
        return Err(NodeError::new(
            "HPE_HEADER_OVERFLOW",
            "request headers exceed the finite header limit",
        ));
    }
    let mut headers = [httparse::EMPTY_HEADER; MAX_RUNTIME_HEADER_COUNT];
    let mut request = httparse::Request::new(&mut headers);
    let consumed = match request.parse(input).map_err(map_parse_error)? {
        httparse::Status::Partial => return Ok(None),
        httparse::Status::Complete(consumed) => consumed,
    };
    if consumed > MAX_HEADER_SIZE {
        return Err(NodeError::new(
            "HPE_HEADER_OVERFLOW",
            "request headers exceed the finite header limit",
        ));
    }
    let head = &input[..consumed];
    if !head.ends_with(b"\r\n\r\n")
        || head.starts_with(b"\r\n")
        || head.iter().enumerate().any(|(index, byte)| {
            (*byte == b'\r' && head.get(index + 1) != Some(&b'\n'))
                || (*byte == b'\n' && (index == 0 || head[index - 1] != b'\r'))
        })
    {
        return Err(NodeError::new(
            "HPE_INVALID_HEADER_TOKEN",
            "request headers require exact CRLF framing",
        ));
    }
    let method = request
        .method
        .ok_or_else(|| NodeError::new("HPE_INVALID_METHOD", "request method is missing"))?
        .to_string();
    let target = request
        .path
        .ok_or_else(|| NodeError::new("HPE_INVALID_URL", "request target is missing"))?
        .to_string();
    let minor = request
        .version
        .ok_or_else(|| NodeError::new("HPE_INVALID_VERSION", "HTTP version is missing"))?;
    let version = match minor {
        0 => "1.0",
        1 => "1.1",
        _ => {
            return Err(NodeError::new(
                "HPE_INVALID_VERSION",
                "only HTTP/1.0 and HTTP/1.1 are supported",
            ));
        }
    }
    .to_string();

    let mut pairs = Vec::with_capacity(request.headers.len());
    let mut content_lengths = Vec::new();
    let mut transfer_encodings = Vec::new();
    let mut connection_tokens = Vec::new();
    let mut host_count = 0;
    for header in request.headers {
        let value = latin1(header.value).trim().to_string();
        validate_header_value(header.name, &value)?;
        if header.name.eq_ignore_ascii_case("content-length") {
            content_lengths.push(parse_content_length(&value)?);
        } else if header.name.eq_ignore_ascii_case("transfer-encoding") {
            if value.split(',').any(|token| token.trim().is_empty()) {
                return Err(NodeError::new(
                    "HPE_INVALID_TRANSFER_ENCODING",
                    "transfer-encoding contains an empty token",
                ));
            }
            transfer_encodings.extend(csv_tokens(&value));
        } else if header.name.eq_ignore_ascii_case("connection") {
            connection_tokens.extend(csv_tokens(&value));
        } else if header.name.eq_ignore_ascii_case("host") {
            host_count += 1;
        }
        pairs.push((header.name.to_string(), value));
    }

    if minor == 1 && host_count != 1 {
        return Err(NodeError::new(
            "HPE_INVALID_HOST",
            "HTTP/1.1 requests require exactly one Host header",
        ));
    }

    if content_lengths.len() > 1 {
        return Err(NodeError::new(
            "HPE_UNEXPECTED_CONTENT_LENGTH",
            "duplicate content-length fields are prohibited",
        ));
    }
    let content_length = content_lengths.first().copied();
    if !transfer_encodings.is_empty() && content_length.is_some() {
        return Err(NodeError::new(
            "HPE_UNEXPECTED_CONTENT_LENGTH",
            "content-length cannot be combined with transfer-encoding",
        ));
    }
    let body = if transfer_encodings.is_empty() {
        match content_length {
            Some(0) | None => RequestBody::None,
            Some(remaining) => RequestBody::ContentLength { remaining },
        }
    } else if transfer_encodings.len() == 1 && transfer_encodings[0] == "chunked" {
        RequestBody::Chunked(ChunkDecoder::new())
    } else {
        return Err(NodeError::new(
            "HPE_INVALID_TRANSFER_ENCODING",
            "the only supported request transfer-encoding is chunked",
        ));
    };
    let keep_alive = if minor == 1 {
        !connection_tokens.iter().any(|token| token == "close")
    } else {
        connection_tokens.iter().any(|token| token == "keep-alive")
    };
    Ok(Some((
        consumed,
        ParsedRequestHead {
            method,
            target,
            version,
            headers: pairs,
            body,
            keep_alive,
        },
    )))
}

fn parse_content_length(value: &str) -> NodeResult<usize> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(NodeError::new(
            "HPE_INVALID_CONTENT_LENGTH",
            "content-length must be an unsigned decimal integer",
        ));
    }
    value.parse().map_err(|_| {
        NodeError::new(
            "HPE_INVALID_CONTENT_LENGTH",
            "content-length exceeds native limits",
        )
    })
}

fn csv_tokens(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|token| token.trim().to_ascii_lowercase())
        .filter(|token| !token.is_empty())
        .collect()
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| char::from(*byte)).collect()
}

fn validate_trailers(bytes: &[u8]) -> NodeResult<()> {
    let mut remaining = bytes;
    loop {
        let (line, next) = match find_crlf(remaining) {
            Some(end) => (&remaining[..end], Some(&remaining[end + 2..])),
            None => (remaining, None),
        };
        if line.is_empty() || line.iter().any(|byte| matches!(byte, b'\r' | b'\n')) {
            return Err(NodeError::new(
                "HPE_INVALID_HEADER_TOKEN",
                "chunk trailers require exact CRLF-separated header fields",
            ));
        }
        let Some(separator) = line.iter().position(|byte| *byte == b':') else {
            return Err(NodeError::new(
                "HPE_INVALID_HEADER_TOKEN",
                "chunk trailer is not a header field",
            ));
        };
        let name = std::str::from_utf8(&line[..separator])
            .map_err(|_| NodeError::new("HPE_INVALID_HEADER_TOKEN", "trailer name is not ASCII"))?;
        let value = latin1(&line[separator + 1..]).trim().to_string();
        validate_header_value(name, &value)?;
        match next {
            Some(rest) => remaining = rest,
            None => break,
        }
    }
    Ok(())
}

fn find_crlf(bytes: &[u8]) -> Option<usize> {
    bytes.windows(2).position(|window| window == b"\r\n")
}

fn has_malformed_line_endings(bytes: &[u8]) -> bool {
    bytes.iter().enumerate().any(|(index, byte)| {
        (*byte == b'\r' && bytes.get(index + 1).is_some_and(|next| *next != b'\n'))
            || (*byte == b'\n' && (index == 0 || bytes[index - 1] != b'\r'))
    })
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn map_parse_error(error: httparse::Error) -> NodeError {
    NodeError::new("HPE_INVALID_REQUEST", error.to_string())
}

#[cfg(test)]
mod runtime_protocol_tests {
    use super::{
        parse_request_head, ChunkDecoder, ChunkStep, RequestBody, MAX_CHUNK_LINE_SIZE,
        MAX_HEADER_SIZE,
    };

    #[test]
    fn fragmented_request_preserves_repeated_headers_and_body_framing() {
        let bytes = b"POST /upload HTTP/1.1\r\nHost: localhost\r\nX-Item: one\r\nX-Item: two\r\nContent-Length: 4\r\n\r\ndata";
        assert!(parse_request_head(&bytes[..25]).unwrap().is_none());
        let (consumed, request) = parse_request_head(bytes).unwrap().unwrap();
        assert_eq!(&bytes[consumed..], b"data");
        assert_eq!(
            request.headers[1],
            ("X-Item".to_string(), "one".to_string())
        );
        assert_eq!(
            request.headers[2],
            ("X-Item".to_string(), "two".to_string())
        );
        assert!(matches!(
            request.body,
            RequestBody::ContentLength { remaining: 4 }
        ));
        assert!(request.keep_alive);
    }

    #[test]
    fn streaming_content_length_is_not_capped_by_buffer_capacity() {
        let length = 64 * 1024 * 1024 + 1;
        let head =
            format!("POST /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: {length}\r\n\r\n");
        let (_, request) = parse_request_head(head.as_bytes()).unwrap().unwrap();
        assert!(
            matches!(request.body, RequestBody::ContentLength { remaining } if remaining == length)
        );
    }

    #[test]
    fn ambiguous_request_framing_is_rejected_before_dispatch() {
        for bytes in [
            b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 4\r\nTransfer-Encoding: chunked\r\n\r\n".as_slice(),
            b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 4\r\nContent-Length: 5\r\n\r\n".as_slice(),
            b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 4\r\nContent-Length: 4\r\n\r\n".as_slice(),
            b"GET / HTTP/1.1\r\nHost: one\r\nHost: two\r\n\r\n".as_slice(),
            b"POST / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: gzip, chunked\r\n\r\n".as_slice(),
            b"GET / HTTP/1.1\nHost: localhost\n\n".as_slice(),
            b"GET / HTTP/1.1\r\nHost: localhost\n\r\n".as_slice(),
            b"\r\nGET / HTTP/1.1\r\nHost: localhost\r\n\r\n".as_slice(),
        ] {
            assert!(parse_request_head(bytes).is_err());
        }
    }

    #[test]
    fn chunk_framing_rejects_oversized_lines_and_malformed_trailers() {
        let mut oversized = vec![b'a'; MAX_CHUNK_LINE_SIZE + 1];
        oversized.extend_from_slice(b"\r\n");
        assert!(ChunkDecoder::new().step(&oversized).is_err());

        let mut decoder = ChunkDecoder::new();
        assert!(matches!(
            decoder.step(b"0\r\n").unwrap(),
            ChunkStep::Consumed(3)
        ));
        assert!(decoder.step(b"X-Item: one\nX-Item: two\r\n\r\n").is_err());

        let mut decoder = ChunkDecoder::new();
        assert!(matches!(
            decoder.step(b"0\r\n").unwrap(),
            ChunkStep::Consumed(3)
        ));
        assert!(matches!(
            decoder.step(b"X-Item: one\r\n\r\n").unwrap(),
            ChunkStep::Complete(15)
        ));

        let mut decoder = ChunkDecoder::new();
        assert!(matches!(
            decoder.step(b"0\r\n").unwrap(),
            ChunkStep::Consumed(3)
        ));
        let mut oversized_trailers = b"X-Item: ".to_vec();
        oversized_trailers.extend(vec![b'a'; MAX_HEADER_SIZE]);
        oversized_trailers.extend_from_slice(b"\r\n\r\n");
        assert!(decoder.step(&oversized_trailers).is_err());

        assert!(ChunkDecoder::new().step(b"4;bad\npart\r\n").is_err());
        let mut decoder = ChunkDecoder::new();
        assert!(matches!(
            decoder.step(b"0\r\n").unwrap(),
            ChunkStep::Consumed(3)
        ));
        assert!(decoder.step(b"X-Item: one\n").is_err());

        let mut decoder = ChunkDecoder::new();
        assert!(matches!(
            decoder.step(b"1\r\n\n\r\n0\r\n\r\n").unwrap(),
            ChunkStep::Consumed(3)
        ));
        assert!(matches!(
            decoder.step(b"\n\r\n0\r\n\r\n").unwrap(),
            ChunkStep::Data {
                consumed: 1,
                length: 1
            }
        ));
        assert!(matches!(
            decoder.step(b"\r\n").unwrap(),
            ChunkStep::Consumed(2)
        ));
        assert!(matches!(
            decoder.step(b"0\r\n").unwrap(),
            ChunkStep::Consumed(3)
        ));
        assert!(matches!(
            decoder.step(b"\r\nGET /later HTTP/1.1\n\n").unwrap(),
            ChunkStep::Complete(2)
        ));
    }
}
