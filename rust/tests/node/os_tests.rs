use tsonic_rust_node::os;

#[test]
fn homedir_uses_platform_environment_and_account_lookup() {
    if let Ok(expected) = std::env::var("TSONIC_NODE_HOME_EXPECTED") {
        assert_eq!(os::homedir().unwrap(), expected);
        return;
    }
    if std::env::var_os("TSONIC_NODE_HOME_ACCOUNT").is_some() {
        let expected = std::env::home_dir().expect("Test account has a home directory");
        assert_eq!(os::homedir().unwrap(), expected.to_string_lossy());
        return;
    }
    let variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let unrelated = if cfg!(windows) { "HOME" } else { "USERPROFILE" };
    let executable = std::env::current_exe().unwrap();
    let case = "os_tests::homedir_uses_platform_environment_and_account_lookup";
    for value in ["/tsonic-home/é", ""] {
        let result = std::process::Command::new(&executable)
            .args(["--exact", case])
            .env(variable, value)
            .env(unrelated, "/unrelated-home")
            .env("TSONIC_NODE_HOME_EXPECTED", value)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
    }
    let result = std::process::Command::new(executable)
        .args(["--exact", case])
        .env_remove(variable)
        .env(unrelated, "/unrelated-home")
        .env("TSONIC_NODE_HOME_ACCOUNT", "1")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
}

#[test]
fn os_wrappers_have_stable_shapes() {
    assert!(!os::platform().is_empty());
    assert!(!os::arch().is_empty());
    assert!(matches!(os::eol(), "\n" | "\r\n"));
    assert!(!os::tmpdir().unwrap().is_empty());
    assert!(!os::homedir().unwrap().is_empty());
    assert!(!os::hostname().is_empty());
    assert!(!os::r#type().is_empty());
    assert!(!os::release().is_empty());
    let _version = os::version();
    let cpus = os::cpus();
    assert!(!cpus.is_empty());
    assert!(cpus.iter().all(|cpu| !cpu.model.is_empty()));
    assert_eq!(cpus.iter().map(|cpu| cpu.times.user).min().unwrap_or(0), 0);
    assert!(os::available_parallelism() >= 1);
    let loadavg = os::loadavg();
    assert_eq!(loadavg.len(), 3);
    assert!(loadavg
        .iter()
        .all(|value| value.is_finite() && *value >= 0.0));
    assert!(os::totalmem() >= os::freemem());
    assert!(os::uptime() >= 0.0);
    assert!(!os::machine().is_empty());
    assert!(matches!(os::endianness(), "LE" | "BE"));
    assert!(!os::dev_null().is_empty());
    let user = os::user_info();
    assert!(user.homedir.is_empty() || std::path::Path::new(&user.homedir).is_absolute());
    let user_with_options = os::user_info_with_options(Some(os::UserInfoOptions {
        encoding: Some("utf8".to_string()),
    }));
    assert_eq!(user.username, user_with_options.username);
    let constants = os::constants();
    assert_eq!(constants.priority.priority_normal, 0);
    assert!(constants.errno.contains_key("ENOENT"));
    assert!(constants.errno.contains_key("ENOSYS"));
    assert!(constants.errno.contains_key("EBADMSG"));
    assert!(constants.errno.contains_key("EDQUOT"));
    assert!(constants.errno.contains_key("EWOULDBLOCK"));
    assert!(constants.errno.contains_key("WSAEADDRINUSE"));
    assert!(constants.errno.contains_key("WSAETOOMANYREFS"));
    assert!(constants.errno.contains_key("WSANOTINITIALISED"));
    assert!(constants.errno.contains_key("WSA_E_NO_MORE"));
    assert!(constants.signals.contains_key("SIGTERM") || cfg!(not(unix)));
    assert!(constants.dlopen.contains_key("RTLD_NOW") || cfg!(not(unix)));
    assert!(constants.dlopen.contains_key("RTLD_DEEPBIND") || cfg!(not(unix)));
    assert_eq!(constants.uv.get("UV_UDP_REUSEADDR"), Some(&4));
    assert_eq!(
        os::errno_constant("ENOENT"),
        constants.errno.get("ENOENT").copied()
    );
    assert_eq!(
        os::errno_constant("WSAEADDRINUSE"),
        constants.errno.get("WSAEADDRINUSE").copied()
    );
    assert_eq!(
        os::errno_constant("WSA_E_NO_MORE"),
        constants.errno.get("WSA_E_NO_MORE").copied()
    );
    assert_eq!(
        os::priority_constant("PRIORITY_NORMAL"),
        Some(constants.priority.priority_normal)
    );
    assert_eq!(
        os::priority_constant("PRIORITY_HIGHEST"),
        Some(constants.priority.priority_highest)
    );
    assert_eq!(os::uv_constant("UV_UDP_REUSEADDR"), Some(4));
    assert_eq!(
        os::signal_constant("SIGTERM"),
        constants.signals.get("SIGTERM").copied()
    );
    assert_eq!(
        os::dlopen_constant("RTLD_NOW"),
        constants.dlopen.get("RTLD_NOW").copied()
    );
    assert_eq!(os::errno_constant("NO_SUCH_ERRNO"), None);
    assert_eq!(os::priority_constant("NO_SUCH_PRIORITY"), None);
    let interfaces = os::network_interfaces().unwrap();
    assert!(interfaces
        .values()
        .flatten()
        .all(|interface| matches!(interface.family.as_str(), "IPv4" | "IPv6")));
    assert!(interfaces.values().flatten().all(|interface| {
        !interface.address.is_empty()
            && !interface.netmask.is_empty()
            && !interface.mac.is_empty()
            && interface
                .cidr
                .as_ref()
                .is_none_or(|cidr| cidr.contains('/'))
    }));
}
