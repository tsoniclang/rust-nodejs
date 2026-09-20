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
            let waker = Arc::new(mio::Waker::new(poll.registry(), mio::Token(0)).map_err(io_error)?);
            *state = Some(Readiness { poll, events: mio::Events::with_capacity(128), waker, next_token: 1 });
        }
        operation(state.as_mut().expect("initialized readiness owner"))
    })
}

fn register(source: &mut impl mio::event::Source) -> NodeResult<mio::Registry> {
    with_readiness(|state| {
        let token = state.next_token;
        state.next_token = token.checked_add(1)
            .ok_or_else(|| NodeError::new("ERR_NODE_READINESS_LIMIT", "readiness identities exhausted"))?;
        state.poll.registry().register(source, mio::Token(token), mio::Interest::READABLE).map_err(io_error)?;
        state.poll.registry().try_clone().map_err(io_error)
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
    registry: mio::Registry,
}

impl Listener {
    pub(crate) fn new(listener: std::net::TcpListener) -> NodeResult<Self> {
        listener.set_nonblocking(true).map_err(io_error)?;
        let mut source = mio::net::TcpListener::from_std(listener.try_clone().map_err(io_error)?);
        let registry = register(&mut source)?;
        Ok(Self { listener, source, registry })
    }
}

impl std::ops::Deref for Listener {
    type Target = std::net::TcpListener;
    fn deref(&self) -> &Self::Target { &self.listener }
}

impl Drop for Listener {
    fn drop(&mut self) { let _ = self.registry.deregister(&mut self.source); }
}

#[cfg(unix)]
pub(crate) struct SignalWake {
    reader: RefCell<mio::net::UnixStream>,
    registry: mio::Registry,
    hook: signal_hook::SigId,
}

#[cfg(unix)]
impl SignalWake {
    pub(crate) fn new(signal: i32) -> NodeResult<Self> {
        let (reader, writer) = std::os::unix::net::UnixStream::pair().map_err(io_error)?;
        reader.set_nonblocking(true).map_err(io_error)?;
        let mut reader = mio::net::UnixStream::from_std(reader);
        let registry = register(&mut reader)?;
        let hook = signal_hook::low_level::pipe::register(signal, writer).map_err(io_error)?;
        Ok(Self { reader: RefCell::new(reader), registry, hook })
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
        let _ = self.registry.deregister(self.reader.get_mut());
    }
}

fn io_error(error: std::io::Error) -> NodeError {
    NodeError::new("ERR_NODE_READINESS", error.to_string())
}
