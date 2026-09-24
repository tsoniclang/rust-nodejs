#[inline]
fn checked_descriptor(
    value: impl tsonic_rust_runtime::conversions::IntegerInput<i32>,
) -> NodeResult<i32> {
    value
        .checked_integer()
        .filter(|value| *value >= 0)
        .ok_or_else(|| {
            NodeError::new(
                "ERR_OUT_OF_RANGE",
                "fd must be a non-negative native descriptor",
            )
        })
}

pub trait NativeFilePosition {
    fn file_position(self) -> NodeResult<Option<u64>>;
}

impl NativeFilePosition for f64 {
    #[inline]
    fn file_position(self) -> NodeResult<Option<u64>> {
        if self == -1.0 {
            return Ok(None);
        }
        tsonic_rust_runtime::conversions::IntegerInput::<u64>::checked_integer(self)
            .map(Some)
            .ok_or_else(|| {
                NodeError::new(
                    "ERR_OUT_OF_RANGE",
                    "position is outside the native file offset range",
                )
            })
    }
}

macro_rules! native_file_positions {
    (unsigned: $($unsigned:ty),*; signed: $($signed:ty),* $(;)?) => {
        $(native_file_positions!(@implementation $unsigned, |_| false);)*
        $(native_file_positions!(@implementation $signed, |value: $signed| value == -1);)*
    };
    (@implementation $native:ty, $is_current:expr) => {
        impl NativeFilePosition for $native {
            #[inline]
            fn file_position(self) -> NodeResult<Option<u64>> {
                if ($is_current)(self) {
                    return Ok(None);
                }
                u64::try_from(self).map(Some)
                    .map_err(|_| NodeError::new("ERR_OUT_OF_RANGE", "position is outside the native file offset range"))
            }
        }
    };
}

native_file_positions!(unsigned: u8, u16, u32, u64, usize, u128; signed: i8, i16, i32, i64, isize, i128);

impl NativeFilePosition for f32 {
    #[inline]
    fn file_position(self) -> NodeResult<Option<u64>> {
        f64::from(self).file_position()
    }
}

fn checked_file_position<Position: NativeFilePosition>(
    value: Option<Position>,
) -> NodeResult<Option<u64>> {
    match value {
        Some(value) => value.file_position(),
        None => Ok(None),
    }
}

fn checked_descriptor_range(
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
    length: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
    available: usize,
) -> NodeResult<std::ops::Range<usize>> {
    let offset = offset.checked_integer().ok_or_else(|| {
        NodeError::new(
            "ERR_OUT_OF_RANGE",
            "offset must be a non-negative native index",
        )
    })?;
    let length = length.checked_integer().ok_or_else(|| {
        NodeError::new(
            "ERR_OUT_OF_RANGE",
            "length must be a non-negative native length",
        )
    })?;
    descriptor_buffer_range(offset, length, available)
}

pub fn close_sync_number(
    fd: impl tsonic_rust_runtime::conversions::IntegerInput<i32>,
) -> NodeResult<()> {
    close_sync(checked_descriptor(fd)?)
}

pub fn read_sync_buffer_number<Position: NativeFilePosition>(
    fd: impl tsonic_rust_runtime::conversions::IntegerInput<i32>,
    buffer: &Buffer,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
    length: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
    position: Option<Position>,
) -> NodeResult<usize> {
    let fd = checked_descriptor(fd)?;
    let position = checked_file_position(position)?;
    buffer.with_mut_bytes(|bytes| {
        let range = checked_descriptor_range(offset, length, bytes.len())?;
        read_descriptor_bytes(fd, &mut bytes[range], position)
    })
}

pub fn write_sync_buffer_number<Position: NativeFilePosition>(
    fd: impl tsonic_rust_runtime::conversions::IntegerInput<i32>,
    buffer: &Buffer,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
    length: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
    position: Option<Position>,
) -> NodeResult<usize> {
    let fd = checked_descriptor(fd)?;
    let position = checked_file_position(position)?;
    buffer.with_bytes(|bytes| {
        let range = checked_descriptor_range(offset, length, bytes.len())?;
        write_descriptor_bytes(fd, &bytes[range], position)
    })
}

pub fn read_sync_uint8_number<Position: NativeFilePosition>(
    fd: impl tsonic_rust_runtime::conversions::IntegerInput<i32>,
    buffer: &tsonic_rust_js::Uint8Array,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
    length: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
    position: Option<Position>,
) -> NodeResult<usize> {
    let fd = checked_descriptor(fd)?;
    let position = checked_file_position(position)?;
    buffer.with_mut_bytes(|bytes| {
        let range = checked_descriptor_range(offset, length, bytes.len())?;
        read_descriptor_bytes(fd, &mut bytes[range], position)
    })
}

pub fn write_sync_uint8_number<Position: NativeFilePosition>(
    fd: impl tsonic_rust_runtime::conversions::IntegerInput<i32>,
    buffer: &tsonic_rust_js::Uint8Array,
    offset: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
    length: impl tsonic_rust_runtime::conversions::IntegerInput<usize>,
    position: Option<Position>,
) -> NodeResult<usize> {
    let fd = checked_descriptor(fd)?;
    let position = checked_file_position(position)?;
    buffer.with_bytes(|bytes| {
        let range = checked_descriptor_range(offset, length, bytes.len())?;
        write_descriptor_bytes(fd, &bytes[range], position)
    })
}

pub fn open_sync_numeric(
    path: &str,
    flags: impl tsonic_rust_runtime::conversions::IntegerInput<i32>,
    mode: impl tsonic_rust_runtime::conversions::IntegerInput<u32>,
) -> NodeResult<i32> {
    open_numeric_path(std::path::Path::new(path), flags, mode)
}

pub fn open_sync_buffer_numeric(
    path: &Buffer,
    flags: impl tsonic_rust_runtime::conversions::IntegerInput<i32>,
    mode: impl tsonic_rust_runtime::conversions::IntegerInput<u32>,
) -> NodeResult<i32> {
    with_buffer_path(path, |path| open_numeric_path(path, flags, mode))
}

fn open_numeric_path(
    path: &std::path::Path,
    flags: impl tsonic_rust_runtime::conversions::IntegerInput<i32>,
    mode: impl tsonic_rust_runtime::conversions::IntegerInput<u32>,
) -> NodeResult<i32> {
    let flags = flags.checked_integer().ok_or_else(|| {
        NodeError::new("ERR_OUT_OF_RANGE", "flags must be a signed 32-bit integer")
    })?;
    let mode = mode.checked_integer().ok_or_else(|| {
        NodeError::new(
            "ERR_OUT_OF_RANGE",
            "mode must be an unsigned 32-bit integer",
        )
    })?;
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
