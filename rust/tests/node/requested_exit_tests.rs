use std::cell::Cell;
use std::rc::Rc;

use tsonic_rust_js::promise::JsPromise;
use tsonic_rust_js::JsValue;
use tsonic_rust_runtime::dispatch::DispatchEnd;
use tsonic_rust_runtime::{Callable, JsError, TsonicError};

const STOPPED_ON_REQUESTED_EXIT: &str = "The event loop stopped on a requested exit.";

fn no_contexts() -> DispatchEnd<TsonicError> {
    DispatchEnd::<TsonicError>::new()
}

fn discard_rejection(message: &'static str) {
    JsPromise::<i32>::from_fallible_factory(move || async move {
        Err(JsValue::from_error(JsError::error(message)))
    })
    .observe_discarded();
}

fn two_due_js_timers(first_requests_exit: bool) -> (bool, bool) {
    let second = Rc::new(Cell::new(false));
    let observed = Rc::clone(&second);
    let ((), status) = tsonic_rust_node::run_entry_with_contexts(
        move || {
            tsonic_rust_js::timers::with_default(|timers| {
                tsonic_rust_js::timers::set_timeout_callable(
                    timers,
                    Callable::new(move |()| {
                        if first_requests_exit {
                            discard_rejection("first JS timer");
                        }
                        Ok(())
                    }),
                    0.0,
                )
            })
            .expect("first JS timer under the owner");
            tsonic_rust_js::timers::with_default(|timers| {
                tsonic_rust_js::timers::set_timeout_callable(
                    timers,
                    Callable::new(move |()| {
                        observed.set(true);
                        Ok(())
                    }),
                    0.0,
                )
            })
            .expect("second JS timer under the owner");
        },
        no_contexts(),
    )
    .unwrap();
    (status.exit_requested(), second.get())
}

fn two_due_node_timers(first_requests_exit: bool) -> (bool, bool) {
    let second = Rc::new(Cell::new(false));
    let observed = Rc::clone(&second);
    let ((), status) = tsonic_rust_node::run_entry_with_contexts(
        move || {
            tsonic_rust_node::timers::set_timeout(
                move || {
                    if first_requests_exit {
                        discard_rejection("first Node timer");
                    }
                },
                0,
            )
            .expect("first Node timer under the owner");
            tsonic_rust_node::timers::set_timeout(move || observed.set(true), 0)
                .expect("second Node timer under the owner");
        },
        no_contexts(),
    )
    .unwrap();
    (status.exit_requested(), second.get())
}

#[test]
fn a_requested_exit_stops_the_node_loop_before_a_second_due_js_timer() {
    let (exit_requested, second_ran) = two_due_js_timers(true);
    assert!(exit_requested, "the first timer's report requested exit");
    assert!(
        !second_ran,
        "the second due JS timer ran after the first requested exit"
    );
}

#[test]
fn without_a_requested_exit_the_node_loop_runs_both_due_js_timers() {
    let (exit_requested, second_ran) = two_due_js_timers(false);
    assert!(!exit_requested);
    assert!(second_ran);
}

#[test]
fn a_requested_exit_stops_the_node_loop_before_a_second_due_node_timer() {
    let (exit_requested, second_ran) = two_due_node_timers(true);
    assert!(exit_requested, "the first timer's report requested exit");
    assert!(
        !second_ran,
        "the second due Node timer ran after the first requested exit"
    );
}

#[test]
fn without_a_requested_exit_the_node_loop_runs_both_due_node_timers() {
    let (exit_requested, second_ran) = two_due_node_timers(false);
    assert!(!exit_requested);
    assert!(second_ran);
}

#[test]
fn node_block_on_reports_a_requested_exit_after_its_root_completed() {
    let result = tsonic_rust_node::block_on(async {
        discard_rejection("root");
        5
    });
    match result {
        Ok(value) => panic!("block_on returned {value} after a requested exit"),
        Err(error) => assert!(
            error.source_error().message() == STOPPED_ON_REQUESTED_EXIT,
            "{error}"
        ),
    }
}
