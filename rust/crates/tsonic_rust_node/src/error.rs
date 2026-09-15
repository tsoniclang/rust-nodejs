use std::fmt;
use tsonic_rust_runtime::{JsError, JsErrorKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeError {
    pub code: String,
    source: JsError,
}

impl NodeError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            source: JsError::new(JsErrorKind::Error, message),
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn message(&self) -> &str {
        self.source.message()
    }

    pub fn source_error(&self) -> &JsError {
        &self.source
    }
}

impl fmt::Display for NodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message())
    }
}

impl std::error::Error for NodeError {}

pub type NodeResult<T> = Result<T, NodeError>;

impl From<NodeError> for tsonic_rust_runtime::TsonicError {
    fn from(value: NodeError) -> Self {
        tsonic_rust_runtime::TsonicError::Node {
            code: value.code,
            source: value.source,
        }
    }
}

pub(crate) fn callback_runtime_error(error: impl fmt::Display) -> tsonic_rust_runtime::TsonicError {
    NodeError::new("ERR_TSONIC_CALLBACK", error.to_string()).into()
}

pub(crate) fn callback_node_error(error: impl fmt::Display) -> NodeError {
    NodeError::new("ERR_TSONIC_CALLBACK", error.to_string())
}
