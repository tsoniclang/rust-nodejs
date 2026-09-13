#![cfg(unix)]

use tsonic_rust_js::{JsArray, JsString, JsValue, Uint8Array};
use tsonic_rust_node::child_process::{spawn_sync_result_with_options, SpawnSyncOptions};
use tsonic_rust_node::process::ProcessEnv;

fn arguments(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn spawn_capture_drains_binary_pipes_concurrently_and_retains_results() {
    let bytes: Vec<u8> = (0..512 * 1024).map(|index| (index % 256) as u8).collect();
    let input = Uint8Array::from_bytes(bytes.clone());
    let result = spawn_sync_result_with_options(
        "/bin/cat",
        &arguments(&[]),
        SpawnSyncOptions {
            input: Some(input),
            max_buffer: Some(bytes.len() as f64),
            timeout: Some(5_000.0),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.status, Some(0));
    assert!(result.pid.unwrap() > 0.0);
    assert!(result.error.is_none());
    assert!(result.signal.is_none());
    assert_eq!(result.stdout.unwrap().as_bytes(), bytes);
    assert!(result.stderr.unwrap().is_empty());

    let result = spawn_sync_result_with_options(
        "/bin/sh",
        &arguments(&["-c", "printf output; printf error >&2; exit 7"]),
        Default::default(),
    )
    .unwrap();
    assert_eq!(result.status, Some(7));
    assert!(result.error.is_none());
    assert_eq!(result.stdout.unwrap().as_bytes(), b"output");
    assert_eq!(result.stderr.unwrap().as_bytes(), b"error");
}

#[test]
fn spawn_capture_bounds_output_and_enforces_timeout_and_signals() {
    let result = spawn_sync_result_with_options(
        "/bin/sh",
        &arguments(&["-c", "while :; do printf abcdefghijklmnopqrstuvwxyz; done"]),
        SpawnSyncOptions {
            max_buffer: Some(64.0),
            timeout: Some(5_000.0),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.error.unwrap().code(), "ENOBUFS");
    assert_eq!(result.stdout.unwrap().len(), 64);
    assert_eq!(result.signal.as_deref(), Some("SIGTERM"));
    assert_eq!(result.status, None);

    let started = std::time::Instant::now();
    let result = spawn_sync_result_with_options(
        "/bin/sleep",
        &arguments(&["5"]),
        SpawnSyncOptions {
            timeout: Some(30.0),
            kill_signal: Some("SIGKILL".to_owned()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.error.unwrap().code(), "ETIMEDOUT");
    assert_eq!(result.signal.as_deref(), Some("SIGKILL"));
    assert_eq!(result.status, None);
    assert!(started.elapsed() < std::time::Duration::from_secs(4));

    let result = spawn_sync_result_with_options(
        "/bin/sh",
        &arguments(&["-c", "kill -TERM $$"]),
        Default::default(),
    )
    .unwrap();
    assert_eq!(result.status, None);
    assert_eq!(result.signal.as_deref(), Some("SIGTERM"));
    assert!(result.error.is_none());
}

#[test]
fn spawn_options_retain_cwd_environment_and_ignored_output() {
    let env = ProcessEnv::default();
    env.set("TSONIC_CHILD_EXACT", Some("only-child".to_owned()))
        .unwrap();
    let result = spawn_sync_result_with_options(
        "/usr/bin/env",
        &arguments(&[]),
        SpawnSyncOptions {
            cwd: Some("/".to_owned()),
            env: Some(env),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        result.stdout.unwrap().to_string_enc("utf8").unwrap(),
        "TSONIC_CHILD_EXACT=only-child\n"
    );
    assert!(std::env::var_os("TSONIC_CHILD_EXACT").is_none());
    let result = spawn_sync_result_with_options(
        "/bin/pwd",
        &arguments(&[]),
        SpawnSyncOptions {
            cwd: Some("/".to_owned()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.stdout.unwrap().as_bytes(), b"/\n");
    let result = spawn_sync_result_with_options(
        "/bin/echo",
        &arguments(&["ignored"]),
        SpawnSyncOptions {
            stdio: Some(JsArray::from_dense(vec![
                JsValue::String(
                    JsString::from_utf8("ignore")
                );
                3
            ])),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.status, Some(0));
    assert!(result.stdout.is_none());
    assert!(result.stderr.is_none());
}

#[test]
fn spawn_options_reject_invalid_numbers_modes_and_missing_executables() {
    for value in [f64::NAN, f64::INFINITY, -1.0, 0.5, 4_294_967_296.0] {
        let error = spawn_sync_result_with_options(
            "/bin/echo",
            &arguments(&[]),
            SpawnSyncOptions {
                uid: Some(value),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.code(), "ERR_OUT_OF_RANGE");
    }
    for options in [
        SpawnSyncOptions {
            max_buffer: Some(-1.0),
            ..Default::default()
        },
        SpawnSyncOptions {
            timeout: Some(f64::INFINITY),
            ..Default::default()
        },
        SpawnSyncOptions {
            encoding: Some("utf8".to_owned()),
            ..Default::default()
        },
        SpawnSyncOptions {
            kill_signal: Some("NOT_A_SIGNAL".to_owned()),
            ..Default::default()
        },
        SpawnSyncOptions {
            stdio: Some(JsArray::from_dense(vec![JsValue::Bool(true)])),
            ..Default::default()
        },
    ] {
        assert!(spawn_sync_result_with_options("/bin/echo", &arguments(&[]), options).is_err());
    }
    let result = spawn_sync_result_with_options(
        "/nonexistent/tsonic/spawn",
        &arguments(&[]),
        Default::default(),
    )
    .unwrap();
    assert_eq!(result.error.unwrap().code(), "ENOENT");
    assert!(result.status.is_none());
    assert!(result.pid.is_none());
    assert!(result.stdout.is_none());
    assert!(result.stderr.is_none());
}

#[test]
fn spawn_options_inherit_open_file_positions_and_native_credentials() {
    use tsonic_rust_node::{buffer::Buffer, fs};
    let directory = std::env::current_dir().unwrap().join(".temp");
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join(format!("spawn-fd-{}", std::process::id()));
    std::fs::write(&file, b"prefix:remaining").unwrap();
    let descriptor = fs::open_sync(file.to_str().unwrap(), "r").unwrap();
    let mut prefix = Buffer::alloc(7);
    assert_eq!(
        fs::read_sync(descriptor, &mut prefix, 0, 7, None).unwrap(),
        7
    );
    assert_eq!(prefix.as_bytes(), b"prefix:");
    let uid = std::process::Command::new("/usr/bin/id")
        .arg("-u")
        .output()
        .unwrap();
    let gid = std::process::Command::new("/usr/bin/id")
        .arg("-g")
        .output()
        .unwrap();
    assert!(uid.status.success() && gid.status.success());
    let result = spawn_sync_result_with_options(
        "/bin/sh",
        &arguments(&["-c", "cat <&3"]),
        SpawnSyncOptions {
            uid: Some(
                String::from_utf8(uid.stdout)
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap(),
            ),
            gid: Some(
                String::from_utf8(gid.stdout)
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap(),
            ),
            stdio: Some(JsArray::from_dense(vec![
                JsValue::String(JsString::from_utf8("pipe")),
                JsValue::String(JsString::from_utf8("pipe")),
                JsValue::String(JsString::from_utf8("pipe")),
                JsValue::Number(f64::from(descriptor)),
            ])),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.status, Some(0));
    assert!(result.error.is_none());
    assert_eq!(result.stdout.unwrap().as_bytes(), b"remaining");
    assert_eq!(
        fs::read_sync(descriptor, &mut prefix, 0, 1, None).unwrap(),
        0
    );
    fs::close_sync(descriptor).unwrap();
    std::fs::remove_file(file).unwrap();
}
