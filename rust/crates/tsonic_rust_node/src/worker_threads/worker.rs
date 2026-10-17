use super::WorkerResources;
use std::cell::RefCell;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::process::{Command, ExitStatus};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tsonic_rust_runtime::dispatch::DispatchOwner;
use tsonic_rust_runtime::dispatch_queue::{TaskReservation, TaskTicket};

use rand::rngs::OsRng;
use rand::RngCore;
use tsonic_rust_js::{JsArray, JsValue};
use tsonic_rust_runtime::Callable;

use crate::error::{NodeError, NodeResult};
use crate::events::{EventEmitter, WeakEventEmitter};

use super::clone::{decode, encode, ClonedValue};
use super::protocol::{
    io_error, read_frame, write_frame, WorkerFrame, WorkerFrameKind, WorkerTransport,
};
use super::{
    encode_environment_data_snapshot, AUTHENTICATION_TOKEN_BYTES, WORKER_ARGUMENT_DELIMITER,
    WORKER_ARGUMENT_MARKER,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const MAXIMUM_QUEUED_SIGNALS: usize = 1 << 16;

mod completion;
mod completion_error;
mod dispatch;
mod physical_child;
use completion::{PhysicalSignal, WorkerPhysical};
pub use completion_error::WorkerCompletionError;
pub(super) use dispatch::RuntimeWorker;
use physical_child::WorkerChild;

#[cfg(test)]
mod completion_tests;
#[cfg(test)]
mod physical_tests;

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
    state: Rc<WorkerState>,
    emitter: EventEmitter<E>,
}

struct WorkerState {
    reservation: TaskReservation,
    physical: Arc<WorkerPhysical>,
    transport: WorkerTransport,
    thread_id: i32,
    delivery: RefCell<WorkerDelivery>,
}

struct WorkerDelivery {
    refed: bool,
    owner: Option<DispatchOwner>,
    reader_terminal_seen: bool,
    terminal_consumed: bool,
    native_failure: Option<WorkerCompletionError>,
}

impl WorkerDelivery {
    fn new() -> Self {
        Self {
            refed: true,
            owner: None,
            reader_terminal_seen: false,
            terminal_consumed: false,
            native_failure: None,
        }
    }

    fn record_failure(&mut self, error: NodeError) {
        match &mut self.native_failure {
            Some(first) => first.record_additional(),
            None => self.native_failure = Some(error.into()),
        }
    }

    #[cfg(test)]
    fn native_failure_count(&self) -> u64 {
        self.native_failure.as_ref().map_or(0, |error| {
            error
                .additional_unobserved_errors()
                .checked_add(1)
                .expect("finite native test error count")
        })
    }

    fn consume_native(&mut self, signal: WorkerSignal, physical: &WorkerPhysical) {
        match signal {
            WorkerSignal::Message(_) => {}
            WorkerSignal::Error(error) => self.record_failure(error),
            WorkerSignal::RemoteError(message) => {
                self.record_failure(NodeError::new("ERR_WORKER_ERROR", message))
            }
            WorkerSignal::NativeIoError(error) => {
                if !physical.suppresses_native_io() {
                    self.record_failure(error);
                }
            }
            WorkerSignal::Exit(_) => self.terminal_consumed = true,
        }
    }
}

enum WorkerSignal {
    Message(ClonedValue),
    Error(NodeError),
    RemoteError(String),
    NativeIoError(NodeError),
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
            child: Some(WorkerChild::spawn(&mut command).map_err(io_error)?),
        };
        let mut stream = accept_worker(
            &listener,
            startup.child.as_ref().expect("starting native worker"),
        )?;
        let authentication = read_frame(&mut stream)?.ok_or_else(|| {
            NodeError::new(
                "ERR_WORKER_AUTHENTICATION",
                "worker transport ended before authentication",
            )
        })?;
        if authentication.kind != WorkerFrameKind::Authenticate
            || !constant_time_equal(&authentication.payload, &token)
        {
            return Err(NodeError::new(
                "ERR_WORKER_AUTHENTICATION",
                "worker process authentication failed",
            ));
        }
        write_frame(
            &mut stream,
            WorkerFrameKind::WorkerData,
            &encode(&ClonedValue::from_js(&options.worker_data)?)?,
        )?;
        write_frame(
            &mut stream,
            WorkerFrameKind::EnvironmentData,
            &encode_environment_data_snapshot()?,
        )?;
        let physical = Arc::new(WorkerPhysical::new(
            startup.child.take().expect("authenticated native worker"),
        )?);
        let frames = Arc::clone(&physical);
        let terminal = Arc::clone(&physical);
        let transport = match WorkerTransport::start_observed(
            stream,
            move |frame| frames.enqueue_frame(frame),
            move |error| terminal.finish_reader(error),
        ) {
            Ok(started) => started,
            Err(error) => {
                let _ = physical.child.kill();
                let _ = physical.child.wait();
                return Err(error);
            }
        };
        let value = Self {
            state: Rc::new(WorkerState {
                reservation,
                physical,
                transport,
                thread_id,
                delivery: RefCell::new(WorkerDelivery::new()),
            }),
            emitter: EventEmitter::new(),
        };
        resources.register_worker(&value)?;
        Ok(value)
    }
}

impl<E: 'static> Worker<E> {
    pub fn thread_id(&self) -> i32 {
        self.state.thread_id
    }

    pub fn post_message(&self, value: JsValue) -> NodeResult<()> {
        if self.state.physical.snapshot().process_finished {
            return Err(NodeError::new(
                "ERR_WORKER_NOT_RUNNING",
                "worker is no longer running",
            ));
        }
        self.state.transport.send(
            WorkerFrameKind::Message,
            &encode(&ClonedValue::from_js(&value)?)?,
        )
    }

    pub fn ref_chain(&self) -> &Self {
        self.state.delivery.borrow_mut().refed = true;
        self
    }

    pub fn unref(&self) -> &Self {
        self.state.delivery.borrow_mut().refed = false;
        self
    }
}

impl WorkerState {
    fn ingest_signals(&self, native_observation: bool) -> NodeResult<()> {
        if !native_observation {
            self.physical.admit_frames()?;
            #[cfg(test)]
            completion_tests::at_ingestion_gap();
            return Ok(());
        }
        if self.delivery.borrow().owner.is_some() {
            return Ok(());
        }
        while let Some(signal) = self.next_signal(None, false)? {
            self.delivery
                .borrow_mut()
                .consume_native(signal, &self.physical);
        }
        #[cfg(test)]
        completion_tests::at_ingestion_gap();
        Ok(())
    }

    fn next_signal(
        &self,
        boundary: Option<TaskTicket>,
        include_exit: bool,
    ) -> NodeResult<Option<WorkerSignal>> {
        loop {
            let Some(signal) = self.physical.take_signal(boundary, include_exit) else {
                return Ok(None);
            };
            return Ok(Some(match signal {
                PhysicalSignal::Frame(frame) => match frame.kind {
                    WorkerFrameKind::Message => match decode(&frame.payload) {
                        Ok(value) => WorkerSignal::Message(value),
                        Err(error) => WorkerSignal::Error(error),
                    },
                    WorkerFrameKind::Error => WorkerSignal::RemoteError(
                        String::from_utf8_lossy(&frame.payload).into_owned(),
                    ),
                    _ => WorkerSignal::Error(NodeError::new(
                        "ERR_WORKER_PROTOCOL",
                        "unexpected worker transport frame",
                    )),
                },
                PhysicalSignal::ReaderEnd(error) => {
                    self.delivery.borrow_mut().reader_terminal_seen = true;
                    let Some(error) = error else {
                        continue;
                    };
                    if error.code() == "ERR_WORKER_TRANSPORT" {
                        WorkerSignal::NativeIoError(error)
                    } else {
                        WorkerSignal::Error(error)
                    }
                }
                PhysicalSignal::Exit(result) => WorkerSignal::Exit(result?),
            }));
        }
    }
    fn finish_native_observation(
        &self,
        observed_unowned: bool,
    ) -> Result<i32, WorkerCompletionError> {
        let mut delivery = self.delivery.borrow_mut();
        let process = self
            .physical
            .result()
            .expect("native process result is published before observation")
            .map_err(WorkerCompletionError::from);
        if delivery.owner.is_none() {
            delivery.terminal_consumed = true;
            self.physical.consume_exit();
        }
        if !observed_unowned {
            return process;
        }
        match &delivery.native_failure {
            Some(error) => Err(error.clone()),
            None => process,
        }
    }
}

struct WorkerStartup {
    child: Option<WorkerChild>,
}

impl Drop for WorkerStartup {
    fn drop(&mut self) {
        if let Some(child) = &self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for WorkerState {
    fn drop(&mut self) {
        let _ = self.physical.request_termination();
        self.physical.close_consumer();
        self.transport.close();
    }
}

fn accept_worker(listener: &TcpListener, child: &WorkerChild) -> NodeResult<std::net::TcpStream> {
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
                if let Some(status) = child.try_wait_startup().map_err(io_error)? {
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
        difference |= usize::from(
            left.get(index).copied().unwrap_or_default()
                ^ right.get(index).copied().unwrap_or_default(),
        );
    }
    difference == 0
}

fn status_code(status: ExitStatus) -> NodeResult<i32> {
    if let Some(code) = status.code() {
        return Ok(code);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.signal().map(|signal| 128 + signal).ok_or_else(|| {
            NodeError::new(
                "ERR_WORKER_EXIT_STATUS",
                "native Unix process status has neither an exit code nor a signal",
            )
        })
    }
    #[cfg(not(unix))]
    Err(NodeError::new(
        "ERR_WORKER_EXIT_STATUS",
        "native process status does not expose its required exit code",
    ))
}

static NEXT_THREAD_ID: AtomicI32 = AtomicI32::new(1);
