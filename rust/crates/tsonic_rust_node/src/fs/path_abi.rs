fn with_buffer_path<Result>(
    path: &Buffer,
    operation: impl FnOnce(&std::path::Path) -> NodeResult<Result>,
) -> NodeResult<Result> {
    path.with_bytes(|bytes| {
        if bytes.contains(&0) {
            return Err(NodeError::new(
                "ERR_INVALID_ARG_VALUE",
                "path must not contain null bytes",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            operation(std::path::Path::new(std::ffi::OsStr::from_bytes(bytes)))
        }
        #[cfg(not(unix))]
        {
            let text = String::from_utf8_lossy(bytes);
            operation(std::path::Path::new(text.as_ref()))
        }
    })
}

fn validate_number_stat_options(options: StatOptions) -> NodeResult<()> {
    if options.bigint == Some(true) {
        return Err(NodeError::new(
            "ERR_INVALID_ARG_VALUE",
            "BigInt stats require a BigInt result carrier",
        ));
    }
    Ok(())
}

#[derive(Clone, Default)]
pub struct BufferEncodingOptions {
    pub encoding: String,
}

#[derive(Clone, Copy, Default)]
pub struct RealpathSync;

pub fn realpath_function() -> RealpathSync {
    RealpathSync
}

fn realpath_bytes(path: &std::path::Path, options: BufferEncodingOptions) -> NodeResult<Buffer> {
    if options.encoding != "buffer" {
        return Err(NodeError::new(
            "ERR_INVALID_ARG_VALUE",
            "expected buffer path encoding",
        ));
    }
    let resolved = fs::canonicalize(path).map_err(map_io_error)?;
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(Buffer::from_bytes(resolved.into_os_string().into_vec()))
    }
    #[cfg(not(unix))]
    {
        Ok(Buffer::from_bytes(
            resolved.to_string_lossy().as_bytes().to_vec(),
        ))
    }
}

pub fn realpath_sync_buffer_path(path: &Buffer) -> NodeResult<String> {
    with_buffer_path(path, |path| {
        fs::canonicalize(path)
            .map(|path| path.to_string_lossy().into_owned())
            .map_err(map_io_error)
    })
}

pub fn realpath_sync_bytes(path: &str, options: BufferEncodingOptions) -> NodeResult<Buffer> {
    realpath_bytes(std::path::Path::new(path), options)
}

pub fn realpath_sync_buffer_path_bytes(
    path: &Buffer,
    options: BufferEncodingOptions,
) -> NodeResult<Buffer> {
    with_buffer_path(path, |path| realpath_bytes(path, options))
}

pub fn stat_sync_buffer(path: &Buffer) -> NodeResult<Stats> {
    with_buffer_path(path, |path| stat_sync(path))
}

pub fn stat_sync_buffer_with_options(
    path: &Buffer,
    options: StatOptions,
) -> NodeResult<Option<Stats>> {
    with_buffer_path(path, |path| stat_sync_with_options(path, options))
}

pub fn lstat_sync_buffer(path: &Buffer) -> NodeResult<Stats> {
    with_buffer_path(path, |path| lstat_sync(path))
}

pub fn lstat_sync_buffer_with_options(
    path: &Buffer,
    options: StatOptions,
) -> NodeResult<Option<Stats>> {
    with_buffer_path(path, |path| lstat_sync_with_options(path, options))
}

pub fn mkdir_sync_buffer(path: &Buffer) -> NodeResult<()> {
    with_buffer_path(path, |path| mkdir_sync(path))
}

pub fn mkdir_sync_buffer_with_options(
    path: &Buffer,
    options: MakeDirectoryOptions,
) -> NodeResult<()> {
    with_buffer_path(path, |path| mkdir_sync_with_options(path, options))
}

pub fn rm_sync_buffer(path: &Buffer) -> NodeResult<()> {
    with_buffer_path(path, |path| rm_sync(path))
}

pub fn rm_sync_buffer_with_options(path: &Buffer, options: RmOptions) -> NodeResult<()> {
    with_buffer_path(path, |path| rm_sync_with_options(path, options))
}

pub fn rmdir_sync_buffer(path: &Buffer) -> NodeResult<()> {
    with_buffer_path(path, |path| rmdir_sync(path))
}

pub fn utimes_sync_buffer(path: &Buffer, atime: f64, mtime: f64) -> NodeResult<()> {
    with_buffer_path(path, |path| utimes_sync(path, atime, mtime))
}

fn read_directory_entries<Name>(
    path: &std::path::Path,
    encode_name: impl Fn(&std::ffi::OsStr) -> Name,
) -> NodeResult<Vec<Dirent<Name>>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(path).map_err(map_io_error)? {
        let entry = entry.map_err(map_io_error)?;
        let kind = entry.file_type().map_err(map_io_error)?;
        #[cfg(unix)]
        let special = {
            use std::os::unix::fs::FileTypeExt;
            (
                kind.is_block_device(),
                kind.is_char_device(),
                kind.is_fifo(),
                kind.is_socket(),
            )
        };
        #[cfg(not(unix))]
        let special = (false, false, false, false);
        entries.push(Dirent {
            name: encode_name(&entry.file_name()),
            parent_path: path.to_string_lossy().into_owned(),
            is_file: kind.is_file(),
            is_directory: kind.is_dir(),
            is_symbolic_link: kind.is_symlink(),
            is_block_device: special.0,
            is_character_device: special.1,
            is_fifo: special.2,
            is_socket: special.3,
        });
    }
    Ok(entries)
}

pub fn readdir_sync_buffer_entries(
    path: impl AsRef<std::path::Path>,
    options: BufferDirectoryOptions,
) -> NodeResult<JsArray<Dirent<Buffer>>> {
    if !options.with_file_types || options.encoding != "buffer" {
        return Err(NodeError::new(
            "ERR_INVALID_ARG_VALUE",
            "Buffer directory entries require withFileTypes:true and encoding:buffer",
        ));
    }
    let entries = read_directory_entries(path.as_ref(), |name| {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Buffer::from_bytes(name.as_bytes().to_vec())
        }
        #[cfg(not(unix))]
        {
            Buffer::from_bytes(name.to_string_lossy().as_bytes().to_vec())
        }
    })?;
    Ok(JsArray::from_dense(entries))
}

pub fn readdir_sync_buffer_path_entries(
    path: &Buffer,
    options: BufferDirectoryOptions,
) -> NodeResult<JsArray<Dirent<Buffer>>> {
    with_buffer_path(path, |path| readdir_sync_buffer_entries(path, options))
}

pub fn readdir_sync_buffer_path(path: &Buffer) -> NodeResult<JsArray<String>> {
    with_buffer_path(path, |path| {
        let mut entries = read_directory_entries(path, |name| name.to_string_lossy().into_owned())?;
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(JsArray::from_dense(
            entries.into_iter().map(|entry| entry.name).collect(),
        ))
    })
}
