use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::hint::black_box;
use std::num::{NonZeroU64, NonZeroUsize};
use std::rc::{Rc, Weak};
use std::sync::mpsc::{Receiver, SyncSender};

use tsonic_rust_node::background::BackgroundTasks;
use tsonic_rust_runtime::dispatch_queue::{TaskBudget, TaskReservation, TaskTicket};
use tsonic_rust_runtime::TsonicError;

#[path = "../support/allocation_counts.rs"]
mod allocation_counts;
use allocation_counts::measure;

struct Failure(TsonicError);

impl From<tsonic_rust_node::NodeError> for Failure {
    fn from(value: tsonic_rust_node::NodeError) -> Self {
        Self(value.into())
    }
}

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
    generation: NativeGeneration,
    _channel: Option<(SyncSender<TaskTicket>, Receiver<TaskTicket>)>,
    _in_flight: BTreeMap<TaskTicket, NativePending<TError>>,
    _ready: VecDeque<(u64, NativePending<TError>)>,
}

struct NativeGeneration {
    owner: Cell<Option<NonZeroUsize>>,
    epoch: Cell<Option<NonZeroU64>>,
}

impl NativeGeneration {
    fn enter(&self, owner: NonZeroUsize) -> bool {
        if self.epoch.get().is_none() || self.owner.get().is_some_and(|current| current != owner) {
            return false;
        }
        self.owner.set(Some(owner));
        true
    }

    fn close(&self, owner: NonZeroUsize) -> bool {
        if self.owner.get() != Some(owner) {
            return false;
        }
        self.owner.set(None);
        self.epoch
            .set(self.epoch.get().and_then(|epoch| epoch.checked_add(1)));
        true
    }

    fn accepts(&self, epoch: Option<NonZeroU64>) -> bool {
        self.owner.get().is_some() && epoch.is_some() && epoch == self.epoch.get()
    }
}

impl<TError> NativeRegistry<TError> {
    fn new() -> Self {
        Self {
            generation: NativeGeneration {
                owner: Cell::new(None),
                epoch: Cell::new(Some(NonZeroU64::MIN)),
            },
            _channel: None,
            _in_flight: BTreeMap::new(),
            _ready: VecDeque::new(),
        }
    }
}

struct NativeHandle<TError> {
    source: Weak<RefCell<NativeRegistry<TError>>>,
    epoch: Option<NonZeroU64>,
}

impl<TError> NativeHandle<TError> {
    fn new(source: &Rc<RefCell<NativeRegistry<TError>>>) -> Self {
        Self {
            source: Rc::downgrade(source),
            epoch: source.borrow().generation.epoch.get(),
        }
    }

    fn accepts(&self) -> bool {
        self.source
            .upgrade()
            .is_some_and(|source| source.borrow().generation.accepts(self.epoch))
    }
}

struct NativeLoop<'source, TError> {
    source: &'source Rc<RefCell<NativeRegistry<TError>>>,
    owner: NonZeroUsize,
}

impl<'source, TError> NativeLoop<'source, TError> {
    fn enter(
        source: &'source Rc<RefCell<NativeRegistry<TError>>>,
        owner: NonZeroUsize,
    ) -> Option<Self> {
        if !source.borrow().generation.enter(owner) {
            return None;
        }
        Some(Self { source, owner })
    }

    fn close(source: &Rc<RefCell<NativeRegistry<TError>>>, owner: NonZeroUsize) -> bool {
        let retired = {
            let mut source = source.borrow_mut();
            if !source.generation.close(owner) {
                return false;
            }
            (
                std::mem::take(&mut source._in_flight),
                std::mem::take(&mut source._ready),
                source._channel.take(),
            )
        };
        drop(retired);
        true
    }
}

impl<TError> Drop for NativeLoop<'_, TError> {
    fn drop(&mut self) {
        Self::close(self.source, self.owner);
    }
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
        let root = Rc::new(RefCell::new(NativeRegistry::<Failure>::new()));
        black_box(NativeHandle::new(&root));
        black_box(root);
    });
    assert_eq!(actual, expected);
    assert!(!root.has_pending_work());
}

#[test]
fn independent_native_weak_registry_preserves_generation_shutdown_and_unwind() {
    struct Capture {
        handle: NativeHandle<Failure>,
        drops: Rc<Cell<usize>>,
    }
    impl Drop for Capture {
        fn drop(&mut self) {
            assert!(!self.handle.accepts());
            let source = self.handle.source.upgrade().unwrap();
            let source = source.borrow();
            assert!(source._in_flight.is_empty());
            assert!(source._ready.is_empty());
            self.drops.set(self.drops.get() + 1);
        }
    }
    for unwind in [false, true] {
        let source = Rc::new(RefCell::new(NativeRegistry::<Failure>::new()));
        let old = NativeHandle::new(&source);
        let first = NonZeroUsize::new(1).unwrap();
        let second = NonZeroUsize::new(2).unwrap();
        let drops = Rc::new(Cell::new(0));
        let budget = TaskBudget::new(NonZeroUsize::new(1).unwrap());
        assert!(!old.accepts());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let owner = NativeLoop::enter(&source, first).unwrap();
            assert!(old.accepts());
            assert!(NativeLoop::enter(&source, second).is_none());
            let capture = Capture {
                handle: NativeHandle::new(&source),
                drops: Rc::clone(&drops),
            };
            let reservation = budget.reserve().unwrap();
            source.borrow_mut()._in_flight.insert(
                reservation.ticket(),
                NativePending {
                    _reservation: reservation,
                    _callback: Box::new(move || {
                        drop(capture);
                        Ok(())
                    }),
                },
            );
            if unwind {
                panic!("independent native owner unwind");
            }
            drop(owner);
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(drops.get(), 1);
        assert_eq!(budget.pending(), 0);
        assert!(!old.accepts());
        let next = NativeHandle::new(&source);
        let owner = NativeLoop::enter(&source, second).unwrap();
        assert!(next.accepts());
        assert!(!old.accepts());
        assert!(!NativeLoop::close(&source, first));
        assert!(next.accepts());
        drop(owner);
        assert!(!next.accepts());
        source
            .borrow()
            .generation
            .epoch
            .set(NonZeroU64::new(u64::MAX));
        let exhausted = NativeHandle::new(&source);
        drop(NativeLoop::enter(&source, first).unwrap());
        assert!(!exhausted.accepts());
        assert!(NativeLoop::enter(&source, second).is_none());
        assert_eq!(Rc::strong_count(&source), 1);
        drop(source);
        assert!(old.source.upgrade().is_none());
        assert!(next.source.upgrade().is_none());
    }
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
    use tsonic_rust_runtime::dispatch::{DispatchContexts, DispatchEnd};
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
                .prepare(tsonic_rust_node::phases::BACKGROUND)
                .unwrap_or_else(|Failure(error)| panic!("empty group cannot fail: {error}"));
            assert!(contexts.next_ready(&frontier).is_none());
            assert!(matches!(contexts.poll_next(&frontier), Ok(false)));
            assert!(!black_box(contexts.has_work()));
            assert!(contexts.next_delay().is_none());
        }
    });
    assert_eq!(cost, (0, 0));
}
