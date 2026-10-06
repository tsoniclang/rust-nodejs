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

impl tsonic_rust_runtime::ErrorObject for NodeError {
    fn error_name(&self) -> tsonic_rust_runtime::ErrorField<'_> {
        self.source.error_name()
    }

    fn error_message(&self) -> tsonic_rust_runtime::ErrorField<'_> {
        self.source.error_message()
    }

    fn error_stack(&self) -> Option<tsonic_rust_runtime::ErrorField<'_>> {
        self.source.error_stack()
    }

    fn error_kind(&self) -> JsErrorKind {
        self.source.error_kind()
    }

    fn error_identity_key(&self) -> usize {
        self.source.error_identity_key()
    }
}

impl tsonic_rust_runtime::ErrorStack for NodeError {
    fn set_stack(&self, value: Option<String>) {
        tsonic_rust_runtime::ErrorStack::set_stack(&self.source, value);
    }
}

impl tsonic_rust_runtime::ToSourceString for NodeError {
    fn to_source_string(&self) -> String {
        self.to_string()
    }
}

pub type NodeResult<T> = Result<T, NodeError>;

impl From<tsonic_rust_runtime::dispatch_queue::TaskQueueError> for NodeError {
    fn from(value: tsonic_rust_runtime::dispatch_queue::TaskQueueError) -> Self {
        use tsonic_rust_runtime::dispatch_queue::TaskQueueError;
        let code = match value {
            TaskQueueError::Capacity => "ERR_NODE_RUNTIME_TASK_LIMIT",
            TaskQueueError::TicketExhausted => "ERR_NODE_RUNTIME_TASK_TICKET_LIMIT",
            TaskQueueError::Closed => "ERR_NODE_RUNTIME_TASK_CLOSED",
        };
        Self::new(code, value.to_string())
    }
}

impl From<NodeError> for tsonic_rust_runtime::TsonicError {
    fn from(value: NodeError) -> Self {
        tsonic_rust_runtime::TsonicError::Node {
            code: value.code,
            source: value.source,
        }
    }
}

impl From<NodeError> for tsonic_rust_runtime::RetainedError {
    fn from(value: NodeError) -> Self {
        Self::from(tsonic_rust_runtime::TsonicError::from(value))
    }
}

pub(crate) fn callback_runtime_error(error: impl fmt::Display) -> tsonic_rust_runtime::TsonicError {
    NodeError::new("ERR_TSONIC_CALLBACK", error.to_string()).into()
}

pub(crate) fn callback_node_error(error: impl fmt::Display) -> NodeError {
    NodeError::new("ERR_TSONIC_CALLBACK", error.to_string())
}
