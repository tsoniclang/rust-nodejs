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
    Failure(NodeError),
    End,
}

#[derive(Clone)]
pub(crate) struct WorkerTransport {
    writer: Arc<Mutex<TcpStream>>,
    closed: Arc<AtomicBool>,
}

impl WorkerTransport {
    pub fn start(stream: TcpStream) -> NodeResult<(Self, Receiver<TransportEvent>)> {
        let (sender, receiver) = sync_channel(MAXIMUM_PENDING_FRAMES);
        let frames = sender.clone();
        let transport = Self::start_reader(
            stream,
            move |frame| frames.send(TransportEvent::Frame(frame)).is_ok(),
            move |error| {
                if let Some(error) = error {
                    let _ = sender.send(TransportEvent::Failure(error));
                }
                let _ = sender.send(TransportEvent::End);
            },
        )?;
        Ok((transport, receiver))
    }

    pub(super) fn start_observed(
        stream: TcpStream,
        frame: impl Fn(WorkerFrame) -> bool + Send + 'static,
        terminal: impl FnOnce(Option<NodeError>) + Send + 'static,
    ) -> NodeResult<Self> {
        Self::start_reader(stream, frame, terminal)
    }

    fn start_reader(
        stream: TcpStream,
        frame_event: impl Fn(WorkerFrame) -> bool + Send + 'static,
        terminal: impl FnOnce(Option<NodeError>) + Send + 'static,
    ) -> NodeResult<Self> {
        stream
            .set_write_timeout(Some(Duration::from_secs(30)))
            .map_err(io_error)?;
        let reader = stream.try_clone().map_err(io_error)?;
        let writer = Arc::new(Mutex::new(stream));
        let closed = Arc::new(AtomicBool::new(false));
        let reader_closed = Arc::clone(&closed);
        let wake = crate::readiness::waker()?;
        thread::Builder::new()
            .name("tsonic-node-worker-transport".to_string())
            .spawn(move || {
                let mut reader = reader;
                let error = loop {
                    match read_frame(&mut reader) {
                        Ok(Some(frame)) => {
                            if frame.kind == WorkerFrameKind::Close {
                                break None;
                            }
                            if !matches!(
                                frame.kind,
                                WorkerFrameKind::Message | WorkerFrameKind::Error
                            ) {
                                break Some(protocol_error("unexpected worker transport frame"));
                            }
                            let sent = frame_event(frame);
                            let _ = wake.wake();
                            if !sent {
                                break None;
                            }
                        }
                        Ok(None) => break None,
                        Err(error) => {
                            if error.code() != "ERR_WORKER_TRANSPORT"
                                || !reader_closed.load(Ordering::SeqCst)
                            {
                                break Some(error);
                            }
                            break None;
                        }
                    }
                };
                terminal(error);
                let _ = wake.wake();
            })
            .map_err(io_error)?;
        Ok(Self { writer, closed })
    }

    pub fn send(&self, kind: WorkerFrameKind, payload: &[u8]) -> NodeResult<()> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(NodeError::new(
                "ERR_CLOSED_MESSAGE_PORT",
                "worker transport is closed",
            ));
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
        return Err(protocol_error(
            "worker frame exceeds the finite transport limit",
        ));
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

pub(crate) fn read_frame(stream: &mut impl Read) -> NodeResult<Option<WorkerFrame>> {
    let mut header = [0_u8; 5];
    let received = loop {
        match stream.read(&mut header) {
            Ok(0) => return Ok(None),
            Ok(received) => break received,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(io_error(error)),
        }
    };
    stream
        .read_exact(&mut header[received..])
        .map_err(io_error)?;
    let kind = WorkerFrameKind::from_byte(header[0])?;
    let length = usize::try_from(u32::from_be_bytes(
        header[1..].try_into().expect("exact header length"),
    ))
    .expect("u32 must fit usize");
    if length > MAXIMUM_FRAME_BYTES {
        return Err(protocol_error(
            "worker frame length exceeds the finite transport limit",
        ));
    }
    let mut payload = vec![0_u8; length];
    stream.read_exact(&mut payload).map_err(io_error)?;
    Ok(Some(WorkerFrame { kind, payload }))
}

pub(crate) fn io_error(error: std::io::Error) -> NodeError {
    NodeError::new("ERR_WORKER_TRANSPORT", error.to_string())
}

fn protocol_error(message: &str) -> NodeError {
    NodeError::new("ERR_WORKER_PROTOCOL", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::net::TcpListener;

    struct CountedReader {
        source: Cursor<Vec<u8>>,
        chunk: usize,
        interrupted: usize,
        reads: usize,
    }

    impl Read for CountedReader {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            self.reads += 1;
            if self.interrupted != 0 {
                self.interrupted -= 1;
                return Err(std::io::Error::from(std::io::ErrorKind::Interrupted));
            }
            let count = buffer.len().min(self.chunk);
            self.source.read(&mut buffer[..count])
        }
    }

    #[test]
    fn complete_native_header_adds_no_read_over_the_equivalent_exact_read() {
        for (chunk, expected) in [(5, 2), (2, 4)] {
            let bytes = vec![WorkerFrameKind::Message as u8, 0, 0, 0, 1, 17];
            let mut baseline = CountedReader {
                source: Cursor::new(bytes.clone()),
                chunk,
                interrupted: 0,
                reads: 0,
            };
            let mut header = [0; 5];
            let mut payload = [0; 1];
            baseline.read_exact(&mut header).unwrap();
            baseline.read_exact(&mut payload).unwrap();
            let mut actual = CountedReader {
                source: Cursor::new(bytes),
                chunk,
                interrupted: 0,
                reads: 0,
            };
            let frame = read_frame(&mut actual)
                .unwrap()
                .expect("complete native frame");
            assert_eq!(frame.payload, payload);
            assert_eq!(actual.reads, expected);
            assert_eq!(
                actual.reads, baseline.reads,
                "no extra full-header native read"
            );
        }
    }

    #[test]
    fn interrupted_native_initial_read_retries_without_changing_eof_or_payload() {
        for chunk in [1, 5] {
            let mut source = CountedReader {
                source: Cursor::new(vec![WorkerFrameKind::Message as u8, 0, 0, 0, 1, 19]),
                chunk,
                interrupted: 2,
                reads: 0,
            };
            let frame = read_frame(&mut source)
                .unwrap()
                .expect("interrupted native frame completes");
            assert_eq!(frame.payload, [19]);
            assert!(read_frame(&mut source).unwrap().is_none());
        }
    }

    fn socket_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let sender = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (receiver, _) = listener.accept().unwrap();
        receiver
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        (sender, receiver)
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn native_worker_sockets_are_close_on_exec_without_a_drain_timeout() {
        use std::os::fd::AsRawFd;
        let (sender, receiver) = socket_pair();
        for socket in [&sender, &receiver] {
            let flags =
                std::fs::read_to_string(format!("/proc/self/fdinfo/{}", socket.as_raw_fd()))
                    .unwrap();
            let flags = flags
                .lines()
                .find_map(|line| line.strip_prefix("flags:\t"))
                .unwrap();
            let flags = u32::from_str_radix(flags, 8).unwrap();
            assert_ne!(flags & 0x80000, 0, "native worker socket is close-on-exec");
        }
    }

    #[test]
    fn native_eof_after_complete_frames_is_not_a_protocol_error() {
        let (mut sender, receiver) = socket_pair();
        let (_transport, incoming) = WorkerTransport::start(receiver).unwrap();
        for payload in [b"first".as_slice(), b"last".as_slice()] {
            write_frame(&mut sender, WorkerFrameKind::Message, payload).unwrap();
        }
        sender.shutdown(Shutdown::Write).unwrap();
        for expected in [b"first".as_slice(), b"last".as_slice()] {
            let TransportEvent::Frame(frame) =
                incoming.recv_timeout(Duration::from_secs(5)).unwrap()
            else {
                panic!("complete native frame precedes EOF");
            };
            assert_eq!(frame.kind, WorkerFrameKind::Message);
            assert_eq!(frame.payload, expected);
        }
        assert!(
            matches!(
                incoming.recv_timeout(Duration::from_secs(5)).unwrap(),
                TransportEvent::End
            ),
            "native EOF is absent frame"
        );
    }

    #[test]
    fn native_eof_inside_a_header_or_payload_remains_an_io_failure() {
        for bytes in [
            vec![WorkerFrameKind::Message as u8],
            vec![WorkerFrameKind::Message as u8, 0, 0, 0],
            vec![WorkerFrameKind::Message as u8, 0, 0, 0, 1],
            vec![WorkerFrameKind::Message as u8, 0, 0, 0, 2, 7],
        ] {
            let (mut sender, receiver) = socket_pair();
            let (_transport, incoming) = WorkerTransport::start(receiver).unwrap();
            sender.write_all(&bytes).unwrap();
            sender.shutdown(Shutdown::Write).unwrap();
            assert!(
                matches!(
                    incoming.recv_timeout(Duration::from_secs(5)).unwrap(),
                    TransportEvent::Failure(_)
                ),
                "truncated native frame remains a failure"
            );
        }
    }

    #[test]
    fn malformed_full_headers_retain_the_protocol_reason_before_one_terminal_end() {
        for (bytes, expected) in [
            ([99, 0, 0, 0, 0], "worker frame kind is invalid"),
            (
                [WorkerFrameKind::Message as u8, 255, 255, 255, 255],
                "worker frame length exceeds the finite transport limit",
            ),
        ] {
            let (mut sender, receiver) = socket_pair();
            let (_transport, incoming) = WorkerTransport::start(receiver).unwrap();
            sender.write_all(&bytes).unwrap();
            sender.shutdown(Shutdown::Write).unwrap();
            let TransportEvent::Failure(error) =
                incoming.recv_timeout(Duration::from_secs(5)).unwrap()
            else {
                panic!("malformed native full header has one protocol failure");
            };
            assert_eq!(error.code(), "ERR_WORKER_PROTOCOL");
            assert_eq!(error.message(), expected);
            assert!(matches!(
                incoming.recv_timeout(Duration::from_secs(5)).unwrap(),
                TransportEvent::End
            ));
        }
    }

    #[test]
    fn native_transport_queues_complete_frames_then_one_terminal_end_without_error() {
        let (mut sender, receiver) = socket_pair();
        let (_transport, incoming) = WorkerTransport::start(receiver).unwrap();
        for value in 0_u8..64 {
            write_frame(&mut sender, WorkerFrameKind::Message, &[value]).unwrap();
        }
        sender.shutdown(Shutdown::Write).unwrap();
        for value in 0_u8..64 {
            let event = incoming.recv_timeout(Duration::from_secs(5)).unwrap();
            let TransportEvent::Frame(frame) = event else {
                panic!("all native data precedes transport end");
            };
            assert_eq!(frame.payload, [value]);
        }
        assert!(matches!(
            incoming.recv_timeout(Duration::from_secs(5)).unwrap(),
            TransportEvent::End
        ));
        assert!(
            incoming.recv_timeout(Duration::from_secs(5)).is_err(),
            "one terminal event"
        );
    }
}
