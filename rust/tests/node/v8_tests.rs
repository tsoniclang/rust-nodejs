use tsonic_rust_node::v8::{get_heap_statistics, set_flags_from_string};

#[test]
fn native_v8_heap_observations_never_return_fabricated_measurements() {
    for _ in 0..3 {
        let error = get_heap_statistics().unwrap_err();
        assert_eq!(error.code(), "ERR_PLATFORM_NOT_SUPPORTED");
        assert_eq!(
            error.message(),
            "node:v8.getHeapStatistics requires a V8 engine; native programs do not host V8"
        );
    }
}

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
