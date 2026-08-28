use std::time::Duration;

const MAX_PENDING_RUNTIME_TASKS: usize = 1 << 20;

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

pub fn run_event_loop() -> tsonic_rust_runtime::TsonicResult<()> {
    loop {
        let task_work = poll_runtime_tasks()?;
        let js_timer_work = tsonic_rust_js::timers::poll_timers()?;
        let timer_work = crate::timers::poll_runtime_timers()?;
        let server_work = crate::http::poll_runtime_servers()?;
        let net_work = crate::net::poll_runtime_servers()?;
        let tls_work = crate::tls::poll_runtime_servers()?;
        let watcher_work = crate::fs::poll_runtime_watchers()?;
        let worker_work = crate::worker_threads::poll_runtime_workers()?;
        let port_work = crate::worker_threads::poll_runtime_ports()?;
        if !has_runtime_tasks() && !tsonic_rust_js::timers::has_timers() &&
            !crate::http::has_active_runtime_servers() &&
            !crate::net::has_refed_runtime_servers() &&
            !crate::tls::has_refed_runtime_servers() &&
            !crate::timers::has_refed_runtime_timers() &&
            !crate::fs::has_refed_runtime_watchers() &&
            !crate::worker_threads::has_refed_runtime_workers() &&
            !crate::worker_threads::has_refed_runtime_ports()
        {
            return Ok(());
        }
        if !task_work && !js_timer_work && !timer_work && !server_work && !net_work && !tls_work && !watcher_work &&
            !worker_work && !port_work
        {
            let timer_delay = [
                crate::timers::next_runtime_timer_delay(),
                tsonic_rust_js::timers::next_timer_delay(),
            ]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(Duration::from_millis(10));
            std::thread::sleep(timer_delay.min(Duration::from_millis(10)));
        }
    }
}
