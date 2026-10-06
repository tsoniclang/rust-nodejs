use super::WorkerResources;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::process::{Child, Command, ExitStatus};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};
use tsonic_rust_runtime::dispatch_queue::{TaskReservation, TaskTicket};

use rand::rngs::OsRng;
use rand::RngCore;
use tsonic_rust_js::{JsArray, JsValue};
use tsonic_rust_runtime::Callable;

use crate::error::{NodeError, NodeResult};
use crate::events::{EventEmitter, WeakEventEmitter};

use super::clone::{decode, encode, ClonedValue};
use super::protocol::{
    io_error, read_frame, write_frame, TransportEvent, WorkerFrameKind, WorkerTransport,
};
use super::{
    encode_environment_data_snapshot, AUTHENTICATION_TOKEN_BYTES, WORKER_ARGUMENT_DELIMITER,
    WORKER_ARGUMENT_MARKER,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const MAXIMUM_QUEUED_SIGNALS: usize = 1 << 16;

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
            env: JsValue::Null,
            worker_data: JsValue::Null,
        }
    }
}

pub struct Worker<E: 'static = tsonic_rust_runtime::TsonicError> {
    state: Rc<RefCell<WorkerState>>,
    emitter: EventEmitter<E>,
}

struct WorkerState {
    reservation: TaskReservation,
    pending: VecDeque<(TaskTicket, WorkerSignal)>,
    child: Child,
    transport: WorkerTransport,
    incoming: Receiver<TransportEvent>,
    thread_id: i32,
    refed: bool,
    complete: bool,
    transport_ended: bool,
    exit_code: Option<i32>,
}

enum WorkerSignal {
    Message(ClonedValue),
    Error(String),
    Exit(i32),
}

impl<E: 'static> Clone for Worker<E> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
            emitter: self.emitter.clone(),
        }
    }
}

impl<E: From<NodeError> + 'static> Worker<E> {
    pub fn spawn_default(
        module_entry_identity: &str,
        resources: &WorkerResources<E>,
    ) -> NodeResult<Self> {
        Self::spawn_with_options(module_entry_identity, WorkerOptions::default(), resources)
    }

    pub fn spawn_with_options(
        module_entry_identity: &str,
        options: WorkerOptions,
        resources: &WorkerResources<E>,
    ) -> NodeResult<Self> {
        if module_entry_identity.is_empty() {
            return Err(NodeError::new(
                "ERR_WORKER_PATH",
                "worker module entry identity cannot be empty",
            ));
        }
        let reservation = super::resources::reserve_resource()?;
        let listener =
            TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).map_err(io_error)?;
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
                command.arg(value);
            }
        }
        apply_environment(&mut command, &options.env)?;

        let mut startup = WorkerStartup {
            child: Some(command.spawn().map_err(io_error)?),
        };
        let mut stream = accept_worker(
            &listener,
            startup.child.as_mut().expect("starting native worker"),
        )?;
        let authentication = read_frame(&mut stream)?;
        if authentication.kind != WorkerFrameKind::Authenticate
            || !constant_time_equal(&authentication.payload, &token)
        {
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
                reservation,
                pending: VecDeque::new(),
                child: startup.child.take().expect("authenticated native worker"),
                transport,
                incoming,
                thread_id,
                refed: true,
                complete: false,
                transport_ended: false,
                exit_code: None,
            })),
            emitter: EventEmitter::new(),
        };
        resources.register_worker(&value);
        Ok(value)
    }

    pub fn thread_id(&self) -> i32 {
        self.state.borrow().thread_id
    }

    pub fn post_message(&self, value: JsValue) -> NodeResult<()> {
        let state = self.state.borrow();
        if state.complete {
            return Err(NodeError::new(
                "ERR_WORKER_NOT_RUNNING",
                "worker is no longer running",
            ));
        }
        let payload = encode(&ClonedValue::from_js(&value)?)?;
        state.transport.send(WorkerFrameKind::Message, &payload)
    }

    pub async fn terminate(&self) -> NodeResult<i32> {
        let exit_code = {
            let mut state = self.state.borrow_mut();
            if let Some(exit_code) = state.exit_code {
                exit_code
            } else {
                state.child.kill().map_err(io_error)?;
                let status = state.child.wait().map_err(io_error)?;
                let exit_code = status_code(status);
                state.complete(exit_code)?;
                exit_code
            }
        };
        Ok(exit_code)
    }

    pub fn ref_chain(&self) -> &Self {
        let mut state = self.state.borrow_mut();
        if !state.complete {
            state.refed = true;
        }
        drop(state);
        self
    }

    pub fn unref(&self) -> &Self {
        self.state.borrow_mut().refed = false;
        self
    }

    pub fn on_callable(
        &self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.emitter.on_callable(event, listener)?;
        Ok(self)
    }

    pub fn on_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.emitter.on_callable1(event, listener)?;
        Ok(self)
    }

    pub fn once_callable(
        &self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.emitter.once_callable(event, listener)?;
        Ok(self)
    }

    pub fn once_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.emitter.once_callable1(event, listener)?;
        Ok(self)
    }

    pub fn off_callable(
        &self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.emitter.off_callable(event, listener)?;
        Ok(self)
    }

    pub fn off_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.emitter.off_callable1(event, listener)?;
        Ok(self)
    }

    pub(super) fn capture(&self) -> Result<Option<TaskTicket>, E> {
        let captured = {
            let mut state = self.state.borrow_mut();
            state
                .ingest_signals()
                .map(|()| state.pending.back().map(|(ticket, _)| *ticket))
        };
        captured.map_err(E::from)
    }

    pub(super) fn poll(&self, boundary: Option<TaskTicket>) -> Result<bool, E> {
        let Some(boundary) = boundary else {
            return Ok(false);
        };
        let mut dispatched = false;
        loop {
            let signal = {
                let mut state = self.state.borrow_mut();
                if !state
                    .pending
                    .front()
                    .is_some_and(|(ticket, _)| *ticket <= boundary)
                {
                    break;
                }
                state.pending.pop_front().map(|(_, signal)| signal)
            };
            let Some(signal) = signal else {
                break;
            };

            let converted;
            let (event, arguments): (&str, &[JsValue]) = match &signal {
                WorkerSignal::Message(value) => {
                    converted = value.to_js();
                    ("message", std::slice::from_ref(&converted))
                }
                WorkerSignal::Error(error) => {
                    converted = JsValue::String((error).to_owned());
                    ("error", std::slice::from_ref(&converted))
                }
                WorkerSignal::Exit(code) => {
                    converted = JsValue::from(*code);
                    ("exit", std::slice::from_ref(&converted))
                }
            };
            let emission = self.emitter.prepare_named_emission(event, arguments)?;
            emission.invoke(arguments)?;
            dispatched = true;
        }
        Ok(dispatched)
    }

    pub(super) fn is_refed_active(&self) -> bool {
        let state = self.state.borrow();
        state.refed && !state.complete
    }
}

impl WorkerState {
    fn ingest_signals(&mut self) -> NodeResult<()> {
        while self.pending.len() < MAXIMUM_QUEUED_SIGNALS {
            match self.incoming.try_recv() {
                Ok(TransportEvent::Frame(frame)) => match frame.kind {
                    WorkerFrameKind::Message => self.pending.push_back((
                        super::resources::admit_signal()?,
                        WorkerSignal::Message(decode(&frame.payload)?),
                    )),
                    WorkerFrameKind::Error => {
                        self.pending.push_back((
                            super::resources::admit_signal()?,
                            WorkerSignal::Error(
                                String::from_utf8_lossy(&frame.payload).into_owned(),
                            ),
                        ));
                    }
                    WorkerFrameKind::Close => break,
                    _ => self.pending.push_back((
                        super::resources::admit_signal()?,
                        WorkerSignal::Error("unexpected worker transport frame".to_string()),
                    )),
                },
                Ok(TransportEvent::Failure(error)) => {
                    self.pending.push_back((
                        super::resources::admit_signal()?,
                        WorkerSignal::Error(error),
                    ));
                    break;
                }
                Ok(TransportEvent::End) | Err(TryRecvError::Disconnected) => {
                    self.transport_ended = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        if !self.complete && self.pending.len() < MAXIMUM_QUEUED_SIGNALS {
            if let Some(status) = self.child.try_wait().map_err(io_error)? {
                let code = status_code(status);
                self.complete(code)?;
            }
        }
        Ok(())
    }

    fn complete(&mut self, exit_code: i32) -> NodeResult<()> {
        if self.complete {
            return Ok(());
        }
        let ticket = super::resources::admit_signal()?;
        self.pending
            .push_back((ticket, WorkerSignal::Exit(exit_code)));
        self.complete = true;
        self.refed = false;
        self.exit_code = Some(exit_code);
        self.transport.close();
        Ok(())
    }
}

struct WorkerStartup {
    child: Option<Child>,
}

impl Drop for WorkerStartup {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
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

pub(super) struct RuntimeWorker<E: 'static> {
    state: Weak<RefCell<WorkerState>>,
    emitter: WeakEventEmitter<E>,
}

impl<E: 'static> Clone for RuntimeWorker<E> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            emitter: self.emitter.clone(),
        }
    }
}

impl<E: 'static> RuntimeWorker<E> {
    pub(super) fn is_alive(&self) -> bool {
        self.state.strong_count() > 0 && self.emitter.is_alive()
    }

    pub(super) fn upgrade(&self) -> Option<Worker<E>> {
        Some(Worker {
            state: self.state.upgrade()?,
            emitter: self.emitter.upgrade()?,
        })
    }
}

impl<E: From<NodeError> + 'static> Worker<E> {
    pub(super) fn downgrade(&self) -> RuntimeWorker<E> {
        RuntimeWorker {
            state: Rc::downgrade(&self.state),
            emitter: self.emitter.downgrade(),
        }
    }

    pub(super) fn ticket(&self) -> TaskTicket {
        self.state.borrow().reservation.ticket()
    }

    pub(super) fn next_reap_delay(&self) -> Option<Duration> {
        let state = self.state.borrow();
        (state.transport_ended && !state.complete).then_some(Duration::from_millis(1))
    }
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
        JsValue::Null => Ok(()),
        JsValue::Object(object) => {
            let entries = object
                .try_borrow()
                .map_err(|_| {
                    NodeError::new(
                        "ERR_WORKER_OPTIONS",
                        "WorkerOptions.env is mutably borrowed",
                    )
                })?
                .entries_exact();
            command.env_clear();
            for (key, value) in entries {
                let key = key.to_utf8().map_err(|_| {
                    NodeError::new(
                        "ERR_WORKER_OPTIONS",
                        "WorkerOptions.env key is not a native string",
                    )
                })?;
                apply_environment_value(command, &key, &value)?;
            }
            Ok(())
        }
        JsValue::Record(record) => {
            command.env_clear();
            record.with_entries(|entries| {
                for (key, value) in entries {
                    apply_environment_value(command, key, value)?;
                }
                Ok(())
            })
        }
        _ => Err(NodeError::new(
            "ERR_WORKER_OPTIONS",
            "WorkerOptions.env must be a closed object or undefined",
        )),
    }
}

fn apply_environment_value(command: &mut Command, key: &str, value: &JsValue) -> NodeResult<()> {
    match value {
        JsValue::Null => Ok(()),
        JsValue::String(value) => {
            command.env(key, value);
            Ok(())
        }
        _ => Err(NodeError::new(
            "ERR_WORKER_OPTIONS",
            "WorkerOptions.env values must be strings or undefined",
        )),
    }
}

#[cfg(test)]
#[path = "../../../../tests/node/worker_environment_tests.rs"]
mod environment_tests;

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

static NEXT_THREAD_ID: AtomicI32 = AtomicI32::new(1);
