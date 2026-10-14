use std::cell::{Cell, OnceCell};
use std::num::NonZeroUsize;
use std::time::Duration;

use crate::error::{NodeError, NodeResult};
use tsonic_rust_runtime::dispatch::{DispatchContexts, DispatchPhase};
use tsonic_rust_runtime::dispatch_queue::{TaskBudget, TaskHandle, TaskQueue, TaskTicket};
use tsonic_rust_runtime::TsonicError;

const MAX_PENDING_RUNTIME_TASKS: usize = 1 << 20;

thread_local! {
    static TASK_BUDGET: OnceCell<TaskBudget> = const { OnceCell::new() };
    static DEFAULT_TASKS: RuntimeTasks<TsonicError> = const { RuntimeTasks::new() };
}

pub struct RuntimeTasks<TError> {
    queue: OnceCell<TaskQueue<TError>>,
    owner: Cell<Option<tsonic_rust_runtime::dispatch::DispatchOwner>>,
}

impl<TError> Default for RuntimeTasks<TError> {
    fn default() -> Self {
        Self::new()
    }
}

impl<TError> RuntimeTasks<TError> {
    pub const fn new() -> Self {
        Self {
            queue: OnceCell::new(),
            owner: Cell::new(None),
        }
    }

    pub fn handle(&self) -> TaskHandle<TError> {
        self.scheduling_queue().handle()
    }

    pub fn enqueue(
        &self,
        callback: impl FnOnce() -> Result<(), TError> + 'static,
    ) -> NodeResult<()> {
        let owner = tsonic_rust_js::event_loop::dispatch_owner().map_err(NodeError::from)?;
        if self.owner.get() != Some(owner) {
            return Err(NodeError::new(
                "ERR_NODE_RUNTIME_TASK_CLOSED",
                "native task context is not registered with this event loop",
            ));
        }
        self.scheduling_queue()
            .enqueue(callback)
            .map(|_| ())
            .map_err(NodeError::from)
    }

    pub fn poll(&self) -> Result<bool, TError> {
        self.queue.get().map_or(Ok(false), TaskQueue::poll_ready)
    }

    pub fn has_pending_work(&self) -> bool {
        self.queue
            .get()
            .is_some_and(|queue| queue.front_ticket().is_some())
    }

    fn scheduling_queue(&self) -> &TaskQueue<TError> {
        self.queue.get_or_init(|| {
            let budget = TASK_BUDGET.with(|budget| {
                budget
                    .get_or_init(|| {
                        TaskBudget::new(
                            NonZeroUsize::new(MAX_PENDING_RUNTIME_TASKS)
                                .expect("finite native task limit"),
                        )
                    })
                    .clone()
            });
            let queue = TaskQueue::new_closed(budget);
            if let Some(owner) = self.owner.get() {
                assert!(
                    queue.enter(owner),
                    "a fresh native queue accepts its registered owner"
                );
            }
            queue
        })
    }
}

impl<TError> DispatchContexts for RuntimeTasks<TError> {
    type Error = TError;
    type Frontier = Option<TaskTicket>;

    fn enter(&self, owner: tsonic_rust_runtime::dispatch::DispatchOwner) -> bool {
        if !owner.claim(&self.owner) {
            return false;
        }
        self.queue.get().is_none_or(|queue| queue.enter(owner))
    }

    fn close(&self, owner: tsonic_rust_runtime::dispatch::DispatchOwner) {
        if owner.release(&self.owner) {
            if let Some(queue) = self.queue.get() {
                queue.close(owner);
            }
        }
    }

    fn prepare(&self, phase: DispatchPhase) -> Result<Self::Frontier, Self::Error> {
        Ok(if phase == crate::phases::RUNTIME_TASKS {
            self.queue.get().and_then(TaskQueue::ready_boundary)
        } else {
            None
        })
    }

    fn next_ready(&self, frontier: &Self::Frontier) -> Option<u64> {
        let boundary = (*frontier)?;
        let ticket = self.queue.get()?.front_ticket()?;
        (ticket <= boundary).then_some(ticket.sequence())
    }

    fn poll_next(&self, frontier: &Self::Frontier) -> Result<bool, Self::Error> {
        if self.next_ready(frontier).is_none() {
            return Ok(false);
        }
        self.queue
            .get()
            .expect("prepared native task owner")
            .poll_one()
    }

    fn has_work(&self) -> bool {
        self.has_pending_work()
    }

    fn next_delay(&self) -> Option<Duration> {
        None
    }
}

pub(crate) fn with_default<TOutput>(
    operation: impl FnOnce(&RuntimeTasks<TsonicError>) -> TOutput,
) -> TOutput {
    DEFAULT_TASKS.with(operation)
}

#[cfg(test)]
mod tests;
