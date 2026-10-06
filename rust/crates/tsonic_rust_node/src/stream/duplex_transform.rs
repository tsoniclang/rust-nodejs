pub struct Duplex<E: 'static = NodeError> {
    readable: Readable<E>,
    writable: Writable<E>,
    allow_half_open: bool,
}

impl<E: From<NodeError> + 'static> Default for Duplex<E> {
    fn default() -> Self {
        let readable = Readable::<E>::default();
        let writable = Writable::<E>::with_backend(
            StreamOptions::default(),
            Rc::new(MemoryWritableBackend::default()),
            Some(readable.lifecycle()),
        );
        Self::new(readable, writable)
    }
}

impl<E: From<NodeError> + 'static> Duplex<E> {
    pub fn new(readable: Readable<E>, writable: Writable<E>) -> Self {
        StreamLifecycle::<E>::join(&readable, &writable);
        Self {
            readable,
            writable,
            allow_half_open: false,
        }
    }

    pub fn with_options(
        readable: Readable<E>,
        writable: Writable<E>,
        options: DuplexOptions,
    ) -> Self {
        StreamLifecycle::<E>::join(&readable, &writable);
        for _ in 0..options.writable_corked {
            writable.cork();
        }
        Self {
            readable,
            writable,
            allow_half_open: options.allow_half_open,
        }
    }

    pub fn readable_handle(&self) -> Readable<E> {
        self.readable.clone()
    }

    pub fn writable_handle(&self) -> Writable<E> {
        self.writable.clone()
    }

    pub fn read(&self) -> Result<Option<Buffer>, E> {
        self.readable.read()
    }

    pub fn on_data(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> Result<Self, E> {
        self.readable.on_data(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_data(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> Result<Self, E> {
        self.readable.once_data(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_data(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> Result<Self, E> {
        self.readable.off_data(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_end(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.readable.on_end(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_end(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.readable.once_end(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_end(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.readable.off_end(event, listener)?;
        Ok(self.clone())
    }

    pub fn write(&self, chunk: Buffer) -> Result<bool, E> {
        self.writable.write(chunk)
    }

    pub fn write_string(&self, chunk: &str) -> Result<bool, E> {
        self.writable.write_string(chunk)
    }

    pub fn write_buffer(&self, chunk: &Buffer) -> Result<bool, E> {
        self.writable.write_buffer(chunk)
    }

    pub fn end(&self) -> Result<Self, E> {
        self.writable.end()?;
        Ok(self.clone())
    }

    pub fn end_string(&self, chunk: &str) -> Result<Self, E> {
        self.writable.end_string(chunk)?;
        Ok(self.clone())
    }

    pub fn end_buffer(&self, chunk: &Buffer) -> Result<Self, E> {
        self.writable.end_buffer(chunk)?;
        Ok(self.clone())
    }

    pub fn cork(&self) {
        self.writable.cork();
    }

    pub fn uncork(&self) -> Result<(), E> {
        self.writable.uncork()
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

    pub fn destroy(&self) -> Result<(), E> {
        self.writable.destroy()
    }

    pub fn destroy_chain(
        &self,
        error: Option<tsonic_rust_runtime::RetainedError>,
    ) -> Result<Self, E> {
        self.writable.destroy_chain(error)?;
        Ok(self.clone())
    }
}

impl<E: From<NodeError> + 'static> WritableTarget<E> for Duplex<E> {
    fn writable_handle(&self) -> Writable<E> {
        self.writable.clone()
    }
}

struct TransformBackend<E: 'static> {
    transform: fn(Buffer) -> Buffer,
    readable: Readable<E>,
}

impl<E: From<NodeError> + 'static> WritableBackend<E> for TransformBackend<E> {
    fn bind(&self, owner: WeakWritable<E>) {
        self.readable.set_capacity_handler(move || {
            owner
                .upgrade()
                .map_or(Ok(()), |writable| writable.poll_progress())
        });
    }

    fn write(&self, chunk: Buffer) -> StreamBackendResult<(), E> {
        self.readable
            .enqueue((self.transform)(chunk))
            .map_err(StreamBackendFailure::Callback)?;
        Ok(())
    }

    fn finish(&self) -> StreamBackendResult<bool, E> {
        self.readable
            .finish_input()
            .map_err(StreamBackendFailure::Callback)?;
        Ok(true)
    }

    fn finish_accepted(&self) -> bool {
        self.readable.state.borrow().producer_ended
    }

    fn destroy(&self) -> StreamBackendResult<(), E> {
        self.readable
            .destroy()
            .map_err(StreamBackendFailure::Callback)
    }

    fn buffered_bytes(&self) -> usize {
        self.readable.queued_bytes()
    }
}

pub struct Transform<E: 'static = NodeError> {
    inner: Duplex<E>,
}

impl<E: From<NodeError> + 'static> Transform<E> {
    pub fn new(transform: fn(Buffer) -> Buffer) -> Self {
        let readable = Readable::<E>::default();
        let writable = Writable::<E>::with_backend(
            StreamOptions::default(),
            Rc::new(TransformBackend::<E> {
                transform,
                readable: readable.clone(),
            }),
            Some(readable.lifecycle()),
        );
        Self {
            inner: Duplex::<E>::new(readable, writable),
        }
    }

    pub(crate) fn from_parts(readable: Readable<E>, writable: Writable<E>) -> Self {
        Self {
            inner: Duplex::<E>::new(readable, writable),
        }
    }

    pub fn duplex_handle(&self) -> Duplex<E> {
        self.inner.clone()
    }

    pub fn readable_handle(&self) -> Readable<E> {
        self.inner.readable_handle()
    }

    pub fn writable_handle(&self) -> Writable<E> {
        self.inner.writable_handle()
    }

    pub fn write(&self, chunk: Buffer) -> Result<bool, E> {
        self.inner.write(chunk)
    }

    pub fn write_string(&self, chunk: &str) -> Result<bool, E> {
        self.inner.write_string(chunk)
    }

    pub fn write_buffer(&self, chunk: &Buffer) -> Result<bool, E> {
        self.inner.write_buffer(chunk)
    }

    pub fn read(&self) -> Result<Option<Buffer>, E> {
        self.inner.read()
    }

    pub fn end(&self) -> Result<Self, E> {
        self.inner.end()?;
        Ok(self.clone())
    }
}

impl<E: From<NodeError> + 'static> WritableTarget<E> for Transform<E> {
    fn writable_handle(&self) -> Writable<E> {
        self.inner.writable_handle()
    }
}

pub struct PassThrough<E: 'static = NodeError> {
    inner: Transform<E>,
}

impl<E: From<NodeError> + 'static> Default for PassThrough<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: From<NodeError> + 'static> PassThrough<E> {
    pub fn new() -> Self {
        Self {
            inner: Transform::<E>::new(|chunk| chunk),
        }
    }

    pub fn write(&self, chunk: Buffer) -> Result<bool, E> {
        self.inner.write(chunk)
    }

    pub fn read(&self) -> Result<Option<Buffer>, E> {
        self.inner.read()
    }

    pub fn end(&self) -> Result<(), E> {
        self.inner.end().map(|_| ())
    }
}

impl<E: From<NodeError> + 'static> WritableTarget<E> for PassThrough<E> {
    fn writable_handle(&self) -> Writable<E> {
        self.inner.writable_handle()
    }
}

pub fn duplex_as_readable<E: From<NodeError> + 'static>(value: &Duplex<E>) -> Readable<E> {
    value.readable_handle()
}

pub fn duplex_as_writable<E: From<NodeError> + 'static>(value: &Duplex<E>) -> Writable<E> {
    value.writable_handle()
}

pub fn duplex_as_stream<E: From<NodeError> + 'static>(value: &Duplex<E>) -> Stream<E> {
    readable_as_stream(&value.readable_handle())
}

pub fn transform_as_duplex<E: From<NodeError> + 'static>(value: &Transform<E>) -> Duplex<E> {
    value.duplex_handle()
}

pub fn transform_as_readable<E: From<NodeError> + 'static>(value: &Transform<E>) -> Readable<E> {
    value.duplex_handle().readable_handle()
}

pub fn transform_as_writable<E: From<NodeError> + 'static>(value: &Transform<E>) -> Writable<E> {
    value.duplex_handle().writable_handle()
}

pub fn transform_as_stream<E: From<NodeError> + 'static>(value: &Transform<E>) -> Stream<E> {
    readable_as_stream(&value.duplex_handle().readable_handle())
}

impl<E: 'static> Clone for Duplex<E> {
    fn clone(&self) -> Self {
        Self {
            readable: self.readable.clone(),
            writable: self.writable.clone(),
            allow_half_open: self.allow_half_open,
        }
    }
}
impl<E: 'static> std::fmt::Debug for Duplex<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Duplex")
            .field("readable", &self.readable)
            .field("writable", &self.writable)
            .field("allow_half_open", &self.allow_half_open)
            .finish()
    }
}
impl<E: 'static> PartialEq for Duplex<E> {
    fn eq(&self, other: &Self) -> bool {
        self.readable == other.readable
            && self.writable == other.writable
            && self.allow_half_open == other.allow_half_open
    }
}
impl<E: 'static> Eq for Duplex<E> {}

impl<E: 'static> Clone for Transform<E> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
impl<E: 'static> std::fmt::Debug for Transform<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("Transform")
            .field(&self.inner)
            .finish()
    }
}
impl<E: 'static> PartialEq for Transform<E> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}
impl<E: 'static> Eq for Transform<E> {}

impl<E: 'static> Clone for PassThrough<E> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
impl<E: 'static> std::fmt::Debug for PassThrough<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("PassThrough")
            .field(&self.inner)
            .finish()
    }
}
impl<E: 'static> PartialEq for PassThrough<E> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}
impl<E: 'static> Eq for PassThrough<E> {}
