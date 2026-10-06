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

struct SubscriptionDrop {
    emitter: super::WeakEventEmitter<NodeError>,
    calls: Rc<Cell<usize>>,
}

impl Drop for SubscriptionDrop {
    fn drop(&mut self) {
        let emitter = self
            .emitter
            .upgrade()
            .expect("native emitter is still owned");
        assert!(
            emitter.state.try_borrow_mut().is_ok(),
            "listener capture dropped under emitter borrow"
        );
        emitter.set_max_listeners(3);
        self.calls.set(self.calls.get() + 1);
    }
}

#[test]
fn bulk_source_listener_removal_allows_native_capture_destructors_to_reenter() {
    for all_events in [false, true] {
        let emitter = EventEmitter::<NodeError>::new();
        let event = JsValue::from("data".to_owned());
        let calls = Rc::new(Cell::new(0));
        let owner = SubscriptionDrop {
            emitter: emitter.downgrade(),
            calls: Rc::clone(&calls),
        };
        let listener = Callable::new(move |()| {
            let _capture = &owner;
            Ok::<(), NodeError>(())
        });
        emitter.on_callable(&event, &listener).unwrap();
        drop(listener);
        if all_events {
            emitter.remove_all_callable_listeners();
        } else {
            emitter.remove_all_callable_listeners_for(&event).unwrap();
        }
        assert_eq!(calls.get(), 1);
        assert_eq!(emitter.get_max_listeners(), 3);
        assert_eq!(emitter.callable_listener_count(&event).unwrap(), 0);
    }
}

#[test]
fn native_id_and_bulk_removal_release_captures_outside_emitter_borrows() {
    for selection in [0, 1, 2] {
        let emitter = EventEmitter::<NodeError>::new();
        let calls = Rc::new(Cell::new(0));
        let owner = SubscriptionDrop {
            emitter: emitter.downgrade(),
            calls: Rc::clone(&calls),
        };
        let id = emitter.on_with_id("data", move |_| {
            let _capture = &owner;
        });
        match selection {
            0 => {
                emitter.off_by_id("data", id);
            }
            1 => {
                emitter.remove_all_listeners(Some("data"));
            }
            _ => {
                emitter.remove_all_listeners(None);
            }
        }
        assert_eq!(calls.get(), 1);
        assert_eq!(emitter.listener_count("data"), 0);
    }
}
