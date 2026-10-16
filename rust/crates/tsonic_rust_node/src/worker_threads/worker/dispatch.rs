use super::*;

impl<E: From<NodeError> + 'static> Worker<E> {
    pub(in crate::worker_threads) fn activate(&self, owner: DispatchOwner) {
        self.state.delivery.borrow_mut().owner = Some(owner);
    }

    pub(in crate::worker_threads) fn deactivate(&self) {
        self.state.delivery.borrow_mut().owner = None;
        self.emitter.remove_all_callable_listeners();
        self.emitter.remove_all_listeners(None);
    }

    fn ensure_owner(&self) -> NodeResult<()> {
        let owner = tsonic_rust_js::event_loop::dispatch_owner().map_err(NodeError::from)?;
        if self.state.delivery.borrow().owner != Some(owner) {
            return Err(NodeError::new(
                "ERR_NODE_RESOURCE_CLOSED",
                "worker callbacks require their selected active native resource owner",
            ));
        }
        Ok(())
    }

    pub fn on_callable(
        &self,
        event: &JsValue,
        listener: &Callable<'static, (), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.ensure_owner()?;
        self.emitter.on_callable(event, listener)?;
        Ok(self)
    }

    pub fn on_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<'static, (JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.ensure_owner()?;
        self.emitter.on_callable1(event, listener)?;
        Ok(self)
    }

    pub fn once_callable(
        &self,
        event: &JsValue,
        listener: &Callable<'static, (), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.ensure_owner()?;
        self.emitter.once_callable(event, listener)?;
        Ok(self)
    }

    pub fn once_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<'static, (JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.ensure_owner()?;
        self.emitter.once_callable1(event, listener)?;
        Ok(self)
    }

    pub fn off_callable(
        &self,
        event: &JsValue,
        listener: &Callable<'_, (), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.ensure_owner()?;
        self.emitter.off_callable(event, listener)?;
        Ok(self)
    }

    pub fn off_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<'_, (JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.ensure_owner()?;
        self.emitter.off_callable1(event, listener)?;
        Ok(self)
    }

    pub(in crate::worker_threads) fn capture(&self) -> Result<Option<TaskTicket>, E> {
        if self.state.delivery.borrow().owner.is_none() {
            return Ok(None);
        }
        self.state.ingest_signals(false).map_err(E::from)?;
        Ok(self
            .state
            .delivery
            .borrow()
            .pending
            .back()
            .map(|(ticket, _)| *ticket))
    }

    pub(in crate::worker_threads) fn poll(&self, boundary: Option<TaskTicket>) -> Result<bool, E> {
        let Some(boundary) = boundary else {
            return Ok(false);
        };
        let selected_owner = tsonic_rust_js::event_loop::dispatch_owner()
            .map_err(NodeError::from)
            .map_err(E::from)?;
        let mut dispatched = false;
        loop {
            let signal = {
                let mut delivery = self.state.delivery.borrow_mut();
                if delivery.owner != Some(selected_owner) {
                    break;
                }
                if !delivery
                    .pending
                    .front()
                    .is_some_and(|(ticket, _)| *ticket <= boundary)
                {
                    break;
                }
                delivery.pending.pop_front().map(|(_, signal)| signal)
            };
            let Some(signal) = signal else {
                break;
            };
            let converted;
            let (event, arguments): (&str, &[JsValue]) = match &signal {
                WorkerSignal::Message(value) => {
                    converted = value.to_js();
                    ("message", std::slice::from_ref(&converted))
                }
                WorkerSignal::Error(error) => {
                    converted = JsValue::String(error.to_string());
                    ("error", std::slice::from_ref(&converted))
                }
                WorkerSignal::RemoteError(error) => {
                    converted = JsValue::String(error.clone());
                    ("error", std::slice::from_ref(&converted))
                }
                WorkerSignal::NativeIoError(error) => {
                    let snapshot = self.state.physical.snapshot();
                    if snapshot.termination_requested || snapshot.exit_observed {
                        continue;
                    }
                    converted = JsValue::String(error.to_string());
                    ("error", std::slice::from_ref(&converted))
                }
                WorkerSignal::Exit(code) => {
                    self.state.delivery.borrow_mut().terminal_consumed = true;
                    converted = JsValue::from(*code);
                    ("exit", std::slice::from_ref(&converted))
                }
            };
            self.emitter
                .prepare_named_emission(event, arguments)?
                .invoke(arguments)?;
            dispatched = true;
        }
        Ok(dispatched)
    }

    pub(in crate::worker_threads) fn is_refed_active(&self) -> bool {
        let delivery = self.state.delivery.borrow();
        delivery.owner.is_some()
            && delivery.refed
            && (!delivery.pending.is_empty() || !delivery.terminal_consumed)
    }
}

pub(in crate::worker_threads) struct RuntimeWorker<E: 'static> {
    state: Weak<WorkerState>,
    emitter: WeakEventEmitter<E>,
}

impl<E: 'static> Clone for RuntimeWorker<E> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            emitter: self.emitter.clone(),
        }
    }
}

impl<E: 'static> RuntimeWorker<E> {
    pub(in crate::worker_threads) fn is_alive(&self) -> bool {
        self.state.strong_count() > 0 && self.emitter.is_alive()
    }

    pub(in crate::worker_threads) fn upgrade(&self) -> Option<Worker<E>> {
        Some(Worker {
            state: self.state.upgrade()?,
            emitter: self.emitter.upgrade()?,
        })
    }
}

impl<E: From<NodeError> + 'static> Worker<E> {
    pub(in crate::worker_threads) fn downgrade(&self) -> RuntimeWorker<E> {
        RuntimeWorker {
            state: Rc::downgrade(&self.state),
            emitter: self.emitter.downgrade(),
        }
    }

    pub(in crate::worker_threads) fn ticket(&self) -> TaskTicket {
        self.state.reservation.ticket()
    }
}
