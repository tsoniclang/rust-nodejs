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

struct ReadStreamState<E: 'static> {
    path: String,
    background: crate::background::BackgroundHandle<E>,
    pending: bool,
    bytes_read: usize,
    file: Option<File>,
    chunk_size: usize,
    remaining: Option<u64>,
    closed: bool,
}

pub struct ReadStream<E: 'static = NodeError> {
    state: Rc<RefCell<ReadStreamState<E>>>,
    readable: crate::stream::Readable<E>,
}

impl<E: 'static> std::fmt::Debug for ReadStream<E> {
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

impl<E: 'static> PartialEq for ReadStream<E> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}

impl<E: 'static> Eq for ReadStream<E> {}

pub fn read_stream_as_readable<E: From<NodeError> + 'static>(
    value: &ReadStream<E>,
) -> crate::stream::Readable<E> {
    value.readable_handle()
}

pub fn read_stream_as_stream<E: From<NodeError> + 'static>(
    value: &ReadStream<E>,
) -> crate::stream::Stream<E> {
    crate::stream::readable_as_stream(&value.readable_handle())
}

impl<E: From<NodeError> + 'static> ReadStream<E> {
    pub fn open(
        background: &crate::background::BackgroundTasks<E>,
        path: &str,
        options: &ReadStreamOptions,
    ) -> NodeResult<Self> {
        let request = prepare_read_open(path, options)?;
        let state = Rc::new(RefCell::new(ReadStreamState::<E> {
            path: request.path.clone(),
            background: background.handle(),
            pending: true,
            bytes_read: 0,
            file: None,
            chunk_size: request.chunk_size,
            remaining: request.remaining,
            closed: false,
        }));
        let readable = crate::stream::Readable::<E>::open(crate::stream::StreamOptions {
            high_water_mark: request.chunk_size,
            ..Default::default()
        });
        let stream = Self { state, readable };
        stream.bind_capacity_resume();
        stream.spawn_open(request)?;
        Ok(stream)
    }

    pub(crate) fn from_file(
        background: &crate::background::BackgroundTasks<E>,
        path: String,
        mut file: File,
        options: &ReadStreamOptions,
    ) -> NodeResult<Self> {
        let request = prepare_read_open(&path, options)?;
        file.seek(std::io::SeekFrom::Start(request.start))
            .map_err(map_io_error)?;
        let state = Rc::new(RefCell::new(ReadStreamState::<E> {
            path,
            background: background.handle(),
            pending: false,
            bytes_read: 0,
            file: Some(file),
            chunk_size: request.chunk_size,
            remaining: request.remaining,
            closed: false,
        }));
        let readable = crate::stream::Readable::<E>::open(crate::stream::StreamOptions {
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
        let background = self.state.borrow().background.clone();
        background.spawn(
            move || open_and_read(request),
            move |result| complete_read(&state, &readable, result),
        )
    }

    fn bind_capacity_resume(&self) {
        let state = Rc::clone(&self.state);
        let readable = self.readable.downgrade();
        self.readable.set_capacity_handler(move || {
            let Some(readable) = readable.upgrade() else {
                return Ok(());
            };
            if let Err(error) = schedule_read(&state, &readable) {
                readable.fail_input(error.clone())?;
                return Err(error.into());
            }
            Ok(())
        });
    }

    pub fn readable_handle(&self) -> crate::stream::Readable<E> {
        self.readable.clone()
    }

    pub fn read(&self) -> Result<Option<Buffer>, E> {
        self.readable.read()
    }

    pub fn pipe_to<W: crate::stream::WritableTarget<E> + Clone + 'static>(
        &self,
        writable: &W,
    ) -> Result<W, E> {
        self.readable.pipe_to(writable)
    }

    pub fn close(&self) -> Result<(), E> {
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
            self.readable.destroy()?;
        }
        Ok(())
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

fn schedule_read<E: From<NodeError> + 'static>(
    state: &Rc<RefCell<ReadStreamState<E>>>,
    readable: &crate::stream::Readable<E>,
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
    let background = state.borrow().background.clone();
    let submitted = background.spawn(
        move || read_file_chunk(work.0, work.1, work.2),
        move |result| complete_read(&completion_state, &completion_readable, result),
    );
    if submitted.is_err() {
        let mut state = state.borrow_mut();
        state.pending = false;
        state.closed = true;
    }
    submitted
}

fn complete_read<E: From<NodeError> + 'static>(
    state: &Rc<RefCell<ReadStreamState<E>>>,
    readable: &crate::stream::Readable<E>,
    result: NodeResult<ReadWorkResult>,
) -> Result<(), E> {
    let work = match result {
        Ok(work) => work,
        Err(error) => {
            let mut state = state.borrow_mut();
            state.pending = false;
            state.closed = true;
            drop(state);
            readable.fail_input(error)?;
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
    readable.buffer_input(
        (!bytes.is_empty()).then(|| Buffer::from_bytes(bytes)),
        complete,
    );
    if !complete {
        if let Err(error) = schedule_read(state, readable) {
            readable.fail_input(error.clone())?;
            return Err(error.into());
        }
    }
    readable.pump_flowing()
}

impl<E: 'static> Clone for ReadStream<E> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
            readable: self.readable.clone(),
        }
    }
}
