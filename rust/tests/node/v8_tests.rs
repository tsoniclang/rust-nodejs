use tsonic_rust_node::v8::set_flags_from_string;

#[test]
fn native_v8_flags_are_explicitly_unsupported_without_changing_state() {
    for flags in ["--stack_size=1024", "", "--trace_gc", "--stack_size=1024"] {
        let error = set_flags_from_string(flags).unwrap_err();
        assert_eq!(error.code(), "ERR_PLATFORM_NOT_SUPPORTED");
        assert_eq!(
            error.message(),
            "node:v8.setFlagsFromString requires a V8 engine; native programs do not host V8"
        );
    }
}
