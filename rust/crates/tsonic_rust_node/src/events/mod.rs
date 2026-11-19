mod async_resource;
mod callable_listeners;
mod callback;
mod keys;

#[cfg(test)]
mod tests;

mod event_target;

pub use async_resource::{EventEmitterAsyncResource, EventEmitterAsyncResourceOptions};
pub use event_target::NodeEventTarget;

pub(crate) use callback::{EventCallback, ListenerCallback as RetainedEventCallback};
pub(crate) use keys::{EventListenerMap, EventName};

use crate::error::{NodeError, NodeResult};
use callback::ListenerCallback;
use keys::EventKey;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tsonic_rust_js::JsValue;

type Listener = Box<dyn FnMut(&[JsValue])>;
type ListenerMap = HashMap<String, Vec<ListenerEntry>>;
static DEFAULT_MAX_LISTENERS: AtomicUsize = AtomicUsize::new(10);
static CAPTURE_REJECTIONS: AtomicBool = AtomicBool::new(false);

pub const ERROR_MONITOR: &str = "events.errorMonitor";
pub const CAPTURE_REJECTION_SYMBOL: &str = "events.captureRejectionSymbol";

struct ListenerEntry {
    id: usize,
    once: bool,
    callback: Listener,
}

struct CallableListenerEntry<E: 'static> {
    identity: usize,
    callback: ListenerCallback<E>,
}

pub(crate) struct CallableEmission<E: 'static> {
    listeners: Option<Rc<Vec<CallableListenerEntry<E>>>>,
}

impl<E: 'static> CallableEmission<E> {
    pub(crate) fn invoke(self, arguments: &[JsValue]) -> Result<bool, E> {
        self.invoke_with_before(arguments, || {})
    }

    pub(crate) fn invoke_with_before(
        self,
        arguments: &[JsValue],
        before: impl Fn(),
    ) -> Result<bool, E> {
        let Some(listeners) = self.listeners else {
            return Ok(false);
        };
        let mut dispatched = false;
        for listener in listeners.iter() {
            dispatched |= listener.callback.invoke(arguments, &before)?;
        }
        Ok(dispatched)
    }
}

impl<E: 'static> Clone for CallableListenerEntry<E> {
    fn clone(&self) -> Self {
        Self {
            identity: self.identity,
            callback: self.callback.clone(),
        }
    }
}

struct EventEmitterState<E: 'static> {
    listeners: ListenerMap,
    listener_event_order: Vec<String>,
    callable_listeners: EventListenerMap<CallableListenerEntry<E>>,
    callable_event_order: Vec<EventKey>,
    max_listeners: Option<usize>,
    next_listener_id: usize,
    capture_rejections: bool,
}

impl<E: 'static> Default for EventEmitterState<E> {
    fn default() -> Self {
        Self {
            listeners: HashMap::new(),
            listener_event_order: Vec::new(),
            callable_listeners: EventListenerMap::new(),
            callable_event_order: Vec::new(),
            max_listeners: None,
            next_listener_id: 0,
            capture_rejections: false,
        }
    }
}

pub struct EventEmitter<E: 'static = NodeError> {
    state: Rc<RefCell<EventEmitterState<E>>>,
}

pub(crate) struct WeakEventEmitter<E: 'static> {
    state: Weak<RefCell<EventEmitterState<E>>>,
}

impl<E: 'static> Clone for EventEmitter<E> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
}

impl<E: 'static> Clone for WeakEventEmitter<E> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl<E: 'static> WeakEventEmitter<E> {
    pub(crate) fn is_alive(&self) -> bool {
        self.state.strong_count() != 0
    }
    pub(crate) fn upgrade(&self) -> Option<EventEmitter<E>> {
        Some(EventEmitter {
            state: self.state.upgrade()?,
        })
    }
}

impl<E: 'static> Default for EventEmitter<E> {
    fn default() -> Self {
        Self {
            state: Rc::new(RefCell::new(EventEmitterState::default())),
        }
    }
}

impl<E: 'static> EventEmitter<E> {
    pub fn new() -> Self {
        Self::with_options(EventEmitterOptions {
            capture_rejections: capture_rejections(),
        })
    }

    pub fn with_options(options: EventEmitterOptions) -> Self {
        Self {
            state: Rc::new(RefCell::new(EventEmitterState {
                max_listeners: Some(default_max_listeners()),
                capture_rejections: options.capture_rejections,
                ..EventEmitterState::default()
            })),
        }
    }

    pub(crate) fn downgrade(&self) -> WeakEventEmitter<E> {
        WeakEventEmitter {
            state: Rc::downgrade(&self.state),
        }
    }

    pub fn capture_rejections(&self) -> bool {
        self.state.borrow().capture_rejections
    }

    fn add_entry<F>(&self, event: String, once: bool, prepend: bool, listener: F) -> usize
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        let mut state = self.state.borrow_mut();
        state.next_listener_id = state
            .next_listener_id
            .checked_add(1)
            .expect("native listener identity range");
        let id = state.next_listener_id;
        let entry = ListenerEntry {
            id,
            once,
            callback: Box::new(listener),
        };
        let is_new_event = !state.listeners.contains_key(&event);
        let listeners = state.listeners.entry(event.clone()).or_default();
        if prepend {
            listeners.insert(0, entry);
        } else {
            listeners.push(entry);
        }
        if is_new_event {
            state.listener_event_order.push(event);
        }
        id
    }

    pub fn on<F>(&self, event: impl Into<String>, listener: F) -> &Self
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.add_entry(event.into(), false, false, listener);
        self
    }

    pub fn on_with_id<F>(&self, event: impl Into<String>, listener: F) -> usize
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.add_entry(event.into(), false, false, listener)
    }

    pub fn prepend_listener<F>(&self, event: impl Into<String>, listener: F) -> &Self
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.add_entry(event.into(), false, true, listener);
        self
    }

    pub fn prepend_listener_with_id<F>(&self, event: impl Into<String>, listener: F) -> usize
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.add_entry(event.into(), false, true, listener)
    }

    pub fn once<F>(&self, event: impl Into<String>, listener: F) -> &Self
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.add_entry(event.into(), true, false, listener);
        self
    }

    pub fn once_with_id<F>(&self, event: impl Into<String>, listener: F) -> usize
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.add_entry(event.into(), true, false, listener)
    }

    pub fn prepend_once_listener<F>(&self, event: impl Into<String>, listener: F) -> &Self
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.add_entry(event.into(), true, true, listener);
        self
    }

    pub fn prepend_once_listener_with_id<F>(&self, event: impl Into<String>, listener: F) -> usize
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.add_entry(event.into(), true, true, listener)
    }

    pub fn add_listener<F>(&self, event: impl Into<String>, listener: F) -> &Self
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.on(event, listener)
    }

    pub fn off_by_id(&self, event: &str, listener_id: usize) -> &Self {
        let removed = {
            let mut state = self.state.borrow_mut();
            let removed = state.listeners.get_mut(event).and_then(|listeners| {
                listeners
                    .iter()
                    .position(|listener| listener.id == listener_id)
                    .map(|index| listeners.remove(index))
            });
            if state.listeners.get(event).is_some_and(Vec::is_empty) {
                state.listeners.remove(event);
                state
                    .listener_event_order
                    .retain(|candidate| candidate != event);
            }
            removed
        };
        drop(removed);
        self
    }

    pub fn remove_listener_by_id(&self, event: &str, listener_id: usize) -> &Self {
        self.off_by_id(event, listener_id)
    }
    pub fn remove_listener(&self, event: &str, listener_id: usize) -> &Self {
        self.off_by_id(event, listener_id)
    }
    pub fn off(&self, event: &str, listener_id: usize) -> &Self {
        self.off_by_id(event, listener_id)
    }

    pub fn listeners(&self, event: &str) -> Vec<usize> {
        self.state
            .borrow()
            .listeners
            .get(event)
            .map(|listeners| listeners.iter().map(|listener| listener.id).collect())
            .unwrap_or_default()
    }

    pub fn raw_listeners(&self, event: &str) -> Vec<usize> {
        self.listeners(event)
    }

    pub fn emit(&self, event: &str, args: &[JsValue]) -> bool {
        let Some(mut listeners) = self.state.borrow_mut().listeners.remove(event) else {
            return false;
        };
        if listeners.is_empty() {
            return false;
        }
        for listener in &mut listeners {
            (listener.callback)(args);
        }
        listeners.retain(|listener| !listener.once);
        let mut state = self.state.borrow_mut();
        if listeners.is_empty() && !state.listeners.contains_key(event) {
            state
                .listener_event_order
                .retain(|candidate| candidate != event);
        } else if !listeners.is_empty() {
            if let Some(added) = state.listeners.remove(event) {
                listeners.extend(added);
            }
            state.listeners.insert(event.to_owned(), listeners);
        }
        true
    }

    pub fn listener_count(&self, event: &str) -> usize {
        self.state.borrow().listeners.get(event).map_or(0, Vec::len)
    }

    pub fn event_names(&self) -> Vec<String> {
        let state = self.state.borrow();
        state
            .listener_event_order
            .iter()
            .filter(|event| state.listeners.contains_key(*event))
            .cloned()
            .collect()
    }

    pub fn remove_all_listeners(&self, event: Option<&str>) -> &Self {
        if let Some(event) = event {
            let removed = {
                let mut state = self.state.borrow_mut();
                state
                    .listener_event_order
                    .retain(|candidate| candidate != event);
                state.listeners.remove(event)
            };
            drop(removed);
        } else {
            let removed = {
                let mut state = self.state.borrow_mut();
                state.listener_event_order.clear();
                std::mem::take(&mut state.listeners)
            };
            drop(removed);
        }
        self
    }

    pub fn set_max_listeners(&self, max: usize) -> &Self {
        self.state.borrow_mut().max_listeners = Some(max);
        self
    }
    pub fn get_max_listeners(&self) -> usize {
        self.state.borrow().max_listeners.unwrap_or(0)
    }
    pub fn has_listeners(&self, event: &str) -> bool {
        self.listener_count(event) > 0
    }
}
pub(crate) fn unhandled_error(arguments: &[JsValue]) -> NodeError {
    let detail = arguments
        .first()
        .map(JsValue::inspect)
        .unwrap_or_else(|| "null".to_string());
    NodeError::new(
        "ERR_UNHANDLED_ERROR",
        format!("Unhandled 'error' event ({detail})"),
    )
}

pub fn listener_count_callable<E: 'static>(
    emitter: &EventEmitter<E>,
    event: &JsValue,
) -> NodeResult<usize> {
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
