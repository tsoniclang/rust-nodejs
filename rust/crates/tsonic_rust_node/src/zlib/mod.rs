pub mod constants;
mod source_abi;

pub use source_abi::{SourceZlibOptions, gzip_sync_source, gunzip_sync_source, deflate_sync_source, inflate_sync_source, deflate_raw_sync_source, inflate_raw_sync_source, create_gzip_source, create_deflate_source, create_inflate_source, create_gunzip_source, create_deflate_raw_source, create_inflate_raw_source, gzip_callable, gunzip_callable, deflate_callable, inflate_callable, gzip_options_callable, gunzip_options_callable, deflate_options_callable, inflate_options_callable};

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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BrotliOptions {
    pub flush: Option<i32>,
    pub finish_flush: Option<i32>,
    pub chunk_size: usize,
    pub params: std::collections::BTreeMap<i32, i32>,
    pub max_output_length: Option<usize>,
    pub info: bool,
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

const MAX_PENDING_ZLIB_OUTPUT: usize = 256 * 1024 * 1024;

#[derive(Debug)]
pub struct Zlib {
    mode: ZlibMode,
    bytes_written: usize,
    closed: bool,
    options: ZlibOptions,
    output: std::rc::Rc<std::cell::RefCell<ZlibOutput>>,
    codec: Option<StreamingCodec>,
}

#[derive(Debug, Default)]
struct ZlibOutput {
    pending: std::collections::VecDeque<Buffer>,
    pending_bytes: usize,
}

#[derive(Debug, Clone)]
struct ZlibSink {
    output: std::rc::Rc<std::cell::RefCell<ZlibOutput>>,
}

impl Write for ZlibSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let mut output = self.output.borrow_mut();
        if output.pending_bytes.saturating_add(bytes.len()) > MAX_PENDING_ZLIB_OUTPUT {
            return Err(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                "pending zlib output exceeds the finite runtime limit",
            ));
        }
        if !bytes.is_empty() {
            output.pending.push_back(Buffer::from_bytes(bytes.to_vec()));
            output.pending_bytes += bytes.len();
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
enum StreamingCodec {
    Deflate(ZlibWriteEncoder<ZlibSink>),
    Inflate(ZlibWriteDecoder<ZlibSink>),
    Gzip(GzWriteEncoder<ZlibSink>),
    Gunzip(GzWriteDecoder<ZlibSink>),
    DeflateRaw(DeflateWriteEncoder<ZlibSink>),
    InflateRaw(DeflateWriteDecoder<ZlibSink>),
}

impl Zlib {
    pub fn new(mode: ZlibMode, options: Option<ZlibOptions>) -> Self {
        let options = options.unwrap_or_default();
        let output = std::rc::Rc::new(std::cell::RefCell::new(ZlibOutput::default()));
        let sink = ZlibSink {
            output: output.clone(),
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
            ZlibMode::Unzip | ZlibMode::BrotliCompress | ZlibMode::BrotliDecompress => None,
        };
        Self {
            mode,
            bytes_written: 0,
            closed: false,
            options,
            output,
            codec,
        }
    }

    pub fn bytes_written(&self) -> usize {
        self.bytes_written
    }

    pub fn close(&mut self, callback: Option<impl FnOnce()>) {
        self.closed = true;
        if let Some(callback) = callback {
            callback();
        }
    }

    pub fn closed(&self) -> bool {
        self.closed
    }

    pub fn reset(&mut self) {
        self.bytes_written = 0;
        self.closed = false;
        let mode = self.mode;
        let options = self.options.clone();
        *self = Self::new(mode, Some(options));
    }

    pub fn flush(&self, callback: Option<impl FnOnce()>) {
        if let Some(callback) = callback {
            callback();
        }
    }

    pub fn params(&mut self, level: i32, strategy: i32, callback: impl FnOnce()) {
        self.options.level = level;
        self.options.strategy = strategy;
        callback();
    }

    pub fn process(&mut self, input: &Buffer) -> NodeResult<Buffer> {
        self.bytes_written += input.len();
        match self.mode {
            ZlibMode::Deflate => deflate_sync_with_options(input, &self.options),
            ZlibMode::Inflate => inflate_sync(input),
            ZlibMode::Gzip => gzip_sync_with_options(input, &self.options),
            ZlibMode::Gunzip => gunzip_sync(input),
            ZlibMode::DeflateRaw => deflate_raw_sync(input),
            ZlibMode::InflateRaw => inflate_raw_sync(input),
            ZlibMode::Unzip => unzip_sync(input),
            ZlibMode::BrotliCompress => brotli_compress_sync(input),
            ZlibMode::BrotliDecompress => brotli_decompress_sync(input),
        }
    }

    pub fn write(&mut self, input: Buffer) -> NodeResult<bool> {
        if self.closed {
            return Err(NodeError::new(
                "ERR_STREAM_WRITE_AFTER_END",
                "zlib stream is closed",
            ));
        }
        self.bytes_written = self.bytes_written.saturating_add(input.len());
        let codec = self.codec.as_mut().ok_or_else(|| {
            NodeError::new(
                "ERR_UNSUPPORTED_OPERATION",
                "the selected compression mode has no streaming implementation",
            )
        })?;
        codec.write_all(&input.as_bytes()).map_err(map_zlib_error)?;
        codec.flush().map_err(map_zlib_error)?;
        Ok(self.output.borrow().pending_bytes < self.options.chunk_size.max(1))
    }

    pub fn read(&mut self) -> Option<Buffer> {
        let mut state = self.output.borrow_mut();
        let output = state.pending.pop_front();
        if let Some(output) = &output {
            state.pending_bytes = state.pending_bytes.saturating_sub(output.len());
        }
        output
    }

    pub fn end(&mut self) -> NodeResult<()> {
        if let Some(codec) = self.codec.as_mut() {
            codec.finish().map_err(map_zlib_error)?;
        }
        self.closed = true;
        Ok(())
    }
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
        }
    }
}

impl StreamingCodec {
    fn finish(&mut self) -> std::io::Result<()> {
        match self {
            Self::Deflate(codec) => codec.try_finish(),
            Self::Inflate(codec) => codec.try_finish(),
            Self::Gzip(codec) => codec.try_finish(),
            Self::Gunzip(codec) => codec.try_finish(),
            Self::DeflateRaw(codec) => codec.try_finish(),
            Self::InflateRaw(codec) => codec.try_finish(),
        }
    }
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

pub fn create_brotli_compress(_options: Option<BrotliOptions>) -> BrotliCompress {
    Zlib::new(ZlibMode::BrotliCompress, None)
}

pub fn create_brotli_decompress(_options: Option<BrotliOptions>) -> BrotliDecompress {
    Zlib::new(ZlibMode::BrotliDecompress, None)
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

#[allow(non_upper_case_globals)]
