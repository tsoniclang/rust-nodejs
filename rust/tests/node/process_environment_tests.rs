use tsonic_rust_node::process::{self, ProcessEnv};

#[test]
fn environment_values_preserve_local_aliases_and_live_process_reads() {
    let name = "TSONIC_RUST_ENV_VALUE_PROOF";
    process::env_delete(name);
    let parent = process::environment();
    let local = ProcessEnv::default();
    local.set(name, Some("child".into())).unwrap();
    let alias = local.clone();
    assert_eq!(alias.get(name).as_deref(), Some("child"));
    assert_eq!(parent.get(name), None);
    alias.set(name, Some("replacement".into())).unwrap();
    assert_eq!(
        local.entries().get(name).map(String::as_str),
        Some("replacement")
    );
    local.set(name, None).unwrap();
    assert_eq!(alias.get(name), None);
    assert!(!local.entries().contains_key(name));
    parent.set(name, Some("parent".into())).unwrap();
    assert_eq!(process::environment().get(name).as_deref(), Some("parent"));
    assert_eq!(local.get(name), None);
    parent.set(name, None).unwrap();
    assert_eq!(parent.get(name), None);
    process::env_delete(name);
    assert_eq!(parent.get(name), None);
    assert!(parent.set("bad=name", Some("value".into())).is_err());
    assert!(parent.set(name, Some("bad\0value".into())).is_err());
}
