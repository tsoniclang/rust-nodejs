use super::{decode, encode, ClonedValue, FORMAT_VERSION};
use tsonic_rust_js::{equality::JsSameValue, JsValue};

#[test]
fn wire_round_trip_preserves_each_native_primitive_and_float_bits() {
    for original in [
        JsValue::from(i8::MIN),
        JsValue::from(u8::MAX),
        JsValue::from(i16::MIN),
        JsValue::from(u16::MAX),
        JsValue::from(i32::MIN),
        JsValue::from(u32::MAX),
        JsValue::from(i64::MIN),
        JsValue::from(u64::MAX),
        JsValue::from(isize::MIN),
        JsValue::from(usize::MAX),
        JsValue::from(0.1_f32),
        JsValue::from(0.1_f64),
        JsValue::from(-0.0_f32),
        JsValue::from(-0.0_f64),
        JsValue::from(f32::from_bits(0x7fc0_1234)),
        JsValue::from(f64::from_bits(0x7ff8_0000_0000_1234)),
    ] {
        let bytes = encode(&ClonedValue::from_js(&original).unwrap()).unwrap();
        assert_eq!(bytes[0], FORMAT_VERSION);
        let round_trip = decode(&bytes).unwrap().to_js();
        assert_eq!(
            std::mem::discriminant(&round_trip),
            std::mem::discriminant(&original)
        );
        assert!(original.same_value(&round_trip));
        assert_eq!(original.type_of(), round_trip.type_of());
        match (&original, &round_trip) {
            (JsValue::Float32(left), JsValue::Float32(right)) => {
                assert_eq!(left.to_bits(), right.to_bits())
            }
            (JsValue::Number(left), JsValue::Number(right)) => {
                assert_eq!(left.to_bits(), right.to_bits())
            }
            _ => assert_eq!(original, round_trip),
        }
        for length in 0..bytes.len() {
            assert!(decode(&bytes[..length]).is_err());
        }
        let mut obsolete = bytes.clone();
        obsolete[0] = 1;
        assert!(decode(&obsolete).is_err());
        let mut invalid = bytes.clone();
        invalid[5] = 255;
        assert!(decode(&invalid).is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode(&trailing).is_err());
    }
}

#[test]
fn malformed_wire_keeps_count_and_reference_guards() {
    assert!(decode(&[FORMAT_VERSION, 255, 255, 255, 255, 1]).is_err());
    assert!(decode(&[FORMAT_VERSION, 0, 0, 0, 0, 6, 0, 0, 0, 0]).is_err());
    assert!(decode(&[FORMAT_VERSION, 0, 0, 0, 0, 7, 255, 255, 255, 255]).is_err());
}

#[test]
fn native_record_wire_retains_native_keys_cycles_aliases_and_exact_payloads() {
    use tsonic_rust_js::{equality::JsStrictEqual, JsArray};
    use tsonic_rust_runtime::Record;
    let record = Record::from_entries([
        (String::from("é😀"), JsValue::UnsignedInteger(u64::MAX)),
        (String::from("present"), JsValue::Null),
    ]);
    let value = JsValue::from(record.clone());
    record.set(String::from("self"), value.clone());
    let root = JsValue::array(JsArray::from_dense(vec![value.clone(), value.clone()]));
    let bytes = encode(&ClonedValue::from_js(&root).unwrap()).unwrap();
    let cloned = decode(&bytes).unwrap().to_js();
    let first = cloned.as_array().unwrap().get(0).unwrap();
    let second = cloned.as_array().unwrap().get(1).unwrap();
    assert!(first.strict_equal(&second));
    assert!(!first.strict_equal(&value));
    let native = first.as_record().unwrap();
    assert_eq!(native.get("é😀"), JsValue::UnsignedInteger(u64::MAX));
    assert!(native.contains_key("present"));
    assert!(!native.contains_key("missing"));
    assert!(first.strict_equal(&native.get("self")));
    for length in 0..bytes.len() {
        assert!(decode(&bytes[..length]).is_err());
    }
    let mut obsolete = bytes;
    obsolete[0] = 2;
    assert!(decode(&obsolete).is_err());
    native.remove("self");
    record.remove("self");
}

#[test]
fn native_record_wire_rejects_duplicate_invalid_or_unbounded_keys_and_references() {
    use tsonic_rust_runtime::Record;
    let one = encode(
        &ClonedValue::from_js(&JsValue::from(Record::from_entries([(
            String::from("key"),
            JsValue::Null,
        )])))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(one[10], 2);
    let entry = one[15..].to_vec();
    let mut duplicate = one.clone();
    duplicate[11..15].copy_from_slice(&2_u32.to_be_bytes());
    duplicate.extend_from_slice(&entry);
    assert!(decode(&duplicate).is_err());
    let mut invalid_utf8 = one.clone();
    invalid_utf8[19] = 255;
    assert!(decode(&invalid_utf8).is_err());
    let mut excessive_key = one.clone();
    excessive_key[15..19].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(decode(&excessive_key).is_err());
    let mut excessive_entries = one.clone();
    excessive_entries[11..15].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(decode(&excessive_entries).is_err());
    let mut invalid_reference = one.clone();
    *invalid_reference.last_mut().unwrap() = 6;
    invalid_reference.extend_from_slice(&1_u32.to_be_bytes());
    assert!(decode(&invalid_reference).is_err());
    let mut unknown_container = one.clone();
    unknown_container[10] = 255;
    assert!(decode(&unknown_container).is_err());
    let mut trailing = one;
    trailing.push(0);
    assert!(decode(&trailing).is_err());
    for mut reader in [
        super::Reader::new(&[]),
        super::Reader::new(&[0, 0, 0, 1, b'x']),
    ] {
        reader.string_units = super::MAXIMUM_STRING_UNITS;
        assert!(reader.native_string().is_err());
    }
}

#[test]
fn native_record_source_preserves_independent_depth_entry_and_string_budgets() {
    use super::super::{
        clone_slot, reserve_entries, reserve_native_string, EncodingState, MAXIMUM_DEPTH,
        MAXIMUM_ENTRIES,
    };
    use tsonic_rust_runtime::Record;
    let record = JsValue::from(Record::from_entries([(String::from("key"), JsValue::Null)]));
    let mut state = EncodingState::default();
    assert!(clone_slot(&record, MAXIMUM_DEPTH + 1, &mut state).is_err());
    state.entries = MAXIMUM_ENTRIES;
    assert!(clone_slot(&record, 0, &mut state).is_err());
    state.entries = usize::MAX;
    assert!(reserve_entries(1, &mut state).is_err());
    state.entries = 0;
    state.string_units = super::MAXIMUM_STRING_UNITS;
    assert!(clone_slot(&record, 0, &mut state).is_err());
    state.string_units = usize::MAX;
    assert!(reserve_native_string(1, &mut state).is_err());
}
