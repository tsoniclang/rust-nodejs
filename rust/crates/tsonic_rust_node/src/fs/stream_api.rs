pub fn create_read_stream(path: &str) -> NodeResult<ReadStream> {
    ReadStream::open(path, &ReadStreamOptions::default())
}

pub fn create_read_stream_with_options(
    path: &str,
    options: ReadStreamOptions,
) -> NodeResult<ReadStream> {
    ReadStream::open(path, &options)
}

pub fn create_write_stream(path: &str) -> NodeResult<WriteStream> {
    WriteStream::open(path, &WriteStreamOptions::default())
}

pub fn create_write_stream_with_options(
    path: &str,
    options: WriteStreamOptions,
) -> NodeResult<WriteStream> {
    WriteStream::open(path, &options)
}
