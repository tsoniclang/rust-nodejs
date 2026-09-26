use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;

use crate::{NodeError, NodeResult};

struct Readiness {
    poll: mio::Poll,
    events: mio::Events,
    waker: Arc<mio::Waker>,
    next_token: usize,
}

thread_local! {
    static READINESS: RefCell<Option<Readiness>> = const { RefCell::new(None) };
}

fn with_readiness<T>(operation: impl FnOnce(&mut Readiness) -> NodeResult<T>) -> NodeResult<T> {
    READINESS.with(|state| {
        let mut state = state.borrow_mut();
        if state.is_none() {
            let poll = mio::Poll::new().map_err(io_error)?;
            let waker =
                Arc::new(mio::Waker::new(poll.registry(), mio::Token(0)).map_err(io_error)?);
            *state = Some(Readiness {
                poll,
                events: mio::Events::with_capacity(128),
                waker,
                next_token: 1,
            });
        }
        operation(state.as_mut().expect("initialized readiness owner"))
    })
}

struct Registration {
    registry: mio::Registry,
    token: mio::Token,
}

fn update_interest(
    source: &mut impl mio::event::Source,
    registration: &Registration,
    registered: &mut bool,
    current_readable: &mut bool,
    current_writable: &mut bool,
    readable: bool,
    writable: bool,
) -> NodeResult<()> {
    if *current_readable == readable && *current_writable == writable {
        return Ok(());
    }
    let interest = match (readable, writable) {
        (true, true) => Some(mio::Interest::READABLE | mio::Interest::WRITABLE),
        (true, false) => Some(mio::Interest::READABLE),
        (false, true) => Some(mio::Interest::WRITABLE),
        (false, false) => None,
    };
    match (*registered, interest) {
        (true, Some(interest)) => registration
            .registry
            .reregister(source, registration.token, interest)
            .map_err(io_error)?,
        (true, None) => {
            registration.registry.deregister(source).map_err(io_error)?;
            *registered = false;
        }
        (false, Some(interest)) => {
            registration
                .registry
                .register(source, registration.token, interest)
                .map_err(io_error)?;
            *registered = true;
        }
        (false, None) => {}
    }
    *current_readable = readable;
    *current_writable = writable;
    Ok(())
}

fn register(
    source: &mut impl mio::event::Source,
    interest: mio::Interest,
) -> NodeResult<Registration> {
    with_readiness(|state| {
        let token = state.next_token;
        state.next_token = token.checked_add(1).ok_or_else(|| {
            NodeError::new("ERR_NODE_READINESS_LIMIT", "readiness identities exhausted")
        })?;
        state
            .poll
            .registry()
            .register(source, mio::Token(token), interest)
            .map_err(io_error)?;
        Ok(Registration {
            registry: state.poll.registry().try_clone().map_err(io_error)?,
            token: mio::Token(token),
        })
    })
}

pub(crate) fn waker() -> NodeResult<Arc<mio::Waker>> {
    with_readiness(|state| Ok(Arc::clone(&state.waker)))
}

pub(crate) fn wait(timeout: Option<Duration>) -> NodeResult<()> {
    with_readiness(|state| match state.poll.poll(&mut state.events, timeout) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => Ok(()),
        Err(error) => Err(io_error(error)),
    })
}

pub(crate) struct Listener {
    listener: std::net::TcpListener,
    source: mio::net::TcpListener,
    registration: Registration,
}

impl Listener {
    pub(crate) fn new(listener: std::net::TcpListener) -> NodeResult<Self> {
        listener.set_nonblocking(true).map_err(io_error)?;
        let mut source = mio::net::TcpListener::from_std(listener.try_clone().map_err(io_error)?);
        let registration = register(&mut source, mio::Interest::READABLE)?;
        Ok(Self {
            listener,
            source,
            registration,
        })
    }
}

impl std::ops::Deref for Listener {
    type Target = std::net::TcpListener;
    fn deref(&self) -> &Self::Target {
        &self.listener
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self.registration.registry.deregister(&mut self.source);
    }
}

#[cfg(unix)]
pub(crate) struct UnixListener {
    listener: std::os::unix::net::UnixListener,
    source: mio::net::UnixListener,
    registration: Registration,
}

#[cfg(unix)]
impl UnixListener {
    pub(crate) fn bind(path: &std::path::Path) -> NodeResult<Self> {
        let listener = std::os::unix::net::UnixListener::bind(path).map_err(io_error)?;
        listener.set_nonblocking(true).map_err(io_error)?;
        let mut source = mio::net::UnixListener::from_std(listener.try_clone().map_err(io_error)?);
        let registration = register(&mut source, mio::Interest::READABLE)?;
        Ok(Self {
            listener,
            source,
            registration,
        })
    }

    pub(crate) fn accept(
        &self,
    ) -> std::io::Result<(
        std::os::unix::net::UnixStream,
        std::os::unix::net::SocketAddr,
    )> {
        self.listener.accept()
    }
}

#[cfg(unix)]
impl Drop for UnixListener {
    fn drop(&mut self) {
        let _ = self.registration.registry.deregister(&mut self.source);
    }
}

pub(crate) struct Connection {
    stream: std::net::TcpStream,
    source: mio::net::TcpStream,
    registration: Registration,
    registered: bool,
    readable: bool,
    writable: bool,
}

impl Connection {
    pub(crate) fn new(stream: std::net::TcpStream) -> NodeResult<Self> {
        stream.set_nonblocking(true).map_err(io_error)?;
        let mut source = mio::net::TcpStream::from_std(stream.try_clone().map_err(io_error)?);
        let registration = register(&mut source, mio::Interest::READABLE)?;
        Ok(Self {
            stream,
            source,
            registration,
            registered: true,
            readable: true,
            writable: false,
        })
    }

    pub(crate) fn set_interest(&mut self, readable: bool, writable: bool) -> NodeResult<()> {
        update_interest(
            &mut self.source,
            &self.registration,
            &mut self.registered,
            &mut self.readable,
            &mut self.writable,
            readable,
            writable,
        )
    }

    pub(crate) fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.stream.local_addr()
    }

    pub(crate) fn peer_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.stream.peer_addr()
    }

    pub(crate) fn shutdown(&self) -> std::io::Result<()> {
        self.stream.shutdown(std::net::Shutdown::Both)
    }
}

impl std::io::Read for Connection {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        std::io::Read::read(&mut self.stream, buffer)
    }
}

impl std::io::Write for Connection {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        std::io::Write::write(&mut self.stream, buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut self.stream)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if self.registered {
            let _ = self.registration.registry.deregister(&mut self.source);
        }
    }
}

#[cfg(unix)]
pub(crate) struct UnixConnection {
    stream: std::os::unix::net::UnixStream,
    source: mio::net::UnixStream,
    registration: Registration,
    registered: bool,
    readable: bool,
    writable: bool,
}

#[cfg(unix)]
impl UnixConnection {
    pub(crate) fn new(stream: std::os::unix::net::UnixStream) -> NodeResult<Self> {
        stream.set_nonblocking(true).map_err(io_error)?;
        let mut source = mio::net::UnixStream::from_std(stream.try_clone().map_err(io_error)?);
        let registration = register(&mut source, mio::Interest::READABLE)?;
        Ok(Self {
            stream,
            source,
            registration,
            registered: true,
            readable: true,
            writable: false,
        })
    }

    pub(crate) fn set_interest(&mut self, readable: bool, writable: bool) -> NodeResult<()> {
        update_interest(
            &mut self.source,
            &self.registration,
            &mut self.registered,
            &mut self.readable,
            &mut self.writable,
            readable,
            writable,
        )
    }

    pub(crate) fn shutdown(&self) -> std::io::Result<()> {
        self.stream.shutdown(std::net::Shutdown::Both)
    }
}

#[cfg(unix)]
impl std::io::Read for UnixConnection {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        std::io::Read::read(&mut self.stream, buffer)
    }
}

#[cfg(unix)]
impl std::io::Write for UnixConnection {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        std::io::Write::write(&mut self.stream, buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut self.stream)
    }
}

#[cfg(unix)]
impl Drop for UnixConnection {
    fn drop(&mut self) {
        if self.registered {
            let _ = self.registration.registry.deregister(&mut self.source);
        }
    }
}

#[cfg(unix)]
pub(crate) struct SignalWake {
    reader: RefCell<mio::net::UnixStream>,
    registration: Registration,
    hook: signal_hook::SigId,
}

#[cfg(unix)]
impl SignalWake {
    pub(crate) fn new(signal: i32) -> NodeResult<Self> {
        let (reader, writer) = std::os::unix::net::UnixStream::pair().map_err(io_error)?;
        reader.set_nonblocking(true).map_err(io_error)?;
        let mut reader = mio::net::UnixStream::from_std(reader);
        let registration = register(&mut reader, mio::Interest::READABLE)?;
        let hook = signal_hook::low_level::pipe::register(signal, writer).map_err(io_error)?;
        Ok(Self {
            reader: RefCell::new(reader),
            registration,
            hook,
        })
    }

    pub(crate) fn drain(&self) -> NodeResult<()> {
        use std::io::Read;
        let mut bytes = [0; 128];
        loop {
            match self.reader.borrow_mut().read(&mut bytes) {
                Ok(0) => return Ok(()),
                Ok(_) => (),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => (),
                Err(error) => return Err(io_error(error)),
            }
        }
    }
}

#[cfg(unix)]
impl Drop for SignalWake {
    fn drop(&mut self) {
        signal_hook::low_level::unregister(self.hook);
        let _ = self.registration.registry.deregister(self.reader.get_mut());
    }
}

fn io_error(error: std::io::Error) -> NodeError {
    NodeError::new("ERR_NODE_READINESS", error.to_string())
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    #[test]
    fn notifications_before_and_during_wait_are_not_lost() {
        let wake = super::waker().unwrap();
        wake.wake().unwrap();
        let before = Instant::now();
        super::wait(Some(Duration::from_secs(3))).unwrap();
        assert!(before.elapsed() < Duration::from_secs(1));
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            wake.wake().unwrap();
        });
        let before = Instant::now();
        super::wait(Some(Duration::from_secs(3))).unwrap();
        worker.join().unwrap();
        assert!(before.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn listener_readiness_reaches_the_native_acceptor() {
        let listener =
            super::Listener::new(std::net::TcpListener::bind("127.0.0.1:0").unwrap()).unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || std::net::TcpStream::connect(address).unwrap());
        super::wait(Some(Duration::from_secs(3))).unwrap();
        assert!(listener.accept().is_ok());
        drop(worker.join().unwrap());
    }
}
