impl Duplex {
    pub fn on_drain<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.on_drain(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_drain<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.once_drain(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_drain<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.off_drain(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_finish<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.on_finish(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_finish<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.once_finish(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_finish<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.off_finish(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_close<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.on_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_close<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.once_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_close<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.off_close(event, listener)?;
        Ok(self.clone())
    }

    pub fn on_error<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(tsonic_rust_runtime::RetainedError,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.on_error(event, listener)?;
        Ok(self.clone())
    }

    pub fn once_error<E: std::fmt::Display + 'static>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(tsonic_rust_runtime::RetainedError,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.once_error(event, listener)?;
        Ok(self.clone())
    }

    pub fn off_error<E>(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(tsonic_rust_runtime::RetainedError,), Result<(), E>>,
    ) -> NodeResult<Self> {
        self.writable.off_error(event, listener)?;
        Ok(self.clone())
    }
}
