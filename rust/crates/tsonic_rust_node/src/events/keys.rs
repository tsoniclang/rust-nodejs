use std::collections::HashMap;
use std::rc::Rc;

use tsonic_rust_js::{JsSymbol, JsValue};

use crate::error::{NodeError, NodeResult};

#[derive(Clone)]
pub(super) enum EventKey {
    String(String),
    Symbol(JsSymbol),
}

#[derive(Clone, Copy)]
pub(crate) enum EventName<'a> {
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
    pub(crate) fn from_value(value: &'a JsValue) -> NodeResult<Self> {
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

pub(crate) struct EventListenerMap<TListener> {
    strings: HashMap<String, Rc<Vec<TListener>>>,
    symbols: HashMap<JsSymbol, Rc<Vec<TListener>>>,
}

impl<TListener> EventListenerMap<TListener> {
    pub(crate) fn new() -> Self {
        Self {
            strings: HashMap::new(),
            symbols: HashMap::new(),
        }
    }

    pub(crate) fn get(&self, name: EventName<'_>) -> Option<&Rc<Vec<TListener>>> {
        match name {
            EventName::String(value) => self.strings.get(value),
            EventName::Symbol(value) => self.symbols.get(value),
        }
    }

    pub(crate) fn get_mut(&mut self, name: EventName<'_>) -> Option<&mut Rc<Vec<TListener>>> {
        match name {
            EventName::String(value) => self.strings.get_mut(value),
            EventName::Symbol(value) => self.symbols.get_mut(value),
        }
    }

    pub(crate) fn insert(&mut self, name: EventName<'_>, listeners: Rc<Vec<TListener>>) {
        match name {
            EventName::String(value) => {
                self.strings.insert(value.to_owned(), listeners);
            }
            EventName::Symbol(value) => {
                self.symbols.insert(value.clone(), listeners);
            }
        }
    }

    pub(crate) fn remove(&mut self, name: EventName<'_>) -> Option<Rc<Vec<TListener>>> {
        match name {
            EventName::String(value) => self.strings.remove(value),
            EventName::Symbol(value) => self.symbols.remove(value),
        }
    }

    pub(crate) fn values_mut(&mut self) -> impl Iterator<Item = &mut Rc<Vec<TListener>>> {
        self.strings.values_mut().chain(self.symbols.values_mut())
    }

    pub(crate) fn remove_empty(&mut self) {
        self.strings.retain(|_, listeners| !listeners.is_empty());
        self.symbols.retain(|_, listeners| !listeners.is_empty());
    }
}
