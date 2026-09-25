use super::EventEmitter;
use tsonic_rust_js::JsValue;

pub struct NodeEventTarget {
    emitter: EventEmitter,
}

impl Default for NodeEventTarget {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeEventTarget {
    pub fn new() -> Self {
        Self {
            emitter: EventEmitter::new(),
        }
    }

    pub fn on<F>(&mut self, event: impl Into<String>, listener: F) -> &mut Self
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.emitter.on(event, listener);
        self
    }

    pub fn add_listener<F>(&mut self, event: impl Into<String>, listener: F) -> &mut Self
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.on(event, listener)
    }

    pub fn once<F>(&mut self, event: impl Into<String>, listener: F) -> &mut Self
    where
        F: FnMut(&[JsValue]) + 'static,
    {
        self.emitter.once(event, listener);
        self
    }

    pub fn off(&mut self, event: &str, listener_id: usize) -> &mut Self {
        self.emitter.off(event, listener_id);
        self
    }

    pub fn remove_listener(&mut self, event: &str, listener_id: usize) -> &mut Self {
        self.off(event, listener_id)
    }

    pub fn remove_all_listeners(&mut self, event: Option<&str>) -> &mut Self {
        self.emitter.remove_all_listeners(event);
        self
    }

    pub fn emit(&mut self, event: &str, args: &[JsValue]) -> bool {
        self.emitter.emit(event, args)
    }

    pub fn listener_count(&self, event: &str) -> usize {
        self.emitter.listener_count(event)
    }

    pub fn event_names(&self) -> Vec<String> {
        self.emitter.event_names()
    }

    pub fn set_max_listeners(&mut self, max: usize) {
        self.emitter.set_max_listeners(max);
    }

    pub fn get_max_listeners(&self) -> usize {
        self.emitter.get_max_listeners()
    }
}
