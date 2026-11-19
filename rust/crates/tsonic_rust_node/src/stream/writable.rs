pub(crate) struct WeakWritable<E: 'static> {
    state: Weak<RefCell<WritableState<E>>>,
}

impl<E: 'static> WeakWritable<E> {
    pub(crate) fn upgrade(&self) -> Option<Writable<E>> {
        self.state.upgrade().map(|state| Writable::<E> { state })
    }
}

pub(crate) trait WritableBackend<E: 'static> {
    fn bind(&self, _owner: WeakWritable<E>) {}
    fn write(&self, chunk: Buffer) -> StreamBackendResult<(), E>;
    fn flush(&self) -> StreamBackendResult<(), E> {
        Ok(())
    }
    fn finish(&self) -> StreamBackendResult<bool, E>;
    fn finish_accepted(&self) -> bool {
        false
    }
    fn destroy(&self) -> StreamBackendResult<(), E>;
    fn buffered_bytes(&self) -> usize;
    fn chunks(&self) -> Vec<Buffer> {
        Vec::new()
    }
    fn is_tty(&self) -> bool {
        false
    }
    fn fd(&self) -> i32 {
        -1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WritableSink {
    Stdout,
    Stderr,
}

#[derive(Default)]
struct MemoryWritableBackend {
    chunks: RefCell<Vec<Buffer>>,
}

impl<E: From<NodeError> + 'static> WritableBackend<E> for MemoryWritableBackend {
    fn write(&self, chunk: Buffer) -> StreamBackendResult<(), E> {
        self.chunks.borrow_mut().push(chunk);
        Ok(())
    }

    fn finish(&self) -> StreamBackendResult<bool, E> {
        Ok(true)
    }

    fn destroy(&self) -> StreamBackendResult<(), E> {
        self.chunks.borrow_mut().clear();
        Ok(())
    }

    fn buffered_bytes(&self) -> usize {
        0
    }

    fn chunks(&self) -> Vec<Buffer> {
        self.chunks.borrow().clone()
    }
}

struct TerminalWritableBackend {
    sink: WritableSink,
}

impl<E: From<NodeError> + 'static> WritableBackend<E> for TerminalWritableBackend {
    fn write(&self, chunk: Buffer) -> StreamBackendResult<(), E> {
        chunk.with_bytes(|bytes| write_sink(self.sink, bytes))?;
        Ok(())
    }

    fn finish(&self) -> StreamBackendResult<bool, E> {
        Ok(true)
    }

    fn destroy(&self) -> StreamBackendResult<(), E> {
        Ok(())
    }

    fn buffered_bytes(&self) -> usize {
        0
    }

    fn is_tty(&self) -> bool {
        use std::io::IsTerminal as _;
        match self.sink {
            WritableSink::Stdout => std::io::stdout().is_terminal(),
            WritableSink::Stderr => std::io::stderr().is_terminal(),
        }
    }

    fn fd(&self) -> i32 {
        match self.sink {
            WritableSink::Stdout => 1,
            WritableSink::Stderr => 2,
        }
    }
}

struct WritableState<E: 'static> {
    options: StreamOptions,
    backend: Rc<dyn WritableBackend<E>>,
    ending: bool,
    finished: bool,
    finish_emitted: bool,
    destroyed: bool,
    lifecycle: StreamLifecycle<E>,
    corked: usize,
    corked_chunks: VecDeque<Buffer>,
    corked_bytes: usize,
    need_drain: bool,
    drain_event: StreamEvent<(), E>,
    finish_event: StreamEvent<(), E>,
}

pub struct Writable<E: 'static = NodeError> {
    state: Rc<RefCell<WritableState<E>>>,
}

impl<E: 'static> std::fmt::Debug for Writable<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.borrow();
        formatter
            .debug_struct("Writable")
            .field("ending", &state.ending)
            .field("finished", &state.finished)
            .field("destroyed", &state.destroyed)
            .field(
                "buffered_bytes",
                &state
                    .backend
                    .buffered_bytes()
                    .saturating_add(state.corked_bytes),
            )
            .finish()
    }
}

impl<E: From<NodeError> + 'static> Default for Writable<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: 'static> PartialEq for Writable<E> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}

impl<E: 'static> Eq for Writable<E> {}

impl<E: From<NodeError> + 'static> Writable<E> {
    pub fn new() -> Self {
        Self::with_backend(
            StreamOptions::default(),
            Rc::new(MemoryWritableBackend::default()),
            None,
        )
    }

    pub fn with_options(options: StreamOptions) -> Self {
        Self::with_backend(options, Rc::new(MemoryWritableBackend::default()), None)
    }

    pub(crate) fn with_backend(
        options: StreamOptions,
        backend: Rc<dyn WritableBackend<E>>,
        lifecycle: Option<StreamLifecycle<E>>,
    ) -> Self {
        let lifecycle = lifecycle.unwrap_or_else(|| StreamLifecycle::<E>::new(options.emit_close));
        let writable = Self {
            state: Rc::new(RefCell::new(WritableState::<E> {
                options,
                backend,
                ending: false,
                finished: false,
                finish_emitted: false,
                destroyed: false,
                lifecycle: lifecycle.clone(),
                corked: 0,
                corked_chunks: VecDeque::new(),
                corked_bytes: 0,
                need_drain: false,
                drain_event: StreamEvent::default(),
                finish_event: StreamEvent::default(),
            })),
        };
        lifecycle.bind_writable(&writable);
        writable.state.borrow().backend.bind(writable.downgrade());
        writable
    }

    pub fn stdout() -> Self {
        Self::with_backend(
            StreamOptions::default(),
            Rc::new(TerminalWritableBackend {
                sink: WritableSink::Stdout,
            }),
            None,
        )
    }

    pub fn stderr() -> Self {
        Self::with_backend(
            StreamOptions::default(),
            Rc::new(TerminalWritableBackend {
                sink: WritableSink::Stderr,
            }),
            None,
        )
    }

    pub fn write(&self, chunk: Buffer) -> Result<bool, E> {
        self.write_owned(chunk)
    }

    fn write_owned(&self, chunk: Buffer) -> Result<bool, E> {
        let backend = {
            let mut state = self.state.borrow_mut();
            if state.ending || state.destroyed {
                drop(state);
                return Err(NodeError::new("ERR_STREAM_WRITE_AFTER_END", "write after end").into());
            }
            if state.corked > 0 {
                state.corked_bytes = state.corked_bytes.saturating_add(chunk.len());
                state.corked_chunks.push_back(chunk);
                let pressured =
                    self.buffered_bytes_with(&state) >= state.options.high_water_mark.max(1);
                state.need_drain |= pressured;
                return Ok(!pressured);
            }
            Rc::clone(&state.backend)
        };
        self.backend_result(backend.write(chunk))?;
        let mut state = self.state.borrow_mut();
        let pressured = self.buffered_bytes_with(&state) >= state.options.high_water_mark.max(1);
        state.need_drain |= pressured;
        Ok(!pressured)
    }

    pub fn write_string(&self, value: &str) -> Result<bool, E> {
        self.write_owned(Buffer::from_string(value, Some("utf8"))?)
    }

    pub fn write_buffer(&self, value: &Buffer) -> Result<bool, E> {
        self.write_owned(value.clone())
    }

    pub fn is_tty(&self) -> bool {
        self.state.borrow().backend.is_tty()
    }

    pub fn fd(&self) -> i32 {
        self.state.borrow().backend.fd()
    }

    pub fn writev(&self, chunks: &[Buffer]) -> Result<bool, E> {
        let mut writable = true;
        for chunk in chunks {
            writable = self.write(chunk.clone())? && writable;
        }
        Ok(writable)
    }

    pub fn cork(&self) {
        self.state.borrow_mut().corked += 1;
    }

    pub fn uncork(&self) -> Result<(), E> {
        {
            let mut state = self.state.borrow_mut();
            state.corked = state.corked.saturating_sub(1);
            if state.corked != 0 {
                return Ok(());
            }
        }
        self.flush_corked()?;
        self.poll_progress()
    }

    fn flush_corked(&self) -> Result<(), E> {
        loop {
            let selected = {
                let mut state = self.state.borrow_mut();
                state.corked_chunks.pop_front().map(|chunk| {
                    state.corked_bytes = state.corked_bytes.saturating_sub(chunk.len());
                    (chunk, Rc::clone(&state.backend))
                })
            };
            let Some((chunk, backend)) = selected else {
                return Ok(());
            };
            self.backend_result(backend.write(chunk))?;
        }
    }

    fn backend_result<Output>(&self, result: StreamBackendResult<Output, E>) -> Result<Output, E> {
        match result {
            Ok(value) => Ok(value),
            Err(StreamBackendFailure::Callback(error)) => Err(error),
            Err(StreamBackendFailure::Native(error)) => {
                self.fail(error.clone())?;
                Err(error.into())
            }
        }
    }

    pub fn writable_corked(&self) -> usize {
        self.state.borrow().corked
    }

    pub fn set_default_encoding(&self, encoding: &str) {
        self.state.borrow_mut().options.default_encoding = encoding.to_ascii_lowercase();
    }

    pub fn default_encoding(&self) -> String {
        self.state.borrow().options.default_encoding.clone()
    }

    pub fn writable_high_water_mark(&self) -> usize {
        self.state.borrow().options.high_water_mark
    }

    pub fn writable_object_mode(&self) -> bool {
        self.state.borrow().options.object_mode
    }

    pub fn writable_length(&self) -> usize {
        let state = self.state.borrow();
        self.buffered_bytes_with(&state)
    }

    pub fn writable_need_drain(&self) -> bool {
        self.state.borrow().need_drain
    }

    pub fn writable(&self) -> bool {
        let state = self.state.borrow();
        !state.ending && !state.destroyed
    }

    pub fn writable_ended(&self) -> bool {
        self.state.borrow().ending
    }

    pub fn writable_finished(&self) -> bool {
        self.state.borrow().finished
    }

    pub fn writable_aborted(&self) -> bool {
        let state = self.state.borrow();
        state.destroyed && !state.finished
    }

    pub fn emit_close(&self) -> bool {
        self.state.borrow().options.emit_close
    }

    pub fn errored(&self) -> Option<String> {
        self.state.borrow().lifecycle.errored()
    }

    pub fn closed(&self) -> bool {
        self.state.borrow().lifecycle.closed()
    }

    pub fn destroyed(&self) -> bool {
        self.state.borrow().destroyed
    }

    pub fn clear_drain(&self) -> Result<(), E> {
        let callbacks = {
            let mut state = self.state.borrow_mut();
            if !state.need_drain {
                return Ok(());
            }
            state.need_drain = false;
            state.drain_event.emission()
        };
        invoke_event(callbacks, ())
    }

    pub fn final_callback(&self, callback: impl FnOnce()) -> Result<(), E> {
        self.end_result()?;
        callback();
        Ok(())
    }

    pub fn construct_callback(&self, callback: impl FnOnce()) {
        callback();
    }

    pub fn destroy_with_error(&self, error: impl Into<String>) -> Result<(), E> {
        self.fail(NodeError::new("ERR_STREAM_DESTROYED", error))
    }

    pub fn add_chunk(&self, chunk: Buffer) -> Result<bool, E> {
        self.write(chunk)
    }

    pub fn write_str(&self, value: &str, encoding: Option<&str>) -> Result<bool, E> {
        self.write(Buffer::from_string(value, encoding)?)
    }

    pub fn flush(&self) -> Result<bool, E> {
        self.clear_drain()?;
        Ok(true)
    }

    pub fn end(&self) -> Result<Self, E> {
        self.end_result()?;
        Ok(self.clone())
    }

    pub(crate) fn flush_backend(&self) -> Result<(), E> {
        let backend = {
            let state = self.state.borrow();
            if state.ending || state.destroyed {
                drop(state);
                return Err(NodeError::new("ERR_STREAM_WRITE_AFTER_END", "flush after end").into());
            }
            Rc::clone(&state.backend)
        };
        self.backend_result(backend.flush())
    }

    pub fn end_string(&self, value: &str) -> Result<Self, E> {
        self.write_string(value)?;
        self.end_result()?;
        Ok(self.clone())
    }

    pub fn end_buffer(&self, value: &Buffer) -> Result<Self, E> {
        self.write_buffer(value)?;
        self.end_result()?;
        Ok(self.clone())
    }

    fn end_result(&self) -> Result<(), E> {
        {
            let mut state = self.state.borrow_mut();
            if state.finished {
                drop(state);
                return self.complete_finish();
            }
            if state.destroyed {
                return Ok(());
            }
            state.ending = true;
            state.corked = 0;
        }
        self.flush_corked()?;
        let backend = Rc::clone(&self.state.borrow().backend);
        let completion = backend.finish();
        if matches!(&completion, Err(StreamBackendFailure::Callback(_)))
            && backend.finish_accepted()
        {
            self.mark_finished();
        }
        let finished = self.backend_result(completion)?;
        if finished {
            self.complete_finish()?;
        }
        Ok(())
    }

    pub fn destroy(&self) -> Result<(), E> {
        self.destroy_result(None)
    }

    pub fn destroy_chain(
        &self,
        error: Option<tsonic_rust_runtime::RetainedError>,
    ) -> Result<Self, E> {
        self.destroy_result(error)?;
        Ok(self.clone())
    }

    fn destroy_result(&self, error: Option<tsonic_rust_runtime::RetainedError>) -> Result<(), E> {
        let lifecycle = self.state.borrow().lifecycle.clone();
        lifecycle.destroy(error)
    }

    fn destroy_storage(&self) -> StreamBackendResult<(), E> {
        let backend = {
            let mut state = self.state.borrow_mut();
            if state.destroyed {
                return Ok(());
            }
            state.destroyed = true;
            state.ending = true;
            state.corked = 0;
            state.corked_chunks.clear();
            state.corked_bytes = 0;
            state.need_drain = false;
            state.backend.clone()
        };
        backend.destroy()
    }

    pub fn is_ended(&self) -> bool {
        self.writable_ended()
    }

    pub fn chunks(&self) -> Vec<Buffer> {
        self.state.borrow().backend.chunks()
    }

    pub(crate) fn downgrade(&self) -> WeakWritable<E> {
        WeakWritable::<E> {
            state: Rc::downgrade(&self.state),
        }
    }

    pub(crate) fn poll_progress(&self) -> Result<(), E> {
        let callbacks = {
            let mut state = self.state.borrow_mut();
            let below_mark =
                self.buffered_bytes_with(&state) < state.options.high_water_mark.max(1);
            if state.need_drain && below_mark {
                state.need_drain = false;
                state.drain_event.emission()
            } else {
                StreamEmission::default()
            }
        };
        invoke_event(callbacks, ())
    }

    pub(crate) fn complete_finish(&self) -> Result<(), E> {
        let (finish_callbacks, lifecycle) = {
            let mut state = self.state.borrow_mut();
            if state.destroyed {
                return Ok(());
            }
            state.finished = true;
            let finish_callbacks = if state.finish_emitted {
                StreamEmission::default()
            } else {
                state.finish_emitted = true;
                state.finish_event.emission()
            };
            (finish_callbacks, state.lifecycle.clone())
        };
        let result = invoke_event(finish_callbacks, ());
        lifecycle.finish_writable();
        result?;
        lifecycle.emit_terminal()
    }

    pub(crate) fn mark_finished(&self) {
        self.state.borrow_mut().finished = true;
    }

    pub(crate) fn fail(&self, error: NodeError) -> Result<(), E> {
        self.destroy_result(Some(error.into()))
    }

    fn buffered_bytes_with(&self, state: &WritableState<E>) -> usize {
        state
            .backend
            .buffered_bytes()
            .saturating_add(state.corked_bytes)
    }
}

fn write_sink(sink: WritableSink, bytes: &[u8]) -> NodeResult<bool> {
    use std::io::Write as _;
    let result = match sink {
        WritableSink::Stdout => {
            let mut output = std::io::stdout().lock();
            output.write_all(bytes).and_then(|()| output.flush())
        }
        WritableSink::Stderr => {
            let mut output = std::io::stderr().lock();
            output.write_all(bytes).and_then(|()| output.flush())
        }
    };
    result
        .map(|()| true)
        .map_err(|error| NodeError::new("EIO", error.to_string()))
}

impl<E: 'static> Clone for Writable<E> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
}
impl<E: 'static> Clone for WeakWritable<E> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}
