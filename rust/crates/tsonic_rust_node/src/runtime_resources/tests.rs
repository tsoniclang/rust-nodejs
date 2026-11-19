use std::cell::{Cell, RefCell};
use std::num::NonZeroUsize;
use std::rc::{Rc, Weak};

use super::*;
use tsonic_rust_runtime::dispatch::{poll_phase, poll_prepared};
use tsonic_rust_runtime::dispatch_queue::{TaskBudget, TaskReservation};

struct Failure(Rc<Cell<i64>>);

enum LoopFailure {
    Source(Failure),
    Native(tsonic_rust_runtime::TsonicError),
}

impl From<Failure> for LoopFailure {
    fn from(error: Failure) -> Self {
        Self::Source(error)
    }
}

impl From<tsonic_rust_runtime::TsonicError> for LoopFailure {
    fn from(error: tsonic_rust_runtime::TsonicError) -> Self {
        Self::Native(error)
    }
}

fn with_loop<T>(resources: &RuntimeResources<Resource>, operation: impl FnOnce() -> T) -> T {
    let contexts = tsonic_rust_runtime::dispatch::prepend(
        resources,
        tsonic_rust_runtime::dispatch::DispatchEnd::<LoopFailure>::new(),
    );
    match crate::block_on_with_contexts(async { operation() }, contexts) {
        Ok((Some(value), _)) => value,
        Ok((None, _)) => panic!("the test loop stopped on a requested exit"),
        Err(LoopFailure::Source(error)) => {
            panic!("unhandled native test source failure: {}", error.0.get())
        }
        Err(LoopFailure::Native(error)) => panic!("native test loop failed: {error}"),
    }
}

fn physical_resource(budget: &TaskBudget) -> Rc<State> {
    Rc::new(State {
        reservation: budget.reserve().unwrap(),
        action: RefCell::new(None),
        polled: Cell::new(0),
    })
}

#[test]
fn retained_physical_registrations_retire_once_and_reject_stale_owners() {
    let resources = RuntimeResources::new(1);
    let budget = TaskBudget::new(NonZeroUsize::new(1).unwrap());
    let state = physical_resource(&budget);
    resources
        .register_physical(state.reservation.ticket(), Resource(Rc::downgrade(&state)))
        .unwrap();
    let first = tsonic_rust_runtime::dispatch::DispatchOwner::new().unwrap();
    assert!(resources.enter_owner(first));
    let retired = Cell::new(0);
    assert!(resources.close_owner_with(first, |resource| {
        assert!(resources.registrations.try_borrow_mut().is_ok());
        retired.set(retired.get() + 1);
        resource.is_alive()
    }));
    assert_eq!(retired.get(), 1);
    assert_eq!(resources.registrations.borrow().len(), 1);
    assert!(
        !resources.close_owner_with(first, |_| panic!("stale owner retired a live registration"))
    );
    let second = tsonic_rust_runtime::dispatch::DispatchOwner::new().unwrap();
    assert!(resources.enter_owner(second));
    assert!(!resources.enter_owner(first));
    assert!(!resources.close_owner_with(first, |_| panic!("old owner reached the new generation")));
    assert!(resources.owns(second));
    assert!(resources.close_owner_with(second, |resource| {
        retired.set(retired.get() + 1);
        resource.is_alive()
    }));
    assert_eq!(retired.get(), 2);
    assert_eq!(Rc::weak_count(&state), 1);
    assert_eq!(budget.pending(), 1);
    drop(state);
    assert_eq!(budget.pending(), 0);
    assert!(resources.prepare(crate::phases::PORTS).is_ok());
    assert!(resources.registrations.borrow().is_empty());
}

#[test]
fn retirement_preserves_new_native_admissions_without_revisiting_them() {
    let resources = RuntimeResources::new(2);
    let budget = TaskBudget::new(NonZeroUsize::new(2).unwrap());
    let original = physical_resource(&budget);
    resources
        .register_physical(
            original.reservation.ticket(),
            Resource(Rc::downgrade(&original)),
        )
        .unwrap();
    let owner = tsonic_rust_runtime::dispatch::DispatchOwner::new().unwrap();
    assert!(resources.enter_owner(owner));
    let mut added = None;
    let mut retired = 0;
    assert!(resources.close_owner_with(owner, |resource| {
        retired += 1;
        assert!(resources.registrations.try_borrow_mut().is_ok());
        assert!(!resources.owns(owner));
        let state = physical_resource(&budget);
        resources
            .register_physical(state.reservation.ticket(), Resource(Rc::downgrade(&state)))
            .unwrap();
        added = Some(state);
        resource.is_alive()
    }));
    assert_eq!(retired, 1);
    assert_eq!(resources.registrations.borrow().len(), 2);
    assert_eq!(Rc::weak_count(&original), 1);
    assert_eq!(Rc::weak_count(added.as_ref().unwrap()), 1);
    drop(original);
    drop(added);
    assert_eq!(budget.pending(), 0);
    assert!(resources.prepare(crate::phases::PORTS).is_ok());
    assert!(resources.registrations.borrow().is_empty());
}

#[test]
fn physical_retention_preserves_quota_and_prunes_dead_weak_entries_before_reuse() {
    let resources = RuntimeResources::new(1);
    let budget = TaskBudget::new(NonZeroUsize::new(1).unwrap());
    let state = physical_resource(&budget);
    resources
        .register_physical(state.reservation.ticket(), Resource(Rc::downgrade(&state)))
        .unwrap();
    for _ in 0..128 {
        let owner = tsonic_rust_runtime::dispatch::DispatchOwner::new().unwrap();
        assert!(resources.enter_owner(owner));
        assert!(resources.close_owner_with(owner, RuntimeResource::is_alive));
        assert_eq!(resources.registrations.borrow().len(), 1);
        assert_eq!(Rc::weak_count(&state), 1);
        assert_eq!(budget.pending(), 1);
        assert!(budget.reserve().is_err());
    }
    let retired_ticket = state.reservation.ticket();
    drop(state);
    assert_eq!(budget.pending(), 0);
    let replacement = physical_resource(&budget);
    resources
        .register_physical(
            replacement.reservation.ticket(),
            Resource(Rc::downgrade(&replacement)),
        )
        .unwrap();
    assert_eq!(resources.registrations.borrow().len(), 1);
    assert!(!resources
        .registrations
        .borrow()
        .contains_key(&retired_ticket));
    let owner = tsonic_rust_runtime::dispatch::DispatchOwner::new().unwrap();
    assert!(resources.enter_owner(owner));
    assert!(resources.close_owner(owner));
    assert!(resources.registrations.borrow().is_empty());
    assert_eq!(budget.pending(), 1);
    drop(replacement);
    assert_eq!(budget.pending(), 0);
}

#[test]
fn native_registration_visits_are_bounded_and_do_not_borrow_across_callbacks() {
    let resources = RuntimeResources::new(2);
    let budget = TaskBudget::new(NonZeroUsize::new(2).unwrap());
    let original = physical_resource(&budget);
    resources
        .register_physical(
            original.reservation.ticket(),
            Resource(Rc::downgrade(&original)),
        )
        .unwrap();
    let mut calls = 0;
    let mut added = None;
    resources.visit(|resource| {
        assert!(resources.registrations.try_borrow_mut().is_ok());
        assert!(resource.0.upgrade().is_some());
        calls += 1;
        let state = physical_resource(&budget);
        resources
            .register_physical(state.reservation.ticket(), Resource(Rc::downgrade(&state)))
            .unwrap();
        added = Some(state);
    });
    assert_eq!(calls, 1);
    assert_eq!(resources.registrations.borrow().len(), 2);
    assert_eq!(budget.pending(), 2);
    drop(original);
    drop(added);
    assert_eq!(budget.pending(), 0);
    assert!(resources.prepare(crate::phases::PORTS).is_ok());
    assert!(resources.registrations.borrow().is_empty());
}

#[test]
fn native_retirement_keeps_the_existing_registry_limit_during_reentrant_admission() {
    let resources = RuntimeResources::new(1);
    let budget = TaskBudget::new(NonZeroUsize::new(2).unwrap());
    let original = physical_resource(&budget);
    resources
        .register_physical(
            original.reservation.ticket(),
            Resource(Rc::downgrade(&original)),
        )
        .unwrap();
    let owner = tsonic_rust_runtime::dispatch::DispatchOwner::new().unwrap();
    assert!(resources.enter_owner(owner));
    assert!(resources.close_owner_with(owner, |resource| {
        let rejected = physical_resource(&budget);
        assert_eq!(
            resources
                .register_physical(
                    rejected.reservation.ticket(),
                    Resource(Rc::downgrade(&rejected))
                )
                .unwrap_err()
                .code(),
            "ERR_NODE_RESOURCE_LIMIT"
        );
        assert_eq!(
            resources
                .register_physical(
                    original.reservation.ticket(),
                    Resource(Rc::downgrade(&rejected))
                )
                .unwrap_err()
                .code(),
            "ERR_NODE_RESOURCE_CLOSED"
        );
        assert_eq!(resources.registrations.borrow().len(), 1);
        resource.is_alive()
    }));
    assert_eq!(Rc::weak_count(&original), 1);
    assert_eq!(budget.pending(), 1);
    drop(original);
    assert_eq!(budget.pending(), 0);
    assert!(resources.prepare(crate::phases::PORTS).is_ok());
    assert!(resources.registrations.borrow().is_empty());
}

#[test]
fn native_partial_retirement_drops_captures_once_outside_registry_borrows() {
    let resources = Rc::new(RuntimeResources::new(1));
    let budget = TaskBudget::new(NonZeroUsize::new(1).unwrap());
    let reservation = budget.reserve().unwrap();
    let calls = Rc::new(Cell::new(0));
    resources
        .register_physical(
            reservation.ticket(),
            ReentrantResource {
                alive: Rc::new(Cell::new(true)),
                _owner: Rc::new(RegistryDrop {
                    resources: Rc::downgrade(&resources),
                    calls: Rc::clone(&calls),
                }),
            },
        )
        .unwrap();
    let owner = tsonic_rust_runtime::dispatch::DispatchOwner::new().unwrap();
    assert!(resources.enter_owner(owner));
    assert!(resources.close_owner_with(owner, |_| false));
    assert_eq!(calls.get(), 1);
    assert!(resources.registrations.borrow().is_empty());
    assert!(!resources.close_owner_with(owner, |_| panic!("resource retired twice")));
    assert_eq!(calls.get(), 1);
}

#[test]
fn loop_owner_plain_native_resource_reservations_do_not_require_a_js_loop() {
    let budget = NativeResourceBudget::new(2);
    assert!(tsonic_rust_js::event_loop::ensure_running().is_err());
    let reservation = budget.reserve().unwrap();
    let next = budget.admit().unwrap();
    assert_eq!(next.sequence(), reservation.ticket().sequence() + 1);
    assert_eq!(budget.budget().pending(), 1);
    drop(reservation);
    assert_eq!(budget.budget().pending(), 0);
    assert!(tsonic_rust_js::event_loop::ensure_running().is_err());
}

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
        crate::phases::PORTS
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
    resources
        .register(state.reservation.ticket(), Resource(Rc::downgrade(&state)))
        .unwrap();
    state
}

#[test]
fn native_resource_frontiers_skip_idle_candidates_without_losing_later_callbacks() {
    let resources = RuntimeResources::new(3);
    with_loop(&resources, || {
        let budget = TaskBudget::new(NonZeroUsize::new(3).unwrap());
        let idle = register(&resources, &budget, Ok(false));
        let ready = register(&resources, &budget, Ok(true));
        assert_eq!(
            poll_phase(&resources, crate::phases::PORTS).ok(),
            Some(true)
        );
        assert_eq!(idle.polled.get(), 1);
        assert_eq!(ready.polled.get(), 1);
        assert_eq!(
            poll_phase(&resources, crate::phases::WORKERS).ok(),
            Some(false)
        );
        assert_eq!(ready.polled.get(), 1);
    });
}

#[test]
fn captured_native_frontiers_exclude_admission_and_reject_foreign_roots() {
    let resources = RuntimeResources::new(2);
    with_loop(&resources, || {
        let other = RuntimeResources::<Resource>::new(2);
        let budget = TaskBudget::new(NonZeroUsize::new(2).unwrap());
        let first = register(&resources, &budget, Ok(true));
        let frontier = resources
            .prepare(crate::phases::PORTS)
            .ok()
            .expect("native frontier");
        let second = register(&resources, &budget, Ok(true));
        assert!(other.next_ready(&frontier).is_none());
        assert_eq!(other.poll_next(&frontier).ok(), Some(false));
        assert_eq!(poll_prepared(&resources, &frontier).ok(), Some(true));
        assert_eq!(first.polled.get(), 1);
        assert_eq!(second.polled.get(), 0);
        assert_eq!(
            poll_phase(&resources, crate::phases::PORTS).ok(),
            Some(true)
        );
        assert_eq!(second.polled.get(), 1);
    });
}

#[test]
fn weak_native_resources_release_shared_quota_before_the_next_registry_poll() {
    let resources = RuntimeResources::new(1);
    with_loop(&resources, || {
        let budget = TaskBudget::new(NonZeroUsize::new(1).unwrap());
        let first = register(&resources, &budget, Ok(true));
        assert_eq!(budget.pending(), 1);
        assert!(budget.reserve().is_err());
        drop(first);
        assert_eq!(budget.pending(), 0);
        let second = register(&resources, &budget, Ok(true));
        assert_eq!(resources.registrations.borrow().len(), 1);
        assert_eq!(
            poll_phase(&resources, crate::phases::PORTS).ok(),
            Some(true)
        );
        assert_eq!(second.polled.get(), 1);
    });
}

#[test]
fn original_native_failure_retains_the_unvisited_resource_frontier() {
    let resources = RuntimeResources::new(2);
    with_loop(&resources, || {
        let budget = TaskBudget::new(NonZeroUsize::new(2).unwrap());
        let original = Rc::new(Cell::new(9_007_199_254_740_993));
        let first = register(&resources, &budget, Err(Failure(Rc::clone(&original))));
        let second = register(&resources, &budget, Ok(true));
        let frontier = resources
            .prepare(crate::phases::PORTS)
            .ok()
            .expect("native frontier");
        let returned = match poll_prepared(&resources, &frontier) {
            Err(error) => error,
            Ok(_) => panic!("original native error"),
        };
        assert!(Rc::ptr_eq(&returned.0, &original));
        assert_eq!(first.polled.get(), 1);
        assert_eq!(second.polled.get(), 0);
        assert_eq!(poll_prepared(&resources, &frontier).ok(), Some(true));
        assert_eq!(second.polled.get(), 1);
    });
}

#[derive(Clone)]
struct ReentrantResource {
    alive: Rc<Cell<bool>>,
    _owner: Rc<RegistryDrop>,
}

struct RegistryDrop {
    resources: Weak<RuntimeResources<ReentrantResource>>,
    calls: Rc<Cell<usize>>,
}

impl Drop for RegistryDrop {
    fn drop(&mut self) {
        let resources = self.resources.upgrade().expect("registry remains owned");
        assert!(
            resources.registrations.try_borrow_mut().is_ok(),
            "resource capture dropped under registry borrow"
        );
        assert!(!resources.has_work());
        self.calls.set(self.calls.get() + 1);
    }
}

impl RuntimeResource for ReentrantResource {
    type Error = crate::NodeError;
    fn is_alive(&self) -> bool {
        self.alive.get()
    }
    fn phase(&self) -> DispatchPhase {
        crate::phases::HTTP
    }
    fn capture(&self) -> crate::NodeResult<Option<TaskTicket>> {
        Ok(None)
    }
    fn poll(&self, _boundary: Option<TaskTicket>) -> crate::NodeResult<bool> {
        Ok(false)
    }
    fn has_work(&self) -> bool {
        false
    }
    fn next_delay(&self) -> Option<Duration> {
        None
    }
}

#[test]
fn loop_owner_shutdown_drops_native_registrations_once_outside_registry_borrows() {
    use tsonic_rust_runtime::dispatch::{prepend, DispatchEnd};
    let resources = Rc::new(RuntimeResources::new(1));
    let budget = TaskBudget::new(NonZeroUsize::new(1).unwrap());
    let calls = Rc::new(Cell::new(0));
    let mut stale = None;
    for attempt in 0..128 {
        let reservation = budget.reserve().unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::block_on_with_contexts(
                async {
                    let current = tsonic_rust_js::event_loop::dispatch_owner().unwrap();
                    if let Some(retired) = stale {
                        assert!(!resources.enter(retired));
                        resources.close(retired);
                    } else {
                        stale = Some(current);
                    }
                    resources
                        .register(
                            reservation.ticket(),
                            ReentrantResource {
                                alive: Rc::new(Cell::new(true)),
                                _owner: Rc::new(RegistryDrop {
                                    resources: Rc::downgrade(&resources),
                                    calls: Rc::clone(&calls),
                                }),
                            },
                        )
                        .unwrap();
                    assert!(resources.generation.owns(current));
                    assert_eq!(resources.registrations.borrow().len(), 1);
                    assert_eq!(attempt % 2, 0, "original native root panic");
                },
                prepend(
                    resources.as_ref(),
                    DispatchEnd::<tsonic_rust_runtime::TsonicError>::new(),
                ),
            )
        }));
        if attempt % 2 == 0 {
            assert!(result.unwrap().is_ok());
        } else {
            assert!(result.is_err());
        }
        assert!(resources.registrations.borrow().is_empty());
        assert_eq!(calls.get(), attempt + 1);
    }
    assert_eq!(Rc::strong_count(&calls), 1);
    assert_eq!(budget.pending(), 0);
}

#[test]
fn loop_owner_stale_native_frontiers_never_select_a_reopened_source() {
    use tsonic_rust_runtime::dispatch::{prepend, DispatchEnd};
    let resources = Rc::new(RuntimeResources::<ReentrantResource>::new(1));
    let calls = Rc::new(Cell::new(0));
    let make_resource = || ReentrantResource {
        alive: Rc::new(Cell::new(true)),
        _owner: Rc::new(RegistryDrop {
            resources: Rc::downgrade(&resources),
            calls: Rc::clone(&calls),
        }),
    };
    let stale = crate::block_on_with_contexts(
        async {
            let budget = TaskBudget::new(NonZeroUsize::new(1).unwrap());
            let reservation = budget.reserve().unwrap();
            resources
                .register(reservation.ticket(), make_resource())
                .unwrap();
            resources.prepare(crate::phases::HTTP).ok().unwrap()
        },
        prepend(
            resources.as_ref(),
            DispatchEnd::<tsonic_rust_runtime::TsonicError>::new(),
        ),
    );
    let stale = stale.ok().unwrap().0.unwrap();
    for _ in 0..128 {
        crate::block_on_with_contexts(
            async {
                let budget = TaskBudget::new(NonZeroUsize::new(1).unwrap());
                let reservation = budget.reserve().unwrap();
                resources
                    .register(reservation.ticket(), make_resource())
                    .unwrap();
                let current = resources.prepare(crate::phases::HTTP).ok().unwrap();
                assert!(resources.next_ready(&current).is_some());
                assert!(resources.next_ready(&stale).is_none());
                assert_eq!(resources.poll_next(&stale).ok(), Some(false));
            },
            prepend(
                resources.as_ref(),
                DispatchEnd::<tsonic_rust_runtime::TsonicError>::new(),
            ),
        )
        .ok()
        .unwrap();
        assert!(resources.registrations.borrow().is_empty());
    }
}

#[test]
fn pruning_and_replacement_release_native_resource_owners_outside_registry_borrows() {
    for replacement in [false, true] {
        let resources = Rc::new(RuntimeResources::new(1));
        crate::block_on_with_contexts(
            async {
                let budget = TaskBudget::new(NonZeroUsize::new(1).unwrap());
                let reservation = budget.reserve().unwrap();
                let calls = Rc::new(Cell::new(0));
                let alive = Rc::new(Cell::new(true));
                resources
                    .register(
                        reservation.ticket(),
                        ReentrantResource {
                            alive: Rc::clone(&alive),
                            _owner: Rc::new(RegistryDrop {
                                resources: Rc::downgrade(&resources),
                                calls: Rc::clone(&calls),
                            }),
                        },
                    )
                    .unwrap();
                if replacement {
                    resources
                        .register(
                            reservation.ticket(),
                            ReentrantResource {
                                alive: Rc::new(Cell::new(true)),
                                _owner: Rc::new(RegistryDrop {
                                    resources: Rc::downgrade(&resources),
                                    calls: Rc::clone(&calls),
                                }),
                            },
                        )
                        .unwrap();
                    assert_eq!(calls.get(), 1);
                    resources
                        .registrations
                        .borrow()
                        .values()
                        .next()
                        .unwrap()
                        .resource
                        .alive
                        .set(false);
                } else {
                    alive.set(false);
                }
                assert!(resources.prepare(crate::phases::HTTP).is_ok());
                assert_eq!(calls.get(), if replacement { 2 } else { 1 });
                assert!(resources.registrations.borrow().is_empty());
            },
            tsonic_rust_runtime::dispatch::prepend(
                resources.as_ref(),
                tsonic_rust_runtime::dispatch::DispatchEnd::<tsonic_rust_runtime::TsonicError>::new(
                ),
            ),
        )
        .unwrap();
    }
}
