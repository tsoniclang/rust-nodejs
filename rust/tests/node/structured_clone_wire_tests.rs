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
