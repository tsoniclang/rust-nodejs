use crate::buffer::Buffer;
use crate::error::NodeResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamOptions {
    pub high_water_mark: usize,
    pub object_mode: bool,
    pub emit_close: bool,
    pub auto_destroy: bool,
    pub allow_half_open: bool,
    pub default_encoding: String,
}

impl Default for StreamOptions {
    fn default() -> Self {
        Self {
            high_water_mark: 16 * 1024,
            object_mode: false,
            emit_close: true,
            auto_destroy: true,
            allow_half_open: false,
            default_encoding: "utf8".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedOptions {
    pub error: bool,
    pub readable: bool,
    pub writable: bool,
    pub cleanup: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abortable {
    pub signal_aborted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadableOperatorOptions {
    pub high_water_mark: Option<usize>,
    pub concurrency: Option<usize>,
    pub signal_aborted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadableIteratorOptions {
    pub destroy_on_return: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeOptions {
    pub end: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadableOptions {
    pub stream: StreamOptions,
    pub encoding: Option<String>,
    pub signal_aborted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritableOptions {
    pub stream: StreamOptions,
    pub decode_strings: bool,
    pub signal_aborted: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DuplexOptions {
    pub stream: StreamOptions,
    pub readable_high_water_mark: Option<usize>,
    pub writable_high_water_mark: Option<usize>,
    pub readable_object_mode: bool,
    pub writable_object_mode: bool,
    pub allow_half_open: bool,
    pub writable_corked: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformOptions {
    pub stream: StreamOptions,
    pub readable_object_mode: bool,
    pub writable_object_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadableToWebOptions {
    pub r#type: Option<String>,
    pub high_water_mark: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritableToWebOptions {
    pub high_water_mark: Option<usize>,
}

impl Default for FinishedOptions {
    fn default() -> Self {
        Self {
            error: true,
            readable: true,
            writable: true,
            cleanup: false,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stream {
    Readable(Readable),
    Writable(Writable),
}

pub fn readable_as_stream(value: &Readable) -> Stream {
    Stream::Readable(value.clone())
}

pub fn writable_as_stream(value: &Writable) -> Stream {
    Stream::Writable(value.clone())
}
