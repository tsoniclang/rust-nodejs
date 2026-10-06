#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZlibMode {
    Deflate,
    Inflate,
    Gzip,
    Gunzip,
    DeflateRaw,
    InflateRaw,
    Unzip,
    BrotliCompress,
    BrotliDecompress,
}

const MAX_PENDING_ZLIB_OUTPUT: usize = 16 * 1024 * 1024;

pub struct Zlib<E: 'static = NodeError> {
    state: std::rc::Rc<std::cell::RefCell<ZlibState<E>>>,
    transform: crate::stream::Transform<E>,
}

struct ZlibState<E: 'static> {
    mode: ZlibMode,
    bytes_written: usize,
    closed: bool,
    options: ZlibOptions,
    output: std::rc::Rc<std::cell::RefCell<ZlibOutput>>,
    codec: Option<StreamingCodec<E>>,
}

struct ZlibOutput {
    pending: std::collections::VecDeque<Buffer>,
    pending_bytes: usize,
    total_bytes: usize,
    maximum_bytes: Option<usize>,
    terminal_error: Option<String>,
    publishing: bool,
}

impl ZlibOutput {
    fn new(maximum_bytes: Option<usize>) -> Self {
        Self {
            pending: std::collections::VecDeque::new(),
            pending_bytes: 0,
            total_bytes: 0,
            maximum_bytes,
            terminal_error: None,
            publishing: false,
        }
    }
}

struct ZlibSink<E: 'static> {
    output: std::rc::Rc<std::cell::RefCell<ZlibOutput>>,
    readable: crate::stream::WeakReadable<E>,
}

impl<E: 'static> Write for ZlibSink<E> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let queued_bytes = self
            .readable
            .upgrade()
            .map_or(0, |readable| readable.queued_bytes());
        let mut output = self.output.borrow_mut();
        let total_bytes = match output.total_bytes.checked_add(bytes.len()) {
            Some(total) => total,
            None => {
                output.terminal_error =
                    Some("compressed output exceeds the native byte range".to_string());
                return Err(std::io::Error::new(
                    std::io::ErrorKind::OutOfMemory,
                    "compressed output exceeds the native byte range",
                ));
            }
        };
        if output
            .maximum_bytes
            .is_some_and(|maximum| total_bytes > maximum)
        {
            output.terminal_error = Some("compressed output exceeds maxOutputLength".to_string());
            return Err(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                "compressed output exceeds maxOutputLength",
            ));
        }
        let pending = output
            .pending_bytes
            .checked_add(bytes.len())
            .and_then(|bytes| bytes.checked_add(queued_bytes));
        if pending.is_none_or(|bytes| bytes > MAX_PENDING_ZLIB_OUTPUT) {
            output.terminal_error =
                Some("pending zlib output exceeds the finite runtime limit".to_string());
            return Err(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                "pending zlib output exceeds the finite runtime limit",
            ));
        }
        if !bytes.is_empty() {
            output.pending.push_back(Buffer::from_bytes(bytes.to_vec()));
            output.pending_bytes += bytes.len();
            output.total_bytes = total_bytes;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

enum StreamingCodec<E: 'static> {
    Deflate(ZlibWriteEncoder<ZlibSink<E>>),
    Inflate(ZlibWriteDecoder<ZlibSink<E>>),
    Gzip(GzWriteEncoder<ZlibSink<E>>),
    Gunzip(GzWriteDecoder<ZlibSink<E>>),
    DeflateRaw(DeflateWriteEncoder<ZlibSink<E>>),
    InflateRaw(DeflateWriteDecoder<ZlibSink<E>>),
    BrotliCompress(brotli::CompressorWriter<ZlibSink<E>>),
    BrotliDecompress(brotli::DecompressorWriter<ZlibSink<E>>),
}

struct ZlibBackend<E: 'static> {
    state: std::rc::Rc<std::cell::RefCell<ZlibState<E>>>,
    readable: crate::stream::Readable<E>,
}

impl<E: From<NodeError> + 'static> crate::stream::WritableBackend<E> for ZlibBackend<E> {
    fn bind(&self, owner: crate::stream::WeakWritable<E>) {
        let state = std::rc::Rc::clone(&self.state);
        let readable = self.readable.downgrade();
        self.readable.set_capacity_handler(move || {
            if let Some(readable) = readable.upgrade() {
                publish_codec_output(&state, &readable)?;
            }
            if let Some(writable) = owner.upgrade() {
                writable.poll_progress()?;
            }
            Ok(())
        });
    }

    fn write(&self, input: Buffer) -> crate::stream::StreamBackendResult<(), E> {
        let state = &self.state;
        {
            let mut state = state.borrow_mut();
            if state.closed {
                return Err(NodeError::new(
                    "ERR_STREAM_WRITE_AFTER_END",
                    "compression stream is closed",
                )
                .into());
            }
            state.bytes_written = state.bytes_written.saturating_add(input.len());
            let codec = state
                .codec
                .as_mut()
                .ok_or_else(unsupported_streaming_codec)?;
            input
                .with_bytes(|bytes| codec.write_all(bytes))
                .map_err(map_zlib_error)?;
        }
        publish_codec_output(state, &self.readable)
            .map_err(crate::stream::StreamBackendFailure::Callback)
    }

    fn finish(&self) -> crate::stream::StreamBackendResult<bool, E> {
        let state = &self.state;
        let codec = {
            let mut state = state.borrow_mut();
            if state.closed {
                None
            } else {
                let codec = state.codec.take().ok_or_else(unsupported_streaming_codec)?;
                state.closed = true;
                Some(codec)
            }
        };
        if let Some(codec) = codec {
            codec.finish().map_err(map_zlib_error)?;
        }
        publish_codec_output(state, &self.readable)
            .map_err(crate::stream::StreamBackendFailure::Callback)?;
        Ok(true)
    }

    fn finish_accepted(&self) -> bool {
        self.state.borrow().closed
    }

    fn destroy(&self) -> crate::stream::StreamBackendResult<(), E> {
        {
            let mut state = self.state.borrow_mut();
            state.closed = true;
            state.codec = None;
            let mut output = state.output.borrow_mut();
            output.pending.clear();
            output.pending_bytes = 0;
        }
        self.readable
            .destroy()
            .map_err(crate::stream::StreamBackendFailure::Callback)
    }

    fn buffered_bytes(&self) -> usize {
        self.readable.queued_bytes()
    }
}

impl<E: From<NodeError> + 'static> Zlib<E> {
    pub fn new(mode: ZlibMode, options: Option<ZlibOptions>) -> Self {
        let options = options.unwrap_or_default();
        let output = std::rc::Rc::new(std::cell::RefCell::new(ZlibOutput::new(
            options.max_output_length,
        )));
        let readable = crate::stream::Readable::<E>::open(crate::stream::StreamOptions {
            high_water_mark: options.chunk_size.max(1),
            ..Default::default()
        });
        let sink = ZlibSink::<E> {
            output: output.clone(),
            readable: readable.downgrade(),
        };
        let codec = match mode {
            ZlibMode::Deflate => Some(StreamingCodec::<E>::Deflate(ZlibWriteEncoder::new(
                sink,
                compression_from_level(options.level),
            ))),
            ZlibMode::Inflate => Some(StreamingCodec::<E>::Inflate(ZlibWriteDecoder::new(sink))),
            ZlibMode::Gzip => Some(StreamingCodec::<E>::Gzip(GzWriteEncoder::new(
                sink,
                compression_from_level(options.level),
            ))),
            ZlibMode::Gunzip => Some(StreamingCodec::<E>::Gunzip(GzWriteDecoder::new(sink))),
            ZlibMode::DeflateRaw => Some(StreamingCodec::<E>::DeflateRaw(
                DeflateWriteEncoder::new(sink, compression_from_level(options.level)),
            )),
            ZlibMode::InflateRaw => Some(StreamingCodec::<E>::InflateRaw(
                DeflateWriteDecoder::new(sink),
            )),
            ZlibMode::BrotliCompress => Some(StreamingCodec::<E>::BrotliCompress(
                brotli::CompressorWriter::new(sink, options.chunk_size.max(1), 5, 22),
            )),
            ZlibMode::BrotliDecompress => Some(StreamingCodec::<E>::BrotliDecompress(
                brotli::DecompressorWriter::new(sink, options.chunk_size.max(1)),
            )),
            ZlibMode::Unzip => None,
        };
        let state = std::rc::Rc::new(std::cell::RefCell::new(ZlibState::<E> {
            mode,
            bytes_written: 0,
            closed: false,
            options: options.clone(),
            output,
            codec,
        }));
        let stream_options = crate::stream::StreamOptions {
            high_water_mark: options.chunk_size.max(1),
            ..Default::default()
        };
        let writable = crate::stream::Writable::<E>::with_backend(
            stream_options,
            std::rc::Rc::new(ZlibBackend::<E> {
                state: std::rc::Rc::clone(&state),
                readable: readable.clone(),
            }),
            Some(readable.lifecycle()),
        );
        Self {
            state,
            transform: crate::stream::Transform::<E>::from_parts(readable, writable),
        }
    }

    pub fn bytes_written(&self) -> usize {
        self.state.borrow().bytes_written
    }

    pub fn close(&self, callback: Option<impl FnOnce()>) -> Result<(), E> {
        self.transform.duplex_handle().destroy()?;
        self.state.borrow_mut().closed = true;
        if let Some(callback) = callback {
            callback();
        }
        Ok(())
    }

    pub fn closed(&self) -> bool {
        self.state.borrow().closed
    }

    pub fn reset(&mut self) {
        let state = self.state.borrow();
        let mode = state.mode;
        let options = state.options.clone();
        drop(state);
        *self = Self::new(mode, Some(options));
    }

    pub fn flush(&self, callback: Option<impl FnOnce()>) -> Result<(), E> {
        let native = {
            let mut state = self.state.borrow_mut();
            state.codec.as_mut().map_or(Ok(()), Write::flush)
        };
        native.map_err(map_zlib_error)?;
        publish_codec_output(&self.state, &self.transform.readable_handle())?;
        if let Some(callback) = callback {
            callback();
        }
        Ok(())
    }

    pub fn params(&mut self, level: i32, strategy: i32, callback: impl FnOnce()) {
        let mut state = self.state.borrow_mut();
        state.options.level = level;
        state.options.strategy = strategy;
        drop(state);
        callback();
    }

    pub fn process(&mut self, input: &Buffer) -> NodeResult<Buffer> {
        let mut state = self.state.borrow_mut();
        state.bytes_written += input.len();
        match state.mode {
            ZlibMode::Deflate => deflate_sync_with_options(input, &state.options),
            ZlibMode::Inflate => inflate_sync(input),
            ZlibMode::Gzip => gzip_sync_with_options(input, &state.options),
            ZlibMode::Gunzip => gunzip_sync(input),
            ZlibMode::DeflateRaw => deflate_raw_sync(input),
            ZlibMode::InflateRaw => inflate_raw_sync(input),
            ZlibMode::Unzip => unzip_sync(input),
            ZlibMode::BrotliCompress => brotli_compress_sync(input),
            ZlibMode::BrotliDecompress => brotli_decompress_sync(input),
        }
    }

    pub fn write(&self, input: Buffer) -> Result<bool, E> {
        self.transform.writable_handle().write_buffer(&input)
    }

    pub fn read(&self) -> Result<Option<Buffer>, E> {
        self.transform.read()
    }

    pub fn end(&self) -> Result<(), E> {
        self.transform.end().map(|_| ())
    }

    pub fn transform_handle(&self) -> crate::stream::Transform<E> {
        self.transform.clone()
    }
}

impl<E: 'static> std::fmt::Debug for Zlib<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.borrow();
        formatter
            .debug_struct("Zlib")
            .field("mode", &state.mode)
            .field("bytes_written", &state.bytes_written)
            .field("closed", &state.closed)
            .finish()
    }
}

impl<E: 'static> PartialEq for Zlib<E> {
    fn eq(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.state, &other.state)
    }
}

impl<E: 'static> Eq for Zlib<E> {}

impl<E: From<NodeError> + 'static> crate::stream::WritableTarget<E> for Zlib<E> {
    fn writable_handle(&self) -> crate::stream::Writable<E> {
        self.transform.writable_handle()
    }
}

pub fn zlib_as_transform<E: From<NodeError> + 'static>(
    value: &Zlib<E>,
) -> crate::stream::Transform<E> {
    value.transform_handle()
}

pub fn zlib_as_duplex<E: From<NodeError> + 'static>(value: &Zlib<E>) -> crate::stream::Duplex<E> {
    value.transform_handle().duplex_handle()
}

pub fn zlib_as_readable<E: From<NodeError> + 'static>(
    value: &Zlib<E>,
) -> crate::stream::Readable<E> {
    value.transform_handle().readable_handle()
}

pub fn zlib_as_writable<E: From<NodeError> + 'static>(
    value: &Zlib<E>,
) -> crate::stream::Writable<E> {
    value.transform_handle().writable_handle()
}

pub fn zlib_as_stream<E: From<NodeError> + 'static>(value: &Zlib<E>) -> crate::stream::Stream<E> {
    crate::stream::transform_as_stream(&value.transform_handle())
}

impl<E: From<NodeError> + 'static> Write for StreamingCodec<E> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Deflate(codec) => codec.write(bytes),
            Self::Inflate(codec) => codec.write(bytes),
            Self::Gzip(codec) => codec.write(bytes),
            Self::Gunzip(codec) => codec.write(bytes),
            Self::DeflateRaw(codec) => codec.write(bytes),
            Self::InflateRaw(codec) => codec.write(bytes),
            Self::BrotliCompress(codec) => codec.write(bytes),
            Self::BrotliDecompress(codec) => codec.write(bytes),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Deflate(codec) => codec.flush(),
            Self::Inflate(codec) => codec.flush(),
            Self::Gzip(codec) => codec.flush(),
            Self::Gunzip(codec) => codec.flush(),
            Self::DeflateRaw(codec) => codec.flush(),
            Self::InflateRaw(codec) => codec.flush(),
            Self::BrotliCompress(codec) => codec.flush(),
            Self::BrotliDecompress(codec) => codec.flush(),
        }
    }
}

impl<E: From<NodeError> + 'static> StreamingCodec<E> {
    fn finish(self) -> std::io::Result<()> {
        match self {
            Self::Deflate(mut codec) => codec.try_finish(),
            Self::Inflate(mut codec) => codec.try_finish(),
            Self::Gzip(mut codec) => codec.try_finish(),
            Self::Gunzip(mut codec) => codec.try_finish(),
            Self::DeflateRaw(mut codec) => codec.try_finish(),
            Self::InflateRaw(mut codec) => codec.try_finish(),
            Self::BrotliCompress(codec) => {
                let output = codec.get_ref().output.clone();
                let _ = codec.into_inner();
                let terminal_error = output.borrow().terminal_error.clone();
                match terminal_error {
                    Some(message) => Err(std::io::Error::new(
                        std::io::ErrorKind::OutOfMemory,
                        message,
                    )),
                    None => Ok(()),
                }
            }
            Self::BrotliDecompress(mut codec) => codec.close(),
        }
    }
}

fn publish_codec_output<E: From<NodeError> + 'static>(
    state: &std::rc::Rc<std::cell::RefCell<ZlibState<E>>>,
    readable: &crate::stream::Readable<E>,
) -> Result<(), E> {
    let output = state.borrow().output.clone();
    {
        let mut output = output.borrow_mut();
        if output.publishing {
            return Ok(());
        }
        output.publishing = true;
    }
    let _publication = ZlibPublication(&output);
    loop {
        if readable.pressured() {
            return Ok(());
        }
        let chunk = {
            let mut output = output.borrow_mut();
            output.pending.pop_front().map(|chunk| {
                output.pending_bytes = output.pending_bytes.saturating_sub(chunk.len());
                chunk
            })
        };
        let Some(chunk) = chunk else {
            let closed = state.borrow().closed;
            return if closed {
                readable.finish_input()
            } else {
                Ok(())
            };
        };
        readable.enqueue(chunk)?;
    }
}

struct ZlibPublication<'a>(&'a std::cell::RefCell<ZlibOutput>);
impl Drop for ZlibPublication<'_> {
    fn drop(&mut self) {
        self.0.borrow_mut().publishing = false;
    }
}

fn unsupported_streaming_codec() -> NodeError {
    NodeError::new(
        "ERR_UNSUPPORTED_OPERATION",
        "the selected compression mode has no streaming implementation",
    )
}

impl<E: 'static> Clone for Zlib<E> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            transform: self.transform.clone(),
        }
    }
}
