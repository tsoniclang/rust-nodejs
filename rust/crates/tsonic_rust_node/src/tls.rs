use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::rc::Rc;
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};

use crate::buffer::Buffer;
use crate::error::{NodeError, NodeResult};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SourceConnectOptions {
    pub host: Option<String>,
    pub servername: Option<String>,
    pub port: Option<u16>,
    pub alpn_protocols: Option<tsonic_rust_js::JsArray<String>>,
    pub reject_unauthorized: Option<bool>,
    pub ca: Option<tsonic_rust_js::JsArray<String>>,
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SourceServerOptions {
    pub key: Option<String>,
    pub cert: Option<String>,
    pub ca: Option<tsonic_rust_js::JsArray<String>>,
    pub alpn_protocols: Option<tsonic_rust_js::JsArray<String>>,
    pub request_cert: Option<bool>,
    pub reject_unauthorized: Option<bool>,
}

enum TlsStream {
    Client(rustls::StreamOwned<rustls::ClientConnection, TcpStream>),
    Server(rustls::StreamOwned<rustls::ServerConnection, TcpStream>),
}

#[expect(
    clippy::large_enum_variant,
    reason = "keeps an established TLS stream allocation-free after connection"
)]
enum TlsSocketPhase {
    Connecting,
    Ready(TlsStream),
    Failed(NodeError),
}

struct TlsSocketState {
    phase: TlsSocketPhase,
    servername: String,
    authorized: bool,
    authorization_error: Option<String>,
    bytes_read: u64,
    bytes_written: u64,
    refed: bool,
}

#[derive(Clone)]
pub struct TlsSocket {
    state: Rc<RefCell<TlsSocketState>>,
}

impl TlsSocket {
    pub fn connect<E: From<NodeError> + 'static>(
        tasks: &crate::background::BackgroundTasks<E>,
        options: SourceConnectOptions,
    ) -> NodeResult<Self> {
        Self::connect_with_callback(tasks, options, None)
    }

    fn connect_with_callback<E: From<NodeError> + 'static>(
        tasks: &crate::background::BackgroundTasks<E>,
        options: SourceConnectOptions,
        callback: Option<tsonic_rust_runtime::Callable<(), Result<(), E>>>,
    ) -> NodeResult<Self> {
        let prepared = PreparedClientConnection::new(options)?;
        let state = Rc::new(RefCell::new(TlsSocketState {
            phase: TlsSocketPhase::Connecting,
            servername: prepared.servername.clone(),
            authorized: false,
            authorization_error: None,
            bytes_read: 0,
            bytes_written: 0,
            refed: true,
        }));
        let completion_state = Rc::clone(&state);
        tasks.spawn(
            move || prepared.connect(),
            move |result| {
                match result {
                    Ok(connected) => {
                        {
                            let mut state = completion_state.borrow_mut();
                            state.phase = TlsSocketPhase::Ready(connected.stream);
                            state.authorized = connected.authorized;
                            state.authorization_error = connected.authorization_error;
                        }
                        if let Some(callback) = callback {
                            callback.call(())?;
                        }
                    }
                    Err(error) => {
                        completion_state.borrow_mut().phase = TlsSocketPhase::Failed(error);
                    }
                }
                Ok(())
            },
        )?;
        Ok(Self { state })
    }

    fn from_server(stream: rustls::StreamOwned<rustls::ServerConnection, TcpStream>) -> Self {
        let authorized = stream
            .conn
            .peer_certificates()
            .is_some_and(|certificates| !certificates.is_empty());
        let servername = stream.conn.server_name().unwrap_or_default().to_string();
        Self {
            state: Rc::new(RefCell::new(TlsSocketState {
                phase: TlsSocketPhase::Ready(TlsStream::Server(stream)),
                servername,
                authorized,
                authorization_error: (!authorized)
                    .then(|| "peer did not provide a certificate".to_string()),
                bytes_read: 0,
                bytes_written: 0,
                refed: true,
            })),
        }
    }

    pub fn write_buffer(&mut self, value: &Buffer) -> NodeResult<bool> {
        value.with_bytes(|bytes| self.write_bytes(bytes))?;
        Ok(true)
    }

    pub fn write_string(&mut self, value: &str) -> NodeResult<bool> {
        self.write_bytes(value.as_bytes())?;
        Ok(true)
    }

    pub fn read_buffer(&mut self) -> NodeResult<Buffer> {
        let mut bytes = vec![0; 16 * 1024];
        let mut state = self.state.borrow_mut();
        let read = match &mut state.phase {
            TlsSocketPhase::Ready(TlsStream::Client(stream)) => stream.read(&mut bytes),
            TlsSocketPhase::Ready(TlsStream::Server(stream)) => stream.read(&mut bytes),
            TlsSocketPhase::Connecting => return Err(connecting_error()),
            TlsSocketPhase::Failed(error) => return Err(error.clone()),
        }
        .map_err(map_io_error)?;
        bytes.truncate(read);
        state.bytes_read = state.bytes_read.saturating_add(read as u64);
        Ok(Buffer::from_bytes(bytes))
    }

    pub fn read_optional_buffer(&mut self) -> NodeResult<Option<Buffer>> {
        let value = self.read_buffer()?;
        Ok((!value.is_empty()).then_some(value))
    }

    pub fn end(&mut self) -> NodeResult<()> {
        let mut state = self.state.borrow_mut();
        match &mut state.phase {
            TlsSocketPhase::Ready(TlsStream::Client(stream)) => {
                stream.conn.send_close_notify();
                stream.flush().map_err(map_io_error)?;
                stream.sock.shutdown(Shutdown::Write).map_err(map_io_error)
            }
            TlsSocketPhase::Ready(TlsStream::Server(stream)) => {
                stream.conn.send_close_notify();
                stream.flush().map_err(map_io_error)?;
                stream.sock.shutdown(Shutdown::Write).map_err(map_io_error)
            }
            TlsSocketPhase::Connecting => Err(connecting_error()),
            TlsSocketPhase::Failed(error) => Err(error.clone()),
        }
    }

    pub fn authorized(&self) -> bool {
        self.state.borrow().authorized
    }

    pub fn authorization_error(&self) -> Option<String> {
        self.state.borrow().authorization_error.clone()
    }

    pub fn encrypted(&self) -> bool {
        true
    }

    pub fn servername(&self) -> String {
        self.state.borrow().servername.clone()
    }

    pub fn servername_string(&self) -> String {
        self.servername()
    }

    pub fn alpn_protocol(&self) -> Option<String> {
        let state = self.state.borrow();
        match &state.phase {
            TlsSocketPhase::Ready(TlsStream::Client(stream)) => stream.conn.alpn_protocol(),
            TlsSocketPhase::Ready(TlsStream::Server(stream)) => stream.conn.alpn_protocol(),
            TlsSocketPhase::Connecting | TlsSocketPhase::Failed(_) => None,
        }
        .map(|value| String::from_utf8_lossy(value).into_owned())
    }

    pub fn protocol(&self) -> Option<String> {
        let state = self.state.borrow();
        let version = match &state.phase {
            TlsSocketPhase::Ready(TlsStream::Client(stream)) => stream.conn.protocol_version(),
            TlsSocketPhase::Ready(TlsStream::Server(stream)) => stream.conn.protocol_version(),
            TlsSocketPhase::Connecting | TlsSocketPhase::Failed(_) => None,
        }?;
        Some(match version {
            rustls::ProtocolVersion::TLSv1_2 => "TLSv1.2".to_string(),
            rustls::ProtocolVersion::TLSv1_3 => "TLSv1.3".to_string(),
            other => format!("{other:?}"),
        })
    }

    pub fn bytes_read(&self) -> u64 {
        self.state.borrow().bytes_read
    }

    pub fn bytes_written(&self) -> u64 {
        self.state.borrow().bytes_written
    }

    pub fn ref_chain(&mut self) -> &mut Self {
        self.state.borrow_mut().refed = true;
        self
    }

    pub fn unref_chain(&mut self) -> &mut Self {
        self.state.borrow_mut().refed = false;
        self
    }

    pub fn has_ref(&self) -> bool {
        self.state.borrow().refed
    }

    fn write_bytes(&mut self, value: &[u8]) -> NodeResult<()> {
        let mut state = self.state.borrow_mut();
        match &mut state.phase {
            TlsSocketPhase::Ready(TlsStream::Client(stream)) => stream.write_all(value),
            TlsSocketPhase::Ready(TlsStream::Server(stream)) => stream.write_all(value),
            TlsSocketPhase::Connecting => return Err(connecting_error()),
            TlsSocketPhase::Failed(error) => return Err(error.clone()),
        }
        .map_err(map_io_error)?;
        state.bytes_written = state.bytes_written.saturating_add(value.len() as u64);
        Ok(())
    }

    fn peer_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        let state = self.state.borrow();
        match &state.phase {
            TlsSocketPhase::Ready(TlsStream::Client(stream)) => stream.sock.peer_addr(),
            TlsSocketPhase::Ready(TlsStream::Server(stream)) => stream.sock.peer_addr(),
            TlsSocketPhase::Connecting => Err(io_connecting_error()),
            TlsSocketPhase::Failed(error) => Err(io_node_error(error)),
        }
    }
}

struct PreparedClientConnection {
    host: String,
    servername: String,
    port: u16,
    timeout: Option<std::time::Duration>,
    reject_unauthorized: bool,
    config: rustls::ClientConfig,
}

struct ConnectedClient {
    stream: TlsStream,
    authorized: bool,
    authorization_error: Option<String>,
}

impl PreparedClientConnection {
    fn new(options: SourceConnectOptions) -> NodeResult<Self> {
        let host = options.host.unwrap_or_else(|| "localhost".to_string());
        let servername = options.servername.unwrap_or_else(|| host.clone());
        let port = options.port.unwrap_or(443);
        let reject_unauthorized = options.reject_unauthorized.unwrap_or(true);
        let mut config = client_config(reject_unauthorized, source_string_array(options.ca))?;
        config.alpn_protocols = options
            .alpn_protocols
            .map(|values| values.values())
            .unwrap_or_default()
            .into_iter()
            .map(String::into_bytes)
            .collect();
        let timeout = options.timeout.map(std::time::Duration::from_millis);
        ServerName::try_from(servername.clone())
            .map_err(|error| NodeError::new("ERR_TLS_CERT_ALTNAME_INVALID", error.to_string()))?;
        Ok(Self {
            host,
            servername,
            port,
            timeout,
            reject_unauthorized,
            config,
        })
    }

    fn connect(self) -> NodeResult<ConnectedClient> {
        let server_name = ServerName::try_from(self.servername.clone())
            .map_err(|error| NodeError::new("ERR_TLS_CERT_ALTNAME_INVALID", error.to_string()))?;
        let connection = rustls::ClientConnection::new(Arc::new(self.config), server_name)
            .map_err(map_tls_error)?;
        let stream = TcpStream::connect((self.host.as_str(), self.port)).map_err(map_io_error)?;
        if let Some(duration) = self.timeout {
            stream
                .set_read_timeout(Some(duration))
                .map_err(map_io_error)?;
            stream
                .set_write_timeout(Some(duration))
                .map_err(map_io_error)?;
        }
        let mut stream = rustls::StreamOwned::new(connection, stream);
        while stream.conn.is_handshaking() {
            stream
                .conn
                .complete_io(&mut stream.sock)
                .map_err(map_io_error)?;
        }
        Ok(ConnectedClient {
            stream: TlsStream::Client(stream),
            authorized: self.reject_unauthorized,
            authorization_error: (!self.reject_unauthorized)
                .then(|| "certificate verification was explicitly disabled".to_string()),
        })
    }
}

impl Read for TlsSocket {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let mut state = self.state.borrow_mut();
        let read = match &mut state.phase {
            TlsSocketPhase::Ready(TlsStream::Client(stream)) => stream.read(output),
            TlsSocketPhase::Ready(TlsStream::Server(stream)) => stream.read(output),
            TlsSocketPhase::Connecting => return Err(io_connecting_error()),
            TlsSocketPhase::Failed(error) => return Err(io_node_error(error)),
        }?;
        state.bytes_read = state.bytes_read.saturating_add(read as u64);
        Ok(read)
    }
}

impl Write for TlsSocket {
    fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
        let mut state = self.state.borrow_mut();
        let written = match &mut state.phase {
            TlsSocketPhase::Ready(TlsStream::Client(stream)) => stream.write(input),
            TlsSocketPhase::Ready(TlsStream::Server(stream)) => stream.write(input),
            TlsSocketPhase::Connecting => return Err(io_connecting_error()),
            TlsSocketPhase::Failed(error) => return Err(io_node_error(error)),
        }?;
        state.bytes_written = state.bytes_written.saturating_add(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.state.borrow_mut().phase {
            TlsSocketPhase::Ready(TlsStream::Client(stream)) => stream.flush(),
            TlsSocketPhase::Ready(TlsStream::Server(stream)) => stream.flush(),
            TlsSocketPhase::Connecting => Err(io_connecting_error()),
            TlsSocketPhase::Failed(error) => Err(io_node_error(error)),
        }
    }
}

impl crate::http::RuntimeTransport for TlsSocket {
    fn peer_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.peer_addr()
    }
}

mod resources;
mod server;
pub use resources::{with_default_tls, TlsServers};
pub use server::{create_server, TlsServer};

pub fn connect<E: From<NodeError> + 'static>(
    tasks: &crate::background::BackgroundTasks<E>,
    options: SourceConnectOptions,
) -> NodeResult<TlsSocket> {
    TlsSocket::connect(tasks, options)
}

pub fn connect_callable<E: From<NodeError> + 'static>(
    tasks: &crate::background::BackgroundTasks<E>,
    options: SourceConnectOptions,
    callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
) -> NodeResult<TlsSocket> {
    TlsSocket::connect_with_callback(tasks, options, Some(callback))
}

include!("tls/configuration.rs");
