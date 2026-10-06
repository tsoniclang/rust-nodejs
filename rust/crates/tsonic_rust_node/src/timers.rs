use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use tsonic_rust_runtime::dispatch::{DispatchContexts, DispatchPhase};
use tsonic_rust_runtime::timer_queue::{TimerContext, TimerHandle};
use tsonic_rust_runtime::{Callable, TsonicResult};

use crate::error::{NodeError, NodeResult};

type MutableCallback = Rc<RefCell<dyn FnMut() -> TsonicResult<()>>>;

pub const fn new<TError>() -> TimerContext<Callable<(), Result<(), TError>>> {
    TimerContext::new(DispatchPhase::Timers)
}

thread_local! {
    static DEFAULT_TIMERS: TimerContext<MutableCallback> = const { TimerContext::new(DispatchPhase::Timers) };
}

pub(crate) fn with_default<TOutput>(
    operation: impl FnOnce(&TimerContext<MutableCallback>) -> TOutput,
) -> TOutput {
    DEFAULT_TIMERS.with(operation)
}

pub struct Timeout<TCallback = MutableCallback> {
    handle: TimerHandle<TCallback>,
}

impl<TCallback> Clone for Timeout<TCallback> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
        }
    }
}

impl<TCallback> std::fmt::Debug for Timeout<TCallback> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.handle.fmt(formatter)
    }
}

impl<TCallback> PartialEq for Timeout<TCallback> {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

impl<TCallback> Eq for Timeout<TCallback> {}

impl<TCallback> Timeout<TCallback> {
    pub fn id(&self) -> u64 {
        self.handle.id()
    }

    pub fn has_ref(&self) -> bool {
        self.handle.has_ref()
    }

    pub fn unref(&mut self) -> &mut Self {
        self.handle.set_ref(false);
        self
    }

    pub fn r#ref(&mut self) -> &mut Self {
        self.handle.set_ref(true);
        self
    }

    pub fn refresh(&mut self) -> &mut Self {
        self.handle.refresh().expect("live native timer deadline");
        self
    }

    pub fn close(&mut self) -> &mut Self {
        self.handle.close();
        self
    }

    pub fn delay_ms(&self) -> u64 {
        self.handle
            .delay()
            .as_millis()
            .try_into()
            .expect("native timer delay entered as u64 milliseconds")
    }

    pub fn on_timeout(&self, callback: impl FnOnce()) {
        callback();
    }

    pub fn on_immediate(&self, callback: impl FnOnce()) {
        callback();
    }
}

pub type Immediate<TCallback = MutableCallback> = Timeout<TCallback>;
pub type Timer<TCallback = MutableCallback> = Timeout<TCallback>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerOptions {
    pub r#ref: bool,
    pub signal_aborted: bool,
}

impl Default for TimerOptions {
    fn default() -> Self {
        Self {
            r#ref: true,
            signal_aborted: false,
        }
    }
}

pub fn set_timeout(callback: impl FnOnce() + 'static, delay_ms: u64) -> Timeout {
    set_timeout_with_options(callback, delay_ms, TimerOptions::default())
}

pub fn set_timeout_with_options(
    callback: impl FnOnce() + 'static,
    delay_ms: u64,
    options: TimerOptions,
) -> Timeout {
    let mut callback = Some(callback);
    schedule(
        move || {
            if let Some(callback) = callback.take() {
                callback();
            }
            Ok(())
        },
        delay_ms,
        false,
        options,
    )
}

pub fn set_immediate(callback: impl FnOnce() + 'static) -> Timeout {
    set_immediate_with_options(callback, TimerOptions::default())
}

pub fn set_immediate_with_options(
    callback: impl FnOnce() + 'static,
    options: TimerOptions,
) -> Timeout {
    set_timeout_with_options(callback, 0, options)
}

pub fn set_interval(callback: impl FnMut() + 'static, delay_ms: u64) -> Timeout {
    set_interval_with_options(callback, delay_ms, TimerOptions::default())
}

pub fn set_interval_with_options(
    callback: impl FnMut() + 'static,
    delay_ms: u64,
    options: TimerOptions,
) -> Timeout {
    let mut callback = callback;
    schedule(
        move || {
            callback();
            Ok(())
        },
        delay_ms.max(1),
        true,
        options,
    )
}

pub fn set_interval_callable<TError>(
    timers: &TimerContext<Callable<(), Result<(), TError>>>,
    callback: Callable<(), Result<(), TError>>,
    delay_ms: i32,
) -> NodeResult<Timeout<Callable<(), Result<(), TError>>>> {
    schedule_callable(
        timers,
        callback,
        u64::try_from(delay_ms).unwrap_or(0).max(1),
        true,
    )
}

pub fn set_timeout_callable<TError>(
    timers: &TimerContext<Callable<(), Result<(), TError>>>,
    callback: Callable<(), Result<(), TError>>,
    delay_ms: i32,
) -> NodeResult<Timeout<Callable<(), Result<(), TError>>>> {
    schedule_callable(
        timers,
        callback,
        u64::try_from(delay_ms).unwrap_or(0),
        false,
    )
}

fn schedule_callable<TError>(
    timers: &TimerContext<Callable<(), Result<(), TError>>>,
    callback: Callable<(), Result<(), TError>>,
    delay_ms: u64,
    interval: bool,
) -> NodeResult<Timeout<Callable<(), Result<(), TError>>>> {
    timers
        .schedule_with(
            Duration::from_millis(delay_ms),
            interval,
            true,
            false,
            || callback,
        )
        .map(|handle| Timeout { handle })
        .map_err(NodeError::from)
}

pub fn clear_timeout<TCallback>(timeout: &mut Timeout<TCallback>) {
    timeout.handle.close();
}

pub fn clear_immediate<TCallback>(timeout: &mut Timeout<TCallback>) {
    clear_timeout(timeout);
}

pub fn clear_interval<TCallback>(timeout: &mut Timeout<TCallback>) {
    clear_timeout(timeout);
}

pub(crate) fn has_refed_runtime_timers() -> bool {
    with_default(TimerContext::has_pending_work)
}

pub(crate) fn next_runtime_timer_delay() -> Option<Duration> {
    with_default(DispatchContexts::next_delay)
}

#[cfg(test)]
fn poll_runtime_timers() -> TsonicResult<bool> {
    with_default(TimerContext::poll)
}

fn schedule(
    callback: impl FnMut() -> TsonicResult<()> + 'static,
    delay_ms: u64,
    interval: bool,
    options: TimerOptions,
) -> Timeout {
    with_default(|timers| {
        timers
            .schedule_with(
                Duration::from_millis(delay_ms),
                interval,
                options.r#ref,
                options.signal_aborted,
                || Rc::new(RefCell::new(callback)) as MutableCallback,
            )
            .map(|handle| Timeout { handle })
            .expect("native timer admission")
    })
}

#[cfg(test)]
mod tests;

pub mod promises {
    use super::{set_timeout_with_options, Timeout, TimerOptions};

    pub fn set_timeout_value<T>(delay_ms: u64, value: T) -> (Timeout, T) {
        set_timeout_value_with_options(delay_ms, value, TimerOptions::default())
    }

    pub fn set_timeout_value_with_options<T>(
        delay_ms: u64,
        value: T,
        options: TimerOptions,
    ) -> (Timeout, T) {
        let timeout = set_timeout_with_options(|| {}, delay_ms, options);
        (timeout, value)
    }

    pub fn set_immediate_value<T>(value: T) -> (Timeout, T) {
        set_timeout_value(0, value)
    }

    pub fn set_immediate_value_with_options<T>(value: T, options: TimerOptions) -> (Timeout, T) {
        set_timeout_value_with_options(0, value, options)
    }

    pub fn set_interval_values<T: Clone>(
        delay_ms: u64,
        value: T,
        count: usize,
    ) -> (Timeout, Vec<T>) {
        set_interval_values_with_options(delay_ms, value, count, TimerOptions::default())
    }

    pub fn set_interval_values_with_options<T: Clone>(
        delay_ms: u64,
        value: T,
        count: usize,
        options: TimerOptions,
    ) -> (Timeout, Vec<T>) {
        let timeout = set_timeout_with_options(|| {}, delay_ms, options);
        (timeout, vec![value; count])
    }

    pub mod scheduler {
        pub fn wait(delay_ms: u64) {
            if delay_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }
        }

        pub fn yield_now() {
            std::thread::yield_now();
        }
    }
}
