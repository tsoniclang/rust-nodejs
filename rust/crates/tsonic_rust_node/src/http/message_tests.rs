use std::cell::Cell;
use std::rc::Rc;

use tsonic_rust_runtime::{Callable, TsonicError};

use super::IncomingMessage;

#[test]
fn aborted_listener_retention_removal_and_single_fire() {
    let message = IncomingMessage::streaming(
        "POST".to_string(),
        "/upload".to_string(),
        "1.1".to_string(),
        Vec::new(),
        None,
        None,
        1024,
    )
    .unwrap();
    let fired = Rc::new(Cell::new(0));
    let removed_fired = Rc::clone(&fired);
    let removed = Callable::new(move |()| {
        removed_fired.set(removed_fired.get() + 1);
        Ok::<(), TsonicError>(())
    });
    message.on_aborted("aborted", &removed).unwrap();
    message.off_aborted("aborted", &removed).unwrap();

    let once_fired = Rc::clone(&fired);
    let once = Callable::new(move |()| {
        once_fired.set(once_fired.get() + 1);
        Ok::<(), TsonicError>(())
    });
    message.once_aborted("aborted", &once).unwrap();
    message.destroy_chain(None).unwrap();
    message.destroy_chain(None).unwrap();
    assert!(message.aborted());
    assert_eq!(fired.get(), 1);
}

enum Failure {
    Source(Rc<Cell<u64>>),
    Native(super::NodeError),
}

impl From<super::NodeError> for Failure {
    fn from(error: super::NodeError) -> Self {
        Self::Native(error)
    }
}

#[test]
fn abort_failure_leaves_physical_cleanup_and_native_terminal_error_intact() {
    let message = IncomingMessage::<Failure>::streaming(
        "POST".to_string(),
        "/upload".to_string(),
        "1.1".to_string(),
        Vec::new(),
        None,
        None,
        1024,
    )
    .unwrap();
    assert!(message
        .push_body(super::Buffer::from_bytes(vec![1, 2, 3]))
        .is_ok());
    let original = Rc::new(Cell::new(9_007_199_254_740_993));
    let captured = Rc::clone(&original);
    let abort = Callable::new(move |()| Err(Failure::Source(Rc::clone(&captured))));
    assert!(message.once_aborted("aborted", &abort).is_ok());
    let supplied: tsonic_rust_runtime::RetainedError =
        super::NodeError::new("E_ABORT", "native abort").into();
    let expected = supplied.clone();
    let errors = Rc::new(Cell::new(0));
    let observed_errors = Rc::clone(&errors);
    let error = Callable::new(move |(received,): (tsonic_rust_runtime::RetainedError,)| {
        assert!(received == expected, "native error identity");
        observed_errors.set(observed_errors.get() + 1);
        Ok::<(), Failure>(())
    });
    assert!(message.on_error("error", &error).is_ok());
    let closes = Rc::new(Cell::new(0));
    let observed_closes = Rc::clone(&closes);
    let close = Callable::new(move |()| {
        observed_closes.set(observed_closes.get() + 1);
        Ok::<(), Failure>(())
    });
    assert!(message.once_close("close", &close).is_ok());
    match message.abort(Some(supplied)) {
        Err(Failure::Source(returned)) => assert!(Rc::ptr_eq(&returned, &original)),
        Err(Failure::Native(error)) => panic!("unexpected native abort error: {}", error.code()),
        Ok(()) => panic!("source abort failure was lost"),
    }
    assert!(message.aborted());
    assert!(message.destroyed());
    assert!(message.readable.closed());
    assert_eq!(message.readable.queued_bytes(), 0);
    assert_eq!(errors.get(), 0);
    assert_eq!(closes.get(), 0);
    assert!(message.abort(None).is_ok());
    assert_eq!(errors.get(), 1);
    assert_eq!(closes.get(), 1);
    assert!(message.abort(None).is_ok());
    assert_eq!(errors.get(), 1);
    assert_eq!(closes.get(), 1);
}
