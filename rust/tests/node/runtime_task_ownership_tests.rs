use std::hint::black_box;
use std::num::NonZeroUsize;
use tsonic_rust_node::runtime_tasks::RuntimeTasks;
use tsonic_rust_runtime::dispatch::{DispatchContexts, DispatchEnd};
use tsonic_rust_runtime::dispatch_queue::{TaskBudget, TaskQueue};

#[path = "../../../../rust-runtime/tests/helpers/allocation_counts.rs"]
pub mod allocation_counts;
use allocation_counts::measure_requests;

#[test]
fn cold_runtime_queries_do_not_allocate_or_initialize_native_storage() {
    let actual = measure_requests(|| {
        let root = black_box(RuntimeTasks::<()>::new());
        for _ in 0..1024 {
            assert!(!black_box(root.has_pending_work()));
            assert_eq!(root.poll(), Ok(false));
            let frontier = root
                .prepare(tsonic_rust_node::phases::RUNTIME_TASKS)
                .unwrap();
            assert_eq!(root.next_ready(&frontier), None);
        }
    });
    assert_eq!(actual, (0, 0));
}

#[test]
fn demanded_runtime_registry_matches_one_idiomatic_native_weak_queue() {
    let warm = RuntimeTasks::<()>::new();
    black_box(warm.handle());
    drop(warm);
    let root = RuntimeTasks::<()>::new();
    let actual = measure_requests(|| {
        black_box(root.handle());
    });
    let budget = TaskBudget::new(NonZeroUsize::new(1024).unwrap());
    let expected = measure_requests(|| {
        let queue = TaskQueue::<()>::new(budget.clone());
        black_box(queue.handle());
        black_box(queue);
    });
    assert_eq!(actual, expected);
}

#[test]
fn warmed_composed_runtime_dispatch_and_weak_handles_allocate_nothing() {
    let root = RuntimeTasks::<tsonic_rust_runtime::TsonicError>::new();
    tsonic_rust_node::run_entry_with_contexts(
        || {
            root.enqueue(|| Ok(())).unwrap();
            assert_eq!(root.poll(), Ok(true));
            let handle = root.handle();
            let contexts = tsonic_rust_runtime::dispatch::prepend(
                &root,
                DispatchEnd::<tsonic_rust_runtime::TsonicError>::new(),
            );
            let actual = measure_requests(|| {
                for _ in 0..1024 {
                    black_box(handle.clone());
                    let frontier = contexts
                        .prepare(tsonic_rust_node::phases::RUNTIME_TASKS)
                        .unwrap();
                    assert_eq!(contexts.next_ready(&frontier), None);
                    assert!(!contexts.has_work());
                    assert_eq!(contexts.next_delay(), None);
                    assert_eq!(contexts.poll_next(&frontier), Ok(false));
                    root.enqueue(|| Ok(())).unwrap();
                    assert_eq!(root.poll(), Ok(true));
                }
            });
            assert_eq!(actual, (0, 0));
        },
        tsonic_rust_runtime::dispatch::prepend(
            &root,
            tsonic_rust_runtime::dispatch::DispatchEnd::<tsonic_rust_runtime::TsonicError>::new(),
        ),
    )
    .unwrap();
}
