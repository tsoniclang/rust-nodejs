use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tsonic_rust_runtime::ordered_dispatch::poll_ordered_entries;
use tsonic_rust_runtime::{Callable, TsonicResult};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

type TimerCallback = Rc<RefCell<dyn FnMut() -> TsonicResult<()>>>;

struct TimerEntry {
    callback: TimerCallback,
    delay: Duration,
    due: Instant,
    interval: bool,
    refed: bool,
}

thread_local! {
    static TIMERS: RefCell<BTreeMap<u64, TimerEntry>> = const { RefCell::new(BTreeMap::new()) };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timeout {
    id: u64,
    delay_ms: u64,
}

impl Timeout {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn has_ref(&self) -> bool {
        TIMERS.with(|timers| {
            timers
                .borrow()
                .get(&self.id)
                .is_some_and(|entry| entry.refed)
        })
    }

    pub fn unref(&mut self) -> &mut Self {
        update_entry(self.id, |entry| entry.refed = false);
        self
    }

    pub fn r#ref(&mut self) -> &mut Self {
        update_entry(self.id, |entry| entry.refed = true);
        self
    }

    pub fn refresh(&mut self) -> &mut Self {
        update_entry(self.id, |entry| entry.due = Instant::now() + entry.delay);
        self
    }

    pub fn close(&mut self) -> &mut Self {
        remove_entry(self.id);
        self
    }

    pub fn delay_ms(&self) -> u64 {
        self.delay_ms
    }

    pub fn on_timeout(&self, callback: impl FnOnce()) {
        callback();
    }

    pub fn on_immediate(&self, callback: impl FnOnce()) {
        callback();
    }
}

pub type Immediate = Timeout;
pub type Timer = Timeout;

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
    let mut callback = Some(callback);
    schedule(
        move || {
            if let Some(callback) = callback.take() {
                callback();
            }
            Ok(())
        },
        0,
        false,
        options,
    )
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

pub fn set_interval_callable<E>(callback: Callable<(), Result<(), E>>, delay_ms: i32) -> Timeout
where
    E: std::fmt::Display + 'static,
{
    let delay_ms = u64::try_from(delay_ms).unwrap_or(0).max(1);
    schedule(
        move || {
            callback
                .call(())
                .map_err(crate::error::callback_runtime_error)
        },
        delay_ms,
        true,
        TimerOptions::default(),
    )
}

pub fn set_timeout_callable<E>(callback: Callable<(), Result<(), E>>, delay_ms: i32) -> Timeout
where
    E: std::fmt::Display + 'static,
{
    let delay_ms = u64::try_from(delay_ms).unwrap_or(0);
    schedule(
        move || {
            callback
                .call(())
                .map_err(crate::error::callback_runtime_error)
        },
        delay_ms,
        false,
        TimerOptions::default(),
    )
}

pub fn clear_timeout(timeout: &mut Timeout) {
    remove_entry(timeout.id);
}

pub fn clear_immediate(timeout: &mut Timeout) {
    clear_timeout(timeout);
}

pub fn clear_interval(timeout: &mut Timeout) {
    clear_timeout(timeout);
}

pub(crate) fn has_refed_runtime_timers() -> bool {
    TIMERS.with(|timers| timers.borrow().values().any(|entry| entry.refed))
}

pub(crate) fn next_runtime_timer_delay() -> Option<Duration> {
    let now = Instant::now();
    TIMERS.with(|timers| {
        timers
            .borrow()
            .values()
            .map(|entry| entry.due.saturating_duration_since(now))
            .min()
    })
}

pub(crate) fn poll_runtime_timers() -> TsonicResult<bool> {
    let now = Instant::now();
    TIMERS.with(|timers| {
        poll_ordered_entries(
            timers,
            |entry| entry.due <= now,
            |timers, id| {
                let entry = timers.get_mut(&id).expect("selected native timer");
                if entry.interval {
                    entry.due = now + entry.delay;
                    Rc::clone(&entry.callback)
                } else {
                    timers.remove(&id).expect("selected native timer").callback
                }
            },
            |callback| callback.borrow_mut()(),
        )
    })
}

fn schedule(
    callback: impl FnMut() -> TsonicResult<()> + 'static,
    delay_ms: u64,
    interval: bool,
    options: TimerOptions,
) -> Timeout {
    let id = NEXT_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .expect("native timer identity range is exhausted");
    let timeout = Timeout { id, delay_ms };
    if options.signal_aborted {
        return timeout;
    }
    let delay = Duration::from_millis(delay_ms);
    TIMERS.with(|timers| {
        timers.borrow_mut().insert(
            id,
            TimerEntry {
                callback: Rc::new(RefCell::new(callback)),
                delay,
                due: Instant::now() + delay,
                interval,
                refed: options.r#ref,
            },
        );
    });
    timeout
}

fn update_entry(id: u64, update: impl FnOnce(&mut TimerEntry)) {
    TIMERS.with(|timers| {
        if let Some(entry) = timers.borrow_mut().get_mut(&id) {
            update(entry);
        }
    });
}

fn remove_entry(id: u64) {
    TIMERS.with(|timers| {
        timers.borrow_mut().remove(&id);
    });
}

#[cfg(test)]
mod dispatch_tests {
    use super::*;
    use std::cell::Cell;
    use tsonic_rust_runtime::ErrorObject;

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
}

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
