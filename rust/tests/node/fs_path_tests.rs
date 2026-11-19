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
    assert_eq!(std::mem::size_of_val(&fs::realpath_function()), 0);
    fs::mkdir_sync_buffer(&directory_buffer).unwrap();
    assert!(directory.is_dir());
    assert_eq!(
        fs::mkdir_sync_buffer(&directory_buffer).unwrap_err().code,
        "EEXIST"
    );
    assert_eq!(
        fs::readdir_sync_buffer_path(&root_buffer).unwrap().values(),
        vec!["nested".to_owned()]
    );
    assert_eq!(
        fs::readdir_sync_buffer_path(&Buffer::from_bytes(vec![0]))
            .unwrap_err()
            .code,
        "ERR_INVALID_ARG_VALUE"
    );
    fs::mkdir_sync_buffer_with_options(
        &directory_buffer,
        fs::MakeDirectoryOptions {
            recursive: Some(true),
            mode: Some(0o700),
        },
    )
    .unwrap();
    let absent = path_buffer(&root.join("absent"));
    assert_eq!(
        fs::readdir_sync_buffer_path(&absent).unwrap_err().code,
        "ENOENT"
    );
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
    let encoding = fs::BufferEncodingOptions {
        encoding: "buffer".into(),
    };
    let resolved = fs::realpath_sync_buffer_path_bytes(&file_buffer, encoding.clone()).unwrap();
    assert_eq!(
        resolved.as_bytes(),
        path_buffer(&std::fs::canonicalize(&file).unwrap()).as_bytes()
    );
    assert_eq!(
        fs::realpath_sync_buffer_path(&root_buffer).unwrap(),
        fs::realpath_sync(root.to_str().unwrap()).unwrap()
    );
    assert_eq!(
        fs::realpath_sync_bytes(root.to_str().unwrap(), encoding.clone())
            .unwrap()
            .as_bytes(),
        path_buffer(&std::fs::canonicalize(&root).unwrap()).as_bytes()
    );
    assert!(fs::realpath_sync_buffer_path_bytes(&absent, encoding.clone()).is_err());
    assert!(
        fs::realpath_sync_buffer_path_bytes(&Buffer::from_bytes(vec![0]), encoding.clone())
            .is_err()
    );
    assert!(fs::realpath_sync_buffer_path_bytes(&file_buffer, Default::default()).is_err());
    fs::utimes_sync_buffer(&file_buffer, 1_700_000_000.25, 1_700_000_001.5).unwrap();
    let stats = fs::stat_sync_buffer_with_options(&file_buffer, options)
        .unwrap()
        .unwrap();
    assert!(stats.is_file());
    assert_eq!(stats.size, 5);
    assert_eq!(stats.mtime_ms(), 1_700_000_001_500.0);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(stats.mode, std::fs::metadata(&file).unwrap().mode());
    }
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
        let relative_root = root.strip_prefix(std::env::current_dir().unwrap()).unwrap();
        let socket = std::os::unix::net::UnixListener::bind(relative_root.join("socket")).unwrap();
        let entries = fs::readdir_sync_buffer_path_entries(
            &root_buffer,
            fs::BufferDirectoryOptions {
                with_file_types: true,
                encoding: "buffer".into(),
            },
        )
        .unwrap();
        assert_eq!(entries.len(), 4);
        let resolved_link =
            fs::realpath_sync_buffer_path_bytes(&path_buffer(&root.join("link")), encoding.clone())
                .unwrap();
        assert_eq!(resolved_link.as_bytes(), resolved.as_bytes());
        let entries = entries.values();
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
