use std::time::Duration;

pub fn run_event_loop() -> tsonic_rust_runtime::TsonicResult<()> {
    loop {
        let timer_work = crate::timers::poll_runtime_timers()?;
        let server_work = crate::http::poll_runtime_servers()?;
        if !crate::http::has_active_runtime_servers() && !crate::timers::has_refed_runtime_timers()
        {
            return Ok(());
        }
        if !timer_work && !server_work {
            let timer_delay =
                crate::timers::next_runtime_timer_delay().unwrap_or(Duration::from_millis(10));
            std::thread::sleep(timer_delay.min(Duration::from_millis(10)));
        }
    }
}
