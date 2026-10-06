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
