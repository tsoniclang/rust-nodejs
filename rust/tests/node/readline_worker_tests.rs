use std::cell::Cell;
use std::collections::BTreeMap;

use tsonic_rust_js::equality::JsStrictEqual;
use tsonic_rust_js::{JsArray, JsObject, JsString, JsValue};
use tsonic_rust_node::{process, readline, worker_threads};

#[test]
fn readline_interface_uses_explicit_input_and_output_buffers() {
    let mut interface = readline::create_interface(vec!["answer".to_string(), "line2".to_string()]);
    assert_eq!(interface.question("name?").as_deref(), Some("answer"));
    interface.write("done");
    assert_eq!(
        interface.output(),
        &["name?".to_string(), "done".to_string()]
    );
    assert_eq!(interface.line(), "done");
    assert_eq!(interface.cursor(), 4);
    assert_eq!(interface.get_cursor_pos().cols, 4);
    interface.set_prompt("next> ");
    assert_eq!(interface.get_prompt(), "next> ");
    interface.prompt(false);
    assert_eq!(interface.output().last().unwrap(), "next> ");
    interface.write_key(
        None,
        Some(readline::Key {
            sequence: Some("!".to_string()),
            name: Some("bang".to_string()),
            ctrl: false,
            meta: false,
            shift: true,
        }),
    );
    assert_eq!(interface.line(), "done!");
    interface.set_terminal(true);
    assert!(interface.terminal());
    assert_eq!(interface.next_line().as_deref(), Some("line2"));
    interface.pause();
    assert!(interface.paused());
    interface.write("ignored");
    assert_eq!(interface.next_line(), None);
    interface.resume();
    assert!(!interface.paused());
    assert!(interface.remaining_lines().is_empty());
    interface.close();
    assert!(interface.closed());
    assert_eq!(interface.question("again?"), None);

    let mut terminal = readline::promises::create_readline(false);
    terminal
        .clear_line(0)
        .cursor_to(2, Some(1))
        .move_cursor(-1, 0);
    assert_eq!(terminal.stream().len(), 0);
    assert_eq!(terminal.pending().len(), 3);
    terminal.rollback();
    assert_eq!(terminal.pending().len(), 0);
    terminal.clear_screen_down().commit();
    assert_eq!(terminal.stream(), &["clearScreenDown".to_string()]);

    let mut auto = readline::Readline::new(true);
    assert!(auto.auto_commit());
    auto.cursor_to(1, None);
    assert_eq!(auto.stream(), &["cursorTo:1:0".to_string()]);

    let configured = readline::Readline::with_options(readline::ReadlineOptions {
        input: Some("stdin".to_string()),
        output: Some("stdout".to_string()),
        terminal: true,
        completer: Some("word-completer".to_string()),
        auto_commit: true,
        signal: Some("abort-signal".to_string()),
        history_size: Some(32),
        history: Some(vec!["old".to_string()]),
        remove_history_duplicates: true,
        escape_code_timeout: Some(500),
        prompt: Some("prompt> ".to_string()),
        crlf_delay: Some(100),
        tab_size: Some(4),
    });
    assert_eq!(configured.options().input.as_deref(), Some("stdin"));
    assert_eq!(configured.options().output.as_deref(), Some("stdout"));
    assert!(configured.options().terminal);
    assert_eq!(
        configured.options().completer.as_deref(),
        Some("word-completer")
    );
    assert_eq!(configured.options().signal.as_deref(), Some("abort-signal"));
    assert_eq!(configured.options().history_size, Some(32));
    assert_eq!(
        configured.options().history.as_deref(),
        Some(&["old".to_string()][..])
    );
    assert!(configured.options().remove_history_duplicates);
    assert_eq!(configured.options().escape_code_timeout, Some(500));
    assert_eq!(configured.options().prompt.as_deref(), Some("prompt> "));
    assert_eq!(configured.options().crlf_delay, Some(100));
    assert_eq!(configured.options().tab_size, Some(4));
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
fn worker_broadcast_channel_is_closed_in_process_state() {
    let left = worker_threads::BroadcastChannel::new("updates");
    let right = worker_threads::BroadcastChannel::new("updates");
    assert_eq!(left.name(), "updates");
    left.post_message(JsValue::from("payload".to_string()))
        .unwrap();
    assert_eq!(
        right.receive_message(),
        Some(JsValue::from("payload".to_string()))
    );
    assert_eq!(right.receive_message(), None);
    left.close();
}

#[test]
fn worker_runs_rust_closure_and_reports_join_result() {
    let worker = worker_threads::Worker::spawn(|| 21 * 2);
    assert_eq!(worker.join().unwrap(), 42);
}

#[test]
fn worker_options_environment_and_transfer_markers_are_closed_state() {
    let mut env = BTreeMap::new();
    env.insert("MODE".to_string(), "test".to_string());
    let mut worker = worker_threads::Worker::spawn_with_options(
        || "done".to_string(),
        worker_threads::WorkerOptions {
            name: Some("compiler-worker".to_string()),
            argv: vec!["--job".to_string()],
            env,
            worker_data: JsValue::from("payload".to_string()),
            resource_limits: worker_threads::ResourceLimits {
                max_old_generation_size_mb: Some(256),
                max_young_generation_size_mb: Some(64),
                code_range_size_mb: Some(32),
                stack_size_mb: Some(8),
            },
        },
    );
    assert!(worker.thread_id() > 0);
    assert_eq!(worker.name(), Some("compiler-worker"));
    assert_eq!(
        worker.resource_limits().max_old_generation_size_mb,
        Some(256)
    );
    assert!(worker.has_ref());
    worker.unref();
    assert!(!worker.has_ref());
    worker.r#ref();
    assert!(worker.has_ref());
    assert_eq!(worker.join().unwrap(), "done");

    worker_threads::set_environment_data("runtime", JsValue::from("rust".to_string())).unwrap();
    assert_eq!(
        worker_threads::get_environment_data("runtime"),
        Some(JsValue::from("rust".to_string()))
    );
    worker_threads::mark_as_untransferable_token("buffer-1");
    assert!(worker_threads::is_marked_as_untransferable_token(
        "buffer-1"
    ));

    let channel = worker_threads::MessageChannel::new();
    let moved = worker_threads::move_message_port_to_context(channel.port1);
    moved.post_message(JsValue::Number(1.0)).unwrap();
    assert_eq!(channel.port2.receive_message(), Some(JsValue::Number(1.0)));
}

#[test]
fn worker_structured_clone_rejects_cyclic_values_deterministically() {
    let value = JsValue::object(JsObject::new());
    value
        .as_object()
        .unwrap()
        .borrow_mut()
        .set("self", value.clone());

    let direct = worker_threads::ClonedValue::from_js(&value).unwrap_err();
    assert_eq!(direct.code(), "DATA_CLONE_ERR");
    assert_eq!(
        direct.message(),
        "circular structure cannot be structured-cloned"
    );

    let environment = worker_threads::set_environment_data("cyclic", value.clone()).unwrap_err();
    assert_eq!(environment, direct);
    assert_eq!(worker_threads::get_environment_data("cyclic"), None);

    let channel = worker_threads::BroadcastChannel::new("cyclic-updates");
    let broadcast = channel.post_message(value.clone()).unwrap_err();
    assert_eq!(broadcast, direct);
    assert_eq!(channel.receive_message(), None);

    let ports = worker_threads::MessageChannel::new();
    let posted = ports.port1.post_message(value).unwrap_err();
    assert_eq!(posted, direct);
    assert_eq!(worker_threads::receive_message_on_port(&ports.port2), None);
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

    let cloned = worker_threads::ClonedValue::from_js(&JsValue::object(object))
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
    let payload = worker_threads::ClonedValue::from_js(&original).unwrap();
    assert_eq!(
        worker_threads::ClonedValue::from_js(&payload.to_js()).unwrap(),
        payload
    );

    let channel = worker_threads::BroadcastChannel::new("structure-updates");
    channel.post_message(original.clone()).unwrap();
    let broadcasted = channel.receive_message().unwrap();
    assert!(!original.strict_equal(&broadcasted));
    assert_eq!(
        broadcasted.as_object().unwrap().borrow().get("kind"),
        JsValue::from("payload".to_string())
    );
}
