use std::cell::{Cell, OnceCell, RefCell};
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::ops::Bound::{Excluded, Included, Unbounded};
use std::time::Duration;
use tsonic_rust_runtime::dispatch::{
    DispatchContexts, DispatchGeneration, DispatchGenerationId, DispatchPhase,
};
use tsonic_rust_runtime::dispatch_queue::{TaskBudget, TaskReservation, TaskTicket};

pub(crate) struct NativeResourceBudget {
    maximum: usize,
    budget: OnceCell<TaskBudget>,
}

impl NativeResourceBudget {
    pub(crate) const fn new(maximum: usize) -> Self {
        assert!(maximum > 0);
        Self {
            maximum,
            budget: OnceCell::new(),
        }
    }

    fn budget(&self) -> &TaskBudget {
        self.budget.get_or_init(|| {
            TaskBudget::new(NonZeroUsize::new(self.maximum).expect("finite native resource limit"))
        })
    }

    pub(crate) fn reserve(&self) -> crate::NodeResult<TaskReservation> {
        self.budget().reserve().map_err(crate::NodeError::from)
    }

    pub(crate) fn admit(&self) -> crate::NodeResult<TaskTicket> {
        self.budget().admit().map_err(crate::NodeError::from)
    }
}

#[cfg(test)]
mod tests;

pub(crate) trait RuntimeResource: Clone {
    type Error;

    fn is_alive(&self) -> bool;
    fn phase(&self) -> DispatchPhase;
    fn capture(&self) -> Result<Option<TaskTicket>, Self::Error>;
    fn poll(&self, boundary: Option<TaskTicket>) -> Result<bool, Self::Error>;
    fn has_work(&self) -> bool;
    fn next_delay(&self) -> Option<Duration>;
}

struct Registration<TResource> {
    resource: TResource,
}

pub(crate) struct RuntimeResources<TResource> {
    registrations: RefCell<BTreeMap<TaskTicket, Registration<TResource>>>,
    limit: usize,
    generation: DispatchGeneration,
}

pub struct ResourceFrontier {
    phase: DispatchPhase,
    boundary: Option<TaskTicket>,
    admission_boundary: Option<TaskTicket>,
    after: Cell<Option<TaskTicket>>,
    owner: usize,
    generation: Option<DispatchGenerationId>,
}

impl<TResource> RuntimeResources<TResource> {
    pub(crate) const fn new(limit: usize) -> Self {
        assert!(limit > 0);
        Self {
            registrations: RefCell::new(BTreeMap::new()),
            limit,
            generation: DispatchGeneration::new_closed(),
        }
    }
}

impl<TResource> RuntimeResources<TResource> {
    pub(crate) fn generation(&self) -> Option<DispatchGenerationId> {
        self.generation.capture()
    }

    pub(crate) fn accepts(&self, generation: Option<DispatchGenerationId>) -> bool {
        self.generation.accepts(generation)
    }

    pub(crate) fn enter_owner(&self, owner: tsonic_rust_runtime::dispatch::DispatchOwner) -> bool {
        self.generation.enter(owner)
    }

    pub(crate) fn owns(&self, owner: tsonic_rust_runtime::dispatch::DispatchOwner) -> bool {
        self.generation.owns(owner)
    }

    pub(crate) fn close_owner(&self, owner: tsonic_rust_runtime::dispatch::DispatchOwner) -> bool {
        if !self.generation.close(owner) {
            return false;
        }
        let retired = std::mem::take(&mut *self.registrations.borrow_mut());
        drop(retired);
        true
    }
}

impl<TResource: RuntimeResource> RuntimeResources<TResource> {
    pub(crate) fn register(
        &self,
        ticket: TaskTicket,
        resource: TResource,
    ) -> crate::NodeResult<()> {
        let owner = tsonic_rust_js::event_loop::dispatch_owner().map_err(crate::NodeError::from)?;
        if !self.generation.owns(owner) {
            return Err(crate::NodeError::new(
                "ERR_NODE_RESOURCE_CLOSED",
                "native resource owner is closed",
            ));
        }
        self.register_physical(ticket, resource)
    }

    pub(crate) fn register_physical(
        &self,
        ticket: TaskTicket,
        resource: TResource,
    ) -> crate::NodeResult<()> {
        if self.registrations.borrow().len() >= self.limit {
            self.prune();
        }
        let previous = {
            let mut registrations = self.registrations.borrow_mut();
            if !self.generation.is_open() && registrations.contains_key(&ticket) {
                return Err(crate::NodeError::new(
                    "ERR_NODE_RESOURCE_CLOSED",
                    "closed native resource registrations cannot be replaced",
                ));
            }
            if registrations.len() >= self.limit && !registrations.contains_key(&ticket) {
                return Err(crate::NodeError::new(
                    "ERR_NODE_RESOURCE_LIMIT",
                    "native resources exceed the finite registration limit",
                ));
            }
            registrations.insert(ticket, Registration { resource })
        };
        drop(previous);
        Ok(())
    }

    fn retain_registered(&self, mut retain: impl FnMut(&TResource) -> bool) {
        let boundary = self
            .registrations
            .borrow()
            .last_key_value()
            .map(|(ticket, _)| *ticket);
        let Some(boundary) = boundary else {
            return;
        };
        let mut after = None;
        loop {
            let next = self
                .registrations
                .borrow()
                .range((after.map_or(Unbounded, Excluded), Included(boundary)))
                .next()
                .map(|(ticket, entry)| (*ticket, entry.resource.clone()));
            let Some((ticket, resource)) = next else {
                return;
            };
            after = Some(ticket);
            if !retain(&resource) {
                let removed = self.registrations.borrow_mut().remove(&ticket);
                drop(removed);
            }
        }
    }

    pub(crate) fn visit(&self, mut operation: impl FnMut(&TResource)) {
        self.retain_registered(|resource| {
            operation(resource);
            true
        });
    }

    pub(crate) fn close_owner_with(
        &self,
        owner: tsonic_rust_runtime::dispatch::DispatchOwner,
        retain: impl FnMut(&TResource) -> bool,
    ) -> bool {
        if !self.generation.close(owner) {
            return false;
        }
        self.retain_registered(retain);
        true
    }

    fn prune(&self) {
        let boundary = self
            .registrations
            .borrow()
            .last_key_value()
            .map(|(ticket, _)| *ticket);
        let Some(boundary) = boundary else {
            return;
        };
        let mut after = None;
        loop {
            let removed = {
                let mut registrations = self.registrations.borrow_mut();
                let ticket = registrations
                    .range((after.map_or(Unbounded, Excluded), Included(boundary)))
                    .find(|(_, entry)| !entry.resource.is_alive())
                    .map(|(ticket, _)| *ticket);
                let Some(ticket) = ticket else {
                    return;
                };
                after = Some(ticket);
                registrations.remove(&ticket)
            };
            drop(removed);
        }
    }

    fn next_resource(&self, frontier: &ResourceFrontier) -> Option<(TaskTicket, TResource)> {
        if frontier.owner != self as *const Self as usize
            || !self.generation.accepts(frontier.generation)
        {
            return None;
        }
        let boundary = frontier.boundary?;
        self.registrations
            .borrow()
            .range((
                frontier.after.get().map_or(Unbounded, Excluded),
                Included(boundary),
            ))
            .find(|(_, entry)| {
                entry.resource.phase() == frontier.phase && entry.resource.is_alive()
            })
            .map(|(ticket, entry)| (*ticket, entry.resource.clone()))
    }
}

impl<TResource: RuntimeResource> DispatchContexts for RuntimeResources<TResource> {
    type Error = TResource::Error;
    type Frontier = ResourceFrontier;

    fn enter(&self, owner: tsonic_rust_runtime::dispatch::DispatchOwner) -> bool {
        self.enter_owner(owner)
    }

    fn close(&self, owner: tsonic_rust_runtime::dispatch::DispatchOwner) {
        self.close_owner(owner);
    }

    fn prepare(&self, phase: DispatchPhase) -> Result<Self::Frontier, Self::Error> {
        self.prune();
        let registrations = self.registrations.borrow();
        let mut frontier = ResourceFrontier {
            phase,
            boundary: registrations.last_key_value().map(|(ticket, _)| *ticket),
            admission_boundary: None,
            after: Cell::new(None),
            owner: self as *const Self as usize,
            generation: self.generation.capture(),
        };
        drop(registrations);
        while let Some((ticket, resource)) = self.next_resource(&frontier) {
            frontier.after.set(Some(ticket));
            if let Some(admitted) = resource.capture()? {
                frontier.admission_boundary = Some(
                    frontier
                        .admission_boundary
                        .map_or(admitted, |current| current.max(admitted)),
                );
            }
        }
        frontier.after.set(None);
        Ok(frontier)
    }

    fn next_ready(&self, frontier: &Self::Frontier) -> Option<u64> {
        self.next_resource(frontier)
            .map(|(ticket, _)| ticket.sequence())
    }

    fn poll_next(&self, frontier: &Self::Frontier) -> Result<bool, Self::Error> {
        let Some((ticket, resource)) = self.next_resource(frontier) else {
            return Ok(false);
        };
        frontier.after.set(Some(ticket));
        resource.poll(frontier.admission_boundary)
    }

    fn has_work(&self) -> bool {
        self.registrations
            .borrow()
            .values()
            .any(|entry| entry.resource.has_work())
    }

    fn next_delay(&self) -> Option<Duration> {
        self.registrations
            .borrow()
            .values()
            .filter_map(|entry| entry.resource.next_delay())
            .min()
    }
}
