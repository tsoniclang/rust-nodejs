mod clone;
mod port;
mod protocol;
mod worker;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::time::Duration;

use tsonic_rust_js::JsValue;

use crate::error::{NodeError, NodeResult};

use clone::{decode, encode, ClonedValue};
use protocol::{io_error, read_frame, write_frame, WorkerFrameKind, WorkerTransport};

pub use clone::ClonedValue as StructuredCloneValue;
pub use port::{MessageChannel, MessagePort};
pub use worker::{Worker, WorkerOptions};

pub(crate) const WORKER_ARGUMENT_MARKER: &str = "--tsonic-node-worker-v1";
pub(crate) const WORKER_ARGUMENT_DELIMITER: &str = "--";
pub(crate) const AUTHENTICATION_TOKEN_BYTES: usize = 32;
const MAXIMUM_ENVIRONMENT_ENTRIES: usize = 1 << 20;

struct WorkerProcessContext {
    thread_id: i32,
    worker_data: JsValue,
    parent_port: MessagePort,
    environment_data: BTreeMap<String, ClonedValue>,
}

thread_local! {
    static WORKER_CONTEXT: RefCell<Option<WorkerProcessContext>> = const { RefCell::new(None) };
    static MAIN_ENVIRONMENT_DATA: RefCell<BTreeMap<String, ClonedValue>> =
        const { RefCell::new(BTreeMap::new()) };
    static UNTRANSFERABLE_VALUES: RefCell<BTreeSet<usize>> =
        const { RefCell::new(BTreeSet::new()) };
}

pub fn initialize_worker_process() -> NodeResult<Option<String>> {
    let arguments = std::env::args_os()
        .skip(1)
        .map(|argument| {
            argument.into_string().map_err(|_| {
                NodeError::new("ERR_WORKER_BOOTSTRAP", "worker bootstrap argument is not Unicode")
            })
        })
        .collect::<NodeResult<Vec<_>>>()?;
    if arguments.first().map(String::as_str) != Some(WORKER_ARGUMENT_MARKER) {
        return Ok(None);
    }
    if arguments.len() < 7 || arguments[6] != WORKER_ARGUMENT_DELIMITER {
        return Err(NodeError::new(
            "ERR_WORKER_BOOTSTRAP",
            "worker bootstrap arguments do not match the closed protocol",
        ));
    }
    let entry_identity = arguments[1].clone();
    if entry_identity.is_empty() {
        return Err(NodeError::new("ERR_WORKER_BOOTSTRAP", "worker entry identity is empty"));
    }
    let port = arguments[2].parse::<u16>().map_err(|_| {
        NodeError::new("ERR_WORKER_BOOTSTRAP", "worker bootstrap port is invalid")
    })?;
    if port == 0 {
        return Err(NodeError::new("ERR_WORKER_BOOTSTRAP", "worker bootstrap port is invalid"));
    }
    let token = decode_token(&arguments[3])?;
    let thread_id = arguments[4].parse::<i32>().map_err(|_| {
        NodeError::new("ERR_WORKER_BOOTSTRAP", "worker thread identity is invalid")
    })?;
    if thread_id <= 0 {
        return Err(NodeError::new("ERR_WORKER_BOOTSTRAP", "worker thread identity is invalid"));
    }

    let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
    let mut stream = TcpStream::connect_timeout(&address.into(), Duration::from_secs(30))
        .map_err(io_error)?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(io_error)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .map_err(io_error)?;
    write_frame(&mut stream, WorkerFrameKind::Authenticate, &token)?;
    let worker_data = read_frame(&mut stream)?;
    let environment_data = read_frame(&mut stream)?;
    if worker_data.kind != WorkerFrameKind::WorkerData
        || environment_data.kind != WorkerFrameKind::EnvironmentData
    {
        return Err(NodeError::new(
            "ERR_WORKER_BOOTSTRAP",
            "worker bootstrap frames are out of order",
        ));
    }
    let worker_data = decode(&worker_data.payload)?.to_js();
    let environment_data = decode_environment_data_snapshot(&environment_data.payload)?;
    stream.set_read_timeout(None).map_err(io_error)?;
    let (transport, incoming) = WorkerTransport::start(stream)?;
    let parent_port = MessagePort::from_transport(transport, incoming);
    WORKER_CONTEXT.with(|context| {
        let mut context = context.borrow_mut();
        if context.is_some() {
            return Err(NodeError::new(
                "ERR_WORKER_BOOTSTRAP",
                "worker bootstrap context was initialized more than once",
            ));
        }
        *context = Some(WorkerProcessContext {
            thread_id,
            worker_data,
            parent_port,
            environment_data,
        });
        Ok(())
    })?;
    crate::process::install_worker_argv(&entry_identity, &arguments[7..]);
    Ok(Some(entry_identity))
}

pub fn receive_message_on_port(port: &MessagePort) -> Option<JsValue> {
    port.receive_message()
}

pub fn is_main_thread() -> bool {
    WORKER_CONTEXT.with(|context| context.borrow().is_none())
}

pub fn thread_id() -> i32 {
    WORKER_CONTEXT.with(|context| {
        context.borrow().as_ref().map(|context| context.thread_id).unwrap_or(0)
    })
}

pub fn parent_port() -> Option<MessagePort> {
    WORKER_CONTEXT.with(|context| {
        context.borrow().as_ref().map(|context| context.parent_port.clone())
    })
}

pub fn worker_data() -> JsValue {
    WORKER_CONTEXT.with(|context| {
        context
            .borrow()
            .as_ref()
            .map(|context| context.worker_data.clone())
            .unwrap_or(JsValue::Undefined)
    })
}

pub fn set_environment_data(key: &str, value: JsValue) -> NodeResult<()> {
    let value = ClonedValue::from_js(&value)?;
    if is_main_thread() {
        MAIN_ENVIRONMENT_DATA.with(|values| {
            values.borrow_mut().insert(key.to_string(), value);
        });
    } else {
        WORKER_CONTEXT.with(|context| {
            let mut context = context.borrow_mut();
            let context = context.as_mut().ok_or_else(|| {
                NodeError::new(
                    "ERR_WORKER_CONTEXT",
                    "worker environment data requires an initialized worker context",
                )
            })?;
            context.environment_data.insert(key.to_string(), value);
            Ok::<(), NodeError>(())
        })?;
    }
    Ok(())
}

pub fn get_environment_data(key: &str) -> Option<JsValue> {
    if is_main_thread() {
        return MAIN_ENVIRONMENT_DATA.with(|values| {
            values.borrow().get(key).map(ClonedValue::to_js)
        });
    }
    WORKER_CONTEXT.with(|context| {
        context
            .borrow()
            .as_ref()
            .and_then(|context| context.environment_data.get(key))
            .map(ClonedValue::to_js)
    })
}

pub fn mark_as_untransferable(value: &JsValue) -> NodeResult<()> {
    let identity = value.reference_identity_key().ok_or_else(|| {
        NodeError::new(
            "ERR_INVALID_ARG_TYPE",
            "markAsUntransferable requires a closed reference value",
        )
    })?;
    UNTRANSFERABLE_VALUES.with(|values| {
        values.borrow_mut().insert(identity);
    });
    Ok(())
}

pub fn is_marked_as_untransferable(value: &JsValue) -> NodeResult<bool> {
    let identity = value.reference_identity_key().ok_or_else(|| {
        NodeError::new(
            "ERR_INVALID_ARG_TYPE",
            "isMarkedAsUntransferable requires a closed reference value",
        )
    })?;
    Ok(UNTRANSFERABLE_VALUES.with(|values| values.borrow().contains(&identity)))
}

pub(crate) fn encode_environment_data_snapshot() -> NodeResult<Vec<u8>> {
    let values = MAIN_ENVIRONMENT_DATA.with(|values| values.borrow().clone());
    if values.len() > MAXIMUM_ENVIRONMENT_ENTRIES {
        return Err(NodeError::new(
            "ERR_WORKER_ENVIRONMENT_LIMIT",
            "worker environment-data entry count exceeds the finite limit",
        ));
    }
    let mut output = Vec::new();
    write_u32(&mut output, values.len())?;
    for (key, value) in values {
        write_bytes(&mut output, key.as_bytes())?;
        write_bytes(&mut output, &encode(&value)?)?;
    }
    if output.len() > protocol::MAXIMUM_FRAME_BYTES {
        return Err(NodeError::new(
            "ERR_WORKER_ENVIRONMENT_LIMIT",
            "worker environment-data snapshot exceeds the finite transport limit",
        ));
    }
    Ok(output)
}

fn decode_environment_data_snapshot(input: &[u8]) -> NodeResult<BTreeMap<String, ClonedValue>> {
    let mut reader = SnapshotReader::new(input);
    let count = reader.count()?;
    if count > MAXIMUM_ENVIRONMENT_ENTRIES {
        return Err(NodeError::new(
            "ERR_WORKER_ENVIRONMENT_LIMIT",
            "worker environment-data entry count exceeds the finite limit",
        ));
    }
    let mut values = BTreeMap::new();
    for _ in 0..count {
        let key = String::from_utf8(reader.bytes()?.to_vec()).map_err(|_| {
            NodeError::new("ERR_WORKER_ENVIRONMENT_DATA", "environment-data key is not UTF-8")
        })?;
        let value = decode(reader.bytes()?)?;
        if values.insert(key, value).is_some() {
            return Err(NodeError::new(
                "ERR_WORKER_ENVIRONMENT_DATA",
                "environment-data snapshot contains a duplicate key",
            ));
        }
    }
    if !reader.complete() {
        return Err(NodeError::new(
            "ERR_WORKER_ENVIRONMENT_DATA",
            "environment-data snapshot contains trailing bytes",
        ));
    }
    Ok(values)
}

fn decode_token(value: &str) -> NodeResult<Vec<u8>> {
    if value.len() != AUTHENTICATION_TOKEN_BYTES * 2 || !value.is_ascii() {
        return Err(NodeError::new(
            "ERR_WORKER_BOOTSTRAP",
            "worker authentication token has an invalid size",
        ));
    }
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(AUTHENTICATION_TOKEN_BYTES);
    for index in (0..bytes.len()).step_by(2) {
        output.push((hex_value(bytes[index])? << 4) | hex_value(bytes[index + 1])?);
    }
    Ok(output)
}

fn hex_value(value: u8) -> NodeResult<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(NodeError::new(
            "ERR_WORKER_BOOTSTRAP",
            "worker authentication token is invalid",
        )),
    }
}

fn write_u32(output: &mut Vec<u8>, value: usize) -> NodeResult<()> {
    let value = u32::try_from(value).map_err(|_| {
        NodeError::new("ERR_WORKER_ENVIRONMENT_LIMIT", "environment-data count is too large")
    })?;
    output.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

fn write_bytes(output: &mut Vec<u8>, value: &[u8]) -> NodeResult<()> {
    write_u32(output, value.len())?;
    output.extend_from_slice(value);
    Ok(())
}

struct SnapshotReader<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> SnapshotReader<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, position: 0 }
    }

    fn complete(&self) -> bool {
        self.position == self.input.len()
    }

    fn count(&mut self) -> NodeResult<usize> {
        let bytes = self.take(4)?;
        Ok(usize::try_from(u32::from_be_bytes(
            bytes.try_into().expect("exact byte count"),
        ))
        .expect("u32 must fit usize"))
    }

    fn bytes(&mut self) -> NodeResult<&'a [u8]> {
        let count = self.count()?;
        self.take(count)
    }

    fn take(&mut self, count: usize) -> NodeResult<&'a [u8]> {
        let end = self.position.checked_add(count).ok_or_else(|| {
            NodeError::new("ERR_WORKER_ENVIRONMENT_DATA", "snapshot position overflowed")
        })?;
        if end > self.input.len() {
            return Err(NodeError::new(
                "ERR_WORKER_ENVIRONMENT_DATA",
                "environment-data snapshot is truncated",
            ));
        }
        let value = &self.input[self.position..end];
        self.position = end;
        Ok(value)
    }
}

pub(crate) use port::{has_refed_runtime_ports, poll_runtime_ports};
pub(crate) use worker::{has_refed_runtime_workers, poll_runtime_workers};
