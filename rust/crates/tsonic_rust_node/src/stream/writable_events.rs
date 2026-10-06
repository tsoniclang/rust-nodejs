impl<E: From<NodeError> + 'static> Writable<E> {
    pub(crate) fn on_drain_internal(&self, callback: impl Fn() -> Result<(), E> + 'static) {
        self.state
            .borrow_mut()
            .drain_event
            .add(empty_listener(&Callable::new(move |()| callback()), true));
    }

    pub fn on_drain(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.add_empty_listener(event, "drain", listener, false)
    }

    pub fn once_drain(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.add_empty_listener(event, "drain", listener, true)
    }

    pub fn off_drain(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.remove_empty_listener(event, "drain", listener.identity_key())
    }

    pub fn on_finish(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.add_empty_listener(event, "finish", listener, false)
    }

    pub fn once_finish(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.add_empty_listener(event, "finish", listener, true)
    }

    pub fn off_finish(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.remove_empty_listener(event, "finish", listener.identity_key())
    }

    pub fn on_close(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.add_empty_listener(event, "close", listener, false)
    }

    pub fn once_close(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.add_empty_listener(event, "close", listener, true)
    }

    pub fn off_close(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.remove_empty_listener(event, "close", listener.identity_key())
    }

    pub fn on_error(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<
            (tsonic_rust_runtime::RetainedError,),
            Result<(), E>,
        >,
    ) -> Result<Self, E> {
        self.add_error_listener(event, listener, false)
    }

    pub fn once_error(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<
            (tsonic_rust_runtime::RetainedError,),
            Result<(), E>,
        >,
    ) -> Result<Self, E> {
        self.add_error_listener(event, listener, true)
    }

    pub fn off_error(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<
            (tsonic_rust_runtime::RetainedError,),
            Result<(), E>,
        >,
    ) -> Result<Self, E> {
        ensure_stream_event(event, "error")?;
        self.state
            .borrow()
            .lifecycle
            .remove_error_listener(listener.identity_key());
        Ok(self.clone())
    }

    fn add_empty_listener(
        &self,
        event: &str,
        expected: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
        once: bool,
    ) -> Result<Self, E> {
        ensure_stream_event(event, expected)?;
        let entry = empty_listener(listener, once);
        let mut state = self.state.borrow_mut();
        match expected {
            "drain" => state.drain_event.add(entry),
            "finish" => state.finish_event.add(entry),
            "close" => state.lifecycle.add_close_listener(entry),
            _ => unreachable!("validated writable event"),
        }
        Ok(self.clone())
    }

    fn remove_empty_listener(
        &self,
        event: &str,
        expected: &str,
        identity: usize,
    ) -> Result<Self, E> {
        ensure_stream_event(event, expected)?;
        let mut state = self.state.borrow_mut();
        match expected {
            "drain" => state.drain_event.remove(identity),
            "finish" => state.finish_event.remove(identity),
            "close" => state.lifecycle.remove_close_listener(identity),
            _ => unreachable!("validated writable event"),
        }
        Ok(self.clone())
    }

    fn add_error_listener(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<
            (tsonic_rust_runtime::RetainedError,),
            Result<(), E>,
        >,
        once: bool,
    ) -> Result<Self, E> {
        ensure_stream_event(event, "error")?;
        self.state
            .borrow()
            .lifecycle
            .add_error_listener(value_listener(listener, once));
        Ok(self.clone())
    }
}
