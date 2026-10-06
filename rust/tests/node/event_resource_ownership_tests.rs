use std::cell::Cell;
use std::hint::black_box;
use std::rc::Rc;
use tsonic_rust_js::JsValue;
use tsonic_rust_node::events::EventEmitter;
use tsonic_rust_node::fs::Watchers;
use tsonic_rust_node::net::{NetServers, Server};
use tsonic_rust_node::process::SignalTasks;
use tsonic_rust_node::worker_threads::{MessageChannel, WorkerResources};
use tsonic_rust_node::NodeError;
use tsonic_rust_runtime::dispatch::{poll_phase, DispatchContexts, DispatchPhase};
use tsonic_rust_runtime::Callable;

#[path = "../support/allocation_counts.rs"]
mod allocation_counts;
use allocation_counts::measure;

#[test]
fn repeated_source_events_reuse_native_keys_and_callbacks_without_allocation() {
    let emitter = EventEmitter::<NodeError>::new();
    let event = JsValue::String("data".to_owned());
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let listener = Callable::new(move |()| {
        observed.set(observed.get() + 1);
        Ok::<(), NodeError>(())
    });
    emitter.on_callable(&event, &listener).unwrap();
    assert!(emitter.emit_callable(&event).unwrap());
    let cost = measure(|| {
        for _ in 0..1024 {
            black_box(emitter.clone());
            assert_eq!(emitter.callable_listener_count(&event).unwrap(), 1);
            assert!(emitter.emit_callable(&event).unwrap());
        }
    });
    assert_eq!(calls.get(), 1025);
    assert_eq!(cost, (0, 0));
}

#[test]
fn cold_worker_and_signal_roots_allocate_no_native_storage() {
    let cost = measure(|| {
        let workers = black_box(WorkerResources::<NodeError>::new());
        let signals = black_box(SignalTasks::<NodeError>::new());
        let servers = black_box(NetServers::<NodeError>::new());
        let watchers = black_box(Watchers::<NodeError>::new());
        assert!(!workers.has_work());
        assert!(!signals.has_work());
        assert!(!servers.has_work());
        assert!(!watchers.has_work());
        assert_eq!(poll_phase(&workers, DispatchPhase::Ports).unwrap(), false);
        assert_eq!(poll_phase(&signals, DispatchPhase::Signals).unwrap(), false);
        assert_eq!(poll_phase(&servers, DispatchPhase::Net).unwrap(), false);
        assert_eq!(
            poll_phase(&watchers, DispatchPhase::Watchers).unwrap(),
            false
        );
        assert_eq!(workers.next_delay(), None);
        assert_eq!(signals.next_delay(), None);
        assert_eq!(servers.next_delay(), None);
        assert_eq!(watchers.next_delay(), None);
    });
    assert_eq!(cost, (0, 0));
}

#[test]
fn warmed_native_server_frontiers_and_shared_handles_do_not_allocate() {
    let servers = NetServers::<NodeError>::new();
    let server = Server::new(&servers).unwrap();
    assert!(!poll_phase(&servers, DispatchPhase::Net).unwrap());
    let cost = measure(|| {
        for _ in 0..1024 {
            black_box(server.clone());
            assert!(!poll_phase(&servers, DispatchPhase::Net).unwrap());
            assert!(!servers.has_work());
            assert_eq!(servers.next_delay(), None);
        }
    });
    assert_eq!(cost, (0, 0));
}

#[test]
fn warmed_native_watcher_frontiers_and_shared_handles_do_not_allocate() {
    let watchers = Watchers::<NodeError>::new();
    let watcher = tsonic_rust_node::fs::watch_file_with_options(
        &watchers,
        "native-idle-watch-cost",
        tsonic_rust_node::fs::WatchFileOptions::default(),
    )
    .unwrap();
    assert!(!poll_phase(&watchers, DispatchPhase::Watchers).unwrap());
    let cost = measure(|| {
        for _ in 0..1024 {
            black_box(watcher.clone());
            assert!(!poll_phase(&watchers, DispatchPhase::Watchers).unwrap());
            assert!(!watchers.has_work());
            assert_eq!(watchers.next_delay(), None);
        }
    });
    assert_eq!(cost, (0, 0));
    watcher.close();
}

#[test]
fn warmed_native_resource_frontiers_do_not_allocate_snapshot_wrappers() {
    let workers = WorkerResources::<NodeError>::new();
    let channel = MessageChannel::new(&workers).unwrap();
    channel.port2.start();
    assert!(!poll_phase(&workers, DispatchPhase::Ports).unwrap());
    let cost = measure(|| {
        for _ in 0..1024 {
            black_box(channel.port2.clone());
            assert!(!poll_phase(&workers, DispatchPhase::Ports).unwrap());
            assert!(workers.has_work());
            assert_eq!(workers.next_delay(), None);
        }
    });
    assert_eq!(cost, (0, 0));
}

#[test]
fn a_once_listener_releases_native_capture_at_invocation_without_waiting_for_pruning() {
    struct DropProbe(Rc<Cell<usize>>);
    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Rc::new(Cell::new(0));
    let emitter = EventEmitter::<NodeError>::new();
    let event = JsValue::String("data".to_owned());
    let probe = DropProbe(Rc::clone(&drops));
    let listener = Callable::new(move |()| {
        black_box(&probe);
        Ok::<(), NodeError>(())
    });
    emitter.once_callable(&event, &listener).unwrap();
    drop(listener);
    assert!(emitter.emit_callable(&event).unwrap());
    assert_eq!(drops.get(), 1);
    assert_eq!(emitter.callable_listener_count(&event).unwrap(), 0);
    assert!(!emitter.emit_callable(&event).unwrap());
    assert_eq!(drops.get(), 1);
}
