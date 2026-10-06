use super::{NodeError, NodeResult, Server, ServerSignal, ServerState, Socket};
use crate::runtime_resources::{
    NativeResourceBudget, ResourceFrontier, RuntimeResource, RuntimeResources,
};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::Duration;
use tsonic_rust_runtime::dispatch::{DispatchContexts, DispatchPhase};
use tsonic_rust_runtime::dispatch_queue::{TaskReservation, TaskTicket};

const MAXIMUM_SERVERS: usize = 1 << 20;
pub(super) const MAXIMUM_PENDING_SIGNALS: usize = 1 << 16;
const ACCEPT_BUDGET: usize = 64;

thread_local! {
    static RESOURCE_BUDGET: NativeResourceBudget = const { NativeResourceBudget::new(MAXIMUM_SERVERS) };
    static DEFAULT_SERVERS: NetServers<tsonic_rust_runtime::TsonicError> = const { NetServers::new() };
}

pub struct NetServers<E: 'static> {
    resources: RuntimeResources<RuntimeServer<E>>,
}

impl<E: 'static> Default for NetServers<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: 'static> NetServers<E> {
    pub const fn new() -> Self {
        Self {
            resources: RuntimeResources::new(MAXIMUM_SERVERS),
        }
    }
}

impl<E: From<NodeError> + 'static> NetServers<E> {
    pub(super) fn register(&self, server: &Server<E>) {
        self.resources.register(
            server.state.borrow().reservation.ticket(),
            RuntimeServer {
                state: Rc::downgrade(&server.state),
            },
        );
    }
}

impl<E: From<NodeError> + 'static> DispatchContexts for NetServers<E> {
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
    state: Weak<RefCell<ServerState<E>>>,
}

impl<E> Clone for RuntimeServer<E> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl<E: From<NodeError>> RuntimeResource for RuntimeServer<E> {
    type Error = E;

    fn is_alive(&self) -> bool {
        self.state.strong_count() != 0
    }

    fn phase(&self) -> DispatchPhase {
        DispatchPhase::Net
    }

    fn capture(&self) -> Result<Option<TaskTicket>, E> {
        let Some(owner) = self.state.upgrade() else {
            return Ok(None);
        };
        let captured = {
            let mut state = owner.borrow_mut();
            capture_connections(&mut state)
                .map(|()| state.pending.back().map(|(ticket, _)| *ticket))
        };
        captured.map_err(E::from)
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
                state.pending.pop_front().map(|(_, signal)| signal)
            };
            match signal {
                Some(ServerSignal::Listening(callback)) => callback.call(())?,
                Some(ServerSignal::Connection(socket, callback)) => callback.call((socket,))?,
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

fn capture_connections<E>(state: &mut ServerState<E>) -> NodeResult<()> {
    if !state.listening {
        return Ok(());
    }
    let Some(callback) = state.connection_callback.as_ref() else {
        return Ok(());
    };
    for _ in 0..ACCEPT_BUDGET {
        if state.pending.len() >= MAXIMUM_PENDING_SIGNALS
            || state
                .max_connections
                .is_some_and(|maximum| state.connections >= maximum)
        {
            break;
        }
        match state
            .listener
            .as_ref()
            .expect("listening server has a listener")
            .accept()
        {
            Ok((stream, _)) => {
                let ticket = admit_signal()?;
                state.connections += 1;
                state.pending.push_back((
                    ticket,
                    ServerSignal::Connection(Socket::from_stream(stream), callback.clone()),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(error) => return Err(super::map_net_error(error)),
        }
    }
    Ok(())
}

pub(super) fn queue_limit() -> NodeError {
    NodeError::new(
        "ERR_NET_PENDING_LIMIT",
        "native server callback queue is full",
    )
}

pub(super) fn reserve_resource() -> NodeResult<TaskReservation> {
    RESOURCE_BUDGET.with(NativeResourceBudget::reserve)
}

pub(super) fn admit_signal() -> NodeResult<TaskTicket> {
    RESOURCE_BUDGET.with(NativeResourceBudget::admit)
}

pub fn with_default<TOutput>(
    operation: impl FnOnce(&NetServers<tsonic_rust_runtime::TsonicError>) -> TOutput,
) -> TOutput {
    DEFAULT_SERVERS.with(operation)
}

#[cfg(test)]
mod tests;
