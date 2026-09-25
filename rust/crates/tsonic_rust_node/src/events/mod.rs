mod callable_listeners;
mod event_target;
mod async_resource;

pub use event_target::NodeEventTarget;
pub use async_resource::{EventEmitterAsyncResource, EventEmitterAsyncResourceOptions};

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use tsonic_rust_js::{JsSymbol, JsValue};

use crate::error::{NodeError, NodeResult};

type Listener = Box<dyn FnMut(&[JsValue])>;
type ListenerMap = HashMap<String, Vec<ListenerEntry>>;
type CallableListener = Rc<dyn Fn(&[JsValue]) -> NodeResult<()>>;
type CallableListenerMap = HashMap<EventKey, Rc<Vec<CallableListenerEntry>>>;
static DEFAULT_MAX_LISTENERS: AtomicUsize = AtomicUsize::new(10);
static CAPTURE_REJECTIONS: AtomicBool = AtomicBool::new(false);

pub const ERROR_MONITOR: &str = "events.errorMonitor";
pub const CAPTURE_REJECTION_SYMBOL: &str = "events.captureRejectionSymbol";

struct ListenerEntry {
    id: usize,
    once: bool,
    callback: Listener,
}

struct CallableListenerEntry {
    identity: usize,
    once: bool,
    callback: CallableListener,
}

pub(crate) struct CallableEmission {
    listeners: Option<Rc<Vec<CallableListenerEntry>>>,
}

impl CallableEmission {
    pub(crate) fn invoke(self, arguments: &[JsValue]) -> NodeResult<bool> {
        let Some(listeners) = self.listeners else {
            return Ok(false);
        };
        for listener in listeners.iter() {
            (listener.callback)(arguments)?;
        }
        Ok(true)
    }
}

impl Clone for CallableListenerEntry {
    fn clone(&self) -> Self {
        Self {
            identity: self.identity,
            once: self.once,
            callback: Rc::clone(&self.callback),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum EventKey {
    String(String),
    Symbol(JsSymbol),
}

impl EventKey {
    fn from_value(value: &JsValue) -> NodeResult<Self> {
        match value {
            JsValue::String(value) => Ok(Self::String(value.clone())),
            JsValue::Symbol(value) => Ok(Self::Symbol(value.clone())),
            _ => Err(NodeError::new(
                "ERR_INVALID_ARG_TYPE",
                "EventEmitter event name must be a string or Symbol",
            )),
        }
    }

    fn is_error(&self) -> bool {
        matches!(self, Self::String(value) if value == "error")
    }

    fn to_value(&self) -> JsValue {
        match self {
            Self::String(value) => JsValue::String(value.clone()),
            Self::Symbol(value) => JsValue::Symbol(value.clone()),
        }
    }
}

#[derive(Default)]
pub struct EventEmitter {
    listeners: ListenerMap,
    listener_event_order: Vec<String>,
    callable_listeners: CallableListenerMap,
    callable_event_order: Vec<EventKey>,
    max_listeners: Option<usize>,
    next_listener_id: usize,
    capture_rejections: bool,
}

impl EventEmitter {
    pub fn new() -> Self {
        Self {
            max_listeners: Some(default_max_listeners()),
            capture_rejections: capture_rejections(),
            ..Self::default()
        }
    }

    pub fn with_options(options: EventEmitterOptions) -> Self {
        Self {
            max_listeners: Some(default_max_listeners()),
            capture_rejections: options.capture_rejections,
            ..Self::default()
        }
    }

    pub fn capture_rejections(&self) -> bool {
        self.capture_rejections
    }

    pub fn on<F>(&mut self, event: impl Into<String>, listener: F) -> &mut Self
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.add_entry(event.into(), false, false, listener);
        self
    }

    pub fn on_with_id<F>(&mut self, event: impl Into<String>, listener: F) -> usize
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.add_entry(event.into(), false, false, listener)
    }

    pub fn prepend_listener<F>(&mut self, event: impl Into<String>, listener: F) -> &mut Self
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.add_entry(event.into(), false, true, listener);
        self
    }

    pub fn prepend_listener_with_id<F>(&mut self, event: impl Into<String>, listener: F) -> usize
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.add_entry(event.into(), false, true, listener)
    }

    fn add_entry<F>(&mut self, event: String, once: bool, prepend: bool, listener: F) -> usize
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.next_listener_id += 1;
        let id = self.next_listener_id;
        let entry = ListenerEntry {
            id,
            once,
            callback: Box::new(listener),
        };
        let is_new_event = !self.listeners.contains_key(&event);
        let listeners = self.listeners.entry(event.clone()).or_default();
        if prepend {
            listeners.insert(0, entry);
        } else {
            listeners.push(entry);
        }
        if is_new_event {
            self.listener_event_order.push(event);
        }
        id
    }

    pub fn off_by_id(&mut self, event: &str, listener_id: usize) -> &mut Self {
        if let Some(listeners) = self.listeners.get_mut(event) {
            listeners.retain(|listener| listener.id != listener_id);
            if listeners.is_empty() {
                self.listeners.remove(event);
                self.listener_event_order
                    .retain(|candidate| candidate != event);
            }
        }
        self
    }

    pub fn remove_listener_by_id(&mut self, event: &str, listener_id: usize) -> &mut Self {
        self.off_by_id(event, listener_id)
    }

    pub fn remove_listener(&mut self, event: &str, listener_id: usize) -> &mut Self {
        self.off_by_id(event, listener_id)
    }

    pub fn off(&mut self, event: &str, listener_id: usize) -> &mut Self {
        self.off_by_id(event, listener_id)
    }

    pub fn listeners(&self, event: &str) -> Vec<usize> {
        self.listeners
            .get(event)
            .map(|listeners| listeners.iter().map(|listener| listener.id).collect())
            .unwrap_or_default()
    }

    pub fn raw_listeners(&self, event: &str) -> Vec<usize> {
        self.listeners(event)
    }

    pub fn add_listener<F>(&mut self, event: impl Into<String>, listener: F) -> &mut Self
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.on(event, listener)
    }

    pub fn once<F>(&mut self, event: impl Into<String>, mut listener: F) -> &mut Self
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.add_entry(event.into(), true, false, move |args| listener(args));
        self
    }

    pub fn once_with_id<F>(&mut self, event: impl Into<String>, mut listener: F) -> usize
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.add_entry(event.into(), true, false, move |args| listener(args))
    }

    pub fn prepend_once_listener<F>(
        &mut self,
        event: impl Into<String>,
        mut listener: F,
    ) -> &mut Self
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.add_entry(event.into(), true, true, move |args| listener(args));
        self
    }

    pub fn prepend_once_listener_with_id<F>(
        &mut self,
        event: impl Into<String>,
        mut listener: F,
    ) -> usize
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.add_entry(event.into(), true, true, move |args| listener(args))
    }

    pub fn emit(&mut self, event: &str, args: &[JsValue]) -> bool {
        let Some(mut listeners) = self.listeners.remove(event) else {
            return false;
        };
        if listeners.is_empty() {
            return false;
        }
        for listener in &mut listeners {
            (listener.callback)(args);
        }
        listeners.retain(|listener| !listener.once);
        if listeners.is_empty() {
            self.listener_event_order
                .retain(|candidate| candidate != event);
        } else {
            self.listeners.insert(event.to_string(), listeners);
        }
        true
    }

    pub fn listener_count(&self, event: &str) -> usize {
        self.listeners.get(event).map_or(0, Vec::len)
    }

    pub fn event_names(&self) -> Vec<String> {
        self.listener_event_order
            .iter()
            .filter(|event| self.listeners.contains_key(*event))
            .cloned()
            .collect()
    }

    pub fn remove_all_listeners(&mut self, event: Option<&str>) -> &mut Self {
        if let Some(event) = event {
            self.listeners.remove(event);
            self.listener_event_order
                .retain(|candidate| candidate != event);
        } else {
            self.listeners.clear();
            self.listener_event_order.clear();
        }
        self
    }

    pub fn set_max_listeners(&mut self, max: usize) -> &mut Self {
        self.max_listeners = Some(max);
        self
    }

    pub fn get_max_listeners(&self) -> usize {
        self.max_listeners.unwrap_or(0)
    }

    pub fn has_listeners(&self, event: &str) -> bool {
        self.listener_count(event) > 0
    }
}

fn unhandled_error(arguments: &[JsValue]) -> NodeError {
    let detail = arguments
        .first()
        .map(JsValue::inspect)
        .unwrap_or_else(|| "null".to_string());
    NodeError::new(
        "ERR_UNHANDLED_ERROR",
        format!("Unhandled 'error' event ({detail})"),
    )
}

pub fn listener_count_callable(emitter: &EventEmitter, event: &JsValue) -> NodeResult<usize> {
    emitter.callable_listener_count(event)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EventEmitterOptions {
    pub capture_rejections: bool,
}

pub fn once<F>(emitter: &mut EventEmitter, event: impl Into<String>, listener: F)
where
    F: FnMut(&[JsValue]) + 'static,
{
    emitter.once(event, listener);
}

pub fn on<F>(emitter: &mut EventEmitter, event: impl Into<String>, listener: F)
where
    F: FnMut(&[JsValue]) + 'static,
{
    emitter.on(event, listener);
}

pub fn listener_count(emitter: &EventEmitter, event: &str) -> usize {
    emitter.listener_count(event)
}

pub fn get_event_listeners(emitter: &EventEmitter, event: &str) -> Vec<usize> {
    emitter.listeners(event)
}

pub fn set_max_listeners(max: usize, emitters: &mut [&mut EventEmitter]) {
    for emitter in emitters {
        emitter.set_max_listeners(max);
    }
}

pub fn default_max_listeners() -> usize {
    DEFAULT_MAX_LISTENERS.load(Ordering::SeqCst)
}

pub fn set_default_max_listeners(max: usize) {
    DEFAULT_MAX_LISTENERS.store(max, Ordering::SeqCst);
}

pub fn capture_rejections() -> bool {
    CAPTURE_REJECTIONS.load(Ordering::SeqCst)
}

pub fn set_capture_rejections(value: bool) {
    CAPTURE_REJECTIONS.store(value, Ordering::SeqCst);
}

pub fn error_monitor() -> &'static str {
    ERROR_MONITOR
}

pub fn capture_rejection_symbol() -> &'static str {
    CAPTURE_REJECTION_SYMBOL
}
