use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::error::NodeError;
use crate::retained_listener::RetainedListener;
use tsonic_rust_runtime::Callable;

enum StreamCallback<T: 'static, E: 'static> {
    Empty(Callable<(), Result<(), E>>),
    Value(Callable<(T,), Result<(), E>>),
}

impl<T: 'static, E: 'static> Clone for StreamCallback<T, E> {
    fn clone(&self) -> Self {
        match self {
            Self::Empty(callback) => Self::Empty(callback.clone()),
            Self::Value(callback) => Self::Value(callback.clone()),
        }
    }
}

impl<T: 'static, E: 'static> StreamCallback<T, E> {
    fn invoke(&self, value: T) -> Result<(), E> {
        match self {
            Self::Empty(callback) => callback.call(()),
            Self::Value(callback) => callback.call((value,)),
        }
    }
}

pub(crate) struct StreamListener<T: 'static, E: 'static> {
    identity: usize,
    callback: RetainedListener<StreamCallback<T, E>>,
}

impl<T: 'static, E: 'static> Clone for StreamListener<T, E> {
    fn clone(&self) -> Self {
        Self {
            identity: self.identity,
            callback: self.callback.clone(),
        }
    }
}

pub(crate) struct StreamEvent<T: 'static, E: 'static> {
    listeners: Option<Rc<Vec<StreamListener<T, E>>>>,
}

impl<T: 'static, E: 'static> Default for StreamEvent<T, E> {
    fn default() -> Self {
        Self { listeners: None }
    }
}

impl<T: 'static, E: 'static> StreamEvent<T, E> {
    pub(crate) fn add(&mut self, listener: StreamListener<T, E>) {
        let listeners = self.listeners.get_or_insert_with(|| Rc::new(Vec::new()));
        let listeners = Rc::make_mut(listeners);
        listeners.retain(|entry| entry.callback.is_pending());
        listeners.push(listener);
    }

    pub(crate) fn remove(&mut self, identity: usize) {
        if let Some(listeners) = &mut self.listeners {
            Rc::make_mut(listeners)
                .retain(|listener| listener.identity != identity && listener.callback.is_pending());
        }
    }

    pub(crate) fn append(&mut self, other: &mut Self) {
        if self.listeners.is_none() {
            self.listeners = other.listeners.take();
        } else if let Some(mut other) = other.listeners.take() {
            let listeners = self.listeners.as_mut().expect("existing listener owner");
            Rc::make_mut(listeners).append(Rc::make_mut(&mut other));
        }
    }

    pub(crate) fn emission(&self) -> StreamEmission<T, E> {
        StreamEmission {
            listeners: self.listeners.clone(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.listeners.as_ref().is_none_or(|listeners| {
            listeners
                .iter()
                .all(|listener| !listener.callback.is_pending())
        })
    }
}

pub(crate) struct StreamEmission<T: 'static, E: 'static> {
    listeners: Option<Rc<Vec<StreamListener<T, E>>>>,
}

impl<T: 'static, E: 'static> Default for StreamEmission<T, E> {
    fn default() -> Self {
        Self { listeners: None }
    }
}

pub(crate) fn value_listener<T: 'static, E: 'static>(
    listener: &Callable<(T,), Result<(), E>>,
    once: bool,
) -> StreamListener<T, E> {
    StreamListener {
        identity: listener.identity_key(),
        callback: RetainedListener::new(StreamCallback::Value(listener.clone()), once),
    }
}

pub(crate) fn empty_listener<E: 'static>(
    listener: &Callable<(), Result<(), E>>,
    once: bool,
) -> StreamListener<(), E> {
    StreamListener {
        identity: listener.identity_key(),
        callback: RetainedListener::new(StreamCallback::Empty(listener.clone()), once),
    }
}

pub(crate) fn invoke_event<T: Clone + 'static, E: 'static>(
    callbacks: StreamEmission<T, E>,
    value: T,
) -> Result<(), E> {
    let Some(listeners) = callbacks.listeners else {
        return Ok(());
    };
    let mut value = Some(value);
    for (index, listener) in listeners.iter().enumerate() {
        listener
            .callback
            .with_callback(|callback| {
                let argument = if index + 1 == listeners.len() {
                    value.take().expect("final event argument")
                } else {
                    value.as_ref().expect("retained event argument").clone()
                };
                callback.invoke(argument)
            })
            .transpose()?;
    }
    Ok(())
}
