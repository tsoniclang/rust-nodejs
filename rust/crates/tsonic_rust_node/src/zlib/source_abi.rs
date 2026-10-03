use super::{
    create_brotli_compress, create_brotli_decompress, create_deflate, create_deflate_raw,
    create_gunzip, create_gzip, create_inflate, create_inflate_raw, deflate_raw_sync_with_options,
    deflate_sync, deflate_sync_with_options, gunzip_sync, gunzip_sync_with_options, gzip_sync,
    gzip_sync_with_options, inflate_raw_sync_with_options, inflate_sync, inflate_sync_with_options,
    BackgroundZlibOptions, BrotliCompress, BrotliDecompress, BrotliOptions, Deflate, DeflateRaw,
    Gunzip, Gzip, Inflate, InflateRaw, ZlibOptions,
};
use crate::buffer::Buffer;
use crate::error::{NodeError, NodeResult};

type CompressionCallback<Failure> =
    tsonic_rust_runtime::Callable<(Option<NodeError>, Option<Buffer>), Result<(), Failure>>;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SourceZlibOptions {
    pub flush: Option<i32>,
    pub finish_flush: Option<i32>,
    pub chunk_size: Option<usize>,
    pub window_bits: Option<i32>,
    pub level: Option<i32>,
    pub mem_level: Option<i32>,
    pub strategy: Option<i32>,
    pub max_output_length: Option<usize>,
    pub dictionary: Option<Buffer>,
    pub info: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SourceBrotliOptions {
    pub chunk_size: Option<usize>,
    pub max_output_length: Option<usize>,
}

impl SourceBrotliOptions {
    fn into_runtime(self) -> NodeResult<BrotliOptions> {
        let defaults = BrotliOptions::default();
        let chunk_size = self.chunk_size.unwrap_or(defaults.chunk_size);
        if chunk_size == 0 {
            return Err(NodeError::new(
                "ERR_OUT_OF_RANGE",
                "chunkSize must be a positive native integer",
            ));
        }
        if self.max_output_length == Some(0) {
            return Err(NodeError::new(
                "ERR_OUT_OF_RANGE",
                "maxOutputLength must be a positive native integer",
            ));
        }
        Ok(BrotliOptions {
            chunk_size,
            max_output_length: self.max_output_length,
            ..defaults
        })
    }
}

impl SourceZlibOptions {
    fn into_runtime(self) -> NodeResult<ZlibOptions> {
        let defaults = ZlibOptions::default();
        let chunk_size = self.chunk_size.unwrap_or(defaults.chunk_size);
        if chunk_size == 0 {
            return Err(NodeError::new(
                "ERR_OUT_OF_RANGE",
                "chunkSize must be a positive native integer",
            ));
        }
        let level = self.level.unwrap_or(defaults.level);
        if !(-1..=9).contains(&level) {
            return Err(NodeError::new(
                "ERR_OUT_OF_RANGE",
                "level must be between -1 and 9",
            ));
        }
        if self.max_output_length == Some(0) {
            return Err(NodeError::new(
                "ERR_OUT_OF_RANGE",
                "maxOutputLength must be a positive native integer",
            ));
        }
        Ok(ZlibOptions {
            flush: self.flush,
            finish_flush: self.finish_flush,
            chunk_size,
            window_bits: self.window_bits,
            level,
            mem_level: self.mem_level,
            strategy: self.strategy.unwrap_or(defaults.strategy),
            max_output_length: self.max_output_length,
            dictionary: self.dictionary,
            info: self.info.unwrap_or(false),
        })
    }
}

pub fn gzip_sync_source(
    input: &tsonic_rust_js::Uint8Array,
    options: SourceZlibOptions,
) -> NodeResult<Buffer> {
    gzip_sync_with_options(input, &options.into_runtime()?)
}

pub fn gunzip_sync_source(
    input: &tsonic_rust_js::Uint8Array,
    options: SourceZlibOptions,
) -> NodeResult<Buffer> {
    gunzip_sync_with_options(input, &options.into_runtime()?)
}

pub fn deflate_sync_source(
    input: &tsonic_rust_js::Uint8Array,
    options: SourceZlibOptions,
) -> NodeResult<Buffer> {
    deflate_sync_with_options(input, &options.into_runtime()?)
}

pub fn inflate_sync_source(
    input: &tsonic_rust_js::Uint8Array,
    options: SourceZlibOptions,
) -> NodeResult<Buffer> {
    inflate_sync_with_options(input, &options.into_runtime()?)
}

pub fn deflate_raw_sync_source(
    input: &tsonic_rust_js::Uint8Array,
    options: SourceZlibOptions,
) -> NodeResult<Buffer> {
    deflate_raw_sync_with_options(input, &options.into_runtime()?)
}

pub fn inflate_raw_sync_source(
    input: &tsonic_rust_js::Uint8Array,
    options: SourceZlibOptions,
) -> NodeResult<Buffer> {
    inflate_raw_sync_with_options(input, &options.into_runtime()?)
}

pub fn create_gzip_source(options: SourceZlibOptions) -> NodeResult<Gzip> {
    Ok(create_gzip(Some(options.into_runtime()?)))
}

pub fn create_deflate_source(options: SourceZlibOptions) -> NodeResult<Deflate> {
    Ok(create_deflate(Some(options.into_runtime()?)))
}

pub fn create_inflate_source(options: SourceZlibOptions) -> NodeResult<Inflate> {
    Ok(create_inflate(Some(options.into_runtime()?)))
}

pub fn create_gunzip_source(options: SourceZlibOptions) -> NodeResult<Gunzip> {
    Ok(create_gunzip(Some(options.into_runtime()?)))
}

pub fn create_deflate_raw_source(options: SourceZlibOptions) -> NodeResult<DeflateRaw> {
    Ok(create_deflate_raw(Some(options.into_runtime()?)))
}

pub fn create_inflate_raw_source(options: SourceZlibOptions) -> NodeResult<InflateRaw> {
    Ok(create_inflate_raw(Some(options.into_runtime()?)))
}

pub fn create_brotli_compress_source(options: SourceBrotliOptions) -> NodeResult<BrotliCompress> {
    Ok(create_brotli_compress(Some(options.into_runtime()?)))
}

pub fn create_brotli_decompress_source(
    options: SourceBrotliOptions,
) -> NodeResult<BrotliDecompress> {
    Ok(create_brotli_decompress(Some(options.into_runtime()?)))
}

pub fn gzip_callable<E>(input: &Buffer, callback: CompressionCallback<E>) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    compress_callable(input, callback, gzip_sync)
}

pub fn gunzip_callable<E>(input: &Buffer, callback: CompressionCallback<E>) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    compress_callable(input, callback, gunzip_sync)
}

pub fn deflate_callable<E>(input: &Buffer, callback: CompressionCallback<E>) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    compress_callable(input, callback, deflate_sync)
}

pub fn inflate_callable<E>(input: &Buffer, callback: CompressionCallback<E>) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    compress_callable(input, callback, inflate_sync)
}

pub fn gzip_options_callable<E>(
    input: &Buffer,
    options: SourceZlibOptions,
    callback: CompressionCallback<E>,
) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    compress_options_callable(input, options, callback, gzip_sync_with_options)
}

pub fn gunzip_options_callable<E>(
    input: &Buffer,
    options: SourceZlibOptions,
    callback: CompressionCallback<E>,
) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    compress_options_callable(input, options, callback, gunzip_sync_with_options)
}

pub fn deflate_options_callable<E>(
    input: &Buffer,
    options: SourceZlibOptions,
    callback: CompressionCallback<E>,
) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    compress_options_callable(input, options, callback, deflate_sync_with_options)
}

pub fn inflate_options_callable<E>(
    input: &Buffer,
    options: SourceZlibOptions,
    callback: CompressionCallback<E>,
) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    compress_options_callable(input, options, callback, inflate_sync_with_options)
}

fn compress_callable<E>(
    input: &Buffer,
    callback: CompressionCallback<E>,
    compress: fn(&tsonic_rust_js::Uint8Array) -> NodeResult<Buffer>,
) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    let input = input.as_bytes();
    crate::background::spawn(
        move || {
            let input = Buffer::from_bytes(input);
            compress(&input).map(|output| output.as_bytes())
        },
        move |result| complete_compression(result, callback),
    )
}

fn compress_options_callable<E>(
    input: &Buffer,
    options: SourceZlibOptions,
    callback: CompressionCallback<E>,
    compress: fn(&tsonic_rust_js::Uint8Array, &ZlibOptions) -> NodeResult<Buffer>,
) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    let input = input.as_bytes();
    let options = BackgroundZlibOptions::from(options.into_runtime()?);
    crate::background::spawn(
        move || {
            let input = Buffer::from_bytes(input);
            let options = options.into_runtime();
            compress(&input, &options).map(|output| output.as_bytes())
        },
        move |result| complete_compression(result, callback),
    )
}

fn complete_compression<Failure: std::fmt::Display>(
    result: NodeResult<Vec<u8>>,
    callback: CompressionCallback<Failure>,
) -> tsonic_rust_runtime::TsonicResult<()> {
    let arguments = match result {
        Ok(output) => (None, Some(Buffer::from_bytes(output))),
        Err(error) => (Some(error), None),
    };
    callback
        .call(arguments)
        .map_err(crate::error::callback_runtime_error)
}

#[cfg(test)]
mod compression_completion {
    use super::{complete_compression, Buffer, NodeError};
    use std::cell::Cell;
    use std::rc::Rc;
    use tsonic_rust_runtime::Callable;

    #[test]
    fn failure_moves_the_original_error_without_a_placeholder_buffer() {
        let error = NodeError::new("compression-code", "native error message");
        let code_address = error.code().as_ptr() as usize;
        let source = error.source_error().clone();
        let observed = Rc::new(Cell::new(false));
        let callback_observed = Rc::clone(&observed);
        let callback = Callable::new(
            move |(error, output): (Option<NodeError>, Option<Buffer>)| {
                let error = error.expect("original native error");
                assert_eq!(error.code().as_ptr() as usize, code_address);
                assert_eq!(error.source_error(), &source);
                assert!(output.is_none());
                callback_observed.set(true);
                Ok::<(), String>(())
            },
        );

        complete_compression(Err(error), callback).unwrap();
        assert!(observed.get());
    }
}

#[cfg(test)]
mod numeric_bounds {
    #[test]
    fn size_retains_the_complete_native_domain() {
        let options = super::SourceZlibOptions {
            max_output_length: Some(usize::MAX),
            ..Default::default()
        }
        .into_runtime()
        .unwrap();
        assert_eq!(options.max_output_length, Some(usize::MAX));
    }

    #[test]
    fn zero_sized_buffers_and_invalid_levels_are_rejected() {
        for options in [
            super::SourceZlibOptions {
                chunk_size: Some(0),
                ..Default::default()
            },
            super::SourceZlibOptions {
                max_output_length: Some(0),
                ..Default::default()
            },
            super::SourceZlibOptions {
                level: Some(10),
                ..Default::default()
            },
        ] {
            assert!(options.into_runtime().is_err());
        }
    }
}
