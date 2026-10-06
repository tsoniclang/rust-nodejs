use super::server::{TlsServer, TlsServerState, TlsSignal};
use super::{map_io_error, map_tls_error, TlsSocket};
use crate::runtime_resources::{
    NativeResourceBudget, ResourceFrontier, RuntimeResource, RuntimeResources,
};
use crate::{NodeError, NodeResult};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::Duration;
use tsonic_rust_runtime::dispatch::{DispatchContexts, DispatchPhase};
use tsonic_rust_runtime::dispatch_queue::{TaskReservation, TaskTicket};
use tsonic_rust_runtime::TsonicError;

const MAXIMUM_SERVERS: usize = 1 << 20;
pub(super) const MAXIMUM_PENDING_SIGNALS: usize = 1 << 16;
const ACCEPT_BUDGET: usize = 64;

thread_local! {
    static RESOURCE_BUDGET: NativeResourceBudget = const { NativeResourceBudget::new(MAXIMUM_SERVERS) };
    static DEFAULT_SERVERS: TlsServers<TsonicError> = const { TlsServers::new() };
}

pub struct TlsServers<E: 'static> {
    resources: RuntimeResources<RuntimeServer<E>>,
}
impl<E: 'static> Default for TlsServers<E> {
    fn default() -> Self {
        Self::new()
    }
}
impl<E: 'static> TlsServers<E> {
    pub const fn new() -> Self {
        Self {
            resources: RuntimeResources::new(MAXIMUM_SERVERS),
        }
    }
}
impl<E: From<NodeError> + 'static> TlsServers<E> {
    pub(super) fn register(&self, server: &TlsServer<E>) {
        self.resources.register(
            server.state.borrow().reservation.ticket(),
            RuntimeServer {
                state: Rc::downgrade(&server.state),
            },
        );
    }
}
impl<E: From<NodeError> + 'static> DispatchContexts for TlsServers<E> {
    type Error = E;
    type Frontier = ResourceFrontier;
    fn prepare(&self, phase: DispatchPhase) -> Result<Self::Frontier, E> {
        self.resources.prepare(phase)
    }
    fn next_ready(&self, frontier: &Self::Frontier) -> Option<u64> {
        self.resources.next_ready(frontier)
    }
    fn poll_next(&self, frontier: &Self::Frontier) -> Result<bool, E> {
        self.resources.poll_next(frontier)
    }
    fn has_work(&self) -> bool {
        self.resources.has_work()
    }
    fn next_delay(&self) -> Option<Duration> {
        self.resources.next_delay()
    }
}

struct RuntimeServer<E> {
    state: Weak<RefCell<TlsServerState<E>>>,
}
impl<E> Clone for RuntimeServer<E> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}
impl<E: From<NodeError> + 'static> RuntimeResource for RuntimeServer<E> {
    type Error = E;
    fn is_alive(&self) -> bool {
        self.state.strong_count() != 0
    }
    fn phase(&self) -> DispatchPhase {
        DispatchPhase::Tls
    }
    fn capture(&self) -> Result<Option<TaskTicket>, E> {
        let Some(owner) = self.state.upgrade() else {
            return Ok(None);
        };
        let result = {
            let mut state = owner.borrow_mut();
            capture_connections(&mut state)
                .map(|()| state.pending.back().map(|(ticket, _)| *ticket))
        };
        result.map_err(E::from)
    }
    fn poll(&self, boundary: Option<TaskTicket>) -> Result<bool, E> {
        let Some(owner) = self.state.upgrade() else {
            return Ok(false);
        };
        let Some(boundary) = boundary else {
            return Ok(false);
        };
        let mut did_work = false;
        loop {
            let signal = {
                let mut state = owner.borrow_mut();
                if !state
                    .pending
                    .front()
                    .is_some_and(|(ticket, _)| *ticket <= boundary)
                {
                    break;
                }
                state
                    .pending
                    .pop_front()
                    .map(|(_, signal)| (signal, state.background.clone()))
            };
            match signal {
                Some((TlsSignal::Listening(callback), _)) => callback.call(())?,
                Some((TlsSignal::Connection(stream, config, callback), background)) => {
                    background
                        .spawn(
                            move || {
                                stream.set_nonblocking(false).map_err(map_io_error)?;
                                let timeout = Duration::from_secs(5);
                                stream
                                    .set_read_timeout(Some(timeout))
                                    .map_err(map_io_error)?;
                                stream
                                    .set_write_timeout(Some(timeout))
                                    .map_err(map_io_error)?;
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
                                let stream = result.map_err(E::from)?;
                                callback.call((TlsSocket::from_server(stream),))
                            },
                        )
                        .map_err(E::from)?;
                }
                None => break,
            }
            did_work = true;
        }
        Ok(did_work)
    }
    fn has_work(&self) -> bool {
        self.state.upgrade().is_some_and(|owner| {
            let state = owner.borrow();
            !state.pending.is_empty() || state.refed && state.listening
        })
    }
    fn next_delay(&self) -> Option<Duration> {
        None
    }
}

fn capture_connections<E>(state: &mut TlsServerState<E>) -> NodeResult<()> {
    if !state.listening {
        return Ok(());
    }
    let listener = state.listener.as_ref().expect("listening TLS server");
    for _ in 0..ACCEPT_BUDGET {
        if state.pending.len() >= MAXIMUM_PENDING_SIGNALS {
            break;
        }
        let ticket = admit_signal()?;
        match listener.accept() {
            Ok((stream, _)) => state.pending.push_back((
                ticket,
                TlsSignal::Connection(stream, Arc::clone(&state.config), state.callback.clone()),
            )),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(error) => return Err(map_io_error(error)),
        }
    }
    Ok(())
}
pub(super) fn reserve_resource() -> NodeResult<TaskReservation> {
    RESOURCE_BUDGET.with(NativeResourceBudget::reserve)
}
pub(super) fn admit_signal() -> NodeResult<TaskTicket> {
    RESOURCE_BUDGET.with(NativeResourceBudget::admit)
}
pub(super) fn queue_limit() -> NodeError {
    NodeError::new(
        "ERR_TLS_CALLBACK_LIMIT",
        "native TLS callback queue is full",
    )
}
pub fn with_default_tls<TOutput>(
    operation: impl FnOnce(&TlsServers<TsonicError>) -> TOutput,
) -> TOutput {
    DEFAULT_SERVERS.with(operation)
}
#[cfg(test)]
mod tests;
