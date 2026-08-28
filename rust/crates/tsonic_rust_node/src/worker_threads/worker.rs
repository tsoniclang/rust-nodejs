use std::cell::RefCell;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::process::{Child, Command, ExitStatus};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use rand::rngs::OsRng;
use rand::RngCore;
use tsonic_rust_js::{JsArray, JsString, JsValue};
use tsonic_rust_runtime::Callable;

use crate::error::{NodeError, NodeResult};
use crate::events::EventEmitter;

use super::clone::{decode, encode, ClonedValue};
use super::protocol::{
    io_error, read_frame, write_frame, TransportEvent, WorkerFrameKind, WorkerTransport,
};
use super::{
    encode_environment_data_snapshot, AUTHENTICATION_TOKEN_BYTES, WORKER_ARGUMENT_DELIMITER,
    WORKER_ARGUMENT_MARKER,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct WorkerOptions {
    pub name: Option<String>,
    pub argv: Option<JsArray<String>>,
    pub env: JsValue,
    pub worker_data: JsValue,
}

impl Default for WorkerOptions {
    fn default() -> Self {
        Self {
            name: None,
            argv: None,
            env: JsValue::Undefined,
            worker_data: JsValue::Undefined,
        }
    }
}

#[derive(Clone)]
pub struct Worker {
    state: Rc<RefCell<WorkerState>>,
    emitter: Rc<RefCell<EventEmitter>>,
}

struct WorkerState {
    child: Child,
    transport: WorkerTransport,
    incoming: Receiver<TransportEvent>,
    thread_id: i32,
    refed: bool,
    complete: bool,
    exit_code: Option<i32>,
    errors: Vec<String>,
    messages: Vec<ClonedValue>,
}

enum WorkerSignal {
    Message(JsValue),
    Error(String),
    Exit(i32),
}

impl Worker {
    pub fn spawn_default(module_entry_identity: &str) -> NodeResult<Self> {
        Self::spawn_with_options(module_entry_identity, WorkerOptions::default())
    }

    pub fn spawn_with_options(
        module_entry_identity: &str,
        options: WorkerOptions,
    ) -> NodeResult<Self> {
        if module_entry_identity.is_empty() {
            return Err(NodeError::new(
                "ERR_WORKER_PATH",
                "worker module entry identity cannot be empty",
            ));
        }
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .map_err(io_error)?;
        listener.set_nonblocking(true).map_err(io_error)?;
        let selected_port = listener.local_addr().map_err(io_error)?.port();
        let thread_id = NEXT_THREAD_ID.fetch_add(1, Ordering::SeqCst);
        let mut token = [0_u8; AUTHENTICATION_TOKEN_BYTES];
        OsRng.fill_bytes(&mut token);

        let mut command = Command::new(std::env::current_exe().map_err(io_error)?);
        command
            .arg(WORKER_ARGUMENT_MARKER)
            .arg(module_entry_identity)
            .arg(selected_port.to_string())
            .arg(encode_token(&token))
            .arg(thread_id.to_string())
            .arg(options.name.as_deref().unwrap_or_default())
            .arg(WORKER_ARGUMENT_DELIMITER);
        if let Some(argv) = &options.argv {
            for value in argv.values() {
                command.arg(value.ok_or_else(|| {
                    NodeError::new("ERR_WORKER_OPTIONS", "WorkerOptions.argv cannot be sparse")
                })?);
            }
        }
        apply_environment(&mut command, &options.env)?;

        let mut child = command.spawn().map_err(io_error)?;
        let mut stream = match accept_worker(&listener, &mut child) {
            Ok(stream) => stream,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        let authentication = read_frame(&mut stream)?;
        if authentication.kind != WorkerFrameKind::Authenticate
            || !constant_time_equal(&authentication.payload, &token)
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(NodeError::new(
                "ERR_WORKER_AUTHENTICATION",
                "worker process authentication failed",
            ));
        }
        let worker_data = encode(&ClonedValue::from_js(&options.worker_data)?)?;
        write_frame(&mut stream, WorkerFrameKind::WorkerData, &worker_data)?;
        write_frame(
            &mut stream,
            WorkerFrameKind::EnvironmentData,
            &encode_environment_data_snapshot()?,
        )?;
        let (transport, incoming) = WorkerTransport::start(stream)?;
        let value = Self {
            state: Rc::new(RefCell::new(WorkerState {
                child,
                transport,
                incoming,
                thread_id,
                refed: true,
                complete: false,
                exit_code: None,
                errors: Vec::new(),
                messages: Vec::new(),
            })),
            emitter: Rc::new(RefCell::new(EventEmitter::new())),
        };
        register_worker(&value);
        Ok(value)
    }

    pub fn thread_id(&self) -> i32 {
        self.state.borrow().thread_id
    }

    pub fn post_message(&self, value: JsValue) -> NodeResult<()> {
        let state = self.state.borrow();
        if state.complete {
            return Err(NodeError::new("ERR_WORKER_NOT_RUNNING", "worker is no longer running"));
        }
        let payload = encode(&ClonedValue::from_js(&value)?)?;
        state.transport.send(WorkerFrameKind::Message, &payload)
    }

    pub fn terminate(&mut self) -> NodeResult<tsonic_rust_js::JsPromise<'static, i32>> {
        let exit_code = {
            let mut state = self.state.borrow_mut();
            if let Some(exit_code) = state.exit_code {
                exit_code
            } else {
                state.child.kill().map_err(io_error)?;
                let status = state.child.wait().map_err(io_error)?;
                let exit_code = status_code(status);
                state.complete(exit_code);
                exit_code
            }
        };
        Ok(tsonic_rust_js::JsPromise::resolved(exit_code))
    }

    pub fn ref_chain(&mut self) -> &mut Self {
        let mut state = self.state.borrow_mut();
        if !state.complete {
            state.refed = true;
        }
        drop(state);
        self
    }

    pub fn unref(&mut self) -> &mut Self {
        self.state.borrow_mut().refed = false;
        self
    }

    pub fn on_callable<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.emitter.borrow_mut().on_callable(event, listener)?;
        Ok(self)
    }

    pub fn on_callable1<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<JsValue, Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.emitter.borrow_mut().on_callable1(event, listener)?;
        Ok(self)
    }

    pub fn once_callable<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.emitter.borrow_mut().once_callable(event, listener)?;
        Ok(self)
    }

    pub fn once_callable1<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<JsValue, Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.emitter.borrow_mut().once_callable1(event, listener)?;
        Ok(self)
    }

    pub fn off_callable<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: 'static,
    {
        self.emitter.borrow_mut().off_callable(event, listener)?;
        Ok(self)
    }

    pub fn off_callable1<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<JsValue, Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: 'static,
    {
        self.emitter.borrow_mut().off_callable1(event, listener)?;
        Ok(self)
    }

    fn poll(&self) -> NodeResult<bool> {
        let signals = self.state.borrow_mut().signals()?;
        for signal in &signals {
            let emission = {
                let mut emitter = self.emitter.borrow_mut();
                match signal {
                    WorkerSignal::Message(value) => emitter.prepare_callable_emission(
                        &event_name("message"),
                        std::slice::from_ref(value),
                    )?,
                    WorkerSignal::Error(error) => emitter.prepare_callable_emission(
                        &event_name("error"),
                        &[JsValue::String(JsString::from_utf8(error))],
                    )?,
                    WorkerSignal::Exit(code) => emitter.prepare_callable_emission(
                        &event_name("exit"),
                        &[JsValue::Number(f64::from(*code))],
                    )?,
                }
            };
            emission.invoke()?;
        }
        Ok(!signals.is_empty())
    }

    fn is_refed_active(&self) -> bool {
        let state = self.state.borrow();
        state.refed && !state.complete
    }
}

impl WorkerState {
    fn signals(&mut self) -> NodeResult<Vec<WorkerSignal>> {
        loop {
            match self.incoming.try_recv() {
                Ok(TransportEvent::Frame(frame)) => match frame.kind {
                    WorkerFrameKind::Message => self.messages.push(decode(&frame.payload)?),
                    WorkerFrameKind::Error => {
                        self.errors.push(String::from_utf8_lossy(&frame.payload).into_owned());
                    }
                    WorkerFrameKind::Close => break,
                    _ => self.errors.push("unexpected worker transport frame".to_string()),
                },
                Ok(TransportEvent::Failure(error)) => {
                    self.errors.push(error);
                    break;
                }
                Ok(TransportEvent::End) | Err(TryRecvError::Disconnected) => break,
                Err(TryRecvError::Empty) => break,
            }
        }
        let mut signals = self
            .messages
            .drain(..)
            .map(|value| WorkerSignal::Message(value.to_js()))
            .chain(self.errors.drain(..).map(WorkerSignal::Error))
            .collect::<Vec<_>>();
        if !self.complete {
            if let Some(status) = self.child.try_wait().map_err(io_error)? {
                let code = status_code(status);
                self.complete(code);
                signals.push(WorkerSignal::Exit(code));
            }
        }
        Ok(signals)
    }

    fn complete(&mut self, exit_code: i32) {
        if self.complete {
            return;
        }
        self.complete = true;
        self.refed = false;
        self.exit_code = Some(exit_code);
        self.transport.close();
    }
}

impl Drop for WorkerState {
    fn drop(&mut self) {
        if !self.complete {
            let _ = self.child.kill();
            let _ = self.child.wait();
            self.transport.close();
        }
    }
}

thread_local! {
    static WORKERS: RefCell<Vec<(Weak<RefCell<WorkerState>>, Weak<RefCell<EventEmitter>>)>> =
        const { RefCell::new(Vec::new()) };
}

fn register_worker(worker: &Worker) {
    WORKERS.with(|workers| {
        workers.borrow_mut().push((
            Rc::downgrade(&worker.state),
            Rc::downgrade(&worker.emitter),
        ));
    });
}

pub(crate) fn poll_runtime_workers() -> tsonic_rust_runtime::TsonicResult<bool> {
    let workers = WORKERS.with(|workers| {
        let mut workers = workers.borrow_mut();
        workers.retain(|(state, emitter)| state.strong_count() > 0 && emitter.strong_count() > 0);
        workers
            .iter()
            .filter_map(|(state, emitter)| Some((state.upgrade()?, emitter.upgrade()?)))
            .map(|(state, emitter)| Worker { state, emitter })
            .collect::<Vec<_>>()
    });
    let mut did_work = false;
    for worker in workers {
        did_work |= worker.poll().map_err(tsonic_rust_runtime::TsonicError::from)?;
    }
    Ok(did_work)
}

pub(crate) fn has_refed_runtime_workers() -> bool {
    WORKERS.with(|workers| {
        workers.borrow().iter().any(|(state, emitter)| {
            let (Some(state), Some(emitter)) = (state.upgrade(), emitter.upgrade()) else {
                return false;
            };
            Worker { state, emitter }.is_refed_active()
        })
    })
}

fn accept_worker(listener: &TcpListener, child: &mut Child) -> NodeResult<std::net::TcpStream> {
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, address)) => {
                if !address.ip().is_loopback() {
                    continue;
                }
                return Ok(stream);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if let Some(status) = child.try_wait().map_err(io_error)? {
                    return Err(NodeError::new(
                        "ERR_WORKER_STARTUP",
                        format!("worker process exited before authentication with status {status}"),
                    ));
                }
                if Instant::now() >= deadline {
                    return Err(NodeError::new(
                        "ERR_WORKER_STARTUP_TIMEOUT",
                        "worker process did not authenticate within the finite startup deadline",
                    ));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(io_error(error)),
        }
    }
}

fn apply_environment(command: &mut Command, value: &JsValue) -> NodeResult<()> {
    match value {
        JsValue::Undefined => Ok(()),
        JsValue::Object(object) => {
            let entries = object.try_borrow().map_err(|_| {
                NodeError::new("ERR_WORKER_OPTIONS", "WorkerOptions.env is mutably borrowed")
            })?.entries_exact();
            command.env_clear();
            for (key, value) in entries {
                let key = key.to_utf8().map_err(|_| {
                    NodeError::new("ERR_WORKER_OPTIONS", "WorkerOptions.env key is not a native string")
                })?;
                match value {
                    JsValue::Undefined => {}
                    JsValue::String(value) => {
                        command.env(key, value.to_utf8().map_err(|_| {
                            NodeError::new("ERR_WORKER_OPTIONS", "WorkerOptions.env value is not a native string")
                        })?);
                    }
                    _ => {
                        return Err(NodeError::new(
                            "ERR_WORKER_OPTIONS",
                            "WorkerOptions.env values must be strings or undefined",
                        ));
                    }
                }
            }
            Ok(())
        }
        _ => Err(NodeError::new(
            "ERR_WORKER_OPTIONS",
            "WorkerOptions.env must be a closed object or undefined",
        )),
    }
}

fn encode_token(token: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(token.len() * 2);
    for value in token {
        output.push(char::from(HEX[usize::from(value >> 4)]));
        output.push(char::from(HEX[usize::from(value & 0x0f)]));
    }
    output
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        let left = left.get(index).copied().unwrap_or_default();
        let right = right.get(index).copied().unwrap_or_default();
        difference |= usize::from(left ^ right);
    }
    difference == 0
}

fn status_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}

fn event_name(value: &str) -> JsValue {
    JsValue::String(JsString::from_utf8(value))
}

static NEXT_THREAD_ID: AtomicI32 = AtomicI32::new(1);
