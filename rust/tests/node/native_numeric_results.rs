use tsonic_rust_js::JsArray;
use tsonic_rust_node::{buffer, fs, process};

#[test]
fn buffer_byte_construction_preserves_native_low_bits_and_owns_its_output() {
    let input = JsArray::from_dense(vec![9_007_199_254_740_993_u64, u64::MAX, 256]);
    let output = buffer::Buffer::from_number_array(&input);
    assert_eq!(output.as_bytes(), vec![1, 255, 0]);
    input.set(0, 17);
    assert_eq!(output.as_bytes(), vec![1, 255, 0]);
    assert_eq!(input.get(0), Some(17));
    let signed = JsArray::from_dense(vec![i128::MIN + 3, -1_i128]);
    assert_eq!(
        buffer::Buffer::from_number_array(&signed).as_bytes(),
        vec![3, 255]
    );
    let floating = JsArray::from_dense(vec![65.9, -1.9, f64::NAN, f64::INFINITY]);
    assert_eq!(
        buffer::Buffer::from_number_array(&floating).as_bytes(),
        vec![65, 255, 0, 0]
    );
}

#[test]
fn buffer_results_preserve_widths_and_counts() {
    let mut bytes = buffer::Buffer::alloc(8);
    let count: usize = buffer::write_uint32_le_number(&mut bytes, u32::MAX as f64, 0.0).unwrap();
    let word: u32 = buffer::read_uint32_le_number(&bytes, 0.0).unwrap();
    assert_eq!(count, 4);
    assert_eq!(word, u32::MAX);
    let count = buffer::write_uint32_le_number(&mut bytes, word, 0_usize).unwrap();
    assert_eq!(count, 4);
    assert_eq!(buffer::read_uint32_le_number(&bytes, 0_u64).unwrap(), word);
    assert!(
        buffer::write_uint32_le_number(&mut bytes, 9_007_199_254_740_993_u64, 0_usize).is_err()
    );
    assert!(buffer::read_uint8_number(&bytes, u128::MAX).is_err());
    buffer::write_int16_le_number(&mut bytes, -32768_i64, 0_usize).unwrap();
    assert_eq!(
        buffer::read_int16_le_number(&bytes, 0_usize).unwrap(),
        i16::MIN
    );
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
    let delta: JsArray<i64> =
        process::hrtime_since(&JsArray::from_dense(vec![previous, 0])).unwrap();
    let after = process::hrtime_open().unwrap();
    assert!(delta.get(0).unwrap() >= before.get(0).unwrap() - previous);
    assert!(delta.get(0).unwrap() <= after.get(0).unwrap() - previous);
    assert!((0..1_000_000_000).contains(&delta.get(1).unwrap()));
    for invalid in [
        vec![],
        vec![0],
        vec![0, 0, 0],
        vec![0, -1],
        vec![0, 1_000_000_000],
    ] {
        assert!(process::hrtime_since(&JsArray::from_dense(invalid)).is_err());
    }
    assert!(process::hrtime_since(&JsArray::from_dense(vec![i64::MIN, 0])).is_err());
}

#[test]
fn memory_fields_do_not_round_large_native_counts() {
    let exact = 9_007_199_254_740_993_u64;
    let value = process::MemoryUsage {
        rss: exact,
        heap_total: exact + 2,
        heap_used: exact + 4,
        external: exact + 6,
        array_buffers: exact + 8,
    };
    assert_eq!(value.rss, exact);
    assert_eq!(value.heap_total - value.rss, 2);
    assert_eq!(value.array_buffers - value.external, 2);
}

#[test]
fn stream_options_preserve_native_offsets_without_a_floating_intermediate() {
    let exact = 9_007_199_254_740_993_u64;
    let read = fs::ReadStreamOptions {
        start: Some(exact),
        end: Some(exact + 2),
        high_water_mark: Some(usize::MAX),
        mode: Some(0o644),
        ..Default::default()
    };
    let write = fs::WriteStreamOptions {
        start: read.start,
        high_water_mark: read.high_water_mark,
        mode: read.mode,
        ..Default::default()
    };
    assert_eq!(read.start, Some(exact));
    assert_eq!(read.end.unwrap() - write.start.unwrap(), 2);
    assert_eq!(write.high_water_mark, Some(usize::MAX));
    assert_eq!(write.mode, Some(0o644));
}

#[test]
fn native_integer_rows_and_worker_values_do_not_round() {
    use tsonic_rust_js::JsValue;
    use tsonic_rust_node::{sqlite::DatabaseSync, worker_threads};
    let database = DatabaseSync::open(":memory:").unwrap();
    let rows = database
        .all(
            "SELECT 9007199254740993 AS exact, -9223372036854775808 AS minimum",
            &[],
        )
        .unwrap();
    assert!(matches!(
        rows[0]["exact"],
        JsValue::Integer(9_007_199_254_740_993)
    ));
    assert!(matches!(rows[0]["minimum"], JsValue::Integer(i64::MIN)));
    for original in [JsValue::from(i64::MIN), JsValue::from(u64::MAX)] {
        let channel = worker_threads::MessageChannel::new();
        channel.port1.post_message(original.clone()).unwrap();
        let transported = worker_threads::receive_message_on_port(&channel.port2).unwrap();
        assert_eq!(transported, original);
        assert_eq!(transported.inspect(), original.inspect());
    }
}

#[test]
fn byte_payloads_keep_integer_slots_in_closed_values() {
    use tsonic_rust_js::JsValue;
    let json = buffer::Buffer::from_bytes(vec![0, 127, 255]).to_json();
    let data = json.as_object().unwrap().borrow().get("data");
    let bytes = data.as_array().unwrap();
    for (index, value) in [0_u64, 127, 255].into_iter().enumerate() {
        assert!(
            matches!(bytes.get(index), Some(JsValue::UnsignedInteger(actual)) if actual == value)
        );
    }
    let database = tsonic_rust_node::sqlite::DatabaseSync::open(":memory:").unwrap();
    let rows = database.all("SELECT X'007FFF' AS bytes", &[]).unwrap();
    let bytes = rows[0]["bytes"].as_array().unwrap();
    for (index, value) in [0_u64, 127, 255].into_iter().enumerate() {
        assert!(
            matches!(bytes.get(index), Some(JsValue::UnsignedInteger(actual)) if actual == value)
        );
    }
}

#[test]
fn process_options_preserve_native_domains_without_a_floating_intermediate() {
    let options = tsonic_rust_node::child_process::SpawnSyncOptions {
        timeout: Some(9_007_199_254_740_993),
        max_buffer: Some(usize::MAX),
        uid: Some(u32::MAX),
        gid: Some(u32::MAX),
        ..Default::default()
    };
    assert_eq!(options.timeout, Some(9_007_199_254_740_993));
    assert_eq!(options.max_buffer, Some(usize::MAX));
    assert_eq!(options.uid, Some(u32::MAX));
    assert_eq!(options.gid, Some(u32::MAX));
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
