
#[derive(Clone)]
pub(crate) struct WeakWritable {
    state: Weak<RefCell<WritableState>>,
}

impl WeakWritable {
    pub(crate) fn upgrade(&self) -> Option<Writable> {
        self.state.upgrade().map(|state| Writable { state })
    }
}

pub(crate) trait WritableBackend {
    fn bind(&self, _owner: WeakWritable) {}
    fn write(&self, chunk: Buffer) -> NodeResult<()>;
    fn flush(&self) -> NodeResult<()> {
        Ok(())
    }
    fn finish(&self) -> NodeResult<bool>;
    fn destroy(&self) -> NodeResult<()>;
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

impl WritableBackend for MemoryWritableBackend {
    fn write(&self, chunk: Buffer) -> NodeResult<()> {
        self.chunks.borrow_mut().push(chunk);
        Ok(())
    }

    fn finish(&self) -> NodeResult<bool> {
        Ok(true)
    }

    fn destroy(&self) -> NodeResult<()> {
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

impl WritableBackend for TerminalWritableBackend {
    fn write(&self, chunk: Buffer) -> NodeResult<()> {
        chunk.with_bytes(|bytes| write_sink(self.sink, bytes))?;
        Ok(())
    }

    fn finish(&self) -> NodeResult<bool> {
        Ok(true)
    }

    fn destroy(&self) -> NodeResult<()> {
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

struct WritableState {
    options: StreamOptions,
    backend: Rc<dyn WritableBackend>,
    ending: bool,
    finished: bool,
    destroyed: bool,
    errored: Option<NodeError>,
    corked: usize,
    corked_chunks: Vec<Buffer>,
    corked_bytes: usize,
    need_drain: bool,
    drain_event: StreamEvent<()>,
    finish_event: StreamEvent<()>,
    error_event: StreamEvent<NodeError>,
    close_event: StreamEvent<()>,
    close_emitted: bool,
}

#[derive(Clone)]
pub struct Writable {
    state: Rc<RefCell<WritableState>>,
}

impl std::fmt::Debug for Writable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.borrow();
        formatter
            .debug_struct("Writable")
            .field("ending", &state.ending)
            .field("finished", &state.finished)
            .field("destroyed", &state.destroyed)
            .field("buffered_bytes", &self.buffered_bytes_with(&state))
            .finish()
    }
}

impl Default for Writable {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for Writable {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}

impl Eq for Writable {}

impl Writable {
    pub fn new() -> Self {
        Self::with_backend(
            StreamOptions::default(),
            Rc::new(MemoryWritableBackend::default()),
        )
    }

    pub fn with_options(options: StreamOptions) -> Self {
        Self::with_backend(options, Rc::new(MemoryWritableBackend::default()))
    }

    pub(crate) fn with_backend(
        options: StreamOptions,
        backend: Rc<dyn WritableBackend>,
    ) -> Self {
        let writable = Self {
            state: Rc::new(RefCell::new(WritableState {
                options,
                backend,
                ending: false,
                finished: false,
                destroyed: false,
                errored: None,
                corked: 0,
                corked_chunks: Vec::new(),
                corked_bytes: 0,
                need_drain: false,
                drain_event: StreamEvent::default(),
                finish_event: StreamEvent::default(),
                error_event: StreamEvent::default(),
                close_event: StreamEvent::default(),
                close_emitted: false,
            })),
        };
        writable
            .state
            .borrow()
            .backend
            .bind(writable.downgrade());
        writable
    }

    pub fn stdout() -> Self {
        Self::with_backend(
            StreamOptions::default(),
            Rc::new(TerminalWritableBackend {
                sink: WritableSink::Stdout,
            }),
        )
    }

    pub fn stderr() -> Self {
        Self::with_backend(
            StreamOptions::default(),
            Rc::new(TerminalWritableBackend {
                sink: WritableSink::Stderr,
            }),
        )
    }

    pub fn write(&self, chunk: Buffer) -> bool {
        self.write_owned(chunk).unwrap_or(false)
    }

    fn write_owned(&self, chunk: Buffer) -> NodeResult<bool> {
        let backend = {
            let mut state = self.state.borrow_mut();
            if state.ending || state.destroyed {
                return Err(NodeError::new(
                    "ERR_STREAM_WRITE_AFTER_END",
                    "write after end",
                ));
            }
            if state.corked > 0 {
                state.corked_bytes = state.corked_bytes.saturating_add(chunk.len());
                state.corked_chunks.push(chunk);
                let pressured = self.buffered_bytes_with(&state)
                    >= state.options.high_water_mark.max(1);
                state.need_drain |= pressured;
                return Ok(!pressured);
            }
            Rc::clone(&state.backend)
        };
        backend.write(chunk)?;
        let mut state = self.state.borrow_mut();
        let pressured = self.buffered_bytes_with(&state) >= state.options.high_water_mark.max(1);
        state.need_drain |= pressured;
        Ok(!pressured)
    }

    pub fn write_string(&self, value: &str) -> NodeResult<bool> {
        self.write_owned(Buffer::from_string(value, Some("utf8"))?)
    }

    pub fn write_buffer(&self, value: &Buffer) -> NodeResult<bool> {
        self.write_owned(value.clone())
    }

    pub fn is_tty(&self) -> bool {
        self.state.borrow().backend.is_tty()
    }

    pub fn fd(&self) -> i32 {
        self.state.borrow().backend.fd()
    }

    pub fn writev(&self, chunks: &[Buffer]) -> bool {
        let mut writable = true;
        for chunk in chunks {
            writable = self.write(chunk.clone()) && writable;
        }
        writable
    }

    pub fn cork(&self) {
        self.state.borrow_mut().corked += 1;
    }

    pub fn uncork(&self) {
        let chunks = {
            let mut state = self.state.borrow_mut();
            state.corked = state.corked.saturating_sub(1);
            if state.corked != 0 {
                return;
            }
            state.corked_bytes = 0;
            std::mem::take(&mut state.corked_chunks)
        };
        for chunk in chunks {
            if let Err(error) = self.write_owned(chunk) {
                let _ = self.fail(error);
                break;
            }
        }
        if let Err(error) = self.poll_progress() {
            let _ = self.fail(error);
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
        self.state
            .borrow()
            .errored
            .as_ref()
            .map(ToString::to_string)
    }

    pub fn closed(&self) -> bool {
        self.state.borrow().close_emitted
    }

    pub fn destroyed(&self) -> bool {
        self.state.borrow().destroyed
    }

    pub fn clear_drain(&self) {
        let callbacks = {
            let mut state = self.state.borrow_mut();
            if !state.need_drain {
                return;
            }
            state.need_drain = false;
            state.drain_event.emission()
        };
        let _ = invoke_event(callbacks, ());
    }

    pub fn final_callback(&self, callback: impl FnOnce()) {
        let _ = self.end_result();
        callback();
    }

    pub fn construct_callback(&self, callback: impl FnOnce()) {
        callback();
    }

    pub fn destroy_with_error(&self, error: impl Into<String>) {
        let _ = self.fail(NodeError::new("ERR_STREAM_DESTROYED", error));
    }

    pub fn add_chunk(&self, chunk: Buffer) -> bool {
        self.write(chunk)
    }

    pub fn write_str(&self, value: &str, encoding: Option<&str>) -> bool {
        match Buffer::from_string(value, encoding) {
            Ok(buffer) => self.write(buffer),
            Err(_) => false,
        }
    }

    pub fn flush(&self) -> bool {
        self.clear_drain();
        true
    }

    pub fn end(&self) -> Self {
        let _ = self.end_result();
        self.clone()
    }

    pub(crate) fn end_checked(&self) -> NodeResult<()> {
        self.end_result()
    }

    pub(crate) fn flush_backend(&self) -> NodeResult<()> {
        let backend = {
            let state = self.state.borrow();
            if state.ending || state.destroyed {
                return Err(NodeError::new(
                    "ERR_STREAM_WRITE_AFTER_END",
                    "flush after end",
                ));
            }
            Rc::clone(&state.backend)
        };
        backend.flush()
    }

    pub fn end_string(&self, value: &str) -> NodeResult<Self> {
        self.write_string(value)?;
        self.end_result()?;
        Ok(self.clone())
    }

    pub fn end_buffer(&self, value: &Buffer) -> NodeResult<Self> {
        self.write_buffer(value)?;
        self.end_result()?;
        Ok(self.clone())
    }

    fn end_result(&self) -> NodeResult<()> {
        let chunks = {
            let mut state = self.state.borrow_mut();
            if state.ending || state.destroyed {
                return Ok(());
            }
            state.ending = true;
            state.corked = 0;
            state.corked_bytes = 0;
            std::mem::take(&mut state.corked_chunks)
        };
        let backend = self.state.borrow().backend.clone();
        for chunk in chunks {
            backend.write(chunk)?;
        }
        if backend.finish()? {
            self.complete_finish()?;
        }
        Ok(())
    }

    pub fn destroy(&self) {
        let _ = self.destroy_result(None);
    }

    pub fn destroy_chain(&self, error: Option<NodeError>) -> NodeResult<Self> {
        self.destroy_result(error)?;
        Ok(self.clone())
    }

    fn destroy_result(&self, error: Option<NodeError>) -> NodeResult<()> {
        let backend = {
            let mut state = self.state.borrow_mut();
            if state.destroyed {
                return Ok(());
            }
            state.destroyed = true;
            state.ending = true;
            state.corked_chunks.clear();
            state.corked_bytes = 0;
            state.backend.clone()
        };
        backend.destroy()?;
        let (error_callbacks, close_callbacks, emitted_error) = {
            let mut state = self.state.borrow_mut();
            let emitted_error = error.or_else(|| state.errored.clone());
            state.errored = emitted_error.clone();
            let error_callbacks = emitted_error
                .as_ref()
                .map(|_| state.error_event.emission())
                .unwrap_or_default();
            let close_callbacks = if !state.options.emit_close || state.close_emitted {
                Vec::new()
            } else {
                state.close_emitted = true;
                state.close_event.emission()
            };
            (error_callbacks, close_callbacks, emitted_error)
        };
        if let Some(error) = emitted_error {
            invoke_event(error_callbacks, error)?;
        }
        invoke_event(close_callbacks, ())
    }

    pub fn is_ended(&self) -> bool {
        self.writable_ended()
    }

    pub fn chunks(&self) -> Vec<Buffer> {
        self.state.borrow().backend.chunks()
    }

    pub(crate) fn downgrade(&self) -> WeakWritable {
        WeakWritable {
            state: Rc::downgrade(&self.state),
        }
    }

    pub(crate) fn poll_progress(&self) -> NodeResult<()> {
        let callbacks = {
            let mut state = self.state.borrow_mut();
            let below_mark =
                self.buffered_bytes_with(&state) < state.options.high_water_mark.max(1);
            if state.need_drain && below_mark {
                state.need_drain = false;
                state.drain_event.emission()
            } else {
                Vec::new()
            }
        };
        invoke_event(callbacks, ())
    }

    pub(crate) fn complete_finish(&self) -> NodeResult<()> {
        let (finish_callbacks, close_callbacks) = {
            let mut state = self.state.borrow_mut();
            if state.finished || state.destroyed {
                return Ok(());
            }
            state.finished = true;
            let finish_callbacks = state.finish_event.emission();
            let close_callbacks = if !state.options.emit_close || state.close_emitted {
                Vec::new()
            } else {
                state.close_emitted = true;
                state.close_event.emission()
            };
            (finish_callbacks, close_callbacks)
        };
        invoke_event(finish_callbacks, ())?;
        invoke_event(close_callbacks, ())
    }

    pub(crate) fn fail(&self, error: NodeError) -> NodeResult<()> {
        self.destroy_result(Some(error))
    }

    pub(crate) fn on_drain_internal(&self, callback: impl Fn() -> NodeResult<()> + 'static) {
        self.state.borrow_mut().drain_event.add(StreamListener {
            identity: 0,
            once: true,
            callback: Rc::new(move |()| callback()),
        });
    }

    pub fn on_drain<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.add_empty_listener(event, "drain", listener, false)
    }

    pub fn once_drain<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.add_empty_listener(event, "drain", listener, true)
    }

    pub fn off_drain<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.remove_empty_listener(event, "drain", listener.identity_key())
    }

    pub fn on_finish<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.add_empty_listener(event, "finish", listener, false)
    }

    pub fn once_finish<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.add_empty_listener(event, "finish", listener, true)
    }

    pub fn off_finish<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.remove_empty_listener(event, "finish", listener.identity_key())
    }

    pub fn on_close<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.add_empty_listener(event, "close", listener, false)
    }

    pub fn once_close<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.add_empty_listener(event, "close", listener, true)
    }

    pub fn off_close<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.remove_empty_listener(event, "close", listener.identity_key())
    }

    pub fn on_error<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(NodeError,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.add_error_listener(event, listener, false)
    }

    pub fn once_error<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(NodeError,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.add_error_listener(event, listener, true)
    }

    pub fn off_error<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(NodeError,), Result<(), E>>,
    ) -> NodeResult<Self> {
        ensure_stream_event(event, "error")?;
        self.state
            .borrow_mut()
            .error_event
            .remove(listener.identity_key());
        Ok(self.clone())
    }

    fn add_empty_listener<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        expected: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
        once: bool,
    ) -> NodeResult<Self> {
        ensure_stream_event(event, expected)?;
        let entry = empty_listener(listener, once);
        let mut state = self.state.borrow_mut();
        match expected {
            "drain" => state.drain_event.add(entry),
            "finish" => state.finish_event.add(entry),
            "close" => state.close_event.add(entry),
            _ => unreachable!("validated writable event"),
        }
        Ok(self.clone())
    }

    fn remove_empty_listener(
        &self,
        event: &str,
        expected: &str,
        identity: usize,
    ) -> NodeResult<Self> {
        ensure_stream_event(event, expected)?;
        let mut state = self.state.borrow_mut();
        match expected {
            "drain" => state.drain_event.remove(identity),
            "finish" => state.finish_event.remove(identity),
            "close" => state.close_event.remove(identity),
            _ => unreachable!("validated writable event"),
        }
        Ok(self.clone())
    }

    fn add_error_listener<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(NodeError,), Result<(), E>>,
        once: bool,
    ) -> NodeResult<Self> {
        ensure_stream_event(event, "error")?;
        self.state
            .borrow_mut()
            .error_event
            .add(value_listener(listener, once));
        Ok(self.clone())
    }

    fn buffered_bytes_with(&self, state: &WritableState) -> usize {
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
