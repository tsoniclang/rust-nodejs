use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};

use crate::buffer::Buffer;
use crate::error::{NodeError, NodeResult};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SourceConnectOptions {
    pub host: Option<String>,
    pub servername: Option<String>,
    pub port: Option<f64>,
    pub alpn_protocols: Option<tsonic_rust_js::JsArray<String>>,
    pub reject_unauthorized: Option<bool>,
    pub ca: Option<tsonic_rust_js::JsArray<String>>,
    pub timeout: Option<f64>,
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
    pub fn connect(options: SourceConnectOptions) -> NodeResult<Self> {
        Self::connect_with_callback(options, None)
    }

    fn connect_with_callback(
        options: SourceConnectOptions,
        callback: Option<tsonic_rust_runtime::Callable<(), tsonic_rust_runtime::TsonicResult<()>>>,
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
        crate::background::spawn(
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
        Ok((value.len() > 0).then_some(value))
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

    pub fn bytes_read_number(&self) -> f64 {
        self.state.borrow().bytes_read as f64
    }

    pub fn bytes_written_number(&self) -> f64 {
        self.state.borrow().bytes_written as f64
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
        let port = source_port(options.port.unwrap_or(443.0))?;
        let reject_unauthorized = options.reject_unauthorized.unwrap_or(true);
        let mut config =
            client_config(reject_unauthorized, source_string_array(options.ca, "ca")?)?;
        config.alpn_protocols = options
            .alpn_protocols
            .map(|values| dense_source_strings(values, "ALPNProtocols"))
            .transpose()?
            .unwrap_or_default()
            .into_iter()
            .map(String::into_bytes)
            .collect();
        let timeout = options
            .timeout
            .map(source_timeout)
            .transpose()?
            .map(std::time::Duration::from_millis);
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

type RuntimeConnectionCallback =
    tsonic_rust_runtime::Callable<(TlsSocket,), tsonic_rust_runtime::TsonicResult<()>>;

struct TlsServerState {
    options: SourceServerOptions,
    config: Arc<rustls::ServerConfig>,
    listener: Option<TcpListener>,
    callback: RuntimeConnectionCallback,
    listening: bool,
    refed: bool,
    registered: bool,
}

#[derive(Clone)]
pub struct TlsServer {
    state: Rc<RefCell<TlsServerState>>,
}

impl TlsServer {
    pub fn create<E>(
        options: SourceServerOptions,
        callback: tsonic_rust_runtime::Callable<(TlsSocket,), Result<(), E>>,
    ) -> NodeResult<Self>
    where
        E: std::fmt::Display + 'static,
    {
        let config = Arc::new(server_config(&options)?);
        let callback = tsonic_rust_runtime::Callable::new(move |arguments| {
            callback
                .call(arguments)
                .map_err(crate::error::callback_runtime_error)
        });
        Ok(Self {
            state: Rc::new(RefCell::new(TlsServerState {
                options,
                config,
                listener: None,
                callback,
                listening: false,
                refed: true,
                registered: false,
            })),
        })
    }

    pub fn listen(&mut self, port: f64, host: &str) -> NodeResult<&mut Self> {
        let listener = TcpListener::bind((host, source_port(port)?)).map_err(map_io_error)?;
        listener.set_nonblocking(true).map_err(map_io_error)?;
        {
            let mut state = self.state.borrow_mut();
            state.listener = Some(listener);
            state.listening = true;
        }
        register_server(self);
        Ok(self)
    }

    pub fn listen_callable<E>(
        &mut self,
        port: f64,
        host: &str,
        callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.listen(port, host)?;
        crate::event_loop::enqueue_runtime_task(move || {
            callback
                .call(())
                .map_err(crate::error::callback_runtime_error)
        })?;
        Ok(self)
    }

    pub fn listen_default_host_callable<E>(
        &mut self,
        port: f64,
        callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.listen_callable(port, "0.0.0.0", callback)
    }

    pub fn close(&mut self) {
        let mut state = self.state.borrow_mut();
        state.listener = None;
        state.listening = false;
    }

    pub fn listening(&self) -> bool {
        self.state.borrow().listening
    }

    pub fn ref_chain(&mut self) -> &mut Self {
        self.state.borrow_mut().refed = true;
        self
    }

    pub fn unref_chain(&mut self) -> &mut Self {
        self.state.borrow_mut().refed = false;
        self
    }

    pub fn options(&self) -> SourceServerOptions {
        self.state.borrow().options.clone()
    }
}

struct RuntimeServer {
    state: Weak<RefCell<TlsServerState>>,
}

thread_local! {
    static RUNTIME_SERVERS: RefCell<std::collections::BTreeMap<u64, RuntimeServer>> =
        RefCell::new(std::collections::BTreeMap::new());
}

static NEXT_RUNTIME_SERVER_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub fn connect(options: SourceConnectOptions) -> NodeResult<TlsSocket> {
    TlsSocket::connect(options)
}

pub fn connect_callable<E>(
    options: SourceConnectOptions,
    callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
) -> NodeResult<TlsSocket>
where
    E: std::fmt::Display + 'static,
{
    let callback = tsonic_rust_runtime::Callable::new(move |()| {
        callback
            .call(())
            .map_err(crate::error::callback_runtime_error)
    });
    TlsSocket::connect_with_callback(options, Some(callback))
}

pub fn create_server<E>(
    options: SourceServerOptions,
    callback: tsonic_rust_runtime::Callable<(TlsSocket,), Result<(), E>>,
) -> NodeResult<TlsServer>
where
    E: std::fmt::Display + 'static,
{
    TlsServer::create(options, callback)
}

pub(crate) fn has_refed_runtime_servers() -> bool {
    RUNTIME_SERVERS.with(|servers| {
        servers.borrow().values().any(|server| {
            server.state.upgrade().is_some_and(|state| {
                let state = state.borrow();
                state.refed && state.listening
            })
        })
    })
}

pub(crate) fn poll_runtime_servers() -> tsonic_rust_runtime::TsonicResult<bool> {
    let states = RUNTIME_SERVERS.with(|servers| {
        let mut servers = servers.borrow_mut();
        servers.retain(|_, server| {
            server
                .state
                .upgrade()
                .is_some_and(|state| state.borrow().listening)
        });
        servers
            .values()
            .filter_map(|server| server.state.upgrade())
            .collect::<Vec<_>>()
    });
    let mut did_work = false;
    for state in states {
        loop {
            let accepted = {
                let state = state.borrow();
                let listener = state
                    .listener
                    .as_ref()
                    .expect("listening TLS server has a listener");
                match listener.accept() {
                    Ok((stream, _)) => {
                        Ok(Some((stream, state.config.clone(), state.callback.clone())))
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
                    Err(error) => Err(map_io_error(error)),
                }
            }
            .map_err(tsonic_rust_runtime::TsonicError::from)?;
            let Some((stream, config, callback)) = accepted else {
                break;
            };
            stream
                .set_nonblocking(false)
                .map_err(map_io_error)
                .map_err(tsonic_rust_runtime::TsonicError::from)?;
            let timeout = std::time::Duration::from_secs(5);
            stream
                .set_read_timeout(Some(timeout))
                .map_err(map_io_error)
                .map_err(tsonic_rust_runtime::TsonicError::from)?;
            stream
                .set_write_timeout(Some(timeout))
                .map_err(map_io_error)
                .map_err(tsonic_rust_runtime::TsonicError::from)?;
            crate::background::spawn(
                move || {
                    let connection =
                        rustls::ServerConnection::new(config).map_err(map_tls_error)?;
                    let mut stream = rustls::StreamOwned::new(connection, stream);
                    while stream.conn.is_handshaking() {
                        stream
                            .conn
                            .complete_io(&mut stream.sock)
                            .map_err(map_io_error)?;
                    }
                    Ok(stream)
                },
                move |result| {
                    let stream = result.map_err(tsonic_rust_runtime::TsonicError::from)?;
                    callback.call((TlsSocket::from_server(stream),))
                },
            )
            .map_err(tsonic_rust_runtime::TsonicError::from)?;
            did_work = true;
        }
    }
    Ok(did_work)
}

fn register_server(server: &TlsServer) {
    {
        let mut state = server.state.borrow_mut();
        if state.registered {
            return;
        }
        state.registered = true;
    }
    let id = NEXT_RUNTIME_SERVER_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    RUNTIME_SERVERS.with(|servers| {
        servers.borrow_mut().insert(
            id,
            RuntimeServer {
                state: Rc::downgrade(&server.state),
            },
        );
    });
}

fn client_config(reject_unauthorized: bool, ca: Vec<String>) -> NodeResult<rustls::ClientConfig> {
    if !reject_unauthorized {
        return Ok(rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoCertificateVerification::new()))
            .with_no_client_auth());
    }
    let mut roots =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    for pem in ca {
        for certificate in parse_certificates(&pem)? {
            roots.add(certificate).map_err(map_tls_error)?;
        }
    }
    Ok(rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth())
}

fn server_config(options: &SourceServerOptions) -> NodeResult<rustls::ServerConfig> {
    let certificate = options.cert.as_deref().ok_or_else(|| {
        NodeError::new("ERR_TLS_CERT_REQUIRED", "TLS server options require cert")
    })?;
    let key = options
        .key
        .as_deref()
        .ok_or_else(|| NodeError::new("ERR_TLS_KEY_REQUIRED", "TLS server options require key"))?;
    let certificates = parse_certificates(certificate)?;
    if certificates.is_empty() {
        return Err(NodeError::new(
            "ERR_TLS_CERT_REQUIRED",
            "TLS certificate chain is empty",
        ));
    }
    let private_key = parse_private_key(key)?;
    let builder = rustls::ServerConfig::builder();
    let mut config = if options.request_cert.unwrap_or(false) {
        let mut roots = rustls::RootCertStore::empty();
        for pem in source_string_array(options.ca.clone(), "ca")? {
            for certificate in parse_certificates(&pem)? {
                roots.add(certificate).map_err(map_tls_error)?;
            }
        }
        if roots.is_empty() {
            return Err(NodeError::new(
                "ERR_TLS_CLIENT_CA_REQUIRED",
                "requestCert requires at least one client certificate authority",
            ));
        }
        let verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(roots));
        let verifier = if options.reject_unauthorized.unwrap_or(true) {
            verifier.build()
        } else {
            verifier.allow_unauthenticated().build()
        }
        .map_err(|error| NodeError::new("ERR_TLS_CERT", error.to_string()))?;
        builder
            .with_client_cert_verifier(verifier)
            .with_single_cert(certificates, private_key)
            .map_err(map_tls_error)?
    } else {
        builder
            .with_no_client_auth()
            .with_single_cert(certificates, private_key)
            .map_err(map_tls_error)?
    };
    config.alpn_protocols = options
        .alpn_protocols
        .clone()
        .map(|values| dense_source_strings(values, "ALPNProtocols"))
        .transpose()?
        .unwrap_or_default()
        .into_iter()
        .map(String::into_bytes)
        .collect();
    Ok(config)
}

fn parse_certificates(pem: &str) -> NodeResult<Vec<CertificateDer<'static>>> {
    rustls_pemfile::certs(&mut pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_io_error)
}

fn parse_private_key(pem: &str) -> NodeResult<PrivateKeyDer<'static>> {
    rustls_pemfile::private_key(&mut pem.as_bytes())
        .map_err(map_io_error)?
        .ok_or_else(|| NodeError::new("ERR_TLS_KEY_REQUIRED", "TLS private key is empty"))
}

fn source_string_array(
    value: Option<tsonic_rust_js::JsArray<String>>,
    name: &str,
) -> NodeResult<Vec<String>> {
    value
        .map(|values| dense_source_strings(values, name))
        .transpose()
        .map(Option::unwrap_or_default)
}

fn dense_source_strings(
    value: tsonic_rust_js::JsArray<String>,
    name: &str,
) -> NodeResult<Vec<String>> {
    value
        .values()
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            value.ok_or_else(|| {
                NodeError::new(
                    "ERR_INVALID_ARG_VALUE",
                    format!("{name}[{index}] must be a present string"),
                )
            })
        })
        .collect()
}

fn source_port(value: f64) -> NodeResult<u16> {
    if !value.is_finite() || value.fract() != 0.0 || value < 0.0 || value > u16::MAX as f64 {
        return Err(NodeError::new(
            "ERR_SOCKET_BAD_PORT",
            "port must be an unsigned 16-bit integer",
        ));
    }
    Ok(value as u16)
}

fn source_timeout(value: f64) -> NodeResult<u64> {
    if !value.is_finite() || value.fract() != 0.0 || value < 0.0 || value > u64::MAX as f64 {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            "timeout must be a non-negative integer",
        ));
    }
    Ok(value as u64)
}

fn map_io_error(error: std::io::Error) -> NodeError {
    NodeError::new("ERR_TLS_IO", error.to_string())
}

fn map_tls_error(error: rustls::Error) -> NodeError {
    NodeError::new("ERR_TLS_HANDSHAKE", error.to_string())
}

fn connecting_error() -> NodeError {
    NodeError::new(
        "ERR_SOCKET_CONNECTING",
        "TLS socket operation requires the secure connection callback to complete",
    )
}

fn io_connecting_error() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::WouldBlock, connecting_error())
}

fn io_node_error(error: &NodeError) -> std::io::Error {
    std::io::Error::other(error.clone())
}

#[derive(Debug)]
struct NoCertificateVerification(Arc<rustls::crypto::CryptoProvider>);

impl NoCertificateVerification {
    fn new() -> Self {
        Self(Arc::new(rustls::crypto::ring::default_provider()))
    }
}

impl rustls::client::danger::ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signed: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            certificate,
            signed,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signed: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            certificate,
            signed,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}
