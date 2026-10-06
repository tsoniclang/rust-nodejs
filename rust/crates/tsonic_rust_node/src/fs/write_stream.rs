const MAX_PENDING_FILE_OUTPUT: usize = 16 * 1024 * 1024;

struct WriteStreamState<E: 'static> {
    path: String,
    background: crate::background::BackgroundHandle<E>,
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

struct FileWritableBackend<E: 'static> {
    state: Rc<RefCell<WriteStreamState<E>>>,
    owner: RefCell<Option<crate::stream::WeakWritable<E>>>,
}

impl<E: From<NodeError> + 'static> FileWritableBackend<E> {
    fn owner(&self) -> Option<crate::stream::Writable<E>> {
        self.owner
            .borrow()
            .as_ref()
            .and_then(crate::stream::WeakWritable::<E>::upgrade)
    }
}

impl<E: From<NodeError> + 'static> crate::stream::WritableBackend<E> for FileWritableBackend<E> {
    fn bind(&self, owner: crate::stream::WeakWritable<E>) {
        *self.owner.borrow_mut() = Some(owner);
    }

    fn write(&self, chunk: Buffer) -> crate::stream::StreamBackendResult<(), E> {
        let bytes = chunk.with_bytes(<[u8]>::to_vec);
        let state = &self.state;
        {
            let mut state = state.borrow_mut();
            if state.ending || state.destroyed || state.closed {
                return Err(
                    NodeError::new("ERR_STREAM_WRITE_AFTER_END", "write stream is closed").into(),
                );
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
                )
                .into());
            }
            state.queued_bytes = queued_bytes;
            state.queue.push_back(bytes);
        }
        if let Some(owner) = self.owner() {
            schedule_write(state, &owner)?;
        }
        Ok(())
    }

    fn finish(&self) -> crate::stream::StreamBackendResult<bool, E> {
        let state = &self.state;
        state.borrow_mut().ending = true;
        if let Some(owner) = self.owner() {
            schedule_write(state, &owner)?;
        }
        let closed = state.borrow().closed;
        Ok(closed)
    }

    fn destroy(&self) -> crate::stream::StreamBackendResult<(), E> {
        let mut state = self.state.borrow_mut();
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
        self.state.borrow().queued_bytes
    }
}

pub struct WriteStream<E: 'static = NodeError> {
    state: Rc<RefCell<WriteStreamState<E>>>,
    writable: crate::stream::Writable<E>,
}

impl<E: 'static> std::fmt::Debug for WriteStream<E> {
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

impl<E: 'static> PartialEq for WriteStream<E> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}

impl<E: 'static> Eq for WriteStream<E> {}

pub fn write_stream_as_writable<E: From<NodeError> + 'static>(
    value: &WriteStream<E>,
) -> crate::stream::Writable<E> {
    value.writable_handle()
}

pub fn write_stream_as_stream<E: From<NodeError> + 'static>(
    value: &WriteStream<E>,
) -> crate::stream::Stream<E> {
    crate::stream::writable_as_stream(&value.writable_handle())
}

impl<E: From<NodeError> + 'static> WriteStream<E> {
    pub fn open(
        background: &crate::background::BackgroundTasks<E>,
        path: &str,
        options: &WriteStreamOptions,
    ) -> NodeResult<Self> {
        validate_write_options(options)?;
        let state = Rc::new(RefCell::new(WriteStreamState::<E> {
            path: path.to_owned(),
            background: background.handle(),
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
        let backend = Rc::new(FileWritableBackend::<E> {
            state: Rc::clone(&state),
            owner: RefCell::new(None),
        });
        let writable = crate::stream::Writable::<E>::with_backend(
            crate::stream::StreamOptions {
                high_water_mark: options.high_water_mark.unwrap_or(64 * 1024),
                ..Default::default()
            },
            backend,
            None,
        );
        let stream = Self { state, writable };
        stream.spawn_open(options.clone())?;
        Ok(stream)
    }

    pub(crate) fn from_file(
        background: &crate::background::BackgroundTasks<E>,
        path: String,
        mut file: File,
        options: &WriteStreamOptions,
    ) -> NodeResult<Self> {
        validate_write_options(options)?;
        if let Some(start) = options.start {
            file.seek(std::io::SeekFrom::Start(start))
                .map_err(map_io_error)?;
        }
        let state = Rc::new(RefCell::new(WriteStreamState::<E> {
            path,
            background: background.handle(),
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
        let backend = Rc::new(FileWritableBackend::<E> {
            state: Rc::clone(&state),
            owner: RefCell::new(None),
        });
        let writable = crate::stream::Writable::<E>::with_backend(
            crate::stream::StreamOptions {
                high_water_mark: options.high_water_mark.unwrap_or(64 * 1024),
                ..Default::default()
            },
            backend,
            None,
        );
        Ok(Self { state, writable })
    }

    fn spawn_open(&self, options: WriteStreamOptions) -> NodeResult<()> {
        let path = self.path();
        let state = Rc::clone(&self.state);
        let writable = self.writable.clone();
        let background = self.state.borrow().background.clone();
        background.spawn(
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
                        schedule_write(&state, &writable).map_err(E::from)?;
                    }
                    Err(error) => {
                        {
                            let mut state = state.borrow_mut();
                            state.opening = false;
                            state.pending = false;
                            state.closed = true;
                        }
                        writable.fail(error)?;
                    }
                }
                Ok(())
            },
        )
    }

    pub fn writable_handle(&self) -> crate::stream::Writable<E> {
        self.writable.clone()
    }

    pub fn write(&self, chunk: Buffer) -> Result<bool, E> {
        self.writable.write_buffer(&chunk)
    }

    pub fn close(&self) -> Result<(), E> {
        self.writable.end().map(|_| ())
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

impl<E: From<NodeError> + 'static> crate::stream::WritableTarget<E> for WriteStream<E> {
    fn writable_handle(&self) -> crate::stream::Writable<E> {
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

fn schedule_write<E: From<NodeError> + 'static>(
    state: &Rc<RefCell<WriteStreamState<E>>>,
    writable: &crate::stream::Writable<E>,
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
    let background = state.borrow().background.clone();
    let submitted = background.spawn(
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
                    if finished {
                        completion_writable.mark_finished();
                    } else {
                        schedule_write(&completion_state, &completion_writable).map_err(E::from)?;
                    }
                    completion_writable.poll_progress()?;
                    if finished {
                        completion_writable.complete_finish()?;
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
                    completion_writable.fail(error)?;
                }
            }
            Ok(())
        },
    );
    if submitted.is_err() {
        let mut state = state.borrow_mut();
        state.writing = false;
        state.pending = false;
        state.closed = true;
        state.queue.clear();
        state.queued_bytes = 0;
    }
    submitted
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

impl<E: 'static> Clone for WriteStream<E> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
            writable: self.writable.clone(),
        }
    }
}
