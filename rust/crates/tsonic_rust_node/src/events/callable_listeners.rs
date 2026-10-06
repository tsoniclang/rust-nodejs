use super::callback::{EventCallback, ListenerCallback};
use super::{
    unhandled_error, CallableEmission, CallableListenerEntry, EventEmitter, EventKey, EventName, Rc,
};
use crate::error::{NodeError, NodeResult};
use tsonic_rust_js::{JsArray, JsValue};
use tsonic_rust_runtime::Callable;

impl<E: 'static> EventEmitter<E> {
    pub fn on_callable(
        &self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_callable_listener(
            EventName::from_value(event)?,
            listener.identity_key(),
            false,
            false,
            EventCallback::Empty(listener.clone()),
        );
        Ok(self)
    }

    pub fn on_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_callable_listener(
            EventName::from_value(event)?,
            listener.identity_key(),
            false,
            false,
            EventCallback::One(listener.clone()),
        );
        Ok(self)
    }

    pub fn on_callable2(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_callable_listener(
            EventName::from_value(event)?,
            listener.identity_key(),
            false,
            false,
            EventCallback::Two(listener.clone()),
        );
        Ok(self)
    }

    pub fn on_callable3(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_callable_listener(
            EventName::from_value(event)?,
            listener.identity_key(),
            false,
            false,
            EventCallback::Three(listener.clone()),
        );
        Ok(self)
    }

    pub fn once_callable(
        &self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_callable_listener(
            EventName::from_value(event)?,
            listener.identity_key(),
            true,
            false,
            EventCallback::Empty(listener.clone()),
        );
        Ok(self)
    }

    pub fn once_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_callable_listener(
            EventName::from_value(event)?,
            listener.identity_key(),
            true,
            false,
            EventCallback::One(listener.clone()),
        );
        Ok(self)
    }

    pub fn once_callable2(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_callable_listener(
            EventName::from_value(event)?,
            listener.identity_key(),
            true,
            false,
            EventCallback::Two(listener.clone()),
        );
        Ok(self)
    }

    pub fn once_callable3(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_callable_listener(
            EventName::from_value(event)?,
            listener.identity_key(),
            true,
            false,
            EventCallback::Three(listener.clone()),
        );
        Ok(self)
    }

    pub fn prepend_callable(
        &self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_callable_listener(
            EventName::from_value(event)?,
            listener.identity_key(),
            false,
            true,
            EventCallback::Empty(listener.clone()),
        );
        Ok(self)
    }

    pub fn prepend_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_callable_listener(
            EventName::from_value(event)?,
            listener.identity_key(),
            false,
            true,
            EventCallback::One(listener.clone()),
        );
        Ok(self)
    }

    pub fn prepend_callable2(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_callable_listener(
            EventName::from_value(event)?,
            listener.identity_key(),
            false,
            true,
            EventCallback::Two(listener.clone()),
        );
        Ok(self)
    }

    pub fn prepend_callable3(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_callable_listener(
            EventName::from_value(event)?,
            listener.identity_key(),
            false,
            true,
            EventCallback::Three(listener.clone()),
        );
        Ok(self)
    }

    pub fn prepend_once_callable(
        &self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_callable_listener(
            EventName::from_value(event)?,
            listener.identity_key(),
            true,
            true,
            EventCallback::Empty(listener.clone()),
        );
        Ok(self)
    }

    pub fn prepend_once_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_callable_listener(
            EventName::from_value(event)?,
            listener.identity_key(),
            true,
            true,
            EventCallback::One(listener.clone()),
        );
        Ok(self)
    }

    pub fn prepend_once_callable2(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_callable_listener(
            EventName::from_value(event)?,
            listener.identity_key(),
            true,
            true,
            EventCallback::Two(listener.clone()),
        );
        Ok(self)
    }

    pub fn prepend_once_callable3(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_callable_listener(
            EventName::from_value(event)?,
            listener.identity_key(),
            true,
            true,
            EventCallback::Three(listener.clone()),
        );
        Ok(self)
    }

    pub fn off_callable(
        &self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.remove_callable_listener(EventName::from_value(event)?, listener.identity_key());
        Ok(self)
    }

    pub fn off_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.remove_callable_listener(EventName::from_value(event)?, listener.identity_key());
        Ok(self)
    }

    pub fn off_callable2(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.remove_callable_listener(EventName::from_value(event)?, listener.identity_key());
        Ok(self)
    }

    pub fn off_callable3(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.remove_callable_listener(EventName::from_value(event)?, listener.identity_key());
        Ok(self)
    }

    pub fn callable_listener_count(&self, event: &JsValue) -> NodeResult<usize> {
        let event = EventName::from_value(event)?;
        Ok(self
            .state
            .borrow()
            .callable_listeners
            .get(event)
            .map_or(0, |listeners| {
                listeners
                    .iter()
                    .filter(|entry| entry.callback.is_pending())
                    .count()
            }))
    }

    pub fn remove_all_callable_listeners(&self) -> &Self {
        let removed = {
            let mut state = self.state.borrow_mut();
            state.callable_event_order.clear();
            std::mem::replace(
                &mut state.callable_listeners,
                super::CallableListenerMap::new(),
            )
        };
        drop(removed);
        self
    }

    pub fn remove_all_callable_listeners_for(&self, event: &JsValue) -> NodeResult<&Self> {
        let event = EventName::from_value(event)?;
        let removed = {
            let mut state = self.state.borrow_mut();
            let removed = state.callable_listeners.remove(event);
            state
                .callable_event_order
                .retain(|candidate| !event.matches(candidate));
            removed
        };
        drop(removed);
        Ok(self)
    }

    pub fn callable_event_names(&self) -> JsArray<JsValue> {
        let state = self.state.borrow();
        JsArray::from_dense(
            state
                .callable_event_order
                .iter()
                .filter(|event| {
                    state
                        .callable_listeners
                        .get(event.as_name())
                        .is_some_and(|listeners| {
                            listeners.iter().any(|entry| entry.callback.is_pending())
                        })
                })
                .map(EventKey::to_value)
                .collect(),
        )
    }

    fn add_callable_listener(
        &self,
        event: EventName<'_>,
        identity: usize,
        once: bool,
        prepend: bool,
        callback: EventCallback<E>,
    ) {
        let mut state = self.state.borrow_mut();
        Self::prune_completed_callable_listeners(&mut state, event);
        let entry = CallableListenerEntry {
            identity,
            callback: ListenerCallback::new(callback, once),
        };
        if let Some(listeners) = state.callable_listeners.get_mut(event) {
            if prepend {
                Rc::make_mut(listeners).insert(0, entry);
            } else {
                Rc::make_mut(listeners).push(entry);
            }
        } else {
            state.callable_listeners.insert(event, Rc::new(vec![entry]));
            state.callable_event_order.push(event.to_owned());
        }
    }

    fn remove_callable_listener(&self, event: EventName<'_>, identity: usize) {
        let mut state = self.state.borrow_mut();
        if let Some(listeners) = state.callable_listeners.get_mut(event) {
            Rc::make_mut(listeners)
                .retain(|entry| entry.identity != identity && entry.callback.is_pending());
            if listeners.is_empty() {
                state.callable_listeners.remove(event);
                state
                    .callable_event_order
                    .retain(|candidate| !event.matches(candidate));
            }
        }
    }

    fn prune_completed_callable_listeners(
        state: &mut super::EventEmitterState<E>,
        event: EventName<'_>,
    ) {
        if let Some(listeners) = state.callable_listeners.get_mut(event) {
            if listeners.iter().all(|entry| entry.callback.is_pending()) {
                return;
            }
            Rc::make_mut(listeners).retain(|entry| entry.callback.is_pending());
            if listeners.is_empty() {
                state.callable_listeners.remove(event);
                state
                    .callable_event_order
                    .retain(|candidate| !event.matches(candidate));
            }
        }
    }
}

impl<E: From<NodeError> + 'static> EventEmitter<E> {
    pub fn emit_callable(&self, event: &JsValue) -> Result<bool, E> {
        self.emit_callable_values(event, &[])
    }

    pub fn emit_callable1(&self, event: &JsValue, first: JsValue) -> Result<bool, E> {
        self.emit_callable_values(event, &[first])
    }

    pub fn emit_callable2(
        &self,
        event: &JsValue,
        first: JsValue,
        second: JsValue,
    ) -> Result<bool, E> {
        self.emit_callable_values(event, &[first, second])
    }

    pub fn emit_callable3(
        &self,
        event: &JsValue,
        first: JsValue,
        second: JsValue,
        third: JsValue,
    ) -> Result<bool, E> {
        self.emit_callable_values(event, &[first, second, third])
    }

    fn emit_callable_values(&self, event: &JsValue, arguments: &[JsValue]) -> Result<bool, E> {
        self.prepare_callable_emission(event, arguments)?
            .invoke(arguments)
    }

    pub(crate) fn prepare_callable_emission(
        &self,
        event: &JsValue,
        arguments: &[JsValue],
    ) -> Result<CallableEmission<E>, E> {
        let event = EventName::from_value(event)?;
        self.prepare_emission(event, arguments)
    }

    pub(crate) fn prepare_named_emission(
        &self,
        event: &str,
        arguments: &[JsValue],
    ) -> Result<CallableEmission<E>, E> {
        self.prepare_emission(EventName::String(event), arguments)
    }

    fn prepare_emission(
        &self,
        event: EventName<'_>,
        arguments: &[JsValue],
    ) -> Result<CallableEmission<E>, E> {
        let state = self.state.borrow();
        let listeners = state.callable_listeners.get(event).cloned();
        if event.is_error()
            && listeners.as_ref().map_or(true, |listeners| {
                listeners.iter().all(|entry| !entry.callback.is_pending())
            })
        {
            return Err(unhandled_error(arguments).into());
        }
        Ok(CallableEmission { listeners })
    }
}
