pub type Deflate<E = NodeError> = Zlib<E>;
pub type Inflate<E = NodeError> = Zlib<E>;
pub type Gzip<E = NodeError> = Zlib<E>;
pub type Gunzip<E = NodeError> = Zlib<E>;
pub type DeflateRaw<E = NodeError> = Zlib<E>;
pub type InflateRaw<E = NodeError> = Zlib<E>;
pub type Unzip<E = NodeError> = Zlib<E>;
pub type BrotliCompress<E = NodeError> = Zlib<E>;
pub type BrotliDecompress<E = NodeError> = Zlib<E>;

pub fn create_deflate<E: From<NodeError> + 'static>(options: Option<ZlibOptions>) -> Deflate<E> {
    Zlib::<E>::new(ZlibMode::Deflate, options)
}

pub fn create_inflate<E: From<NodeError> + 'static>(options: Option<ZlibOptions>) -> Inflate<E> {
    Zlib::<E>::new(ZlibMode::Inflate, options)
}

pub fn create_gzip<E: From<NodeError> + 'static>(options: Option<ZlibOptions>) -> Gzip<E> {
    Zlib::<E>::new(ZlibMode::Gzip, options)
}

pub fn create_gunzip<E: From<NodeError> + 'static>(options: Option<ZlibOptions>) -> Gunzip<E> {
    Zlib::<E>::new(ZlibMode::Gunzip, options)
}

pub fn create_deflate_raw<E: From<NodeError> + 'static>(
    options: Option<ZlibOptions>,
) -> DeflateRaw<E> {
    Zlib::<E>::new(ZlibMode::DeflateRaw, options)
}

pub fn create_inflate_raw<E: From<NodeError> + 'static>(
    options: Option<ZlibOptions>,
) -> InflateRaw<E> {
    Zlib::<E>::new(ZlibMode::InflateRaw, options)
}

pub fn create_unzip<E: From<NodeError> + 'static>(options: Option<ZlibOptions>) -> Unzip<E> {
    Zlib::<E>::new(ZlibMode::Unzip, options)
}

pub fn create_brotli_compress<E: From<NodeError> + 'static>(
    options: Option<BrotliOptions>,
) -> BrotliCompress<E> {
    let options = options.unwrap_or_default();
    Zlib::<E>::new(
        ZlibMode::BrotliCompress,
        Some(ZlibOptions {
            chunk_size: options.chunk_size,
            max_output_length: options.max_output_length,
            ..ZlibOptions::default()
        }),
    )
}

pub fn create_brotli_decompress<E: From<NodeError> + 'static>(
    options: Option<BrotliOptions>,
) -> BrotliDecompress<E> {
    let options = options.unwrap_or_default();
    Zlib::<E>::new(
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
