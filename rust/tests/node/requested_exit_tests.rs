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

fn discard_after_a_settled_reaction() {
    drop(JsPromise::resolved(0).then(
        Callable::new(|(value,): (i32,)| {
            discard_rejection("chained");
            Ok(value)
        }),
        None,
    ));
}

fn two_due_js_timers(first: fn()) -> (bool, bool) {
    let second = Rc::new(Cell::new(false));
    let observed = Rc::clone(&second);
    let ((), status) = tsonic_rust_node::run_entry_with_contexts(
        move || {
            tsonic_rust_js::timers::with_default(|timers| {
                tsonic_rust_js::timers::set_timeout_callable(
                    timers,
                    Callable::new(move |()| {
                        first();
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

fn two_due_node_timers(first: fn()) -> (bool, bool) {
    let second = Rc::new(Cell::new(false));
    let observed = Rc::clone(&second);
    let ((), status) = tsonic_rust_node::run_entry_with_contexts(
        move || {
            tsonic_rust_node::timers::set_timeout(first, 0)
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
    let (exit_requested, second_ran) = two_due_js_timers(|| discard_rejection("first JS timer"));
    assert!(exit_requested, "the first timer's report requested exit");
    assert!(
        !second_ran,
        "the second due JS timer ran after the first requested exit"
    );
}

#[test]
fn a_report_queued_by_a_reaction_stops_the_node_loop_before_a_second_due_js_timer() {
    let (exit_requested, second_ran) = two_due_js_timers(discard_after_a_settled_reaction);
    assert!(exit_requested, "the chained report requested exit");
    assert!(
        !second_ran,
        "the second due JS timer ran before the chained report"
    );
}

#[test]
fn without_a_requested_exit_the_node_loop_runs_both_due_js_timers() {
    let (exit_requested, second_ran) = two_due_js_timers(|| {});
    assert!(!exit_requested);
    assert!(second_ran);
}

#[test]
fn a_requested_exit_stops_the_node_loop_before_a_second_due_node_timer() {
    let (exit_requested, second_ran) =
        two_due_node_timers(|| discard_rejection("first Node timer"));
    assert!(exit_requested, "the first timer's report requested exit");
    assert!(
        !second_ran,
        "the second due Node timer ran after the first requested exit"
    );
}

#[test]
fn a_report_queued_by_a_reaction_stops_the_node_loop_before_a_second_due_node_timer() {
    let (exit_requested, second_ran) = two_due_node_timers(discard_after_a_settled_reaction);
    assert!(exit_requested, "the chained report requested exit");
    assert!(
        !second_ran,
        "the second due Node timer ran before the chained report"
    );
}

#[test]
fn without_a_requested_exit_the_node_loop_runs_both_due_node_timers() {
    let (exit_requested, second_ran) = two_due_node_timers(|| {});
    assert!(!exit_requested);
    assert!(second_ran);
}

#[test]
fn a_report_queued_by_a_reaction_in_the_entry_stops_the_node_loop_before_a_zero_delay_timer() {
    let timer_ran = Rc::new(Cell::new(false));
    let observed = Rc::clone(&timer_ran);
    let ((), status) = tsonic_rust_node::run_entry_with_contexts(
        move || {
            tsonic_rust_node::timers::set_timeout(move || observed.set(true), 0)
                .expect("Node timer under the owner");
            discard_after_a_settled_reaction();
        },
        no_contexts(),
    )
    .unwrap();
    assert!(status.exit_requested(), "the chained report requested exit");
    assert!(
        !timer_ran.get(),
        "the zero-delay Node timer ran before the chained report"
    );
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
