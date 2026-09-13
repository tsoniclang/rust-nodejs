use std::process::Command;
use tsonic_rust_js::{JsArray, JsStringNumber, Uint8Array};

use super::{
    capture, capture_command, prepare_command, NodeError, NodeResult, SpawnOptions,
    SpawnSyncArguments, SpawnSyncResult, Stdio, StdioOptions,
};
use crate::process::ProcessEnv;

#[derive(Debug, Clone, Default)]
pub struct SpawnSyncOptions {
    pub encoding: Option<String>,
    pub max_buffer: Option<f64>,
    pub cwd: Option<String>,
    pub env: Option<ProcessEnv>,
    pub uid: Option<f64>,
    pub gid: Option<f64>,
    pub stdio: Option<JsArray<JsStringNumber>>,
    pub input: Option<Uint8Array>,
    pub timeout: Option<f64>,
    pub kill_signal: Option<String>,
}

fn integer(value: f64, maximum: f64, label: &str) -> NodeResult<u64> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > maximum {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            format!("{label} is outside its exact integer range"),
        ));
    }
    Ok(value as u64)
}

impl SpawnSyncOptions {
    fn native(&self) -> NodeResult<SpawnOptions> {
        if self
            .encoding
            .as_deref()
            .is_some_and(|encoding| encoding != "buffer")
        {
            return Err(NodeError::new(
                "ERR_INVALID_ARG_VALUE",
                "binary spawnSync requires buffer encoding",
            ));
        }
        let mut stdio = Vec::new();
        if let Some(values) = &self.stdio {
            for (index, value) in values.values().into_iter().enumerate() {
                let mode = match value {
                    None | Some(JsStringNumber::Undefined) | Some(JsStringNumber::Null) => {
                        if index < 3 {
                            Stdio::Pipe
                        } else {
                            Stdio::Ignore
                        }
                    }
                    Some(JsStringNumber::String(value)) => {
                        if value == "pipe" {
                            Stdio::Pipe
                        } else if value == "ignore" {
                            Stdio::Ignore
                        } else if value == "inherit" {
                            Stdio::Inherit
                        } else {
                            return Err(NodeError::new(
                                "ERR_INVALID_ARG_VALUE",
                                "unsupported spawnSync stdio mode",
                            ));
                        }
                    }
                    Some(JsStringNumber::Number(value)) => {
                        Stdio::Descriptor(
                            integer(value, i32::MAX as f64, "stdio descriptor")? as i32
                        )
                    }
                };
                stdio.push(mode);
            }
        }
        let at = |index| stdio.get(index).copied().unwrap_or(Stdio::Pipe);
        Ok(SpawnOptions {
            cwd: self.cwd.as_ref().map(Into::into),
            env: self.env.as_ref().map(ProcessEnv::entries),
            uid: self
                .uid
                .map(|value| integer(value, u32::MAX as f64, "uid").map(|value| value as u32))
                .transpose()?,
            gid: self
                .gid
                .map(|value| integer(value, u32::MAX as f64, "gid").map(|value| value as u32))
                .transpose()?,
            stdio: StdioOptions::tuple(at(0), at(1), at(2)),
            extra_stdio: stdio.into_iter().skip(3).collect(),
            input: self
                .input
                .as_ref()
                .map(|input| input.with_bytes(<[u8]>::to_vec)),
            max_buffer: Some(integer(
                self.max_buffer.unwrap_or(1024.0 * 1024.0),
                (usize::MAX as f64).min(9_007_199_254_740_991.0),
                "maxBuffer",
            )? as usize),
            timeout_ms: self
                .timeout
                .map(|value| integer(value, 9_007_199_254_740_991.0, "timeout"))
                .transpose()?,
            kill_signal: self.kill_signal.clone(),
            ..SpawnOptions::default()
        })
    }
}

pub fn spawn_sync_result_with_options<Arguments: SpawnSyncArguments + ?Sized>(
    program: &str,
    arguments: &Arguments,
    options: SpawnSyncOptions,
) -> NodeResult<SpawnSyncResult> {
    let options = options.native()?;
    let command = arguments
        .with_spawn_sync_arguments(|arguments| prepare_command(program, arguments, &options))??;
    let output = capture_command(command, &options);
    Ok(match output {
        Ok(output) => SpawnSyncResult {
            pid: output.pid.map(f64::from),
            status: output.status,
            signal: output.signal,
            error: output.error,
            stdout: matches!(options.stdio.stdout, Stdio::Pipe)
                .then(|| crate::buffer::Buffer::from_bytes(output.stdout)),
            stderr: matches!(options.stdio.stderr, Stdio::Pipe)
                .then(|| crate::buffer::Buffer::from_bytes(output.stderr)),
        },
        Err(error) => SpawnSyncResult {
            pid: None,
            status: None,
            signal: None,
            error: Some(error),
            stdout: None,
            stderr: None,
        },
    })
}

#[cfg(unix)]
pub(super) fn descriptor(fd: i32) -> NodeResult<std::fs::File> {
    use std::os::fd::AsFd;
    let owned = match fd {
        0 => std::io::stdin().as_fd().try_clone_to_owned(),
        1 => std::io::stdout().as_fd().try_clone_to_owned(),
        2 => std::io::stderr().as_fd().try_clone_to_owned(),
        _ => return crate::fs::clone_file_descriptor(fd),
    }
    .map_err(capture::io_error)?;
    Ok(owned.into())
}

#[cfg(not(unix))]
pub(super) fn descriptor(fd: i32) -> NodeResult<std::fs::File> {
    crate::fs::clone_file_descriptor(fd)
}

#[cfg(unix)]
pub(super) fn configure_native(command: &mut Command, options: &SpawnOptions) -> NodeResult<()> {
    use command_fds::{CommandFdExt, FdMapping};
    use std::os::unix::process::CommandExt;
    use std::str::FromStr;
    if let Some(uid) = options.uid {
        command.uid(uid);
    }
    if let Some(gid) = options.gid {
        command.gid(gid);
    }
    if let Some(argv0) = &options.argv0 {
        command.arg0(argv0);
    }
    if let Some(signal) = &options.kill_signal {
        nix::sys::signal::Signal::from_str(signal)
            .map_err(|_| NodeError::new("ERR_UNKNOWN_SIGNAL", signal))?;
    }
    let mut mappings = Vec::new();
    for (index, mode) in options.extra_stdio.iter().enumerate() {
        let child_fd = i32::try_from(index + 3)
            .map_err(|_| NodeError::new("ERR_OUT_OF_RANGE", "too many stdio descriptors"))?;
        let file = match mode {
            Stdio::Descriptor(fd) => descriptor(*fd)?,
            Stdio::Ignore => std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/null")
                .map_err(capture::io_error)?,
            Stdio::Inherit => descriptor(child_fd)?,
            Stdio::Pipe | Stdio::Ipc => {
                return Err(NodeError::new(
                    "ERR_UNSUPPORTED_OPERATION",
                    "extra synchronous pipe/IPC descriptors are not supported",
                ))
            }
        };
        mappings.push(FdMapping {
            parent_fd: file.into(),
            child_fd,
        });
    }
    if !mappings.is_empty() {
        command
            .fd_mappings(mappings)
            .map_err(|error| NodeError::new("EINVAL", error.to_string()))?;
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn configure_native(_command: &mut Command, options: &SpawnOptions) -> NodeResult<()> {
    if options.uid.is_some()
        || options.gid.is_some()
        || !options.extra_stdio.is_empty()
        || options.argv0.is_some()
    {
        return Err(NodeError::new(
            "ERR_FEATURE_UNAVAILABLE",
            "native spawn identity/extra descriptors require Unix",
        ));
    }
    if options
        .kill_signal
        .as_deref()
        .is_some_and(|signal| !matches!(signal, "SIGTERM" | "SIGKILL" | "SIGINT"))
    {
        return Err(NodeError::new(
            "ERR_UNKNOWN_SIGNAL",
            "unsupported Windows termination signal",
        ));
    }
    Ok(())
}
