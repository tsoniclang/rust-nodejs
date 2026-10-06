use super::{close_watcher_state, FsWatcher, FsWatcherState, NodeError, NodeResult, WatchCallback};
use crate::runtime_resources::{
    NativeResourceBudget, ResourceFrontier, RuntimeResource, RuntimeResources,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Unbounded};
use std::rc::{Rc, Weak};
use std::time::Duration;
use tsonic_rust_runtime::dispatch::{DispatchContexts, DispatchPhase};
use tsonic_rust_runtime::dispatch_queue::{TaskReservation, TaskTicket};

const MAXIMUM_WATCHERS: usize = 1 << 20;
pub(super) const MAXIMUM_PENDING_EVENTS: usize = 1 << 16;

trait StatWatcherControl {
    fn matches_path(&self, path: &str) -> bool;
    fn close_registration(&self);
}

impl<E> StatWatcherControl for RefCell<FsWatcherState<E>> {
    fn matches_path(&self, path: &str) -> bool {
        self.borrow().path == path
    }
    fn close_registration(&self) {
        close_watcher_state(self);
    }
}

thread_local! {
    static RESOURCE_BUDGET: NativeResourceBudget = const { NativeResourceBudget::new(MAXIMUM_WATCHERS) };
    static DEFAULT_WATCHERS: Watchers<tsonic_rust_runtime::TsonicError> = const { Watchers::new() };
    static STAT_WATCHERS: RefCell<BTreeMap<TaskTicket, Weak<dyn StatWatcherControl>>> =
        const { RefCell::new(BTreeMap::new()) };
}

enum WatchOwnership<E> {
    Handle(Weak<RefCell<FsWatcherState<E>>>),
    Process(Rc<RefCell<FsWatcherState<E>>>),
}

impl<E> Clone for WatchOwnership<E> {
    fn clone(&self) -> Self {
        match self {
            Self::Handle(state) => Self::Handle(state.clone()),
            Self::Process(state) => Self::Process(Rc::clone(state)),
        }
    }
}

impl<E> WatchOwnership<E> {
    fn owner(&self) -> Option<Rc<RefCell<FsWatcherState<E>>>> {
        match self {
            Self::Handle(state) => state.upgrade(),
            Self::Process(state) => Some(Rc::clone(state)),
        }
    }
}

pub struct Watchers<E: 'static> {
    resources: RuntimeResources<WatchOwnership<E>>,
}

impl<E: 'static> Default for Watchers<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: 'static> Watchers<E> {
    pub const fn new() -> Self {
        Self {
            resources: RuntimeResources::new(MAXIMUM_WATCHERS),
        }
    }
}

impl<E: From<NodeError> + 'static> Watchers<E> {
    pub(super) fn register_handle(&self, watcher: &FsWatcher<E>) {
        self.resources.register(
            ticket(watcher),
            WatchOwnership::Handle(Rc::downgrade(&watcher.state)),
        );
    }

    pub(super) fn retain_stat(&self, watcher: &FsWatcher<E>) {
        let ticket = ticket(watcher);
        self.resources
            .register(ticket, WatchOwnership::Process(Rc::clone(&watcher.state)));
        let control: Rc<dyn StatWatcherControl> = watcher.state.clone();
        STAT_WATCHERS.with(|watchers| {
            let mut watchers = watchers.borrow_mut();
            watchers.retain(|_, control| control.strong_count() != 0);
            assert!(watchers.len() < MAXIMUM_WATCHERS || watchers.contains_key(&ticket));
            watchers.insert(ticket, Rc::downgrade(&control));
        });
    }
}

fn ticket<E: 'static>(watcher: &FsWatcher<E>) -> TaskTicket {
    watcher
        .state
        .borrow()
        .reservation
        .as_ref()
        .expect("open native watcher")
        .ticket()
}

impl<E: From<NodeError> + 'static> DispatchContexts for Watchers<E> {
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

impl<E: From<NodeError>> RuntimeResource for WatchOwnership<E> {
    type Error = E;

    fn is_alive(&self) -> bool {
        self.owner().is_some_and(|owner| !owner.borrow().closed)
    }
    fn phase(&self) -> DispatchPhase {
        DispatchPhase::Watchers
    }

    fn capture(&self) -> Result<Option<TaskTicket>, E> {
        let Some(owner) = self.owner() else {
            return Ok(None);
        };
        let captured = {
            let mut state = owner.borrow_mut();
            match &state.callback {
                Some(WatchCallback::Event(_)) => state
                    .ingest_events()
                    .map(|()| state.pending_events.back().map(|(ticket, _)| *ticket)),
                Some(WatchCallback::Stat(_)) => state
                    .capture_stat()
                    .map(|()| state.pending_stat.as_ref().map(|(ticket, _, _)| *ticket)),
                None => Ok(None),
            }
        };
        captured.map_err(E::from)
    }

    fn poll(&self, boundary: Option<TaskTicket>) -> Result<bool, E> {
        let Some(owner) = self.owner() else {
            return Ok(false);
        };
        let Some(boundary) = boundary else {
            return Ok(false);
        };
        let mut did_work = false;
        loop {
            let event = {
                let mut state = owner.borrow_mut();
                if state.closed {
                    break;
                }
                match &state.callback {
                    Some(WatchCallback::Event(callback)) => {
                        let callback = callback.clone();
                        if !state
                            .pending_events
                            .front()
                            .is_some_and(|(ticket, _)| *ticket <= boundary)
                        {
                            break;
                        }
                        state
                            .pending_events
                            .pop_front()
                            .map(|(_, event)| WatchEmission::Event(callback, event))
                    }
                    Some(WatchCallback::Stat(callback)) => {
                        let callback = callback.clone();
                        if !state
                            .pending_stat
                            .as_ref()
                            .is_some_and(|(ticket, _, _)| *ticket <= boundary)
                        {
                            break;
                        }
                        state.pending_stat.take().map(|(_, current, previous)| {
                            WatchEmission::Stat(callback, current, previous)
                        })
                    }
                    None => None,
                }
            };
            match event {
                Some(WatchEmission::Event(callback, event)) => {
                    callback.call((event.event_type, event.filename))?
                }
                Some(WatchEmission::Stat(callback, current, previous)) => {
                    callback.call((current, previous))?
                }
                None => break,
            }
            did_work = true;
        }
        Ok(did_work)
    }

    fn has_work(&self) -> bool {
        self.owner().is_some_and(|owner| {
            let state = owner.borrow();
            !state.closed
                && state.callback.is_some()
                && (state.refed || !state.pending_events.is_empty() || state.pending_stat.is_some())
        })
    }

    fn next_delay(&self) -> Option<Duration> {
        let owner = self.owner()?;
        let state = owner.borrow();
        if state.closed || state.callback.is_none() {
            return None;
        }
        state
            .stat_interval
            .map(|interval| interval.saturating_sub(state.last_stat_check.elapsed()))
    }
}

enum WatchEmission<E> {
    Event(
        tsonic_rust_runtime::Callable<(String, String), Result<(), E>>,
        super::FsWatchEvent,
    ),
    Stat(
        tsonic_rust_runtime::Callable<(super::Stats, super::Stats), Result<(), E>>,
        super::Stats,
        super::Stats,
    ),
}

pub(super) fn unwatch_file(path: &str) {
    let mut after = None;
    loop {
        let next = STAT_WATCHERS.with(|watchers| {
            watchers
                .borrow()
                .range((after.map_or(Unbounded, Excluded), Unbounded))
                .next()
                .map(|(ticket, control)| (*ticket, control.clone()))
        });
        let Some((ticket, control)) = next else {
            break;
        };
        after = Some(ticket);
        match control.upgrade() {
            Some(control) if control.matches_path(path) => {
                STAT_WATCHERS.with(|watchers| watchers.borrow_mut().remove(&ticket));
                control.close_registration();
            }
            None => {
                STAT_WATCHERS.with(|watchers| watchers.borrow_mut().remove(&ticket));
            }
            _ => {}
        }
    }
}

pub(super) fn reserve_resource() -> NodeResult<TaskReservation> {
    RESOURCE_BUDGET.with(NativeResourceBudget::reserve)
}
pub(super) fn admit_event() -> NodeResult<TaskTicket> {
    RESOURCE_BUDGET.with(NativeResourceBudget::admit)
}
pub(super) fn queue_limit() -> NodeError {
    NodeError::new(
        "ERR_FS_WATCHER_QUEUE_LIMIT",
        "native filesystem watcher queue is full",
    )
}
pub fn with_default_watchers<TOutput>(
    operation: impl FnOnce(&Watchers<tsonic_rust_runtime::TsonicError>) -> TOutput,
) -> TOutput {
    DEFAULT_WATCHERS.with(operation)
}

#[cfg(test)]
mod tests;
