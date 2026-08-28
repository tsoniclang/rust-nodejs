use std::cell::{Cell, RefCell};
use std::rc::Rc;

use tsonic_rust_js::equality::JsStrictEqual;
use tsonic_rust_js::{JsArray, JsObject, JsString, JsValue};
use tsonic_rust_node::buffer::Buffer;
use tsonic_rust_node::stream::{Readable, Writable};
use tsonic_rust_node::{process, readline, worker_threads};
use tsonic_rust_runtime::{Callable, TsonicError};

#[test]
fn readline_interface_uses_explicit_input_and_output_buffers() {
    let input = Readable::from_chunks(vec![
        Buffer::from_string("answer\r\nline2\n", Some("utf8")).unwrap(),
    ]);
    let output = Writable::new();
    let mut interface = readline::create_interface(readline::SourceInterfaceOptions {
        input,
        output: Some(output),
        terminal: Some(true),
        prompt: Some("name? ".to_string()),
    });

    let answer = Rc::new(RefCell::new(None::<String>));
    let callback_answer = Rc::clone(&answer);
    interface
        .question_callable(
            "name? ",
            Callable::new(move |(value,)| {
                *callback_answer.borrow_mut() = Some(value);
                Ok::<(), TsonicError>(())
            }),
        )
        .unwrap();
    tsonic_rust_node::run_event_loop().unwrap();
    assert_eq!(answer.borrow().as_deref(), Some("answer"));

    interface.write("done😀").unwrap();
    assert_eq!(interface.line(), "done😀");
    assert_eq!(interface.cursor_number(), 6.0);
    assert_eq!(interface.get_cursor_pos().cols, 6);
    interface.set_prompt("next> ");
    assert_eq!(interface.get_prompt(), "next> ");
    interface.prompt().unwrap();
    assert!(interface.terminal());
    assert_eq!(interface.next_line().unwrap().as_deref(), Some("line2"));
    assert!(interface.next_line().unwrap().is_none());
    interface.pause_chain();
    assert!(interface.is_paused());
    interface.write("ignored").unwrap();
    assert!(interface.next_line().unwrap().is_none());
    interface.resume_chain();
    assert!(!interface.is_paused());
    interface.close();
    assert!(interface.next_line().unwrap().is_none());
}

#[test]
fn process_next_tick_executes_without_event_loop_guessing() {
    let called = Cell::new(false);
    process::next_tick(|| called.set(true));
    assert!(called.get());
}

#[test]
fn worker_message_channel_structured_clones_js_values() {
    let channel = worker_threads::MessageChannel::new();
    channel.port1.start();
    assert!(channel.port1.has_ref());
    channel.port1.unref();
    assert!(!channel.port1.has_ref());
    channel.port1.r#ref();
    assert!(channel.port1.has_ref());
    channel
        .port1
        .post_message(JsValue::from("hello".to_string()))
        .unwrap();
    assert_eq!(
        worker_threads::receive_message_on_port(&channel.port2),
        Some(JsValue::from("hello".to_string()))
    );
    assert!(worker_threads::is_main_thread());
    assert!(worker_threads::parent_port().is_none());
    assert_eq!(worker_threads::worker_data(), JsValue::Undefined);
}

#[test]
fn worker_environment_data_and_transfer_markers_use_exact_reference_identity() {
    worker_threads::set_environment_data("runtime", JsValue::from("rust".to_string())).unwrap();
    assert_eq!(
        worker_threads::get_environment_data("runtime"),
        Some(JsValue::from("rust".to_string()))
    );
    let reference = JsValue::object(JsObject::new());
    let distinct = JsValue::object(JsObject::new());
    worker_threads::mark_as_untransferable(&reference).unwrap();
    assert!(worker_threads::is_marked_as_untransferable(&reference).unwrap());
    assert!(!worker_threads::is_marked_as_untransferable(&distinct).unwrap());
    assert_eq!(
        worker_threads::mark_as_untransferable(&JsValue::Number(1.0))
            .unwrap_err()
            .code(),
        "ERR_INVALID_ARG_TYPE"
    );
}

#[test]
fn worker_structured_clone_preserves_cycles_and_repeated_aliases() {
    let value = JsValue::object(JsObject::new());
    value
        .as_object()
        .unwrap()
        .borrow_mut()
        .set("self", value.clone());

    let direct = worker_threads::StructuredCloneValue::from_js(&value)
        .unwrap()
        .to_js();
    let direct_self = direct.as_object().unwrap().borrow().get("self");
    assert!(direct.strict_equal(&direct_self));

    let aliases = JsArray::from_dense(vec![value.clone(), value.clone()]);
    let ports = worker_threads::MessageChannel::new();
    ports.port1.post_message(JsValue::array(aliases)).unwrap();
    let received = worker_threads::receive_message_on_port(&ports.port2).unwrap();
    let received = received.as_array().unwrap();
    let first = received.get(0).unwrap();
    let second = received.get(1).unwrap();
    assert!(first.strict_equal(&second));
    assert!(first.strict_equal(&first.as_object().unwrap().borrow().get("self")));
}

#[test]
fn worker_message_port_round_trips_structure_without_identity() {
    let sparse = JsArray::with_length(3);
    sparse.set(0, JsValue::Number(1.0));
    sparse.set(2, JsValue::from("tail".to_string()));
    let original = JsValue::object(JsObject::from_pairs([
        ("kind", JsValue::from("payload".to_string())),
        ("items", JsValue::array(sparse)),
    ]));

    let channel = worker_threads::MessageChannel::new();
    channel.port1.post_message(original.clone()).unwrap();
    let received = worker_threads::receive_message_on_port(&channel.port2).unwrap();

    // Identity does not cross the port.
    assert!(!original.strict_equal(&received));

    // Structural content does, including sparse array holes.
    assert_eq!(
        received.as_object().unwrap().borrow().get("kind"),
        JsValue::from("payload".to_string())
    );
    let items = received
        .as_object()
        .unwrap()
        .borrow()
        .get("items")
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(items.len(), 3);
    assert_eq!(items.get(0), Some(JsValue::Number(1.0)));
    assert!(!items.has_index(1));
    assert_eq!(items.get(2), Some(JsValue::from("tail".to_string())));

    // Each delivery mints fresh handles: two posts of the same value are not
    // strict-equal to each other after crossing the port.
    channel.port1.post_message(original.clone()).unwrap();
    let received_again = worker_threads::receive_message_on_port(&channel.port2).unwrap();
    assert!(!received.strict_equal(&received_again));
}

#[test]
fn worker_structured_clone_preserves_exact_string_keys_and_values() {
    let key = JsString::from_units(vec![0xd800]);
    let value = JsString::from_units(vec![0xdc00]);
    let mut object = JsObject::new();
    object.set_exact(key.clone(), JsValue::String(value.clone()));

    let cloned = worker_threads::StructuredCloneValue::from_js(&JsValue::object(object))
        .unwrap()
        .to_js();
    let entries = cloned.as_object().unwrap().borrow().entries_exact();

    assert_eq!(entries, vec![(key, JsValue::String(value))]);
}

#[test]
fn worker_environment_data_round_trips_structure_without_identity() {
    let sparse = JsArray::with_length(3);
    sparse.set(0, JsValue::Number(1.0));
    sparse.set(2, JsValue::from("tail".to_string()));
    let original = JsValue::object(JsObject::from_pairs([
        ("kind", JsValue::from("payload".to_string())),
        ("items", JsValue::array(sparse)),
    ]));

    worker_threads::set_environment_data("payload", original.clone()).unwrap();
    let received = worker_threads::get_environment_data("payload").unwrap();

    // Identity does not survive the structured-clone boundary.
    assert!(!original.strict_equal(&received));
    let received_again = worker_threads::get_environment_data("payload").unwrap();
    assert!(!received.strict_equal(&received_again));

    // Structural content does survive it, including sparse array holes.
    let object = received.as_object().unwrap().borrow().clone();
    assert_eq!(object.get("kind"), JsValue::from("payload".to_string()));
    let items = received
        .as_object()
        .unwrap()
        .borrow()
        .get("items")
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(items.len(), 3);
    assert_eq!(items.get(0), Some(JsValue::Number(1.0)));
    assert!(!items.has_index(1));
    assert_eq!(items.get(2), Some(JsValue::from("tail".to_string())));

    // The payload rebuilds identical structure on every rebuild.
    let payload = worker_threads::StructuredCloneValue::from_js(&original).unwrap();
    assert_eq!(
        worker_threads::StructuredCloneValue::from_js(&payload.to_js()).unwrap(),
        payload
    );
}
