use crate::error::{NodeError, NodeResult};
use tsonic_rust_runtime::Callable;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Process;

pub fn process() -> Process {
    Process
}

pub fn kill(pid: f64, signal: Option<i32>) -> NodeResult<bool> {
    if !pid.is_finite() || pid.fract() != 0.0 || pid < i32::MIN as f64 || pid > i32::MAX as f64 {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            "pid must be a signed 32-bit integer",
        ));
    }
    kill_native(pid as i32, signal.unwrap_or(15))
}

pub fn kill_named(pid: f64, signal: &str) -> NodeResult<bool> {
    kill(pid, Some(signal_number(signal)?))
}

pub fn kill_number(pid: f64, signal: f64) -> NodeResult<bool> {
    if !signal.is_finite() || signal.fract() != 0.0 || signal < 0.0 || signal > i32::MAX as f64 {
        return Err(NodeError::new(
            "ERR_UNKNOWN_SIGNAL",
            "signal must be a nonnegative integer",
        ));
    }
    kill(pid, Some(signal as i32))
}

pub fn kill_default(pid: f64) -> NodeResult<bool> {
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

pub fn once<E>(signal: &str, listener: &Callable<(), Result<(), E>>) -> NodeResult<Process>
where
    E: std::fmt::Display + 'static,
{
    #[cfg(unix)]
    native::once(signal, listener)?;
    #[cfg(not(unix))]
    {
        let _ = listener;
        signal_number(signal)?;
    }
    Ok(Process)
}

pub fn remove_listener<E>(
    signal: &str,
    listener: &Callable<(), Result<(), E>>,
) -> NodeResult<Process>
where
    E: 'static,
{
    #[cfg(unix)]
    native::remove_listener(signal, listener)?;
    #[cfg(not(unix))]
    {
        let _ = listener;
        signal_number(signal)?;
    }
    Ok(Process)
}

pub fn poll_signals() -> NodeResult<bool> {
    #[cfg(unix)]
    {
        native::poll()
    }
    #[cfg(not(unix))]
    {
        Ok(false)
    }
}

#[cfg(unix)]
mod native {
    use super::*;
    use crate::events::EventEmitter;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, OnceLock};
    use std::thread::ThreadId;
    use tsonic_rust_js::JsValue;

    static OWNER: OnceLock<ThreadId> = OnceLock::new();
    thread_local! {
        static STATE: RefCell<SignalState> = RefCell::new(SignalState::default());
    }

    struct SignalFlags {
        wake: crate::readiness::SignalWake,
        pending: Arc<AtomicBool>,
        default_action: Arc<AtomicBool>,
        default_enabled: bool,
        event: JsValue,
    }

    #[derive(Default)]
    struct SignalState {
        emitter: EventEmitter,
        signals: BTreeMap<i32, SignalFlags>,
    }

    impl Drop for SignalState {
        fn drop(&mut self) {
            for flags in self.signals.values() {
                flags
                    .default_action
                    .store(flags.default_enabled, Ordering::SeqCst);
            }
        }
    }

    fn check_owner() -> NodeResult<()> {
        if !crate::worker_threads::is_main_thread() {
            return Err(NodeError::new(
                "ERR_UNSUPPORTED_OPERATION",
                "process signal listeners are unavailable in workers",
            ));
        }
        let current = std::thread::current().id();
        if *OWNER.get_or_init(|| current) != current {
            return Err(NodeError::new(
                "ERR_UNSUPPORTED_OPERATION",
                "process signal listeners belong to one event-loop thread",
            ));
        }
        Ok(())
    }

    fn event(name: &str) -> JsValue {
        JsValue::String((name).to_owned())
    }

    fn registration_error(error: std::io::Error) -> NodeError {
        let code = error
            .raw_os_error()
            .map(|code| format!("{:?}", nix::errno::Errno::from_raw(code)))
            .unwrap_or_else(|| "ERR_SIGNAL_REGISTRATION".to_owned());
        NodeError::new(code, error.to_string())
    }

    pub(super) fn once<E>(name: &str, listener: &Callable<(), Result<(), E>>) -> NodeResult<()>
    where
        E: std::fmt::Display + 'static,
    {
        check_owner()?;
        let signal = signal_number(name)?;
        if signal_hook::consts::FORBIDDEN.contains(&signal) {
            return Err(NodeError::new(
                "ERR_UNSUPPORTED_OPERATION",
                "this native signal cannot safely register an asynchronous listener",
            ));
        }
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            if let std::collections::btree_map::Entry::Vacant(entry) = state.signals.entry(signal) {
                let default_enabled = signal != nix::libc::SIGPIPE;
                let flags = SignalFlags {
                    wake: crate::readiness::SignalWake::new(signal)?,
                    pending: Arc::new(AtomicBool::new(false)),
                    default_action: Arc::new(AtomicBool::new(default_enabled)),
                    default_enabled,
                    event: event(name),
                };
                let default_hook = signal_hook::flag::register_conditional_default(
                    signal,
                    Arc::clone(&flags.default_action),
                )
                .map_err(registration_error)?;
                if let Err(error) = signal_hook::flag::register(signal, Arc::clone(&flags.pending))
                {
                    signal_hook::low_level::unregister(default_hook);
                    return Err(registration_error(error));
                }
                entry.insert(flags);
            }
            let selected = state
                .signals
                .get(&signal)
                .expect("registered signal")
                .event
                .clone();
            state.emitter.once_callable(&selected, listener)?;
            state
                .signals
                .get(&signal)
                .expect("registered signal")
                .default_action
                .store(false, Ordering::SeqCst);
            Ok(())
        })
    }

    pub(super) fn remove_listener<E>(
        name: &str,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<()>
    where
        E: 'static,
    {
        check_owner()?;
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            let event = event(name);
            state.emitter.off_callable(&event, listener)?;
            if state.emitter.callable_listener_count(&event)? == 0 {
                if let Ok(signal) = signal_number(name) {
                    if let Some(flags) = state.signals.get(&signal) {
                        flags
                            .default_action
                            .store(flags.default_enabled, Ordering::SeqCst);
                    }
                }
            }
            Ok(())
        })
    }

    pub(super) fn poll() -> NodeResult<bool> {
        if OWNER
            .get()
            .is_none_or(|owner| *owner != std::thread::current().id())
        {
            return Ok(false);
        }
        STATE.with(|state| -> NodeResult<()> {
            for flags in state.borrow().signals.values() {
                flags.wake.drain()?;
            }
            Ok(())
        })?;
        let pending = STATE.with(|state| {
            state
                .borrow()
                .signals
                .iter()
                .filter_map(|(signal, flags)| {
                    flags
                        .pending
                        .swap(false, Ordering::SeqCst)
                        .then_some(*signal)
                })
                .collect::<Vec<_>>()
        });
        let mut dispatched = false;
        for signal in pending {
            let emission = STATE.with(|state| -> NodeResult<_> {
                let mut state = state.borrow_mut();
                let flags = state.signals.get(&signal).expect("retained signal");
                let event = flags.event.clone();
                let emission = state.emitter.prepare_callable_emission(&event, &[])?;
                if state.emitter.callable_listener_count(&event)? == 0 {
                    let flags = state.signals.get(&signal).expect("retained signal");
                    flags
                        .default_action
                        .store(flags.default_enabled, Ordering::SeqCst);
                }
                Ok(emission)
            })?;
            dispatched |= emission.invoke(&[])?;
        }
        Ok(dispatched)
    }
}
