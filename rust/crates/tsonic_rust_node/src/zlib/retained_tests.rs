use super::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tsonic_rust_runtime::Callable;

enum Failure {
    Original(Rc<Cell<u64>>),
    Native(NodeError),
}
impl From<NodeError> for Failure {
    fn from(error: NodeError) -> Self {
        Self::Native(error)
    }
}

#[test]
fn native_codec_projections_retain_the_original_codec_without_a_reference_cycle() {
    for keep_writable in [false, true] {
        let codec = Zlib::<NodeError>::new(ZlibMode::Gzip, None);
        let owner = Rc::downgrade(&codec.state);
        let readable = zlib_as_readable(&codec);
        let writable = zlib_as_writable(&codec);
        let readable_owner = readable.downgrade();
        drop(codec);
        if keep_writable {
            drop(readable);
            assert!(owner.upgrade().is_some());
            writable.write_string("native alias").unwrap();
            writable.end().unwrap();
            drop(writable);
        } else {
            drop(writable);
            assert!(owner.upgrade().is_some());
            readable.destroy().unwrap();
            assert!(owner.upgrade().is_none_or(|state| state.borrow().closed));
            drop(readable);
        }
        assert!(readable_owner.upgrade().is_none());
        assert!(owner.upgrade().is_none());
    }
}

#[test]
fn native_codec_finalization_is_not_repeated_after_the_original_end_failure() {
    let codec = Zlib::<Failure>::new(ZlibMode::Gzip, None);
    let readable = zlib_as_readable(&codec);
    let writable = zlib_as_writable(&codec);
    let received = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::clone(&received);
    let data = Callable::new(move |(chunk,): (Buffer,)| {
        chunk.with_bytes(|bytes| observed.borrow_mut().extend_from_slice(bytes));
        Ok::<(), Failure>(())
    });
    assert!(readable.on_data("data", &data).is_ok());
    let original = Rc::new(Cell::new(9_007_199_254_740_993));
    let captured = Rc::clone(&original);
    let end = Callable::new(move |()| Err(Failure::Original(Rc::clone(&captured))));
    assert!(readable.once_end("end", &end).is_ok());
    assert!(writable.write_string("exact native output").is_ok());
    match writable.end() {
        Err(Failure::Original(returned)) => assert!(Rc::ptr_eq(&returned, &original)),
        _ => panic!("end failure was not preserved"),
    }
    assert!(codec.closed());
    assert!(writable.writable_finished());
    assert!(writable.end().is_ok());
    assert!(writable.closed());
    let compressed = Buffer::from_bytes(received.borrow().clone());
    assert_eq!(
        gunzip_sync(&compressed)
            .unwrap()
            .to_string(Some("utf8"))
            .unwrap(),
        "exact native output"
    );
}

#[test]
fn native_codec_guards_still_enter_the_producing_failure_domain() {
    let codec = Zlib::<Failure>::new(ZlibMode::Gunzip, None);
    let error = codec
        .write(Buffer::from_bytes(vec![255; 32]))
        .err()
        .expect("invalid compressed input");
    assert!(matches!(error, Failure::Native(error) if error.code() == "Z_DATA_ERROR"));
}

#[test]
fn native_codec_capacity_retains_pending_output_and_publishes_eof_after_the_last_chunk() {
    let codec = Zlib::<NodeError>::new(
        ZlibMode::Gzip,
        Some(ZlibOptions {
            chunk_size: 1,
            ..Default::default()
        }),
    );
    let readable = zlib_as_readable(&codec);
    let writable = zlib_as_writable(&codec);
    writable
        .write_string("bounded output across native read pressure")
        .unwrap();
    writable.end().unwrap();
    assert!(writable.writable_finished());
    assert!(readable.pressured());
    assert!(!readable.readable_ended());
    assert!(codec.state.borrow().output.borrow().pending_bytes > 0);
    let mut bytes = Vec::new();
    while let Some(chunk) = readable.read().unwrap() {
        chunk.with_bytes(|chunk| bytes.extend_from_slice(chunk));
    }
    assert!(readable.readable_ended());
    assert!(readable.closed());
    assert_eq!(codec.state.borrow().output.borrow().pending_bytes, 0);
    assert_eq!(
        gunzip_sync(&Buffer::from_bytes(bytes))
            .unwrap()
            .to_string(Some("utf8"))
            .unwrap(),
        "bounded output across native read pressure"
    );
}
