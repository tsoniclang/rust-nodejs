use std::cell::{Cell, RefCell};
use std::num::NonZeroUsize;
use std::rc::{Rc, Weak};

use super::*;
use tsonic_rust_runtime::dispatch::{poll_phase, poll_prepared};
use tsonic_rust_runtime::dispatch_queue::{TaskBudget, TaskReservation};

struct Failure(Rc<Cell<i64>>);

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
}

#[test]
fn captured_native_frontiers_exclude_admission_and_reject_foreign_roots() {
    let resources = RuntimeResources::new(2);
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
        poll_phase(&resources, crate::phases::PORTS).ok(),
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
    let stale = stale.ok().unwrap();
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
    }
}
