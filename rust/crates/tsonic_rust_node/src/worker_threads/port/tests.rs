use std::cell::Cell;
use std::rc::Rc;

use super::*;
use tsonic_rust_runtime::dispatch::{poll_phase, DispatchContexts, DispatchPhase};

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
fn port_failure_retains_exact_error_and_uninvoked_messages() {
    let resources = WorkerResources::<Failure>::new();
    let channel = MessageChannel::new(&resources).unwrap();
    let original = Rc::new(Cell::new(9_007_199_254_740_993));
    let captured = Rc::clone(&original);
    let first =
        Callable::new(move |(_value,): (JsValue,)| Err(Failure::Original(Rc::clone(&captured))));
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let second = Callable::new(move |(_value,): (JsValue,)| {
        observed.set(observed.get() + 1);
        Ok::<(), Failure>(())
    });
    let event = JsValue::String("message".to_owned());
    channel.port2.once_callable1(&event, &first).unwrap();
    channel.port2.on_callable1(&event, &second).unwrap();
    channel.port1.post_message(JsValue::from(1)).unwrap();
    channel.port1.post_message(JsValue::from(2)).unwrap();
    let returned = poll_phase(&resources, DispatchPhase::Ports)
        .err()
        .expect("original port error");
    match returned {
        Failure::Original(value) => {
            assert!(Rc::ptr_eq(&value, &original));
            assert_eq!(value.get(), 9_007_199_254_740_993);
        }
        Failure::Native(error) => panic!("source error replaced: {}", error.code()),
    }
    assert_eq!(calls.get(), 0);
    assert_eq!(channel.port2.state.borrow().messages.len(), 1);
    assert_eq!(
        poll_phase(&resources, DispatchPhase::Ports).ok(),
        Some(true)
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn reentrant_port_messages_wait_for_the_next_captured_frontier() {
    let resources = WorkerResources::<Failure>::new();
    let channel = MessageChannel::new(&resources).unwrap();
    let sender = channel.port1.clone();
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let listener = Callable::new(move |(_value,): (JsValue,)| {
        observed.set(observed.get() + 1);
        if observed.get() == 1 {
            sender.post_message(JsValue::from(2))?;
        }
        Ok::<(), Failure>(())
    });
    channel
        .port2
        .on_callable1(&JsValue::String("message".to_owned()), &listener)
        .unwrap();
    channel.port1.post_message(JsValue::from(1)).unwrap();
    assert_eq!(
        poll_phase(&resources, DispatchPhase::Ports).ok(),
        Some(true)
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(
        poll_phase(&resources, DispatchPhase::Ports).ok(),
        Some(true)
    );
    assert_eq!(calls.get(), 2);
}

#[test]
fn weak_resource_root_does_not_keep_ports_or_listener_captures_alive() {
    struct DropProbe(Rc<Cell<usize>>);
    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let resources = WorkerResources::<Failure>::new();
    let drops = Rc::new(Cell::new(0));
    {
        let channel = MessageChannel::new(&resources).unwrap();
        let probe = DropProbe(Rc::clone(&drops));
        let listener = Callable::new(move |()| {
            std::hint::black_box(&probe);
            Ok::<(), Failure>(())
        });
        channel
            .port2
            .on_callable(&JsValue::String("message".to_owned()), &listener)
            .unwrap();
        assert_eq!(drops.get(), 0);
    }
    assert_eq!(drops.get(), 1);
    assert!(!resources.has_work());
    assert_eq!(
        poll_phase(&resources, DispatchPhase::Ports).ok(),
        Some(false)
    );
}

#[test]
fn all_selected_roots_capture_before_any_callback_admits_work_to_another_root() {
    use tsonic_rust_runtime::dispatch::{prepend, DispatchEnd};
    let first = WorkerResources::<Failure>::new();
    let second = WorkerResources::<Failure>::new();
    let left = MessageChannel::new(&first).unwrap();
    let right = MessageChannel::new(&second).unwrap();
    let sender = right.port1.clone();
    let left_listener = Callable::new(move |()| {
        sender.post_message(JsValue::from(2))?;
        Ok::<(), Failure>(())
    });
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let right_listener = Callable::new(move |()| {
        observed.set(observed.get() + 1);
        Ok::<(), Failure>(())
    });
    let event = JsValue::String("message".to_owned());
    left.port2.once_callable(&event, &left_listener).unwrap();
    right.port2.on_callable(&event, &right_listener).unwrap();
    left.port1.post_message(JsValue::from(1)).unwrap();
    right.port1.post_message(JsValue::from(1)).unwrap();
    let group = prepend(&first, prepend(&second, DispatchEnd::<Failure>::new()));
    assert_eq!(poll_phase(&group, DispatchPhase::Ports).ok(), Some(true));
    assert_eq!(calls.get(), 1);
    assert_eq!(poll_phase(&group, DispatchPhase::Ports).ok(), Some(true));
    assert_eq!(calls.get(), 2);
}

#[test]
fn empty_port_receive_uses_the_single_native_absence_state() {
    let resources = WorkerResources::<Failure>::new();
    let channel = MessageChannel::new(&resources).unwrap();
    assert_eq!(
        crate::worker_threads::receive_message_on_port(&channel.port2).unwrap(),
        JsValue::Null
    );
}
