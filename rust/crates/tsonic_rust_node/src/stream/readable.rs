use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadableSource {
    Stdin,
}

struct ReadableState {
    chunks: VecDeque<Buffer>,
    queued_bytes: usize,
    options: StreamOptions,
    paused: bool,
    destroyed: bool,
    errored: Option<NodeError>,
    encoding: Option<String>,
    did_read: bool,
    data_event: StreamEvent<Buffer>,
    end_event: StreamEvent<()>,
    error_event: StreamEvent<NodeError>,
    close_event: StreamEvent<()>,
    source: Option<ReadableSource>,
    producer_ended: bool,
    end_emitted: bool,
    close_emitted: bool,
    capacity_handler: Option<Rc<dyn Fn() -> NodeResult<()>>>,
}

#[derive(Clone)]
pub struct Readable {
    state: Rc<RefCell<ReadableState>>,
}

impl std::fmt::Debug for Readable {
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

impl Default for Readable {
    fn default() -> Self {
        Self::open(StreamOptions::default())
    }
}

impl PartialEq for Readable {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}

impl Eq for Readable {}

impl Readable {
    pub(crate) fn open(options: StreamOptions) -> Self {
        Self {
            state: Rc::new(RefCell::new(ReadableState {
                chunks: VecDeque::new(),
                queued_bytes: 0,
                options,
                paused: false,
                destroyed: false,
                errored: None,
                encoding: None,
                did_read: false,
                data_event: StreamEvent::default(),
                end_event: StreamEvent::default(),
                error_event: StreamEvent::default(),
                close_event: StreamEvent::default(),
                source: None,
                producer_ended: false,
                end_emitted: false,
                close_emitted: false,
                capacity_handler: None,
            })),
        }
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

    pub fn from_source(chunks: &tsonic_rust_js::JsArray<Buffer>) -> NodeResult<Self> {
        Ok(Self::from_chunks(chunks.values()))
    }

    pub fn read(&self) -> Option<Buffer> {
        match self.read_result() {
            Ok(value) => value,
            Err(error) => {
                let _ = self.fail_input(error);
                None
            }
        }
    }

    pub fn read_buffer(&self, size: Option<i32>) -> NodeResult<Option<Buffer>> {
        let size = size
            .map(|value| {
                usize::try_from(value).map_err(|_| {
                    NodeError::new("ERR_OUT_OF_RANGE", "read size must be a positive integer")
                })
            })
            .transpose()?;
        if size == Some(0) {
            return Err(NodeError::new(
                "ERR_OUT_OF_RANGE",
                "read size must be a positive integer",
            ));
        }
        self.read_sized_result(size)
    }

    pub fn read_result(&self) -> NodeResult<Option<Buffer>> {
        self.read_sized_result(None)
    }

    fn read_sized_result(&self, size: Option<usize>) -> NodeResult<Option<Buffer>> {
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

    fn fill_stdin(&self) -> NodeResult<()> {
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

    fn after_consumption(&self, before_pressure: bool) -> NodeResult<()> {
        if before_pressure && !self.pressured() {
            let handler = self.state.borrow().capacity_handler.clone();
            if let Some(handler) = handler {
                handler()?;
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

    pub(crate) fn queued_bytes(&self) -> usize {
        self.state.borrow().queued_bytes
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

    pub fn pipe<W: WritableTarget + Clone + 'static>(&self, writable: &W) -> NodeResult<()> {
        install_pipeline(self.clone(), writable.clone())
    }

    pub fn pipe_to<W: WritableTarget + Clone + 'static>(&self, writable: &W) -> NodeResult<W> {
        self.pipe(writable)?;
        Ok(writable.clone())
    }

    pub fn unpipe(&self, _writable: &Writable) -> NodeResult<()> {
        Ok(())
    }

    pub fn destroy(&self) {
        let _ = self.destroy_result(None);
    }

    pub fn destroy_with_error(&self, error: impl Into<String>) {
        let _ = self.destroy_result(Some(NodeError::new("ERR_STREAM_DESTROYED", error)));
    }

    pub fn destroy_chain(&self, error: Option<NodeError>) -> NodeResult<Self> {
        self.destroy_result(error)?;
        Ok(self.clone())
    }

    fn destroy_result(&self, error: Option<NodeError>) -> NodeResult<()> {
        let (error_callbacks, close_callbacks, emitted_error) = {
            let mut state = self.state.borrow_mut();
            if state.destroyed {
                return Ok(());
            }
            state.destroyed = true;
            state.chunks.clear();
            state.queued_bytes = 0;
            let emitted_error = error.or_else(|| state.errored.clone());
            state.errored = emitted_error.clone();
            let error_callbacks = emitted_error
                .as_ref()
                .map(|_| state.error_event.emission())
                .unwrap_or_default();
            let close_callbacks = if state.close_emitted {
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

    pub fn destroyed(&self) -> bool {
        self.state.borrow().destroyed
    }

    pub fn closed(&self) -> bool {
        self.state.borrow().close_emitted
    }

    pub fn errored(&self) -> Option<String> {
        self.state
            .borrow()
            .errored
            .as_ref()
            .map(ToString::to_string)
    }

    pub fn pause(&self) -> Self {
        self.state.borrow_mut().paused = true;
        self.clone()
    }

    pub fn pause_chain(&self) -> Self {
        self.pause()
    }

    pub fn resume(&self) -> Self {
        self.state.borrow_mut().paused = false;
        let _ = self.pump_flowing();
        self.clone()
    }

    pub fn resume_chain(&self) -> Self {
        self.resume()
    }

    pub fn is_paused(&self) -> bool {
        self.state.borrow().paused
    }

    pub fn set_encoding(&self, encoding: &str) {
        self.state.borrow_mut().encoding = Some(encoding.to_ascii_lowercase());
    }

    pub fn push(&self, chunk: Buffer) -> bool {
        self.enqueue(chunk).unwrap_or(false)
    }

    pub(crate) fn enqueue(&self, chunk: Buffer) -> NodeResult<bool> {
        {
            let mut state = self.state.borrow_mut();
            if state.destroyed || state.producer_ended {
                return Ok(false);
            }
            state.queued_bytes = state.queued_bytes.saturating_add(chunk.len());
            state.chunks.push_back(chunk);
        }
        self.pump_flowing()?;
        Ok(!self.pressured())
    }

    pub fn unshift(&self, chunk: Buffer) {
        let mut state = self.state.borrow_mut();
        state.queued_bytes = state.queued_bytes.saturating_add(chunk.len());
        state.chunks.push_front(chunk);
    }

    pub fn wrap(readable: Readable) -> Self {
        readable
    }

    pub fn iterator(&self) -> Vec<Buffer> {
        self.drain_remaining()
    }

    pub fn take(&self, limit: usize) -> Vec<Buffer> {
        let mut output = Vec::new();
        while output.len() < limit {
            let Some(chunk) = self.read() else {
                break;
            };
            output.push(chunk);
        }
        output
    }

    pub fn drop(&self, limit: usize) -> Vec<Buffer> {
        for _ in 0..limit {
            if self.read().is_none() {
                break;
            }
        }
        self.drain_remaining()
    }

    pub fn map(&self, mapper: impl Fn(Buffer) -> Buffer) -> Readable {
        let options = self.state.borrow().options.clone();
        Readable::from_chunks_with_options(
            self.drain_remaining().into_iter().map(mapper).collect(),
            options,
        )
    }

    pub fn filter(&self, predicate: impl Fn(&Buffer) -> bool) -> Readable {
        let options = self.state.borrow().options.clone();
        Readable::from_chunks_with_options(
            self.drain_remaining().into_iter().filter(predicate).collect(),
            options,
        )
    }

    pub fn flat_map(&self, mapper: impl Fn(Buffer) -> Vec<Buffer>) -> Readable {
        let options = self.state.borrow().options.clone();
        Readable::from_chunks_with_options(
            self.drain_remaining().into_iter().flat_map(mapper).collect(),
            options,
        )
    }

    pub fn for_each(&self, mut callback: impl FnMut(Buffer)) {
        while let Some(chunk) = self.read() {
            callback(chunk);
        }
    }

    pub fn every(&self, predicate: impl Fn(&Buffer) -> bool) -> bool {
        self.drain_remaining().iter().all(predicate)
    }

    pub fn some(&self, predicate: impl Fn(&Buffer) -> bool) -> bool {
        self.drain_remaining().iter().any(predicate)
    }

    pub fn find(&self, predicate: impl Fn(&Buffer) -> bool) -> Option<Buffer> {
        self.drain_remaining().into_iter().find(predicate)
    }

    pub fn reduce<T>(&self, initial: T, reducer: impl Fn(T, Buffer) -> T) -> T {
        self.drain_remaining().into_iter().fold(initial, reducer)
    }

    pub fn compose(self, next: impl Fn(Readable) -> Readable) -> Readable {
        next(self)
    }

    pub fn to_array(&self) -> Vec<Buffer> {
        self.drain_remaining()
    }

    pub fn to_vec(self) -> Vec<Buffer> {
        self.drain_remaining()
    }

    pub(crate) fn finish_input(&self) -> NodeResult<()> {
        self.state.borrow_mut().producer_ended = true;
        self.pump_flowing()?;
        self.emit_terminal_if_ready()
    }

    pub(crate) fn fail_input(&self, error: NodeError) -> NodeResult<()> {
        self.destroy_result(Some(error))
    }

    pub(crate) fn set_capacity_handler(
        &self,
        handler: impl Fn() -> NodeResult<()> + 'static,
    ) {
        self.state.borrow_mut().capacity_handler = Some(Rc::new(handler));
    }

    pub(crate) fn pressured(&self) -> bool {
        let state = self.state.borrow();
        state.queued_bytes >= state.options.high_water_mark.max(1)
    }

    pub(crate) fn on_data_internal(
        &self,
        callback: impl Fn(Buffer) -> NodeResult<()> + 'static,
    ) -> NodeResult<()> {
        self.state.borrow_mut().data_event.add(StreamListener {
            identity: 0,
            once: false,
            callback: Rc::new(callback),
        });
        self.pump_flowing()
    }

    pub(crate) fn on_end_internal(&self, callback: impl Fn() -> NodeResult<()> + 'static) {
        self.state.borrow_mut().end_event.add(StreamListener {
            identity: 0,
            once: true,
            callback: Rc::new(move |()| callback()),
        });
    }

    pub fn on_data<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.add_data_listener(event, listener, false)
    }

    pub fn once_data<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.add_data_listener(event, listener, true)
    }

    pub fn off_data<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> NodeResult<Self> {
        ensure_stream_event(event, "data")?;
        self.state
            .borrow_mut()
            .data_event
            .remove(listener.identity_key());
        Ok(self.clone())
    }

    pub fn on_end<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.add_empty_listener(event, "end", listener, false)
    }

    pub fn once_end<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.add_empty_listener(event, "end", listener, true)
    }

    pub fn off_end<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.remove_empty_listener(event, "end", listener.identity_key())
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

    fn add_data_listener<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
        once: bool,
    ) -> NodeResult<Self> {
        ensure_stream_event(event, "data")?;
        self.state
            .borrow_mut()
            .data_event
            .add(value_listener(listener, once));
        self.pump_flowing()?;
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
        let already_emitted = {
            let mut state = self.state.borrow_mut();
            if expected == "end" {
                let emitted = state.end_emitted;
                if !emitted {
                    state.end_event.add(entry);
                }
                emitted
            } else {
                let emitted = state.close_emitted;
                if !emitted {
                    state.close_event.add(entry);
                }
                emitted
            }
        };
        if already_emitted && once {
            listener
                .call(())
                .map_err(crate::error::callback_node_error)?;
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
        if expected == "end" {
            state.end_event.remove(identity);
        } else {
            state.close_event.remove(identity);
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

    fn pump_flowing(&self) -> NodeResult<()> {
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
                let before_pressure =
                    state.queued_bytes >= state.options.high_water_mark.max(1);
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

    fn emit_terminal_if_ready(&self) -> NodeResult<()> {
        let (end_callbacks, close_callbacks) = {
            let mut state = self.state.borrow_mut();
            if state.destroyed || !state.producer_ended || !state.chunks.is_empty() {
                return Ok(());
            }
            let end_callbacks = if state.end_emitted {
                Vec::new()
            } else {
                state.end_emitted = true;
                state.end_event.emission()
            };
            let close_callbacks = if !state.options.emit_close || state.close_emitted {
                Vec::new()
            } else {
                state.close_emitted = true;
                state.close_event.emission()
            };
            (end_callbacks, close_callbacks)
        };
        invoke_event(end_callbacks, ())?;
        invoke_event(close_callbacks, ())
    }

    fn drain_remaining(&self) -> Vec<Buffer> {
        let mut output = Vec::new();
        while let Some(chunk) = self.read() {
            output.push(chunk);
        }
        output
    }
}

fn take_sized_chunk(state: &mut ReadableState, size: usize) -> Option<Buffer> {
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
        let chunk = state.chunks.pop_front().expect("available bytes are queued");
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

fn ensure_stream_event(actual: &str, expected: &str) -> NodeResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(NodeError::new(
            "ERR_INVALID_ARG_VALUE",
            format!("expected stream event {expected}, received {actual}"),
        ))
    }
}
