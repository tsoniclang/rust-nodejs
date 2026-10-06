use super::EventEmitter;
use crate::NodeError;
use std::cell::Cell;
use std::rc::Rc;
use tsonic_rust_js::JsValue;
use tsonic_rust_runtime::Callable;

enum Failure {
    Original(Rc<Cell<i64>>),
    Native(NodeError),
}

impl From<NodeError> for Failure {
    fn from(error: NodeError) -> Self {
        Self::Native(error)
    }
}

#[test]
fn original_non_display_error_stops_without_consuming_later_once_listeners() {
    let emitter = EventEmitter::<Failure>::new();
    let event = JsValue::from("data".to_owned());
    let original = Rc::new(Cell::new(9_007_199_254_740_993));
    let captured = Rc::clone(&original);
    let first = Callable::new(move |()| Err(Failure::Original(Rc::clone(&captured))));
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let second = Callable::new(move |()| {
        observed.set(observed.get() + 1);
        Ok::<(), Failure>(())
    });
    emitter.once_callable(&event, &first).unwrap();
    emitter.once_callable(&event, &second).unwrap();
    let returned = emitter
        .emit_callable(&event)
        .err()
        .expect("original callback error");
    match returned {
        Failure::Original(value) => {
            assert!(Rc::ptr_eq(&value, &original));
            assert_eq!(value.get(), 9_007_199_254_740_993);
        }
        Failure::Native(_) => panic!("source error was replaced"),
    }
    assert_eq!(calls.get(), 0);
    assert_eq!(emitter.callable_listener_count(&event).unwrap(), 1);
    assert!(emitter.emit_callable(&event).is_ok());
    assert_eq!(calls.get(), 1);
    assert_eq!(emitter.callable_listener_count(&event).unwrap(), 0);
}

#[test]
fn reentrant_once_emission_preserves_shared_registration_state() {
    let emitter = EventEmitter::<NodeError>::new();
    let event = JsValue::from("data".to_owned());
    let nested = emitter.clone();
    let nested_event = event.clone();
    let first_calls = Rc::new(Cell::new(0));
    let second_calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&first_calls);
    let first = Callable::new(move |()| {
        observed.set(observed.get() + 1);
        assert!(nested.emit_callable(&nested_event)?);
        Ok::<(), NodeError>(())
    });
    let observed = Rc::clone(&second_calls);
    let second = Callable::new(move |()| {
        observed.set(observed.get() + 1);
        Ok::<(), NodeError>(())
    });
    emitter.once_callable(&event, &first).unwrap();
    emitter.once_callable(&event, &second).unwrap();
    assert!(emitter.emit_callable(&event).unwrap());
    assert_eq!(first_calls.get(), 1);
    assert_eq!(second_calls.get(), 1);
    assert!(!emitter.emit_callable(&event).unwrap());
}

#[test]
fn pending_snapshot_and_new_registration_keep_distinct_once_ownership() {
    let emitter = EventEmitter::<NodeError>::new();
    let event = JsValue::from("data".to_owned());
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let second = Callable::new(move |()| {
        observed.set(observed.get() + 1);
        Ok::<(), NodeError>(())
    });
    let nested = emitter.clone();
    let nested_event = event.clone();
    let listener = second.clone();
    let first = Callable::new(move |()| {
        nested.off_callable(&nested_event, &listener)?;
        nested.once_callable(&nested_event, &listener)?;
        Ok::<(), NodeError>(())
    });
    emitter.once_callable(&event, &first).unwrap();
    emitter.once_callable(&event, &second).unwrap();
    assert!(emitter.emit_callable(&event).unwrap());
    assert_eq!(calls.get(), 1);
    assert_eq!(emitter.callable_listener_count(&event).unwrap(), 1);
    assert!(emitter.emit_callable(&event).unwrap());
    assert_eq!(calls.get(), 2);
}

#[test]
fn native_event_guards_enter_the_exact_error_domain() {
    let emitter = EventEmitter::<Failure>::new();
    let event = JsValue::from("error".to_owned());
    let returned = emitter
        .emit_callable(&event)
        .err()
        .expect("unhandled native event");
    assert!(matches!(returned, Failure::Native(error) if error.code() == "ERR_UNHANDLED_ERROR"));
    let returned = emitter
        .emit_callable(&JsValue::Null)
        .err()
        .expect("invalid native event");
    assert!(matches!(returned, Failure::Native(error) if error.code() == "ERR_INVALID_ARG_TYPE"));
}

#[test]
fn retaining_a_normal_source_listener_has_no_second_callable_owner() {
    let emitter = EventEmitter::<Failure>::new();
    let event = JsValue::from("data".to_owned());
    let callback = Callable::new(|()| Ok::<(), Failure>(()));
    emitter.on_callable(&event, &callback).unwrap();
    let state = emitter.state.borrow();
    let entry = &state
        .callable_listeners
        .get(super::EventName::from_value(&event).unwrap())
        .unwrap()[0];
    assert!(matches!(&entry.callback,
        super::callback::ListenerCallback::Repeated(super::callback::EventCallback::Empty(stored))
            if Callable::same(stored, &callback)));
}
