#[cfg(unix)]
fn platform_constant_o_rdonly() -> i32 {
    libc::O_RDONLY
}

#[cfg(not(unix))]
fn platform_constant_o_rdonly() -> i32 {
    0
}

#[cfg(unix)]
fn platform_constant_o_wronly() -> i32 {
    libc::O_WRONLY
}

#[cfg(not(unix))]
fn platform_constant_o_wronly() -> i32 {
    1
}

#[cfg(unix)]
fn platform_constant_o_rdwr() -> i32 {
    libc::O_RDWR
}

#[cfg(not(unix))]
fn platform_constant_o_rdwr() -> i32 {
    2
}

#[cfg(unix)]
fn platform_constant_o_creat() -> i32 {
    libc::O_CREAT
}

#[cfg(not(unix))]
fn platform_constant_o_creat() -> i32 {
    0x100
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_constant_o_direct() -> i32 {
    libc::O_DIRECT
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn platform_constant_o_direct() -> i32 {
    0
}

#[cfg(unix)]
fn platform_constant_o_directory() -> i32 {
    libc::O_DIRECTORY
}

#[cfg(not(unix))]
fn platform_constant_o_directory() -> i32 {
    0
}

#[cfg(unix)]
fn platform_constant_o_dsync() -> i32 {
    libc::O_DSYNC
}

#[cfg(not(unix))]
fn platform_constant_o_dsync() -> i32 {
    0
}

#[cfg(unix)]
fn platform_constant_o_excl() -> i32 {
    libc::O_EXCL
}

#[cfg(not(unix))]
fn platform_constant_o_excl() -> i32 {
    0x400
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_constant_o_noatime() -> i32 {
    libc::O_NOATIME
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn platform_constant_o_noatime() -> i32 {
    0
}

#[cfg(unix)]
fn platform_constant_o_noctty() -> i32 {
    libc::O_NOCTTY
}

#[cfg(not(unix))]
fn platform_constant_o_noctty() -> i32 {
    0
}

#[cfg(unix)]
fn platform_constant_o_nofollow() -> i32 {
    libc::O_NOFOLLOW
}

#[cfg(not(unix))]
fn platform_constant_o_nofollow() -> i32 {
    0
}

#[cfg(unix)]
fn platform_constant_o_nonblock() -> i32 {
    libc::O_NONBLOCK
}

#[cfg(not(unix))]
fn platform_constant_o_nonblock() -> i32 {
    0
}

#[cfg(unix)]
fn platform_constant_o_sync() -> i32 {
    libc::O_SYNC
}

#[cfg(not(unix))]
fn platform_constant_o_sync() -> i32 {
    0
}

#[cfg(unix)]
fn platform_constant_o_trunc() -> i32 {
    libc::O_TRUNC
}

#[cfg(not(unix))]
fn platform_constant_o_trunc() -> i32 {
    0x200
}

#[cfg(unix)]
fn platform_constant_o_append() -> i32 {
    libc::O_APPEND
}

#[cfg(not(unix))]
fn platform_constant_o_append() -> i32 {
    0x8
}

fn map_io_error(error: std::io::Error) -> NodeError {
    let code = platform_io_error_code(&error).unwrap_or_else(|| match error.kind() {
        std::io::ErrorKind::NotFound => "ENOENT",
        std::io::ErrorKind::AlreadyExists => "EEXIST",
        std::io::ErrorKind::PermissionDenied => "EACCES",
        std::io::ErrorKind::IsADirectory => "EISDIR",
        std::io::ErrorKind::NotADirectory => "ENOTDIR",
        _ => "EIO",
    });
    NodeError::new(code, error.to_string())
}

#[cfg(unix)]
fn platform_io_error_code(error: &std::io::Error) -> Option<&'static str> {
    match error.raw_os_error()? {
        libc::EPERM => Some("EPERM"),
        libc::ENOENT => Some("ENOENT"),
        libc::EACCES => Some("EACCES"),
        libc::EBUSY => Some("EBUSY"),
        libc::EEXIST => Some("EEXIST"),
        libc::ENOTDIR => Some("ENOTDIR"),
        libc::EISDIR => Some("EISDIR"),
        libc::EMFILE => Some("EMFILE"),
        libc::ENFILE => Some("ENFILE"),
        libc::ENOTEMPTY => Some("ENOTEMPTY"),
        _ => None,
    }
}

#[cfg(windows)]
fn platform_io_error_code(error: &std::io::Error) -> Option<&'static str> {
    match error.raw_os_error()? {
        2 | 3 => Some("ENOENT"),
        4 => Some("EMFILE"),
        5 => Some("EACCES"),
        32 | 33 => Some("EBUSY"),
        80 | 183 => Some("EEXIST"),
        145 => Some("ENOTEMPTY"),
        267 => Some("ENOTDIR"),
        _ => None,
    }
}

#[cfg(not(any(unix, windows)))]
fn platform_io_error_code(_error: &std::io::Error) -> Option<&'static str> {
    None
}
