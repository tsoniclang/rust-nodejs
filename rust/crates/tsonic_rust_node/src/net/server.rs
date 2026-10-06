use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use tsonic_rust_runtime::dispatch_queue::{TaskReservation, TaskTicket};
use tsonic_rust_runtime::Callable;

enum ServerSignal<E> {
    Listening(Callable<(), Result<(), E>>),
    Connection(Socket, Callable<(Socket,), Result<(), E>>),
}

struct ServerState<E> {
    reservation: TaskReservation,
    listener: Option<crate::readiness::Listener>,
    connection_callback: Option<Callable<(Socket,), Result<(), E>>>,
    pending: VecDeque<(TaskTicket, ServerSignal<E>)>,
    refed: bool,
    max_connections: Option<usize>,
    connections: usize,
    listening: bool,
}

pub struct Server<E: 'static = tsonic_rust_runtime::TsonicError> {
    state: Rc<RefCell<ServerState<E>>>,
}

impl<E: 'static> Clone for Server<E> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
}

impl<E: From<NodeError> + 'static> Server<E> {
    pub fn new(resources: &NetServers<E>) -> NodeResult<Self> {
        let value = Self {
            state: Rc::new(RefCell::new(ServerState {
                reservation: resources::reserve_resource()?,
                listener: None,
                connection_callback: None,
                pending: VecDeque::new(),
                refed: true,
                max_connections: None,
                connections: 0,
                listening: false,
            })),
        };
        resources.register(&value);
        Ok(value)
    }

    pub fn listen(resources: &NetServers<E>, host: &str, port: u16) -> NodeResult<Self> {
        let server = Self::new(resources)?;
        server.bind(host, port)?;
        Ok(server)
    }

    pub fn listen_with_options(
        resources: &NetServers<E>,
        options: &ListenOptions,
    ) -> NodeResult<Self> {
        Self::listen(resources, &options.host, options.port)
    }

    pub fn bind(&self, host: &str, port: u16) -> NodeResult<&Self> {
        let listener = TcpListener::bind((host, port)).map_err(map_net_error)?;
        let listener = crate::readiness::Listener::new(listener)?;
        let mut state = self.state.borrow_mut();
        state.listener = Some(listener);
        state.listening = true;
        drop(state);
        Ok(self)
    }

    pub fn listen_source(
        &self,
        port: impl tsonic_rust_runtime::conversions::IntegerInput<u16>,
        host: &str,
        callback: Option<Callable<(), Result<(), E>>>,
    ) -> NodeResult<&Self> {
        if callback.is_some()
            && self.state.borrow().pending.len() >= resources::MAXIMUM_PENDING_SIGNALS
        {
            return Err(resources::queue_limit());
        }
        self.bind(host, source_port(port)?)?;
        if let Some(callback) = callback {
            let ticket = resources::admit_signal()?;
            self.state
                .borrow_mut()
                .pending
                .push_back((ticket, ServerSignal::Listening(callback)));
        }
        Ok(self)
    }

    pub fn listen_port(
        &self,
        port: impl tsonic_rust_runtime::conversions::IntegerInput<u16>,
    ) -> NodeResult<&Self> {
        self.bind("0.0.0.0", source_port(port)?)
    }

    pub fn listen_port_host(
        &self,
        port: impl tsonic_rust_runtime::conversions::IntegerInput<u16>,
        host: &str,
    ) -> NodeResult<&Self> {
        self.bind(host, source_port(port)?)
    }

    pub fn listen_port_callable(
        &self,
        port: impl tsonic_rust_runtime::conversions::IntegerInput<u16>,
        callback: Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.listen_source(port, "0.0.0.0", Some(callback))
    }

    pub fn listen_port_host_callable(
        &self,
        port: impl tsonic_rust_runtime::conversions::IntegerInput<u16>,
        host: &str,
        callback: Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.listen_source(port, host, Some(callback))
    }

    pub fn set_connection_callback(&self, callback: Callable<(Socket,), Result<(), E>>) {
        self.state.borrow_mut().connection_callback = Some(callback);
    }

    pub fn address(&self) -> NodeResult<AddressInfo> {
        self.state
            .borrow()
            .listener
            .as_ref()
            .ok_or_else(|| NodeError::new("ERR_SERVER_NOT_RUNNING", "server is not listening"))?
            .local_addr()
            .map(address_info)
            .map_err(map_net_error)
    }

    pub fn local_addr(&self) -> NodeResult<String> {
        self.state
            .borrow()
            .listener
            .as_ref()
            .ok_or_else(|| NodeError::new("ERR_SERVER_NOT_RUNNING", "server is not listening"))?
            .local_addr()
            .map(|addr| addr.to_string())
            .map_err(map_net_error)
    }

    pub fn local_port(&self) -> NodeResult<u16> {
        self.state
            .borrow()
            .listener
            .as_ref()
            .ok_or_else(|| NodeError::new("ERR_SERVER_NOT_RUNNING", "server is not listening"))?
            .local_addr()
            .map(|addr| addr.port())
            .map_err(map_net_error)
    }

    pub fn accept(&self) -> NodeResult<Socket> {
        let mut state = self.state.borrow_mut();
        let (stream, _) = state
            .listener
            .as_ref()
            .ok_or_else(|| NodeError::new("ERR_SERVER_NOT_RUNNING", "server is not listening"))?
            .accept()
            .map_err(map_net_error)?;
        state.connections += 1;
        Ok(Socket::from_stream(stream))
    }

    pub fn close(&self) {
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

    pub fn set_max_connections(&self, value: Option<usize>) {
        self.state.borrow_mut().max_connections = value;
    }

    pub fn connections(&self) -> usize {
        self.state.borrow().connections
    }

    pub fn get_connections(&self) -> usize {
        self.connections()
    }

    pub fn r#ref(&self) {
        self.state.borrow_mut().refed = true;
    }

    pub fn ref_chain(&self) -> &Self {
        self.r#ref();
        self
    }

    pub fn unref(&self) {
        self.state.borrow_mut().refed = false;
    }

    pub fn unref_chain(&self) -> &Self {
        self.unref();
        self
    }

    pub fn has_ref(&self) -> bool {
        self.state.borrow().refed
    }
}

fn source_port(value: impl tsonic_rust_runtime::conversions::IntegerInput<u16>) -> NodeResult<u16> {
    value.checked_integer().ok_or_else(|| {
        NodeError::new(
            "ERR_SOCKET_BAD_PORT",
            "port must be an unsigned 16-bit integer",
        )
    })
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

pub fn create_connection_source(
    port: impl tsonic_rust_runtime::conversions::IntegerInput<u16>,
    host: &str,
) -> NodeResult<Socket> {
    create_connection(host, source_port(port)?)
}

pub fn create_connection_default_host(
    port: impl tsonic_rust_runtime::conversions::IntegerInput<u16>,
) -> NodeResult<Socket> {
    create_connection_source(port, "localhost")
}

pub fn create_connection_default_host_callable<E>(
    tasks: &crate::runtime_tasks::RuntimeTasks<E>,
    port: impl tsonic_rust_runtime::conversions::IntegerInput<u16>,
    callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
) -> NodeResult<Socket>
where
    E: 'static,
{
    create_connection_callable(tasks, port, "localhost", callback)
}

pub fn create_connection_callable<E>(
    tasks: &crate::runtime_tasks::RuntimeTasks<E>,
    port: impl tsonic_rust_runtime::conversions::IntegerInput<u16>,
    host: &str,
    callback: tsonic_rust_runtime::Callable<(), Result<(), E>>,
) -> NodeResult<Socket>
where
    E: 'static,
{
    let socket = create_connection_source(port, host)?;
    tasks.enqueue(move || callback.call(()))?;
    Ok(socket)
}

pub fn create_bound_server<E: From<NodeError> + 'static>(
    resources: &NetServers<E>,
    host: &str,
    port: u16,
) -> NodeResult<Server<E>> {
    Server::listen(resources, host, port)
}

pub fn create_server_with_options<E: From<NodeError> + 'static>(
    resources: &NetServers<E>,
    options: &ListenOptions,
) -> NodeResult<Server<E>> {
    Server::listen_with_options(resources, options)
}

pub fn create_server<E: From<NodeError> + 'static>(
    resources: &NetServers<E>,
) -> NodeResult<Server<E>> {
    Server::new(resources)
}

pub fn create_server_callable<E: From<NodeError> + 'static>(
    resources: &NetServers<E>,
    callback: tsonic_rust_runtime::Callable<(Socket,), Result<(), E>>,
) -> NodeResult<Server<E>> {
    let server = Server::new(resources)?;
    server.set_connection_callback(callback);
    Ok(server)
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
