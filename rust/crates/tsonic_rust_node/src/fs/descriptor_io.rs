#[cfg(unix)]
fn register_open_file(file: File) -> i32 {
    use std::os::fd::AsRawFd;
    let descriptor = file.as_raw_fd();
    crate::sync::lock(file_table()).insert(descriptor, file);
    descriptor
}

#[cfg(not(unix))]
fn register_open_file(file: File) -> i32 {
    let descriptor = NEXT_FD.fetch_add(1, Ordering::SeqCst);
    crate::sync::lock(file_table()).insert(descriptor, file);
    descriptor
}

fn descriptor_buffer_range(
    offset: usize,
    length: usize,
    available: usize,
) -> NodeResult<std::ops::Range<usize>> {
    let end = offset
        .checked_add(length)
        .filter(|end| *end <= available)
        .ok_or_else(|| NodeError::new("ERR_OUT_OF_RANGE", "buffer range is out of bounds"))?;
    Ok(offset..end)
}

#[cfg(unix)]
fn read_descriptor_bytes(fd: i32, bytes: &mut [u8], position: Option<u64>) -> NodeResult<usize> {
    let position = position
        .map(libc::off_t::try_from)
        .transpose()
        .map_err(|_| NodeError::new("ERR_OUT_OF_RANGE", "file position is out of range"))?;
    loop {
        let count = unsafe {
            match position {
                Some(position) => libc::pread(fd, bytes.as_mut_ptr().cast(), bytes.len(), position),
                None => libc::read(fd, bytes.as_mut_ptr().cast(), bytes.len()),
            }
        };
        if count >= 0 {
            return Ok(count as usize);
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(map_io_error(error));
        }
    }
}

#[cfg(unix)]
fn write_descriptor_bytes(fd: i32, bytes: &[u8], position: Option<u64>) -> NodeResult<usize> {
    let position = position
        .map(libc::off_t::try_from)
        .transpose()
        .map_err(|_| NodeError::new("ERR_OUT_OF_RANGE", "file position is out of range"))?;
    loop {
        let count = unsafe {
            match position {
                Some(position) => libc::pwrite(fd, bytes.as_ptr().cast(), bytes.len(), position),
                None => libc::write(fd, bytes.as_ptr().cast(), bytes.len()),
            }
        };
        if count >= 0 {
            return Ok(count as usize);
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(map_io_error(error));
        }
    }
}

#[cfg(not(unix))]
fn with_positioned_file<Result>(
    fd: i32,
    position: Option<u64>,
    operation: impl FnOnce(&mut File) -> std::io::Result<Result>,
) -> NodeResult<Result> {
    use std::io::{Seek, SeekFrom};
    let mut table = crate::sync::lock(file_table());
    let file = table
        .get_mut(&fd)
        .ok_or_else(|| NodeError::new("EBADF", "bad file descriptor"))?;
    let original = position
        .map(|_| file.stream_position())
        .transpose()
        .map_err(map_io_error)?;
    if let Some(position) = position {
        file.seek(SeekFrom::Start(position)).map_err(map_io_error)?;
    }
    let result = operation(file);
    if let Some(original) = original {
        file.seek(SeekFrom::Start(original)).map_err(map_io_error)?;
    }
    result.map_err(map_io_error)
}

#[cfg(not(unix))]
fn read_descriptor_bytes(fd: i32, bytes: &mut [u8], position: Option<u64>) -> NodeResult<usize> {
    if fd == 0 && position.is_none() {
        return std::io::stdin().read(bytes).map_err(map_io_error);
    }
    with_positioned_file(fd, position, |file| file.read(bytes))
}

#[cfg(not(unix))]
fn write_descriptor_bytes(fd: i32, bytes: &[u8], position: Option<u64>) -> NodeResult<usize> {
    if position.is_none() {
        match fd {
            1 => return std::io::stdout().write(bytes).map_err(map_io_error),
            2 => return std::io::stderr().write(bytes).map_err(map_io_error),
            _ => {}
        }
    }
    with_positioned_file(fd, position, |file| file.write(bytes))
}
