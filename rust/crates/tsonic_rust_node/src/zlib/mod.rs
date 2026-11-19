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

include!("options.rs");
include!("streaming.rs");
include!("operations.rs");

#[cfg(test)]
mod retained_tests;
