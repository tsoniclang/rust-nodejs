use super::{EventEmitter, EventEmitterOptions};
use crate::async_hooks::{AsyncResource, AsyncResourceOptions};

pub struct EventEmitterAsyncResource {
    emitter: EventEmitter,
    async_resource: AsyncResource,
}

impl EventEmitterAsyncResource {
    pub fn new(options: EventEmitterAsyncResourceOptions) -> Self {
        let async_resource = AsyncResource::new(
            options.name.unwrap_or_else(|| "EventEmitter".to_string()),
            Some(AsyncResourceOptions {
                trigger_async_id: options.trigger_async_id,
                require_manual_destroy: options.require_manual_destroy,
            }),
        );
        Self {
            emitter: EventEmitter::with_options(EventEmitterOptions {
                capture_rejections: options.capture_rejections,
            }),
            async_resource,
        }
    }

    pub fn event_emitter(&self) -> &EventEmitter {
        &self.emitter
    }

    pub fn event_emitter_mut(&mut self) -> &mut EventEmitter {
        &mut self.emitter
    }

    pub fn async_resource(&self) -> &AsyncResource {
        &self.async_resource
    }

    pub fn async_id(&self) -> u64 {
        self.async_resource.async_id()
    }

    pub fn trigger_async_id(&self) -> u64 {
        self.async_resource.trigger_async_id()
    }

    pub fn emit_destroy(&mut self) {
        self.async_resource.emit_destroy();
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventEmitterAsyncResourceOptions {
    pub name: Option<String>,
    pub trigger_async_id: Option<u64>,
    pub require_manual_destroy: bool,
    pub capture_rejections: bool,
}
