const MAX_PENDING_RUNTIME_TASKS: usize = 1 << 20;

use std::future::Future;
use std::sync::Arc;
use std::task::{Wake, Waker};
use std::time::Duration;

use tsonic_rust_js::event_loop::EventLoopDriver;
use tsonic_rust_runtime::TsonicResult;

thread_local! {
    static RUNTIME_TASKS: std::cell::RefCell<std::collections::VecDeque<RuntimeTask>> =
        std::cell::RefCell::new(std::collections::VecDeque::new());
}

type RuntimeTask = Box<dyn FnOnce() -> tsonic_rust_runtime::TsonicResult<()>>;

pub(crate) fn enqueue_runtime_task(
    task: impl FnOnce() -> tsonic_rust_runtime::TsonicResult<()> + 'static,
) -> crate::NodeResult<()> {
    RUNTIME_TASKS.with(|tasks| {
        let mut tasks = tasks.borrow_mut();
        if tasks.len() >= MAX_PENDING_RUNTIME_TASKS {
            return Err(crate::NodeError::new(
                "ERR_NODE_RUNTIME_TASK_LIMIT",
                "pending Node runtime tasks exceed the finite queue limit",
            ));
        }
        tasks.push_back(Box::new(task));
        Ok(())
    })
}

fn poll_runtime_tasks() -> tsonic_rust_runtime::TsonicResult<bool> {
    let tasks = RUNTIME_TASKS.with(|tasks| {
        let mut tasks = tasks.borrow_mut();
        let count = tasks.len();
        tasks.drain(..count).collect::<Vec<_>>()
    });
    let did_work = !tasks.is_empty();
    for task in tasks {
        task()?;
    }
    Ok(did_work)
}

fn has_runtime_tasks() -> bool {
    RUNTIME_TASKS.with(|tasks| !tasks.borrow().is_empty())
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
    tsonic_rust_js::event_loop::run_with_driver(&mut NodeDriver)
}

pub fn block_on<Output>(future: impl Future<Output = Output>) -> TsonicResult<Output> {
    tsonic_rust_js::event_loop::block_on_with_driver(future, &mut NodeDriver)
}

struct NodeDriver;

struct NodeWake(Arc<mio::Waker>);

impl Wake for NodeWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.wake().expect("live Node readiness wakeup");
    }
}

impl EventLoopDriver for NodeDriver {
    fn poll(&mut self) -> TsonicResult<bool> {
        let can_dispatch_signals = has_runtime_work();
        let background_work = crate::background::poll()?;
        let task_work = poll_runtime_tasks()?;
        let signal_work = can_dispatch_signals && crate::process::poll_signals()?;
        let timer_work = crate::timers::poll_runtime_timers()?;
        let server_work = crate::http::poll_runtime_servers()?;
        let net_work = crate::net::poll_runtime_servers()?;
        let tls_work = crate::tls::poll_runtime_servers()?;
        let watcher_work = crate::fs::poll_runtime_watchers()?;
        let worker_work = crate::worker_threads::poll_runtime_workers()?;
        let port_work = crate::worker_threads::poll_runtime_ports()?;
        Ok(background_work
            || task_work
            || signal_work
            || timer_work
            || server_work
            || net_work
            || tls_work
            || watcher_work
            || worker_work
            || port_work)
    }

    fn has_work(&self) -> bool {
        has_runtime_work()
    }

    fn wait(&mut self, js_timer_delay: Option<Duration>) -> TsonicResult<()> {
        let timer_delay = [
            crate::timers::next_runtime_timer_delay(),
            js_timer_delay,
            crate::fs::next_runtime_watcher_delay(),
            crate::worker_threads::next_runtime_reap_delay(),
        ]
        .into_iter()
        .flatten()
        .min();
        crate::readiness::wait(timer_delay)?;
        Ok(())
    }

    fn waker(&mut self) -> TsonicResult<Waker> {
        Ok(Waker::from(Arc::new(NodeWake(crate::readiness::waker()?))))
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
