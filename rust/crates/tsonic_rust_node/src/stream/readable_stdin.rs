impl<E: From<NodeError> + 'static> Readable<E> {
    #[cfg(target_os = "linux")]
    pub(crate) fn attach_stdin(&self, input: crate::readiness::Stdin) {
        self.state.borrow_mut().source = Some(ReadableSource::Stdin(Box::new(input)));
    }

    pub(crate) fn mark_stdin(&self, lease: tsonic_rust_runtime::dispatch_queue::TaskReservation) {
        self.state.borrow_mut().source = Some(ReadableSource::StdinFile { _lease: lease });
    }

    pub(crate) fn wants_stdin_input(&self) -> bool {
        let state = self.state.borrow();
        state.source.is_some()
            && !state.destroyed
            && !state.producer_ended
            && !state.paused
            && state.end_requested
            && !readable_pressure(&state)
    }

    pub(crate) fn prepare_stdin(&self) -> Result<(), E> {
        #[cfg(target_os = "linux")]
        {
            let active = self.wants_stdin_input();
            let result = {
                let mut state = self.state.borrow_mut();
                let capacity = state
                    .options
                    .high_water_mark
                    .max(1)
                    .saturating_sub(state.queued_bytes)
                    .max(1);
                match &mut state.source {
                    Some(ReadableSource::Stdin(input)) => input.prepare(active, capacity),
                    _ => Ok(()),
                }
            };
            result.map_err(E::from)?;
        }
        Ok(())
    }

    pub(crate) fn poll_stdin(&self) -> Result<bool, E> {
        if self.wants_stdin_input() {
            self.fill_stdin()
        } else {
            Ok(false)
        }
    }

    fn fill_stdin(&self) -> Result<bool, E> {
        #[cfg(target_os = "linux")]
        {
            let result = {
                let mut state = self.state.borrow_mut();
                if state.destroyed || state.producer_ended {
                    return Ok(false);
                }
                let capacity = state
                    .options
                    .high_water_mark
                    .max(1)
                    .saturating_sub(state.queued_bytes)
                    .max(1);
                match &mut state.source {
                    Some(ReadableSource::Stdin(input)) => input.read(capacity),
                    _ => Ok(None),
                }
            };
            let bytes = match result {
                Ok(bytes) => bytes,
                Err(error) => {
                    self.fail_input(error.clone())?;
                    return Err(E::from(error));
                }
            };
            if let Some(bytes) = bytes {
                if bytes.is_empty() {
                    self.finish_input()?;
                } else {
                    self.enqueue(Buffer::from_bytes(bytes))?;
                }
                return Ok(true);
            }
        }
        Ok(false)
    }
}
