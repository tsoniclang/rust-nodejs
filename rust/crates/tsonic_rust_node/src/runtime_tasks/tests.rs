use super::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

enum Failure {
    Native(TsonicError),
    Source(Rc<Cell<i64>>),
}

impl From<TsonicError> for Failure {
    fn from(value: TsonicError) -> Self {
        Self::Native(value)
    }
}

#[test]
fn context_free_queries_do_not_allocate_a_native_task_root() {
    with_default(|tasks| {
        assert!(!tasks.has_pending_work());
        assert!(!tasks.poll().unwrap());
        assert!(tasks.queue.get().is_none());
    });
    TASK_BUDGET.with(|budget| assert!(budget.get().is_none()));
}

#[test]
fn typed_runtime_driver_preserves_original_failure_and_uninvoked_work() {
    let root = RuntimeTasks::<Failure>::new();
    let original = Rc::new(Cell::new(9_007_199_254_740_993));
    let failure = Rc::clone(&original);
    root.enqueue(move || Err(Failure::Source(failure))).unwrap();
    let observed = Rc::new(Cell::new(false));
    let callback_observed = Rc::clone(&observed);
    root.enqueue(move || {
        callback_observed.set(true);
        Ok(())
    })
    .unwrap();
    let contexts = tsonic_rust_runtime::dispatch::prepend(
        &root,
        tsonic_rust_runtime::dispatch::DispatchEnd::<Failure>::new(),
    );
    match crate::run_with_contexts(&contexts)
        .err()
        .expect("original source failure")
    {
        Failure::Source(value) => {
            assert!(Rc::ptr_eq(&value, &original));
            assert_eq!(value.get(), 9_007_199_254_740_993);
        }
        Failure::Native(error) => panic!("unexpected native failure: {error}"),
    }
    assert!(!observed.get());
    assert!(root.has_pending_work());
    assert!(crate::run_with_contexts(&contexts).is_ok());
    assert!(observed.get());
    assert!(!root.has_pending_work());
}

#[test]
fn independent_runtime_roots_share_finite_capacity_and_checked_sequence() {
    TASK_BUDGET.with(|budget| {
        assert!(budget
            .set(TaskBudget::new(NonZeroUsize::new(2).unwrap()))
            .is_ok());
    });
    let first = RuntimeTasks::<Failure>::new();
    let second = RuntimeTasks::<()>::new();
    first.enqueue(|| Ok(())).unwrap();
    second.enqueue(|| Ok(())).unwrap();
    let first_frontier = first.prepare(DispatchPhase::RuntimeTasks).ok().unwrap();
    let second_frontier = second.prepare(DispatchPhase::RuntimeTasks).ok().unwrap();
    assert!(first.next_ready(&first_frontier) < second.next_ready(&second_frontier));
    assert_eq!(
        first.enqueue(|| Ok(())).unwrap_err().code,
        "ERR_NODE_RUNTIME_TASK_LIMIT"
    );
    assert_eq!(
        second.enqueue(|| Ok(())).unwrap_err().code,
        "ERR_NODE_RUNTIME_TASK_LIMIT"
    );
    assert_eq!(first.poll().ok(), Some(true));
    second.enqueue(|| Ok(())).unwrap();
    drop(second);
    TASK_BUDGET.with(|budget| assert_eq!(budget.get().unwrap().pending(), 0));
}

#[test]
fn complete_phase_frontier_defers_cross_root_reentrant_tasks() {
    let first = RuntimeTasks::<Failure>::new();
    let second = RuntimeTasks::<Failure>::new();
    let handle = second.handle();
    let observed = Rc::new(RefCell::new(Vec::new()));
    let first_observed = Rc::clone(&observed);
    first
        .enqueue(move || {
            first_observed.borrow_mut().push(1);
            let reentrant = Rc::clone(&first_observed);
            handle
                .enqueue(move || {
                    reentrant.borrow_mut().push(3);
                    Ok(())
                })
                .unwrap();
            Ok(())
        })
        .unwrap();
    let second_observed = Rc::clone(&observed);
    second
        .enqueue(move || {
            second_observed.borrow_mut().push(2);
            Ok(())
        })
        .unwrap();
    let contexts = tsonic_rust_runtime::dispatch::prepend(
        &first,
        tsonic_rust_runtime::dispatch::prepend(
            &second,
            tsonic_rust_runtime::dispatch::DispatchEnd::<Failure>::new(),
        ),
    );
    assert_eq!(
        tsonic_rust_runtime::dispatch::poll_phase(&contexts, DispatchPhase::RuntimeTasks).ok(),
        Some(true)
    );
    assert_eq!(*observed.borrow(), [1, 2]);
    assert!(contexts.has_work());
    assert_eq!(
        tsonic_rust_runtime::dispatch::poll_phase(&contexts, DispatchPhase::RuntimeTasks).ok(),
        Some(true)
    );
    assert_eq!(*observed.borrow(), [1, 2, 3]);
    assert!(!contexts.has_work());
}

#[test]
fn callback_releases_its_borrow_and_capacity_before_reentrant_admission() {
    TASK_BUDGET.with(|budget| {
        assert!(budget
            .set(TaskBudget::new(NonZeroUsize::new(1).unwrap()))
            .is_ok());
    });
    let root = RuntimeTasks::<()>::new();
    let handle = root.handle();
    root.enqueue(move || {
        handle.enqueue(|| Ok(())).unwrap();
        Ok(())
    })
    .unwrap();
    assert_eq!(root.poll(), Ok(true));
    assert!(root.has_pending_work());
    assert_eq!(root.poll(), Ok(true));
    assert!(!root.has_pending_work());
}

#[test]
fn weak_runtime_handle_does_not_retain_owner_or_queued_callback() {
    let root = RuntimeTasks::<Failure>::new();
    let handle = root.handle();
    let retained = Rc::new(Cell::new(7));
    let callback = Rc::clone(&retained);
    root.enqueue(move || {
        callback.set(9);
        Ok(())
    })
    .unwrap();
    assert_eq!(Rc::strong_count(&retained), 2);
    drop(root);
    assert_eq!(Rc::strong_count(&retained), 1);
    assert!(matches!(
        handle.enqueue(|| Ok(())),
        Err(tsonic_rust_runtime::dispatch_queue::TaskQueueError::Closed)
    ));
    TASK_BUDGET.with(|budget| assert_eq!(budget.get().unwrap().pending(), 0));
}

#[test]
fn native_and_component_tasks_use_one_phase_order_without_reboxing() {
    let root = RuntimeTasks::<Failure>::new();
    let observed = Rc::new(RefCell::new(Vec::new()));
    let native_observed = Rc::clone(&observed);
    crate::runtime_tasks::with_default(|tasks| {
        tasks.enqueue(move || {
            native_observed.borrow_mut().push(1);
            Ok(())
        })
    })
    .unwrap();
    let typed_observed = Rc::clone(&observed);
    root.enqueue(move || {
        typed_observed.borrow_mut().push(2);
        Ok(())
    })
    .unwrap();
    let last_observed = Rc::clone(&observed);
    crate::runtime_tasks::with_default(|tasks| {
        tasks.enqueue(move || {
            last_observed.borrow_mut().push(3);
            Ok(())
        })
    })
    .unwrap();
    assert!(
        crate::run_with_contexts(tsonic_rust_runtime::dispatch::prepend(
            &root,
            tsonic_rust_runtime::dispatch::DispatchEnd::<Failure>::new()
        ))
        .is_ok()
    );
    assert_eq!(*observed.borrow(), [1, 2, 3]);
}
