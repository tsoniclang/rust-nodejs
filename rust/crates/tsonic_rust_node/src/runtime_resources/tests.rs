use std::cell::{Cell, RefCell};
use std::num::NonZeroUsize;
use std::rc::{Rc, Weak};

use super::*;
use tsonic_rust_runtime::dispatch::{poll_phase, poll_prepared};
use tsonic_rust_runtime::dispatch_queue::{TaskBudget, TaskReservation};

struct Failure(Rc<Cell<i64>>);

struct State {
    reservation: TaskReservation,
    action: RefCell<Option<Result<bool, Failure>>>,
    polled: Cell<usize>,
}

#[derive(Clone)]
struct Resource(Weak<State>);

impl RuntimeResource for Resource {
    type Error = Failure;
    fn is_alive(&self) -> bool {
        self.0.strong_count() != 0
    }
    fn phase(&self) -> DispatchPhase {
        DispatchPhase::Ports
    }
    fn capture(&self) -> Result<Option<TaskTicket>, Failure> {
        Ok(self.0.upgrade().map(|state| state.reservation.ticket()))
    }
    fn poll(&self, _boundary: Option<TaskTicket>) -> Result<bool, Failure> {
        let Some(state) = self.0.upgrade() else {
            return Ok(false);
        };
        state.polled.set(state.polled.get() + 1);
        let result = state.action.borrow_mut().take().unwrap_or(Ok(false));
        result
    }
    fn has_work(&self) -> bool {
        self.0
            .upgrade()
            .is_some_and(|state| state.action.borrow().is_some())
    }
    fn next_delay(&self) -> Option<Duration> {
        None
    }
}

fn register(
    resources: &RuntimeResources<Resource>,
    budget: &TaskBudget,
    action: Result<bool, Failure>,
) -> Rc<State> {
    let state = Rc::new(State {
        reservation: budget.reserve().unwrap(),
        action: RefCell::new(Some(action)),
        polled: Cell::new(0),
    });
    resources.register(state.reservation.ticket(), Resource(Rc::downgrade(&state)));
    state
}

#[test]
fn native_resource_frontiers_skip_idle_candidates_without_losing_later_callbacks() {
    let resources = RuntimeResources::new(3);
    let budget = TaskBudget::new(NonZeroUsize::new(3).unwrap());
    let idle = register(&resources, &budget, Ok(false));
    let ready = register(&resources, &budget, Ok(true));
    assert_eq!(
        poll_phase(&resources, DispatchPhase::Ports).ok(),
        Some(true)
    );
    assert_eq!(idle.polled.get(), 1);
    assert_eq!(ready.polled.get(), 1);
    assert_eq!(
        poll_phase(&resources, DispatchPhase::Workers).ok(),
        Some(false)
    );
    assert_eq!(ready.polled.get(), 1);
}

#[test]
fn captured_native_frontiers_exclude_admission_and_reject_foreign_roots() {
    let resources = RuntimeResources::new(2);
    let other = RuntimeResources::<Resource>::new(2);
    let budget = TaskBudget::new(NonZeroUsize::new(2).unwrap());
    let first = register(&resources, &budget, Ok(true));
    let frontier = resources
        .prepare(DispatchPhase::Ports)
        .ok()
        .expect("native frontier");
    let second = register(&resources, &budget, Ok(true));
    assert!(other.next_ready(&frontier).is_none());
    assert_eq!(other.poll_next(&frontier).ok(), Some(false));
    assert_eq!(poll_prepared(&resources, &frontier).ok(), Some(true));
    assert_eq!(first.polled.get(), 1);
    assert_eq!(second.polled.get(), 0);
    assert_eq!(
        poll_phase(&resources, DispatchPhase::Ports).ok(),
        Some(true)
    );
    assert_eq!(second.polled.get(), 1);
}

#[test]
fn weak_native_resources_release_shared_quota_before_the_next_registry_poll() {
    let resources = RuntimeResources::new(1);
    let budget = TaskBudget::new(NonZeroUsize::new(1).unwrap());
    let first = register(&resources, &budget, Ok(true));
    assert_eq!(budget.pending(), 1);
    assert!(budget.reserve().is_err());
    drop(first);
    assert_eq!(budget.pending(), 0);
    let second = register(&resources, &budget, Ok(true));
    assert_eq!(resources.registrations.borrow().len(), 1);
    assert_eq!(
        poll_phase(&resources, DispatchPhase::Ports).ok(),
        Some(true)
    );
    assert_eq!(second.polled.get(), 1);
}

#[test]
fn original_native_failure_retains_the_unvisited_resource_frontier() {
    let resources = RuntimeResources::new(2);
    let budget = TaskBudget::new(NonZeroUsize::new(2).unwrap());
    let original = Rc::new(Cell::new(9_007_199_254_740_993));
    let first = register(&resources, &budget, Err(Failure(Rc::clone(&original))));
    let second = register(&resources, &budget, Ok(true));
    let frontier = resources
        .prepare(DispatchPhase::Ports)
        .ok()
        .expect("native frontier");
    let returned = poll_prepared(&resources, &frontier)
        .err()
        .expect("original native error");
    assert!(Rc::ptr_eq(&returned.0, &original));
    assert_eq!(first.polled.get(), 1);
    assert_eq!(second.polled.get(), 0);
    assert_eq!(poll_prepared(&resources, &frontier).ok(), Some(true));
    assert_eq!(second.polled.get(), 1);
}
