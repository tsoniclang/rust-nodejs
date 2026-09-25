use super::{unhandled_error, CallableEmission, CallableListener, CallableListenerEntry, EventEmitter, EventKey, Rc};
use crate::error::{callback_node_error, NodeResult};
use tsonic_rust_js::{JsArray, JsValue};
use tsonic_rust_runtime::Callable;

impl EventEmitter {
    pub fn on_callable<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.add_callable_listener(
            EventKey::from_value(event)?,
            listener.identity_key(),
            false,
            false,
            {
                let listener = listener.clone();
                move |_| listener.call(()).map_err(callback_node_error)
            },
        );
        Ok(self)
    }

    pub fn on_callable1<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.add_callable_listener(
            EventKey::from_value(event)?,
            listener.identity_key(),
            false,
            false,
            {
                let listener = listener.clone();
                move |arguments| {
                    listener
                        .call((arguments.first().cloned().unwrap_or(JsValue::Null),))
                        .map_err(callback_node_error)
                }
            },
        );
        Ok(self)
    }

    pub fn on_callable2<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.add_callable_listener(
            EventKey::from_value(event)?,
            listener.identity_key(),
            false,
            false,
            {
                let listener = listener.clone();
                move |arguments| {
                    listener
                        .call((
                            arguments.first().cloned().unwrap_or(JsValue::Null),
                            arguments.get(1).cloned().unwrap_or(JsValue::Null),
                        ))
                        .map_err(callback_node_error)
                }
            },
        );
        Ok(self)
    }

    pub fn on_callable3<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.add_callable_listener(
            EventKey::from_value(event)?,
            listener.identity_key(),
            false,
            false,
            {
                let listener = listener.clone();
                move |arguments| {
                    listener
                        .call((
                            arguments.first().cloned().unwrap_or(JsValue::Null),
                            arguments.get(1).cloned().unwrap_or(JsValue::Null),
                            arguments.get(2).cloned().unwrap_or(JsValue::Null),
                        ))
                        .map_err(callback_node_error)
                }
            },
        );
        Ok(self)
    }

    pub fn once_callable<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.add_callable_listener(
            EventKey::from_value(event)?,
            listener.identity_key(),
            true,
            false,
            {
                let listener = listener.clone();
                move |_| listener.call(()).map_err(callback_node_error)
            },
        );
        Ok(self)
    }

    pub fn once_callable1<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.add_callable_listener(
            EventKey::from_value(event)?,
            listener.identity_key(),
            true,
            false,
            {
                let listener = listener.clone();
                move |arguments| {
                    listener
                        .call((arguments.first().cloned().unwrap_or(JsValue::Null),))
                        .map_err(callback_node_error)
                }
            },
        );
        Ok(self)
    }

    pub fn once_callable2<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.add_callable_listener(
            EventKey::from_value(event)?,
            listener.identity_key(),
            true,
            false,
            {
                let listener = listener.clone();
                move |arguments| {
                    listener
                        .call((
                            arguments.first().cloned().unwrap_or(JsValue::Null),
                            arguments.get(1).cloned().unwrap_or(JsValue::Null),
                        ))
                        .map_err(callback_node_error)
                }
            },
        );
        Ok(self)
    }

    pub fn once_callable3<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.add_callable_listener(
            EventKey::from_value(event)?,
            listener.identity_key(),
            true,
            false,
            {
                let listener = listener.clone();
                move |arguments| {
                    listener
                        .call((
                            arguments.first().cloned().unwrap_or(JsValue::Null),
                            arguments.get(1).cloned().unwrap_or(JsValue::Null),
                            arguments.get(2).cloned().unwrap_or(JsValue::Null),
                        ))
                        .map_err(callback_node_error)
                }
            },
        );
        Ok(self)
    }

    pub fn prepend_callable<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.add_callable_listener(
            EventKey::from_value(event)?,
            listener.identity_key(),
            false,
            true,
            {
                let listener = listener.clone();
                move |_| listener.call(()).map_err(callback_node_error)
            },
        );
        Ok(self)
    }

    pub fn prepend_callable1<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.add_callable_listener(
            EventKey::from_value(event)?,
            listener.identity_key(),
            false,
            true,
            {
                let listener = listener.clone();
                move |arguments| {
                    listener
                        .call((arguments.first().cloned().unwrap_or(JsValue::Null),))
                        .map_err(callback_node_error)
                }
            },
        );
        Ok(self)
    }

    pub fn prepend_callable2<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.add_callable_listener(
            EventKey::from_value(event)?,
            listener.identity_key(),
            false,
            true,
            {
                let listener = listener.clone();
                move |arguments| {
                    listener
                        .call((
                            arguments.first().cloned().unwrap_or(JsValue::Null),
                            arguments.get(1).cloned().unwrap_or(JsValue::Null),
                        ))
                        .map_err(callback_node_error)
                }
            },
        );
        Ok(self)
    }

    pub fn prepend_callable3<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.add_callable_listener(
            EventKey::from_value(event)?,
            listener.identity_key(),
            false,
            true,
            {
                let listener = listener.clone();
                move |arguments| {
                    listener
                        .call((
                            arguments.first().cloned().unwrap_or(JsValue::Null),
                            arguments.get(1).cloned().unwrap_or(JsValue::Null),
                            arguments.get(2).cloned().unwrap_or(JsValue::Null),
                        ))
                        .map_err(callback_node_error)
                }
            },
        );
        Ok(self)
    }

    pub fn prepend_once_callable<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.add_callable_listener(
            EventKey::from_value(event)?,
            listener.identity_key(),
            true,
            true,
            {
                let listener = listener.clone();
                move |_| listener.call(()).map_err(callback_node_error)
            },
        );
        Ok(self)
    }

    pub fn prepend_once_callable1<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.add_callable_listener(
            EventKey::from_value(event)?,
            listener.identity_key(),
            true,
            true,
            {
                let listener = listener.clone();
                move |arguments| {
                    listener
                        .call((arguments.first().cloned().unwrap_or(JsValue::Null),))
                        .map_err(callback_node_error)
                }
            },
        );
        Ok(self)
    }

    pub fn prepend_once_callable2<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.add_callable_listener(
            EventKey::from_value(event)?,
            listener.identity_key(),
            true,
            true,
            {
                let listener = listener.clone();
                move |arguments| {
                    listener
                        .call((
                            arguments.first().cloned().unwrap_or(JsValue::Null),
                            arguments.get(1).cloned().unwrap_or(JsValue::Null),
                        ))
                        .map_err(callback_node_error)
                }
            },
        );
        Ok(self)
    }

    pub fn prepend_once_callable3<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.add_callable_listener(
            EventKey::from_value(event)?,
            listener.identity_key(),
            true,
            true,
            {
                let listener = listener.clone();
                move |arguments| {
                    listener
                        .call((
                            arguments.first().cloned().unwrap_or(JsValue::Null),
                            arguments.get(1).cloned().unwrap_or(JsValue::Null),
                            arguments.get(2).cloned().unwrap_or(JsValue::Null),
                        ))
                        .map_err(callback_node_error)
                }
            },
        );
        Ok(self)
    }

    pub fn off_callable<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: 'static,
    {
        self.remove_callable_listener(&EventKey::from_value(event)?, listener.identity_key());
        Ok(self)
    }

    pub fn off_callable1<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: 'static,
    {
        self.remove_callable_listener(&EventKey::from_value(event)?, listener.identity_key());
        Ok(self)
    }

    pub fn off_callable2<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: 'static,
    {
        self.remove_callable_listener(&EventKey::from_value(event)?, listener.identity_key());
        Ok(self)
    }

    pub fn off_callable3<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(JsValue, JsValue, JsValue), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: 'static,
    {
        self.remove_callable_listener(&EventKey::from_value(event)?, listener.identity_key());
        Ok(self)
    }

    pub fn emit_callable(&mut self, event: &JsValue) -> NodeResult<bool> {
        self.emit_callable_values(event, &[])
    }

    pub fn emit_callable1(&mut self, event: &JsValue, first: JsValue) -> NodeResult<bool> {
        self.emit_callable_values(event, &[first])
    }

    pub fn emit_callable2(
        &mut self,
        event: &JsValue,
        first: JsValue,
        second: JsValue,
    ) -> NodeResult<bool> {
        self.emit_callable_values(event, &[first, second])
    }

    pub fn emit_callable3(
        &mut self,
        event: &JsValue,
        first: JsValue,
        second: JsValue,
        third: JsValue,
    ) -> NodeResult<bool> {
        self.emit_callable_values(event, &[first, second, third])
    }

    fn emit_callable_values(&mut self, event: &JsValue, arguments: &[JsValue]) -> NodeResult<bool> {
        self.prepare_callable_emission(event, arguments)?
            .invoke(arguments)
    }

    pub(crate) fn prepare_callable_emission(
        &mut self,
        event: &JsValue,
        arguments: &[JsValue],
    ) -> NodeResult<CallableEmission> {
        let event = EventKey::from_value(event)?;
        let listeners = self.callable_listeners.get(&event).cloned();
        if listeners.is_none() && event.is_error() {
            return Err(unhandled_error(arguments));
        }
        self.remove_once_callable_listeners(&event);
        Ok(CallableEmission { listeners })
    }

    pub fn callable_listener_count(&self, event: &JsValue) -> NodeResult<usize> {
        let event = EventKey::from_value(event)?;
        Ok(self
            .callable_listeners
            .get(&event)
            .map_or(0, |listeners| listeners.len()))
    }

    pub fn remove_all_callable_listeners(&mut self) -> &mut Self {
        self.callable_listeners.clear();
        self.callable_event_order.clear();
        self
    }

    pub fn remove_all_callable_listeners_for(&mut self, event: &JsValue) -> NodeResult<&mut Self> {
        let event = EventKey::from_value(event)?;
        self.callable_listeners.remove(&event);
        self.callable_event_order
            .retain(|candidate| candidate != &event);
        Ok(self)
    }

    pub fn callable_event_names(&self) -> JsArray<JsValue> {
        JsArray::from_dense(
            self.callable_event_order
                .iter()
                .filter(|event| self.callable_listeners.contains_key(*event))
                .map(EventKey::to_value)
                .collect(),
        )
    }

    fn add_callable_listener(
        &mut self,
        event: EventKey,
        identity: usize,
        once: bool,
        prepend: bool,
        callback: impl Fn(&[JsValue]) -> NodeResult<()> + 'static,
    ) {
        let is_new_event = !self.callable_listeners.contains_key(&event);
        let listeners = self.callable_listeners.entry(event.clone()).or_default();
        let listeners = Rc::make_mut(listeners);
        let entry = CallableListenerEntry {
            identity,
            once,
            callback: Rc::new(callback),
        };
        if prepend {
            listeners.insert(0, entry);
        } else {
            listeners.push(entry);
        }
        if is_new_event {
            self.callable_event_order.push(event);
        }
    }

    fn remove_callable_listener(&mut self, event: &EventKey, identity: usize) {
        if let Some(listeners) = self.callable_listeners.get_mut(event) {
            Rc::make_mut(listeners).retain(|entry| entry.identity != identity);
            if listeners.is_empty() {
                self.callable_listeners.remove(event);
                self.callable_event_order
                    .retain(|candidate| candidate != event);
            }
        }
    }

    fn remove_once_callable_listeners(&mut self, event: &EventKey) {
        if let Some(listeners) = self.callable_listeners.get_mut(event) {
            if !listeners.iter().any(|entry| entry.once) {
                return;
            }
            Rc::make_mut(listeners).retain(|entry| !entry.once);
            if listeners.is_empty() {
                self.callable_listeners.remove(event);
                self.callable_event_order
                    .retain(|candidate| candidate != event);
            }
        }
    }

}
