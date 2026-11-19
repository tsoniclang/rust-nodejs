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
pub enum Stream<E: 'static = NodeError> {
    Readable(Readable<E>),
    Writable(Writable<E>),
}

pub fn readable_as_stream<E: 'static>(value: &Readable<E>) -> Stream<E> {
    Stream::<E>::Readable(value.clone())
}

pub fn writable_as_stream<E: 'static>(value: &Writable<E>) -> Stream<E> {
    Stream::<E>::Writable(value.clone())
}

impl<E: 'static> Clone for Stream<E> {
    fn clone(&self) -> Self {
        match self {
            Self::Readable(readable) => Self::Readable(readable.clone()),
            Self::Writable(writable) => Self::Writable(writable.clone()),
        }
    }
}
impl<E: 'static> std::fmt::Debug for Stream<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Readable(value) => formatter.debug_tuple("Readable").field(value).finish(),
            Self::Writable(value) => formatter.debug_tuple("Writable").field(value).finish(),
        }
    }
}
impl<E: 'static> PartialEq for Stream<E> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Readable(left), Self::Readable(right)) => left == right,
            (Self::Writable(left), Self::Writable(right)) => left == right,
            _ => false,
        }
    }
}
impl<E: 'static> Eq for Stream<E> {}
