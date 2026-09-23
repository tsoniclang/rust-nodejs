use std::io::Write;
use std::process::{Command, Stdio};
use tsonic_rust_js::Uint8Array;
use tsonic_rust_node::{buffer::Buffer, fs};

#[test]
fn native_file_positions_preserve_all_64_bits_without_float_transport() {
    use fs::NativeFilePosition;
    assert_eq!(9_007_199_254_740_993_i64.file_position().unwrap(), Some(9_007_199_254_740_993));
    assert_eq!(u64::MAX.file_position().unwrap(), Some(u64::MAX));
    assert_eq!(i64::MAX.file_position().unwrap(), Some(i64::MAX as u64));
    assert_eq!((-1_i64).file_position().unwrap(), None);
    assert!((-2_i64).file_position().is_err());
    assert!((u64::MAX as f64).file_position().is_err());
    assert!(f64::NAN.file_position().is_err());
}

#[test]
fn compiler_descriptor_views_positions_and_validation() {
    if std::env::var_os("TSONIC_DESCRIPTOR_CHILD").is_none() {
        let result = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "fs_descriptor_tests::compiler_descriptor_views_positions_and_validation",
            ])
            .env("TSONIC_DESCRIPTOR_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    let root = std::env::current_dir().unwrap().join(".temp");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join(format!("compiler-fd-{}", std::process::id()));
    let path_text = path.to_str().unwrap();
    let constants = fs::constants();
    let fd = fs::open_sync_numeric(
        path_text,
        f64::from(constants.o_rdwr | constants.o_creat | constants.o_excl),
        0o600 as f64,
    )
    .unwrap();
    let bytes = Uint8Array::from_vec(vec![19.0, 97.0, 98.0, 99.0, 100.0, 23.0]).unwrap();
    let view = bytes.subarray(1.0, Some(5.0));
    assert_eq!(
        fs::write_sync_uint8_number(fd, &view, 0.0, 4.0, None::<f64>).unwrap(),
        4.0
    );
    let reader = fs::open_sync_number(path_text, "r").unwrap();
    let copied = Buffer::alloc(4);
    assert_eq!(
        fs::read_sync_buffer_number(reader, &copied, 0.0, 4.0, None::<f64>).unwrap(),
        4.0
    );
    assert_eq!(copied.as_bytes(), b"abcd");
    assert_eq!(fs::read_sync_buffer_number(reader, &copied, 0.0, 1.0, Some(9_007_199_254_740_993_i64)).unwrap(), 0.0);
    assert_eq!(fs::read_sync_buffer_number(reader, &copied, 0.0, 1.0, Some(0_i64)).unwrap(), 1.0);
    fs::close_sync_number(reader).unwrap();
    assert_eq!(
        fs::open_sync_number(path_text, "invalid").unwrap_err().code,
        "ERR_INVALID_ARG_VALUE"
    );
    let target = Uint8Array::from_vec(vec![17.0; 6]).unwrap();
    let target_view = target.subarray(1.0, Some(5.0));
    assert_eq!(
        fs::read_sync_uint8_number(fd, &target_view, 1.0, 2.0, Some(1.0)).unwrap(),
        2.0
    );
    target.with_bytes(|bytes| assert_eq!(bytes, &[17, 17, 98, 99, 17, 17]));
    assert_eq!(
        fs::read_sync_uint8_number(fd, &target_view, 0.0, 1.0, None::<f64>).unwrap(),
        0.0
    );
    let replacement = Buffer::from_bytes(vec![90]);
    assert_eq!(
        fs::write_sync_buffer_number(fd, &replacement, 0.0, 1.0, Some(0.0)).unwrap(),
        1.0
    );
    assert_eq!(
        fs::read_sync_uint8_number(fd, &target_view, 0.0, 1.0, None::<f64>).unwrap(),
        0.0
    );
    let result = Buffer::alloc(4);
    assert_eq!(
        fs::read_sync_buffer_number(fd, &result, 0.0, 1.0, Some(-1.0)).unwrap(),
        0.0
    );
    assert_eq!(
        fs::read_sync_buffer_number(fd, &result, 0.0, 4.0, Some(0.0)).unwrap(),
        4.0
    );
    assert_eq!(result.as_bytes(), b"Zbcd");
    for invalid in [f64::NAN, f64::INFINITY, -1.0, 0.5, 5.0] {
        assert_eq!(
            fs::read_sync_uint8_number(fd, &target_view, invalid, 1.0, None::<f64>)
                .unwrap_err()
                .code,
            "ERR_OUT_OF_RANGE"
        );
        assert_eq!(
            fs::write_sync_buffer_number(fd, &result, 0.0, invalid, None::<f64>)
                .unwrap_err()
                .code,
            "ERR_OUT_OF_RANGE"
        );
    }
    assert_eq!(
        fs::read_sync_uint8_number(fd, &target_view, 3.0, 2.0, Some(0.0))
            .unwrap_err()
            .code,
        "ERR_OUT_OF_RANGE"
    );
    assert_eq!(
        fs::write_sync_buffer_number(fd, &result, 3.0, 2.0, Some(0.0))
            .unwrap_err()
            .code,
        "ERR_OUT_OF_RANGE"
    );
    assert_eq!(
        fs::read_sync_uint8_number(fd, &target_view, 0.0, 1.0, Some(18_446_744_073_709_551_616.0))
            .unwrap_err()
            .code,
        "ERR_OUT_OF_RANGE"
    );
    for invalid in [f64::NAN, f64::INFINITY, -2.0, 0.5] {
        assert_eq!(
            fs::read_sync_buffer_number(fd, &result, 0.0, 1.0, Some(invalid))
                .unwrap_err()
                .code,
            "ERR_OUT_OF_RANGE"
        );
    }
    assert_eq!(
        fs::close_sync_number(0.5).unwrap_err().code,
        "ERR_OUT_OF_RANGE"
    );
    fs::close_sync_number(fd).unwrap();
    assert_eq!(
        fs::read_sync_uint8_number(fd, &target_view, 0.0, 1.0, None::<f64>)
            .unwrap_err()
            .code,
        "EBADF"
    );
    assert_eq!(fs::close_sync_number(fd).unwrap_err().code, "EBADF");
    target.with_bytes(|bytes| assert_eq!(bytes, &[17, 17, 98, 99, 17, 17]));
    assert_eq!(std::fs::read(&path).unwrap(), b"Zbcd");
    std::fs::remove_file(path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let name = format!("compiler-raw-fd-{}-", std::process::id());
        let raw = root.join(std::ffi::OsStr::from_bytes(
            &[name.as_bytes(), &[0xff]].concat(),
        ));
        let raw_buffer = Buffer::from_bytes(raw.as_os_str().as_bytes().to_vec());
        let fd = fs::open_sync_buffer_numeric(
            &raw_buffer,
            f64::from(constants.o_wronly | constants.o_creat | constants.o_excl),
            0o600 as f64,
        )
        .unwrap();
        assert!(unsafe { libc::fcntl(fd as i32, libc::F_GETFD) } >= 0);
        fs::write_sync_buffer_number(fd, &replacement, 0.0, 1.0, None::<f64>).unwrap();
        fs::close_sync_number(fd).unwrap();
        assert_eq!(std::fs::read(&raw).unwrap(), b"Z");
        std::fs::remove_file(raw).unwrap();
        let invalid = Buffer::from_bytes(vec![0]);
        assert_eq!(
            fs::open_sync_buffer_numeric(&invalid, 0.0, 0o600 as f64)
                .unwrap_err()
                .code,
            "ERR_INVALID_ARG_VALUE"
        );
    }
}

#[test]
fn compiler_standard_descriptors_execute_native_io() {
    if std::env::var_os("TSONIC_STDIO_CHILD").is_some() {
        let bytes = Uint8Array::new(16.0).unwrap();
        assert_eq!(
            fs::read_sync_uint8_number(0.0, &bytes, 0.0, 16.0, None::<f64>).unwrap(),
            16.0
        );
        assert_eq!(
            fs::write_sync_uint8_number(1.0, &bytes, 0.0, 16.0, None::<f64>).unwrap(),
            16.0
        );
        assert_eq!(
            fs::write_sync_uint8_number(2.0, &bytes, 0.0, 16.0, None::<f64>).unwrap(),
            16.0
        );
        return;
    }
    let message = b"native-fd-proof\n";
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "fs_descriptor_tests::compiler_standard_descriptors_execute_native_io",
        ])
        .env("TSONIC_STDIO_CHILD", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(message).unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result
        .stdout
        .windows(message.len())
        .any(|window| window == message));
    assert_eq!(result.stderr, message);
}
