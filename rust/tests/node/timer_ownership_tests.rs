use std::cell::{Cell, OnceCell, RefCell};
use std::collections::BTreeMap;
use std::hint::black_box;
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::time::{Duration, Instant};

#[path = "../support/allocation_counts.rs"]
mod allocation_counts;
use allocation_counts::measure;

struct DropProbe(Rc<Cell<usize>>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

use tsonic_rust_node::{run_event_loop, timers};
use tsonic_rust_runtime::dispatch_queue::{TaskBudget, TaskReservation};
use tsonic_rust_runtime::TsonicResult;

struct NativeTimer {
    _callback: Rc<RefCell<dyn FnMut() -> TsonicResult<()>>>,
    _delay: Duration,
    _due: Instant,
    _interval: bool,
    _refed: bool,
    _reservation: TaskReservation,
}

#[test]
fn retained_mutable_timer_allocations_match_one_native_callback_owner() {
    let native = OnceCell::new();
    let budget = OnceCell::new();
    for count in [0, 1, 64, 1024] {
        let observed = Rc::new(Cell::new(0_usize));
        let mut handles = Vec::with_capacity(count);
        let delay = Duration::from_secs(60);
        let actual = measure(|| {
            for initial in 0..count {
                let observed = Rc::clone(&observed);
                let mut current = initial;
                handles.push(timers::set_interval(
                    move || {
                        current += 1;
                        observed.set(current);
                    },
                    60_000,
                ));
            }
        });
        let expected = measure(|| {
            for initial in 0..count {
                let reservation = budget
                    .get_or_init(|| TaskBudget::new(NonZeroUsize::new(1 << 20).unwrap()))
                    .reserve()
                    .unwrap();
                let observed = Rc::clone(&observed);
                let mut current = initial;
                let callback: Rc<RefCell<dyn FnMut() -> TsonicResult<()>>> =
                    Rc::new(RefCell::new(move || {
                        current += 1;
                        observed.set(current);
                        Ok(())
                    }));
                native
                    .get_or_init(|| Rc::new(RefCell::new(BTreeMap::new())))
                    .borrow_mut()
                    .insert(
                        initial as u64,
                        NativeTimer {
                            _callback: callback,
                            _delay: delay,
                            _due: Instant::now() + delay,
                            _interval: true,
                            _refed: true,
                            _reservation: reservation,
                        },
                    );
            }
        });
        black_box(&native);
        for mut handle in handles {
            timers::clear_interval(&mut handle);
            assert!(!handle.has_ref());
        }
        for index in 0..count {
            native.get().unwrap().borrow_mut().remove(&(index as u64));
        }
        assert_eq!(observed.get(), 0);
        assert_eq!(actual, expected, "registrations={count}");
    }
}

#[test]
fn aborted_callbacks_allocate_nothing_and_drop_once() {
    let drops = Rc::new(Cell::new(0));
    let probe = DropProbe(Rc::clone(&drops));
    let cost = measure(|| {
        let timer = timers::set_timeout_with_options(
            move || {
                black_box(&probe);
                panic!("aborted timer executed");
            },
            0,
            timers::TimerOptions {
                r#ref: true,
                signal_aborted: true,
            },
        );
        assert!(!timer.has_ref());
    });
    assert_eq!(cost, (0, 0));
    assert_eq!(drops.get(), 1);
    run_event_loop().unwrap();
}

#[test]
fn selected_timer_queries_and_empty_dispatch_allocate_nothing() {
    let timers = timers::new::<tsonic_rust_runtime::TsonicError>();
    let cost = measure(|| {
        assert!(!timers.has_pending_work());
        assert!(!timers.poll().unwrap());
        let contexts = tsonic_rust_runtime::dispatch::prepend(
            &timers,
            tsonic_rust_runtime::dispatch::DispatchEnd::<tsonic_rust_runtime::TsonicError>::new(),
        );
        tsonic_rust_node::run_with_contexts(&contexts).unwrap();
    });
    assert_eq!(cost, (0, 0));
}

#[test]
fn cancelling_and_running_once_callbacks_release_their_captures_exactly_once() {
    let drops = Rc::new(Cell::new(0));
    let calls = Rc::new(Cell::new(0));
    let cancelled_probe = DropProbe(Rc::clone(&drops));
    let mut cancelled = timers::set_timeout(
        move || {
            black_box(&cancelled_probe);
            panic!("cancelled timer executed");
        },
        0,
    );
    timers::clear_timeout(&mut cancelled);
    assert_eq!(drops.get(), 1);
    let called_probe = DropProbe(Rc::clone(&drops));
    let callback_calls = Rc::clone(&calls);
    timers::set_immediate(move || {
        black_box(&called_probe);
        callback_calls.set(callback_calls.get() + 1);
        let nested_calls = Rc::clone(&callback_calls);
        timers::set_immediate(move || nested_calls.set(nested_calls.get() + 1));
        run_event_loop().unwrap();
    });
    run_event_loop().unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(drops.get(), 2);
}

#[test]
fn mutable_interval_state_survives_repeated_calls_and_self_cancellation() {
    let observed = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    let slot = Rc::new(RefCell::new(None::<timers::Timeout>));
    let callback_slot = Rc::clone(&slot);
    let callback_observed = Rc::clone(&observed);
    let probe = DropProbe(Rc::clone(&drops));
    let mut current = 0;
    let timer = timers::set_interval(
        move || {
            black_box(&probe);
            current += 1;
            callback_observed.set(current);
            if current == 2 {
                callback_slot.borrow_mut().as_mut().unwrap().close();
            }
        },
        1,
    );
    *slot.borrow_mut() = Some(timer);
    run_event_loop().unwrap();
    assert_eq!(observed.get(), 2);
    assert_eq!(drops.get(), 1);
    assert!(!slot.borrow().as_ref().unwrap().has_ref());
}

#[test]
fn mutable_callbacks_reject_overlapping_reentrant_invocation() {
    let drops = Rc::new(Cell::new(0));
    let probe = DropProbe(Rc::clone(&drops));
    let mut timer = timers::set_interval(
        move || {
            black_box(&probe);
            std::thread::sleep(Duration::from_millis(2));
            run_event_loop().unwrap();
        },
        1,
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run_event_loop));
    timers::clear_interval(&mut timer);
    assert!(result.is_err());
    assert_eq!(drops.get(), 1);
}
