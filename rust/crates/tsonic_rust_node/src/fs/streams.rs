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

include!("native_streams.rs");

pub type StatsBase = Stats;
pub type BigIntStats = Stats;
pub type BigIntStatsFs = StatFs;
pub type StatsFsBase = StatFs;
pub type CreateReadStreamOptions = ReadStreamOptions;
pub type CreateWriteStreamOptions = WriteStreamOptions;

fn configure_write_stream_open(open: &mut OpenOptions, flags: &str) -> NodeResult<()> {
    match flags {
        "w" => {
            open.create(true).write(true).truncate(true);
        }
        "wx" => {
            open.create_new(true).write(true);
        }
        "w+" => {
            open.create(true).read(true).write(true).truncate(true);
        }
        "wx+" => {
            open.create_new(true).read(true).write(true);
        }
        "a" | "as" => {
            open.create(true).append(true);
        }
        "ax" => {
            open.create_new(true).append(true);
        }
        "a+" | "as+" => {
            open.create(true).read(true).append(true);
        }
        "ax+" => {
            open.create_new(true).read(true).append(true);
        }
        other => return Err(invalid_stream_option("flags", other)),
    }
    Ok(())
}

fn invalid_stream_option(name: &str, value: &str) -> NodeError {
    NodeError::new(
        "ERR_INVALID_ARG_VALUE",
        format!("unsupported {name} value '{value}'"),
    )
}

#[cfg(unix)]
fn apply_open_mode(open: &mut OpenOptions, mode: u32) -> NodeResult<()> {
    use std::os::unix::fs::OpenOptionsExt;
    if mode > 0o7777 {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            "mode must fit a Unix permission mask",
        ));
    }
    open.mode(mode);
    Ok(())
}

#[cfg(not(unix))]
fn apply_open_mode(_open: &mut OpenOptions, _mode: u32) -> NodeResult<()> {
    Ok(())
}

pub type WatchOptionsWithBufferEncoding = WatchOptions;
pub type WatchOptionsWithStringEncoding = WatchOptions;
