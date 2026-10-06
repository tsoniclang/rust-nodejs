use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Included, Unbounded};
use std::time::Duration;
use tsonic_rust_runtime::dispatch::{DispatchContexts, DispatchPhase};
use tsonic_rust_runtime::dispatch_queue::TaskTicket;

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
}

pub struct ResourceFrontier {
    phase: DispatchPhase,
    boundary: Option<TaskTicket>,
    admission_boundary: Option<TaskTicket>,
    after: Cell<Option<TaskTicket>>,
    owner: usize,
}

impl<TResource> RuntimeResources<TResource> {
    pub(crate) const fn new(limit: usize) -> Self {
        assert!(limit > 0);
        Self {
            registrations: RefCell::new(BTreeMap::new()),
            limit,
        }
    }
}

impl<TResource: RuntimeResource> RuntimeResources<TResource> {
    pub(crate) fn register(&self, ticket: TaskTicket, resource: TResource) {
        let mut registrations = self.registrations.borrow_mut();
        if registrations.len() >= self.limit {
            registrations.retain(|_, entry| entry.resource.is_alive());
        }
        assert!(registrations.len() < self.limit || registrations.contains_key(&ticket));
        registrations.insert(ticket, Registration { resource });
    }

    fn next_resource(&self, frontier: &ResourceFrontier) -> Option<(TaskTicket, TResource)> {
        if frontier.owner != self as *const Self as usize {
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

    fn prepare(&self, phase: DispatchPhase) -> Result<Self::Frontier, Self::Error> {
        let mut registrations = self.registrations.borrow_mut();
        registrations.retain(|_, entry| entry.resource.is_alive());
        let mut frontier = ResourceFrontier {
            phase,
            boundary: registrations.last_key_value().map(|(ticket, _)| *ticket),
            admission_boundary: None,
            after: Cell::new(None),
            owner: self as *const Self as usize,
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
