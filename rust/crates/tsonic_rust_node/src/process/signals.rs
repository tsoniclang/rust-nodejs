use crate::error::{NodeError, NodeResult};
use tsonic_rust_runtime::conversions::IntegerInput;
use tsonic_rust_runtime::Callable;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Process;

pub fn process() -> Process {
    Process
}

pub fn kill(pid: impl IntegerInput<i32>, signal: Option<i32>) -> NodeResult<bool> {
    let pid = pid
        .checked_integer()
        .ok_or_else(|| NodeError::new("ERR_OUT_OF_RANGE", "pid must be a signed 32-bit integer"))?;
    kill_native(pid, signal.unwrap_or(15))
}

pub fn kill_named(pid: impl IntegerInput<i32>, signal: &str) -> NodeResult<bool> {
    kill(pid, Some(signal_number(signal)?))
}

pub fn kill_number(
    pid: impl IntegerInput<i32>,
    signal: impl IntegerInput<i32>,
) -> NodeResult<bool> {
    let signal = signal
        .checked_integer()
        .filter(|value| *value >= 0)
        .ok_or_else(|| {
            NodeError::new("ERR_UNKNOWN_SIGNAL", "signal must be a nonnegative integer")
        })?;
    kill(pid, Some(signal))
}

pub fn kill_default(pid: impl IntegerInput<i32>) -> NodeResult<bool> {
    kill(pid, None)
}

#[cfg(unix)]
fn signal_number(name: &str) -> NodeResult<i32> {
    use std::str::FromStr;
    nix::sys::signal::Signal::from_str(name)
        .map(|signal| signal as i32)
        .map_err(|_| NodeError::new("ERR_UNKNOWN_SIGNAL", format!("unknown signal: {name}")))
}

#[cfg(unix)]
fn kill_native(pid: i32, signal: i32) -> NodeResult<bool> {
    use nix::sys::signal::{kill, Signal};
    let signal = if signal == 0 {
        None
    } else {
        Some(
            Signal::try_from(signal)
                .map_err(|_| NodeError::new("ERR_UNKNOWN_SIGNAL", "unknown signal number"))?,
        )
    };
    kill(nix::unistd::Pid::from_raw(pid), signal)
        .map(|()| true)
        .map_err(|error| NodeError::new(format!("{error:?}"), error.to_string()))
}

#[cfg(not(unix))]
fn signal_number(_name: &str) -> NodeResult<i32> {
    Err(NodeError::new(
        "ERR_FEATURE_UNAVAILABLE",
        "native process signals require Unix",
    ))
}

#[cfg(not(unix))]
fn kill_native(_pid: i32, _signal: i32) -> NodeResult<bool> {
    Err(NodeError::new(
        "ERR_FEATURE_UNAVAILABLE",
        "native process signals require Unix",
    ))
}

mod tasks;
pub use tasks::{with_default as with_default_signals, SignalTasks};

pub fn once<E: 'static>(
    tasks: &SignalTasks<E>,
    signal: &str,
    listener: &Callable<(), Result<(), E>>,
) -> NodeResult<Process> {
    tasks.once(signal, listener)?;
    Ok(Process)
}

pub fn remove_listener<E: 'static>(
    tasks: &SignalTasks<E>,
    signal: &str,
    listener: &Callable<(), Result<(), E>>,
) -> NodeResult<Process> {
    tasks.remove_listener(signal, listener)?;
    Ok(Process)
}

pub fn poll_signals() -> tsonic_rust_runtime::TsonicResult<bool> {
    with_default_signals(|tasks| {
        tsonic_rust_runtime::dispatch::poll_phase(
            tasks,
            tsonic_rust_runtime::dispatch::DispatchPhase::Signals,
        )
    })
}
