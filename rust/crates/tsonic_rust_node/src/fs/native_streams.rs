use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::Seek as _;
use std::rc::{Rc, Weak};

const MAX_PENDING_FILE_OUTPUT: usize = 16 * 1024 * 1024;

#[derive(Clone)]
struct ReadOpenRequest {
    path: String,
    flags: String,
    mode: Option<u32>,
    start: u64,
    remaining: Option<u64>,
    chunk_size: usize,
}

struct ReadWorkResult {
    file: File,
    bytes: Vec<u8>,
    remaining: Option<u64>,
    complete: bool,
}

struct ReadStreamState {
    path: String,
    pending: bool,
    bytes_read: usize,
    file: Option<File>,
    chunk_size: usize,
    remaining: Option<u64>,
    closed: bool,
}

#[derive(Clone)]
pub struct ReadStream {
    state: Rc<RefCell<ReadStreamState>>,
    readable: crate::stream::Readable,
}

impl std::fmt::Debug for ReadStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.borrow();
        formatter
            .debug_struct("ReadStream")
            .field("path", &state.path)
            .field("pending", &state.pending)
            .field("bytes_read", &state.bytes_read)
            .field("closed", &state.closed)
            .finish()
    }
}

impl PartialEq for ReadStream {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}

impl Eq for ReadStream {}

pub fn read_stream_as_readable(value: &ReadStream) -> crate::stream::Readable {
    value.readable_handle()
}

pub fn read_stream_as_stream(value: &ReadStream) -> crate::stream::Stream {
    crate::stream::readable_as_stream(&value.readable_handle())
}

impl ReadStream {
    pub fn open(path: &str, options: &ReadStreamOptions) -> NodeResult<Self> {
        let request = prepare_read_open(path, options)?;
        let state = Rc::new(RefCell::new(ReadStreamState {
            path: request.path.clone(),
            pending: true,
            bytes_read: 0,
            file: None,
            chunk_size: request.chunk_size,
            remaining: request.remaining,
            closed: false,
        }));
        let readable = crate::stream::Readable::open(crate::stream::StreamOptions {
            high_water_mark: request.chunk_size,
            ..Default::default()
        });
        let stream = Self { state, readable };
        stream.bind_capacity_resume();
        stream.spawn_open(request)?;
        Ok(stream)
    }

    pub(crate) fn from_file(
        path: String,
        mut file: File,
        options: &ReadStreamOptions,
    ) -> NodeResult<Self> {
        let request = prepare_read_open(&path, options)?;
        file.seek(std::io::SeekFrom::Start(request.start))
            .map_err(map_io_error)?;
        let state = Rc::new(RefCell::new(ReadStreamState {
            path,
            pending: false,
            bytes_read: 0,
            file: Some(file),
            chunk_size: request.chunk_size,
            remaining: request.remaining,
            closed: false,
        }));
        let readable = crate::stream::Readable::open(crate::stream::StreamOptions {
            high_water_mark: request.chunk_size,
            ..Default::default()
        });
        let stream = Self { state, readable };
        stream.bind_capacity_resume();
        schedule_read(&stream.state, &stream.readable)?;
        Ok(stream)
    }

    fn spawn_open(&self, request: ReadOpenRequest) -> NodeResult<()> {
        let state = Rc::clone(&self.state);
        let readable = self.readable.clone();
        crate::background::spawn(
            move || open_and_read(request),
            move |result| complete_read(&state, &readable, result),
        )
    }

    fn bind_capacity_resume(&self) {
        let state = Rc::downgrade(&self.state);
        let readable = self.readable.clone();
        self.readable.set_capacity_handler(move || {
            let Some(state) = state.upgrade() else {
                return Ok(());
            };
            schedule_read(&state, &readable)
        });
    }

    pub fn readable_handle(&self) -> crate::stream::Readable {
        self.readable.clone()
    }

    pub fn read(&self) -> NodeResult<Option<Buffer>> {
        self.readable.read_result()
    }

    pub fn pipe_to<W: crate::stream::WritableTarget + Clone + 'static>(
        &self,
        writable: &W,
    ) -> NodeResult<W> {
        self.readable.pipe_to(writable)
    }

    pub fn close(&self) {
        let should_destroy = {
            let mut state = self.state.borrow_mut();
            if state.closed {
                false
            } else {
                state.closed = true;
                state.pending = false;
                state.file = None;
                true
            }
        };
        if should_destroy {
            self.readable.destroy();
        }
    }

    pub fn closed(&self) -> bool {
        self.state.borrow().closed
    }

    pub fn path(&self) -> String {
        self.state.borrow().path.clone()
    }

    pub fn bytes_read(&self) -> usize {
        self.state.borrow().bytes_read
    }

    pub fn pending(&self) -> bool {
        self.state.borrow().pending
    }
}

fn prepare_read_open(path: &str, options: &ReadStreamOptions) -> NodeResult<ReadOpenRequest> {
    let flags = options.flags.as_deref().unwrap_or("r");
    if !matches!(flags, "r" | "r+" | "rs+") {
        return Err(invalid_stream_option("flags", flags));
    }
    if let Some(mode) = options.mode {
        validate_open_mode(mode)?;
    }
    let start = options.start.unwrap_or(0);
    let remaining = options
        .end
        .map(|end| {
            if end < start {
                return Err(NodeError::new(
                    "ERR_OUT_OF_RANGE",
                    "end must be greater than or equal to start",
                ));
            }
            (end - start).checked_add(1).ok_or_else(|| {
                NodeError::new(
                    "ERR_OUT_OF_RANGE",
                    "stream range exceeds the native byte count",
                )
            })
        })
        .transpose()?;
    let chunk_size = options.high_water_mark.unwrap_or(64 * 1024);
    if chunk_size == 0 {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            "highWaterMark must be a positive integer",
        ));
    }
    Ok(ReadOpenRequest {
        path: path.to_owned(),
        flags: flags.to_owned(),
        mode: options.mode,
        start,
        remaining,
        chunk_size,
    })
}

fn open_and_read(request: ReadOpenRequest) -> NodeResult<ReadWorkResult> {
    let mut open = OpenOptions::new();
    match request.flags.as_str() {
        "r" => {
            open.read(true);
        }
        "r+" | "rs+" => {
            open.read(true).write(true);
        }
        _ => unreachable!("read flags were validated"),
    }
    if let Some(mode) = request.mode {
        apply_open_mode(&mut open, mode)?;
    }
    let mut file = open.open(&request.path).map_err(map_io_error)?;
    file.seek(std::io::SeekFrom::Start(request.start))
        .map_err(map_io_error)?;
    read_file_chunk(file, request.chunk_size, request.remaining)
}

fn read_file_chunk(
    mut file: File,
    chunk_size: usize,
    remaining: Option<u64>,
) -> NodeResult<ReadWorkResult> {
    let maximum = remaining
        .map(|remaining| remaining.min(chunk_size as u64) as usize)
        .unwrap_or(chunk_size);
    if maximum == 0 {
        return Ok(ReadWorkResult {
            file,
            bytes: Vec::new(),
            remaining,
            complete: true,
        });
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(maximum).map_err(|_| {
        NodeError::new(
            "ERR_OUT_OF_RANGE",
            "highWaterMark exceeds the allocatable native buffer size",
        )
    })?;
    bytes.resize(maximum, 0);
    let read = file.read(&mut bytes).map_err(map_io_error)?;
    bytes.truncate(read);
    let remaining = remaining.map(|remaining| remaining.saturating_sub(read as u64));
    Ok(ReadWorkResult {
        file,
        bytes,
        remaining,
        complete: read == 0 || remaining == Some(0),
    })
}

fn schedule_read(
    state: &Rc<RefCell<ReadStreamState>>,
    readable: &crate::stream::Readable,
) -> NodeResult<()> {
    if readable.pressured() {
        return Ok(());
    }
    let work = {
        let mut state = state.borrow_mut();
        if state.closed || state.pending {
            return Ok(());
        }
        let Some(file) = state.file.take() else {
            return Ok(());
        };
        state.pending = true;
        (file, state.chunk_size, state.remaining)
    };
    let completion_state = Rc::clone(state);
    let completion_readable = readable.clone();
    crate::background::spawn(
        move || read_file_chunk(work.0, work.1, work.2),
        move |result| complete_read(&completion_state, &completion_readable, result),
    )
}

fn complete_read(
    state: &Rc<RefCell<ReadStreamState>>,
    readable: &crate::stream::Readable,
    result: NodeResult<ReadWorkResult>,
) -> tsonic_rust_runtime::TsonicResult<()> {
    let work = match result {
        Ok(work) => work,
        Err(error) => {
            let mut state = state.borrow_mut();
            state.pending = false;
            state.closed = true;
            drop(state);
            readable
                .fail_input(error)
                .map_err(tsonic_rust_runtime::TsonicError::from)?;
            return Ok(());
        }
    };
    let bytes = work.bytes;
    let complete = work.complete;
    {
        let mut state = state.borrow_mut();
        state.pending = false;
        if state.closed {
            return Ok(());
        }
        state.remaining = work.remaining;
        state.bytes_read = state.bytes_read.saturating_add(bytes.len());
        if complete {
            state.closed = true;
        } else {
            state.file = Some(work.file);
        }
    }
    if !bytes.is_empty() {
        readable
            .enqueue(Buffer::from_bytes(bytes))
            .map_err(tsonic_rust_runtime::TsonicError::from)?;
    }
    if complete {
        readable
            .finish_input()
            .map_err(tsonic_rust_runtime::TsonicError::from)?;
    } else {
        schedule_read(state, readable).map_err(tsonic_rust_runtime::TsonicError::from)?;
    }
    Ok(())
}

struct WriteStreamState {
    path: String,
    pending: bool,
    bytes_written: usize,
    file: Option<File>,
    queue: VecDeque<Vec<u8>>,
    queued_bytes: usize,
    flush_on_finish: bool,
    opening: bool,
    writing: bool,
    ending: bool,
    destroyed: bool,
    closed: bool,
}

struct FileWritableBackend {
    state: Weak<RefCell<WriteStreamState>>,
    owner: RefCell<Option<crate::stream::WeakWritable>>,
}

impl FileWritableBackend {
    fn owner(&self) -> Option<crate::stream::Writable> {
        self.owner
            .borrow()
            .as_ref()
            .and_then(crate::stream::WeakWritable::upgrade)
    }
}

impl crate::stream::WritableBackend for FileWritableBackend {
    fn bind(&self, owner: crate::stream::WeakWritable) {
        *self.owner.borrow_mut() = Some(owner);
    }

    fn write(&self, chunk: Buffer) -> NodeResult<()> {
        let bytes = chunk.with_bytes(<[u8]>::to_vec);
        let Some(state) = self.state.upgrade() else {
            return Err(NodeError::new(
                "ERR_STREAM_CLOSED",
                "write stream is closed",
            ));
        };
        {
            let mut state = state.borrow_mut();
            if state.ending || state.destroyed || state.closed {
                return Err(NodeError::new(
                    "ERR_STREAM_WRITE_AFTER_END",
                    "write stream is closed",
                ));
            }
            let queued_bytes = state.queued_bytes.checked_add(bytes.len()).ok_or_else(|| {
                NodeError::new(
                    "ERR_FS_STREAM_BUFFER_OVERFLOW",
                    "pending file bytes overflow",
                )
            })?;
            if queued_bytes > MAX_PENDING_FILE_OUTPUT {
                return Err(NodeError::new(
                    "ERR_FS_STREAM_BUFFER_OVERFLOW",
                    "pending file bytes exceed the finite stream limit",
                ));
            }
            state.queued_bytes = queued_bytes;
            state.queue.push_back(bytes);
        }
        if let Some(owner) = self.owner() {
            schedule_write(&state, &owner)?;
        }
        Ok(())
    }

    fn finish(&self) -> NodeResult<bool> {
        let Some(state) = self.state.upgrade() else {
            return Ok(true);
        };
        state.borrow_mut().ending = true;
        if let Some(owner) = self.owner() {
            schedule_write(&state, &owner)?;
        }
        let closed = state.borrow().closed;
        Ok(closed)
    }

    fn destroy(&self) -> NodeResult<()> {
        let Some(state) = self.state.upgrade() else {
            return Ok(());
        };
        let mut state = state.borrow_mut();
        state.destroyed = true;
        state.ending = true;
        state.queue.clear();
        state.queued_bytes = 0;
        state.file = None;
        if !state.opening && !state.writing {
            state.closed = true;
        }
        Ok(())
    }

    fn buffered_bytes(&self) -> usize {
        self.state
            .upgrade()
            .map(|state| state.borrow().queued_bytes)
            .unwrap_or(0)
    }
}

#[derive(Clone)]
pub struct WriteStream {
    state: Rc<RefCell<WriteStreamState>>,
    writable: crate::stream::Writable,
}

impl std::fmt::Debug for WriteStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.borrow();
        formatter
            .debug_struct("WriteStream")
            .field("path", &state.path)
            .field("pending", &state.pending)
            .field("bytes_written", &state.bytes_written)
            .field("closed", &state.closed)
            .finish()
    }
}

impl PartialEq for WriteStream {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}

impl Eq for WriteStream {}

pub fn write_stream_as_writable(value: &WriteStream) -> crate::stream::Writable {
    value.writable_handle()
}

pub fn write_stream_as_stream(value: &WriteStream) -> crate::stream::Stream {
    crate::stream::writable_as_stream(&value.writable_handle())
}

impl WriteStream {
    pub fn open(path: &str, options: &WriteStreamOptions) -> NodeResult<Self> {
        validate_write_options(options)?;
        let state = Rc::new(RefCell::new(WriteStreamState {
            path: path.to_owned(),
            pending: true,
            bytes_written: 0,
            file: None,
            queue: VecDeque::new(),
            queued_bytes: 0,
            flush_on_finish: options.flush.unwrap_or(false),
            opening: true,
            writing: false,
            ending: false,
            destroyed: false,
            closed: false,
        }));
        let backend = Rc::new(FileWritableBackend {
            state: Rc::downgrade(&state),
            owner: RefCell::new(None),
        });
        let writable = crate::stream::Writable::with_backend(
            crate::stream::StreamOptions {
                high_water_mark: options.high_water_mark.unwrap_or(64 * 1024),
                ..Default::default()
            },
            backend,
        );
        let stream = Self { state, writable };
        stream.spawn_open(options.clone())?;
        Ok(stream)
    }

    pub(crate) fn from_file(
        path: String,
        mut file: File,
        options: &WriteStreamOptions,
    ) -> NodeResult<Self> {
        validate_write_options(options)?;
        if let Some(start) = options.start {
            file.seek(std::io::SeekFrom::Start(start))
                .map_err(map_io_error)?;
        }
        let state = Rc::new(RefCell::new(WriteStreamState {
            path,
            pending: false,
            bytes_written: 0,
            file: Some(file),
            queue: VecDeque::new(),
            queued_bytes: 0,
            flush_on_finish: options.flush.unwrap_or(false),
            opening: false,
            writing: false,
            ending: false,
            destroyed: false,
            closed: false,
        }));
        let backend = Rc::new(FileWritableBackend {
            state: Rc::downgrade(&state),
            owner: RefCell::new(None),
        });
        let writable = crate::stream::Writable::with_backend(
            crate::stream::StreamOptions {
                high_water_mark: options.high_water_mark.unwrap_or(64 * 1024),
                ..Default::default()
            },
            backend,
        );
        Ok(Self { state, writable })
    }

    fn spawn_open(&self, options: WriteStreamOptions) -> NodeResult<()> {
        let path = self.path();
        let state = Rc::clone(&self.state);
        let writable = self.writable.clone();
        crate::background::spawn(
            move || open_write_file(&path, &options),
            move |result| {
                match result {
                    Ok(file) => {
                        {
                            let mut state = state.borrow_mut();
                            state.opening = false;
                            state.pending = state.writing;
                            if state.destroyed {
                                state.closed = true;
                                return Ok(());
                            }
                            state.file = Some(file);
                        }
                        schedule_write(&state, &writable)
                            .map_err(tsonic_rust_runtime::TsonicError::from)?;
                    }
                    Err(error) => {
                        {
                            let mut state = state.borrow_mut();
                            state.opening = false;
                            state.pending = false;
                            state.closed = true;
                        }
                        writable
                            .fail(error)
                            .map_err(tsonic_rust_runtime::TsonicError::from)?;
                    }
                }
                Ok(())
            },
        )
    }

    pub fn writable_handle(&self) -> crate::stream::Writable {
        self.writable.clone()
    }

    pub fn write(&self, chunk: Buffer) -> NodeResult<bool> {
        self.writable.write_buffer(&chunk)
    }

    pub fn close(&self) -> NodeResult<()> {
        self.writable.end_checked()
    }

    pub fn closed(&self) -> bool {
        self.state.borrow().closed
    }

    pub fn path(&self) -> String {
        self.state.borrow().path.clone()
    }

    pub fn bytes_written(&self) -> usize {
        self.state.borrow().bytes_written
    }

    pub fn pending(&self) -> bool {
        self.state.borrow().pending
    }
}

impl crate::stream::WritableTarget for WriteStream {
    fn writable_handle(&self) -> crate::stream::Writable {
        self.writable.clone()
    }
}

fn validate_write_options(options: &WriteStreamOptions) -> NodeResult<()> {
    let mut open = OpenOptions::new();
    configure_write_stream_open(&mut open, options.flags.as_deref().unwrap_or("w"))?;
    if let Some(mode) = options.mode {
        validate_open_mode(mode)?;
    }
    if options.high_water_mark == Some(0) {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            "highWaterMark must be a positive integer",
        ));
    }
    Ok(())
}

fn open_write_file(path: &str, options: &WriteStreamOptions) -> NodeResult<File> {
    let mut open = OpenOptions::new();
    configure_write_stream_open(&mut open, options.flags.as_deref().unwrap_or("w"))?;
    if let Some(mode) = options.mode {
        apply_open_mode(&mut open, mode)?;
    }
    let mut file = open.open(path).map_err(map_io_error)?;
    if let Some(start) = options.start {
        file.seek(std::io::SeekFrom::Start(start))
            .map_err(map_io_error)?;
    }
    Ok(file)
}

fn schedule_write(
    state: &Rc<RefCell<WriteStreamState>>,
    writable: &crate::stream::Writable,
) -> NodeResult<()> {
    enum Work {
        Write(File, Vec<u8>),
        Finish(File, bool),
    }
    let work = {
        let mut state = state.borrow_mut();
        if state.opening || state.writing || state.closed || state.destroyed {
            return Ok(());
        }
        let Some(file) = state.file.take() else {
            return Ok(());
        };
        if let Some(bytes) = state.queue.pop_front() {
            state.writing = true;
            state.pending = true;
            Work::Write(file, bytes)
        } else if state.ending {
            state.writing = true;
            state.pending = true;
            Work::Finish(file, state.flush_on_finish)
        } else {
            state.file = Some(file);
            return Ok(());
        }
    };

    let completion_state = Rc::clone(state);
    let completion_writable = writable.clone();
    crate::background::spawn(
        move || match work {
            Work::Write(mut file, bytes) => {
                file.write_all(&bytes).map_err(map_io_error)?;
                Ok((Some(file), bytes.len(), false))
            }
            Work::Finish(file, flush) => {
                if flush {
                    file.sync_all().map_err(map_io_error)?;
                }
                Ok((None, 0, true))
            }
        },
        move |result| {
            match result {
                Ok((file, written, finished)) => {
                    {
                        let mut state = completion_state.borrow_mut();
                        state.writing = false;
                        state.pending = false;
                        state.queued_bytes = state.queued_bytes.saturating_sub(written);
                        state.bytes_written = state.bytes_written.saturating_add(written);
                        if state.destroyed || finished {
                            state.closed = true;
                        } else {
                            state.file = file;
                        }
                    }
                    completion_writable
                        .poll_progress()
                        .map_err(tsonic_rust_runtime::TsonicError::from)?;
                    if finished {
                        completion_writable
                            .complete_finish()
                            .map_err(tsonic_rust_runtime::TsonicError::from)?;
                    } else {
                        schedule_write(&completion_state, &completion_writable)
                            .map_err(tsonic_rust_runtime::TsonicError::from)?;
                    }
                }
                Err(error) => {
                    {
                        let mut state = completion_state.borrow_mut();
                        state.writing = false;
                        state.pending = false;
                        state.closed = true;
                        state.file = None;
                        state.queue.clear();
                        state.queued_bytes = 0;
                    }
                    completion_writable
                        .fail(error)
                        .map_err(tsonic_rust_runtime::TsonicError::from)?;
                }
            }
            Ok(())
        },
    )
}

fn validate_open_mode(mode: u32) -> NodeResult<()> {
    if mode > 0o7777 {
        Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            "mode must fit a Unix permission mask",
        ))
    } else {
        Ok(())
    }
}
