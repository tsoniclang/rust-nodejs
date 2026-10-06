const MAX_PENDING_RUNTIME_TASKS: usize = 1 << 20;

use std::cell::OnceCell;
use std::future::Future;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::task::{Wake, Waker};

use crate::dispatch::{DispatchContexts, DispatchEnd, DispatchPhase};
use tsonic_rust_js::event_loop::EventLoopDriver;
use tsonic_rust_runtime::dispatch_queue::{TaskBudget, TaskQueue};
use tsonic_rust_runtime::TsonicResult;

thread_local! {
    static RUNTIME_TASK_BUDGET: OnceCell<TaskBudget> = const { OnceCell::new() };
    static RUNTIME_TASKS: OnceCell<TaskQueue<tsonic_rust_runtime::TsonicError>> =
        const { OnceCell::new() };
}

pub(crate) fn runtime_task_budget() -> TaskBudget {
    RUNTIME_TASK_BUDGET.with(|budget| {
        budget
            .get_or_init(|| {
                TaskBudget::new(
                    NonZeroUsize::new(MAX_PENDING_RUNTIME_TASKS).expect("finite native task limit"),
                )
            })
            .clone()
    })
}

pub(crate) fn enqueue_runtime_task(
    task: impl FnOnce() -> tsonic_rust_runtime::TsonicResult<()> + 'static,
) -> crate::NodeResult<()> {
    RUNTIME_TASKS.with(|tasks| {
        tasks
            .get_or_init(|| TaskQueue::new(runtime_task_budget()))
            .enqueue(task)
            .map(|_| ())
            .map_err(crate::NodeError::from)
    })
}

fn poll_runtime_tasks() -> tsonic_rust_runtime::TsonicResult<bool> {
    RUNTIME_TASKS.with(|tasks| tasks.get().map_or(Ok(false), TaskQueue::poll_ready))
}

fn has_runtime_tasks() -> bool {
    RUNTIME_TASKS.with(|tasks| {
        tasks
            .get()
            .is_some_and(|tasks| tasks.front_ticket().is_some())
    })
}

fn has_runtime_work() -> bool {
    crate::background::has_pending_work()
        || has_runtime_tasks()
        || crate::http::has_active_runtime_servers()
        || crate::net::has_refed_runtime_servers()
        || crate::tls::has_refed_runtime_servers()
        || crate::timers::has_refed_runtime_timers()
        || crate::fs::has_refed_runtime_watchers()
        || crate::worker_threads::has_refed_runtime_workers()
        || crate::worker_threads::has_refed_runtime_ports()
}

pub fn run_event_loop() -> tsonic_rust_runtime::TsonicResult<()> {
    run_with_contexts(DispatchEnd::<tsonic_rust_runtime::TsonicError>::new())
}

pub fn block_on<Output>(future: impl Future<Output = Output>) -> TsonicResult<Output> {
    block_on_with_contexts(
        future,
        DispatchEnd::<tsonic_rust_runtime::TsonicError>::new(),
    )
}

pub fn run_with_contexts<TContexts: DispatchContexts>(
    contexts: TContexts,
) -> Result<(), TContexts::Error>
where
    TContexts::Error: From<tsonic_rust_runtime::TsonicError>,
{
    tsonic_rust_js::event_loop::run_with_driver(&mut NodeDriver { contexts })
}

pub fn block_on_with_contexts<TOutput, TContexts: DispatchContexts>(
    future: impl Future<Output = TOutput>,
    contexts: TContexts,
) -> Result<TOutput, TContexts::Error>
where
    TContexts::Error: From<tsonic_rust_runtime::TsonicError>,
{
    tsonic_rust_js::event_loop::block_on_with_driver(future, &mut NodeDriver { contexts })
}

struct NodeDriver<TContexts> {
    contexts: TContexts,
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

    fn poll(&mut self) -> Result<bool, Self::Error> {
        let mut did_work = false;
        let mut can_dispatch_signals = false;
        for phase in [
            DispatchPhase::JsTimers,
            DispatchPhase::Background,
            DispatchPhase::RuntimeTasks,
            DispatchPhase::Signals,
            DispatchPhase::Timers,
            DispatchPhase::Http,
            DispatchPhase::Net,
            DispatchPhase::Tls,
            DispatchPhase::Watchers,
            DispatchPhase::Workers,
            DispatchPhase::Ports,
        ] {
            let work = if phase == DispatchPhase::Background {
                crate::background::with_default(|native| {
                    crate::dispatch::poll_phase(
                        &crate::dispatch::prepend(native, &self.contexts),
                        phase,
                    )
                })?
            } else {
                let frontier = self.contexts.prepare(phase)?;
                let native =
                    poll_native_phase(phase, can_dispatch_signals).map_err(Self::Error::from)?;
                let selected = crate::dispatch::poll_prepared(&self.contexts, &frontier)?;
                native || selected
            };
            did_work |= work;
            if phase == DispatchPhase::JsTimers {
                can_dispatch_signals = has_runtime_work() || self.contexts.has_work();
            }
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
            crate::fs::next_runtime_watcher_delay(),
            crate::worker_threads::next_runtime_reap_delay(),
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

fn poll_native_phase(phase: DispatchPhase, can_dispatch_signals: bool) -> TsonicResult<bool> {
    match phase {
        DispatchPhase::JsTimers => tsonic_rust_js::timers::poll_timers(),
        DispatchPhase::Background => {
            unreachable!("native background belongs to the composed phase")
        }
        DispatchPhase::RuntimeTasks => poll_runtime_tasks(),
        DispatchPhase::Signals => {
            if can_dispatch_signals {
                crate::process::poll_signals().map_err(tsonic_rust_runtime::TsonicError::from)
            } else {
                Ok(false)
            }
        }
        DispatchPhase::Timers => crate::timers::poll_runtime_timers(),
        DispatchPhase::Http => crate::http::poll_runtime_servers(),
        DispatchPhase::Net => crate::net::poll_runtime_servers(),
        DispatchPhase::Tls => crate::tls::poll_runtime_servers(),
        DispatchPhase::Watchers => crate::fs::poll_runtime_watchers(),
        DispatchPhase::Workers => crate::worker_threads::poll_runtime_workers(),
        DispatchPhase::Ports => crate::worker_threads::poll_runtime_ports(),
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
    fn context_free_queries_do_not_allocate_a_native_task_root() {
        assert!(!super::has_runtime_tasks());
        assert!(!super::poll_runtime_tasks().unwrap());
        assert!(super::RUNTIME_TASKS.with(|tasks| tasks.get().is_none()));
        assert!(super::RUNTIME_TASK_BUDGET.with(|budget| budget.get().is_none()));
    }

    #[test]
    fn runtime_tasks_preserve_exact_failures_and_uninvoked_work() {
        let original = tsonic_rust_runtime::JsError::error("original native failure");
        let retained = original.clone();
        let observed = std::rc::Rc::new(Cell::new(0));
        let completed = std::rc::Rc::clone(&observed);
        super::enqueue_runtime_task(move || Err(retained.into())).unwrap();
        super::enqueue_runtime_task(move || {
            completed.set(7);
            Ok(())
        })
        .unwrap();
        let failure = super::poll_runtime_tasks().unwrap_err();
        assert!(matches!(failure, tsonic_rust_runtime::TsonicError::Js(_)));
        assert!(failure.source_error().has_same_identity(&original));
        assert_eq!(observed.get(), 0);
        assert!(super::has_runtime_tasks());
        assert!(super::poll_runtime_tasks().unwrap());
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
        let source = JsPromise::create(Callable::new(
            |(resolve, _): (PromiseResolve<i32>, PromiseReject)| {
                tsonic_rust_js::timers::set_timeout_callable(
                    Callable::new(move |()| resolve.call((PromiseResolution::Value(3),))),
                    1.0,
                );
                Ok(())
            },
        ));
        let completed = std::rc::Rc::new(Cell::new(0));
        let observed = std::rc::Rc::clone(&completed);
        drop(source.then(
            Callable::new(move |(value,)| {
                observed.set(value);
                Ok(())
            }),
            None,
        ));
        super::run_event_loop().unwrap();
        assert_eq!(completed.get(), 3);
    }
}
