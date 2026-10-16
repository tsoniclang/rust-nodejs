use std::cell::Cell;
use std::future::Future;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;
use tsonic_rust_js::{JsArray, JsValue};
use tsonic_rust_node::worker_threads::{Worker, WorkerOptions, WorkerResources};
use tsonic_rust_node::NodeError;
use tsonic_rust_runtime::dispatch::{prepend, DispatchEnd};
use tsonic_rust_runtime::{Callable, TsonicError};

#[path = "../support/allocation_counts.rs"]
mod allocation_counts;

fn main() {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|argument| argument == "--tsonic-node-worker-v1")
    {
        child(&arguments);
        return;
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "--native-control")
    {
        let selected = arguments
            .get(1)
            .expect("one explicit native control name")
            .as_str();
        match selected {
            "construction" | "cancellation" | "concurrent" | "signal-status" | "ready-cost" => {
                native_termination_without_a_js_owner(Some(selected))
            }
            "retained-prefix" => retained_prefix_with_later_explicit_owner(),
            "unobserved-errors" => native_unobserved_errors_after_physical_join(),
            "default-ref" => default_ref_without_listeners_reaches_real_physical_completion(),
            "saturated-terminate" => native_saturated_public_termination(),
            _ => panic!("unknown native control: {selected}"),
        }
        println!("P05 native control {selected}: passed");
        return;
    }
    for _ in 0..8 {
        native_termination_without_a_js_owner(None);
    }
    retained_prefix_with_later_explicit_owner();
    native_unobserved_errors_after_physical_join();
    default_ref_without_listeners_reaches_real_physical_completion();
    native_saturated_public_termination();
    println!("P05 public native lifecycle: native executor, cancellation, concurrent waiters, retained FIFO, unobserved errors passed");
}

fn spawn<E: From<NodeError> + From<TsonicError> + std::fmt::Debug + 'static>(
    entry: &str,
    port: u16,
    resources: &WorkerResources<E>,
) -> Worker<E> {
    assert!(
        tsonic_rust_js::event_loop::dispatch_owner().is_err(),
        "no JS owner before native construction"
    );
    let options = WorkerOptions {
        argv: Some(JsArray::from_dense(vec![port.to_string()])),
        ..WorkerOptions::default()
    };
    let worker = if std::env::args().any(|argument| argument == "--base-construction-owner") {
        let (worker, _) = tsonic_rust_node::run_entry_with_contexts(
            || {
                let worker = Worker::spawn_with_options(entry, options, resources).unwrap();
                worker.unref();
                worker
            },
            prepend(resources, DispatchEnd::<E>::new()),
        )
        .unwrap();
        worker.ref_chain();
        worker
    } else {
        Worker::spawn_with_options(entry, options, resources).unwrap()
    };
    assert!(
        tsonic_rust_js::event_loop::dispatch_owner().is_err(),
        "native construction creates no JS owner"
    );
    worker
}

fn control() -> TcpListener {
    TcpListener::bind("127.0.0.1:0").unwrap()
}

fn accept_ready(listener: &TcpListener) -> TcpStream {
    let (mut stream, _) = listener.accept().unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut ready = [0];
    stream.read_exact(&mut ready).unwrap();
    assert_eq!(
        ready,
        [1],
        "native worker reached the declared physical boundary"
    );
    stream
}

struct NativeWake {
    ready: Mutex<bool>,
    changed: Condvar,
}

impl Wake for NativeWake {
    fn wake(self: Arc<Self>) {
        *self.ready.lock().unwrap() = true;
        self.changed.notify_one();
    }
}

fn native_executor<T>(future: impl Future<Output = T>) -> T {
    let wake = Arc::new(NativeWake {
        ready: Mutex::new(false),
        changed: Condvar::new(),
    });
    let waker = Waker::from(Arc::clone(&wake));
    let mut future = std::pin::pin!(future);
    let mut polls = 0;
    loop {
        polls += 1;
        assert!(
            polls < 1_000_000,
            "native lifecycle makes finite progress without busy self-wake"
        );
        *wake.ready.lock().unwrap() = false;
        if let Poll::Ready(value) = future.as_mut().poll(&mut Context::from_waker(&waker)) {
            return value;
        }
        let selected = wake.ready.lock().unwrap();
        let (ready, timeout) = wake
            .changed
            .wait_timeout_while(selected, Duration::from_secs(10), |ready| !*ready)
            .unwrap();
        assert!(
            *ready && !timeout.timed_out(),
            "real native producer must wake an ordinary executor"
        );
    }
}

fn native_termination_without_a_js_owner(selected: Option<&str>) {
    let listener = control();
    let resources = WorkerResources::<TsonicError>::new();
    let worker = spawn(
        "native-pending",
        listener.local_addr().unwrap().port(),
        &resources,
    );
    let _control = accept_ready(&listener);
    assert!(
        worker
            .on_callable(
                &JsValue::String("message".to_owned()),
                &Callable::new(|()| Ok::<(), TsonicError>(()))
            )
            .is_err(),
        "callback admission still requires explicit owner"
    );
    if selected.is_none() || selected == Some("cancellation") {
        let mut cancelled = std::pin::pin!(worker.terminate());
        let _ = cancelled
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()));
    }
    let mut first = std::pin::pin!(worker.terminate());
    let mut second = std::pin::pin!(worker.terminate());
    let mut first_result = None;
    let mut second_result = None;
    let concurrent = selected.is_none() || selected == Some("concurrent");
    native_executor(std::future::poll_fn(|context| {
        if first_result.is_none() {
            if let Poll::Ready(result) = first.as_mut().poll(context) {
                first_result = Some(result);
            }
        }
        if concurrent && second_result.is_none() {
            if let Poll::Ready(result) = second.as_mut().poll(context) {
                second_result = Some(result);
            }
        }
        if first_result.is_some() && (!concurrent || second_result.is_some()) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }));
    let first = first_result.unwrap().unwrap();
    let second = if concurrent {
        second_result.unwrap().unwrap()
    } else {
        first
    };
    assert_eq!(
        first, second,
        "concurrent native observers share one physical status"
    );
    #[cfg(unix)]
    if selected.is_none() || selected == Some("signal-status") {
        assert_eq!(first, 137, "Unix signal status remains native");
    }
    assert_eq!(
        native_executor(worker.terminate()).unwrap(),
        first,
        "repeat observation needs no JS owner"
    );
    if selected.is_some() && selected != Some("ready-cost") {
        return;
    }
    let cost = allocation_counts::measure(|| {
        for _ in 0..1024 {
            let mut ready = std::pin::pin!(worker.terminate());
            assert!(
                matches!(ready.as_mut().poll(&mut Context::from_waker(Waker::noop())),
                Poll::Ready(Ok(code)) if code == first),
                "ready native observation is inline"
            );
        }
    });
    assert_eq!(
        cost,
        (0, 0),
        "ready native completion adds no allocation or boxing"
    );
    assert!(tsonic_rust_js::event_loop::dispatch_owner().is_err());
}

fn retained_prefix_with_later_explicit_owner() {
    let listener = control();
    let resources = WorkerResources::<TsonicError>::new();
    let worker = spawn(
        "native-burst",
        listener.local_addr().unwrap().port(),
        &resources,
    );
    let _control = accept_ready(&listener);
    let count = Rc::new(Cell::new(0));
    let observed = Rc::clone(&count);
    let exits = Rc::new(Cell::new(0));
    let terminal = Rc::clone(&exits);
    let prefix = Rc::clone(&count);
    let ((), _) = tsonic_rust_node::run_entry_with_contexts(
        || {
            worker
                .on_callable1(
                    &JsValue::String("message".to_owned()),
                    &Callable::new(move |(value,): (JsValue,)| {
                        assert_eq!(
                            value,
                            JsValue::Integer(observed.get() as i64),
                            "retained physical FIFO has no loss/replay"
                        );
                        observed.set(observed.get() + 1);
                        Ok::<(), TsonicError>(())
                    }),
                )
                .unwrap();
            worker
                .on_callable1(
                    &JsValue::String("exit".to_owned()),
                    &Callable::new(move |(value,): (JsValue,)| {
                        assert!(matches!(value, JsValue::Int32(0)), "normal native exit");
                        assert_eq!(
                            prefix.get(),
                            (1 << 17) + 17,
                            "more than twice capacity reaches listeners before exit"
                        );
                        terminal.set(terminal.get() + 1);
                        Ok::<(), TsonicError>(())
                    }),
                )
                .unwrap();
        },
        prepend(&resources, DispatchEnd::<TsonicError>::new()),
    )
    .unwrap();
    assert_eq!(count.get(), (1 << 17) + 17);
    assert_eq!(exits.get(), 1);
}

fn native_unobserved_errors_after_physical_join() {
    let listener = control();
    let resources = WorkerResources::<TsonicError>::new();
    let worker = spawn(
        "native-errors",
        listener.local_addr().unwrap().port(),
        &resources,
    );
    let _control = accept_ready(&listener);
    let error = native_executor(worker.terminate()).unwrap_err();
    assert_eq!(error.code(), "ERR_WORKER_ERROR");
    assert!(error.message().starts_with("first native error"));
    assert_eq!(
        error.message(),
        "first native error",
        "first reason is never formatted into a replacement error"
    );
    assert!(tsonic_rust_js::event_loop::dispatch_owner().is_err());
}

fn default_ref_without_listeners_reaches_real_physical_completion() {
    let listener = control();
    let resources = WorkerResources::<TsonicError>::new();
    let mut retained = None;
    let ((), _) = tsonic_rust_node::run_entry_with_contexts(
        || {
            retained = Some(
                Worker::spawn_with_options(
                    "native-pending",
                    WorkerOptions {
                        argv: Some(JsArray::from_dense(vec![listener
                            .local_addr()
                            .unwrap()
                            .port()
                            .to_string()])),
                        ..WorkerOptions::default()
                    },
                    &resources,
                )
                .unwrap(),
            );
            let mut control = accept_ready(&listener);
            control.write_all(&[1]).unwrap();
        },
        prepend(&resources, DispatchEnd::<TsonicError>::new()),
    )
    .unwrap();
    let worker = retained.unwrap();
    let mut completion = std::pin::pin!(worker.terminate());
    assert!(matches!(completion.as_mut().poll(&mut Context::from_waker(Waker::noop())), Poll::Ready(Ok(0))),
        "default-ref without any listeners keeps the actual reader and process alive through physical completion");
    assert!(tsonic_rust_js::event_loop::dispatch_owner().is_err());
}

fn native_saturated_public_termination() {
    let listener = control();
    let resources = WorkerResources::<TsonicError>::new();
    let worker = spawn(
        "native-saturated",
        listener.local_addr().unwrap().port(),
        &resources,
    );
    let mut control = accept_ready(&listener);
    let mut prefix = [0];
    control.read_exact(&mut prefix).unwrap();
    assert_eq!(
        prefix,
        [2],
        "real child wrote more than raw capacity before public termination"
    );
    let code = native_executor(worker.terminate()).unwrap();
    #[cfg(unix)]
    assert_eq!(
        code, 137,
        "standalone public termination joins the actual native process after retained input drains"
    );
    #[cfg(not(unix))]
    assert!(
        code != 0,
        "native process termination preserves its actual native exit status"
    );
    assert!(tsonic_rust_js::event_loop::dispatch_owner().is_err());
}

fn child(arguments: &[String]) {
    let mut stream = TcpStream::connect((
        std::net::Ipv4Addr::LOCALHOST,
        arguments[2].parse::<u16>().unwrap(),
    ))
    .unwrap();
    let token = arguments[3]
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect::<Vec<_>>();
    write_frame(&mut stream, 1, &token);
    assert_eq!(read_frame(&mut stream).0, 2);
    assert_eq!(read_frame(&mut stream).0, 3);
    let mut control = TcpStream::connect((
        std::net::Ipv4Addr::LOCALHOST,
        arguments[7].parse::<u16>().unwrap(),
    ))
    .unwrap();
    if arguments[1] == "native-errors" {
        write_frame(&mut stream, 5, b"first native error");
        write_frame(&mut stream, 5, b"second native error");
        write_frame(&mut stream, 6, &[]);
        control.write_all(&[1]).unwrap();
        return;
    }
    control.write_all(&[1]).unwrap();
    if arguments[1] == "native-pending" {
        let mut released = [0];
        let _ = control.read_exact(&mut released);
        return;
    }
    assert!(arguments[1] == "native-burst" || arguments[1] == "native-saturated");
    let count = if arguments[1] == "native-saturated" {
        (1 << 16) + 17
    } else {
        (1 << 17) + 17
    };
    for value in 0..count as i64 {
        let mut payload = [0; 14];
        payload[0] = 3;
        payload[5] = 8;
        payload[6..].copy_from_slice(&value.to_be_bytes());
        write_frame(&mut stream, 4, &payload);
    }
    if arguments[1] == "native-saturated" {
        control.write_all(&[2]).unwrap();
        let mut released = [0];
        let _ = control.read_exact(&mut released);
        return;
    }
    stream.shutdown(Shutdown::Write).unwrap();
}

fn write_frame(stream: &mut TcpStream, kind: u8, payload: &[u8]) {
    let mut header = [0; 5];
    header[0] = kind;
    header[1..].copy_from_slice(&(payload.len() as u32).to_be_bytes());
    stream.write_all(&header).unwrap();
    stream.write_all(payload).unwrap();
}

fn read_frame(stream: &mut TcpStream) -> (u8, Vec<u8>) {
    let mut header = [0; 5];
    stream.read_exact(&mut header).unwrap();
    let length = u32::from_be_bytes(header[1..].try_into().unwrap()) as usize;
    assert!(length < 4096, "finite bootstrap payload");
    let mut payload = vec![0; length];
    stream.read_exact(&mut payload).unwrap();
    (header[0], payload)
}
