use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadableSource {
    Stdin,
}

struct ReadableState<E: 'static> {
    chunks: VecDeque<Buffer>,
    queued_bytes: usize,
    options: StreamOptions,
    paused: bool,
    destroyed: bool,
    lifecycle: StreamLifecycle<E>,
    encoding: Option<String>,
    did_read: bool,
    data_event: StreamEvent<Buffer, E>,
    end_event: StreamEvent<(), E>,
    source: Option<ReadableSource>,
    producer_ended: bool,
    end_emitted: bool,
    capacity_handler: Option<Callable<(), Result<(), E>>>,
}

pub struct Readable<E: 'static = NodeError> {
    state: Rc<RefCell<ReadableState<E>>>,
}

pub(crate) struct WeakReadable<E: 'static> {
    state: Weak<RefCell<ReadableState<E>>>,
}

impl<E: 'static> WeakReadable<E> {
    pub(crate) fn upgrade(&self) -> Option<Readable<E>> {
        self.state.upgrade().map(|state| Readable { state })
    }
}

impl<E: 'static> Clone for WeakReadable<E> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl<E: 'static> std::fmt::Debug for Readable<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.borrow();
        formatter
            .debug_struct("Readable")
            .field("queued_bytes", &state.queued_bytes)
            .field("paused", &state.paused)
            .field("destroyed", &state.destroyed)
            .field("producer_ended", &state.producer_ended)
            .finish()
    }
}

impl<E: From<NodeError> + 'static> Default for Readable<E> {
    fn default() -> Self {
        Self::open(StreamOptions::default())
    }
}

impl<E: 'static> PartialEq for Readable<E> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}

impl<E: 'static> Eq for Readable<E> {}

impl<E: From<NodeError> + 'static> Readable<E> {
    pub(crate) fn open(options: StreamOptions) -> Self {
        let lifecycle = StreamLifecycle::<E>::new(options.emit_close);
        let readable = Self {
            state: Rc::new(RefCell::new(ReadableState::<E> {
                chunks: VecDeque::new(),
                queued_bytes: 0,
                options,
                paused: false,
                destroyed: false,
                lifecycle: lifecycle.clone(),
                encoding: None,
                did_read: false,
                data_event: StreamEvent::default(),
                end_event: StreamEvent::default(),
                source: None,
                producer_ended: false,
                end_emitted: false,
                capacity_handler: None,
            })),
        };
        lifecycle.bind_readable(&readable);
        readable
    }

    pub(crate) fn lifecycle(&self) -> StreamLifecycle<E> {
        self.state.borrow().lifecycle.clone()
    }

    pub fn from_chunks(chunks: Vec<Buffer>) -> Self {
        Self::from_chunks_with_options(chunks, StreamOptions::default())
    }

    pub fn stdin() -> Self {
        let readable = Self::open(StreamOptions::default());
        {
            let mut state = readable.state.borrow_mut();
            state.source = Some(ReadableSource::Stdin);
        }
        readable
    }

    pub(crate) fn is_stdin_source(&self) -> bool {
        matches!(self.state.borrow().source, Some(ReadableSource::Stdin))
    }

    pub fn from_chunks_with_options(chunks: Vec<Buffer>, options: StreamOptions) -> Self {
        let readable = Self::open(options);
        {
            let mut state = readable.state.borrow_mut();
            state.queued_bytes = chunks.iter().map(Buffer::len).sum();
            state.chunks = chunks.into();
            state.producer_ended = true;
        }
        readable
    }

    pub fn from(chunks: Vec<Buffer>) -> Self {
        Self::from_chunks(chunks)
    }

    pub fn from_source(chunks: &tsonic_rust_js::JsArray<Buffer>) -> Self {
        Self::from_chunks(chunks.values())
    }

    pub fn read(&self) -> Result<Option<Buffer>, E> {
        self.read_sized_result(None)
    }

    pub fn read_buffer(&self, size: Option<i32>) -> Result<Option<Buffer>, E> {
        let size = size
            .map(|value| {
                usize::try_from(value).map_err(|_| {
                    NodeError::new("ERR_OUT_OF_RANGE", "read size must be a positive integer")
                })
            })
            .transpose()?;
        if size == Some(0) {
            return Err(
                NodeError::new("ERR_OUT_OF_RANGE", "read size must be a positive integer").into(),
            );
        }
        self.read_sized_result(size)
    }

    fn read_sized_result(&self, size: Option<usize>) -> Result<Option<Buffer>, E> {
        let before_pressure = self.pressured();
        let chunk = {
            let mut state = self.state.borrow_mut();
            if state.destroyed {
                return Ok(None);
            }
            if state.chunks.is_empty() {
                drop(state);
                self.fill_stdin()?;
                state = self.state.borrow_mut();
            }
            let Some(size) = size else {
                let chunk = state.chunks.pop_front();
                if let Some(chunk) = &chunk {
                    state.queued_bytes = state.queued_bytes.saturating_sub(chunk.len());
                    state.did_read = true;
                }
                drop(state);
                self.after_consumption(before_pressure)?;
                return Ok(chunk);
            };
            take_sized_chunk(&mut state, size)
        };
        self.after_consumption(before_pressure)?;
        Ok(chunk)
    }

    fn fill_stdin(&self) -> Result<(), E> {
        let should_read = {
            let state = self.state.borrow();
            matches!(state.source, Some(ReadableSource::Stdin)) && !state.producer_ended
        };
        if !should_read {
            return Ok(());
        }
        use std::io::Read as _;
        let size = self.state.borrow().options.high_water_mark.max(1);
        let mut bytes = vec![0_u8; size];
        let read = std::io::stdin()
            .read(&mut bytes)
            .map_err(|error| NodeError::new("EIO", error.to_string()))?;
        if read == 0 {
            self.finish_input()?;
        } else {
            bytes.truncate(read);
            self.enqueue(Buffer::from_bytes(bytes))?;
        }
        Ok(())
    }

    fn after_consumption(&self, before_pressure: bool) -> Result<(), E> {
        if before_pressure && !self.pressured() {
            let handler = self.state.borrow().capacity_handler.clone();
            if let Some(handler) = handler {
                handler.call(())?;
            }
        }
        self.emit_terminal_if_ready()
    }

    pub fn is_ended(&self) -> bool {
        let state = self.state.borrow();
        state.producer_ended && state.chunks.is_empty()
    }

    pub fn readable(&self) -> bool {
        let state = self.state.borrow();
        !state.destroyed && (!state.producer_ended || !state.chunks.is_empty())
    }

    pub fn readable_ended(&self) -> bool {
        self.is_ended()
    }

    pub fn readable_length(&self) -> usize {
        self.state.borrow().chunks.len()
    }

    pub fn readable_high_water_mark(&self) -> usize {
        self.state.borrow().options.high_water_mark
    }

    pub fn readable_object_mode(&self) -> bool {
        self.state.borrow().options.object_mode
    }

    pub fn readable_flowing(&self) -> Option<bool> {
        let state = self.state.borrow();
        (!state.destroyed).then_some(!state.paused && !state.data_event.is_empty())
    }

    pub fn readable_did_read(&self) -> bool {
        self.state.borrow().did_read
    }

    pub fn readable_aborted(&self) -> bool {
        let state = self.state.borrow();
        state.destroyed && !state.producer_ended
    }

    pub fn readable_encoding(&self) -> Option<String> {
        self.state.borrow().encoding.clone()
    }

    pub fn pipe<W: WritableTarget<E> + Clone + 'static>(&self, writable: &W) -> Result<(), E> {
        install_pipeline(self.clone(), writable.clone())
    }

    pub fn pipe_to<W: WritableTarget<E> + Clone + 'static>(&self, writable: &W) -> Result<W, E> {
        self.pipe(writable)?;
        Ok(writable.clone())
    }

    pub fn unpipe(&self, _writable: &Writable<E>) -> Result<(), E> {
        Ok(())
    }

    pub fn destroy(&self) -> Result<(), E> {
        self.destroy_result(None)
    }

    pub fn destroy_with_error(&self, error: impl Into<String>) -> Result<(), E> {
        self.destroy_result(Some(NodeError::new("ERR_STREAM_DESTROYED", error).into()))
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

    pub(crate) fn destroy_before_terminal(
        &self,
        error: Option<tsonic_rust_runtime::RetainedError>,
        before_terminal: impl FnOnce() -> Result<(), E>,
    ) -> Result<(), E> {
        let lifecycle = self.state.borrow().lifecycle.clone();
        lifecycle.destroy_before_terminal(error, before_terminal)
    }

    fn destroy_storage(&self) {
        let mut state = self.state.borrow_mut();
        state.destroyed = true;
        state.chunks.clear();
        state.queued_bytes = 0;
        let capacity_handler = state.capacity_handler.take();
        drop(state);
        drop(capacity_handler);
    }

    pub fn destroyed(&self) -> bool {
        self.state.borrow().destroyed
    }

    pub fn closed(&self) -> bool {
        self.state.borrow().lifecycle.closed()
    }

    pub fn errored(&self) -> Option<String> {
        self.state.borrow().lifecycle.errored()
    }

    pub fn pause(&self) -> Self {
        self.state.borrow_mut().paused = true;
        self.clone()
    }

    pub fn pause_chain(&self) -> Self {
        self.pause()
    }

    pub fn resume(&self) -> Result<Self, E> {
        self.state.borrow_mut().paused = false;
        self.pump_flowing()?;
        Ok(self.clone())
    }

    pub fn resume_chain(&self) -> Result<Self, E> {
        self.resume()
    }

    pub fn is_paused(&self) -> bool {
        self.state.borrow().paused
    }

    pub fn set_encoding(&self, encoding: &str) {
        self.state.borrow_mut().encoding = Some(encoding.to_ascii_lowercase());
    }

    pub fn push(&self, chunk: Buffer) -> Result<bool, E> {
        self.enqueue(chunk)
    }

    pub(crate) fn enqueue(&self, chunk: Buffer) -> Result<bool, E> {
        if !self.buffer_input(Some(chunk), false) {
            return Ok(false);
        }
        self.pump_flowing()?;
        Ok(!self.pressured())
    }

    pub(crate) fn buffer_input(&self, chunk: Option<Buffer>, complete: bool) -> bool {
        let mut state = self.state.borrow_mut();
        if state.destroyed || (state.producer_ended && chunk.is_some()) {
            return false;
        }
        if let Some(chunk) = chunk {
            state.queued_bytes = state.queued_bytes.saturating_add(chunk.len());
            state.chunks.push_back(chunk);
        }
        state.producer_ended |= complete;
        true
    }

    pub(crate) fn downgrade(&self) -> WeakReadable<E> {
        WeakReadable {
            state: Rc::downgrade(&self.state),
        }
    }

    pub fn unshift(&self, chunk: Buffer) {
        let mut state = self.state.borrow_mut();
        state.queued_bytes = state.queued_bytes.saturating_add(chunk.len());
        state.chunks.push_front(chunk);
    }

    pub(crate) fn finish_input(&self) -> Result<(), E> {
        self.buffer_input(None, true);
        self.pump_flowing()
    }

    pub(crate) fn fail_input(&self, error: NodeError) -> Result<(), E> {
        self.destroy_result(Some(error.into()))
    }

    pub(crate) fn set_capacity_handler(&self, handler: impl Fn() -> Result<(), E> + 'static) {
        let previous = self
            .state
            .borrow_mut()
            .capacity_handler
            .replace(Callable::new(move |()| handler()));
        drop(previous);
    }

    pub(crate) fn pressured(&self) -> bool {
        let state = self.state.borrow();
        state.queued_bytes >= state.options.high_water_mark.max(1)
    }

    pub(crate) fn pump_flowing(&self) -> Result<(), E> {
        loop {
            let (chunk, callbacks, before_pressure) = {
                let mut state = self.state.borrow_mut();
                if state.paused
                    || state.destroyed
                    || state.data_event.is_empty()
                    || state.chunks.is_empty()
                {
                    break;
                }
                let before_pressure = state.queued_bytes >= state.options.high_water_mark.max(1);
                let chunk = state.chunks.pop_front().expect("checked nonempty queue");
                state.queued_bytes = state.queued_bytes.saturating_sub(chunk.len());
                state.did_read = true;
                let callbacks = state.data_event.emission();
                (chunk, callbacks, before_pressure)
            };
            invoke_event(callbacks, chunk)?;
            self.after_consumption(before_pressure)?;
        }
        self.emit_terminal_if_ready()
    }

    fn emit_terminal_if_ready(&self) -> Result<(), E> {
        let (end_callbacks, lifecycle) = {
            let mut state = self.state.borrow_mut();
            if state.destroyed || !state.producer_ended || !state.chunks.is_empty() {
                return Ok(());
            }
            let end_callbacks = if state.end_emitted {
                StreamEmission::default()
            } else {
                state.end_emitted = true;
                state.end_event.emission()
            };
            (end_callbacks, state.lifecycle.clone())
        };
        let result = invoke_event(end_callbacks, ());
        lifecycle.finish_readable();
        result?;
        lifecycle.emit_terminal()
    }

    fn drain_remaining(&self) -> Result<Vec<Buffer>, E> {
        let mut output = Vec::new();
        while let Some(chunk) = self.read()? {
            output.push(chunk);
        }
        Ok(output)
    }
}

fn take_sized_chunk<E: 'static>(state: &mut ReadableState<E>, size: usize) -> Option<Buffer> {
    if state.chunks.is_empty() {
        return None;
    }
    let available = state.queued_bytes.min(size);
    if available == 0 {
        return None;
    }
    let first_len = state.chunks.front().map(Buffer::len).unwrap_or_default();
    if first_len == available {
        let chunk = state.chunks.pop_front();
        state.queued_bytes = state.queued_bytes.saturating_sub(available);
        state.did_read = true;
        return chunk;
    }
    if first_len > available {
        let first = state.chunks.pop_front().expect("checked nonempty queue");
        let selected = first.subarray(0, Some(available as isize));
        state
            .chunks
            .push_front(first.subarray(available as isize, None));
        state.queued_bytes = state.queued_bytes.saturating_sub(available);
        state.did_read = true;
        return Some(selected);
    }

    let mut parts = Vec::new();
    let mut remaining = available;
    while remaining > 0 {
        let chunk = state
            .chunks
            .pop_front()
            .expect("available bytes are queued");
        if chunk.len() <= remaining {
            remaining -= chunk.len();
            parts.push(chunk);
        } else {
            parts.push(chunk.subarray(0, Some(remaining as isize)));
            state
                .chunks
                .push_front(chunk.subarray(remaining as isize, None));
            remaining = 0;
        }
    }
    state.queued_bytes = state.queued_bytes.saturating_sub(available);
    state.did_read = true;
    Some(Buffer::concat_dense(&parts))
}

fn ensure_stream_event<E: From<NodeError>>(actual: &str, expected: &str) -> Result<(), E> {
    if actual == expected {
        Ok(())
    } else {
        Err(NodeError::new(
            "ERR_INVALID_ARG_VALUE",
            format!("expected stream event {expected}, received {actual}"),
        )
        .into())
    }
}

impl<E: 'static> Readable<E> {
    pub(crate) fn queued_bytes(&self) -> usize {
        self.state.borrow().queued_bytes
    }
}

impl<E: 'static> Clone for Readable<E> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
}
