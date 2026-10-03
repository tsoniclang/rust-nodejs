use std::collections::BTreeMap;
use std::process::Command;

use tsonic_rust_js::{JsObject, JsString, JsValue};
use tsonic_rust_runtime::Record;

use super::apply_environment;

fn environment(command: &Command) -> BTreeMap<String, Option<String>> {
    command
        .get_envs()
        .map(|(key, value)| {
            (
                key.to_str().unwrap().to_owned(),
                value.map(|value| value.to_str().unwrap().to_owned()),
            )
        })
        .collect()
}

#[test]
fn native_record_environment_reads_live_unicode_values_without_retyping_absence() {
    let record = Record::from_entries([
        (
            String::from("é😀"),
            JsValue::String(String::from("original")),
        ),
        (String::from("present"), JsValue::Null),
    ]);
    let options = JsValue::from(record.clone());
    let mut first = Command::new("unused");
    first.env("discard", "old");
    apply_environment(&mut first, &options).unwrap();
    assert_eq!(
        environment(&first),
        BTreeMap::from([(String::from("é😀"), Some(String::from("original"))),])
    );
    record.set(
        String::from("é😀"),
        JsValue::String(String::from("updated")),
    );
    record.set(String::from("added"), JsValue::String(String::from("new")));
    let mut second = Command::new("unused");
    apply_environment(&mut second, &options).unwrap();
    assert_eq!(
        environment(&second),
        BTreeMap::from([
            (String::from("é😀"), Some(String::from("updated"))),
            (String::from("added"), Some(String::from("new"))),
        ])
    );
    assert_eq!(
        environment(&first).get("é😀").unwrap().as_deref(),
        Some("original")
    );
    assert!(record.contains_key("present"));
    assert!(!record.contains_key("missing"));
    let mut inherited = Command::new("unused");
    inherited.env("keep", "selected");
    apply_environment(&mut inherited, &JsValue::Null).unwrap();
    assert_eq!(
        environment(&inherited).get("keep").unwrap().as_deref(),
        Some("selected")
    );
}

#[test]
fn native_record_environment_rejects_wrong_value_carriers_at_the_existing_owner() {
    for value in [
        JsValue::UnsignedInteger(u64::MAX),
        JsValue::Bool(false),
        JsValue::Utf16String(JsString::from_units(vec![0xd800])),
    ] {
        let record = JsValue::from(Record::from_entries([(String::from("invalid"), value)]));
        let error = apply_environment(&mut Command::new("unused"), &record).unwrap_err();
        assert_eq!(error.code(), "ERR_WORKER_OPTIONS");
        assert_eq!(
            error.message(),
            "WorkerOptions.env values must be strings or undefined"
        );
    }
    let error = apply_environment(&mut Command::new("unused"), &JsValue::Bool(true)).unwrap_err();
    assert_eq!(error.code(), "ERR_WORKER_OPTIONS");
}

#[test]
fn existing_exact_object_environment_retains_borrow_and_native_key_rejections() {
    let value = JsValue::object(JsObject::from_pairs([
        ("selected", JsValue::String(String::from("native"))),
        ("absent", JsValue::Null),
    ]));
    let mut command = Command::new("unused");
    apply_environment(&mut command, &value).unwrap();
    assert_eq!(
        environment(&command).get("selected").unwrap().as_deref(),
        Some("native")
    );
    assert!(!environment(&command).contains_key("absent"));
    let borrowed = value.as_object().unwrap().borrow_mut();
    assert!(apply_environment(&mut Command::new("unused"), &value).is_err());
    drop(borrowed);
    let invalid = JsValue::object(JsObject::from_exact_pairs([(
        JsString::from_units(vec![0xd800]),
        JsValue::String(String::from("native")),
    )]));
    assert!(apply_environment(&mut Command::new("unused"), &invalid).is_err());
}
