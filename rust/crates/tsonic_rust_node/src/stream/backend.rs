pub(crate) enum StreamBackendFailure<E> {
    Native(NodeError),
    Callback(E),
}

impl<E> From<NodeError> for StreamBackendFailure<E> {
    fn from(error: NodeError) -> Self {
        Self::Native(error)
    }
}

pub(crate) type StreamBackendResult<T, E> = Result<T, StreamBackendFailure<E>>;
