use super::*;
use std::cell::Cell;

enum Failure {
    Source(Rc<Cell<u64>>),
    Native(NodeError),
}

impl From<NodeError> for Failure {
    fn from(error: NodeError) -> Self {
        Self::Native(error)
    }
}

fn failing_callback(original: &Rc<Cell<u64>>) -> Callable<(), Result<(), Failure>> {
    let original = Rc::clone(original);
    Callable::new(move |()| Err(Failure::Source(Rc::clone(&original))))
}

fn assert_original(result: Result<(), Failure>, original: &Rc<Cell<u64>>) {
    match result {
        Err(Failure::Source(returned)) => assert!(Rc::ptr_eq(&returned, original)),
        _ => panic!("original source failure was not preserved"),
    }
}

fn close_counter(calls: &Rc<Cell<usize>>) -> Callable<(), Result<(), Failure>> {
    let calls = Rc::clone(calls);
    Callable::new(move |()| {
        calls.set(calls.get() + 1);
        Ok(())
    })
}

#[test]
fn finish_failure_commits_physical_closure_and_preserves_later_close_delivery() {
    let original = Rc::new(Cell::new(9_007_199_254_740_993));
    let writable = Writable::<Failure>::new();
    let closes = Rc::new(Cell::new(0));
    assert!(writable
        .once_finish("finish", &failing_callback(&original))
        .is_ok());
    assert!(writable
        .once_close("close", &close_counter(&closes))
        .is_ok());
    assert_original(writable.end().map(|_| ()), &original);
    assert!(writable.writable_finished());
    assert!(writable.closed());
    assert_eq!(closes.get(), 0);
    assert!(writable.end().is_ok());
    assert_eq!(closes.get(), 1);
    assert!(writable.end().is_ok());
    assert_eq!(closes.get(), 1);
}

#[test]
fn end_failure_commits_physical_closure_without_repeating_end_callbacks() {
    let original = Rc::new(Cell::new(9_007_199_254_740_993));
    let readable = Readable::<Failure>::from_chunks(Vec::new());
    let closes = Rc::new(Cell::new(0));
    assert!(readable
        .once_end("end", &failing_callback(&original))
        .is_ok());
    assert!(readable
        .once_close("close", &close_counter(&closes))
        .is_ok());
    assert_original(readable.read().map(|_| ()), &original);
    assert!(readable.closed());
    assert_eq!(closes.get(), 0);
    assert!(matches!(readable.read(), Ok(None)));
    assert_eq!(closes.get(), 1);
    assert!(matches!(readable.read(), Ok(None)));
    assert_eq!(closes.get(), 1);
}

#[test]
fn first_data_failure_preserves_the_uninvoked_once_registration() {
    let readable = Readable::<Failure>::default();
    let original = Rc::new(Cell::new(7));
    let captured = Rc::clone(&original);
    let first =
        Callable::new(move |(_chunk,): (Buffer,)| Err(Failure::Source(Rc::clone(&captured))));
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let second = Callable::new(move |(chunk,): (Buffer,)| {
        assert_eq!(chunk.len(), 2);
        observed.set(observed.get() + 1);
        Ok::<(), Failure>(())
    });
    assert!(readable.once_data("data", &first).is_ok());
    assert!(readable.once_data("data", &second).is_ok());
    assert_original(
        readable.push(Buffer::from_bytes(vec![1])).map(|_| ()),
        &original,
    );
    assert_eq!(calls.get(), 0);
    assert!(readable.push(Buffer::from_bytes(vec![2, 3])).is_ok());
    assert_eq!(calls.get(), 1);
    assert!(readable.push(Buffer::from_bytes(vec![4])).is_ok());
    assert_eq!(calls.get(), 1);
    assert_eq!(
        readable.read().ok().flatten().map(|chunk| chunk.len()),
        Some(1)
    );
}

#[test]
fn stream_snapshots_share_direct_callable_storage_without_an_emission_allocation() {
    let readable = Readable::<Failure>::default();
    let callback = Callable::new(|(_chunk,): (Buffer,)| Ok::<(), Failure>(()));
    assert!(readable.on_data("data", &callback).is_ok());
    let state = readable.state.borrow();
    let snapshot = state.data_event.emission();
    let listeners = state
        .data_event
        .listeners
        .as_ref()
        .expect("listener storage");
    assert!(Rc::ptr_eq(
        listeners,
        snapshot.listeners.as_ref().expect("same native snapshot")
    ));
    assert!(matches!(&listeners[0].callback,
        RetainedListener::Repeated(StreamCallback::Value(stored)) if Callable::same(stored, &callback)));
}

#[test]
fn corked_writes_keep_later_native_chunks_after_the_first_source_failure() {
    let transform = Transform::<Failure>::new(|chunk| chunk);
    let readable = transform.readable_handle();
    let writable = transform.writable_handle();
    let original = Rc::new(Cell::new(11));
    let captured = Rc::clone(&original);
    let failure =
        Callable::new(move |(_chunk,): (Buffer,)| Err(Failure::Source(Rc::clone(&captured))));
    assert!(readable.once_data("data", &failure).is_ok());
    writable.cork();
    assert!(writable.write(Buffer::from_bytes(vec![1])).is_ok());
    assert!(writable.write(Buffer::from_bytes(vec![2])).is_ok());
    assert_original(writable.uncork(), &original);
    assert_eq!(writable.state.borrow().corked_chunks.len(), 1);
    assert!(writable.uncork().is_ok());
    assert_eq!(
        readable.read().ok().flatten().map(|chunk| chunk.len()),
        Some(1)
    );
    assert!(writable.end().is_ok());
    assert!(writable.closed());
}

#[test]
fn native_stream_guards_are_not_formatted_source_errors() {
    let writable = Writable::<Failure>::new();
    assert!(writable.end().is_ok());
    let error = writable
        .write_string("after end")
        .err()
        .expect("native stream guard");
    assert!(
        matches!(error, Failure::Native(error) if error.code() == "ERR_STREAM_WRITE_AFTER_END")
    );
}

struct CapacityDrop {
    readable: WeakReadable<Failure>,
    calls: Rc<Cell<usize>>,
}

impl Drop for CapacityDrop {
    fn drop(&mut self) {
        if let Some(readable) = self.readable.upgrade() {
            assert!(!readable.is_paused());
            readable.pause();
        }
        self.calls.set(self.calls.get() + 1);
    }
}

#[test]
fn replacing_a_native_capacity_owner_releases_its_capture_outside_stream_borrows() {
    let readable = Readable::<Failure>::default();
    let calls = Rc::new(Cell::new(0));
    let owner = CapacityDrop {
        readable: readable.downgrade(),
        calls: Rc::clone(&calls),
    };
    readable.set_capacity_handler(move || {
        let _capture = &owner;
        Ok(())
    });
    readable.set_capacity_handler(|| Ok(()));
    assert_eq!(calls.get(), 1);
    assert!(readable.is_paused());
}

struct ReentrantFailure(NodeError);

thread_local! {
    static REENTRANT_WRITABLE: RefCell<Option<Weak<RefCell<WritableState<ReentrantFailure>>>>> = const { RefCell::new(None) };
}

impl From<NodeError> for ReentrantFailure {
    fn from(error: NodeError) -> Self {
        REENTRANT_WRITABLE.with(|owner| {
            let state = owner
                .borrow()
                .as_ref()
                .and_then(Weak::upgrade)
                .expect("native writable owner");
            let state = state
                .try_borrow_mut()
                .expect("native guard conversion outside owner borrow");
            assert!(state.finished);
        });
        Self(error)
    }
}

#[test]
fn native_writable_guards_release_storage_before_entering_the_selected_error_domain() {
    let writable = Writable::<ReentrantFailure>::new();
    assert!(writable.end().is_ok());
    REENTRANT_WRITABLE.with(|owner| *owner.borrow_mut() = Some(Rc::downgrade(&writable.state)));
    for result in [
        writable.write_string("after end").map(|_| ()),
        writable.flush_backend(),
    ] {
        match result {
            Err(ReentrantFailure(error)) => assert_eq!(error.code(), "ERR_STREAM_WRITE_AFTER_END"),
            Ok(()) => panic!("native write-after-end guard lost"),
        }
    }
    REENTRANT_WRITABLE.with(|owner| *owner.borrow_mut() = None);
}
