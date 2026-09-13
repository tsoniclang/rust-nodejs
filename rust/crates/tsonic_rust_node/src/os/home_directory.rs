#[cfg(unix)]
fn account_home_directory() -> NodeResult<String> {
    let failure = |message| NodeError::new("ERR_SYSTEM_ERROR", message);
    let mut buffer = vec![0_u8; 1024];
    loop {
        let mut record = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result = std::ptr::null_mut();
        let status = unsafe {
            libc::getpwuid_r(
                libc::geteuid(),
                record.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if status == libc::ERANGE {
            let length = buffer
                .len()
                .checked_mul(2)
                .ok_or_else(|| failure("Home directory lookup buffer is too large"))?;
            buffer
                .try_reserve_exact(length - buffer.len())
                .map_err(|_| failure("Home directory lookup allocation failed"))?;
            buffer.resize(length, 0);
            continue;
        }
        if status != 0 {
            return Err(NodeError::new(
                "ERR_SYSTEM_ERROR",
                std::io::Error::from_raw_os_error(status).to_string(),
            ));
        }
        if result.is_null() {
            return Err(failure("The effective user has no account directory"));
        }
        let record = unsafe { record.assume_init() };
        if record.pw_dir.is_null() {
            return Err(failure("The effective user's home directory is absent"));
        }
        return Ok(unsafe { std::ffi::CStr::from_ptr(record.pw_dir) }
            .to_string_lossy()
            .into_owned());
    }
}

#[cfg(windows)]
fn account_home_directory() -> NodeResult<String> {
    std::env::home_dir()
        .map(|path| path.to_string_lossy().into_owned())
        .ok_or_else(|| {
            NodeError::new(
                "ERR_SYSTEM_ERROR",
                "The current user profile directory is unavailable",
            )
        })
}

#[cfg(not(any(unix, windows)))]
fn account_home_directory() -> NodeResult<String> {
    Err(NodeError::new(
        "ERR_SYSTEM_ERROR",
        "This platform has no supported account directory lookup",
    ))
}
