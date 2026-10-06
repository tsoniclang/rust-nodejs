use super::*;
use std::cell::Cell;
use tsonic_rust_runtime::{ErrorObject, TsonicError};

#[test]
fn first_timer_failure_retains_original_identity_and_uninvoked_callbacks() {
    let original = tsonic_rust_runtime::JsError::error("original timer failure");
    let failure = original.clone();
    schedule(
        move || Err(failure.clone().into()),
        0,
        false,
        TimerOptions::default(),
    );
    let observed = Rc::new(Cell::new(0));
    let recorded = observed.clone();
    set_timeout(move || recorded.set(1), 0);
    let returned = poll_runtime_timers().unwrap_err();
    assert_eq!(
        returned.source_error().error_identity_key(),
        original.error_identity_key()
    );
    assert_eq!(observed.get(), 0);
    assert!(has_refed_runtime_timers());
    assert!(poll_runtime_timers().unwrap());
    assert_eq!(observed.get(), 1);
    assert!(!has_refed_runtime_timers());
}

#[test]
fn cancellation_and_reentrant_admission_follow_the_live_timer_store() {
    let observed = Rc::new(Cell::new(0));
    let cancelled = Rc::new(RefCell::new(None::<Timeout>));
    let handle = cancelled.clone();
    let recorded = observed.clone();
    set_timeout(
        move || {
            clear_timeout(handle.borrow_mut().as_mut().unwrap());
            let later = recorded.clone();
            set_timeout(move || later.set(7), 0);
        },
        0,
    );
    *cancelled.borrow_mut() = Some(set_timeout(|| panic!("cancelled ready timer"), 0));
    assert!(poll_runtime_timers().unwrap());
    assert_eq!(observed.get(), 0);
    assert!(has_refed_runtime_timers());
    assert!(poll_runtime_timers().unwrap());
    assert_eq!(observed.get(), 7);
    assert!(!has_refed_runtime_timers());
}

#[test]
fn selected_timer_driver_retains_non_display_non_send_nominal_failures() {
    enum Failure {
        Native(TsonicError),
        Source(Rc<Cell<i64>>),
    }
    impl From<TsonicError> for Failure {
        fn from(value: TsonicError) -> Self {
            Self::Native(value)
        }
    }
    let timers = new::<Failure>();
    let original = Rc::new(Cell::new(9_007_199_254_740_993));
    let captured = Rc::clone(&original);
    set_timeout_callable(
        &timers,
        Callable::new(move |()| Err(Failure::Source(Rc::clone(&captured)))),
        0,
    )
    .unwrap();
    let observed = Rc::new(Cell::new(false));
    let recorded = Rc::clone(&observed);
    set_timeout_callable(
        &timers,
        Callable::new(move |()| {
            recorded.set(true);
            Ok(())
        }),
        0,
    )
    .unwrap();
    let contexts = tsonic_rust_runtime::dispatch::prepend(
        &timers,
        tsonic_rust_runtime::dispatch::DispatchEnd::<Failure>::new(),
    );
    match crate::run_with_contexts(&contexts)
        .err()
        .expect("original typed timer failure")
    {
        Failure::Source(value) => {
            assert!(Rc::ptr_eq(&value, &original));
            assert_eq!(value.get(), 9_007_199_254_740_993);
        }
        Failure::Native(error) => panic!("unexpected native fault: {error}"),
    }
    assert!(!observed.get());
    assert!(timers.has_pending_work());
    assert!(crate::run_with_contexts(&contexts).is_ok());
    assert!(observed.get());
    assert!(!timers.has_pending_work());
}

#[test]
fn timer_handle_does_not_retain_its_root_or_callback() {
    let timers = new::<()>();
    let capture = Rc::new(Cell::new(0));
    let retained = Rc::clone(&capture);
    let mut handle = set_timeout_callable(
        &timers,
        Callable::new(move |()| {
            retained.set(1);
            Ok(())
        }),
        0,
    )
    .unwrap();
    assert_eq!(Rc::strong_count(&capture), 2);
    drop(timers);
    assert_eq!(Rc::strong_count(&capture), 1);
    assert!(!handle.has_ref());
    handle.close().refresh().r#ref();
    assert!(!handle.has_ref());
}
