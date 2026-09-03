#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Utf8Stream {
    pub file: String,
    pub fd: i32,
    pub min_length: usize,
    pub max_length: usize,
    pub content_mode: String,
    pub writing: bool,
    pub append: bool,
    pub sync: bool,
    pub periodic_flush: Option<u64>,
    pub fsync: bool,
    pub mkdir: bool,
    pub mode: u32,
    buffer: Vec<u8>,
    destroyed: bool,
}

impl Utf8Stream {
    pub fn new(options: Utf8StreamOptions) -> NodeResult<Self> {
        if let Some(dest) = &options.dest {
            if options.mkdir {
                if let Some(parent) = std::path::Path::new(dest).parent() {
                    if !parent.as_os_str().is_empty() {
                        fs::create_dir_all(parent).map_err(map_io_error)?;
                    }
                }
            }
        }

        Ok(Self {
            file: options.dest.unwrap_or_default(),
            fd: options.fd.unwrap_or(-1),
            min_length: options.min_length,
            max_length: options.max_length,
            content_mode: options.content_mode,
            writing: false,
            append: options.append,
            sync: options.sync,
            periodic_flush: options.periodic_flush_ms,
            fsync: options.fsync,
            mkdir: options.mkdir,
            mode: options.mode,
            buffer: Vec::new(),
            destroyed: false,
        })
    }

    pub fn reopen(&mut self, file: &str) -> NodeResult<()> {
        self.flush_sync()?;
        self.file = file.to_string();
        Ok(())
    }

    pub fn write(&mut self, data: FsWriteData<'_>) -> bool {
        if self.destroyed {
            return false;
        }
        match data {
            FsWriteData::String(value) => self.write_bytes(value.as_bytes()),
            FsWriteData::Buffer(value) => value.with_bytes(|bytes| self.write_bytes(bytes)),
            FsWriteData::Bytes(value) => self.write_bytes(value),
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> bool {
        if self.buffer.len().saturating_add(bytes.len()) > self.max_length {
            return false;
        }
        self.writing = true;
        self.buffer.extend_from_slice(bytes);
        if self.sync || self.buffer.len() >= self.min_length {
            self.flush_sync().is_ok()
        } else {
            true
        }
    }

    pub fn flush(&mut self, callback: impl FnOnce(NodeResult<()>)) {
        callback(self.flush_sync());
    }

    pub fn flush_sync(&mut self) -> NodeResult<()> {
        if self.buffer.is_empty() {
            self.writing = false;
            return Ok(());
        }
        if self.file.is_empty() {
            self.buffer.clear();
            self.writing = false;
            return Ok(());
        }
        let mut options = OpenOptions::new();
        options.create(true).write(true);
        if self.append {
            options.append(true);
        } else {
            options.truncate(true);
        }
        let mut file = options.open(&self.file).map_err(map_io_error)?;
        file.write_all(&self.buffer).map_err(map_io_error)?;
        if self.fsync {
            file.sync_all().map_err(map_io_error)?;
        }
        self.buffer.clear();
        self.writing = false;
        Ok(())
    }

    pub fn end(&mut self) -> NodeResult<()> {
        self.flush_sync()?;
        self.destroyed = true;
        Ok(())
    }

    pub fn destroy(&mut self) {
        self.buffer.clear();
        self.writing = false;
        self.destroyed = true;
    }
}

#[derive(Debug)]
pub struct ReadStream {
    pub path: String,
    pub pending: bool,
    pub bytes_read: usize,
    file: Option<File>,
    chunk_size: usize,
    remaining: Option<u64>,
    closed: bool,
    events: StreamEventState,
}

impl ReadStream {
    pub fn open(path: &str, options: &ReadStreamOptions) -> NodeResult<Self> {
        let mut open = OpenOptions::new();
        match options.flags.as_deref().unwrap_or("r") {
            "r" => {
                open.read(true);
            }
            "r+" | "rs+" => {
                open.read(true).write(true);
            }
            flag => return Err(invalid_stream_option("flags", flag)),
        }
        if let Some(mode) = options.mode {
            apply_open_mode(&mut open, mode)?;
        }
        let file = open.open(path).map_err(map_io_error)?;
        Self::from_file(path.to_string(), file, options)
    }

    pub(crate) fn from_file(
        path: String,
        mut file: File,
        options: &ReadStreamOptions,
    ) -> NodeResult<Self> {
        use std::io::{Seek, SeekFrom};

        let start = optional_non_negative_integer(options.start, "start")?.unwrap_or(0);
        let end = optional_non_negative_integer(options.end, "end")?;
        if end.is_some_and(|end| end < start) {
            return Err(NodeError::new("ERR_OUT_OF_RANGE", "end must be greater than or equal to start"));
        }
        file.seek(SeekFrom::Start(start)).map_err(map_io_error)?;
        let remaining = end.map(|end| end - start + 1);
        let chunk_size = optional_positive_usize(options.high_water_mark, "highWaterMark")?
            .unwrap_or(64 * 1024);
        Ok(Self {
            path,
            pending: false,
            bytes_read: 0,
            file: Some(file),
            chunk_size,
            remaining,
            closed: false,
            events: StreamEventState::default(),
        })
    }

    pub fn read(&mut self) -> NodeResult<Option<Buffer>> {
        if self.closed {
            return Ok(None);
        }
        let maximum = self
            .remaining
            .map(|remaining| remaining.min(self.chunk_size as u64) as usize)
            .unwrap_or(self.chunk_size);
        if maximum == 0 {
            self.close();
            return Ok(None);
        }
        let mut bytes = vec![0; maximum];
        let read = self
            .file
            .as_mut()
            .ok_or_else(|| NodeError::new("ERR_STREAM_CLOSED", "read stream is closed"))?
            .read(&mut bytes)
            .map_err(map_io_error)?;
        if read == 0 {
            self.close();
            return Ok(None);
        }
        bytes.truncate(read);
        self.bytes_read = self.bytes_read.saturating_add(read);
        if let Some(remaining) = &mut self.remaining {
            *remaining = remaining.saturating_sub(read as u64);
        }
        Ok(Some(Buffer::from_bytes(bytes)))
    }

    pub fn bytes_read_number(&self) -> f64 {
        self.bytes_read as f64
    }

    pub fn pipe_to<'a, W: crate::stream::WritableTarget>(
        &mut self,
        writable: &'a mut W,
    ) -> NodeResult<&'a mut W> {
        crate::stream::pipe_chunks(|| self.read(), writable)?;
        Ok(writable)
    }

    pub fn close(&mut self) {
        self.file = None;
        self.pending = false;
        self.closed = true;
    }

    pub fn closed(&self) -> bool {
        self.closed
    }

    pub fn text(&mut self, encoding: Option<&str>) -> NodeResult<String> {
        let mut bytes = Vec::new();
        while let Some(chunk) = self.read()? {
            chunk.with_bytes(|chunk_bytes| bytes.extend_from_slice(chunk_bytes));
        }
        crate::buffer::decode_bytes(&bytes, encoding)
    }

    pub fn add_listener(&mut self, event: &str) -> &mut Self {
        self.events.add_listener(event);
        self
    }

    pub fn on(&mut self, event: &str) -> &mut Self {
        self.add_listener(event)
    }

    pub fn once(&mut self, event: &str) -> &mut Self {
        self.events.add_listener(event);
        self
    }

    pub fn prepend_listener(&mut self, event: &str) -> &mut Self {
        self.events.add_listener(event);
        self
    }

    pub fn prepend_once_listener(&mut self, event: &str) -> &mut Self {
        self.events.add_listener(event);
        self
    }

    pub fn remove_listener(&mut self, event: &str) -> &mut Self {
        self.events.remove_listener(event);
        self
    }

    pub fn off(&mut self, event: &str) -> &mut Self {
        self.events.remove_listener(event);
        self
    }

    pub fn remove_all_listeners(&mut self, event: Option<&str>) -> &mut Self {
        self.events.remove_all_listeners(event);
        self
    }

    pub fn listeners(&self, event: &str) -> Vec<String> {
        self.events.listeners(event)
    }

    pub fn raw_listeners(&self, event: &str) -> Vec<String> {
        self.listeners(event)
    }

    pub fn listener_count(&self, event: &str) -> usize {
        self.events.listener_count(event)
    }

    pub fn emit(&self, event: &str) -> bool {
        self.events.emit(event)
    }
}

#[derive(Debug)]
pub struct WriteStream {
    pub path: String,
    pub pending: bool,
    pub bytes_written: usize,
    file: Option<File>,
    flush_each_write: bool,
    closed: bool,
    events: StreamEventState,
}

impl WriteStream {
    pub fn open(path: &str, options: &WriteStreamOptions) -> NodeResult<Self> {
        let mut open = OpenOptions::new();
        configure_write_stream_open(&mut open, options.flags.as_deref().unwrap_or("w"))?;
        if let Some(mode) = options.mode {
            apply_open_mode(&mut open, mode)?;
        }
        let file = open.open(path).map_err(map_io_error)?;
        Self::from_file(path.to_string(), file, options)
    }

    pub(crate) fn from_file(
        path: String,
        mut file: File,
        options: &WriteStreamOptions,
    ) -> NodeResult<Self> {
        use std::io::{Seek, SeekFrom};

        if let Some(start) = optional_non_negative_integer(options.start, "start")? {
            file.seek(SeekFrom::Start(start)).map_err(map_io_error)?;
        }
        Ok(Self {
            path,
            pending: false,
            bytes_written: 0,
            file: Some(file),
            flush_each_write: options.flush.unwrap_or(false),
            closed: false,
            events: StreamEventState::default(),
        })
    }

    pub fn write(&mut self, chunk: Buffer) -> NodeResult<bool> {
        if self.closed {
            return Err(NodeError::new("ERR_STREAM_WRITE_AFTER_END", "write stream is closed"));
        }
        let len = chunk.len();
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| NodeError::new("ERR_STREAM_CLOSED", "write stream is closed"))?;
        chunk.with_bytes(|bytes| file.write_all(bytes).map_err(map_io_error))?;
        if self.flush_each_write {
            file.flush().map_err(map_io_error)?;
        }
        self.bytes_written = self.bytes_written.saturating_add(len);
        Ok(true)
    }

    pub fn bytes_written_number(&self) -> f64 {
        self.bytes_written as f64
    }

    pub fn flush(&mut self) -> NodeResult<()> {
        self.file
            .as_mut()
            .ok_or_else(|| NodeError::new("ERR_STREAM_CLOSED", "write stream is closed"))?
            .flush()
            .map_err(map_io_error)
    }

    pub fn close(&mut self) -> NodeResult<()> {
        if let Some(mut file) = self.file.take() {
            file.flush().map_err(map_io_error)?;
        }
        self.pending = false;
        self.closed = true;
        Ok(())
    }

    pub fn closed(&self) -> bool {
        self.closed
    }

    pub fn add_listener(&mut self, event: &str) -> &mut Self {
        self.events.add_listener(event);
        self
    }

    pub fn on(&mut self, event: &str) -> &mut Self {
        self.add_listener(event)
    }

    pub fn once(&mut self, event: &str) -> &mut Self {
        self.events.add_listener(event);
        self
    }

    pub fn prepend_listener(&mut self, event: &str) -> &mut Self {
        self.events.add_listener(event);
        self
    }

    pub fn prepend_once_listener(&mut self, event: &str) -> &mut Self {
        self.events.add_listener(event);
        self
    }

    pub fn remove_listener(&mut self, event: &str) -> &mut Self {
        self.events.remove_listener(event);
        self
    }

    pub fn off(&mut self, event: &str) -> &mut Self {
        self.events.remove_listener(event);
        self
    }

    pub fn remove_all_listeners(&mut self, event: Option<&str>) -> &mut Self {
        self.events.remove_all_listeners(event);
        self
    }

    pub fn listeners(&self, event: &str) -> Vec<String> {
        self.events.listeners(event)
    }

    pub fn raw_listeners(&self, event: &str) -> Vec<String> {
        self.listeners(event)
    }

    pub fn listener_count(&self, event: &str) -> usize {
        self.events.listener_count(event)
    }

    pub fn emit(&self, event: &str) -> bool {
        self.events.emit(event)
    }
}

impl crate::stream::WritableTarget for WriteStream {
    fn write_target_chunk(&mut self, chunk: Buffer) -> NodeResult<bool> {
        self.write(chunk)
    }

    fn drain_target(&mut self) -> NodeResult<()> {
        self.flush()
    }

    fn finish_target(&mut self) -> NodeResult<()> {
        self.close()
    }
}

pub type StatsBase = Stats;
pub type BigIntStats = Stats;
pub type BigIntStatsFs = StatFs;
pub type StatsFsBase = StatFs;
pub type CreateReadStreamOptions = ReadStreamOptions;
pub type CreateWriteStreamOptions = WriteStreamOptions;

fn configure_write_stream_open(open: &mut OpenOptions, flags: &str) -> NodeResult<()> {
    match flags {
        "w" => { open.create(true).write(true).truncate(true); }
        "wx" => { open.create_new(true).write(true); }
        "w+" => { open.create(true).read(true).write(true).truncate(true); }
        "wx+" => { open.create_new(true).read(true).write(true); }
        "a" | "as" => { open.create(true).append(true); }
        "ax" => { open.create_new(true).append(true); }
        "a+" | "as+" => { open.create(true).read(true).append(true); }
        "ax+" => { open.create_new(true).read(true).append(true); }
        other => return Err(invalid_stream_option("flags", other)),
    }
    Ok(())
}

fn optional_non_negative_integer(value: Option<f64>, name: &str) -> NodeResult<Option<u64>> {
    value.map(|value| {
        if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > u64::MAX as f64 {
            return Err(NodeError::new("ERR_OUT_OF_RANGE", format!("{name} must be a non-negative integer")));
        }
        Ok(value as u64)
    }).transpose()
}

fn optional_positive_usize(value: Option<f64>, name: &str) -> NodeResult<Option<usize>> {
    value.map(|value| {
        if !value.is_finite() || value <= 0.0 || value.fract() != 0.0 || value > usize::MAX as f64 {
            return Err(NodeError::new("ERR_OUT_OF_RANGE", format!("{name} must be a positive integer")));
        }
        Ok(value as usize)
    }).transpose()
}

fn invalid_stream_option(name: &str, value: &str) -> NodeError {
    NodeError::new("ERR_INVALID_ARG_VALUE", format!("unsupported {name} value '{value}'"))
}

#[cfg(unix)]
fn apply_open_mode(open: &mut OpenOptions, mode: f64) -> NodeResult<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mode = optional_non_negative_integer(Some(mode), "mode")?
        .ok_or_else(|| NodeError::new("ERR_OUT_OF_RANGE", "mode is required"))?;
    if mode > 0o7777 {
        return Err(NodeError::new("ERR_OUT_OF_RANGE", "mode must fit a Unix permission mask"));
    }
    open.mode(mode as u32);
    Ok(())
}

#[cfg(not(unix))]
fn apply_open_mode(_open: &mut OpenOptions, mode: f64) -> NodeResult<()> {
    let _ = optional_non_negative_integer(Some(mode), "mode")?;
    Ok(())
}
pub type WatchOptionsWithBufferEncoding = WatchOptions;
pub type WatchOptionsWithStringEncoding = WatchOptions;
