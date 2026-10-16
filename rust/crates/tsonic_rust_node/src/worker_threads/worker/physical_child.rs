use std::io;
use std::process::{Child, Command, ExitStatus};
use std::sync::Mutex;

pub(super) struct WorkerChild {
    state: Mutex<ChildState>,
}

struct ChildState {
    child: Child,
    reaped: Option<ExitStatus>,
}

impl WorkerChild {
    pub(super) fn spawn(command: &mut Command) -> io::Result<Self> {
        Ok(Self {
            state: Mutex::new(ChildState {
                child: command.spawn()?,
                reaped: None,
            }),
        })
    }

    #[cfg(test)]
    pub(super) fn finished(child: Child, status: ExitStatus) -> Self {
        Self {
            state: Mutex::new(ChildState {
                child,
                reaped: Some(status),
            }),
        }
    }

    pub(super) fn try_wait_startup(&self) -> io::Result<Option<ExitStatus>> {
        let mut state = crate::sync::lock(&self.state);
        if state.reaped.is_none() {
            state.reaped = state.child.try_wait()?;
        }
        Ok(state.reaped)
    }

    pub(super) fn kill(&self) -> io::Result<()> {
        let mut state = crate::sync::lock(&self.state);
        if state.reaped.is_some() {
            return Ok(());
        }
        state.child.kill()
    }

    pub(super) fn wait(&self) -> io::Result<ExitStatus> {
        let state = crate::sync::lock(&self.state);
        if let Some(status) = state.reaped {
            return Ok(status);
        }
        #[cfg(unix)]
        let handle = state.child.id();
        #[cfg(windows)]
        let handle = {
            use std::os::windows::io::AsRawHandle;
            state.child.as_raw_handle()
        };
        drop(state);
        #[cfg(any(unix, windows))]
        wait_without_reaping(handle)?;
        #[cfg(not(any(unix, windows)))]
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native worker waiting requires POSIX waitid or a Windows process handle",
        ));
        #[cfg(any(unix, windows))]
        {
            let mut state = crate::sync::lock(&self.state);
            let status = state.child.wait()?;
            state.reaped = Some(status);
            Ok(status)
        }
    }
}

#[cfg(unix)]
fn wait_without_reaping(process: u32) -> io::Result<()> {
    loop {
        let mut information = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                process as libc::id_t,
                information.as_mut_ptr(),
                libc::WEXITED | libc::WNOWAIT,
            )
        };
        if result == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

#[cfg(windows)]
fn wait_without_reaping(handle: *mut std::ffi::c_void) -> io::Result<()> {
    #[link(name = "kernel32")]
    extern "system" {
        fn WaitForSingleObject(handle: *mut std::ffi::c_void, milliseconds: u32) -> u32;
    }
    match unsafe { WaitForSingleObject(handle, u32::MAX) } {
        0 => Ok(()),
        u32::MAX => Err(io::Error::last_os_error()),
        status => Err(io::Error::other(format!(
            "native process wait returned unexpected status {status}"
        ))),
    }
}
