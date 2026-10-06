pub(crate) struct StreamLifecycle<E: 'static> {
    state: Rc<RefCell<StreamLifecycleState<E>>>,
}

struct StreamLifecycleState<E: 'static> {
    readable: Option<Weak<RefCell<ReadableState<E>>>>,
    writable: Option<Weak<RefCell<WritableState<E>>>>,
    readable_closed: bool,
    writable_closed: bool,
    emit_close: bool,
    destroying: bool,
    errored: Option<tsonic_rust_runtime::RetainedError>,
    error_emitted: bool,
    close_emitted: bool,
    error_event: StreamEvent<tsonic_rust_runtime::RetainedError, E>,
    close_event: StreamEvent<(), E>,
}

impl<E: From<NodeError> + 'static> StreamLifecycle<E> {
    fn new(emit_close: bool) -> Self {
        Self {
            state: Rc::new(RefCell::new(StreamLifecycleState::<E> {
                readable: None,
                writable: None,
                readable_closed: true,
                writable_closed: true,
                emit_close,
                destroying: false,
                errored: None,
                error_emitted: false,
                close_emitted: false,
                error_event: StreamEvent::default(),
                close_event: StreamEvent::default(),
            })),
        }
    }

    fn bind_readable(&self, readable: &Readable<E>) {
        let mut state = self.state.borrow_mut();
        state.readable = Some(Rc::downgrade(&readable.state));
        state.readable_closed = false;
    }

    fn bind_writable(&self, writable: &Writable<E>) {
        let mut state = self.state.borrow_mut();
        state.writable = Some(Rc::downgrade(&writable.state));
        state.writable_closed = false;
        state.emit_close |= writable.state.borrow().options.emit_close;
    }

    fn join(readable: &Readable<E>, writable: &Writable<E>) {
        let lifecycle = readable.state.borrow().lifecycle.clone();
        let previous = writable.state.borrow().lifecycle.clone();
        if Rc::ptr_eq(&lifecycle.state, &previous.state) {
            return;
        }
        {
            let mut state = lifecycle.state.borrow_mut();
            let mut previous = previous.state.borrow_mut();
            state.writable = Some(Rc::downgrade(&writable.state));
            state.writable_closed = previous.writable_closed;
            state.emit_close |= previous.emit_close;
            state.destroying |= previous.destroying;
            state.error_emitted |= previous.error_emitted;
            state.close_emitted |= previous.close_emitted;
            if state.errored.is_none() {
                state.errored = previous.errored.take();
            }
            state.error_event.append(&mut previous.error_event);
            state.close_event.append(&mut previous.close_event);
        }
        writable.state.borrow_mut().lifecycle = lifecycle;
    }

    fn destroy(&self, error: Option<tsonic_rust_runtime::RetainedError>) -> Result<(), E> {
        self.destroy_before_terminal(error, || Ok(()))
    }

    fn destroy_before_terminal(
        &self,
        error: Option<tsonic_rust_runtime::RetainedError>,
        before_terminal: impl FnOnce() -> Result<(), E>,
    ) -> Result<(), E> {
        let completed = {
            let state = self.state.borrow();
            state
                .destroying
                .then_some(state.readable_closed && state.writable_closed)
        };
        if let Some(completed) = completed {
            return if completed {
                before_terminal()?;
                self.emit_terminal()
            } else {
                Ok(())
            };
        }
        let (readable, writable) = {
            let mut state = self.state.borrow_mut();
            state.destroying = true;
            state.errored = error.or_else(|| state.errored.clone());
            (
                state.readable.as_ref().and_then(Weak::upgrade),
                state.writable.as_ref().and_then(Weak::upgrade),
            )
        };
        if let Some(state) = readable {
            Readable::<E> { state }.destroy_storage();
        }
        let cleanup = writable.map_or(Ok(()), |state| Writable::<E> { state }.destroy_storage());
        {
            let mut state = self.state.borrow_mut();
            state.readable_closed = true;
            state.writable_closed = true;
            if state.errored.is_none() {
                if let Err(StreamBackendFailure::Native(error)) = &cleanup {
                    state.errored = Some(error.clone().into());
                }
            }
        }
        let native_failure = match cleanup {
            Err(StreamBackendFailure::Callback(error)) => return Err(error),
            Err(StreamBackendFailure::Native(error)) => Some(error),
            Ok(()) => None,
        };
        before_terminal()?;
        self.emit_terminal()?;
        native_failure.map_or(Ok(()), |error| Err(error.into()))
    }

    fn finish_readable(&self) {
        self.state.borrow_mut().readable_closed = true;
    }

    fn finish_writable(&self) {
        self.state.borrow_mut().writable_closed = true;
    }

    fn emit_terminal(&self) -> Result<(), E> {
        let (error, error_callbacks) = {
            let mut state = self.state.borrow_mut();
            let readable_released = state
                .readable
                .as_ref()
                .is_none_or(|owner| owner.strong_count() == 0);
            let writable_released = state
                .writable
                .as_ref()
                .is_none_or(|owner| owner.strong_count() == 0);
            state.readable_closed |= readable_released;
            state.writable_closed |= writable_released;
            let error = if state.error_emitted {
                None
            } else {
                state.errored.clone()
            };
            let error_callbacks = if error.is_some() {
                state.error_emitted = true;
                state.error_event.emission()
            } else {
                StreamEmission::default()
            };
            (error, error_callbacks)
        };
        if let Some(error) = error {
            invoke_event(error_callbacks, error)?;
        }
        let close_callbacks = {
            let mut state = self.state.borrow_mut();
            if state.emit_close
                && state.readable_closed
                && state.writable_closed
                && !state.close_emitted
            {
                state.close_emitted = true;
                state.close_event.emission()
            } else {
                StreamEmission::default()
            }
        };
        invoke_event(close_callbacks, ())
    }

    fn errored(&self) -> Option<String> {
        self.state
            .borrow()
            .errored
            .as_ref()
            .map(ToString::to_string)
    }

    fn closed(&self) -> bool {
        let state = self.state.borrow();
        state.readable_closed && state.writable_closed
    }

    fn add_error_listener(&self, listener: StreamListener<tsonic_rust_runtime::RetainedError, E>) {
        self.state.borrow_mut().error_event.add(listener);
    }

    fn remove_error_listener(&self, identity: usize) {
        self.state.borrow_mut().error_event.remove(identity);
    }

    fn add_close_listener(&self, listener: StreamListener<(), E>) {
        self.state.borrow_mut().close_event.add(listener);
    }

    fn remove_close_listener(&self, identity: usize) {
        self.state.borrow_mut().close_event.remove(identity);
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::{Duplex, Readable, Transform, Writable};
    use std::rc::Rc;

    fn assert_shared_lifecycle(duplex: &Duplex) {
        let readable = duplex.readable_handle();
        let writable = duplex.writable_handle();
        assert!(Rc::ptr_eq(
            &readable.state.borrow().lifecycle.state,
            &writable.state.borrow().lifecycle.state,
        ));
    }

    #[test]
    fn native_compound_constructors_share_one_lifecycle_owner() {
        assert_shared_lifecycle(&Duplex::default());
        assert_shared_lifecycle(&Transform::new(|chunk| chunk).duplex_handle());
        let codec = crate::zlib::create_gzip(None);
        assert_shared_lifecycle(&crate::zlib::zlib_as_duplex(&codec));
    }

    #[test]
    fn joining_independent_native_sides_preserves_both_aliases() {
        let readable = Readable::default();
        let writable = Writable::new();
        let duplex = Duplex::new(readable.clone(), writable.clone());
        assert_shared_lifecycle(&duplex);
        readable.destroy_chain(None).unwrap();
        assert!(writable.destroyed());
        assert!(duplex.destroyed());
        assert!(readable.closed());
        assert!(writable.closed());
    }
}

impl<E: 'static> Clone for StreamLifecycle<E> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
}
