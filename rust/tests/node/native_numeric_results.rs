use tsonic_rust_js::JsArray;
use tsonic_rust_node::{buffer, process};

#[test]
fn buffer_results_preserve_widths_and_counts() {
    let mut bytes = buffer::Buffer::alloc(8);
    let count: usize = buffer::write_uint32_le_number(&mut bytes, u32::MAX as f64, 0.0).unwrap();
    let word: u32 = buffer::read_uint32_le_number(&bytes, 0.0).unwrap();
    assert_eq!(count, 4);
    assert_eq!(word, u32::MAX);
    let count: usize = buffer::write_float_le_number(&mut bytes, 1.5, 4.0).unwrap();
    let single: f32 = buffer::read_float_le_number(&bytes, 4.0).unwrap();
    assert_eq!(count, 8);
    assert_eq!(single, 1.5);
    assert!(buffer::read_uint8_number(&bytes, 0.5).is_err());
    assert!(buffer::read_uint8_number(&bytes, f64::INFINITY).is_err());
    assert!(buffer::read_uint8_number(&bytes, 8.0).is_err());
}

#[test]
fn high_resolution_time_preserves_large_integer_inputs() {
    let before: JsArray<i64> = process::hrtime_open().unwrap();
    let previous = 9_007_199_254_740_993_i64;
    let delta: JsArray<i64> = process::hrtime_since(&JsArray::from_dense(vec![previous, 0])).unwrap();
    let after = process::hrtime_open().unwrap();
    assert!(delta.get(0).unwrap() >= before.get(0).unwrap() - previous);
    assert!(delta.get(0).unwrap() <= after.get(0).unwrap() - previous);
    assert!((0..1_000_000_000).contains(&delta.get(1).unwrap()));
    for invalid in [vec![], vec![0], vec![0, 0, 0], vec![0, -1], vec![0, 1_000_000_000]] {
        assert!(process::hrtime_since(&JsArray::from_dense(invalid)).is_err());
    }
    assert!(process::hrtime_since(&JsArray::from_dense(vec![i64::MIN, 0])).is_err());
}

#[test]
fn memory_fields_do_not_round_large_native_counts() {
    let exact = 9_007_199_254_740_993_u64;
    let value = process::MemoryUsage {
        rss: exact, heap_total: exact + 2, heap_used: exact + 4,
        external: exact + 6, array_buffers: exact + 8,
    };
    assert_eq!(value.rss, exact);
    assert_eq!(value.heap_total - value.rss, 2);
    assert_eq!(value.array_buffers - value.external, 2);
}

#[test]
fn socket_timeout_rejects_rounded_native_upper_bounds_before_io() {
    let mut socket = tsonic_rust_node::net::Socket::new_with_options(Default::default()).unwrap();
    for invalid in [u64::MAX as f64, f64::INFINITY, f64::NAN, -1.0, 0.5] {
        let error = match socket.set_timeout_number(invalid) {
            Ok(_) => panic!("invalid timeout was accepted"),
            Err(error) => error,
        };
        assert_eq!(error.code, "ERR_OUT_OF_RANGE");
    }
}
