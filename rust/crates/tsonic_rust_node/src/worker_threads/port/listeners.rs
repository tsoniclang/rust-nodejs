use crate::events::{EventCallback, EventListenerMap, EventName, RetainedEventCallback};

const MAXIMUM_PORT_LISTENERS: usize = 1 << 16;

trait PortListenerLifecycle {
    fn retire(&self, ticket: TaskTicket, slot: usize, captured: bool);
    fn is_registered(&self, ticket: TaskTicket, slot: usize) -> bool;
    fn prune(&self);
}

struct PortOwnerRegistration {
    ticket: TaskTicket,
    owner: Weak<dyn PortListenerLifecycle>,
}

struct PortOwners {
    first: Option<PortOwnerRegistration>,
    others: Vec<PortOwnerRegistration>,
}

impl PortOwners {
    fn new() -> Self {
        Self {
            first: None,
            others: Vec::new(),
        }
    }

    fn iter(&self) -> impl DoubleEndedIterator<Item = &PortOwnerRegistration> {
        self.first.iter().chain(self.others.iter())
    }

    fn push(&mut self, owner: PortOwnerRegistration) {
        if self.first.is_none() && self.others.is_empty() {
            self.first = Some(owner);
        } else {
            self.others.push(owner);
        }
    }

    fn remove(&mut self, ticket: TaskTicket) {
        if self
            .first
            .as_ref()
            .is_some_and(|owner| owner.ticket == ticket)
        {
            self.first = None;
        }
        self.others.retain(|owner| owner.ticket != ticket);
    }

    fn get(&self, ticket: TaskTicket) -> Option<&Weak<dyn PortListenerLifecycle>> {
        if let Some(owner) = self.first.as_ref().filter(|entry| entry.ticket == ticket) {
            return Some(&owner.owner);
        }
        self.others
            .binary_search_by_key(&ticket, |entry| entry.ticket)
            .ok()
            .map(|index| &self.others[index].owner)
    }
}

#[derive(Clone)]
struct PortRoute {
    owner: TaskTicket,
    ticket: TaskTicket,
    identity: usize,
    once: bool,
    slot: usize,
}

struct PortRouting {
    owners: PortOwners,
    events: EventListenerMap<PortRoute>,
    listeners: usize,
    active: Option<PortEmission>,
    cleanup_pending: bool,
}

impl PortRouting {
    fn new() -> Self {
        Self {
            owners: PortOwners::new(),
            events: EventListenerMap::new(),
            listeners: 0,
            active: None,
            cleanup_pending: false,
        }
    }

    fn owner(&self, ticket: TaskTicket) -> Option<Weak<dyn PortListenerLifecycle>> {
        self.owners.get(ticket).cloned()
    }

    fn captured(&self, ticket: TaskTicket) -> bool {
        self.active.as_ref().is_some_and(|emission| {
            emission.routes.as_ref().is_some_and(|routes| {
                routes
                    .binary_search_by_key(&ticket, |route| route.ticket)
                    .is_ok_and(|index| index >= emission.cursor)
            })
        })
    }

    fn remove(&mut self, event: EventName<'_>, identity: usize) -> Option<PortRoute> {
        let routes = self.events.get_mut(event)?;
        let index = routes.iter().position(|route| route.identity == identity)?;
        let removed = Rc::make_mut(routes).remove(index);
        self.listeners -= 1;
        if routes.is_empty() {
            self.events.remove(event);
        }
        Some(removed)
    }

    fn compact_once(&mut self, event: &str) {
        let event = EventName::String(event);
        let Some(routes) = self.events.get_mut(event) else {
            return;
        };
        let owners = &self.owners;
        let previous = routes.len();
        Rc::make_mut(routes).retain(|route| {
            !route.once
                || owners
                    .get(route.owner)
                    .and_then(Weak::upgrade)
                    .is_some_and(|owner| owner.is_registered(route.ticket, route.slot))
        });
        self.listeners -= previous - routes.len();
        if routes.is_empty() {
            self.events.remove(event);
        }
    }

    fn remove_owner(&mut self, ticket: TaskTicket) {
        self.owners.remove(ticket);
        for routes in self.events.values_mut() {
            if routes.iter().any(|route| route.owner == ticket) {
                let previous = routes.len();
                Rc::make_mut(routes).retain(|route| route.owner != ticket);
                self.listeners -= previous - routes.len();
            }
        }
        self.events.remove_empty();
        if let Some(emission) = self.active.as_mut() {
            if let Some(routes) = emission.routes.as_mut() {
                if routes.iter().any(|route| route.owner == ticket) {
                    let removed_before_cursor = routes
                        .iter()
                        .take(emission.cursor)
                        .filter(|route| route.owner == ticket)
                        .count();
                    Rc::make_mut(routes).retain(|route| route.owner != ticket);
                    emission.cursor -= removed_before_cursor;
                }
            }
        }
    }
}

struct PortCallback<E: 'static> {
    ticket: TaskTicket,
    active: bool,
    callback: Option<RetainedEventCallback<E>>,
    next: Option<usize>,
}

struct PortCallbackSlots<E: 'static> {
    entries: Vec<PortCallback<E>>,
    free: Option<usize>,
    retired: Option<usize>,
}

impl<E: 'static> PortCallbackSlots<E> {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            free: None,
            retired: None,
        }
    }

    fn insert(&mut self, ticket: TaskTicket, callback: RetainedEventCallback<E>) -> usize {
        let entry = PortCallback {
            ticket,
            active: true,
            callback: Some(callback),
            next: None,
        };
        if let Some(slot) = self.free {
            self.free = self.entries[slot].next;
            self.entries[slot] = entry;
            slot
        } else {
            let slot = self.entries.len();
            self.entries.push(entry);
            slot
        }
    }

    fn retire(
        &mut self,
        ticket: TaskTicket,
        slot: usize,
        captured: bool,
    ) -> Option<RetainedEventCallback<E>> {
        let entry = self.entries.get_mut(slot)?;
        if entry.ticket != ticket || !entry.active {
            return None;
        }
        entry.active = false;
        if captured {
            entry.next = self.retired;
            self.retired = Some(slot);
            None
        } else {
            entry.next = self.free;
            self.free = Some(slot);
            entry.callback.take()
        }
    }

    fn pop_retired(&mut self) -> Option<RetainedEventCallback<E>> {
        let slot = self.retired?;
        let entry = &mut self.entries[slot];
        self.retired = entry.next;
        entry.next = self.free;
        self.free = Some(slot);
        entry.callback.take()
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.entries.iter().all(|entry| entry.callback.is_none())
    }
}

struct PortOwner<E: 'static> {
    state: Rc<RefCell<MessagePortState>>,
    reservation: TaskReservation,
    callbacks: RefCell<PortCallbackSlots<E>>,
}

impl<E: 'static> PortListenerLifecycle for PortOwner<E> {
    fn retire(&self, ticket: TaskTicket, slot: usize, captured: bool) {
        let removed = self.callbacks.borrow_mut().retire(ticket, slot, captured);
        drop(removed);
    }

    fn is_registered(&self, ticket: TaskTicket, slot: usize) -> bool {
        self.callbacks
            .borrow()
            .entries
            .get(slot)
            .is_some_and(|entry| entry.ticket == ticket && entry.active)
    }

    fn prune(&self) {
        loop {
            let removed = self.callbacks.borrow_mut().pop_retired();
            let Some(removed) = removed else {
                return;
            };
            drop(removed);
        }
    }
}

impl<E: 'static> Drop for PortOwner<E> {
    fn drop(&mut self) {
        self.state
            .borrow_mut()
            .routing
            .remove_owner(self.reservation.ticket());
    }
}

impl<E: 'static> MessagePort<E> {
    fn add_listener(
        &self,
        event: &JsValue,
        identity: usize,
        once: bool,
        callback: EventCallback<E>,
    ) -> NodeResult<&Self> {
        let event = EventName::from_value(event)?;
        {
            let mut state = self.owner.state.borrow_mut();
            if state.routing.listeners >= MAXIMUM_PORT_LISTENERS {
                return Err(NodeError::new(
                    "ERR_WORKER_LISTENER_LIMIT",
                    "message port exceeds the finite listener limit",
                ));
            }
            let ticket = super::resources::admit_signal()?;
            let slot = self
                .owner
                .callbacks
                .borrow_mut()
                .insert(ticket, RetainedEventCallback::new(callback, once));
            let route = PortRoute {
                owner: self.owner.reservation.ticket(),
                ticket,
                identity,
                once,
                slot,
            };
            if let Some(routes) = state.routing.events.get_mut(event) {
                Rc::make_mut(routes).push(route);
            } else {
                state.routing.events.insert(event, Rc::new(vec![route]));
            }
            state.routing.listeners += 1;
            state.started = true;
        }
        Ok(self)
    }

    fn remove_listener(&self, event: &JsValue, identity: usize) -> NodeResult<&Self> {
        let event = EventName::from_value(event)?;
        loop {
            let removed = {
                let mut state = self.owner.state.borrow_mut();
                state.routing.remove(event, identity).map(|route| {
                    let owner = state.routing.owner(route.owner);
                    let captured = state.routing.captured(route.ticket);
                    state.routing.cleanup_pending |= captured;
                    (owner, route.ticket, route.slot, captured)
                })
            };
            let Some((owner, ticket, slot, captured)) = removed else {
                return Ok(self);
            };
            if let Some(owner) = owner.and_then(|owner| owner.upgrade()) {
                owner.retire(ticket, slot, captured);
            }
        }
    }

    pub fn on_callable(
        &self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_listener(
            event,
            listener.identity_key(),
            false,
            EventCallback::Empty(listener.clone()),
        )
    }

    pub fn on_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_listener(
            event,
            listener.identity_key(),
            false,
            EventCallback::One(listener.clone()),
        )
    }

    pub fn once_callable(
        &self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_listener(
            event,
            listener.identity_key(),
            true,
            EventCallback::Empty(listener.clone()),
        )
    }

    pub fn once_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.add_listener(
            event,
            listener.identity_key(),
            true,
            EventCallback::One(listener.clone()),
        )
    }

    pub fn off_callable(
        &self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.remove_listener(event, listener.identity_key())
    }

    pub fn off_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.remove_listener(event, listener.identity_key())
    }

    fn finish_emission(&self) {
        let boundary = {
            let mut state = self.owner.state.borrow_mut();
            let event = state
                .routing
                .active
                .take()
                .map(|emission| emission.signal.event());
            if !std::mem::take(&mut state.routing.cleanup_pending) {
                return;
            }
            if let Some(event) = event {
                state.routing.compact_once(event);
            }
            let boundary = state
                .routing
                .owners
                .iter()
                .next_back()
                .map(|entry| entry.ticket);
            boundary
        };
        let Some(boundary) = boundary else {
            return;
        };
        let mut after = None;
        loop {
            let next = self
                .owner
                .state
                .borrow()
                .routing
                .owners
                .iter()
                .find(|entry| {
                    after.is_none_or(|after| entry.ticket > after) && entry.ticket <= boundary
                })
                .map(|entry| (entry.ticket, entry.owner.clone()));
            let Some((ticket, owner)) = next else {
                return;
            };
            after = Some(ticket);
            if let Some(owner) = owner.upgrade() {
                owner.prune();
            }
        }
    }
}
