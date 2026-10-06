use super::tests::Failure;
use super::*;
use std::cell::Cell;
use tsonic_rust_runtime::dispatch::{
    poll_phase, poll_prepared, prepend, DispatchContexts, DispatchEnd, DispatchPhase,
};

enum OtherFailure {
    Original(Rc<Cell<u64>>),
    Native(NodeError),
}

impl From<NodeError> for OtherFailure {
    fn from(error: NodeError) -> Self {
        Self::Native(error)
    }
}

enum CombinedFailure {
    First(Failure),
    Second(OtherFailure),
}

struct WorkerContextReset(Option<crate::worker_threads::WorkerProcessContext>);

impl Drop for WorkerContextReset {
    fn drop(&mut self) {
        let removed =
            crate::worker_threads::WORKER_CONTEXT.with(|context| context.replace(self.0.take()));
        drop(removed);
    }
}

fn install_parent(state: Rc<RefCell<MessagePortState>>) -> WorkerContextReset {
    let previous = crate::worker_threads::WORKER_CONTEXT.with(|context| {
        context.replace(Some(crate::worker_threads::WorkerProcessContext {
            thread_id: 1,
            worker_data: JsValue::Null,
            parent_port: state,
            environment_data: std::collections::BTreeMap::new(),
        }))
    });
    WorkerContextReset(previous)
}

impl From<Failure> for CombinedFailure {
    fn from(error: Failure) -> Self {
        Self::First(error)
    }
}

impl From<OtherFailure> for CombinedFailure {
    fn from(error: OtherFailure) -> Self {
        Self::Second(error)
    }
}

fn physical_port() -> (MessagePort<NodeError>, Rc<RefCell<MessagePortState>>) {
    let resources = WorkerResources::<NodeError>::new();
    let channel = MessageChannel::new(&resources).unwrap();
    let state = Rc::clone(&channel.port2.owner.state);
    drop(channel.port2);
    (channel.port1, state)
}

#[test]
fn one_physical_port_delivers_interleaved_callbacks_to_their_exact_component_domains() {
    let (sender, state) = physical_port();
    let first = WorkerResources::<Failure>::new();
    let second = WorkerResources::<OtherFailure>::new();
    let left = first.bind_parent(Rc::clone(&state)).unwrap();
    let right = second.bind_parent(state).unwrap();
    let order = Rc::new(RefCell::new(Vec::new()));
    let left_order = Rc::clone(&order);
    let right_order = Rc::clone(&order);
    let last_order = Rc::clone(&order);
    let event = JsValue::String("message".to_owned());
    left.on_callable(
        &event,
        &Callable::new(move |()| {
            left_order.borrow_mut().push(1);
            Ok::<(), Failure>(())
        }),
    )
    .unwrap();
    right
        .on_callable(
            &event,
            &Callable::new(move |()| {
                right_order.borrow_mut().push(2);
                Ok::<(), OtherFailure>(())
            }),
        )
        .unwrap();
    left.on_callable(
        &event,
        &Callable::new(move |()| {
            last_order.borrow_mut().push(3);
            Ok::<(), Failure>(())
        }),
    )
    .unwrap();
    sender.post_message(JsValue::from(1)).unwrap();
    sender.post_message(JsValue::from(2)).unwrap();
    let group = prepend(
        &second,
        prepend(&first, DispatchEnd::<CombinedFailure>::new()),
    );
    assert_eq!(poll_phase(&group, DispatchPhase::Ports).ok(), Some(true));
    assert_eq!(*order.borrow(), [1, 2, 3, 1, 2, 3]);
    assert!(left.owner.state.borrow().messages.is_empty());
}

#[test]
fn a_shared_port_failure_retains_original_payload_and_uninvoked_once_registration() {
    let (sender, state) = physical_port();
    let first = WorkerResources::<Failure>::new();
    let second = WorkerResources::<OtherFailure>::new();
    let left = first.bind_parent(Rc::clone(&state)).unwrap();
    let right = second.bind_parent(state).unwrap();
    let original = Rc::new(Cell::new(9_007_199_254_740_993_u64));
    let captured = Rc::clone(&original);
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let event = JsValue::String("message".to_owned());
    right
        .once_callable(
            &event,
            &Callable::new(move |()| Err(OtherFailure::Original(Rc::clone(&captured)))),
        )
        .unwrap();
    left.once_callable(
        &event,
        &Callable::new(move |()| {
            observed.set(observed.get() + 1);
            Ok::<(), Failure>(())
        }),
    )
    .unwrap();
    sender.post_message(JsValue::from(1)).unwrap();
    sender.post_message(JsValue::from(2)).unwrap();
    let group = prepend(
        &first,
        prepend(&second, DispatchEnd::<CombinedFailure>::new()),
    );
    match poll_phase(&group, DispatchPhase::Ports)
        .err()
        .expect("original source failure")
    {
        CombinedFailure::Second(OtherFailure::Original(returned)) => {
            assert!(Rc::ptr_eq(&returned, &original));
            assert_eq!(returned.get(), 9_007_199_254_740_993);
        }
        CombinedFailure::Second(OtherFailure::Native(error)) => {
            panic!("source error replaced: {}", error.code())
        }
        CombinedFailure::First(Failure::Native(error)) => {
            panic!("wrong native domain: {}", error.code())
        }
        CombinedFailure::First(Failure::Original(_)) => panic!("wrong source domain"),
    }
    assert_eq!(calls.get(), 0);
    assert_eq!(left.owner.state.borrow().messages.len(), 1);
    assert_eq!(poll_phase(&group, DispatchPhase::Ports).ok(), Some(true));
    assert_eq!(calls.get(), 1);
    assert_eq!(poll_phase(&group, DispatchPhase::Ports).ok(), Some(false));
}

#[test]
fn cross_component_cancellation_retires_capture_outside_native_and_typed_borrows() {
    struct DropProbe {
        state: Weak<RefCell<MessagePortState>>,
        owner: Weak<PortOwner<NodeError>>,
        drops: Rc<Cell<usize>>,
    }
    impl Drop for DropProbe {
        fn drop(&mut self) {
            assert!(self.state.upgrade().unwrap().try_borrow_mut().is_ok());
            assert!(self
                .owner
                .upgrade()
                .unwrap()
                .callbacks
                .try_borrow_mut()
                .is_ok());
            self.drops.set(self.drops.get() + 1);
        }
    }
    let (_sender, state) = physical_port();
    let first = WorkerResources::<NodeError>::new();
    let second = WorkerResources::<NodeError>::new();
    let left = first.bind_parent(Rc::clone(&state)).unwrap();
    let right = second.bind_parent(Rc::clone(&state)).unwrap();
    let drops = Rc::new(Cell::new(0));
    let probe = DropProbe {
        state: Rc::downgrade(&state),
        owner: Rc::downgrade(&right.owner),
        drops: Rc::clone(&drops),
    };
    let callback = Callable::new(move |()| {
        std::hint::black_box(&probe);
        Ok::<(), NodeError>(())
    });
    let event = JsValue::String("message".to_owned());
    right.on_callable(&event, &callback).unwrap();
    left.off_callable(&event, &callback).unwrap();
    assert!(right.owner.callbacks.borrow().is_empty());
    drop(callback);
    assert_eq!(drops.get(), 1);
    assert_eq!(state.borrow().routing.listeners, 0);
}

#[test]
fn cancellation_during_shared_delivery_preserves_current_snapshot_but_not_the_next_message() {
    let (sender, state) = physical_port();
    let first = WorkerResources::<NodeError>::new();
    let second = WorkerResources::<NodeError>::new();
    let left = first.bind_parent(Rc::clone(&state)).unwrap();
    let right = second.bind_parent(state).unwrap();
    let order = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::clone(&order);
    let canceled = Callable::new(move |()| {
        observed.borrow_mut().push(2);
        Ok::<(), NodeError>(())
    });
    let cancel_port = left.downgrade();
    let cancel_callback = canceled.clone();
    let observed = Rc::clone(&order);
    let event = JsValue::String("message".to_owned());
    left.on_callable(
        &event,
        &Callable::new(move |()| {
            observed.borrow_mut().push(1);
            cancel_port
                .upgrade()
                .expect("live cancellation projection")
                .off_callable(&JsValue::String("message".to_owned()), &cancel_callback)?;
            Ok::<(), NodeError>(())
        }),
    )
    .unwrap();
    right.on_callable(&event, &canceled).unwrap();
    sender.post_message(JsValue::from(1)).unwrap();
    sender.post_message(JsValue::from(2)).unwrap();
    let group = prepend(&first, prepend(&second, DispatchEnd::<NodeError>::new()));
    assert!(poll_phase(&group, DispatchPhase::Ports).unwrap());
    assert_eq!(*order.borrow(), [1, 2, 1]);
    assert!(right.owner.callbacks.borrow().is_empty());
}

#[test]
fn shared_capture_defers_reentrant_messages_and_rejects_foreign_projection_frontiers() {
    let (sender, state) = physical_port();
    let first = WorkerResources::<NodeError>::new();
    let second = WorkerResources::<NodeError>::new();
    let left = first.bind_parent(Rc::clone(&state)).unwrap();
    let right = second.bind_parent(state).unwrap();
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let callback_sender = sender.clone();
    let event = JsValue::String("message".to_owned());
    left.on_callable(
        &event,
        &Callable::new(move |()| {
            observed.set(observed.get() + 1);
            if observed.get() == 1 {
                callback_sender.post_message(JsValue::from(2))?;
            }
            Ok::<(), NodeError>(())
        }),
    )
    .unwrap();
    sender.post_message(JsValue::from(1)).unwrap();
    let frontier = first.prepare(DispatchPhase::Ports).unwrap();
    assert!(second.next_ready(&frontier).is_none());
    assert!(!second.poll_next(&frontier).unwrap());
    drop(frontier);
    let group = prepend(&second, prepend(&first, DispatchEnd::<NodeError>::new()));
    let frontier = group.prepare(DispatchPhase::Ports).unwrap();
    assert!(poll_prepared(&group, &frontier).unwrap());
    assert_eq!(calls.get(), 1);
    drop(frontier);
    assert!(poll_phase(&group, DispatchPhase::Ports).unwrap());
    assert_eq!(calls.get(), 2);
}

#[test]
fn a_lazily_used_component_joins_the_existing_physical_frontier_without_eager_callback_storage() {
    let (sender, state) = physical_port();
    let _context = install_parent(Rc::clone(&state));
    let first = WorkerResources::<NodeError>::new();
    let second = Rc::new(WorkerResources::<NodeError>::new());
    let left = crate::worker_threads::parent_port(&first).unwrap().unwrap();
    let unused = second.prepare(DispatchPhase::Ports).unwrap();
    assert_eq!(state.borrow().routing.owners.iter().count(), 1);
    drop(unused);
    let order = Rc::new(RefCell::new(Vec::new()));
    let first_order = Rc::clone(&order);
    let second_order = Rc::clone(&order);
    let second_listener = Callable::new(move |()| {
        second_order.borrow_mut().push(2);
        Ok::<(), NodeError>(())
    });
    let captured_root = Rc::clone(&second);
    let event = JsValue::String("message".to_owned());
    left.on_callable(
        &event,
        &Callable::new(move |()| {
            first_order.borrow_mut().push(1);
            if first_order.borrow().len() == 1 {
                let port = crate::worker_threads::parent_port(&captured_root)?.unwrap();
                port.on_callable(&JsValue::String("message".to_owned()), &second_listener)?;
            }
            Ok::<(), NodeError>(())
        }),
    )
    .unwrap();
    sender.post_message(JsValue::from(1)).unwrap();
    sender.post_message(JsValue::from(2)).unwrap();
    let group = prepend(
        second.as_ref(),
        prepend(&first, DispatchEnd::<NodeError>::new()),
    );
    assert!(poll_phase(&group, DispatchPhase::Ports).unwrap());
    assert_eq!(*order.borrow(), [1, 1, 2]);
}

#[test]
fn callback_slots_reuse_native_storage_without_reviving_an_old_registration() {
    let resources = WorkerResources::<NodeError>::new();
    let channel = MessageChannel::new(&resources).unwrap();
    let event = JsValue::String("message".to_owned());
    let removed_calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&removed_calls);
    let removed = Callable::new(move |()| {
        observed.set(observed.get() + 1);
        Ok::<(), NodeError>(())
    });
    let replacement_calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&replacement_calls);
    let replacement = Callable::new(move |()| {
        observed.set(observed.get() + 1);
        Ok::<(), NodeError>(())
    });
    channel.port2.on_callable(&event, &removed).unwrap();
    channel.port2.off_callable(&event, &removed).unwrap();
    channel.port2.on_callable(&event, &replacement).unwrap();
    assert_eq!(channel.port2.owner.callbacks.borrow().entries.len(), 1);
    channel.port1.post_message(JsValue::from(1)).unwrap();
    assert!(poll_phase(&resources, DispatchPhase::Ports).unwrap());
    assert_eq!(removed_calls.get(), 0);
    assert_eq!(replacement_calls.get(), 1);
}

#[test]
fn dropping_a_component_retires_its_routes_without_retaining_its_capture_or_consuming_delivery() {
    struct DropProbe(Rc<Cell<usize>>);
    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let (sender, state) = physical_port();
    let survivor = WorkerResources::<NodeError>::new();
    let drops = Rc::new(Cell::new(0));
    let event = JsValue::String("message".to_owned());
    {
        let temporary = WorkerResources::<NodeError>::new();
        let port = temporary.bind_parent(Rc::clone(&state)).unwrap();
        let probe = DropProbe(Rc::clone(&drops));
        port.on_callable(
            &event,
            &Callable::new(move |()| {
                std::hint::black_box(&probe);
                Ok::<(), NodeError>(())
            }),
        )
        .unwrap();
    }
    assert_eq!(drops.get(), 1);
    let port = survivor.bind_parent(state).unwrap();
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    port.once_callable(
        &event,
        &Callable::new(move |()| {
            observed.set(observed.get() + 1);
            Ok::<(), NodeError>(())
        }),
    )
    .unwrap();
    sender.post_message(JsValue::from(1)).unwrap();
    assert!(poll_phase(&survivor, DispatchPhase::Ports).unwrap());
    assert_eq!(calls.get(), 1);
    assert_eq!(port.owner.state.borrow().routing.listeners, 0);
}

#[test]
fn port_listener_admission_and_native_errors_preserve_independent_guards() {
    let resources = WorkerResources::<NodeError>::new();
    let channel = MessageChannel::new(&resources).unwrap();
    let listener = Callable::new(|()| Ok::<(), NodeError>(()));
    assert_eq!(
        channel
            .port2
            .on_callable(&JsValue::Null, &listener)
            .err()
            .unwrap()
            .code(),
        "ERR_INVALID_ARG_TYPE"
    );
    assert!(!channel.port2.owner.state.borrow().started);
    channel.port2.owner.state.borrow_mut().routing.listeners = MAXIMUM_PORT_LISTENERS;
    assert_eq!(
        channel
            .port2
            .on_callable(&JsValue::String("message".to_owned()), &listener)
            .err()
            .unwrap()
            .code(),
        "ERR_WORKER_LISTENER_LIMIT"
    );
    assert!(channel.port2.owner.callbacks.borrow().is_empty());
    channel.port2.owner.state.borrow_mut().routing.listeners = 0;
    channel.port2.start();
    channel
        .port2
        .owner
        .state
        .borrow_mut()
        .push_error("native port failure".to_owned())
        .unwrap();
    assert_eq!(
        poll_phase(&resources, DispatchPhase::Ports)
            .err()
            .unwrap()
            .code(),
        "ERR_UNHANDLED_ERROR"
    );
    assert!(!poll_phase(&resources, DispatchPhase::Ports).unwrap());
}
