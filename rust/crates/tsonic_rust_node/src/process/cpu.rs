use super::{CpuUsage, NodeError, NodeResult};

pub fn cpu_usage(previous: Option<CpuUsage>) -> NodeResult<CpuUsage> {
    subtract_previous(process_cpu()?, previous)
}

pub fn cpu_usage_current() -> NodeResult<CpuUsage> {
    cpu_usage(None)
}

pub fn cpu_usage_since(previous: CpuUsage) -> NodeResult<CpuUsage> {
    cpu_usage(Some(previous))
}

pub fn thread_cpu_usage(previous: Option<CpuUsage>) -> NodeResult<CpuUsage> {
    subtract_previous(thread_cpu()?, previous)
}

fn subtract_previous(current: CpuUsage, previous: Option<CpuUsage>) -> NodeResult<CpuUsage> {
    let Some(previous) = previous else {
        return Ok(current);
    };
    for value in [previous.user, previous.system] {
        if value < 0 {
            return Err(NodeError::new(
                "ERR_INVALID_ARG_VALUE",
                "CPU usage fields must be non-negative integers",
            ));
        }
    }
    Ok(CpuUsage {
        user: current.user - previous.user,
        system: current.system - previous.system,
    })
}

#[cfg(unix)]
fn process_cpu() -> NodeResult<CpuUsage> {
    read_usage(nix::sys::resource::UsageWho::RUSAGE_SELF)
}

#[cfg(not(unix))]
fn process_cpu() -> NodeResult<CpuUsage> {
    Err(NodeError::new(
        "ERR_PROCESS_CPU_UNSUPPORTED",
        "native CPU usage is unavailable on this platform",
    ))
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
fn thread_cpu() -> NodeResult<CpuUsage> {
    read_usage(nix::sys::resource::UsageWho::RUSAGE_THREAD)
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_os = "freebsd")))]
fn thread_cpu() -> NodeResult<CpuUsage> {
    Err(NodeError::new(
        "ERR_THREAD_CPU_UNSUPPORTED",
        "native thread CPU usage is unavailable on this platform",
    ))
}

#[cfg(unix)]
fn read_usage(who: nix::sys::resource::UsageWho) -> NodeResult<CpuUsage> {
    use nix::sys::time::TimeValLike;
    let usage = nix::sys::resource::getrusage(who)
        .map_err(|error| NodeError::new("ERR_CPU_USAGE", error.to_string()))?;
    Ok(CpuUsage {
        user: usage.user_time().num_microseconds(),
        system: usage.system_time().num_microseconds(),
    })
}
