fn checked_descriptor(value: f64) -> NodeResult<i32> {
    require_non_negative_integer(value, "fd", i32::MAX as u64).map(|value| value as i32)
}

pub trait NativeFilePosition {
    fn file_position(self) -> NodeResult<Option<u64>>;
}

impl NativeFilePosition for f64 {
    #[inline]
    fn file_position(self) -> NodeResult<Option<u64>> {
        if self == -1.0 { return Ok(None); }
        require_non_negative_integer(self, "position", u64::MAX).map(Some)
    }
}

impl NativeFilePosition for i64 {
    #[inline]
    fn file_position(self) -> NodeResult<Option<u64>> {
        if self == -1 { return Ok(None); }
        u64::try_from(self).map(Some)
            .map_err(|_| NodeError::new("ERR_OUT_OF_RANGE", "position must be non-negative"))
    }
}

impl NativeFilePosition for u64 {
    #[inline]
    fn file_position(self) -> NodeResult<Option<u64>> { Ok(Some(self)) }
}

fn checked_file_position<Position: NativeFilePosition>(value: Option<Position>) -> NodeResult<Option<u64>> {
    match value {
        Some(value) => value.file_position(),
        None => Ok(None),
    }
}

fn checked_descriptor_range(
    offset: f64,
    length: f64,
    available: usize,
) -> NodeResult<std::ops::Range<usize>> {
    let offset = require_non_negative_integer(offset, "offset", available as u64)? as usize;
    let length = require_non_negative_integer(length, "length", available as u64)? as usize;
    descriptor_buffer_range(offset, length, available)
}

pub fn open_sync_number(path: &str, flags: &str) -> NodeResult<f64> {
    open_sync(path, flags).map(f64::from)
}

pub fn close_sync_number(fd: f64) -> NodeResult<()> {
    close_sync(checked_descriptor(fd)?)
}

pub fn read_sync_buffer_number<Position: NativeFilePosition>(
    fd: f64,
    buffer: &Buffer,
    offset: f64,
    length: f64,
    position: Option<Position>,
) -> NodeResult<f64> {
    let fd = checked_descriptor(fd)?;
    let position = checked_file_position(position)?;
    buffer.with_mut_bytes(|bytes| {
        let range = checked_descriptor_range(offset, length, bytes.len())?;
        read_descriptor_bytes(fd, &mut bytes[range], position).map(|count| count as f64)
    })
}

pub fn write_sync_buffer_number<Position: NativeFilePosition>(
    fd: f64,
    buffer: &Buffer,
    offset: f64,
    length: f64,
    position: Option<Position>,
) -> NodeResult<f64> {
    let fd = checked_descriptor(fd)?;
    let position = checked_file_position(position)?;
    buffer.with_bytes(|bytes| {
        let range = checked_descriptor_range(offset, length, bytes.len())?;
        write_descriptor_bytes(fd, &bytes[range], position).map(|count| count as f64)
    })
}

pub fn read_sync_uint8_number<Position: NativeFilePosition>(
    fd: f64,
    buffer: &tsonic_rust_js::Uint8Array,
    offset: f64,
    length: f64,
    position: Option<Position>,
) -> NodeResult<f64> {
    let fd = checked_descriptor(fd)?;
    let position = checked_file_position(position)?;
    buffer.with_mut_bytes(|bytes| {
        let range = checked_descriptor_range(offset, length, bytes.len())?;
        read_descriptor_bytes(fd, &mut bytes[range], position).map(|count| count as f64)
    })
}

pub fn write_sync_uint8_number<Position: NativeFilePosition>(
    fd: f64,
    buffer: &tsonic_rust_js::Uint8Array,
    offset: f64,
    length: f64,
    position: Option<Position>,
) -> NodeResult<f64> {
    let fd = checked_descriptor(fd)?;
    let position = checked_file_position(position)?;
    buffer.with_bytes(|bytes| {
        let range = checked_descriptor_range(offset, length, bytes.len())?;
        write_descriptor_bytes(fd, &bytes[range], position).map(|count| count as f64)
    })
}

pub fn open_sync_numeric(path: &str, flags: f64, mode: f64) -> NodeResult<f64> {
    open_numeric_path(std::path::Path::new(path), flags, mode).map(f64::from)
}

pub fn open_sync_buffer_numeric(path: &Buffer, flags: f64, mode: f64) -> NodeResult<f64> {
    with_buffer_path(path, |path| {
        open_numeric_path(path, flags, mode).map(f64::from)
    })
}

fn open_numeric_path(path: &std::path::Path, flags: f64, mode: f64) -> NodeResult<i32> {
    if !flags.is_finite()
        || flags.fract() != 0.0
        || flags < i32::MIN as f64
        || flags > i32::MAX as f64
    {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            "flags must be a signed 32-bit integer",
        ));
    }
    let flags = flags as i32;
    let mode = require_non_negative_integer(mode, "mode", u32::MAX as u64)? as u32;
    let constants = constants();
    let access = flags & 3;
    if access == 3 {
        return Err(NodeError::new("EINVAL", "invalid file access mode"));
    }
    let mut options = OpenOptions::new();
    options
        .read(access == constants.o_rdonly || access == constants.o_rdwr)
        .write(access == constants.o_wronly || access == constants.o_rdwr);
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::OpenOptionsExt;
        if path.as_os_str().as_bytes().contains(&0) {
            return Err(NodeError::new(
                "ERR_INVALID_ARG_VALUE",
                "path must not contain null bytes",
            ));
        }
        options.custom_flags(flags).mode(mode);
    }
    #[cfg(not(unix))]
    {
        let supported =
            3 | constants.o_creat | constants.o_excl | constants.o_trunc | constants.o_append;
        if flags & !supported != 0 {
            return Err(NodeError::new(
                "ERR_INVALID_ARG_VALUE",
                "unsupported native open flags",
            ));
        }
        options
            .create(flags & constants.o_creat != 0)
            .create_new(flags & constants.o_creat != 0 && flags & constants.o_excl != 0)
            .truncate(flags & constants.o_trunc != 0)
            .append(flags & constants.o_append != 0);
        let _ = mode;
    }
    options
        .open(path)
        .map(register_open_file)
        .map_err(map_io_error)
}
