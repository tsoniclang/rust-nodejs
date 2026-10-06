use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::hint::black_box;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, SyncSender};

use tsonic_rust_node::background::BackgroundTasks;
use tsonic_rust_runtime::dispatch_queue::{TaskReservation, TaskTicket};
use tsonic_rust_runtime::TsonicError;

#[path = "../support/allocation_counts.rs"]
mod allocation_counts;
use allocation_counts::measure;

struct Failure(TsonicError);

impl From<TsonicError> for Failure {
    fn from(value: TsonicError) -> Self {
        Self(value)
    }
}

struct NativePending<TError> {
    _reservation: TaskReservation,
    _callback: Box<dyn FnOnce() -> Result<(), TError>>,
}

struct NativeRegistry<TError> {
    _sender: SyncSender<TaskTicket>,
    _receiver: Receiver<TaskTicket>,
    _in_flight: BTreeMap<TaskTicket, NativePending<TError>>,
    _ready: VecDeque<(u64, NativePending<TError>)>,
}

#[test]
fn cold_background_queries_have_no_allocation_or_native_root_initialization() {
    let cost = measure(|| {
        let root = black_box(BackgroundTasks::<Failure>::new());
        for _ in 0..1024 {
            assert!(!black_box(root.has_pending_work()));
            let ready = root.poll().unwrap_or_else(|Failure(error)| {
                panic!("empty native registry cannot fail: {error}");
            });
            assert!(!black_box(ready));
        }
    });
    assert_eq!(cost, (0, 0));
}

#[test]
fn demanded_background_registry_matches_one_idiomatic_native_weak_owner() {
    let root = BackgroundTasks::<Failure>::new();
    let actual = measure(|| {
        black_box(root.handle());
    });
    let expected = measure(|| {
        let (sender, receiver) = std::sync::mpsc::sync_channel(16 * 1024);
        let root = Rc::new(RefCell::new(NativeRegistry::<Failure> {
            _sender: sender,
            _receiver: receiver,
            _in_flight: BTreeMap::new(),
            _ready: VecDeque::new(),
        }));
        black_box(Rc::downgrade(&root));
        black_box(root);
    });
    assert_eq!(actual, expected);
    assert!(!root.has_pending_work());
}

#[test]
fn warmed_background_queries_and_weak_handle_clones_allocate_nothing() {
    let root = BackgroundTasks::<Failure>::new();
    let handle = root.handle();
    let cost = measure(|| {
        for _ in 0..1024 {
            black_box(handle.clone());
            black_box(root.handle());
            assert!(!black_box(root.has_pending_work()));
            let ready = root.poll().unwrap_or_else(|Failure(error)| {
                panic!("empty native registry cannot fail: {error}");
            });
            assert!(!black_box(ready));
        }
    });
    assert_eq!(cost, (0, 0));
}

#[test]
fn composed_background_dispatch_queries_allocate_nothing() {
    use tsonic_rust_runtime::dispatch::{DispatchContexts, DispatchEnd, DispatchPhase};
    let first = BackgroundTasks::<Failure>::new();
    let second = BackgroundTasks::<Failure>::new();
    black_box(first.handle());
    black_box(second.handle());
    let contexts = tsonic_rust_runtime::dispatch::prepend(
        &first,
        tsonic_rust_runtime::dispatch::prepend(&second, DispatchEnd::<Failure>::new()),
    );
    let cost = measure(|| {
        for _ in 0..1024 {
            let frontier = contexts
                .prepare(DispatchPhase::Background)
                .unwrap_or_else(|Failure(error)| panic!("empty group cannot fail: {error}"));
            assert!(contexts.next_ready(&frontier).is_none());
            assert!(matches!(contexts.poll_next(&frontier), Ok(false)));
            assert!(!black_box(contexts.has_work()));
            assert!(contexts.next_delay().is_none());
        }
    });
    assert_eq!(cost, (0, 0));
}
