use super::port::{MessagePort, RuntimePort};
use super::worker::{RuntimeWorker, Worker};
use crate::error::{NodeError, NodeResult};
use crate::runtime_resources::{ResourceFrontier, RuntimeResource, RuntimeResources};
use std::cell::OnceCell;
use std::num::NonZeroUsize;
use std::time::Duration;
use tsonic_rust_runtime::dispatch::{DispatchContexts, DispatchPhase};
use tsonic_rust_runtime::dispatch_queue::{TaskBudget, TaskReservation, TaskTicket};

const MAXIMUM_WORKER_RESOURCES: usize = 1 << 20;

thread_local! {
    static RESOURCE_BUDGET: OnceCell<TaskBudget> = const { OnceCell::new() };
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
            Self::Port(_) => DispatchPhase::Ports,
            Self::Worker(_) => DispatchPhase::Workers,
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
    pub(super) fn register_port(&self, port: &MessagePort<E>) {
        self.resources
            .register(port.ticket(), WorkerResource::Port(port.downgrade()));
    }

    pub(super) fn register_worker(&self, worker: &Worker<E>) {
        self.resources
            .register(worker.ticket(), WorkerResource::Worker(worker.downgrade()));
    }

    pub(super) fn parent_port(&self) -> NodeResult<Option<MessagePort<E>>> {
        if let Some(port) = self.parent.get() {
            return Ok(Some(port.clone()));
        }
        let Some(state) = super::parent_port_state() else {
            return Ok(None);
        };
        let port = MessagePort::from_state(self, state)?;
        self.parent.set(port.clone()).map_err(|_| {
            NodeError::new(
                "ERR_WORKER_PARENT_PORT",
                "worker parent port was initialized twice",
            )
        })?;
        Ok(Some(port))
    }
}

impl<E: From<NodeError> + 'static> DispatchContexts for WorkerResources<E> {
    type Error = E;
    type Frontier = ResourceFrontier;

    fn prepare(&self, phase: DispatchPhase) -> Result<Self::Frontier, E> {
        self.resources.prepare(phase)
    }

    fn next_ready(&self, frontier: &Self::Frontier) -> Option<u64> {
        self.resources.next_ready(frontier)
    }

    fn poll_next(&self, frontier: &Self::Frontier) -> Result<bool, E> {
        self.resources.poll_next(frontier)
    }

    fn has_work(&self) -> bool {
        self.resources.has_work()
    }
    fn next_delay(&self) -> Option<Duration> {
        self.resources.next_delay()
    }
}

pub(super) fn reserve_resource() -> NodeResult<TaskReservation> {
    RESOURCE_BUDGET.with(|budget| {
        budget
            .get_or_init(|| {
                TaskBudget::new(
                    NonZeroUsize::new(MAXIMUM_WORKER_RESOURCES)
                        .expect("finite worker resource limit"),
                )
            })
            .reserve()
            .map_err(NodeError::from)
    })
}

pub(super) fn admit_signal() -> NodeResult<TaskTicket> {
    RESOURCE_BUDGET.with(|budget| {
        budget
            .get_or_init(|| {
                TaskBudget::new(
                    NonZeroUsize::new(MAXIMUM_WORKER_RESOURCES)
                        .expect("finite worker resource limit"),
                )
            })
            .admit()
            .map_err(NodeError::from)
    })
}

pub fn with_default<TOutput>(
    operation: impl FnOnce(&WorkerResources<tsonic_rust_runtime::TsonicError>) -> TOutput,
) -> TOutput {
    DEFAULT_RESOURCES.with(operation)
}
