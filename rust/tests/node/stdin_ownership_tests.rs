#![cfg(target_os = "linux")]

use std::cell::{Cell, RefCell};
use std::io::{BufRead, BufReader, Read, Seek, Write};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tsonic_rust_node::background::BackgroundTasks;
use tsonic_rust_node::process::{self, StdinResources};
use tsonic_rust_node::runtime_tasks::RuntimeTasks;
use tsonic_rust_node::{buffer::Buffer, NodeError};
use tsonic_rust_runtime::dispatch::{prepend, DispatchContexts, DispatchEnd};
use tsonic_rust_runtime::{Callable, TsonicError};

static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);

#[test]
fn native_stdin_child() {
    let Ok(mode) = std::env::var("TSONIC_STDIN_PROOF") else {
        return;
    };
    if mode == "failure" {
        check_failure_identity();
        return;
    }
    let tasks = RuntimeTasks::<TsonicError>::new();
    let background = BackgroundTasks::<TsonicError>::new();
    let roots = StdinResources::<TsonicError>::new();
    let before_flags = nix::fcntl::fcntl(std::io::stdin(), nix::fcntl::FcntlArg::F_GETFL).unwrap();
    let stream = process::stdin(&roots, &background, tasks.handle()).unwrap();
    assert_eq!(
        stream,
        process::stdin(&roots, &background, tasks.handle()).unwrap()
    );
    assert_eq!(
        nix::fcntl::fcntl(std::io::stdin(), nix::fcntl::FcntlArg::F_GETFL).unwrap(),
        before_flags
    );
    if mode == "readline" || mode == "readline-cancel" || mode == "readline-file" {
        check_readline(
            mode == "readline-cancel",
            stream,
            &tasks,
            &background,
            &roots,
        );
        return;
    }
    if mode == "readline-drop" {
        check_dropped_readline(stream, &tasks, &background, &roots);
        return;
    }
    if mode != "file" && mode != "null" {
        assert!(stream.read().unwrap().is_none());
    }
    println!("READY");
    std::io::stdout().flush().unwrap();
    let bytes = Rc::new(RefCell::new(Vec::new()));
    let trace = Rc::new(RefCell::new(Vec::new()));
    let collected = bytes.clone();
    let observed = trace.clone();
    let control = stream.clone();
    let terminal = mode == "terminal";
    let pause_after_first = mode == "pause";
    let listener = Callable::new(move |(chunk,): (Buffer,)| {
        collected.borrow_mut().extend_from_slice(&chunk.as_bytes());
        observed.borrow_mut().push("data");
        if terminal {
            control.destroy()?;
        } else if pause_after_first && observed.borrow().len() == 1 {
            control.pause();
        }
        Ok::<(), TsonicError>(())
    });
    stream.on_data("data", &listener).unwrap();
    let observed = trace.clone();
    stream
        .once_end(
            "end",
            &Callable::new(move |()| {
                observed.borrow_mut().push("end");
                Ok(())
            }),
        )
        .unwrap();
    tasks
        .enqueue(|| {
            println!("TASK_WITH_INPUT_WITHHELD");
            std::io::stdout().flush().unwrap();
            Ok(())
        })
        .unwrap();
    stream.resume().unwrap();
    let end = DispatchEnd::<TsonicError>::new();
    tsonic_rust_node::run_with_contexts(prepend(
        &roots,
        prepend(&background, prepend(&tasks, &end)),
    ))
    .unwrap();
    if pause_after_first {
        assert!(stream.is_paused());
        assert!(!stream.readable_ended());
        assert_eq!(trace.borrow().as_slice(), ["data"]);
        assert_eq!(stream.readable_length(), 0);
        assert!(!roots.has_work());
        assert!(!background.has_pending_work());
        assert!(!tasks.has_pending_work());
        println!("PAUSED_WITHOUT_PENDING_INPUT_WORK");
        std::io::stdout().flush().unwrap();
        stream.resume().unwrap();
        tsonic_rust_node::run_with_contexts(prepend(
            &roots,
            prepend(&background, prepend(&tasks, &end)),
        ))
        .unwrap();
    }
    if terminal {
        assert!(process::stdin_is_tty());
        assert!(!stream.readable_ended());
        assert!(stream.destroyed());
    } else {
        assert!(stream.readable_ended());
        assert_eq!(trace.borrow().last(), Some(&"end"));
        assert_eq!(
            trace
                .borrow()
                .iter()
                .filter(|&&event| event == "end")
                .count(),
            1
        );
    }
    stream.off_data("data", &listener).unwrap();
    let actual = String::from_utf8(bytes.borrow().clone()).unwrap();
    println!("BYTES:{actual:?}");
    assert_eq!(
        nix::fcntl::fcntl(std::io::stdin(), nix::fcntl::FcntlArg::F_GETFL).unwrap(),
        before_flags
    );
}

fn child(mode: &str, input: Stdio) -> std::process::Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "native_stdin_child", "--nocapture"])
        .env("TSONIC_STDIN_PROOF", mode)
        .stdin(input)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

fn await_marker(output: &mut BufReader<std::process::ChildStdout>, expected: &str) {
    loop {
        let mut line = String::new();
        assert!(
            output.read_line(&mut line).unwrap() != 0,
            "stdin child ended before {expected}"
        );
        if line.trim() == expected {
            return;
        }
    }
}

fn finish(
    mut process: std::process::Child,
    mut output: BufReader<std::process::ChildStdout>,
) -> String {
    let mut text = String::new();
    output.read_to_string(&mut text).unwrap();
    let status = process.wait().unwrap();
    let mut error = String::new();
    process
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut error)
        .unwrap();
    assert!(
        status.success(),
        "stdin child failed: {status}; {error}; {text}"
    );
    text
}

#[test]
fn withheld_pipe_input_does_not_block_tasks_and_flowing_drains_to_deferred_eof() {
    let mut process = child("pipe", Stdio::piped());
    let mut output = BufReader::new(process.stdout.take().unwrap());
    await_marker(&mut output, "READY");
    await_marker(&mut output, "TASK_WITH_INPUT_WITHHELD");
    let mut input = process.stdin.take().unwrap();
    let text = "native 🦀 input";
    input.write_all(text.as_bytes()).unwrap();
    drop(input);
    assert!(finish(process, output).contains(&format!("BYTES:{text:?}")));
}

#[test]
fn paused_pipe_releases_readiness_interest_and_resumes_the_same_input_owner() {
    let mut process = child("pause", Stdio::piped());
    let mut output = BufReader::new(process.stdout.take().unwrap());
    await_marker(&mut output, "READY");
    await_marker(&mut output, "TASK_WITH_INPUT_WITHHELD");
    let mut input = process.stdin.take().unwrap();
    input.write_all(b"first").unwrap();
    await_marker(&mut output, "PAUSED_WITHOUT_PENDING_INPUT_WORK");
    input.write_all(b"second").unwrap();
    drop(input);
    assert!(finish(process, output).contains("BYTES:\"firstsecond\""));
}

#[test]
fn regular_stdin_uses_existing_background_file_owner_at_the_current_offset() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".temp");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!(
        "stdin-{}-{}",
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, b"skipnative file bytes").unwrap();
    let mut file = std::fs::File::open(&path).unwrap();
    file.seek(std::io::SeekFrom::Start(4)).unwrap();
    let mut process = child("file", Stdio::from(file));
    let mut output = BufReader::new(process.stdout.take().unwrap());
    await_marker(&mut output, "READY");
    let result = finish(process, output);
    std::fs::remove_file(&path).unwrap();
    assert!(result.contains("BYTES:\"native file bytes\""));
}

#[test]
fn null_device_flowing_reaches_eof_without_poll_registration_or_replayed_callbacks() {
    let mut process = child("null", Stdio::null());
    let mut output = BufReader::new(process.stdout.take().unwrap());
    await_marker(&mut output, "READY");
    assert!(finish(process, output).contains("BYTES:\"\""));
}

fn check_readline(
    cancel: bool,
    stream: tsonic_rust_node::stream::Readable<TsonicError>,
    tasks: &RuntimeTasks<TsonicError>,
    background: &BackgroundTasks<TsonicError>,
    roots: &StdinResources<TsonicError>,
) {
    let mut interface = tsonic_rust_node::readline::create_interface(
        tsonic_rust_node::readline::SourceInterfaceOptions {
            input: stream.clone(),
            output: None,
            terminal: None,
            prompt: None,
        },
    );
    let answers = Rc::new(RefCell::new(Vec::new()));
    let observed = answers.clone();
    interface
        .question_callable(
            tasks,
            "",
            Callable::new(move |(answer,)| {
                assert!(!cancel, "canceled native readline callback");
                observed.borrow_mut().push(answer);
                Ok(())
            }),
        )
        .unwrap();
    if cancel {
        interface.close();
    }
    println!("READY");
    std::io::stdout().flush().unwrap();
    tasks
        .enqueue(|| {
            println!("TASK_WITH_INPUT_WITHHELD");
            std::io::stdout().flush().unwrap();
            Ok(())
        })
        .unwrap();
    let end = DispatchEnd::<TsonicError>::new();
    tsonic_rust_node::run_with_contexts(prepend(roots, prepend(background, prepend(tasks, &end))))
        .unwrap();
    if cancel {
        assert!(answers.borrow().is_empty());
        println!("CANCELED_WITHOUT_BLOCKING_WORKER");
        return;
    }
    assert_eq!(answers.borrow().len(), 1);
    let observed = answers.clone();
    interface
        .question_callable(
            tasks,
            "",
            Callable::new(move |(answer,)| {
                observed.borrow_mut().push(answer);
                Ok(())
            }),
        )
        .unwrap();
    tsonic_rust_node::run_with_contexts(prepend(roots, prepend(background, prepend(tasks, &end))))
        .unwrap();
    assert_eq!(*answers.borrow(), ["native 🦀 one", "native two"]);
    stream.destroy().unwrap();
    println!("EXACT_READLINE_ANSWERS");
}

#[test]
fn readline_uses_the_same_nonblocking_pipe_owner_and_preserves_unread_lines() {
    let mut process = child("readline", Stdio::piped());
    let mut output = BufReader::new(process.stdout.take().unwrap());
    await_marker(&mut output, "READY");
    await_marker(&mut output, "TASK_WITH_INPUT_WITHHELD");
    let mut input = process.stdin.take().unwrap();
    input
        .write_all("native 🦀 one\r\nnative two\n".as_bytes())
        .unwrap();
    drop(input);
    assert!(finish(process, output).contains("EXACT_READLINE_ANSWERS"));
}

#[test]
fn closing_readline_cancels_the_wait_without_leaving_a_blocked_stdin_worker() {
    let mut process = child("readline-cancel", Stdio::piped());
    let mut output = BufReader::new(process.stdout.take().unwrap());
    await_marker(&mut output, "READY");
    await_marker(&mut output, "TASK_WITH_INPUT_WITHHELD");
    assert!(finish(process, output).contains("CANCELED_WITHOUT_BLOCKING_WORKER"));
}

fn check_dropped_readline(
    stream: tsonic_rust_node::stream::Readable<TsonicError>,
    tasks: &RuntimeTasks<TsonicError>,
    background: &BackgroundTasks<TsonicError>,
    roots: &StdinResources<TsonicError>,
) {
    let retained = Rc::new(());
    let weak = Rc::downgrade(&retained);
    let answer = Rc::new(RefCell::new(None));
    let observed = answer.clone();
    let mut reader = tsonic_rust_node::readline::create_interface(
        tsonic_rust_node::readline::SourceInterfaceOptions {
            input: stream.clone(),
            output: None,
            terminal: None,
            prompt: None,
        },
    );
    reader
        .question_callable(
            tasks,
            "",
            Callable::new(move |(line,)| {
                let _capture = &retained;
                *observed.borrow_mut() = Some(line);
                Ok(())
            }),
        )
        .unwrap();
    drop(reader);
    assert!(weak.upgrade().is_some());
    println!("READY");
    std::io::stdout().flush().unwrap();
    let end = DispatchEnd::<TsonicError>::new();
    tsonic_rust_node::run_with_contexts(prepend(roots, prepend(background, prepend(tasks, &end))))
        .unwrap();
    assert_eq!(answer.borrow().as_deref(), Some("owned native answer"));
    assert!(weak.upgrade().is_none());
    stream.destroy().unwrap();
    println!("DROPPED_READER_RETAINS_EXACT_INPUT_CALLBACK");
}

#[test]
fn native_input_owner_retains_a_question_after_the_authored_reader_scope_ends() {
    let mut process = child("readline-drop", Stdio::piped());
    let mut output = BufReader::new(process.stdout.take().unwrap());
    await_marker(&mut output, "READY");
    let mut input = process.stdin.take().unwrap();
    input.write_all(b"owned native answer\n").unwrap();
    drop(input);
    assert!(finish(process, output).contains("DROPPED_READER_RETAINS_EXACT_INPUT_CALLBACK"));
}

#[test]
fn regular_stdin_readline_does_not_race_a_second_standard_input_reader() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".temp");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!(
        "stdin-lines-{}-{}",
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, "native 🦀 one\r\nnative two\n".as_bytes()).unwrap();
    let file = std::fs::File::open(&path).unwrap();
    let mut process = child("readline-file", Stdio::from(file));
    let mut output = BufReader::new(process.stdout.take().unwrap());
    await_marker(&mut output, "READY");
    assert!(finish(process, output).contains("EXACT_READLINE_ANSWERS"));
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn terminal_readiness_and_destruction_release_input_without_a_blocked_worker() {
    let terminal = nix::pty::openpty(None, None).unwrap();
    let mut process = child("terminal", Stdio::from(std::fs::File::from(terminal.slave)));
    let mut output = BufReader::new(process.stdout.take().unwrap());
    await_marker(&mut output, "READY");
    await_marker(&mut output, "TASK_WITH_INPUT_WITHHELD");
    let mut master = std::fs::File::from(terminal.master);
    master.write_all(b"terminal bytes\n").unwrap();
    assert!(finish(process, output).contains("BYTES:\"terminal bytes\\n\""));
}

#[derive(Debug)]
enum Failure {
    Original(Rc<Cell<u64>>),
    Native(NodeError),
    Runtime(TsonicError),
}

impl From<NodeError> for Failure {
    fn from(error: NodeError) -> Self {
        Self::Native(error)
    }
}

impl From<TsonicError> for Failure {
    fn from(error: TsonicError) -> Self {
        Self::Runtime(error)
    }
}

fn check_failure_identity() {
    let tasks = RuntimeTasks::<Failure>::new();
    let background = BackgroundTasks::<Failure>::new();
    let roots = StdinResources::<Failure>::new();
    let stream = process::stdin(&roots, &background, tasks.handle()).unwrap();
    let original = Rc::new(Cell::new(9_007_199_254_740_993));
    let captured = original.clone();
    let listener =
        Callable::new(move |(_chunk,): (Buffer,)| Err(Failure::Original(captured.clone())));
    stream.once_data("data", &listener).unwrap();
    println!("READY");
    std::io::stdout().flush().unwrap();
    let end = DispatchEnd::<Failure>::new();
    match tsonic_rust_node::run_with_contexts(prepend(
        &roots,
        prepend(&background, prepend(&tasks, &end)),
    )) {
        Err(Failure::Original(returned)) => assert!(Rc::ptr_eq(&returned, &original)),
        Err(Failure::Native(error)) => panic!("native failure instead of original: {error}"),
        Err(Failure::Runtime(error)) => panic!("runtime failure instead of original: {error}"),
        Ok(_) => panic!("original callback failure was lost"),
    }
    stream.destroy().unwrap();
    tasks.poll().unwrap();
    assert!(!roots.has_work());
    println!("ORIGINAL_FAILURE_RETAINED");
}

#[test]
fn readiness_delivery_preserves_the_original_generic_callback_failure() {
    let mut process = child("failure", Stdio::piped());
    let mut output = BufReader::new(process.stdout.take().unwrap());
    await_marker(&mut output, "READY");
    let mut input = process.stdin.take().unwrap();
    input.write_all(b"fail with original identity").unwrap();
    drop(input);
    assert!(finish(process, output).contains("ORIGINAL_FAILURE_RETAINED"));
}
