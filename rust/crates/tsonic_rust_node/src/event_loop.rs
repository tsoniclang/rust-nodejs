use std::future::Future;
use std::sync::Arc;
use std::task::{Wake, Waker};

use tsonic_rust_js::event_loop::{EventLoopDriver, LoopExitStatus, LoopLifecycle};
use tsonic_rust_runtime::dispatch::{DispatchContexts, DispatchEnd, DispatchPhase};
use tsonic_rust_runtime::TsonicResult;

fn has_runtime_tasks() -> bool {
    crate::runtime_tasks::with_default(crate::runtime_tasks::RuntimeTasks::has_pending_work)
}

fn has_runtime_work() -> bool {
    crate::background::has_pending_work()
        || has_runtime_tasks()
        || crate::http::with_default_http(DispatchContexts::has_work)
        || crate::net::with_default(DispatchContexts::has_work)
        || crate::tls::with_default_tls(DispatchContexts::has_work)
        || crate::timers::has_refed_runtime_timers()
        || crate::fs::with_default_watchers(DispatchContexts::has_work)
        || crate::worker_threads::resources::with_default(DispatchContexts::has_work)
}

const STOPPED_BEFORE_COMPLETION: &str =
    "The event loop stopped on a requested exit before the awaited future completed.";

pub fn run_event_loop() -> tsonic_rust_runtime::TsonicResult<()> {
    run_with_contexts(DispatchEnd::<tsonic_rust_runtime::TsonicError>::new()).map(|_| ())
}

pub fn block_on<Output>(future: impl Future<Output = Output>) -> TsonicResult<Output> {
    let (output, _) = block_on_with_contexts(
        future,
        DispatchEnd::<tsonic_rust_runtime::TsonicError>::new(),
    )?;
    output.ok_or_else(|| tsonic_rust_runtime::JsError::error(STOPPED_BEFORE_COMPLETION).into())
}

pub fn run_with_contexts<TContexts: DispatchContexts>(
    contexts: TContexts,
) -> Result<LoopExitStatus, TContexts::Error>
where
    TContexts::Error: From<tsonic_rust_runtime::TsonicError>,
{
    tsonic_rust_js::event_loop::run_with_driver(&mut NodeDriver { contexts })
}

pub fn block_on_with_contexts<TOutput, TContexts: DispatchContexts>(
    future: impl Future<Output = TOutput>,
    contexts: TContexts,
) -> Result<(Option<TOutput>, LoopExitStatus), TContexts::Error>
where
    TContexts::Error: From<tsonic_rust_runtime::TsonicError>,
{
    tsonic_rust_js::event_loop::block_on_with_driver(future, &mut NodeDriver { contexts })
}

pub fn run_entry_with_contexts<TOutput, TContexts: DispatchContexts>(
    entry: impl FnOnce() -> TOutput,
    contexts: TContexts,
) -> Result<(TOutput, LoopExitStatus), TContexts::Error>
where
    TContexts::Error: From<tsonic_rust_runtime::TsonicError>,
{
    tsonic_rust_js::event_loop::run_entry_with_driver(entry, &mut NodeDriver { contexts })
}

struct NodeDriver<TContexts> {
    contexts: TContexts,
}

impl<TContexts: DispatchContexts> NodeDriver<TContexts> {
    fn poll_context<TNative: DispatchContexts>(
        &self,
        native: &TNative,
        phase: DispatchPhase,
    ) -> Result<bool, TContexts::Error>
    where
        TContexts::Error: From<TNative::Error>,
    {
        tsonic_rust_runtime::dispatch::poll_phase(
            &tsonic_rust_runtime::dispatch::prepend(native, &self.contexts),
            phase,
        )
    }
}

struct NodeWake(Arc<mio::Waker>);

impl Wake for NodeWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.wake().expect("live Node readiness wakeup");
    }
}

impl<TContexts: DispatchContexts> EventLoopDriver for NodeDriver<TContexts>
where
    TContexts::Error: From<tsonic_rust_runtime::TsonicError>,
{
    type Error = TContexts::Error;

    fn admission_error(error: tsonic_rust_runtime::JsError) -> Self::Error {
        tsonic_rust_runtime::TsonicError::from(error).into()
    }

    fn lifecycle(&mut self, transition: LoopLifecycle) -> Result<(), Self::Error> {
        match transition {
            LoopLifecycle::Enter(owner) => {
                let entered = tsonic_rust_js::timers::with_default(|context| context.enter(owner))
                    && crate::timers::with_default(|context| context.enter(owner))
                    && self.contexts.enter(owner)
                    && crate::background::with_default(|context| context.enter(owner))
                    && crate::runtime_tasks::with_default(|context| context.enter(owner))
                    && crate::http::with_default_http(|context| context.enter(owner))
                    && crate::net::with_default(|context| context.enter(owner))
                    && crate::tls::with_default_tls(|context| context.enter(owner))
                    && crate::fs::with_default_watchers(|context| context.enter(owner))
                    && crate::worker_threads::resources::with_default(|context| {
                        context.enter(owner)
                    })
                    && crate::process::with_default_signals(|context| context.enter(owner));
                if !entered {
                    return Err(Self::admission_error(tsonic_rust_runtime::JsError::error("native dispatch context belongs to another event loop or has exhausted its generations")));
                }
            }
            LoopLifecycle::Shutdown(owner) => {
                crate::runtime_tasks::with_default(|context| context.close(owner));
                crate::background::with_default(|context| context.close(owner));
                self.contexts.close(owner);
                crate::worker_threads::resources::with_default(|context| context.close(owner));
                crate::fs::with_default_watchers(|context| context.close(owner));
                crate::tls::with_default_tls(|context| context.close(owner));
                crate::net::with_default(|context| context.close(owner));
                crate::http::with_default_http(|context| context.close(owner));
                crate::process::with_default_signals(|context| context.close(owner));
                crate::timers::with_default(|context| context.close(owner));
                tsonic_rust_js::timers::with_default(|context| context.close(owner));
            }
        }
        Ok(())
    }

    fn poll(&mut self) -> Result<bool, Self::Error> {
        let mut did_work = tsonic_rust_js::timers::with_default(|native| {
            self.poll_context(native, tsonic_rust_js::timers::PHASE)
        })?;
        let can_dispatch_signals = has_runtime_work() || self.contexts.has_work();
        for domain in crate::phases::ORDER {
            use crate::phases::Phase;
            let phase = domain.registration();
            let work = match domain {
                Phase::Background => {
                    crate::background::with_default(|native| self.poll_context(native, phase))?
                }
                Phase::RuntimeTasks => {
                    crate::runtime_tasks::with_default(|native| self.poll_context(native, phase))?
                }
                Phase::Signals if can_dispatch_signals => {
                    crate::process::with_default_signals(|native| self.poll_context(native, phase))?
                }
                Phase::Signals => false,
                Phase::Timers => {
                    crate::timers::with_default(|native| self.poll_context(native, phase))?
                }
                Phase::Http => {
                    crate::http::with_default_http(|native| self.poll_context(native, phase))?
                }
                Phase::Net => crate::net::with_default(|native| self.poll_context(native, phase))?,
                Phase::Tls => {
                    crate::tls::with_default_tls(|native| self.poll_context(native, phase))?
                }
                Phase::Watchers => {
                    crate::fs::with_default_watchers(|native| self.poll_context(native, phase))?
                }
                Phase::Workers | Phase::Ports => {
                    crate::worker_threads::resources::with_default(|native| {
                        self.poll_context(native, phase)
                    })?
                }
            };
            did_work |= work;
        }
        Ok(did_work)
    }

    fn has_work(&self) -> bool {
        has_runtime_work() || tsonic_rust_js::timers::has_timers() || self.contexts.has_work()
    }

    fn wait(&mut self) -> Result<(), Self::Error> {
        let timer_delay = [
            crate::timers::next_runtime_timer_delay(),
            tsonic_rust_js::timers::next_timer_delay(),
            crate::fs::with_default_watchers(DispatchContexts::next_delay),
            crate::worker_threads::resources::with_default(DispatchContexts::next_delay),
            self.contexts.next_delay(),
        ]
        .into_iter()
        .flatten()
        .min();
        crate::readiness::wait(timer_delay)
            .map_err(tsonic_rust_runtime::TsonicError::from)
            .map_err(Self::Error::from)?;
        Ok(())
    }

    fn waker(&mut self) -> Result<Waker, Self::Error> {
        let wake = crate::readiness::waker()
            .map_err(tsonic_rust_runtime::TsonicError::from)
            .map_err(Self::Error::from)?;
        Ok(Waker::from(Arc::new(NodeWake(wake))))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::future::poll_fn;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc};
    use std::task::{Poll, Waker};
    use tsonic_rust_js::promise::{JsPromise, PromiseReject, PromiseResolution, PromiseResolve};
    use tsonic_rust_runtime::Callable;

    #[test]
    fn runtime_tasks_preserve_exact_failures_and_uninvoked_work() {
        let original = tsonic_rust_runtime::JsError::error("original native failure");
        let retained = original.clone();
        let observed = std::rc::Rc::new(Cell::new(0));
        let completed = std::rc::Rc::clone(&observed);
        crate::runtime_tasks::with_default(|tasks| tasks.enqueue(move || Err(retained.into())))
            .unwrap();
        crate::runtime_tasks::with_default(|tasks| {
            tasks.enqueue(move || {
                completed.set(7);
                Ok(())
            })
        })
        .unwrap();
        let failure = crate::runtime_tasks::with_default(|tasks| tasks.poll()).unwrap_err();
        assert!(matches!(failure, tsonic_rust_runtime::TsonicError::Js(_)));
        assert!(failure.source_error().has_same_identity(&original));
        assert_eq!(observed.get(), 0);
        assert!(super::has_runtime_tasks());
        assert!(crate::runtime_tasks::with_default(|tasks| tasks.poll()).unwrap());
        assert_eq!(observed.get(), 7);
        assert!(!super::has_runtime_tasks());
    }

    #[test]
    fn mio_driver_receives_native_future_wakes_without_a_polling_timer() {
        let ready = Arc::new(AtomicBool::new(false));
        let worker_ready = Arc::clone(&ready);
        let (sender, receiver) = mpsc::sync_channel::<Waker>(1);
        let worker = std::thread::spawn(move || {
            let waker = receiver.recv().unwrap();
            worker_ready.store(true, Ordering::Release);
            waker.wake();
        });
        let polls = Cell::new(0);
        super::block_on(poll_fn(|context| {
            polls.set(polls.get() + 1);
            if ready.load(Ordering::Acquire) {
                Poll::Ready(())
            } else {
                sender.send(context.waker().clone()).unwrap();
                Poll::Pending
            }
        }))
        .unwrap();
        worker.join().unwrap();
        assert_eq!(polls.get(), 2);
    }

    #[test]
    fn node_driver_runs_js_timers_and_discarded_continuations() {
        let completed = std::rc::Rc::new(Cell::new(0));
        let observed = std::rc::Rc::clone(&completed);
        let ((), status) = super::run_entry_with_contexts(
            move || {
                let source = JsPromise::create(Callable::new(
                    |(resolve, _): (PromiseResolve<i32>, PromiseReject)| {
                        tsonic_rust_js::timers::with_default(|timers| {
                            tsonic_rust_js::timers::set_timeout_callable(
                                timers,
                                Callable::new(move |()| {
                                    resolve
                                        .call((PromiseResolution::Value(3),))
                                        .expect("native Promise resolution is infallible");
                                    Ok(())
                                }),
                                1.0,
                            )
                        })
                        .unwrap();
                        Ok(())
                    },
                ));
                drop(source.then(
                    Callable::new(move |(value,)| {
                        observed.set(value);
                        Ok(())
                    }),
                    None,
                ));
            },
            super::DispatchEnd::<tsonic_rust_runtime::TsonicError>::new(),
        )
        .unwrap();
        assert!(!status.exit_requested());
        assert_eq!(completed.get(), 3);
    }

    fn zero_delay_timer(ran: std::rc::Rc<Cell<bool>>) {
        tsonic_rust_js::timers::with_default(|timers| {
            tsonic_rust_js::timers::set_timeout_callable(
                timers,
                Callable::new(move |()| {
                    ran.set(true);
                    Ok(())
                }),
                0.0,
            )
        })
        .unwrap();
    }

    #[test]
    fn a_requested_exit_stops_the_node_loop_before_a_zero_delay_timer() {
        let timer_ran = std::rc::Rc::new(Cell::new(false));
        let observed = std::rc::Rc::clone(&timer_ran);
        let ((), status) = super::run_entry_with_contexts(
            move || {
                zero_delay_timer(observed);
                JsPromise::<i32>::from_fallible_factory(|| async {
                    Err(tsonic_rust_js::JsValue::from_error(
                        tsonic_rust_runtime::JsError::error("boom"),
                    ))
                })
                .observe_discarded();
            },
            super::DispatchEnd::<tsonic_rust_runtime::TsonicError>::new(),
        )
        .unwrap();
        assert!(status.exit_requested());
        assert_eq!(
            std::process::ExitCode::from(status),
            std::process::ExitCode::from(1)
        );
        assert!(
            !timer_ran.get(),
            "no loop work restarts after the exit request"
        );
    }

    #[test]
    fn without_a_discarded_rejection_the_node_loop_runs_the_zero_delay_timer() {
        let timer_ran = std::rc::Rc::new(Cell::new(false));
        let observed = std::rc::Rc::clone(&timer_ran);
        let ((), status) = super::run_entry_with_contexts(
            move || zero_delay_timer(observed),
            super::DispatchEnd::<tsonic_rust_runtime::TsonicError>::new(),
        )
        .unwrap();
        assert!(!status.exit_requested());
        assert!(timer_ran.get());
    }
}
