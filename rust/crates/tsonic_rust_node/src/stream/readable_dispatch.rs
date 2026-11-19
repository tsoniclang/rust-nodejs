impl<E: From<NodeError> + 'static> Readable<E> {
    fn schedule_dispatch(&self) -> Result<(), E> {
        let tasks = {
            let mut state = self.state.borrow_mut();
            let can_flow = state.flowing && !state.paused && !state.chunks.is_empty();
            let can_end = state.end_requested
                && state.producer_ended
                && state.chunks.is_empty()
                && (!state.end_emitted || state.lifecycle.has_pending_close());
            if state.destroyed || state.dispatch_pending || (!can_flow && !can_end) {
                return Ok(());
            }
            state.dispatch_pending = true;
            state.tasks.clone()
        };
        let readable = self.clone();
        if let Err(error) = tasks.enqueue(move || {
            let result = readable.dispatch();
            readable.state.borrow_mut().dispatch_pending = false;
            result
        }) {
            self.state.borrow_mut().dispatch_pending = false;
            return Err(NodeError::from(error).into());
        }
        Ok(())
    }

    fn dispatch(&self) -> Result<(), E> {
        loop {
            let (chunk, callbacks, before_pressure) = {
                let mut state = self.state.borrow_mut();
                if state.paused || state.destroyed || !state.flowing || state.chunks.is_empty() {
                    break;
                }
                let before_pressure = readable_pressure(&state);
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
        let (end_callbacks, lifecycle, data_callbacks) = {
            let mut state = self.state.borrow_mut();
            if state.destroyed
                || !state.end_requested
                || !state.producer_ended
                || !state.chunks.is_empty()
            {
                return Ok(());
            }
            let end_callbacks = if state.end_emitted {
                StreamEmission::default()
            } else {
                state.end_emitted = true;
                state.end_event.take_emission()
            };
            (
                end_callbacks,
                state.lifecycle.clone(),
                state.data_event.take_emission(),
            )
        };
        drop(data_callbacks);
        let result = invoke_event(end_callbacks, ());
        lifecycle.finish_readable();
        result?;
        lifecycle.emit_terminal()
    }
}
