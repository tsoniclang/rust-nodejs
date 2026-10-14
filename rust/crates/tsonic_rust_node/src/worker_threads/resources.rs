use super::port::{capture_parent, MessagePort, MessagePortState, PortFrontier, RuntimePort};
use super::worker::{RuntimeWorker, Worker};
use crate::error::{NodeError, NodeResult};
use crate::runtime_resources::{
    NativeResourceBudget, ResourceFrontier, RuntimeResource, RuntimeResources,
};
use std::cell::OnceCell;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use tsonic_rust_runtime::dispatch::{DispatchContexts, DispatchPhase};
use tsonic_rust_runtime::dispatch_queue::{TaskReservation, TaskTicket};

const MAXIMUM_WORKER_RESOURCES: usize = 1 << 20;

thread_local! {
    static RESOURCE_BUDGET: NativeResourceBudget = const { NativeResourceBudget::new(MAXIMUM_WORKER_RESOURCES) };
    static DEFAULT_RESOURCES: WorkerResources<tsonic_rust_runtime::TsonicError> = const { WorkerResources::new() };
}

enum WorkerResource<E: 'static> {
    Port(RuntimePort<E>),
    Worker(RuntimeWorker<E>),
}

impl<E: 'static> Clone for WorkerResource<E> {
    fn clone(&self) -> Self {
        match self {
            Self::Port(resource) => Self::Port(resource.clone()),
            Self::Worker(resource) => Self::Worker(resource.clone()),
        }
    }
}

impl<E: From<NodeError> + 'static> RuntimeResource for WorkerResource<E> {
    type Error = E;

    fn is_alive(&self) -> bool {
        match self {
            Self::Port(resource) => resource.is_alive(),
            Self::Worker(resource) => resource.is_alive(),
        }
    }

    fn phase(&self) -> DispatchPhase {
        match self {
            Self::Port(_) => crate::phases::PORTS,
            Self::Worker(_) => crate::phases::WORKERS,
        }
    }

    fn capture(&self) -> Result<Option<TaskTicket>, E> {
        match self {
            Self::Port(resource) => resource.upgrade().map_or(Ok(None), |port| port.capture()),
            Self::Worker(resource) => resource
                .upgrade()
                .map_or(Ok(None), |worker| worker.capture()),
        }
    }

    fn poll(&self, boundary: Option<TaskTicket>) -> Result<bool, E> {
        match self {
            Self::Port(resource) => resource
                .upgrade()
                .map_or(Ok(false), |port| port.poll(boundary)),
            Self::Worker(resource) => resource
                .upgrade()
                .map_or(Ok(false), |worker| worker.poll(boundary)),
        }
    }

    fn has_work(&self) -> bool {
        match self {
            Self::Port(resource) => resource
                .upgrade()
                .is_some_and(|port| port.is_refed_active()),
            Self::Worker(resource) => resource
                .upgrade()
                .is_some_and(|worker| worker.is_refed_active()),
        }
    }

    fn next_delay(&self) -> Option<Duration> {
        match self {
            Self::Worker(resource) => resource
                .upgrade()
                .and_then(|worker| worker.next_reap_delay()),
            Self::Port(_) => None,
        }
    }
}

pub struct WorkerResources<E: 'static> {
    resources: RuntimeResources<WorkerResource<E>>,
    parent: OnceCell<MessagePort<E>>,
}

pub struct WorkerFrontier {
    owner: usize,
    generation: Option<tsonic_rust_runtime::dispatch::DispatchGenerationId>,
    resources: ResourceFrontier,
    parent: Option<PortFrontier>,
}

impl<E: 'static> Default for WorkerResources<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: 'static> WorkerResources<E> {
    pub const fn new() -> Self {
        Self {
            resources: RuntimeResources::new(MAXIMUM_WORKER_RESOURCES),
            parent: OnceCell::new(),
        }
    }
}

impl<E: From<NodeError> + 'static> WorkerResources<E> {
    pub(super) fn register_port(&self, port: &MessagePort<E>) -> NodeResult<()> {
        self.resources
            .register_physical(port.ticket(), WorkerResource::Port(port.downgrade()))?;
        self.activate_port(port);
        Ok(())
    }

    fn activate_port(&self, port: &MessagePort<E>) {
        if let Ok(owner) = tsonic_rust_js::event_loop::dispatch_owner() {
            if self.resources.owns(owner) {
                port.activate(owner);
            }
        }
    }

    pub(super) fn register_worker(&self, worker: &Worker<E>) -> NodeResult<()> {
        self.resources
            .register(worker.ticket(), WorkerResource::Worker(worker.downgrade()))
    }

    pub(super) fn parent_port(&self) -> NodeResult<Option<MessagePort<E>>> {
        if let Some(port) = self.parent.get() {
            return Ok(Some(port.clone()));
        }
        let Some(state) = super::parent_port_state() else {
            return Ok(None);
        };
        self.bind_parent(state).map(Some)
    }

    pub(super) fn bind_parent(
        &self,
        state: Rc<RefCell<MessagePortState>>,
    ) -> NodeResult<MessagePort<E>> {
        let port = MessagePort::from_state(state)?;
        self.activate_port(&port);
        self.parent.set(port.clone()).map_err(|_| {
            NodeError::new(
                "ERR_WORKER_PARENT_PORT",
                "worker parent port was initialized twice",
            )
        })?;
        Ok(port)
    }
}

impl<E: From<NodeError> + 'static> DispatchContexts for WorkerResources<E> {
    type Error = E;
    type Frontier = WorkerFrontier;

    fn enter(&self, owner: tsonic_rust_runtime::dispatch::DispatchOwner) -> bool {
        if !self.resources.enter(owner) {
            return false;
        }
        self.resources.visit(|resource| {
            if let WorkerResource::Port(resource) = resource {
                if let Some(port) = resource.upgrade() {
                    port.activate(owner);
                }
            }
        });
        if let Some(parent) = self.parent.get() {
            parent.activate(owner);
        }
        true
    }
    fn close(&self, owner: tsonic_rust_runtime::dispatch::DispatchOwner) {
        if !self.resources.owns(owner) {
            return;
        }
        self.resources.visit(|resource| {
            if let WorkerResource::Port(resource) = resource {
                if let Some(port) = resource.upgrade() {
                    port.deactivate();
                }
            }
        });
        if let Some(parent) = self.parent.get() {
            parent.deactivate();
        }
        if self
            .resources
            .close_owner_with(owner, |resource| match resource {
                WorkerResource::Port(resource) => {
                    if let Some(port) = resource.upgrade() {
                        port.detach();
                    }
                    resource.is_alive()
                }
                WorkerResource::Worker(_) => false,
            })
        {
            if let Some(parent) = self.parent.get() {
                parent.detach();
            }
        }
    }

    fn prepare(&self, phase: DispatchPhase) -> Result<Self::Frontier, E> {
        let resources = self.resources.prepare(phase)?;
        let generation = self.resources.generation();
        let parent = if phase == crate::phases::PORTS && self.resources.accepts(generation) {
            self.parent
                .get()
                .map(MessagePort::physical_state)
                .or_else(super::parent_port_state)
                .map(capture_parent)
                .transpose()
                .map_err(E::from)?
        } else {
            None
        };
        Ok(WorkerFrontier {
            owner: self as *const Self as usize,
            generation,
            resources,
            parent,
        })
    }

    fn next_ready(&self, frontier: &Self::Frontier) -> Option<u64> {
        if frontier.owner != self as *const Self as usize
            || !self.resources.accepts(frontier.generation)
        {
            return None;
        }
        let resources = self.resources.next_ready(&frontier.resources);
        let parent = frontier
            .parent
            .as_ref()
            .and_then(|frontier| self.parent.get()?.next_shared(frontier));
        match (resources, parent) {
            (Some(resources), Some(parent)) => Some(resources.min(parent)),
            (resources, parent) => resources.or(parent),
        }
    }

    fn poll_next(&self, frontier: &Self::Frontier) -> Result<bool, E> {
        if frontier.owner != self as *const Self as usize
            || !self.resources.accepts(frontier.generation)
        {
            return Ok(false);
        }
        let resources = self.resources.next_ready(&frontier.resources);
        if let (Some(port), Some(parent)) = (self.parent.get(), frontier.parent.as_ref()) {
            if let Some(ticket) = port.next_shared(parent) {
                if resources.is_none_or(|resources| ticket <= resources) {
                    return port.poll_shared(parent);
                }
            }
        }
        self.resources.poll_next(&frontier.resources)
    }

    fn has_work(&self) -> bool {
        self.resources.has_work() || self.parent.get().is_some_and(MessagePort::is_refed_active)
    }
    fn next_delay(&self) -> Option<Duration> {
        self.resources.next_delay()
    }
}

pub(super) fn reserve_resource() -> NodeResult<TaskReservation> {
    RESOURCE_BUDGET.with(NativeResourceBudget::reserve)
}

pub(super) fn admit_signal() -> NodeResult<TaskTicket> {
    RESOURCE_BUDGET.with(NativeResourceBudget::admit)
}

pub fn with_default<TOutput>(
    operation: impl FnOnce(&WorkerResources<tsonic_rust_runtime::TsonicError>) -> TOutput,
) -> TOutput {
    DEFAULT_RESOURCES.with(operation)
}
