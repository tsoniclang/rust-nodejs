#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WritableSink {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Writable {
    chunks: Vec<Buffer>,
    ended: bool,
    options: StreamOptions,
    destroyed: bool,
    errored: Option<String>,
    corked: usize,
    need_drain: bool,
    events: StreamEventState,
    sink: Option<WritableSink>,
}

impl Writable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_options(options: StreamOptions) -> Self {
        Self {
            options,
            ..Self::default()
        }
    }

    pub fn stdout() -> Self {
        Self {
            sink: Some(WritableSink::Stdout),
            ..Self::default()
        }
    }

    pub fn stderr() -> Self {
        Self {
            sink: Some(WritableSink::Stderr),
            ..Self::default()
        }
    }

    pub fn write(&mut self, chunk: Buffer) -> bool {
        if self.ended || self.destroyed {
            return false;
        }
        if let Some(sink) = self.sink {
            return chunk
                .with_bytes(|bytes| write_sink(sink, bytes))
                .is_ok();
        }
        self.chunks.push(chunk);
        self.need_drain = self.chunks.len() >= self.options.high_water_mark;
        !self.need_drain
    }

    pub fn write_string(&mut self, value: &str) -> NodeResult<bool> {
        let buffer = Buffer::from_string(value, Some("utf8"))?;
        self.write_buffer(&buffer)
    }

    pub fn write_buffer(&mut self, value: &Buffer) -> NodeResult<bool> {
        if let Some(sink) = self.sink {
            return value.with_bytes(|bytes| write_sink(sink, bytes));
        }
        Ok(self.write(value.clone()))
    }

    pub fn is_tty(&self) -> bool {
        use std::io::IsTerminal as _;
        match self.sink {
            Some(WritableSink::Stdout) => std::io::stdout().is_terminal(),
            Some(WritableSink::Stderr) => std::io::stderr().is_terminal(),
            None => false,
        }
    }

    pub fn fd(&self) -> i32 {
        match self.sink {
            Some(WritableSink::Stdout) => 1,
            Some(WritableSink::Stderr) => 2,
            None => -1,
        }
    }

    pub fn writev(&mut self, chunks: &[Buffer]) -> bool {
        let mut ok = true;
        for chunk in chunks {
            ok = self.write(chunk.clone()) && ok;
        }
        ok
    }

    pub fn cork(&mut self) {
        self.corked += 1;
    }

    pub fn uncork(&mut self) {
        self.corked = self.corked.saturating_sub(1);
    }

    pub fn writable_corked(&self) -> usize {
        self.corked
    }

    pub fn set_default_encoding(&mut self, encoding: &str) {
        self.options.default_encoding = encoding.to_ascii_lowercase();
    }

    pub fn default_encoding(&self) -> &str {
        &self.options.default_encoding
    }

    pub fn writable_high_water_mark(&self) -> usize {
        self.options.high_water_mark
    }

    pub fn writable_object_mode(&self) -> bool {
        self.options.object_mode
    }

    pub fn writable_length(&self) -> usize {
        self.chunks.len()
    }

    pub fn writable_need_drain(&self) -> bool {
        self.need_drain
    }

    pub fn writable(&self) -> bool {
        !self.ended && !self.destroyed
    }

    pub fn writable_ended(&self) -> bool {
        self.ended
    }

    pub fn writable_finished(&self) -> bool {
        self.ended && !self.destroyed
    }

    pub fn writable_aborted(&self) -> bool {
        self.destroyed && !self.ended
    }

    pub fn emit_close(&self) -> bool {
        self.options.emit_close
    }

    pub fn errored(&self) -> Option<&str> {
        self.errored.as_deref()
    }

    pub fn closed(&self) -> bool {
        self.ended || self.destroyed
    }

    pub fn destroyed(&self) -> bool {
        self.destroyed
    }

    pub fn clear_drain(&mut self) {
        self.need_drain = false;
    }

    pub fn final_callback(&mut self, callback: impl FnOnce()) {
        self.end();
        callback();
    }

    pub fn construct_callback(&self, callback: impl FnOnce()) {
        callback();
    }

    pub fn add_listener(&mut self, event: &str) -> &mut Self {
        self.events.add_listener(event);
        self
    }

    pub fn on(&mut self, event: &str) -> &mut Self {
        self.add_listener(event)
    }

    pub fn once(&mut self, event: &str) -> &mut Self {
        self.add_listener(event)
    }

    pub fn prepend_listener(&mut self, event: &str) -> &mut Self {
        self.add_listener(event)
    }

    pub fn prepend_once_listener(&mut self, event: &str) -> &mut Self {
        self.add_listener(event)
    }

    pub fn remove_listener(&mut self, event: &str) -> &mut Self {
        self.events.remove_listener(event);
        self
    }

    pub fn off(&mut self, event: &str) -> &mut Self {
        self.remove_listener(event)
    }

    pub fn remove_all_listeners(&mut self, event: Option<&str>) -> &mut Self {
        self.events.remove_all_listeners(event);
        self
    }

    pub fn listeners(&self, event: &str) -> Vec<String> {
        self.events.listeners(event)
    }

    pub fn raw_listeners(&self, event: &str) -> Vec<String> {
        self.events.listeners(event)
    }

    pub fn listener_count(&self, event: &str) -> usize {
        self.events.listener_count(event)
    }

    pub fn event_names(&self) -> Vec<String> {
        self.events.event_names()
    }

    pub fn emit(&self, event: &str) -> bool {
        self.events.emit(event)
    }

    pub fn destroy_with_error(&mut self, error: impl Into<String>) {
        self.errored = Some(error.into());
        self.destroy();
    }

    pub fn add_chunk(&mut self, chunk: Buffer) -> bool {
        self.write(chunk)
    }

    pub fn write_str(&mut self, value: &str, encoding: Option<&str>) -> bool {
        match Buffer::from_string(value, encoding) {
            Ok(buffer) => self.write(buffer),
            Err(_) => false,
        }
    }

    pub fn flush(&mut self) -> bool {
        self.clear_drain();
        true
    }

    pub fn end(&mut self) -> &mut Self {
        self.ended = true;
        self
    }

    pub fn end_string(&mut self, value: &str) -> NodeResult<&mut Self> {
        self.write_string(value)?;
        Ok(self.end())
    }

    pub fn end_buffer(&mut self, value: &Buffer) -> NodeResult<&mut Self> {
        self.write_buffer(value)?;
        Ok(self.end())
    }

    pub fn destroy(&mut self) {
        self.destroyed = true;
        self.ended = true;
    }

    pub fn is_ended(&self) -> bool {
        self.ended
    }

    pub fn chunks(&self) -> &[Buffer] {
        &self.chunks
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
        .map_err(|error| crate::NodeError::new("EIO", error.to_string()))
}
