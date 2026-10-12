use std::cell::{Cell, RefCell};
use std::num::NonZeroUsize;
use std::rc::Rc;

use tsonic_rust_node::buffer::Buffer;
use tsonic_rust_node::runtime_tasks::RuntimeTasks;
use tsonic_rust_node::stream::{Readable, StreamOptions};
use tsonic_rust_node::NodeError;
use tsonic_rust_runtime::dispatch_queue::{TaskBudget, TaskQueue};
use tsonic_rust_runtime::Callable;

#[path = "../support/allocation_counts.rs"]
mod allocation_counts;

#[test]
fn final_chunk_returns_before_exactly_once_end_and_close() {
    let tasks = RuntimeTasks::<NodeError>::new();
    let stream = Readable::from_chunks(tasks.handle(), vec![Buffer::from_bytes(vec![1, 2, 3])]);
    let trace = Rc::new(RefCell::new(Vec::new()));
    for event in ["end", "close"] {
        let observed = trace.clone();
        let listener = Callable::new(move |()| {
            observed.borrow_mut().push(event);
            Ok(())
        });
        if event == "end" {
            stream.once_end(event, &listener).unwrap();
        } else {
            stream.once_close(event, &listener).unwrap();
        }
    }
    assert_eq!(stream.read().unwrap().unwrap().as_bytes(), [1, 2, 3]);
    trace.borrow_mut().push("chunk");
    assert_eq!(*trace.borrow(), ["chunk"]);
    assert!(!stream.readable_ended());
    assert!(!stream.closed());
    assert!(stream.read().unwrap().is_none());
    tasks.poll().unwrap();
    assert_eq!(*trace.borrow(), ["chunk", "end", "close"]);
    assert!(stream.readable_ended());
    assert!(stream.closed());
    stream.read().unwrap();
    tasks.poll().unwrap();
    assert_eq!(*trace.borrow(), ["chunk", "end", "close"]);
}

#[test]
fn late_end_listeners_are_not_replayed_and_subscription_is_not_execution() {
    let tasks = RuntimeTasks::<NodeError>::new();
    let stream = Readable::from_chunks(tasks.handle(), Vec::new());
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let listener = Callable::new(move |()| {
        observed.set(observed.get() + 1);
        Ok(())
    });
    stream.once_end("end", &listener).unwrap();
    assert_eq!(calls.get(), 0);
    stream.resume().unwrap();
    assert_eq!(calls.get(), 0);
    tasks.poll().unwrap();
    assert_eq!(calls.get(), 1);
    stream.once_end("end", &listener).unwrap();
    stream.on_end("end", &listener).unwrap();
    stream.resume().unwrap();
    tasks.poll().unwrap();
    assert_eq!(calls.get(), 1);
}

#[test]
fn resume_drains_without_listeners_and_pause_preserves_queued_data() {
    let tasks = RuntimeTasks::<NodeError>::new();
    let stream = Readable::from_chunks(tasks.handle(), vec![Buffer::from_bytes(vec![1; 13])]);
    assert_eq!(stream.readable_length(), 13);
    stream.resume().unwrap();
    assert_eq!(stream.readable_flowing(), Some(true));
    stream.pause();
    tasks.poll().unwrap();
    assert_eq!(stream.readable_length(), 13);
    assert!(!stream.readable_ended());
    stream.resume().unwrap();
    tasks.poll().unwrap();
    assert_eq!(stream.readable_length(), 0);
    assert!(stream.readable_did_read());
    assert!(stream.readable_ended());
}

#[test]
fn byte_lengths_include_partial_reads_and_unshift_without_copying() {
    let tasks = RuntimeTasks::<NodeError>::new();
    let bytes = Buffer::from_bytes(vec![1, 2, 3, 4]);
    let stream = Readable::from_chunks(
        tasks.handle(),
        vec![bytes.clone(), Buffer::from_bytes(vec![5, 6])],
    );
    assert_eq!(stream.readable_length(), 6);
    let first = stream.read_buffer(Some(2)).unwrap().unwrap();
    assert_eq!(first.as_bytes(), [1, 2]);
    assert_eq!(stream.readable_length(), 4);
    first.set_number(0_usize, 9_u8);
    assert_eq!(bytes.as_bytes(), [9, 2, 3, 4]);
    stream.unshift(Buffer::from_bytes(vec![7, 8, 9])).unwrap();
    assert_eq!(stream.readable_length(), 7);
    assert_eq!(
        stream.read_buffer(Some(4)).unwrap().unwrap().as_bytes(),
        [7, 8, 9, 3]
    );
    assert_eq!(stream.readable_length(), 3);
    stream.to_array().unwrap();
    tasks.poll().unwrap();
    assert!(stream.readable_ended());
}

#[test]
fn object_mode_counts_objects_and_reads_one_object_regardless_of_size() {
    let tasks = RuntimeTasks::<NodeError>::new();
    let stream = Readable::from_chunks_with_options(
        tasks.handle(),
        vec![
            Buffer::from_bytes(Vec::new()),
            Buffer::from_bytes(vec![1; 32]),
        ],
        StreamOptions {
            object_mode: true,
            high_water_mark: 2,
            ..Default::default()
        },
    );
    assert_eq!(stream.readable_length(), 2);
    assert_eq!(stream.read_buffer(Some(1)).unwrap().unwrap().len(), 0);
    assert_eq!(stream.readable_length(), 1);
    assert_eq!(stream.read_buffer(Some(1)).unwrap().unwrap().len(), 32);
    assert_eq!(stream.readable_length(), 0);
    tasks.poll().unwrap();
    assert!(stream.readable_ended());
}

#[test]
fn empty_byte_chunks_do_not_inflate_lengths_or_obstruct_eof() {
    let tasks = RuntimeTasks::<NodeError>::new();
    let stream = Readable::from_chunks(
        tasks.handle(),
        vec![
            Buffer::from_bytes(Vec::new()),
            Buffer::from_bytes(vec![1]),
            Buffer::from_bytes(Vec::new()),
        ],
    );
    assert_eq!(stream.readable_length(), 1);
    assert_eq!(
        stream.read_buffer(Some(2)).unwrap().unwrap().as_bytes(),
        [1]
    );
    assert!(stream.read().unwrap().is_none());
    tasks.poll().unwrap();
    assert!(stream.readable_ended());
}

#[test]
fn destruction_cancels_queued_data_and_end_without_replaying_events() {
    let tasks = RuntimeTasks::<NodeError>::new();
    let stream = Readable::from_chunks(tasks.handle(), vec![Buffer::from_bytes(vec![1])]);
    let data = Callable::new(|(_chunk,): (Buffer,)| panic!("destroyed data callback"));
    let end = Callable::new(|()| panic!("destroyed end callback"));
    stream.on_data("data", &data).unwrap();
    stream.once_end("end", &end).unwrap();
    stream.destroy().unwrap();
    tasks.poll().unwrap();
    assert!(stream.destroyed());
    assert!(!stream.readable_ended());
    assert_eq!(stream.readable_length(), 0);
}

#[test]
fn reentrant_pause_stops_the_same_pump_before_the_next_chunk() {
    let tasks = RuntimeTasks::<NodeError>::new();
    let stream = Readable::from_chunks(
        tasks.handle(),
        vec![Buffer::from_bytes(vec![1]), Buffer::from_bytes(vec![2])],
    );
    let observed = Rc::new(Cell::new(0));
    let calls = observed.clone();
    let control = stream.clone();
    let listener = Callable::new(move |(_chunk,): (Buffer,)| {
        calls.set(calls.get() + 1);
        if calls.get() == 1 {
            control.pause();
        }
        Ok(())
    });
    stream.on_data("data", &listener).unwrap();
    tasks.poll().unwrap();
    assert_eq!(observed.get(), 1);
    assert_eq!(stream.readable_length(), 1);
    stream.resume().unwrap();
    tasks.poll().unwrap();
    assert_eq!(observed.get(), 2);
    assert!(stream.readable_ended());
    stream.off_data("data", &listener).unwrap();
}

#[test]
fn capacity_rejection_is_explicit_and_the_same_stream_can_retry() {
    let budget = TaskBudget::new(NonZeroUsize::new(1).unwrap());
    let queue = TaskQueue::<NodeError>::new(budget.clone());
    let stream = Readable::from_chunks(queue.handle(), vec![Buffer::from_bytes(vec![1])]);
    queue.enqueue(|| Ok(())).unwrap();
    let failure = stream.resume().unwrap_err();
    assert_eq!(failure.code(), "ERR_NODE_RUNTIME_TASK_LIMIT");
    assert_eq!(stream.readable_length(), 1);
    queue.poll_ready().unwrap();
    stream.resume().unwrap();
    queue.poll_ready().unwrap();
    assert_eq!(budget.pending(), 0);
    assert_eq!(stream.readable_length(), 0);
    assert!(stream.readable_ended());
}

#[test]
fn released_scheduler_is_a_native_failure_not_a_fallback() {
    let queue = TaskQueue::<NodeError>::new(TaskBudget::new(NonZeroUsize::new(1).unwrap()));
    let stream = Readable::from_chunks(queue.handle(), vec![Buffer::from_bytes(vec![1])]);
    drop(queue);
    assert_eq!(
        stream.resume().unwrap_err().code(),
        "ERR_NODE_RUNTIME_TASK_CLOSED"
    );
    assert_eq!(stream.readable_length(), 1);
}

#[test]
fn ten_thousand_chunks_use_one_pending_task_and_allocation_free_delivery() {
    let tasks = RuntimeTasks::<NodeError>::new();
    tasks.enqueue(|| Ok(())).unwrap();
    tasks.poll().unwrap();
    let chunks = (0..10_000).map(|_| Buffer::from_bytes(vec![7])).collect();
    let stream = Readable::from_chunks(tasks.handle(), chunks);
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let listener = Callable::new(move |(chunk,): (Buffer,)| {
        assert_eq!(chunk.len(), 1);
        observed.set(observed.get() + 1);
        Ok(())
    });
    assert_eq!(
        allocation_counts::measure(|| {
            stream.on_data("data", &listener).unwrap();
        })
        .0,
        3
    );
    let repeated = allocation_counts::measure(|| {
        for _ in 0..10_000 {
            stream.resume().unwrap();
        }
    });
    assert_eq!(repeated, (0, 0));
    assert_eq!(
        allocation_counts::measure(|| {
            tasks.poll().unwrap();
        }),
        (0, 0)
    );
    assert_eq!(calls.get(), 10_000);
    assert!(!tasks.has_pending_work());
    assert!(stream.readable_ended());
}

#[test]
fn terminal_events_release_native_callbacks_that_capture_the_stream() {
    let tasks = RuntimeTasks::<NodeError>::new();
    let stream = Readable::from_chunks(tasks.handle(), vec![Buffer::from_bytes(vec![1])]);
    let token = Rc::new(());
    let weak = Rc::downgrade(&token);
    let owner = stream.clone();
    let data_token = token.clone();
    let listener = Callable::new(move |(_chunk,): (Buffer,)| {
        let _capture = (&owner, &data_token);
        Ok(())
    });
    stream.on_data("data", &listener).unwrap();
    let owner = stream.clone();
    let end_token = token.clone();
    let end = Callable::new(move |()| {
        let _capture = (&owner, &end_token);
        Ok(())
    });
    stream.on_end("end", &end).unwrap();
    let owner = stream.clone();
    let close_token = token.clone();
    let close = Callable::new(move |()| {
        let _capture = (&owner, &close_token);
        Ok(())
    });
    stream.on_close("close", &close).unwrap();
    drop(listener);
    drop(end);
    drop(close);
    drop(token);
    assert!(weak.upgrade().is_some());
    tasks.poll().unwrap();
    assert!(weak.upgrade().is_none());
}

#[test]
fn successful_eof_releases_unemitted_error_and_disabled_close_captures() {
    for emit_close in [false, true] {
        let tasks = RuntimeTasks::<NodeError>::new();
        let stream = Readable::from_chunks_with_options(
            tasks.handle(),
            Vec::new(),
            StreamOptions {
                emit_close,
                ..Default::default()
            },
        );
        let token = Rc::new(());
        let weak = Rc::downgrade(&token);
        let owner = stream.clone();
        let retained = token.clone();
        let error = Callable::new(move |(_error,): (tsonic_rust_runtime::RetainedError,)| {
            let _capture = (&owner, &retained);
            panic!("successful EOF cannot fabricate an error event");
        });
        stream.on_error("error", &error).unwrap();
        let owner = stream.clone();
        let retained = token.clone();
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        let close = Callable::new(move |()| {
            let _capture = (&owner, &retained);
            observed.set(observed.get() + 1);
            Ok(())
        });
        stream.on_close("close", &close).unwrap();
        drop(error);
        drop(close);
        drop(token);
        stream.resume().unwrap();
        tasks.poll().unwrap();
        assert!(stream.closed());
        assert_eq!(calls.get(), usize::from(emit_close));
        assert!(weak.upgrade().is_none());
    }
}

#[test]
fn terminated_streams_do_not_retain_late_event_captures_or_replay_them() {
    let tasks = RuntimeTasks::<NodeError>::new();
    let stream = Readable::from_chunks(tasks.handle(), Vec::new());
    stream.resume().unwrap();
    tasks.poll().unwrap();
    let token = Rc::new(());
    let weak = Rc::downgrade(&token);
    let owner = stream.clone();
    let retained = token.clone();
    let error = Callable::new(move |(_error,): (tsonic_rust_runtime::RetainedError,)| {
        let _capture = (&owner, &retained);
        panic!("late error event");
    });
    stream.on_error("error", &error).unwrap();
    let owner = stream.clone();
    let retained = token.clone();
    let data = Callable::new(move |(_chunk,): (Buffer,)| {
        let _capture = (&owner, &retained);
        panic!("late data event");
    });
    stream.on_data("data", &data).unwrap();
    let owner = stream.clone();
    let retained = token.clone();
    let terminal = Callable::new(move |()| {
        let _capture = (&owner, &retained);
        panic!("late terminal event");
    });
    stream.on_end("end", &terminal).unwrap();
    stream.on_close("close", &terminal).unwrap();
    drop(error);
    drop(data);
    drop(terminal);
    drop(token);
    tasks.poll().unwrap();
    assert!(weak.upgrade().is_none());
}
