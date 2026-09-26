use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::error::NodeError;
use tsonic_rust_runtime::Callable;

#[derive(Clone)]
pub(crate) struct StreamListener<T> {
    pub(crate) identity: usize,
    pub(crate) once: bool,
    pub(crate) callback: Rc<dyn Fn(T) -> NodeResult<()>>,
}

pub(crate) struct StreamEvent<T> {
    listeners: Vec<StreamListener<T>>,
}

impl<T> Default for StreamEvent<T> {
    fn default() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }
}

impl<T> StreamEvent<T> {
    pub(crate) fn add(&mut self, listener: StreamListener<T>) {
        self.listeners.push(listener);
    }

    pub(crate) fn remove(&mut self, identity: usize) {
        self.listeners
            .retain(|listener| listener.identity != identity);
    }

    pub(crate) fn emission(&mut self) -> Vec<Rc<dyn Fn(T) -> NodeResult<()>>> {
        let callbacks = self
            .listeners
            .iter()
            .map(|listener| Rc::clone(&listener.callback))
            .collect();
        self.listeners.retain(|listener| !listener.once);
        callbacks
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.listeners.is_empty()
    }
}

pub(crate) fn value_listener<T: 'static, E: std::fmt::Display + 'static>(
    listener: &Callable<(T,), Result<(), E>>,
    once: bool,
) -> StreamListener<T> {
    let callback = listener.clone();
    StreamListener {
        identity: listener.identity_key(),
        once,
        callback: Rc::new(move |value| {
            callback
                .call((value,))
                .map_err(crate::error::callback_node_error)
        }),
    }
}

pub(crate) fn empty_listener<E: std::fmt::Display + 'static>(
    listener: &Callable<(), Result<(), E>>,
    once: bool,
) -> StreamListener<()> {
    let callback = listener.clone();
    StreamListener {
        identity: listener.identity_key(),
        once,
        callback: Rc::new(move |()| callback.call(()).map_err(crate::error::callback_node_error)),
    }
}

pub(crate) fn invoke_event<T: Clone>(
    callbacks: Vec<Rc<dyn Fn(T) -> NodeResult<()>>>,
    value: T,
) -> NodeResult<()> {
    for callback in callbacks {
        callback(value.clone())?;
    }
    Ok(())
}
