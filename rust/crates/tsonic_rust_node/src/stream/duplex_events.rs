impl<E: From<NodeError> + 'static> Duplex<E> {
    pub fn on_drain(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.on_drain(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_drain(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.once_drain(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_drain(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.off_drain(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_finish(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.on_finish(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_finish(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.once_finish(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_finish(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.off_finish(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_close(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.on_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_close(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.once_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_close(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.writable.off_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_error(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<
            (tsonic_rust_runtime::RetainedError,),
            Result<(), E>,
        >,
    ) -> Result<Self, E> {
        self.writable.on_error(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_error(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<
            (tsonic_rust_runtime::RetainedError,),
            Result<(), E>,
        >,
    ) -> Result<Self, E> {
        self.writable.once_error(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_error(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<
            (tsonic_rust_runtime::RetainedError,),
            Result<(), E>,
        >,
    ) -> Result<Self, E> {
        self.writable.off_error(event, listener)?;
        Ok(self.clone())
    }
}
