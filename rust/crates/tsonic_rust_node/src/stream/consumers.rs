use super::Readable;
use crate::buffer::Buffer;
use crate::error::NodeError;
use tsonic_rust_js::json;
use tsonic_rust_js::web::{Blob, BlobPart};
use tsonic_rust_js::{ArrayBuffer, JsValue};

pub fn buffer<E: From<NodeError> + 'static>(readable: &mut Readable<E>) -> Result<Buffer, E> {
    let mut chunks = Vec::new();
    while let Some(chunk) = readable.read()? {
        chunks.push(chunk);
    }
    Ok(Buffer::concat_dense(&chunks))
}

pub fn text<E: From<NodeError> + 'static>(
    readable: &mut Readable<E>,
    encoding: Option<&str>,
) -> Result<String, E> {
    buffer(readable)?.to_string(encoding).map_err(E::from)
}

pub fn array_buffer<E: From<NodeError> + 'static>(
    readable: &mut Readable<E>,
) -> Result<ArrayBuffer, E> {
    Ok(ArrayBuffer::from_bytes(
        buffer(readable)?.as_bytes().to_vec(),
    ))
}

pub fn blob<E: From<NodeError> + 'static>(
    readable: &mut Readable<E>,
    content_type: impl Into<String>,
) -> Result<Blob, E> {
    Ok(Blob::new(
        &[BlobPart::Bytes(buffer(readable)?.as_bytes().to_vec())],
        content_type,
    ))
}

pub fn json<E: From<NodeError> + 'static>(
    readable: &mut Readable<E>,
    encoding: Option<&str>,
) -> Result<JsValue, E> {
    json::parse(&text(readable, encoding)?)
        .map_err(|error| E::from(NodeError::new("ERR_INVALID_JSON", error.to_string())))
}
