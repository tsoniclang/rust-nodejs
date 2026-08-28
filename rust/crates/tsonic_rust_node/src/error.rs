use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeError {
    pub code: String,
    pub message: String,
}

impl NodeError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for NodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for NodeError {}

pub type NodeResult<T> = Result<T, NodeError>;

impl From<NodeError> for tsonic_rust_runtime::TsonicError {
    fn from(value: NodeError) -> Self {
        tsonic_rust_runtime::TsonicError::Node {
            code: value.code,
            message: value.message,
        }
    }
}

pub(crate) fn callback_runtime_error(error: impl fmt::Display) -> tsonic_rust_runtime::TsonicError {
    tsonic_rust_runtime::TsonicError::Node {
        code: "ERR_TSONIC_CALLBACK".to_string(),
        message: error.to_string(),
    }
}

pub(crate) fn callback_node_error(error: impl fmt::Display) -> NodeError {
    NodeError::new("ERR_TSONIC_CALLBACK", error.to_string())
}
