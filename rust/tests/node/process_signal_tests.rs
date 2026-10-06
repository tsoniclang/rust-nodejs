#![cfg(unix)]

use std::cell::Cell;
use std::os::unix::process::ExitStatusExt;
use std::rc::Rc;
use tsonic_rust_node::process;
use tsonic_rust_runtime::Callable;
use tsonic_rust_runtime::TsonicError;

#[test]
fn process_signal_delivery_is_isolated_and_restores_native_defaults() {
    for scenario in [
        "delivery",
        "remove",
        "once-default",
        "no-keepalive",
        "reentrant",
        "typed-domains",
        "typed-drop",
    ] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "process_signal_tests::process_signal_child",
                "--nocapture",
            ])
            .env("TSONIC_SIGNAL_PROOF", scenario)
            .output()
            .unwrap();
        if matches!(scenario, "remove" | "once-default" | "typed-drop") {
            assert_eq!(
                output.status.signal(),
                Some(nix::libc::SIGUSR1),
                "{scenario}: {output:?}"
            );
        } else {
            assert!(output.status.success(), "{scenario}: {output:?}");
        }
    }
}

#[test]
fn process_signal_child() {
    let Ok(scenario) = std::env::var("TSONIC_SIGNAL_PROOF") else {
        return;
    };
    if scenario == "typed-domains" || scenario == "typed-drop" {
        verify_typed_signal_owners(&scenario);
        return;
    }
    let count = Rc::new(Cell::new(0));
    let listener = Callable::new({
        let count = Rc::clone(&count);
        move |()| {
            count.set(count.get() + 1);
            Ok::<(), TsonicError>(())
        }
    });
    let pid = process::pid() as f64;
    process::with_default_signals(|tasks| process::once(tasks, "SIGUSR1", &listener)).unwrap();
    if scenario == "no-keepalive" {
        process::kill_named(pid, "SIGUSR1").unwrap();
        let started = std::time::Instant::now();
        tsonic_rust_node::run_event_loop().unwrap();
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        assert_eq!(count.get(), 0);
        return;
    }
    if scenario == "remove" {
        process::with_default_signals(|tasks| {
            process::remove_listener(tasks, "SIGUSR1", &listener.clone())
        })
        .unwrap();
        process::kill_named(pid, "SIGUSR1").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        panic!("removed listener must restore the signal's native default");
    }
    if scenario == "reentrant" {
        let next = listener.clone();
        let nested = Callable::new(move |()| {
            process::with_default_signals(|tasks| process::once(tasks, "SIGUSR1", &next))?;
            Ok::<(), TsonicError>(())
        });
        process::with_default_signals(|tasks| process::once(tasks, "SIGUSR1", &nested)).unwrap();
    }
    process::kill_named(pid, "SIGUSR1").unwrap();
    assert_eq!(count.get(), 0);
    poll_until_received();
    assert_eq!(count.get(), 1);
    assert!(!process::poll_signals().unwrap());
    if scenario == "once-default" {
        process::kill_named(pid, "SIGUSR1").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        panic!("once must restore the native default after its only listener");
    }
    if scenario == "reentrant" {
        process::kill_named(pid, "SIGUSR1").unwrap();
        poll_until_received();
        assert_eq!(count.get(), 2);
    }
    let signal_error =
        process::with_default_signals(|tasks| process::once(tasks, "SIGKILL", &listener))
            .unwrap_err();
    assert_eq!(signal_error.code, "ERR_UNSUPPORTED_OPERATION");
    assert_eq!(
        process::with_default_signals(|tasks| process::once(tasks, "INVALID", &listener))
            .unwrap_err()
            .code,
        "ERR_UNKNOWN_SIGNAL"
    );
}

enum SignalFailure {
    Original(Rc<Cell<i64>>),
    Native(tsonic_rust_node::NodeError),
}

impl From<tsonic_rust_node::NodeError> for SignalFailure {
    fn from(error: tsonic_rust_node::NodeError) -> Self {
        Self::Native(error)
    }
}

fn verify_typed_signal_owners(scenario: &str) {
    use tsonic_rust_runtime::dispatch::{
        poll_phase, prepend, DispatchContexts, DispatchEnd, DispatchPhase,
    };
    let first = process::SignalTasks::<SignalFailure>::new();
    let second = process::SignalTasks::<SignalFailure>::new();
    let original = Rc::new(Cell::new(9_007_199_254_740_993));
    let captured = Rc::clone(&original);
    let failed = Callable::new(move |()| Err(SignalFailure::Original(Rc::clone(&captured))));
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let pending = Callable::new(move |()| {
        observed.set(observed.get() + 1);
        Ok::<(), SignalFailure>(())
    });
    process::once(&first, "SIGUSR1", &failed).unwrap();
    process::once(&second, "SIGUSR1", &pending).unwrap();
    assert!(!first.has_work());
    assert!(!second.has_work());
    if scenario == "typed-drop" {
        drop(first);
        drop(second);
        process::kill_named(process::pid(), "SIGUSR1").unwrap();
        panic!("dropping the final typed root must restore the native signal default");
    }
    process::kill_named(process::pid(), "SIGUSR1").unwrap();
    let group = prepend(
        &first,
        prepend(&second, DispatchEnd::<SignalFailure>::new()),
    );
    let returned = poll_phase(&group, DispatchPhase::Signals)
        .err()
        .expect("original typed signal failure");
    match returned {
        SignalFailure::Original(value) => {
            assert!(Rc::ptr_eq(&value, &original));
            assert_eq!(value.get(), 9_007_199_254_740_993);
        }
        SignalFailure::Native(error) => {
            panic!("native error replaced source failure: {}", error.code())
        }
    }
    assert_eq!(calls.get(), 0);
    assert_eq!(poll_phase(&group, DispatchPhase::Signals).ok(), Some(true));
    assert_eq!(calls.get(), 1);
    assert_eq!(poll_phase(&group, DispatchPhase::Signals).ok(), Some(false));
    assert_eq!(calls.get(), 1);
}

fn poll_until_received() {
    let started = std::time::Instant::now();
    while !process::poll_signals().unwrap() {
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "native signal was not delivered"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn process_kill_preserves_checked_numbers_and_native_errors() {
    assert!(process::kill_number(process::pid() as f64, 0.0).unwrap());
    assert!(process::kill_number(0.0, 0.0).unwrap());
    for pid in [
        f64::NAN,
        f64::INFINITY,
        1.5,
        i32::MAX as f64 + 1.0,
        i32::MIN as f64 - 1.0,
    ] {
        assert_eq!(
            process::kill_number(pid, 0.0).unwrap_err().code,
            "ERR_OUT_OF_RANGE"
        );
    }
    for signal in [f64::NAN, f64::INFINITY, 1.5, -1.0, i32::MAX as f64] {
        assert_eq!(
            process::kill_number(process::pid() as f64, signal)
                .unwrap_err()
                .code,
            "ERR_UNKNOWN_SIGNAL"
        );
    }
    assert_eq!(
        process::kill_named(process::pid() as f64, "INVALID")
            .unwrap_err()
            .code,
        "ERR_UNKNOWN_SIGNAL"
    );
    assert_eq!(
        process::kill_number(i32::MAX as f64, 0.0).unwrap_err().code,
        "ESRCH"
    );
}
