use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::error::{NodeError, NodeResult};

pub(crate) const MAXIMUM_FRAME_BYTES: usize = 64 * 1024 * 1024;
const MAXIMUM_PENDING_FRAMES: usize = 1 << 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum WorkerFrameKind {
    Authenticate = 1,
    WorkerData = 2,
    EnvironmentData = 3,
    Message = 4,
    Error = 5,
    Close = 6,
}

impl WorkerFrameKind {
    fn from_byte(value: u8) -> NodeResult<Self> {
        match value {
            1 => Ok(Self::Authenticate),
            2 => Ok(Self::WorkerData),
            3 => Ok(Self::EnvironmentData),
            4 => Ok(Self::Message),
            5 => Ok(Self::Error),
            6 => Ok(Self::Close),
            _ => Err(protocol_error("worker frame kind is invalid")),
        }
    }
}

pub(crate) struct WorkerFrame {
    pub kind: WorkerFrameKind,
    pub payload: Vec<u8>,
}

pub(crate) enum TransportEvent {
    Frame(WorkerFrame),
    Failure(String),
    End,
}

#[derive(Clone)]
pub(crate) struct WorkerTransport {
    writer: Arc<Mutex<TcpStream>>,
    closed: Arc<AtomicBool>,
}

impl WorkerTransport {
    pub fn start(stream: TcpStream) -> NodeResult<(Self, Receiver<TransportEvent>)> {
        stream
            .set_write_timeout(Some(Duration::from_secs(30)))
            .map_err(io_error)?;
        let reader = stream.try_clone().map_err(io_error)?;
        let writer = Arc::new(Mutex::new(stream));
        let closed = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = sync_channel(MAXIMUM_PENDING_FRAMES);
        let reader_closed = Arc::clone(&closed);
        thread::Builder::new()
            .name("tsonic-node-worker-transport".to_string())
            .spawn(move || {
                let mut reader = reader;
                loop {
                    match read_frame(&mut reader) {
                        Ok(frame) => {
                            let close = frame.kind == WorkerFrameKind::Close;
                            if sender.send(TransportEvent::Frame(frame)).is_err() || close {
                                break;
                            }
                        }
                        Err(error) => {
                            if !reader_closed.load(Ordering::SeqCst) {
                                let _ = sender.send(TransportEvent::Failure(error.to_string()));
                            }
                            break;
                        }
                    }
                }
                let _ = sender.send(TransportEvent::End);
            })
            .map_err(io_error)?;
        Ok((Self { writer, closed }, receiver))
    }

    pub fn send(&self, kind: WorkerFrameKind, payload: &[u8]) -> NodeResult<()> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(NodeError::new("ERR_CLOSED_MESSAGE_PORT", "worker transport is closed"));
        }
        let mut writer = crate::sync::lock(&self.writer);
        write_frame(&mut writer, kind, payload)
    }

    pub fn close(&self) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        let writer = crate::sync::lock(&self.writer);
        let _ = writer.shutdown(Shutdown::Both);
    }
}

impl Drop for WorkerTransport {
    fn drop(&mut self) {
        if Arc::strong_count(&self.writer) == 1 {
            self.close();
        }
    }
}

pub(crate) fn write_frame(
    stream: &mut TcpStream,
    kind: WorkerFrameKind,
    payload: &[u8],
) -> NodeResult<()> {
    if payload.len() > MAXIMUM_FRAME_BYTES {
        return Err(protocol_error("worker frame exceeds the finite transport limit"));
    }
    let length = u32::try_from(payload.len())
        .map_err(|_| protocol_error("worker frame length exceeds the finite transport limit"))?;
    let mut header = [0_u8; 5];
    header[0] = kind as u8;
    header[1..].copy_from_slice(&length.to_be_bytes());
    stream.write_all(&header).map_err(io_error)?;
    stream.write_all(payload).map_err(io_error)?;
    stream.flush().map_err(io_error)
}

pub(crate) fn read_frame(stream: &mut TcpStream) -> NodeResult<WorkerFrame> {
    let mut header = [0_u8; 5];
    stream.read_exact(&mut header).map_err(io_error)?;
    let kind = WorkerFrameKind::from_byte(header[0])?;
    let length = usize::try_from(u32::from_be_bytes(
        header[1..].try_into().expect("exact header length"),
    ))
    .expect("u32 must fit usize");
    if length > MAXIMUM_FRAME_BYTES {
        return Err(protocol_error("worker frame length exceeds the finite transport limit"));
    }
    let mut payload = vec![0_u8; length];
    stream.read_exact(&mut payload).map_err(io_error)?;
    Ok(WorkerFrame { kind, payload })
}

pub(crate) fn io_error(error: std::io::Error) -> NodeError {
    NodeError::new("ERR_WORKER_TRANSPORT", error.to_string())
}

fn protocol_error(message: &str) -> NodeError {
    NodeError::new("ERR_WORKER_PROTOCOL", message)
}
