use tsonic_rust_node::{buffer::Buffer, fs};

#[test]
fn compiler_paths_preserve_names_kinds_options_and_metadata() {
    let scratch = std::env::current_dir().unwrap().join(".temp");
    std::fs::create_dir_all(&scratch).unwrap();
    let root = scratch.join(format!("compiler-paths-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let path_buffer = |path: &std::path::Path| {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Buffer::from_bytes(path.as_os_str().as_bytes().to_vec())
        }
        #[cfg(not(unix))]
        Buffer::from_bytes(path.to_str().unwrap().as_bytes().to_vec())
    };
    let root_buffer = path_buffer(&root);
    let directory = root.join("nested");
    let directory_buffer = path_buffer(&directory);
    fs::mkdir_sync_buffer_with_options(
        &directory_buffer,
        fs::MakeDirectoryOptions {
            recursive: Some(true),
            mode: Some(0o700 as f64),
        },
    )
    .unwrap();
    let absent = path_buffer(&root.join("absent"));
    let options = fs::StatOptions {
        throw_if_no_entry: Some(false),
        ..Default::default()
    };
    assert!(fs::stat_sync_buffer_with_options(&absent, options)
        .unwrap()
        .is_none());
    assert!(fs::lstat_sync_buffer_with_options(&absent, options)
        .unwrap()
        .is_none());
    assert_eq!(
        fs::stat_sync_buffer_with_options(&absent, Default::default())
            .unwrap_err()
            .code,
        "ENOENT"
    );
    assert!(fs::stat_sync_buffer(&directory_buffer)
        .unwrap()
        .is_directory());
    let file_name = {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            std::ffi::OsString::from_vec(vec![b'f', 0xff])
        }
        #[cfg(not(unix))]
        std::ffi::OsString::from("file")
    };
    let file = root.join(&file_name);
    std::fs::write(&file, b"bytes").unwrap();
    let file_buffer = path_buffer(&file);
    fs::utimes_sync_buffer(&file_buffer, 1_700_000_000.25, 1_700_000_001.5).unwrap();
    let stats = fs::stat_sync_buffer_with_options(&file_buffer, options)
        .unwrap()
        .unwrap();
    assert!(stats.is_file());
    assert_eq!(stats.size, 5);
    assert_eq!(stats.mtime_ms(), 1_700_000_001_500.0);
    assert_eq!(stats.mode_number(), f64::from(stats.mode));
    assert_eq!(
        fs::stat_sync_buffer_with_options(
            &file_buffer,
            fs::StatOptions {
                bigint: Some(true),
                ..Default::default()
            }
        )
        .unwrap_err()
        .code,
        "ERR_INVALID_ARG_VALUE"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(&file_name, root.join("link")).unwrap();
        let link = path_buffer(&root.join("link"));
        assert!(fs::lstat_sync_buffer(&link).unwrap().is_symbolic_link());
        assert!(fs::stat_sync_buffer(&link).unwrap().is_file());
        let socket = std::os::unix::net::UnixListener::bind(root.join("socket")).unwrap();
        let entries = fs::readdir_sync_buffer_path_entries(
            &root_buffer,
            fs::BufferDirectoryOptions {
                with_file_types: true,
                encoding: "buffer".into(),
            },
        )
        .unwrap();
        assert_eq!(entries.len(), 4);
        let entries: Vec<_> = entries.values().into_iter().map(Option::unwrap).collect();
        assert!(entries
            .iter()
            .any(|entry| entry.name.as_bytes() == [b'f', 0xff] && entry.is_file()));
        assert!(entries
            .iter()
            .any(|entry| entry.name.as_bytes() == b"nested" && entry.is_directory()));
        assert!(entries
            .iter()
            .any(|entry| entry.name.as_bytes() == b"link" && entry.is_symbolic_link()));
        assert!(entries
            .iter()
            .any(|entry| entry.name.as_bytes() == b"socket" && entry.is_socket()));
        drop(socket);
    }
    assert_eq!(
        fs::readdir_sync_buffer_path_entries(&root_buffer, Default::default())
            .unwrap_err()
            .code,
        "ERR_INVALID_ARG_VALUE"
    );
    assert_eq!(
        fs::stat_sync_buffer(&Buffer::from_bytes(vec![0]))
            .unwrap_err()
            .code,
        "ERR_INVALID_ARG_VALUE"
    );
    fs::rmdir_sync_buffer(&directory_buffer).unwrap();
    fs::rm_sync_buffer(&file_buffer).unwrap();
    fs::rm_sync_buffer_with_options(
        &root_buffer,
        fs::RmOptions {
            recursive: Some(true),
            force: Some(true),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!root.exists());
}
