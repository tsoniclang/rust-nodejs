use crate::error::NodeError;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerCompletionError {
    first: NodeError,
    additional_unobserved_errors: u64,
}

impl WorkerCompletionError {
    pub fn first(&self) -> &NodeError {
        &self.first
    }

    pub fn additional_unobserved_errors(&self) -> u64 {
        self.additional_unobserved_errors
    }

    pub fn into_first(self) -> NodeError {
        self.first
    }

    pub(super) fn record_additional(&mut self) {
        self.additional_unobserved_errors = self
            .additional_unobserved_errors
            .checked_add(1)
            .expect("finite killed-worker input fits the native error counter");
    }
}

impl From<NodeError> for WorkerCompletionError {
    fn from(first: NodeError) -> Self {
        Self {
            first,
            additional_unobserved_errors: 0,
        }
    }
}

impl fmt::Display for WorkerCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.first, formatter)
    }
}

impl std::error::Error for WorkerCompletionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.first)
    }
}

impl From<WorkerCompletionError> for tsonic_rust_runtime::TsonicError {
    fn from(value: WorkerCompletionError) -> Self {
        Self::from(value.into_first())
    }
}

impl From<WorkerCompletionError> for tsonic_rust_runtime::RetainedError {
    fn from(value: WorkerCompletionError) -> Self {
        Self::from(tsonic_rust_runtime::TsonicError::from(value))
    }
}
