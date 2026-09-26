
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Duplex {
    readable: Readable,
    writable: Writable,
    allow_half_open: bool,
}

impl Default for Duplex {
    fn default() -> Self {
        Self::new(Readable::default(), Writable::new())
    }
}

impl Duplex {
    pub fn new(readable: Readable, writable: Writable) -> Self {
        Self {
            readable,
            writable,
            allow_half_open: false,
        }
    }

    pub fn with_options(
        readable: Readable,
        writable: Writable,
        options: DuplexOptions,
    ) -> Self {
        for _ in 0..options.writable_corked {
            writable.cork();
        }
        Self {
            readable,
            writable,
            allow_half_open: options.allow_half_open,
        }
    }

    pub fn readable_handle(&self) -> Readable {
        self.readable.clone()
    }

    pub fn writable_handle(&self) -> Writable {
        self.writable.clone()
    }

    pub fn read(&self) -> Option<Buffer> {
        self.readable.read()
    }

    pub fn on_data<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.on_data(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_data<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.once_data(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_data<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.off_data(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_end<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.on_end(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_end<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.once_end(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_end<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.readable.off_end(event, listener)?;
        Ok(self.clone())
    }

    pub fn write(&self, chunk: Buffer) -> bool {
        self.writable.write(chunk)
    }

    pub fn write_string(&self, chunk: &str) -> NodeResult<bool> {
        self.writable.write_string(chunk)
    }

    pub fn write_buffer(&self, chunk: &Buffer) -> NodeResult<bool> {
        self.writable.write_buffer(chunk)
    }

    pub fn end(&self) -> Self {
        self.writable.end();
        self.clone()
    }

    pub fn end_string(&self, chunk: &str) -> NodeResult<Self> {
        self.writable.end_string(chunk)?;
        Ok(self.clone())
    }

    pub fn end_buffer(&self, chunk: &Buffer) -> NodeResult<Self> {
        self.writable.end_buffer(chunk)?;
        Ok(self.clone())
    }

    pub fn cork(&self) {
        self.writable.cork();
    }

    pub fn uncork(&self) {
        self.writable.uncork();
    }

    pub fn writable_chunks(&self) -> Vec<Buffer> {
        self.writable.chunks()
    }

    pub fn allow_half_open(&self) -> bool {
        self.allow_half_open
    }

    pub fn readable(&self) -> bool {
        self.readable.readable()
    }

    pub fn readable_ended(&self) -> bool {
        self.readable.readable_ended()
    }

    pub fn writable(&self) -> bool {
        self.writable.writable()
    }

    pub fn writable_ended(&self) -> bool {
        self.writable.writable_ended()
    }

    pub fn writable_finished(&self) -> bool {
        self.writable.writable_finished()
    }

    pub fn writable_need_drain(&self) -> bool {
        self.writable.writable_need_drain()
    }

    pub fn destroyed(&self) -> bool {
        self.readable.destroyed() || self.writable.destroyed()
    }

    pub fn destroy(&self) {
        self.readable.destroy();
        self.writable.destroy();
    }

    pub fn destroy_chain(&self, error: Option<NodeError>) -> NodeResult<Self> {
        self.readable.destroy_chain(error.clone())?;
        self.writable.destroy_chain(error)?;
        Ok(self.clone())
    }

}

impl WritableTarget for Duplex {
    fn writable_handle(&self) -> Writable {
        self.writable.clone()
    }
}

struct TransformBackend {
    transform: fn(Buffer) -> Buffer,
    readable: Readable,
}

impl WritableBackend for TransformBackend {
    fn write(&self, chunk: Buffer) -> NodeResult<()> {
        self.readable.enqueue((self.transform)(chunk))?;
        Ok(())
    }

    fn finish(&self) -> NodeResult<bool> {
        self.readable.finish_input()?;
        Ok(true)
    }

    fn destroy(&self) -> NodeResult<()> {
        self.readable.destroy();
        Ok(())
    }

    fn buffered_bytes(&self) -> usize {
        self.readable.queued_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transform {
    inner: Duplex,
}

impl Transform {
    pub fn new(transform: fn(Buffer) -> Buffer) -> Self {
        let readable = Readable::default();
        let writable = Writable::with_backend(
            StreamOptions::default(),
            Rc::new(TransformBackend {
                transform,
                readable: readable.clone(),
            }),
        );
        Self {
            inner: Duplex::new(readable, writable),
        }
    }

    pub(crate) fn from_parts(readable: Readable, writable: Writable) -> Self {
        Self {
            inner: Duplex::new(readable, writable),
        }
    }

    pub fn duplex_handle(&self) -> Duplex {
        self.inner.clone()
    }

    pub fn readable_handle(&self) -> Readable {
        self.inner.readable_handle()
    }

    pub fn writable_handle(&self) -> Writable {
        self.inner.writable_handle()
    }

    pub fn write(&self, chunk: Buffer) -> bool {
        self.inner.write(chunk)
    }

    pub fn write_string(&self, chunk: &str) -> NodeResult<bool> {
        self.inner.write_string(chunk)
    }

    pub fn write_buffer(&self, chunk: &Buffer) -> NodeResult<bool> {
        self.inner.write_buffer(chunk)
    }

    pub fn read(&self) -> Option<Buffer> {
        self.inner.read()
    }

    pub fn end(&self) -> Self {
        self.inner.end();
        self.clone()
    }

    pub(crate) fn end_checked(&self) -> NodeResult<()> {
        self.inner.writable_handle().end_checked()
    }
}

impl WritableTarget for Transform {
    fn writable_handle(&self) -> Writable {
        self.inner.writable_handle()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassThrough {
    inner: Transform,
}

impl Default for PassThrough {
    fn default() -> Self {
        Self::new()
    }
}

impl PassThrough {
    pub fn new() -> Self {
        Self {
            inner: Transform::new(|chunk| chunk),
        }
    }

    pub fn write(&self, chunk: Buffer) -> bool {
        self.inner.write(chunk)
    }

    pub fn read(&self) -> Option<Buffer> {
        self.inner.read()
    }

    pub fn end(&self) {
        self.inner.end();
    }
}

impl WritableTarget for PassThrough {
    fn writable_handle(&self) -> Writable {
        self.inner.writable_handle()
    }
}

pub fn duplex_as_readable(value: &Duplex) -> Readable {
    value.readable_handle()
}

pub fn duplex_as_writable(value: &Duplex) -> Writable {
    value.writable_handle()
}

pub fn duplex_as_stream(value: &Duplex) -> Stream {
    readable_as_stream(&value.readable_handle())
}

pub fn transform_as_duplex(value: &Transform) -> Duplex {
    value.duplex_handle()
}

pub fn transform_as_readable(value: &Transform) -> Readable {
    value.duplex_handle().readable_handle()
}

pub fn transform_as_writable(value: &Transform) -> Writable {
    value.duplex_handle().writable_handle()
}

pub fn transform_as_stream(value: &Transform) -> Stream {
    readable_as_stream(&value.duplex_handle().readable_handle())
}
