use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use tsonic_rust_node::{buffer::Buffer, fs, stream, zlib, NodeError};
use tsonic_rust_runtime::{Callable, RetainedError};

#[test]
fn file_stream_projection_preserves_listener_identity_pressure_and_finish() {
    let background =
        tsonic_rust_node::background::BackgroundTasks::<tsonic_rust_runtime::TsonicError>::new();
    let root = std::env::current_dir().unwrap().join(".temp").join(format!(
        "native-stream-inheritance-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("output.txt");
    let file = fs::create_write_stream_with_options(
        &background,
        &path.to_string_lossy(),
        fs::WriteStreamOptions {
            high_water_mark: Some(1),
            ..Default::default()
        },
    )
    .unwrap();
    let writable = fs::write_stream_as_writable(&file);
    assert_eq!(writable, fs::write_stream_as_writable(&file));
    let removed_calls = Rc::new(Cell::new(0));
    let callback_calls = Rc::clone(&removed_calls);
    let removed = Callable::new(move |()| {
        callback_calls.set(callback_calls.get() + 1);
        Ok::<(), tsonic_rust_runtime::TsonicError>(())
    });
    let callback_calls = Rc::clone(&removed_calls);
    let removed_error = Callable::new(move |(_error,): (RetainedError,)| {
        callback_calls.set(callback_calls.get() + 1);
        Ok::<(), tsonic_rust_runtime::TsonicError>(())
    });
    assert_eq!(
        writable,
        writable.on_error("error", &removed_error).unwrap()
    );
    assert_eq!(
        writable,
        writable.off_error("error", &removed_error).unwrap()
    );
    assert_eq!(
        writable,
        writable.once_error("error", &removed_error).unwrap()
    );
    writable.off_error("error", &removed_error).unwrap();
    assert_eq!(writable, writable.once_drain("drain", &removed).unwrap());
    assert_eq!(writable, writable.off_drain("drain", &removed).unwrap());
    assert_eq!(writable, writable.once_finish("finish", &removed).unwrap());
    assert_eq!(writable, writable.off_finish("finish", &removed).unwrap());
    assert!(writable.once_drain("error", &removed).is_err());
    assert!(writable.once_error("finish", &removed_error).is_err());

    let drain_calls = Rc::new(Cell::new(0));
    let callback_calls = Rc::clone(&drain_calls);
    let drain = Callable::new(move |()| {
        callback_calls.set(callback_calls.get() + 1);
        Ok::<(), tsonic_rust_runtime::TsonicError>(())
    });
    let finish_calls = Rc::new(Cell::new(0));
    let callback_calls = Rc::clone(&finish_calls);
    let finish = Callable::new(move |()| {
        callback_calls.set(callback_calls.get() + 1);
        Ok::<(), tsonic_rust_runtime::TsonicError>(())
    });
    writable.once_drain("drain", &drain).unwrap();
    writable.once_finish("finish", &finish).unwrap();
    let bytes = Buffer::from_string("native stream", Some("utf8")).unwrap();
    assert!(!writable.write_buffer(&bytes).unwrap());
    assert!(writable.writable_need_drain());
    assert_eq!(writable, writable.end().unwrap());
    assert!(writable.writable_ended());
    tsonic_rust_node::run_with_contexts(tsonic_rust_runtime::dispatch::prepend(
        &background,
        tsonic_rust_runtime::dispatch::DispatchEnd::<tsonic_rust_runtime::TsonicError>::new(),
    ))
    .unwrap();
    assert_eq!(removed_calls.get(), 0);
    assert_eq!(drain_calls.get(), 1);
    assert_eq!(finish_calls.get(), 1);
    assert!(writable.writable_finished());
    assert!(!writable.writable_need_drain());
    assert_eq!(file.bytes_written(), bytes.len());
    assert_eq!(std::fs::read(&path).unwrap(), b"native stream");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn transform_and_zlib_base_projections_destroy_the_original_stream() {
    let transform = stream::Transform::<NodeError>::new(|chunk| chunk);
    let duplex = stream::transform_as_duplex(&transform);
    assert_eq!(duplex, duplex.destroy_chain(None).unwrap());
    assert!(stream::transform_as_readable(&transform).destroyed());
    assert!(stream::transform_as_writable(&transform).destroyed());

    let codec = zlib::create_gzip::<NodeError>(None);
    let duplex = zlib::zlib_as_duplex(&codec);
    assert_eq!(duplex, duplex.destroy_chain(None).unwrap());
    assert!(codec.closed());
    assert!(zlib::zlib_as_readable(&codec).destroyed());
    assert!(zlib::zlib_as_writable(&codec).destroyed());
}

#[test]
fn projected_codec_destruction_retains_error_identity_for_shared_lifecycle_listeners() {
    let codec = zlib::create_gzip::<NodeError>(None);
    let readable = zlib::zlib_as_readable(&codec);
    let writable = zlib::zlib_as_writable(&codec);
    let observed = Rc::new(RefCell::new(Vec::new()));
    let callback_observed = Rc::clone(&observed);
    let listener = Callable::new(move |(error,): (RetainedError,)| {
        callback_observed.borrow_mut().push(error);
        Ok::<(), NodeError>(())
    });
    readable.once_error("error", &listener).unwrap();
    writable.once_error("error", &listener).unwrap();
    let error = RetainedError::from(tsonic_rust_runtime::JsError::error("native codec failure"));
    zlib::zlib_as_duplex(&codec)
        .destroy_chain(Some(error.clone()))
        .unwrap();
    zlib::zlib_as_duplex(&codec)
        .destroy_chain(Some(error.clone()))
        .unwrap();
    let observed = observed.borrow();
    assert_eq!(observed.len(), 2);
    assert!(observed.iter().all(|value| value == &error));
    assert!(codec.closed());
}
