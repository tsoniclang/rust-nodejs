#![cfg(unix)]

use std::cell::Cell;
use std::os::unix::process::ExitStatusExt;
use std::rc::Rc;
use tsonic_rust_node::{process, NodeError};
use tsonic_rust_runtime::Callable;

#[test]
fn process_signal_delivery_is_isolated_and_restores_native_defaults() {
    for scenario in [
        "delivery",
        "remove",
        "once-default",
        "no-keepalive",
        "reentrant",
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
        if matches!(scenario, "remove" | "once-default") {
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
    let count = Rc::new(Cell::new(0));
    let listener = Callable::new({
        let count = Rc::clone(&count);
        move |()| {
            count.set(count.get() + 1);
            Ok::<(), NodeError>(())
        }
    });
    let pid = process::pid() as f64;
    process::once("SIGUSR1", &listener).unwrap();
    if scenario == "no-keepalive" {
        process::kill_named(pid, "SIGUSR1").unwrap();
        let started = std::time::Instant::now();
        tsonic_rust_node::run_event_loop().unwrap();
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        assert_eq!(count.get(), 0);
        return;
    }
    if scenario == "remove" {
        process::remove_listener("SIGUSR1", &listener.clone()).unwrap();
        process::kill_named(pid, "SIGUSR1").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        panic!("removed listener must restore the signal's native default");
    }
    if scenario == "reentrant" {
        let next = listener.clone();
        let nested = Callable::new(move |()| {
            process::once("SIGUSR1", &next)?;
            Ok::<(), NodeError>(())
        });
        process::once("SIGUSR1", &nested).unwrap();
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
    let signal_error = process::once("SIGKILL", &listener).unwrap_err();
    assert_eq!(signal_error.code, "ERR_UNSUPPORTED_OPERATION");
    assert_eq!(
        process::once("INVALID", &listener).unwrap_err().code,
        "ERR_UNKNOWN_SIGNAL"
    );
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
