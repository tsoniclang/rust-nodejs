use super::*;
use std::cell::Cell;
use tsonic_rust_runtime::{ErrorObject, TsonicError};

#[test]
fn loop_owner_all_convenience_timer_admissions_fail_exactly_without_a_loop() {
    let options = TimerOptions::default();
    let operations: [fn(TimerOptions) -> NodeResult<Timeout>; 6] = [
        |_| set_timeout(|| panic!("unadmitted timeout"), 0),
        |options| set_timeout_with_options(|| panic!("unadmitted timeout"), 0, options),
        |_| set_immediate(|| panic!("unadmitted immediate")),
        |options| set_immediate_with_options(|| panic!("unadmitted immediate"), options),
        |_| set_interval(|| panic!("unadmitted interval"), 0),
        |options| set_interval_with_options(|| panic!("unadmitted interval"), 0, options),
    ];
    let assert_no_owner = |failure: Box<dyn std::any::Any + Send>| {
        let message = failure
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| failure.downcast_ref::<&str>().copied());
        assert_eq!(
            message,
            Some("No JavaScript event loop is running on this thread.")
        );
    };
    for operation in operations {
        let Err(failure) =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(options)))
        else {
            panic!("missing loop must panic at timer admission");
        };
        assert_no_owner(failure);
    }
    let capture = Rc::new(Cell::new(0));
    let observed = Rc::clone(&capture);
    let Err(failure) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        set_timeout(move || observed.set(1), 0)
    })) else {
        panic!("missing loop must panic before retaining the callback");
    };
    assert_no_owner(failure);
    assert_eq!(capture.get(), 0);
    assert_eq!(Rc::strong_count(&capture), 1);
    assert!(!has_refed_runtime_timers());
}

#[test]
fn loop_owner_shutdown_releases_unrefed_timers_and_stale_handles_cannot_touch_replacements() {
    let capture = Rc::new(Cell::new(0));
    let mut first = None;
    let options = TimerOptions {
        r#ref: false,
        signal_aborted: false,
    };
    crate::block_on(async {
        let observed = Rc::clone(&capture);
        first = Some(set_timeout_with_options(move || observed.set(1), 60_000, options).unwrap());
    })
    .unwrap();
    assert_eq!(Rc::strong_count(&capture), 1);
    let mut first = first.unwrap();
    assert!(!first.has_ref());
    for _ in 0..128 {
        crate::block_on(async {
            let observed = Rc::clone(&capture);
            let mut current =
                set_timeout_with_options(move || observed.set(2), 60_000, options).unwrap();
            first.close().refresh().r#ref();
            current.r#ref();
            assert!(current.has_ref());
            current.unref();
            assert!(!first.has_ref());
            assert_eq!(Rc::strong_count(&capture), 2);
        })
        .unwrap();
        assert_eq!(Rc::strong_count(&capture), 1);
    }
    assert_eq!(capture.get(), 0);
}

#[test]
fn loop_owner_native_timers_cannot_attach_to_a_js_only_loop() {
    tsonic_rust_js::event_loop::block_on(async {
        let error = set_timeout(|| panic!("unregistered native timer"), 0)
            .err()
            .unwrap();
        assert_eq!(error.code(), "ERR_NODE_TIMER_CLOSED");
    })
    .unwrap();
    assert!(!has_refed_runtime_timers());
}

#[test]
fn local_timer_context_and_handle_retain_borrowed_callback_environment() {
    let calls = Cell::new(0);
    let timers = new::<std::convert::Infallible>();
    let handle = set_timeout_callable(
        &timers,
        Callable::new(|()| {
            calls.set(calls.get() + 1);
            Ok(())
        }),
        0,
    )
    .unwrap();
    assert_eq!(calls.get(), 0);
    assert!(handle.has_ref());
    assert!(timers.poll().unwrap());
    assert_eq!(calls.get(), 1);
    assert!(!timers.has_pending_work());
    drop(handle);
}

#[test]
fn local_timer_context_preserves_borrowed_failure_identity() {
    let failure = String::from("scoped timer failure");
    let timers = new::<&String>();
    let handle = set_timeout_callable(&timers, Callable::new(|()| Err(&failure)), 0).unwrap();
    assert!(std::ptr::eq(timers.poll().unwrap_err(), &failure));
    assert!(!timers.has_pending_work());
    drop(handle);
}

#[test]
fn first_timer_failure_retains_original_identity_and_uninvoked_callbacks() {
    let original = tsonic_rust_runtime::JsError::error("original timer failure");
    let failure = original.clone();
    schedule(
        move || Err(failure.clone().into()),
        0,
        false,
        TimerOptions::default(),
    )
    .unwrap();
    let observed = Rc::new(Cell::new(0));
    let recorded = observed.clone();
    set_timeout(move || recorded.set(1), 0).unwrap();
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
            set_timeout(move || later.set(7), 0).unwrap();
        },
        0,
    )
    .unwrap();
    *cancelled.borrow_mut() = Some(set_timeout(|| panic!("cancelled ready timer"), 0).unwrap());
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
    match crate::run_with_contexts(&contexts) {
        Err(Failure::Source(value)) => {
            assert!(Rc::ptr_eq(&value, &original));
            assert_eq!(value.get(), 9_007_199_254_740_993);
        }
        Err(Failure::Native(error)) => panic!("unexpected native fault: {error}"),
        Ok(_) => panic!("original typed timer failure"),
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
