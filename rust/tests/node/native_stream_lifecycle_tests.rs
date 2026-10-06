use std::cell::{Cell, RefCell};
use std::rc::Rc;

use tsonic_rust_node::{buffer::Buffer, stream, zlib, NodeError};
use tsonic_rust_runtime::{Callable, RetainedError};

#[test]
fn idle_codec_destruction_through_readable_releases_the_original_resource() {
    for mode in [
        zlib::ZlibMode::Gzip,
        zlib::ZlibMode::Gunzip,
        zlib::ZlibMode::Deflate,
        zlib::ZlibMode::Inflate,
        zlib::ZlibMode::DeflateRaw,
        zlib::ZlibMode::InflateRaw,
        zlib::ZlibMode::BrotliCompress,
        zlib::ZlibMode::BrotliDecompress,
        zlib::ZlibMode::Unzip,
    ] {
        let codec = zlib::Zlib::new(mode, None);
        let readable = zlib::zlib_as_readable(&codec);
        let writable = zlib::zlib_as_writable(&codec);
        let closes = Rc::new(Cell::new(0));
        let observed = Rc::clone(&closes);
        let close = Callable::new(move |()| {
            observed.set(observed.get() + 1);
            Ok::<(), NodeError>(())
        });
        readable.on_close("close", &close).unwrap();
        assert_eq!(readable, readable.destroy_chain(None).unwrap());
        readable.destroy_chain(None).unwrap();
        tsonic_rust_node::run_event_loop().unwrap();
        assert!(codec.closed());
        assert!(readable.destroyed());
        assert!(writable.destroyed());
        assert!(!writable.writable_finished());
        assert!(!readable.readable());
        assert!(!writable.writable());
        assert_eq!(closes.get(), 1);
    }
}

#[test]
fn codec_destruction_shares_error_and_close_identity_across_all_base_projections() {
    let codec = zlib::create_gzip(None);
    let duplex = zlib::zlib_as_duplex(&codec);
    let readable = zlib::zlib_as_readable(&codec);
    let writable = zlib::zlib_as_writable(&codec);
    let expected =
        RetainedError::from(tsonic_rust_runtime::JsError::error("native codec identity"));
    let trace = Rc::new(RefCell::new(Vec::new()));
    let observed_errors = Rc::new(RefCell::new(Vec::new()));
    let removed = Callable::new(|(_error,): (RetainedError,)| {
        Err::<(), _>("removed native listener was invoked")
    });
    duplex.on_error("error", &removed).unwrap();
    readable.off_error("error", &removed).unwrap();
    duplex.once_error("error", &removed).unwrap();
    writable.off_error("error", &removed).unwrap();
    let callback_codec = codec.clone();
    let callback_readable = readable.clone();
    let callback_writable = writable.clone();
    let callback_errors = Rc::clone(&observed_errors);
    let callback_trace = Rc::clone(&trace);
    let listener = Callable::new(move |(error,): (RetainedError,)| {
        assert!(callback_codec.closed());
        assert!(callback_readable.destroyed());
        assert!(callback_writable.destroyed());
        callback_errors.borrow_mut().push(error);
        callback_trace.borrow_mut().push("error");
        Ok::<(), NodeError>(())
    });
    duplex.once_error("error", &listener).unwrap();
    let callback_trace = Rc::clone(&trace);
    let close = Callable::new(move |()| {
        callback_trace.borrow_mut().push("close");
        Ok::<(), NodeError>(())
    });
    duplex.on_close("close", &close).unwrap();
    readable.off_close("close", &close).unwrap();
    duplex.once_close("close", &close).unwrap();
    writable.off_close("close", &close).unwrap();
    duplex.once_close("close", &close).unwrap();
    assert!(duplex.once_close("error", &close).is_err());
    assert!(duplex.once_error("close", &listener).is_err());
    readable.destroy_chain(Some(expected.clone())).unwrap();
    duplex.destroy_chain(Some(expected.clone())).unwrap();
    assert_eq!(*trace.borrow(), ["error", "close"]);
    let errors = observed_errors.borrow();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0], expected);
    assert!(readable.closed());
    assert!(writable.closed());
    drop(errors);
    readable.once_close("close", &close).unwrap();
    readable.destroy_chain(None).unwrap();
    assert_eq!(*trace.borrow(), ["error", "close"]);
    tsonic_rust_node::run_event_loop().unwrap();
}

#[test]
fn duplex_drain_finish_and_close_use_their_owning_native_states_once() {
    let transform = stream::Transform::new(|chunk| chunk);
    let duplex = stream::transform_as_duplex(&transform);
    let readable = stream::transform_as_readable(&transform);
    let trace = Rc::new(RefCell::new(Vec::new()));
    let removed = Callable::new(|()| Err::<(), _>("removed lifecycle listener"));
    assert_eq!(duplex, duplex.on_drain("drain", &removed).unwrap());
    duplex.off_drain("drain", &removed).unwrap();
    duplex.once_drain("drain", &removed).unwrap();
    duplex.off_drain("drain", &removed).unwrap();
    duplex.on_finish("finish", &removed).unwrap();
    duplex.off_finish("finish", &removed).unwrap();
    duplex.once_finish("finish", &removed).unwrap();
    duplex.off_finish("finish", &removed).unwrap();
    for event in ["drain", "finish", "close"] {
        let callback_trace = Rc::clone(&trace);
        let callback = Callable::new(move |()| {
            callback_trace.borrow_mut().push(event);
            Ok::<(), NodeError>(())
        });
        match event {
            "drain" => duplex.once_drain(event, &callback).unwrap(),
            "finish" => duplex.once_finish(event, &callback).unwrap(),
            "close" => duplex.once_close(event, &callback).unwrap(),
            _ => unreachable!(),
        };
    }
    duplex.cork();
    let bytes = Buffer::from_bytes(vec![7; 16 * 1024]);
    assert!(!duplex.write_buffer(&bytes).unwrap());
    duplex.uncork().unwrap();
    assert!(duplex.writable_need_drain());
    assert_eq!(readable.read_buffer(None).unwrap().unwrap(), bytes);
    assert!(!duplex.writable_need_drain());
    assert_eq!(duplex, duplex.end().unwrap());
    duplex.end().unwrap();
    assert_eq!(*trace.borrow(), ["drain", "finish", "close"]);
    assert!(duplex.writable_finished());
    assert!(readable.readable_ended());
}

#[test]
fn error_callbacks_can_change_the_subsequent_shared_close_subscription() {
    let codec = zlib::create_gzip(None);
    let duplex = zlib::zlib_as_duplex(&codec);
    let readable = zlib::zlib_as_readable(&codec);
    let observed = Rc::new(Cell::new(0));
    let callback_observed = Rc::clone(&observed);
    let close = Callable::new(move |()| {
        callback_observed.set(callback_observed.get() + 1);
        Ok::<(), NodeError>(())
    });
    let removed = Callable::new(|()| Err::<(), _>("removed close listener was invoked"));
    duplex.once_close("close", &removed).unwrap();
    let callback_readable = readable.clone();
    let listener = Callable::new(move |(_error,): (RetainedError,)| {
        assert!(callback_readable.closed());
        callback_readable.off_close("close", &removed)?;
        callback_readable.once_close("close", &close)?;
        Ok::<(), NodeError>(())
    });
    duplex.once_error("error", &listener).unwrap();
    let expected = RetainedError::from(tsonic_rust_runtime::JsError::error(
        "native lifecycle error",
    ));
    readable.destroy_chain(Some(expected)).unwrap();
    assert_eq!(observed.get(), 1);
    assert!(codec.closed());
}

#[test]
fn native_resource_closure_does_not_depend_on_emitting_the_close_event() {
    let writable = stream::Writable::with_options(stream::StreamOptions {
        emit_close: false,
        ..Default::default()
    });
    let close = Callable::new(|()| Err::<(), _>("disabled close event was emitted"));
    writable.once_close("close", &close).unwrap();
    writable.cork();
    writable.write_string("pending native input").unwrap();
    writable.destroy_chain(None).unwrap();
    assert!(writable.closed());
    assert!(writable.destroyed());
    assert_eq!(writable.writable_length(), 0);
    assert_eq!(writable.writable_corked(), 0);
    assert!(!writable.writable_finished());
}

#[test]
fn end_and_uncork_propagate_native_callback_failures_instead_of_discarding_them() {
    let writable = stream::Writable::new();
    let finish = Callable::new(|()| Err::<(), _>("native finish callback"));
    writable.once_finish("finish", &finish).unwrap();
    let failure = writable.end().unwrap_err();
    assert_eq!(failure.code(), "ERR_TSONIC_CALLBACK");
    assert_eq!(failure.message(), "native finish callback");
    assert!(writable.writable_finished());

    let writable = stream::Writable::with_options(stream::StreamOptions {
        high_water_mark: 1,
        ..Default::default()
    });
    let drain = Callable::new(|()| Err::<(), _>("native drain callback"));
    writable.once_drain("drain", &drain).unwrap();
    writable.cork();
    assert!(!writable.write_buffer(&Buffer::from_bytes(vec![1])).unwrap());
    let failure = writable.uncork().unwrap_err();
    assert_eq!(failure.code(), "ERR_TSONIC_CALLBACK");
    assert_eq!(failure.message(), "native drain callback");
    assert!(!writable.writable_need_drain());
    writable.end().unwrap();
    assert!(writable.writable_finished());
}

#[test]
fn destruction_callback_failure_does_not_prevent_codec_cleanup_or_close() {
    let codec = zlib::create_gzip(None);
    let duplex = zlib::zlib_as_duplex(&codec);
    let closes = Rc::new(Cell::new(0));
    let observed = Rc::clone(&closes);
    let close = Callable::new(move |()| {
        observed.set(observed.get() + 1);
        Err::<(), _>("later native close listener failure")
    });
    duplex.once_close("close", &close).unwrap();
    let expected = RetainedError::from(tsonic_rust_runtime::JsError::error("supplied codec error"));
    let callback_expected = expected.clone();
    let listener = Callable::new(move |(error,): (RetainedError,)| {
        assert_eq!(error, callback_expected);
        Err::<(), _>("native destruction listener")
    });
    duplex.once_error("error", &listener).unwrap();
    let failure = duplex.destroy_chain(Some(expected)).unwrap_err();
    assert_eq!(failure.code(), "ERR_TSONIC_CALLBACK");
    assert_eq!(failure.message(), "native destruction listener");
    duplex.destroy_chain(None).unwrap();
    assert!(codec.closed());
    assert!(duplex.destroyed());
    assert_eq!(closes.get(), 1);
    tsonic_rust_node::run_event_loop().unwrap();
}

#[test]
fn uncork_codec_failure_destroys_both_sides_without_a_successful_finish() {
    let codec = zlib::create_gunzip(None);
    let duplex = zlib::zlib_as_duplex(&codec);
    let errors = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::clone(&errors);
    let listener = Callable::new(move |(error,): (RetainedError,)| {
        observed.borrow_mut().push(error);
        Ok::<(), NodeError>(())
    });
    duplex.once_error("error", &listener).unwrap();
    duplex.cork();
    duplex.write_string("not compressed data").unwrap();
    assert!(duplex.uncork().is_err());
    assert_eq!(errors.borrow().len(), 1);
    assert!(codec.closed());
    assert!(duplex.destroyed());
    assert!(!duplex.writable_finished());
    assert!(zlib::zlib_as_readable(&codec)
        .read_buffer(None)
        .unwrap()
        .is_none());
    assert!(duplex.write_string("late data").is_err());
    tsonic_rust_node::run_event_loop().unwrap();
}
