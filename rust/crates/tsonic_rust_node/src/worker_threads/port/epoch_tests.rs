use super::*;
use std::cell::Cell;
use tsonic_rust_runtime::dispatch::{poll_phase, prepend, DispatchContexts, DispatchEnd};
use tsonic_rust_runtime::TsonicError;

fn in_loop<T>(
    resources: &WorkerResources<NodeError>,
    body: impl std::future::Future<Output = T>,
) -> T {
    crate::block_on_with_contexts(body, prepend(resources, DispatchEnd::<TsonicError>::new()))
        .unwrap()
        .0
        .unwrap()
}

#[test]
fn native_channel_construction_and_exact_post_receive_need_no_js_loop() {
    assert!(tsonic_rust_js::event_loop::ensure_running().is_err());
    let resources = WorkerResources::<NodeError>::new();
    let channel = MessageChannel::new(&resources).unwrap();
    for value in [9_007_199_254_740_993_i64, i64::MAX, i64::MIN] {
        channel.port1.post_message(JsValue::Integer(value)).unwrap();
        assert_eq!(
            channel.port2.receive_message().unwrap(),
            Some(JsValue::Integer(value))
        );
    }
    assert!(!resources.has_work());
    assert!(tsonic_rust_js::event_loop::ensure_running().is_err());
    channel.port1.close().unwrap();
    assert_eq!(
        channel
            .port1
            .post_message(JsValue::Integer(1))
            .unwrap_err()
            .code(),
        "ERR_CLOSED_MESSAGE_PORT"
    );
}

#[test]
fn local_callback_captures_retire_while_physical_channel_survives() {
    let resources = WorkerResources::<NodeError>::new();
    let captured = Rc::new(Cell::new(0));
    let witness = Rc::downgrade(&captured);
    let channel = in_loop(&resources, async {
        let channel = MessageChannel::new(&resources).unwrap();
        let callback = Callable::new(move |()| {
            captured.set(captured.get() + 1);
            Ok::<(), NodeError>(())
        });
        channel
            .port2
            .on_callable(&JsValue::String("message".to_owned()), &callback)
            .unwrap();
        channel.port1.unref();
        channel.port2.unref();
        channel
    });
    assert!(
        witness.upgrade().is_none(),
        "retired callback must release its capture"
    );
    assert!(channel.port2.owner.callbacks.borrow().is_empty());
    assert!(!channel.port2.owner.state.borrow().closed);
    channel
        .port1
        .post_message(JsValue::Integer(9_007_199_254_740_993))
        .unwrap();
    assert_eq!(
        channel.port2.receive_message().unwrap(),
        Some(JsValue::Integer(9_007_199_254_740_993))
    );
}

#[test]
fn only_fresh_subscription_can_deliver_after_a_new_loop_generation() {
    let resources = WorkerResources::<NodeError>::new();
    let old_calls = Rc::new(Cell::new(0));
    let channel = in_loop(&resources, async {
        let channel = MessageChannel::new(&resources).unwrap();
        let observed = Rc::clone(&old_calls);
        channel
            .port2
            .on_callable(
                &JsValue::String("message".to_owned()),
                &Callable::new(move |()| {
                    observed.set(observed.get() + 1);
                    Ok::<(), NodeError>(())
                }),
            )
            .unwrap();
        channel.port1.unref();
        channel.port2.unref();
        channel
    });
    channel
        .port1
        .post_message(JsValue::Integer(i64::MAX))
        .unwrap();
    in_loop(&resources, async {
        assert!(!poll_phase(&resources, crate::phases::PORTS).unwrap());
        assert_eq!(channel.port2.owner.state.borrow().messages.len(), 1);
    });
    let delivered = Rc::new(RefCell::new(Vec::new()));
    in_loop(&resources, async {
        let values = Rc::clone(&delivered);
        channel
            .port2
            .on_callable1(
                &JsValue::String("message".to_owned()),
                &Callable::new(move |(value,)| {
                    values.borrow_mut().push(value);
                    Ok::<(), NodeError>(())
                }),
            )
            .unwrap();
        assert!(poll_phase(&resources, crate::phases::PORTS).unwrap());
        assert_eq!(delivered.borrow().as_slice(), [JsValue::Integer(i64::MAX)]);
    });
    assert_eq!(old_calls.get(), 0);
    assert_eq!(Rc::strong_count(&delivered), 1);
}

#[test]
fn aborted_loop_releases_local_captures_and_preserves_native_data() {
    let resources = WorkerResources::<NodeError>::new();
    let mut retained = None;
    let capture = Rc::new(());
    let witness = Rc::downgrade(&capture);
    let aborted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        in_loop(&resources, async {
            let channel = MessageChannel::new(&resources).unwrap();
            channel
                .port2
                .on_callable(
                    &JsValue::String("message".to_owned()),
                    &Callable::new(move |()| {
                        std::hint::black_box(&capture);
                        Ok::<(), NodeError>(())
                    }),
                )
                .unwrap();
            retained = Some(channel);
            panic!("original loop abort");
        });
    }));
    assert!(aborted.is_err());
    assert!(
        witness.upgrade().is_none(),
        "abort must retire callback captures"
    );
    let channel = retained.unwrap();
    channel
        .port1
        .post_message(JsValue::Integer(i64::MIN))
        .unwrap();
    assert_eq!(
        channel.port2.receive_message().unwrap(),
        Some(JsValue::Integer(i64::MIN))
    );
    assert!(!resources.has_work());
}

#[test]
fn callback_activation_rejects_missing_or_unselected_owner_before_mutation() {
    let resources = WorkerResources::<NodeError>::new();
    let foreign = WorkerResources::<NodeError>::new();
    let channel = MessageChannel::new(&resources).unwrap();
    let listener = Callable::new(|()| Ok::<(), NodeError>(()));
    let event = JsValue::String("message".to_owned());
    assert_eq!(
        channel.port2.start().unwrap_err().code(),
        "ERR_NODE_EVENT_LOOP"
    );
    assert_eq!(
        channel
            .port2
            .on_callable(&event, &listener)
            .err()
            .unwrap()
            .code(),
        "ERR_NODE_EVENT_LOOP"
    );
    assert!(!channel.port2.owner.state.borrow().started);
    assert!(channel.port2.owner.callbacks.borrow().is_empty());
    assert_eq!(
        channel
            .port2
            .owner
            .state
            .borrow()
            .routing
            .owners
            .iter()
            .count(),
        0
    );
    in_loop(&foreign, async {
        assert_eq!(
            channel.port2.start().unwrap_err().code(),
            "ERR_NODE_RESOURCE_CLOSED"
        );
        assert_eq!(
            channel
                .port2
                .on_callable(&event, &listener)
                .err()
                .unwrap()
                .code(),
            "ERR_NODE_RESOURCE_CLOSED"
        );
        assert!(!channel.port2.owner.state.borrow().started);
        assert!(channel.port2.owner.callbacks.borrow().is_empty());
        assert_eq!(
            channel
                .port2
                .owner
                .state
                .borrow()
                .routing
                .owners
                .iter()
                .count(),
            0
        );
    });
    in_loop(&resources, async {
        channel.port2.start().unwrap();
        assert!(channel.port2.has_ref());
        channel.port2.unref();
        assert!(!channel.port2.has_ref());
        channel.port2.ref_chain();
        assert!(channel.port2.has_ref());
        channel.port2.unref();
    });
    assert_eq!(
        channel.port2.start().unwrap_err().code(),
        "ERR_NODE_EVENT_LOOP"
    );
    assert!(!resources.has_work());
}

#[test]
fn stale_parent_frontier_cannot_consume_fresh_data_or_retire_a_new_capture() {
    let channels = WorkerResources::<NodeError>::new();
    let channel = MessageChannel::new(&channels).unwrap();
    let resources = WorkerResources::<NodeError>::new();
    let port = resources
        .bind_parent(channel.port2.physical_state())
        .unwrap();
    let old_calls = Rc::new(Cell::new(0));
    let (stale_owner, stale_frontier) = in_loop(&resources, async {
        let observed = Rc::clone(&old_calls);
        port.on_callable(
            &JsValue::String("message".to_owned()),
            &Callable::new(move |()| {
                observed.set(observed.get() + 1);
                Ok::<(), NodeError>(())
            }),
        )
        .unwrap();
        port.unref();
        channel
            .port1
            .post_message(JsValue::Integer(i64::MIN))
            .unwrap();
        (
            tsonic_rust_js::event_loop::dispatch_owner().unwrap(),
            resources.prepare(crate::phases::PORTS).unwrap(),
        )
    });
    assert_eq!(
        port.receive_message().unwrap(),
        Some(JsValue::Integer(i64::MIN))
    );
    channel
        .port1
        .post_message(JsValue::Integer(i64::MAX))
        .unwrap();
    let calls = Rc::new(Cell::new(0));
    in_loop(&resources, async {
        assert!(!resources.enter(stale_owner));
        resources.close(stale_owner);
        assert!(resources.next_ready(&stale_frontier).is_none());
        assert!(!resources.poll_next(&stale_frontier).unwrap());
        let observed = Rc::clone(&calls);
        port.on_callable1(
            &JsValue::String("message".to_owned()),
            &Callable::new(move |(value,)| {
                assert_eq!(value, JsValue::Integer(i64::MAX));
                observed.set(observed.get() + 1);
                Ok::<(), NodeError>(())
            }),
        )
        .unwrap();
        let current = resources.prepare(crate::phases::PORTS).unwrap();
        assert_eq!(port.owner.state.borrow().capture_count, 1);
        drop(stale_frontier);
        assert_eq!(port.owner.state.borrow().capture_count, 1);
        assert!(tsonic_rust_runtime::dispatch::poll_prepared(&resources, &current).unwrap());
        assert_eq!(calls.get(), 1);
        drop(current);
        assert_eq!(port.owner.state.borrow().capture_count, 0);
    });
    assert_eq!(old_calls.get(), 0);
    assert_eq!(Rc::strong_count(&calls), 1);
}

#[test]
fn local_retirement_releases_a_callback_that_captures_its_own_port() {
    let resources = WorkerResources::<NodeError>::new();
    let capture = Rc::new(());
    let witness = Rc::downgrade(&capture);
    let weak_port = in_loop(&resources, async {
        let channel = MessageChannel::new(&resources).unwrap();
        let captured_port = channel.port2.clone();
        let weak_port = Rc::downgrade(&channel.port2.owner);
        channel
            .port2
            .on_callable(
                &JsValue::String("message".to_owned()),
                &Callable::new(move |()| {
                    std::hint::black_box(&captured_port);
                    std::hint::black_box(&capture);
                    Ok::<(), NodeError>(())
                }),
            )
            .unwrap();
        channel.port2.unref();
        weak_port
    });
    assert!(witness.upgrade().is_none());
    assert!(weak_port.upgrade().is_none());
    assert!(!resources.has_work());
}

#[test]
fn retained_channel_reuses_fresh_callback_generations_without_resurrection() {
    let resources = WorkerResources::<NodeError>::new();
    let channel = MessageChannel::new(&resources).unwrap();
    let delivered = Rc::new(RefCell::new(Vec::new()));
    channel.port2.unref();
    for generation in 0..128_i64 {
        let value = 9_007_199_254_740_993 + generation;
        channel.port1.post_message(JsValue::Integer(value)).unwrap();
        in_loop(&resources, async {
            let values = Rc::clone(&delivered);
            channel
                .port2
                .once_callable1(
                    &JsValue::String("message".to_owned()),
                    &Callable::new(move |(payload,)| {
                        values.borrow_mut().push(payload);
                        Ok::<(), NodeError>(())
                    }),
                )
                .unwrap();
            assert!(poll_phase(&resources, crate::phases::PORTS).unwrap());
            assert!(!poll_phase(&resources, crate::phases::PORTS).unwrap());
            assert_eq!(delivered.borrow().last(), Some(&JsValue::Integer(value)));
        });
        assert!(channel.port2.owner.callbacks.borrow().is_empty());
        assert_eq!(Rc::strong_count(&delivered), 1);
    }
    assert_eq!(delivered.borrow().len(), 128);
    channel.port1.close().unwrap();
    assert_eq!(
        channel
            .port1
            .post_message(JsValue::Null)
            .unwrap_err()
            .code(),
        "ERR_CLOSED_MESSAGE_PORT"
    );
}

#[test]
fn callback_abort_retires_the_emission_but_preserves_unconsumed_native_messages() {
    let resources = WorkerResources::<NodeError>::new();
    let channel = MessageChannel::new(&resources).unwrap();
    let capture = Rc::new(());
    let witness = Rc::downgrade(&capture);
    let old_calls = Rc::new(Cell::new(0));
    channel.port2.unref();
    let aborted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        in_loop(&resources, async {
            let calls = Rc::clone(&old_calls);
            channel
                .port2
                .on_callable(
                    &JsValue::String("message".to_owned()),
                    &Callable::new(move |()| -> NodeResult<()> {
                        calls.set(calls.get() + 1);
                        std::hint::black_box(&capture);
                        panic!("original port callback abort");
                    }),
                )
                .unwrap();
            channel
                .port1
                .post_message(JsValue::Integer(9_007_199_254_740_993))
                .unwrap();
            channel
                .port1
                .post_message(JsValue::Integer(i64::MAX))
                .unwrap();
            let _ = poll_phase(&resources, crate::phases::PORTS);
        });
    }));
    assert!(aborted.is_err());
    assert_eq!(old_calls.get(), 1);
    assert!(witness.upgrade().is_none());
    assert!(channel.port2.owner.state.borrow().routing.active.is_none());
    channel
        .port1
        .post_message(JsValue::Integer(i64::MIN))
        .unwrap();
    let delivered = Rc::new(RefCell::new(Vec::new()));
    in_loop(&resources, async {
        let values = Rc::clone(&delivered);
        channel
            .port2
            .on_callable1(
                &JsValue::String("message".to_owned()),
                &Callable::new(move |(value,)| {
                    values.borrow_mut().push(value);
                    Ok::<(), NodeError>(())
                }),
            )
            .unwrap();
        assert!(poll_phase(&resources, crate::phases::PORTS).unwrap());
        assert_eq!(
            delivered.borrow().as_slice(),
            [JsValue::Integer(i64::MAX), JsValue::Integer(i64::MIN)]
        );
    });
    assert_eq!(old_calls.get(), 1);
}

#[test]
fn retired_native_channel_keeps_exact_fifo_and_rejects_queue_overflow_atomically() {
    let resources = WorkerResources::<NodeError>::new();
    let channel = MessageChannel::new(&resources).unwrap();
    channel.port2.unref();
    in_loop(&resources, async {
        channel.port2.start().unwrap();
    });
    for index in 0..MAXIMUM_QUEUED_MESSAGES {
        channel
            .port1
            .post_message(JsValue::Integer(9_007_199_254_740_993 + index as i64))
            .unwrap();
    }
    assert_eq!(
        channel
            .port1
            .post_message(JsValue::Integer(-1))
            .unwrap_err()
            .code(),
        "ERR_WORKER_MESSAGE_QUEUE_LIMIT"
    );
    assert_eq!(
        channel.port2.owner.state.borrow().messages.len(),
        MAXIMUM_QUEUED_MESSAGES
    );
    let calls = Rc::new(Cell::new(0));
    in_loop(&resources, async {
        let observed = Rc::clone(&calls);
        channel
            .port2
            .on_callable1(
                &JsValue::String("message".to_owned()),
                &Callable::new(move |(value,)| {
                    assert_eq!(
                        value,
                        JsValue::Integer(9_007_199_254_740_993 + observed.get() as i64)
                    );
                    observed.set(observed.get() + 1);
                    Ok::<(), NodeError>(())
                }),
            )
            .unwrap();
        assert!(poll_phase(&resources, crate::phases::PORTS).unwrap());
        assert!(!poll_phase(&resources, crate::phases::PORTS).unwrap());
    });
    assert_eq!(calls.get(), MAXIMUM_QUEUED_MESSAGES);
    assert_eq!(Rc::strong_count(&calls), 1);
    assert_eq!(channel.port2.receive_message().unwrap(), None);
}

#[test]
fn root_retirement_closes_all_callback_admission_before_releasing_user_captures() {
    struct RetireProbe {
        other: MessagePort<NodeError>,
        owner: Weak<PortOwner<NodeError>>,
        state: Weak<RefCell<MessagePortState>>,
        drops: Rc<Cell<usize>>,
    }
    impl Drop for RetireProbe {
        fn drop(&mut self) {
            assert!(self
                .owner
                .upgrade()
                .unwrap()
                .callbacks
                .try_borrow_mut()
                .is_ok());
            assert!(self.state.upgrade().unwrap().try_borrow_mut().is_ok());
            assert_eq!(
                self.other.start().unwrap_err().code(),
                "ERR_NODE_RESOURCE_CLOSED"
            );
            self.drops.set(self.drops.get() + 1);
        }
    }
    let resources = WorkerResources::<NodeError>::new();
    let channel = MessageChannel::new(&resources).unwrap();
    let drops = Rc::new(Cell::new(0));
    in_loop(&resources, async {
        let probe = RetireProbe {
            other: channel.port2.clone(),
            owner: Rc::downgrade(&channel.port1.owner),
            state: Rc::downgrade(&channel.port1.owner.state),
            drops: Rc::clone(&drops),
        };
        channel
            .port1
            .on_callable(
                &JsValue::String("message".to_owned()),
                &Callable::new(move |()| {
                    std::hint::black_box(&probe);
                    Ok::<(), NodeError>(())
                }),
            )
            .unwrap();
        channel.port2.start().unwrap();
        let owner = tsonic_rust_js::event_loop::dispatch_owner().unwrap();
        resources.close(owner);
        assert_eq!(drops.get(), 1);
        assert!(!resources.has_work());
        resources.close(owner);
        assert_eq!(drops.get(), 1);
    });
    channel
        .port1
        .post_message(JsValue::Integer(i64::MAX))
        .unwrap();
    assert_eq!(
        channel.port2.receive_message().unwrap(),
        Some(JsValue::Integer(i64::MAX))
    );
}

#[test]
fn reopened_root_in_the_same_loop_cannot_reuse_a_retired_physical_frontier() {
    let channels = WorkerResources::<NodeError>::new();
    let channel = MessageChannel::new(&channels).unwrap();
    let resources = WorkerResources::<NodeError>::new();
    let port = resources
        .bind_parent(channel.port2.physical_state())
        .unwrap();
    port.unref();
    in_loop(&resources, async {
        port.start().unwrap();
        channel
            .port1
            .post_message(JsValue::Integer(i64::MIN))
            .unwrap();
        let stale = resources.prepare(crate::phases::PORTS).unwrap();
        assert_eq!(
            port.receive_message().unwrap(),
            Some(JsValue::Integer(i64::MIN))
        );
        let owner = tsonic_rust_js::event_loop::dispatch_owner().unwrap();
        resources.close(owner);
        assert!(resources.enter(owner));
        let calls = Rc::new(Cell::new(0));
        let observed = Rc::clone(&calls);
        port.on_callable1(
            &JsValue::String("message".to_owned()),
            &Callable::new(move |(value,)| {
                assert_eq!(value, JsValue::Integer(i64::MAX));
                observed.set(observed.get() + 1);
                Ok::<(), NodeError>(())
            }),
        )
        .unwrap();
        channel
            .port1
            .post_message(JsValue::Integer(i64::MAX))
            .unwrap();
        let current = resources.prepare(crate::phases::PORTS).unwrap();
        assert_eq!(port.owner.state.borrow().capture_count, 1);
        assert!(!resources.poll_next(&stale).unwrap());
        drop(stale);
        assert_eq!(port.owner.state.borrow().capture_count, 1);
        assert!(tsonic_rust_runtime::dispatch::poll_prepared(&resources, &current).unwrap());
        assert_eq!(calls.get(), 1);
        drop(current);
        assert_eq!(port.owner.state.borrow().capture_count, 0);
    });
}

#[test]
fn physical_frontier_generation_exhaustion_fails_before_native_queue_mutation() {
    let channels = WorkerResources::<NodeError>::new();
    let channel = MessageChannel::new(&channels).unwrap();
    let resources = WorkerResources::<NodeError>::new();
    let port = resources
        .bind_parent(channel.port2.physical_state())
        .unwrap();
    port.unref();
    in_loop(&resources, async {
        port.start().unwrap();
        let original = port.owner.state.borrow().capture_epoch;
        port.owner.state.borrow_mut().capture_epoch = u64::MAX;
        channel
            .port1
            .post_message(JsValue::Integer(i64::MAX))
            .unwrap();
        assert_eq!(
            resources
                .prepare(crate::phases::PORTS)
                .err()
                .unwrap()
                .code(),
            "ERR_WORKER_CAPTURE_LIMIT"
        );
        assert_eq!(port.owner.state.borrow().capture_epoch, u64::MAX);
        assert_eq!(port.owner.state.borrow().capture_count, 0);
        assert!(port.owner.state.borrow().capture_owner.is_none());
        assert_eq!(port.owner.state.borrow().messages.len(), 1);
        port.owner.state.borrow_mut().capture_epoch = original;
    });
    assert_eq!(
        port.receive_message().unwrap(),
        Some(JsValue::Integer(i64::MAX))
    );
}
