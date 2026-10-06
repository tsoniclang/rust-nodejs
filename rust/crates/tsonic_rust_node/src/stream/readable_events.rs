impl<E: From<NodeError> + 'static> Readable<E> {
    pub(crate) fn on_data_internal(
        &self,
        callback: impl Fn(Buffer) -> Result<(), E> + 'static,
    ) -> Result<(), E> {
        self.state.borrow_mut().data_event.add(value_listener(
            &Callable::new(move |(value,)| callback(value)),
            false,
        ));
        self.pump_flowing()
    }

    pub(crate) fn on_end_internal(&self, callback: impl Fn() -> Result<(), E> + 'static) {
        self.state
            .borrow_mut()
            .end_event
            .add(empty_listener(&Callable::new(move |()| callback()), true));
    }

    pub fn on_data(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> Result<Self, E> {
        self.add_data_listener(event, listener, false)
    }

    pub fn once_data(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> Result<Self, E> {
        self.add_data_listener(event, listener, true)
    }

    pub fn off_data(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
    ) -> Result<Self, E> {
        ensure_stream_event(event, "data")?;
        self.state
            .borrow_mut()
            .data_event
            .remove(listener.identity_key());
        Ok(self.clone())
    }

    pub fn on_end(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.add_empty_listener(event, "end", listener, false)
    }

    pub fn once_end(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.add_empty_listener(event, "end", listener, true)
    }

    pub fn off_end(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> Result<Self, E> {
        self.remove_empty_listener(event, "end", listener.identity_key())
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

    fn add_data_listener(
        &self,
        event: &str,
        listener: &tsonic_rust_runtime::Callable<(Buffer,), Result<(), E>>,
        once: bool,
    ) -> Result<Self, E> {
        ensure_stream_event(event, "data")?;
        self.state
            .borrow_mut()
            .data_event
            .add(value_listener(listener, once));
        self.pump_flowing()?;
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
        let already_emitted = {
            let mut state = self.state.borrow_mut();
            if expected == "end" {
                let emitted = state.end_emitted;
                if !emitted {
                    state.end_event.add(entry);
                }
                emitted
            } else {
                state.lifecycle.add_close_listener(entry);
                false
            }
        };
        if already_emitted && once {
            listener.call(())?;
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
        if expected == "end" {
            state.end_event.remove(identity);
        } else {
            state.lifecycle.remove_close_listener(identity);
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
