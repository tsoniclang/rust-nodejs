use std::collections::HashMap;
use std::rc::Rc;

use tsonic_rust_js::{JsSymbol, JsValue};

use super::CallableListenerEntry;
use crate::error::{NodeError, NodeResult};

#[derive(Clone)]
pub(super) enum EventKey {
    String(String),
    Symbol(JsSymbol),
}

#[derive(Clone, Copy)]
pub(super) enum EventName<'a> {
    String(&'a str),
    Symbol(&'a JsSymbol),
}

impl EventKey {
    pub(super) fn as_name(&self) -> EventName<'_> {
        match self {
            Self::String(value) => EventName::String(value),
            Self::Symbol(value) => EventName::Symbol(value),
        }
    }

    pub(super) fn to_value(&self) -> JsValue {
        match self {
            Self::String(value) => JsValue::String(value.clone()),
            Self::Symbol(value) => JsValue::Symbol(value.clone()),
        }
    }
}

impl<'a> EventName<'a> {
    pub(super) fn from_value(value: &'a JsValue) -> NodeResult<Self> {
        match value {
            JsValue::String(value) => Ok(Self::String(value)),
            JsValue::Symbol(value) => Ok(Self::Symbol(value)),
            _ => Err(NodeError::new(
                "ERR_INVALID_ARG_TYPE",
                "EventEmitter event name must be a string or Symbol",
            )),
        }
    }

    pub(super) fn is_error(self) -> bool {
        matches!(self, Self::String("error"))
    }

    pub(super) fn matches(self, key: &EventKey) -> bool {
        match (self, key) {
            (Self::String(left), EventKey::String(right)) => left == right,
            (Self::Symbol(left), EventKey::Symbol(right)) => left == right,
            _ => false,
        }
    }

    pub(super) fn to_owned(self) -> EventKey {
        match self {
            Self::String(value) => EventKey::String(value.to_owned()),
            Self::Symbol(value) => EventKey::Symbol(value.clone()),
        }
    }
}

pub(super) struct CallableListenerMap<E: 'static> {
    strings: HashMap<String, Rc<Vec<CallableListenerEntry<E>>>>,
    symbols: HashMap<JsSymbol, Rc<Vec<CallableListenerEntry<E>>>>,
}

impl<E: 'static> CallableListenerMap<E> {
    pub(super) fn new() -> Self {
        Self {
            strings: HashMap::new(),
            symbols: HashMap::new(),
        }
    }

    pub(super) fn get(&self, name: EventName<'_>) -> Option<&Rc<Vec<CallableListenerEntry<E>>>> {
        match name {
            EventName::String(value) => self.strings.get(value),
            EventName::Symbol(value) => self.symbols.get(value),
        }
    }

    pub(super) fn get_mut(
        &mut self,
        name: EventName<'_>,
    ) -> Option<&mut Rc<Vec<CallableListenerEntry<E>>>> {
        match name {
            EventName::String(value) => self.strings.get_mut(value),
            EventName::Symbol(value) => self.symbols.get_mut(value),
        }
    }

    pub(super) fn insert(
        &mut self,
        name: EventName<'_>,
        listeners: Rc<Vec<CallableListenerEntry<E>>>,
    ) {
        match name {
            EventName::String(value) => {
                self.strings.insert(value.to_owned(), listeners);
            }
            EventName::Symbol(value) => {
                self.symbols.insert(value.clone(), listeners);
            }
        }
    }

    pub(super) fn remove(&mut self, name: EventName<'_>) {
        match name {
            EventName::String(value) => {
                self.strings.remove(value);
            }
            EventName::Symbol(value) => {
                self.symbols.remove(value);
            }
        }
    }

    pub(super) fn clear(&mut self) {
        self.strings.clear();
        self.symbols.clear();
    }
}
