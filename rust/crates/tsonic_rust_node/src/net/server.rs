type RuntimeConnectionCallback = tsonic_rust_runtime::Callable<
    (Socket,),
    tsonic_rust_runtime::TsonicResult<()>,
>;

struct ServerState {
    listener: Option<TcpListener>,
    connection_callback: Option<RuntimeConnectionCallback>,
    refed: bool,
    max_connections: Option<usize>,
    connections: usize,
    listening: bool,
    registered: bool,
}

#[derive(Clone)]
pub struct Server {
    state: std::rc::Rc<std::cell::RefCell<ServerState>>,
}

impl Server {
    pub fn new() -> Self {
        Self {
            state: std::rc::Rc::new(std::cell::RefCell::new(ServerState {
                listener: None,
                connection_callback: None,
                refed: true,
                max_connections: None,
                connections: 0,
                listening: false,
                registered: false,
            })),
        }
    }

    pub fn listen(host: &str, port: u16) -> NodeResult<Self> {
        let mut server = Self::new();
        server.bind(host, port)?;
        Ok(server)
    }

    pub fn listen_with_options(options: &ListenOptions) -> NodeResult<Self> {
        Self::listen(&options.host, options.port)
    }

    pub fn bind(&mut self, host: &str, port: u16) -> NodeResult<&mut Self> {
        let listener = TcpListener::bind((host, port)).map_err(map_net_error)?;
        listener.set_nonblocking(true).map_err(map_net_error)?;
        let mut state = self.state.borrow_mut();
        state.listener = Some(listener);
        state.listening = true;
        drop(state);
        register_runtime_server(self);
        Ok(self)
    }

    pub fn listen_source<E>(
        &mut self,
        port: f64,
        host: &str,
        callback: Option<tsonic_rust_runtime::Callable<(), Result<(), E>>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.bind(host, source_port(port)?)?;
        if let Some(callback) = callback {
            crate::event_loop::enqueue_runtime_task(move || {
                callback.call(()).map_err(crate::error::callback_runtime_error)
            })?;
        }
        Ok(self)
    }

    pub fn listen_port(&mut self, port: f64) -> NodeResult<&mut Self> {
        self.bind("0.0.0.0", source_port(port)?)
    }

    pub fn listen_port_host(&mut self, port: f64, host: &str) -> NodeResult<&mut Self> {
        self.bind(host, source_port(port)?)
    }

    pub fn listen_port_callable<E>(
        &mut self,
        port: f64,
        callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.listen_source(port, "0.0.0.0", Some(callback))
    }

    pub fn listen_port_host_callable<E>(
        &mut self,
        port: f64,
        host: &str,
        callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.listen_source(port, host, Some(callback))
    }

    pub fn set_connection_callback<E>(
        &mut self,
        callback: tsonic_rust_runtime::Callable<(Socket,), Result<(), E>>,
    ) where
        E: std::fmt::Display + 'static,
    {
        self.state.borrow_mut().connection_callback = Some(
            tsonic_rust_runtime::Callable::new(move |arguments| {
                callback
                    .call(arguments)
                    .map_err(crate::error::callback_runtime_error)
            }),
        );
    }

    pub fn address(&self) -> NodeResult<AddressInfo> {
        self.state.borrow().listener.as_ref()
            .ok_or_else(|| NodeError::new("ERR_SERVER_NOT_RUNNING", "server is not listening"))?
            .local_addr()
            .map(address_info)
            .map_err(map_net_error)
    }

    pub fn local_addr(&self) -> NodeResult<String> {
        self.state.borrow().listener.as_ref()
            .ok_or_else(|| NodeError::new("ERR_SERVER_NOT_RUNNING", "server is not listening"))?
            .local_addr()
            .map(|addr| addr.to_string())
            .map_err(map_net_error)
    }

    pub fn local_port(&self) -> NodeResult<u16> {
        self.state.borrow().listener.as_ref()
            .ok_or_else(|| NodeError::new("ERR_SERVER_NOT_RUNNING", "server is not listening"))?
            .local_addr()
            .map(|addr| addr.port())
            .map_err(map_net_error)
    }

    pub fn accept(&mut self) -> NodeResult<Socket> {
        let mut state = self.state.borrow_mut();
        let (stream, _) = state.listener.as_ref()
            .ok_or_else(|| NodeError::new("ERR_SERVER_NOT_RUNNING", "server is not listening"))?
            .accept()
            .map_err(map_net_error)?;
        state.connections += 1;
        Ok(Socket::from_stream(stream))
    }

    pub fn close(&mut self) {
        let mut state = self.state.borrow_mut();
        state.listener = None;
        state.listening = false;
    }

    pub fn listening(&self) -> bool {
        self.state.borrow().listening
    }

    pub fn max_connections(&self) -> Option<usize> {
        self.state.borrow().max_connections
    }

    pub fn set_max_connections(&mut self, value: Option<usize>) {
        self.state.borrow_mut().max_connections = value;
    }

    pub fn connections(&self) -> usize {
        self.state.borrow().connections
    }

    pub fn get_connections(&self) -> usize {
        self.state.borrow().connections
    }

    pub fn r#ref(&mut self) {
        self.state.borrow_mut().refed = true;
    }

    pub fn ref_chain(&mut self) -> &mut Self {
        self.r#ref();
        self
    }

    pub fn unref(&mut self) {
        self.state.borrow_mut().refed = false;
    }

    pub fn unref_chain(&mut self) -> &mut Self {
        self.unref();
        self
    }

    pub fn has_ref(&self) -> bool {
        self.state.borrow().refed
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

struct RuntimeServer {
    state: std::rc::Weak<std::cell::RefCell<ServerState>>,
}

thread_local! {
    static RUNTIME_SERVERS: std::cell::RefCell<std::collections::BTreeMap<u64, RuntimeServer>> =
        std::cell::RefCell::new(std::collections::BTreeMap::new());
}

static NEXT_RUNTIME_SERVER_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

fn register_runtime_server(server: &Server) {
    {
        let mut state = server.state.borrow_mut();
        if state.registered {
            return;
        }
        state.registered = true;
    }
    let id = NEXT_RUNTIME_SERVER_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    RUNTIME_SERVERS.with(|servers| {
        servers.borrow_mut().insert(id, RuntimeServer {
            state: std::rc::Rc::downgrade(&server.state),
        });
    });
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
        servers.retain(|_, server| server.state.upgrade().is_some_and(|state| state.borrow().listening));
        servers.values().filter_map(|server| server.state.upgrade()).collect::<Vec<_>>()
    });
    let mut did_work = false;
    for state in states {
        loop {
            let accepted = {
                let mut state = state.borrow_mut();
                if state.max_connections.is_some_and(|maximum| state.connections >= maximum) {
                    Ok(None)
                } else {
                    match state.listener.as_ref().expect("listening server has a listener").accept() {
                        Ok((stream, _)) => {
                            state.connections += 1;
                            Ok(Some((Socket::from_stream(stream), state.connection_callback.clone())))
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
                        Err(error) => Err(map_net_error(error)),
                    }
                }
            }.map_err(tsonic_rust_runtime::TsonicError::from)?;
            let Some((socket, callback)) = accepted else {
                break;
            };
            did_work = true;
            if let Some(callback) = callback {
                callback.call((socket,))?;
            }
        }
    }
    Ok(did_work)
}

fn source_port(value: f64) -> NodeResult<u16> {
    if !value.is_finite() || value.fract() != 0.0 || value < 0.0 || value > u16::MAX as f64 {
        return Err(NodeError::new("ERR_SOCKET_BAD_PORT", "port must be an unsigned 16-bit integer"));
    }
    Ok(value as u16)
}

pub fn is_ip(value: &str) -> u8 {
    value
        .parse::<std::net::IpAddr>()
        .map(|addr| if addr.is_ipv4() { 4 } else { 6 })
        .unwrap_or(0)
}

pub fn is_ipv4(value: &str) -> bool {
    is_ip(value) == 4
}

pub fn is_ipv6(value: &str) -> bool {
    is_ip(value) == 6
}

pub fn is_ip_number(value: &str) -> f64 {
    is_ip(value) as f64
}

pub fn connect(host: &str, port: u16) -> NodeResult<Socket> {
    Socket::connect(host, port)
}

pub fn connect_with_options(options: &ConnectOptions) -> NodeResult<Socket> {
    if options
        .block_list
        .as_ref()
        .is_some_and(|block_list| block_list.check(&options.host).unwrap_or(false))
    {
        return Err(NodeError::new("ERR_BLOCKED_ADDRESS", "address blocked"));
    }
    let mut socket = Socket::connect(&options.host, options.port)?;
    if options.no_delay {
        socket.set_no_delay(true)?;
    }
    if options.keep_alive {
        socket.set_keep_alive(true, options.keep_alive_initial_delay)?;
    }
    if let Some(timeout) = options.timeout {
        socket.set_timeout(timeout)?;
    }
    Ok(socket)
}

pub fn create_connection(host: &str, port: u16) -> NodeResult<Socket> {
    connect(host, port)
}

pub fn create_connection_with_options(options: &ConnectOptions) -> NodeResult<Socket> {
    connect_with_options(options)
}

pub fn create_connection_source(port: f64, host: &str) -> NodeResult<Socket> {
    create_connection(host, source_port(port)?)
}

pub fn create_connection_default_host(port: f64) -> NodeResult<Socket> {
    create_connection_source(port, "localhost")
}

pub fn create_connection_default_host_callable<E>(
    port: f64,
    callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
) -> NodeResult<Socket>
where
    E: std::fmt::Display + 'static,
{
    create_connection_callable(port, "localhost", callback)
}

pub fn create_connection_callable<E>(
    port: f64,
    host: &str,
    callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
) -> NodeResult<Socket>
where
    E: std::fmt::Display + 'static,
{
    let socket = create_connection_source(port, host)?;
    crate::event_loop::enqueue_runtime_task(move || {
        callback.call(()).map_err(crate::error::callback_runtime_error)
    })?;
    Ok(socket)
}

pub fn create_bound_server(host: &str, port: u16) -> NodeResult<Server> {
    Server::listen(host, port)
}

pub fn create_server_with_options(options: &ListenOptions) -> NodeResult<Server> {
    Server::listen_with_options(options)
}

pub fn create_server() -> Server {
    Server::new()
}

pub fn create_server_callable<E>(
    callback: tsonic_rust_runtime::Callable<(Socket,), Result<(), E>>,
) -> Server
where
    E: std::fmt::Display + 'static,
{
    let mut server = Server::new();
    server.set_connection_callback(callback);
    server
}

pub fn lookup_endpoint(host: &str, port: u16) -> NodeResult<Vec<String>> {
    (host, port)
        .to_socket_addrs()
        .map_err(map_net_error)
        .map(|items| items.map(|addr| addr.to_string()).collect())
}

fn address_info(addr: std::net::SocketAddr) -> AddressInfo {
    AddressInfo {
        address: addr.ip().to_string(),
        family: family_string(addr.ip()),
        port: addr.port(),
    }
}

fn socket_address(addr: std::net::SocketAddr) -> SocketAddress {
    SocketAddress {
        address: addr.ip().to_string(),
        family: family_string(addr.ip()),
        port: addr.port(),
        flowlabel: 0,
    }
}

fn family_string(ip: std::net::IpAddr) -> String {
    if ip.is_ipv4() {
        "IPv4".to_string()
    } else {
        "IPv6".to_string()
    }
}

fn parse_ip(value: &str) -> NodeResult<IpAddr> {
    value
        .parse::<IpAddr>()
        .map_err(|error| NodeError::new("EINVAL", error.to_string()))
}

fn ip_to_u128(ip: IpAddr) -> u128 {
    match ip {
        IpAddr::V4(value) => u32::from(value) as u128,
        IpAddr::V6(value) => u128::from(value),
    }
}

fn same_subnet(net: IpAddr, ip: IpAddr, prefix: u8) -> bool {
    match (net, ip) {
        (IpAddr::V4(net), IpAddr::V4(ip)) => {
            let mask = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - prefix)
            };
            u32::from(net) & mask == u32::from(ip) & mask
        }
        (IpAddr::V6(net), IpAddr::V6(ip)) => {
            let mask = if prefix == 0 {
                0
            } else {
                u128::MAX << (128 - prefix)
            };
            u128::from(net) & mask == u128::from(ip) & mask
        }
        _ => false,
    }
}

fn map_net_error(error: std::io::Error) -> NodeError {
    NodeError::new("ENET", error.to_string())
}
