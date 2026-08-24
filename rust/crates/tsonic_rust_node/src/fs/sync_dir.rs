pub fn mkdir_sync(path: &str) -> NodeResult<()> {
    fs::create_dir(path).map_err(map_io_error)
}

pub fn mkdir_sync_with_options(path: &str, options: MakeDirectoryOptions) -> NodeResult<()> {
    let mode = options
        .mode
        .map(|value| require_non_negative_integer(value, "mode", 0o7777))
        .transpose()?;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(options.recursive.unwrap_or(false));
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(mode as u32);
    }
    #[cfg(not(unix))]
    let _ = mode;
    builder.create(path).map_err(map_io_error)
}

pub fn rm_sync(path: &str) -> NodeResult<()> {
    remove_path(path, false, false)
}

fn remove_path(path: &str, recursive: bool, force: bool) -> NodeResult<()> {
    let path_ref = std::path::Path::new(path);
    let metadata = match fs::symlink_metadata(path_ref) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && force => return Ok(()),
        Err(error) => return Err(map_io_error(error)),
    };
    if metadata.file_type().is_dir() {
        if !recursive {
            return Err(NodeError::new(
                "EISDIR",
                "path is a directory and recursive removal was not requested",
            ));
        }
        fs::remove_dir_all(path_ref).map_err(map_io_error)
    } else {
        fs::remove_file(path_ref).map_err(map_io_error)
    }
}

pub fn rm_sync_with_options(path: &str, options: RmOptions) -> NodeResult<()> {
    let recursive = options.recursive.unwrap_or(false);
    let force = options.force.unwrap_or(false);
    let configured_max_retries = require_non_negative_integer(
        options.max_retries.unwrap_or(0.0),
        "maxRetries",
        u32::MAX as u64,
    )? as u32;
    let max_retries = if recursive { configured_max_retries } else { 0 };
    let retry_delay = require_non_negative_integer(
        options.retry_delay_ms.unwrap_or(100.0),
        "retryDelay",
        u32::MAX as u64,
    )?;
    let mut attempts = 0_u32;
    loop {
        match remove_path(path, recursive, force) {
            Ok(()) => return Ok(()),
            Err(error) if attempts < max_retries && rm_error_is_retryable(&error) => {
                attempts += 1;
                let delay = retry_delay.checked_mul(u64::from(attempts)).ok_or_else(|| {
                    NodeError::new(
                        "ERR_OUT_OF_RANGE",
                        "retryDelay multiplied by the retry count exceeds the supported range",
                    )
                })?;
                if delay > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                }
            }
            Err(error) => return Err(error),
        }
    }
}

fn require_non_negative_integer(value: f64, name: &str, maximum: u64) -> NodeResult<u64> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > maximum as f64 {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            format!("{name} must be a finite non-negative integer in range"),
        ));
    }
    Ok(value as u64)
}

fn rm_error_is_retryable(error: &NodeError) -> bool {
    let standard = matches!(
        error.code.as_str(),
        "EBUSY" | "EMFILE" | "ENFILE" | "ENOTEMPTY" | "EPERM"
    );
    #[cfg(windows)]
    {
        standard || error.code == "EACCES"
    }
    #[cfg(not(windows))]
    {
        standard
    }
}

pub fn readdir_sync(path: &str) -> NodeResult<JsArray<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(path).map_err(map_io_error)? {
        let entry = entry.map_err(map_io_error)?;
        names.push(entry.file_name().to_string_lossy().to_string());
    }
    names.sort();
    Ok(JsArray::from_dense(names))
}

pub fn unlink_sync(path: &str) -> NodeResult<()> {
    fs::remove_file(path).map_err(map_io_error)
}

pub fn rename_sync(from: &str, to: &str) -> NodeResult<()> {
    fs::rename(from, to).map_err(map_io_error)
}

pub fn copy_file_sync(from: &str, to: &str) -> NodeResult<()> {
    fs::copy(from, to).map(|_| ()).map_err(map_io_error)
}

pub fn copy_file_sync_with_mode(from: &str, to: &str, mode: i32) -> NodeResult<()> {
    if mode & constants().copyfile_excl != 0 && std::path::Path::new(to).exists() {
        return Err(NodeError::new("EEXIST", "destination already exists"));
    }
    copy_file_sync(from, to)
}

pub fn cp_sync(from: &str, to: &str, recursive: bool) -> NodeResult<()> {
    let metadata = fs::metadata(from).map_err(map_io_error)?;
    if metadata.is_dir() {
        if !recursive {
            return Err(NodeError::new("EISDIR", "source is a directory"));
        }
        fs::create_dir_all(to).map_err(map_io_error)?;
        for entry in fs::read_dir(from).map_err(map_io_error)? {
            let entry = entry.map_err(map_io_error)?;
            let child_from = entry.path();
            let child_to = std::path::Path::new(to).join(entry.file_name());
            cp_sync(
                &child_from.to_string_lossy(),
                &child_to.to_string_lossy(),
                true,
            )?;
        }
        Ok(())
    } else {
        copy_file_sync(from, to)
    }
}

pub fn cp_sync_with_options(from: &str, to: &str, options: &CopySyncOptions) -> NodeResult<()> {
    if let Some(filter) = &options.filter {
        if !filter.accepts(from, to) {
            return Ok(());
        }
    }
    if std::path::Path::new(to).exists() && options.base.error_on_exist {
        return Err(NodeError::new("EEXIST", "destination already exists"));
    }
    if std::path::Path::new(to).exists() && !options.base.force {
        return Ok(());
    }
    if options.base.mode != 0 {
        copy_file_sync_with_mode(from, to, options.base.mode)
    } else {
        cp_sync(from, to, options.base.recursive)
    }
}

pub fn copy_sync(from: &str, to: &str, options: &CopyOptions) -> NodeResult<()> {
    cp_sync_with_options(
        from,
        to,
        &CopySyncOptions {
            base: options.base.clone(),
            filter: options.filter.clone(),
        },
    )
}

pub fn link_sync(existing_path: &str, new_path: &str) -> NodeResult<()> {
    fs::hard_link(existing_path, new_path).map_err(map_io_error)
}

pub fn symlink_sync(target: &str, path: &str) -> NodeResult<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, path).map_err(map_io_error)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, path).map_err(map_io_error)
    }
}

pub fn readlink_sync(path: &str) -> NodeResult<String> {
    fs::read_link(path)
        .map(|path| path.to_string_lossy().to_string())
        .map_err(map_io_error)
}

pub fn realpath_sync(path: &str) -> NodeResult<String> {
    fs::canonicalize(path)
        .map(|path| path.to_string_lossy().to_string())
        .map_err(map_io_error)
}

pub fn realpath_native(path: &str) -> NodeResult<String> {
    realpath_sync(path)
}

pub fn realpath_sync_native(path: &str) -> NodeResult<String> {
    realpath_sync(path)
}

pub fn rmdir_sync(path: &str) -> NodeResult<()> {
    fs::remove_dir(path).map_err(map_io_error)
}

pub fn truncate_sync(path: &str, len: u64) -> NodeResult<()> {
    OpenOptions::new()
        .write(true)
        .open(path)
        .and_then(|file| file.set_len(len))
        .map_err(map_io_error)
}

pub fn mkdtemp_sync(prefix: &str) -> NodeResult<String> {
    for index in 0..10_000 {
        let candidate = format!("{prefix}{index:06}");
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(map_io_error(error)),
        }
    }
    Err(NodeError::new(
        "EEXIST",
        "unable to create temporary directory",
    ))
}

pub fn mkdtemp_disposable_sync(prefix: &str) -> NodeResult<DisposableTempDir> {
    mkdtemp_sync(prefix).map(DisposableTempDir::new)
}

pub fn opendir_sync(path: &str) -> NodeResult<Vec<Dirent>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(path).map_err(map_io_error)? {
        let entry = entry.map_err(map_io_error)?;
        let metadata = entry.file_type().map_err(map_io_error)?;
        entries.push(Dirent {
            name: entry.file_name().to_string_lossy().to_string(),
            parent_path: path.to_string(),
            is_file: metadata.is_file(),
            is_directory: metadata.is_dir(),
            is_symbolic_link: metadata.is_symlink(),
            is_block_device: false,
            is_character_device: false,
            is_fifo: false,
            is_socket: false,
        });
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

pub fn opendir_handle_sync(path: &str) -> NodeResult<Dir> {
    Dir::open(path)
}

pub fn open_sync(path: &str, flags: &str) -> NodeResult<i32> {
    let mut options = OpenOptions::new();
    match flags {
        "r" => {
            options.read(true);
        }
        "r+" => {
            options.read(true).write(true);
        }
        "w" => {
            options.write(true).create(true).truncate(true);
        }
        "w+" => {
            options.read(true).write(true).create(true).truncate(true);
        }
        "a" => {
            options.write(true).create(true).append(true);
        }
        "a+" => {
            options.read(true).write(true).create(true).append(true);
        }
        _ => {
            return Err(NodeError::new(
                "ERR_INVALID_ARG_VALUE",
                "unsupported open flag",
            ))
        }
    }
    let file = options.open(path).map_err(map_io_error)?;
    let fd = NEXT_FD.fetch_add(1, Ordering::SeqCst);
    crate::sync::lock(file_table()).insert(fd, file);
    Ok(fd)
}

pub fn close_sync(fd: i32) -> NodeResult<()> {
    crate::sync::lock(file_table())
        .remove(&fd)
        .map(|_| ())
        .ok_or_else(|| NodeError::new("EBADF", "bad file descriptor"))
}
