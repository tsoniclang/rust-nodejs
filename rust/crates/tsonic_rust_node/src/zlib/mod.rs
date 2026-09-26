#[allow(non_upper_case_globals)]
pub mod constants;
mod source_abi;

pub use source_abi::{
    create_brotli_compress_source, create_brotli_decompress_source, create_deflate_raw_source,
    create_deflate_source, create_gunzip_source, create_gzip_source, create_inflate_raw_source,
    create_inflate_source, deflate_callable, deflate_options_callable, deflate_raw_sync_source,
    deflate_sync_source, gunzip_callable, gunzip_options_callable, gunzip_sync_source,
    gzip_callable, gzip_options_callable, gzip_sync_source, inflate_callable,
    inflate_options_callable, inflate_raw_sync_source, inflate_sync_source, SourceBrotliOptions,
    SourceZlibOptions,
};

use std::io::{Read, Write};

use flate2::read::{
    DeflateDecoder as DeflateReadDecoder, GzDecoder as GzReadDecoder,
    ZlibDecoder as ZlibReadDecoder,
};
use flate2::write::{
    DeflateDecoder as DeflateWriteDecoder, DeflateEncoder as DeflateWriteEncoder,
    GzDecoder as GzWriteDecoder, GzEncoder as GzWriteEncoder, ZlibDecoder as ZlibWriteDecoder,
    ZlibEncoder as ZlibWriteEncoder,
};
use flate2::Compression;

use crate::buffer::Buffer;
use crate::error::{NodeError, NodeResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZlibOptions {
    pub flush: Option<i32>,
    pub finish_flush: Option<i32>,
    pub chunk_size: usize,
    pub window_bits: Option<i32>,
    pub level: i32,
    pub mem_level: Option<i32>,
    pub strategy: i32,
    pub max_output_length: Option<usize>,
    pub dictionary: Option<Buffer>,
    pub info: bool,
}

impl Default for ZlibOptions {
    fn default() -> Self {
        Self {
            flush: None,
            finish_flush: None,
            chunk_size: constants::Z_DEFAULT_CHUNK as usize,
            window_bits: None,
            level: constants::Z_DEFAULT_COMPRESSION,
            mem_level: None,
            strategy: constants::Z_DEFAULT_STRATEGY,
            max_output_length: None,
            dictionary: None,
            info: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BackgroundZlibOptions {
    flush: Option<i32>,
    finish_flush: Option<i32>,
    chunk_size: usize,
    window_bits: Option<i32>,
    level: i32,
    mem_level: Option<i32>,
    strategy: i32,
    max_output_length: Option<usize>,
    dictionary: Option<Vec<u8>>,
    info: bool,
}

impl From<ZlibOptions> for BackgroundZlibOptions {
    fn from(value: ZlibOptions) -> Self {
        Self {
            flush: value.flush,
            finish_flush: value.finish_flush,
            chunk_size: value.chunk_size,
            window_bits: value.window_bits,
            level: value.level,
            mem_level: value.mem_level,
            strategy: value.strategy,
            max_output_length: value.max_output_length,
            dictionary: value.dictionary.map(|dictionary| dictionary.as_bytes()),
            info: value.info,
        }
    }
}

impl BackgroundZlibOptions {
    fn into_runtime(self) -> ZlibOptions {
        ZlibOptions {
            flush: self.flush,
            finish_flush: self.finish_flush,
            chunk_size: self.chunk_size,
            window_bits: self.window_bits,
            level: self.level,
            mem_level: self.mem_level,
            strategy: self.strategy,
            max_output_length: self.max_output_length,
            dictionary: self.dictionary.map(Buffer::from_bytes),
            info: self.info,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrotliOptions {
    pub flush: Option<i32>,
    pub finish_flush: Option<i32>,
    pub chunk_size: usize,
    pub params: std::collections::BTreeMap<i32, i32>,
    pub max_output_length: Option<usize>,
    pub info: bool,
}

impl Default for BrotliOptions {
    fn default() -> Self {
        Self {
            flush: None,
            finish_flush: None,
            chunk_size: constants::Z_DEFAULT_CHUNK as usize,
            params: std::collections::BTreeMap::new(),
            max_output_length: None,
            info: false,
        }
    }
}

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

#[derive(Clone)]
pub struct Zlib {
    state: std::rc::Rc<std::cell::RefCell<ZlibState>>,
    transform: crate::stream::Transform,
}

struct ZlibState {
    mode: ZlibMode,
    bytes_written: usize,
    closed: bool,
    options: ZlibOptions,
    output: std::rc::Rc<std::cell::RefCell<ZlibOutput>>,
    codec: Option<StreamingCodec>,
}

struct ZlibOutput {
    pending: std::collections::VecDeque<Buffer>,
    pending_bytes: usize,
    total_bytes: usize,
    maximum_bytes: Option<usize>,
    terminal_error: Option<String>,
}

impl ZlibOutput {
    fn new(maximum_bytes: Option<usize>) -> Self {
        Self {
            pending: std::collections::VecDeque::new(),
            pending_bytes: 0,
            total_bytes: 0,
            maximum_bytes,
            terminal_error: None,
        }
    }
}

#[derive(Clone)]
struct ZlibSink {
    output: std::rc::Rc<std::cell::RefCell<ZlibOutput>>,
    readable: crate::stream::Readable,
}

impl Write for ZlibSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
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
            .and_then(|bytes| bytes.checked_add(self.readable.queued_bytes()));
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

enum StreamingCodec {
    Deflate(ZlibWriteEncoder<ZlibSink>),
    Inflate(ZlibWriteDecoder<ZlibSink>),
    Gzip(GzWriteEncoder<ZlibSink>),
    Gunzip(GzWriteDecoder<ZlibSink>),
    DeflateRaw(DeflateWriteEncoder<ZlibSink>),
    InflateRaw(DeflateWriteDecoder<ZlibSink>),
    BrotliCompress(brotli::CompressorWriter<ZlibSink>),
    BrotliDecompress(brotli::DecompressorWriter<ZlibSink>),
}

struct ZlibBackend {
    state: std::rc::Weak<std::cell::RefCell<ZlibState>>,
    readable: crate::stream::Readable,
}

impl crate::stream::WritableBackend for ZlibBackend {
    fn bind(&self, owner: crate::stream::WeakWritable) {
        self.readable.set_capacity_handler(move || {
            if let Some(writable) = owner.upgrade() {
                writable.poll_progress()?;
            }
            Ok(())
        });
    }

    fn write(&self, input: Buffer) -> NodeResult<()> {
        let state = self.state.upgrade().ok_or_else(|| {
            NodeError::new(
                "ERR_STREAM_DESTROYED",
                "compression stream is no longer available",
            )
        })?;
        {
            let mut state = state.borrow_mut();
            if state.closed {
                return Err(NodeError::new(
                    "ERR_STREAM_WRITE_AFTER_END",
                    "compression stream is closed",
                ));
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
        publish_codec_output(&state, &self.readable)
    }

    fn finish(&self) -> NodeResult<bool> {
        let state = self.state.upgrade().ok_or_else(|| {
            NodeError::new(
                "ERR_STREAM_DESTROYED",
                "compression stream is no longer available",
            )
        })?;
        let codec = {
            let mut state = state.borrow_mut();
            state.closed = true;
            state.codec.take().ok_or_else(unsupported_streaming_codec)?
        };
        codec.finish().map_err(map_zlib_error)?;
        publish_codec_output(&state, &self.readable)?;
        self.readable.finish_input()?;
        Ok(true)
    }

    fn destroy(&self) -> NodeResult<()> {
        if let Some(state) = self.state.upgrade() {
            let mut state = state.borrow_mut();
            state.closed = true;
            state.codec = None;
            let mut output = state.output.borrow_mut();
            output.pending.clear();
            output.pending_bytes = 0;
        }
        self.readable.destroy();
        Ok(())
    }

    fn buffered_bytes(&self) -> usize {
        self.readable.queued_bytes()
    }
}

impl Zlib {
    pub fn new(mode: ZlibMode, options: Option<ZlibOptions>) -> Self {
        let options = options.unwrap_or_default();
        let output = std::rc::Rc::new(std::cell::RefCell::new(ZlibOutput::new(
            options.max_output_length,
        )));
        let readable = crate::stream::Readable::open(crate::stream::StreamOptions {
            high_water_mark: options.chunk_size.max(1),
            ..Default::default()
        });
        let sink = ZlibSink {
            output: output.clone(),
            readable: readable.clone(),
        };
        let codec = match mode {
            ZlibMode::Deflate => Some(StreamingCodec::Deflate(ZlibWriteEncoder::new(
                sink,
                compression_from_level(options.level),
            ))),
            ZlibMode::Inflate => Some(StreamingCodec::Inflate(ZlibWriteDecoder::new(sink))),
            ZlibMode::Gzip => Some(StreamingCodec::Gzip(GzWriteEncoder::new(
                sink,
                compression_from_level(options.level),
            ))),
            ZlibMode::Gunzip => Some(StreamingCodec::Gunzip(GzWriteDecoder::new(sink))),
            ZlibMode::DeflateRaw => Some(StreamingCodec::DeflateRaw(DeflateWriteEncoder::new(
                sink,
                compression_from_level(options.level),
            ))),
            ZlibMode::InflateRaw => {
                Some(StreamingCodec::InflateRaw(DeflateWriteDecoder::new(sink)))
            }
            ZlibMode::BrotliCompress => Some(StreamingCodec::BrotliCompress(
                brotli::CompressorWriter::new(sink, options.chunk_size.max(1), 5, 22),
            )),
            ZlibMode::BrotliDecompress => Some(StreamingCodec::BrotliDecompress(
                brotli::DecompressorWriter::new(sink, options.chunk_size.max(1)),
            )),
            ZlibMode::Unzip => None,
        };
        let state = std::rc::Rc::new(std::cell::RefCell::new(ZlibState {
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
        let writable = crate::stream::Writable::with_backend(
            stream_options,
            std::rc::Rc::new(ZlibBackend {
                state: std::rc::Rc::downgrade(&state),
                readable: readable.clone(),
            }),
        );
        Self {
            state,
            transform: crate::stream::Transform::from_parts(readable, writable),
        }
    }

    pub fn bytes_written(&self) -> usize {
        self.state.borrow().bytes_written
    }

    pub fn close(&self, callback: Option<impl FnOnce()>) {
        self.transform.duplex_handle().destroy();
        self.state.borrow_mut().closed = true;
        if let Some(callback) = callback {
            callback();
        }
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

    pub fn flush(&self, callback: Option<impl FnOnce()>) {
        if let Some(codec) = self.state.borrow_mut().codec.as_mut() {
            let _ = codec.flush();
        }
        let _ = publish_codec_output(&self.state, &self.transform.readable_handle());
        if let Some(callback) = callback {
            callback();
        }
    }

    pub fn params(&mut self, level: i32, strategy: i32, callback: impl FnOnce()) {
        let mut state = self.state.borrow_mut();
        state.options.level = level;
        state.options.strategy = strategy;
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

    pub fn write(&self, input: Buffer) -> NodeResult<bool> {
        self.transform.writable_handle().write_buffer(&input)
    }

    pub fn read(&self) -> Option<Buffer> {
        self.transform.read()
    }

    pub fn end(&self) -> NodeResult<()> {
        self.transform.end_checked()
    }

    pub fn transform_handle(&self) -> crate::stream::Transform {
        self.transform.clone()
    }
}

impl std::fmt::Debug for Zlib {
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

impl PartialEq for Zlib {
    fn eq(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.state, &other.state)
    }
}

impl Eq for Zlib {}

impl crate::stream::WritableTarget for Zlib {
    fn writable_handle(&self) -> crate::stream::Writable {
        self.transform.writable_handle()
    }
}

pub fn zlib_as_transform(value: &Zlib) -> crate::stream::Transform {
    value.transform_handle()
}

pub fn zlib_as_duplex(value: &Zlib) -> crate::stream::Duplex {
    value.transform_handle().duplex_handle()
}

pub fn zlib_as_readable(value: &Zlib) -> crate::stream::Readable {
    value.transform_handle().readable_handle()
}

pub fn zlib_as_writable(value: &Zlib) -> crate::stream::Writable {
    value.transform_handle().writable_handle()
}

pub fn zlib_as_stream(value: &Zlib) -> crate::stream::Stream {
    crate::stream::transform_as_stream(&value.transform_handle())
}

impl Write for StreamingCodec {
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

impl StreamingCodec {
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

fn publish_codec_output(
    state: &std::rc::Rc<std::cell::RefCell<ZlibState>>,
    readable: &crate::stream::Readable,
) -> NodeResult<()> {
    let output = state.borrow().output.clone();
    let chunks = {
        let mut output = output.borrow_mut();
        output.pending_bytes = 0;
        output.pending.drain(..).collect::<Vec<_>>()
    };
    for chunk in chunks {
        readable.enqueue(chunk)?;
    }
    Ok(())
}

fn unsupported_streaming_codec() -> NodeError {
    NodeError::new(
        "ERR_UNSUPPORTED_OPERATION",
        "the selected compression mode has no streaming implementation",
    )
}

pub type Deflate = Zlib;
pub type Inflate = Zlib;
pub type Gzip = Zlib;
pub type Gunzip = Zlib;
pub type DeflateRaw = Zlib;
pub type InflateRaw = Zlib;
pub type Unzip = Zlib;
pub type BrotliCompress = Zlib;
pub type BrotliDecompress = Zlib;

pub fn create_deflate(options: Option<ZlibOptions>) -> Deflate {
    Zlib::new(ZlibMode::Deflate, options)
}

pub fn create_inflate(options: Option<ZlibOptions>) -> Inflate {
    Zlib::new(ZlibMode::Inflate, options)
}

pub fn create_gzip(options: Option<ZlibOptions>) -> Gzip {
    Zlib::new(ZlibMode::Gzip, options)
}

pub fn create_gunzip(options: Option<ZlibOptions>) -> Gunzip {
    Zlib::new(ZlibMode::Gunzip, options)
}

pub fn create_deflate_raw(options: Option<ZlibOptions>) -> DeflateRaw {
    Zlib::new(ZlibMode::DeflateRaw, options)
}

pub fn create_inflate_raw(options: Option<ZlibOptions>) -> InflateRaw {
    Zlib::new(ZlibMode::InflateRaw, options)
}

pub fn create_unzip(options: Option<ZlibOptions>) -> Unzip {
    Zlib::new(ZlibMode::Unzip, options)
}

pub fn create_brotli_compress(options: Option<BrotliOptions>) -> BrotliCompress {
    let options = options.unwrap_or_default();
    Zlib::new(
        ZlibMode::BrotliCompress,
        Some(ZlibOptions {
            chunk_size: options.chunk_size,
            max_output_length: options.max_output_length,
            ..ZlibOptions::default()
        }),
    )
}

pub fn create_brotli_decompress(options: Option<BrotliOptions>) -> BrotliDecompress {
    let options = options.unwrap_or_default();
    Zlib::new(
        ZlibMode::BrotliDecompress,
        Some(ZlibOptions {
            chunk_size: options.chunk_size,
            max_output_length: options.max_output_length,
            ..ZlibOptions::default()
        }),
    )
}

pub fn gzip_sync(input: &tsonic_rust_js::Uint8Array) -> NodeResult<Buffer> {
    gzip_sync_with_options(input, &ZlibOptions::default())
}

pub fn gzip_sync_with_options(
    input: &tsonic_rust_js::Uint8Array,
    options: &ZlibOptions,
) -> NodeResult<Buffer> {
    let mut encoder = GzWriteEncoder::new(Vec::new(), compression_from_level(options.level));
    input
        .with_bytes(|bytes| encoder.write_all(bytes))
        .map_err(map_zlib_error)?;
    limit_output(
        encoder.finish().map_err(map_zlib_error)?,
        options.max_output_length,
    )
}

pub fn gunzip_sync(input: &tsonic_rust_js::Uint8Array) -> NodeResult<Buffer> {
    gunzip_sync_with_options(input, &ZlibOptions::default())
}

pub fn gunzip_sync_with_options(
    input: &tsonic_rust_js::Uint8Array,
    options: &ZlibOptions,
) -> NodeResult<Buffer> {
    input.with_bytes(|bytes| {
        let mut decoder = GzReadDecoder::new(bytes);
        let mut output = Vec::new();
        decoder.read_to_end(&mut output).map_err(map_zlib_error)?;
        limit_output(output, options.max_output_length)
    })
}

pub fn deflate_sync(input: &tsonic_rust_js::Uint8Array) -> NodeResult<Buffer> {
    deflate_sync_with_options(input, &ZlibOptions::default())
}

pub fn deflate_sync_with_options(
    input: &tsonic_rust_js::Uint8Array,
    options: &ZlibOptions,
) -> NodeResult<Buffer> {
    let mut encoder = ZlibWriteEncoder::new(Vec::new(), compression_from_level(options.level));
    input
        .with_bytes(|bytes| encoder.write_all(bytes))
        .map_err(map_zlib_error)?;
    limit_output(
        encoder.finish().map_err(map_zlib_error)?,
        options.max_output_length,
    )
}

pub fn inflate_sync(input: &tsonic_rust_js::Uint8Array) -> NodeResult<Buffer> {
    inflate_sync_with_options(input, &ZlibOptions::default())
}

pub fn inflate_sync_with_options(
    input: &tsonic_rust_js::Uint8Array,
    options: &ZlibOptions,
) -> NodeResult<Buffer> {
    input.with_bytes(|bytes| {
        let mut decoder = ZlibReadDecoder::new(bytes);
        let mut output = Vec::new();
        decoder.read_to_end(&mut output).map_err(map_zlib_error)?;
        limit_output(output, options.max_output_length)
    })
}

pub fn deflate_raw_sync(input: &tsonic_rust_js::Uint8Array) -> NodeResult<Buffer> {
    deflate_raw_sync_with_options(input, &ZlibOptions::default())
}

pub fn deflate_raw_sync_with_options(
    input: &tsonic_rust_js::Uint8Array,
    options: &ZlibOptions,
) -> NodeResult<Buffer> {
    let mut encoder = DeflateWriteEncoder::new(Vec::new(), compression_from_level(options.level));
    input
        .with_bytes(|bytes| encoder.write_all(bytes))
        .map_err(map_zlib_error)?;
    limit_output(
        encoder.finish().map_err(map_zlib_error)?,
        options.max_output_length,
    )
}

pub fn inflate_raw_sync(input: &tsonic_rust_js::Uint8Array) -> NodeResult<Buffer> {
    inflate_raw_sync_with_options(input, &ZlibOptions::default())
}

pub fn inflate_raw_sync_with_options(
    input: &tsonic_rust_js::Uint8Array,
    options: &ZlibOptions,
) -> NodeResult<Buffer> {
    input.with_bytes(|bytes| {
        let mut decoder = DeflateReadDecoder::new(bytes);
        let mut output = Vec::new();
        decoder.read_to_end(&mut output).map_err(map_zlib_error)?;
        limit_output(output, options.max_output_length)
    })
}

pub fn unzip_sync(input: &tsonic_rust_js::Uint8Array) -> NodeResult<Buffer> {
    gunzip_sync(input).or_else(|_| inflate_sync(input))
}

pub fn gzip_string_sync(input: &str, encoding: &str) -> NodeResult<Buffer> {
    let bytes = Buffer::from_string(input, Some(encoding))?;
    gzip_sync(&bytes)
}

pub fn gunzip_string_sync(input: &Buffer, encoding: &str) -> NodeResult<String> {
    gunzip_sync(input)?.to_string(Some(encoding))
}

pub fn brotli_compress_sync(input: &tsonic_rust_js::Uint8Array) -> NodeResult<Buffer> {
    let mut output = Vec::new();
    {
        let mut writer = brotli::CompressorWriter::new(&mut output, 4096, 5, 22);
        input
            .with_bytes(|bytes| writer.write_all(bytes))
            .map_err(map_zlib_error)?;
    }
    Ok(Buffer::from_bytes(output))
}

pub fn brotli_decompress_sync(input: &tsonic_rust_js::Uint8Array) -> NodeResult<Buffer> {
    input.with_bytes(|bytes| {
        let mut decoder = brotli::Decompressor::new(bytes, 4096);
        let mut output = Vec::new();
        decoder.read_to_end(&mut output).map_err(map_zlib_error)?;
        Ok(Buffer::from_bytes(output))
    })
}

fn compression_from_level(level: i32) -> Compression {
    if level == constants::Z_DEFAULT_COMPRESSION {
        Compression::default()
    } else {
        Compression::new(level.clamp(0, 9) as u32)
    }
}

fn limit_output(output: Vec<u8>, max_output_length: Option<usize>) -> NodeResult<Buffer> {
    if max_output_length.is_some_and(|max| output.len() > max) {
        return Err(NodeError::new(
            "ERR_BUFFER_TOO_LARGE",
            "compressed output exceeds maxOutputLength",
        ));
    }
    Ok(Buffer::from_bytes(output))
}

fn map_zlib_error(error: std::io::Error) -> NodeError {
    NodeError::new("Z_DATA_ERROR", error.to_string())
}
