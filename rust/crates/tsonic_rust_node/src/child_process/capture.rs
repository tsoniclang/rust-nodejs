use std::io::{Read, Write};
use std::process::{Child, ExitStatus};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::{NodeError, NodeResult, SpawnOptions, SpawnOutput};

fn record_failure(failure: &Mutex<Option<NodeError>>, error: NodeError) {
    failure
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .get_or_insert(error);
}

fn capture(reader: Option<impl Read>, limit: usize, failure: &Mutex<Option<NodeError>>) -> Vec<u8> {
    let Some(mut reader) = reader else {
        return Vec::new();
    };
    let mut output = Vec::new();
    let mut block = [0_u8; 8192];
    loop {
        let count = match reader.read(&mut block) {
            Ok(0) => return output,
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                record_failure(failure, io_error(error));
                return output;
            }
        };
        let retained = count.min(limit.saturating_sub(output.len()));
        if let Err(error) = output.try_reserve(retained) {
            record_failure(failure, NodeError::new("ENOMEM", error.to_string()));
            return output;
        }
        output.extend_from_slice(&block[..retained]);
        if retained < count {
            record_failure(
                failure,
                NodeError::new("ENOBUFS", "child process output exceeded maxBuffer"),
            );
        }
    }
}

pub(super) fn collect(mut child: Child, options: &SpawnOptions) -> NodeResult<SpawnOutput> {
    let pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdin = child.stdin.take();
    let failure = Mutex::new(None);
    let limit = options.max_buffer.unwrap_or(1024 * 1024);
    let started = Instant::now();
    std::thread::scope(|scope| {
        let stdout = scope.spawn(|| capture(stdout, limit, &failure));
        let stderr = scope.spawn(|| capture(stderr, limit, &failure));
        let stdin = scope.spawn(|| match (stdin, options.input.as_ref()) {
            (Some(mut writer), Some(input)) => writer.write_all(input).map_err(io_error),
            _ => Ok(()),
        });
        let mut error = None;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Err(wait_error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(io_error(wait_error));
                }
                Ok(None) => {}
            }
            let timeout = options.timeout_ms.is_some_and(|timeout| {
                timeout != 0 && started.elapsed() >= Duration::from_millis(timeout)
            });
            let io_failure = failure
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .clone();
            if error.is_none() && (io_failure.is_some() || timeout) {
                error = io_failure.or_else(|| {
                    Some(NodeError::new(
                        "ETIMEDOUT",
                        "child process exceeded timeout",
                    ))
                });
                if let Err(kill_error) = terminate(
                    &mut child,
                    options.kill_signal.as_deref().unwrap_or("SIGTERM"),
                ) {
                    let _ = child.kill();
                    error = Some(kill_error);
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        };
        let stdout = stdout
            .join()
            .map_err(|_| NodeError::new("ERR_CHILD_PROCESS_IO", "stdout reader failed"))?;
        let stderr = stderr
            .join()
            .map_err(|_| NodeError::new("ERR_CHILD_PROCESS_IO", "stderr reader failed"))?;
        let input = stdin
            .join()
            .map_err(|_| NodeError::new("ERR_CHILD_PROCESS_IO", "stdin writer failed"))?;
        let status = status?;
        if error.is_none() {
            error = failure
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .clone();
        }
        if error.is_none() {
            error = input.err();
        }
        Ok(SpawnOutput {
            pid: Some(pid),
            status: status.code(),
            signal: signal_name(status),
            stdout,
            stderr,
            error,
        })
    })
}

pub(super) fn io_error(error: std::io::Error) -> NodeError {
    let code = match error.kind() {
        std::io::ErrorKind::NotFound => "ENOENT",
        std::io::ErrorKind::PermissionDenied => "EACCES",
        std::io::ErrorKind::BrokenPipe => "EPIPE",
        std::io::ErrorKind::InvalidInput => "EINVAL",
        _ => "EIO",
    };
    NodeError::new(code, error.to_string())
}

#[cfg(unix)]
fn terminate(child: &mut Child, name: &str) -> NodeResult<()> {
    use std::str::FromStr;
    let signal = nix::sys::signal::Signal::from_str(name)
        .map_err(|_| NodeError::new("ERR_UNKNOWN_SIGNAL", name))?;
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(child.id() as i32), signal)
        .map_err(|error| NodeError::new(error.to_string(), error.to_string()))
}

#[cfg(not(unix))]
fn terminate(child: &mut Child, _name: &str) -> NodeResult<()> {
    child.kill().map_err(io_error)
}

#[cfg(unix)]
pub(super) fn signal_name(status: ExitStatus) -> Option<String> {
    use std::os::unix::process::ExitStatusExt;
    status
        .signal()
        .and_then(|signal| nix::sys::signal::Signal::try_from(signal).ok())
        .map(|signal| format!("{signal:?}"))
}

#[cfg(not(unix))]
pub(super) fn signal_name(_status: ExitStatus) -> Option<String> {
    None
}
