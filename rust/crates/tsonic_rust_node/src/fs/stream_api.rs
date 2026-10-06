pub fn create_read_stream<E: From<NodeError> + 'static>(
    background: &crate::background::BackgroundTasks<E>,
    path: &str,
) -> NodeResult<ReadStream<E>> {
    ReadStream::open(background, path, &ReadStreamOptions::default())
}

pub fn create_read_stream_with_options<E: From<NodeError> + 'static>(
    background: &crate::background::BackgroundTasks<E>,
    path: &str,
    options: ReadStreamOptions,
) -> NodeResult<ReadStream<E>> {
    ReadStream::open(background, path, &options)
}

pub fn create_write_stream<E: From<NodeError> + 'static>(
    background: &crate::background::BackgroundTasks<E>,
    path: &str,
) -> NodeResult<WriteStream<E>> {
    WriteStream::open(background, path, &WriteStreamOptions::default())
}

pub fn create_write_stream_with_options<E: From<NodeError> + 'static>(
    background: &crate::background::BackgroundTasks<E>,
    path: &str,
    options: WriteStreamOptions,
) -> NodeResult<WriteStream<E>> {
    WriteStream::open(background, path, &options)
}
