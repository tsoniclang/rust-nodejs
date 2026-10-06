use crate::error::{NodeError, NodeResult};
use std::cell::OnceCell;
use std::time::Duration;
use tsonic_rust_runtime::dispatch::{DispatchContexts, DispatchPhase};
use tsonic_rust_runtime::Callable;

thread_local! {
    static DEFAULT_SIGNALS: SignalTasks<tsonic_rust_runtime::TsonicError> = const { SignalTasks::new() };
}

pub struct SignalTasks<E: 'static> {
    #[cfg(unix)]
    source: OnceCell<native::SignalSource<E>>,
    #[cfg(not(unix))]
    error: std::marker::PhantomData<fn() -> E>,
}

pub struct SignalFrontier {
    #[cfg(unix)]
    ready: Vec<(i32, u64)>,
    #[cfg(unix)]
    cursor: std::cell::Cell<usize>,
    owner: usize,
}

impl<E: 'static> Default for SignalTasks<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: 'static> SignalTasks<E> {
    pub const fn new() -> Self {
        Self {
            #[cfg(unix)]
            source: OnceCell::new(),
            #[cfg(not(unix))]
            error: std::marker::PhantomData,
        }
    }

    pub(super) fn once(
        &self,
        name: &str,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<()> {
        #[cfg(unix)]
        {
            native::check_owner()?;
            let signal = super::signal_number(name)?;
            if signal_hook::consts::FORBIDDEN.contains(&signal) {
                return Err(NodeError::new(
                    "ERR_UNSUPPORTED_OPERATION",
                    "this native signal cannot safely register an asynchronous listener",
                ));
            }
            self.source
                .get_or_init(native::SignalSource::new)
                .once(signal, name, listener)
        }
        #[cfg(not(unix))]
        {
            let _ = listener;
            super::signal_number(name).map(|_| ())
        }
    }

    pub(super) fn remove_listener(
        &self,
        name: &str,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<()> {
        #[cfg(unix)]
        {
            native::check_owner()?;
            if let Some(source) = self.source.get() {
                source.remove_listener(super::signal_number(name)?, name, listener)?;
            }
            Ok(())
        }
        #[cfg(not(unix))]
        {
            let _ = listener;
            super::signal_number(name).map(|_| ())
        }
    }
}

impl<E: From<NodeError> + 'static> DispatchContexts for SignalTasks<E> {
    type Error = E;
    type Frontier = SignalFrontier;

    fn prepare(&self, phase: DispatchPhase) -> Result<Self::Frontier, E> {
        #[cfg(unix)]
        let ready = if phase == DispatchPhase::Signals {
            self.source
                .get()
                .map_or(Ok(Vec::new()), native::SignalSource::ready)?
        } else {
            Vec::new()
        };
        #[cfg(not(unix))]
        let _ = phase;
        Ok(SignalFrontier {
            #[cfg(unix)]
            ready,
            #[cfg(unix)]
            cursor: std::cell::Cell::new(0),
            owner: self as *const Self as usize,
        })
    }

    fn next_ready(&self, frontier: &Self::Frontier) -> Option<u64> {
        if frontier.owner != self as *const Self as usize {
            return None;
        }
        #[cfg(unix)]
        {
            frontier
                .ready
                .get(frontier.cursor.get())
                .map(|(signal, _)| *signal as u64)
        }
        #[cfg(not(unix))]
        {
            None
        }
    }

    fn poll_next(&self, frontier: &Self::Frontier) -> Result<bool, E> {
        if self.next_ready(frontier).is_none() {
            return Ok(false);
        }
        #[cfg(unix)]
        {
            let (signal, generation) = frontier.ready[frontier.cursor.get()];
            frontier.cursor.set(frontier.cursor.get() + 1);
            self.source
                .get()
                .expect("prepared signal source")
                .emit(signal, generation)
        }
        #[cfg(not(unix))]
        {
            Ok(false)
        }
    }

    fn has_work(&self) -> bool {
        false
    }
    fn next_delay(&self) -> Option<Duration> {
        None
    }
}

pub fn with_default<TOutput>(
    operation: impl FnOnce(&SignalTasks<tsonic_rust_runtime::TsonicError>) -> TOutput,
) -> TOutput {
    DEFAULT_SIGNALS.with(operation)
}

#[cfg(unix)]
mod native {
    use super::*;
    use crate::events::EventEmitter;
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeMap;
    use std::rc::{Rc, Weak};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, OnceLock};
    use std::thread::ThreadId;
    use tsonic_rust_js::JsValue;

    static OWNER: OnceLock<ThreadId> = OnceLock::new();
    thread_local! {
        static FLAGS: RefCell<BTreeMap<i32, Rc<SignalFlags>>> = const { RefCell::new(BTreeMap::new()) };
    }

    struct SignalFlags {
        wake: crate::readiness::SignalWake,
        pending: Arc<AtomicBool>,
        default_action: Arc<AtomicBool>,
        default_enabled: bool,
        generation: Cell<u64>,
        listeners: RefCell<Vec<Weak<Cell<usize>>>>,
        default_hook: signal_hook::SigId,
        pending_hook: signal_hook::SigId,
    }

    impl SignalFlags {
        fn update_default(&self) {
            let mut listeners = self.listeners.borrow_mut();
            listeners.retain(|count| count.strong_count() != 0);
            let has_listeners = listeners
                .iter()
                .any(|count| count.upgrade().is_some_and(|count| count.get() != 0));
            self.default_action
                .store(self.default_enabled && !has_listeners, Ordering::SeqCst);
        }

        fn generation(&self) -> NodeResult<u64> {
            self.wake.drain()?;
            if self.pending.swap(false, Ordering::SeqCst) {
                self.generation
                    .set(self.generation.get().checked_add(1).ok_or_else(|| {
                        NodeError::new(
                            "ERR_SIGNAL_GENERATION_LIMIT",
                            "native signal delivery generation is exhausted",
                        )
                    })?);
            }
            Ok(self.generation.get())
        }
    }

    impl Drop for SignalFlags {
        fn drop(&mut self) {
            self.default_action
                .store(self.default_enabled, Ordering::SeqCst);
            signal_hook::low_level::unregister(self.default_hook);
            signal_hook::low_level::unregister(self.pending_hook);
        }
    }

    struct Subscription {
        flags: Rc<SignalFlags>,
        count: Rc<Cell<usize>>,
        observed: Cell<u64>,
        event: JsValue,
    }

    impl Drop for Subscription {
        fn drop(&mut self) {
            self.count.set(0);
            self.flags.update_default();
        }
    }

    pub(super) struct SignalSource<E: 'static> {
        emitter: EventEmitter<E>,
        subscriptions: RefCell<BTreeMap<i32, Subscription>>,
    }

    impl<E: 'static> SignalSource<E> {
        pub(super) fn new() -> Self {
            Self {
                emitter: EventEmitter::new(),
                subscriptions: RefCell::new(BTreeMap::new()),
            }
        }

        pub(super) fn once(
            &self,
            signal: i32,
            name: &str,
            listener: &Callable<(), Result<(), E>>,
        ) -> NodeResult<()> {
            let flags = signal_flags(signal)?;
            let mut subscriptions = self.subscriptions.borrow_mut();
            let subscription = subscriptions.entry(signal).or_insert_with(|| {
                let count = Rc::new(Cell::new(0));
                flags.listeners.borrow_mut().push(Rc::downgrade(&count));
                Subscription {
                    observed: Cell::new(flags.generation.get()),
                    flags,
                    count,
                    event: JsValue::String(name.to_owned()),
                }
            });
            let emitter = &self.emitter;
            emitter.once_callable(&subscription.event, listener)?;
            subscription
                .count
                .set(emitter.callable_listener_count(&subscription.event)?);
            subscription.flags.update_default();
            Ok(())
        }

        pub(super) fn remove_listener(
            &self,
            signal: i32,
            name: &str,
            listener: &Callable<(), Result<(), E>>,
        ) -> NodeResult<()> {
            let event = JsValue::String(name.to_owned());
            let emitter = &self.emitter;
            emitter.off_callable(&event, listener)?;
            if let Some(subscription) = self.subscriptions.borrow().get(&signal) {
                subscription
                    .count
                    .set(emitter.callable_listener_count(&event)?);
                subscription.flags.update_default();
            }
            Ok(())
        }

        pub(super) fn ready(&self) -> NodeResult<Vec<(i32, u64)>> {
            let mut ready = Vec::new();
            for (signal, subscription) in self.subscriptions.borrow().iter() {
                let generation = subscription.flags.generation()?;
                if subscription.count.get() != 0 && generation > subscription.observed.get() {
                    ready.push((*signal, generation));
                }
            }
            Ok(ready)
        }

        fn update_count(&self, signal: i32) {
            let subscriptions = self.subscriptions.borrow();
            if let Some(subscription) = subscriptions.get(&signal) {
                subscription.count.set(
                    self.emitter
                        .callable_listener_count(&subscription.event)
                        .expect("validated signal event"),
                );
                subscription.flags.update_default();
            }
        }
    }

    impl<E: From<NodeError> + 'static> SignalSource<E> {
        pub(super) fn emit(&self, signal: i32, generation: u64) -> Result<bool, E> {
            let emission = {
                let subscriptions = self.subscriptions.borrow();
                let Some(subscription) = subscriptions.get(&signal) else {
                    return Ok(false);
                };
                if subscription.count.get() == 0 || generation <= subscription.observed.get() {
                    return Ok(false);
                }
                subscription.observed.set(generation);
                self.emitter
                    .prepare_callable_emission(&subscription.event, &[])?
            };
            let result = emission.invoke_with_before(&[], || self.update_count(signal));
            self.update_count(signal);
            result
        }
    }

    pub(super) fn check_owner() -> NodeResult<()> {
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

    fn registration_error(error: std::io::Error) -> NodeError {
        let code = error
            .raw_os_error()
            .map(|code| format!("{:?}", nix::errno::Errno::from_raw(code)))
            .unwrap_or_else(|| "ERR_SIGNAL_REGISTRATION".to_owned());
        NodeError::new(code, error.to_string())
    }

    fn signal_flags(signal: i32) -> NodeResult<Rc<SignalFlags>> {
        FLAGS.with(|flags| {
            let mut flags = flags.borrow_mut();
            if let Some(flags) = flags.get(&signal) {
                return Ok(Rc::clone(flags));
            }
            let default_enabled = signal != nix::libc::SIGPIPE;
            let default_action = Arc::new(AtomicBool::new(default_enabled));
            let pending = Arc::new(AtomicBool::new(false));
            let wake = crate::readiness::SignalWake::new(signal)?;
            let default_hook = signal_hook::flag::register_conditional_default(
                signal,
                Arc::clone(&default_action),
            )
            .map_err(registration_error)?;
            let pending_hook = match signal_hook::flag::register(signal, Arc::clone(&pending)) {
                Ok(hook) => hook,
                Err(error) => {
                    signal_hook::low_level::unregister(default_hook);
                    return Err(registration_error(error));
                }
            };
            let value = Rc::new(SignalFlags {
                wake,
                pending,
                default_action,
                default_enabled,
                generation: Cell::new(0),
                listeners: RefCell::new(Vec::new()),
                default_hook,
                pending_hook,
            });
            flags.insert(signal, Rc::clone(&value));
            Ok(value)
        })
    }
}
