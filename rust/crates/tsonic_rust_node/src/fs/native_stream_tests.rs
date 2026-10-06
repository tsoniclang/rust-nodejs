use super::*;
use crate::background::BackgroundTasks;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tsonic_rust_runtime::Callable;

static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);

struct Fixture(std::path::PathBuf);

impl Fixture {
    fn new() -> Self {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".temp");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!(
            "native-stream-owner-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, b"native bytes").unwrap();
        Self(path)
    }
    fn text(&self) -> String {
        self.0.to_str().unwrap().to_owned()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}

enum Failure {
    Source(Rc<Cell<u64>>),
    Native(NodeError),
}

impl From<NodeError> for Failure {
    fn from(error: NodeError) -> Self {
        Self::Native(error)
    }
}

#[test]
fn readable_alias_retains_the_native_file_without_a_capacity_reference_cycle() {
    let fixture = Fixture::new();
    let background = BackgroundTasks::<NodeError>::new();
    let stream = ReadStream::from_file(
        &background,
        fixture.text(),
        File::open(&fixture.0).unwrap(),
        &ReadStreamOptions::default(),
    )
    .unwrap();
    let file_owner = Rc::downgrade(&stream.state);
    let readable = stream.readable_handle();
    let readable_owner = readable.downgrade();
    drop(stream);
    drop(background);
    assert!(file_owner.upgrade().is_some());
    drop(readable);
    assert!(readable_owner.upgrade().is_none());
    assert!(file_owner.upgrade().is_none());
}

#[test]
fn writable_alias_retains_the_actual_file_until_its_last_native_handle_is_dropped() {
    let fixture = Fixture::new();
    let background = BackgroundTasks::<NodeError>::new();
    let stream = WriteStream::from_file(
        &background,
        fixture.text(),
        File::create(&fixture.0).unwrap(),
        &WriteStreamOptions::default(),
    )
    .unwrap();
    let file_owner = Rc::downgrade(&stream.state);
    let writable = stream.writable_handle();
    drop(stream);
    assert!(file_owner
        .upgrade()
        .is_some_and(|state| state.borrow().file.is_some()));
    drop(writable);
    assert!(file_owner.upgrade().is_none());
    assert!(!background.has_pending_work());
}

#[test]
fn native_submission_rejection_clears_physical_pending_state() {
    let fixture = Fixture::new();
    let background = BackgroundTasks::<Failure>::new();
    let stream = WriteStream::from_file(
        &background,
        fixture.text(),
        File::create(&fixture.0).unwrap(),
        &WriteStreamOptions::default(),
    )
    .unwrap();
    drop(background);
    let error = stream
        .write(Buffer::from_bytes(vec![1]))
        .err()
        .expect("closed background owner");
    assert!(
        matches!(error, Failure::Native(error) if error.code() == "ERR_NODE_BACKGROUND_CLOSED")
    );
    assert!(stream.closed());
    assert!(!stream.pending());
    assert_eq!(stream.writable_handle().writable_length(), 0);
}

#[test]
fn file_write_continuation_survives_the_original_source_drain_failure() {
    let fixture = Fixture::new();
    let background = BackgroundTasks::<Failure>::new();
    let stream = WriteStream::from_file(
        &background,
        fixture.text(),
        File::create(&fixture.0).unwrap(),
        &WriteStreamOptions {
            high_water_mark: Some(1),
            ..Default::default()
        },
    )
    .unwrap();
    let original = Rc::new(Cell::new(9_007_199_254_740_993));
    let captured = Rc::clone(&original);
    let drain = Callable::new(move |()| Err(Failure::Source(Rc::clone(&captured))));
    assert!(stream.writable_handle().once_drain("drain", &drain).is_ok());
    assert!(matches!(
        stream.write(Buffer::from_bytes(vec![1])),
        Ok(false)
    ));
    assert!(matches!(
        stream.write(Buffer::from_bytes(vec![2, 3])),
        Ok(false)
    ));
    assert!(stream.close().is_ok());
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut observed = false;
    while background.has_pending_work() {
        assert!(
            Instant::now() < deadline,
            "native file completion timed out"
        );
        match background.poll() {
            Ok(_) => {}
            Err(Failure::Source(returned)) => {
                assert!(!observed);
                assert!(Rc::ptr_eq(&returned, &original));
                assert!(background.has_pending_work());
                observed = true;
            }
            Err(Failure::Native(error)) => {
                panic!("native file completion failed: {}", error.code())
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(observed);
    assert!(stream.closed());
    assert!(!stream.pending());
    assert!(stream.writable_handle().writable_finished());
    assert_eq!(std::fs::read(&fixture.0).unwrap(), [1, 2, 3]);
}
