use crate::runtime_resources::{
    NativeResourceBudget, ResourceFrontier, RuntimeResource, RuntimeResources,
};
use std::cell::OnceCell;
use std::time::Duration;
use tsonic_rust_runtime::dispatch::{DispatchContexts, DispatchPhase};
use tsonic_rust_runtime::TsonicError;

const HTTP_MAXIMUM_RESOURCES: usize = 1 << 20;
thread_local! {
    static HTTP_RESOURCE_BUDGET: NativeResourceBudget = const { NativeResourceBudget::new(HTTP_MAXIMUM_RESOURCES) };
    static DEFAULT_HTTP: HttpServers<TsonicError> = const { HttpServers::new() };
}

pub struct HttpServers<E: 'static> {
    resources: OnceCell<Rc<RuntimeResources<HttpResource<E>>>>,
}
pub struct HttpHandle<E: 'static> {
    resources: Weak<RuntimeResources<HttpResource<E>>>,
}
impl<E: 'static> Clone for HttpHandle<E> {
    fn clone(&self) -> Self {
        Self {
            resources: self.resources.clone(),
        }
    }
}
impl<E: 'static> Default for HttpServers<E> {
    fn default() -> Self {
        Self::new()
    }
}
impl<E: 'static> HttpServers<E> {
    pub const fn new() -> Self {
        Self {
            resources: OnceCell::new(),
        }
    }
    pub fn handle(&self) -> HttpHandle<E> {
        HttpHandle {
            resources: Rc::downgrade(
                self.resources
                    .get_or_init(|| Rc::new(RuntimeResources::new(HTTP_MAXIMUM_RESOURCES))),
            ),
        }
    }
}
impl<E: From<NodeError> + 'static> HttpHandle<E> {
    fn register(&self, ticket: TaskTicket, resource: HttpResource<E>) -> NodeResult<()> {
        let resources = self.resources.upgrade().ok_or_else(|| {
            NodeError::new(
                "ERR_HTTP_OWNER_CLOSED",
                "the owning HTTP dispatch context is closed",
            )
        })?;
        resources.register(ticket, resource);
        Ok(())
    }
    fn retain_response(&self, response: ServerResponse<E>) -> NodeResult<()> {
        if response.writable_finished() || response.destroyed() {
            return Ok(());
        }
        let reservation = reserve_http_resource()?;
        let ticket = reservation.ticket();
        response.state.borrow_mut().retained_reservation = Some(reservation);
        self.register(ticket, HttpResource::Detached(response))
    }
}
impl<E: From<NodeError> + 'static> DispatchContexts for HttpServers<E> {
    type Error = E;
    type Frontier = Option<ResourceFrontier>;
    fn prepare(&self, phase: DispatchPhase) -> Result<Self::Frontier, E> {
        self.resources
            .get()
            .map(|resources| resources.prepare(phase))
            .transpose()
    }
    fn next_ready(&self, frontier: &Self::Frontier) -> Option<u64> {
        self.resources.get()?.next_ready(frontier.as_ref()?)
    }
    fn poll_next(&self, frontier: &Self::Frontier) -> Result<bool, E> {
        match (self.resources.get(), frontier.as_ref()) {
            (Some(resources), Some(frontier)) => resources.poll_next(frontier),
            _ => Ok(false),
        }
    }
    fn has_work(&self) -> bool {
        self.resources
            .get()
            .is_some_and(|resources| resources.has_work())
    }
    fn next_delay(&self) -> Option<Duration> {
        self.resources
            .get()
            .and_then(|resources| resources.next_delay())
    }
}

enum HttpResource<E: 'static> {
    Server(Weak<RefCell<RuntimeServerState<E>>>),
    Connection(Weak<RefCell<RuntimeConnection<E>>>),
    Detached(ServerResponse<E>),
}
impl<E: 'static> Clone for HttpResource<E> {
    fn clone(&self) -> Self {
        match self {
            Self::Server(state) => Self::Server(state.clone()),
            Self::Connection(state) => Self::Connection(state.clone()),
            Self::Detached(response) => Self::Detached(response.clone()),
        }
    }
}
impl<E: From<NodeError> + 'static> RuntimeResource for HttpResource<E> {
    type Error = E;
    fn is_alive(&self) -> bool {
        match self {
            Self::Server(owner) => owner.strong_count() != 0,
            Self::Connection(owner) => owner.strong_count() != 0,
            Self::Detached(response) => !response.writable_finished() && !response.destroyed(),
        }
    }
    fn phase(&self) -> DispatchPhase {
        DispatchPhase::Http
    }
    fn capture(&self) -> Result<Option<TaskTicket>, E> {
        match self {
            Self::Server(owner) => match owner.upgrade() {
                Some(owner) => capture_http_server(&owner).map_err(E::from),
                None => Ok(None),
            },
            Self::Connection(owner) => match owner.upgrade() {
                Some(owner) => capture_http_connection(&owner).map_err(E::from),
                None => Ok(None),
            },
            Self::Detached(_) => Ok(None),
        }
    }
    fn poll(&self, boundary: Option<TaskTicket>) -> Result<bool, E> {
        let Some(boundary) = boundary else {
            return Ok(false);
        };
        match self {
            Self::Server(owner) => match owner.upgrade() {
                Some(owner) => poll_http_server(&owner, boundary),
                None => Ok(false),
            },
            Self::Connection(owner) => match owner.upgrade() {
                Some(owner) => poll_http_connection(&owner, boundary),
                None => Ok(false),
            },
            Self::Detached(_) => Ok(false),
        }
    }
    fn has_work(&self) -> bool {
        match self {
            Self::Server(owner) => owner.upgrade().is_some_and(|owner| {
                let state = owner.borrow();
                !state.pending.is_empty()
                    || state.listening_pending
                    || state.closing
                    || state.refed && (state.listening || !state.connections.is_empty())
            }),
            Self::Connection(owner) => owner
                .upgrade()
                .is_some_and(|owner| owner.borrow().pending.is_some()),
            Self::Detached(response) => !response.writable_finished() && !response.destroyed(),
        }
    }
    fn next_delay(&self) -> Option<Duration> {
        None
    }
}

fn capture_http_server<E: From<NodeError> + 'static>(
    owner: &Rc<RefCell<RuntimeServerState<E>>>,
) -> NodeResult<Option<TaskTicket>> {
    {
        let mut state = owner.borrow_mut();
        if state.listening_pending {
            let count = 1 + usize::from(state.pending_listen_callback.is_some());
            if state.pending.len() + count > HTTP_MAXIMUM_PENDING_SIGNALS {
                return Err(http_queue_limit());
            }
            let callback_ticket = state
                .pending_listen_callback
                .as_ref()
                .map(|_| admit_http_signal())
                .transpose()?;
            let event_ticket = admit_http_signal()?;
            let callback = state.pending_listen_callback.take();
            state.listening_pending = false;
            if let (Some(ticket), Some(callback)) = (callback_ticket, callback) {
                state
                    .pending
                    .push_back((ticket, ServerSignal::ListeningCallback(callback)));
            }
            let emission = state.listening_event.emission();
            state
                .pending
                .push_back((event_ticket, ServerSignal::Event(emission)));
        }
        if state.closing && state.connections.is_empty() {
            if state.pending.len() >= HTTP_MAXIMUM_PENDING_SIGNALS {
                return Err(http_queue_limit());
            }
            let ticket = admit_http_signal()?;
            state.closing = false;
            state.address = None;
            let emission = state.close_event.emission();
            state
                .pending
                .push_back((ticket, ServerSignal::Event(emission)));
        }
        if !state.closing && !state.listening {
            while !state.pending_close_callbacks.is_empty()
                && state.pending.len() < HTTP_MAXIMUM_PENDING_SIGNALS
            {
                let ticket = admit_http_signal()?;
                let callback = state
                    .pending_close_callbacks
                    .pop_front()
                    .expect("pending close callback");
                state
                    .pending
                    .push_back((ticket, ServerSignal::CloseCallback(callback, None)));
            }
        }
    }
    for _ in 0..128 {
        let accepted = {
            let state = owner.borrow();
            let Some(listener) = state.listener.as_ref() else {
                break;
            };
            match listener {
                RuntimeListener::Tcp(listener) => match listener.accept() {
                    Ok((stream, _)) => Some(RuntimeConnectionIo::Tcp(
                        crate::readiness::Connection::new(stream)?,
                    )),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
                    Err(error) => return Err(runtime_http_io_error(error)),
                },
                #[cfg(unix)]
                RuntimeListener::Unix(listener) => match listener.accept() {
                    Ok((stream, _)) => Some(RuntimeConnectionIo::Unix(
                        crate::readiness::UnixConnection::new(stream)?,
                    )),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
                    Err(error) => return Err(runtime_http_io_error(error)),
                },
            }
        };
        let Some(io) = accepted else {
            break;
        };
        let reservation = reserve_http_resource()?;
        let ticket = reservation.ticket();
        let (handler, resources) = {
            let state = owner.borrow();
            (state.handler.clone(), state.resources.clone())
        };
        let connection = Rc::new(RefCell::new(RuntimeConnection::new(
            io,
            owner,
            handler,
            reservation,
        )));
        resources.register(ticket, HttpResource::Connection(Rc::downgrade(&connection)))?;
        owner
            .borrow_mut()
            .connections
            .insert(ticket.sequence(), connection);
    }
    Ok(owner.borrow().pending.back().map(|(ticket, _)| *ticket))
}
fn poll_http_server<E: From<NodeError> + 'static>(
    owner: &Rc<RefCell<RuntimeServerState<E>>>,
    boundary: TaskTicket,
) -> Result<bool, E> {
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
            Some(ServerSignal::ListeningCallback(callback)) => callback.call(())?,
            Some(ServerSignal::CloseCallback(callback, error)) => callback.call((error,))?,
            Some(ServerSignal::Event(emission)) => crate::stream::invoke_event(emission, ())?,
            None => break,
        }
        did_work = true;
    }
    Ok(did_work)
}
fn capture_http_connection<E: From<NodeError> + 'static>(
    owner: &Rc<RefCell<RuntimeConnection<E>>>,
) -> NodeResult<Option<TaskTicket>> {
    if let Some((ticket, _)) = &owner.borrow().pending {
        return Ok(Some(*ticket));
    }
    let ticket = admit_http_signal()?;
    let action = match next_connection_action(owner) {
        Ok(action) => action,
        Err(error) => {
            let mut state = owner.borrow_mut();
            state.failure = Some(error);
            state.close();
            None
        }
    };
    let action = match action {
        Some(action) => Some(action),
        None if owner.borrow().closed => Some(next_release_action(owner)),
        None => None,
    };
    if let Some(action) = action {
        owner.borrow_mut().pending = Some((ticket, action));
        Ok(Some(ticket))
    } else {
        Ok(None)
    }
}
fn next_release_action<E: From<NodeError> + 'static>(
    owner: &Rc<RefCell<RuntimeConnection<E>>>,
) -> ConnectionAction<E> {
    let mut state = owner.borrow_mut();
    let error = state.failure.clone().unwrap_or_else(|| {
        NodeError::new("ECONNRESET", "HTTP connection closed before completion")
    });
    if let Some(request) = state.request.take() {
        if !request.complete() && !request.destroyed() {
            return ConnectionAction::AbortRequest { request, error };
        }
    }
    if let Some(response) = state.response.take() {
        if !response.writable_finished() && !response.destroyed() {
            return ConnectionAction::DestroyResponse { response, error };
        }
    }
    ConnectionAction::Release
}
fn poll_http_connection<E: From<NodeError> + 'static>(
    owner: &Rc<RefCell<RuntimeConnection<E>>>,
    boundary: TaskTicket,
) -> Result<bool, E> {
    let action = {
        let mut state = owner.borrow_mut();
        if !state
            .pending
            .as_ref()
            .is_some_and(|(ticket, _)| *ticket <= boundary)
        {
            return Ok(false);
        }
        state.pending.take().map(|(_, action)| action)
    };
    match action {
        Some(ConnectionAction::FramingProgress) => {}
        Some(ConnectionAction::Dispatch {
            handler,
            request,
            response,
        }) => {
            if let Err(error) = handler.call((request, response)) {
                owner.borrow_mut().close();
                return Err(error);
            }
        }
        Some(ConnectionAction::BodyChunk { request, chunk }) => {
            let accepted = request.push_body(chunk)?;
            let result = {
                let mut state = owner.borrow_mut();
                state.body_paused = !accepted || request.body_pressured();
                state.refresh_interest()
            };
            result.map_err(E::from)?;
        }
        Some(ConnectionAction::BodyEnd(request)) => request.finish_body()?,
        Some(ConnectionAction::AbortRequest { request, error }) => {
            request.abort(Some(error.into()))?
        }
        Some(ConnectionAction::DestroyResponse { response, error }) => {
            response.destroy_chain(Some(error.into()))?;
        }
        Some(ConnectionAction::WritableProgress(writable)) => writable.poll_progress()?,
        Some(ConnectionAction::WritableComplete(writable)) => {
            writable.poll_progress()?;
            writable.complete_finish()?;
            finalize_response(owner).map_err(E::from)?;
        }
        Some(ConnectionAction::Release) => {
            let (server, id) = {
                let state = owner.borrow();
                (
                    state.server.upgrade(),
                    state.reservation.ticket().sequence(),
                )
            };
            if let Some(server) = server {
                let released = server.borrow_mut().connections.remove(&id);
                drop(released);
            }
        }
        None => return Ok(false),
    }
    Ok(true)
}
fn reserve_http_resource() -> NodeResult<TaskReservation> {
    HTTP_RESOURCE_BUDGET.with(NativeResourceBudget::reserve)
}
fn admit_http_signal() -> NodeResult<TaskTicket> {
    HTTP_RESOURCE_BUDGET.with(NativeResourceBudget::admit)
}
fn http_queue_limit() -> NodeError {
    NodeError::new(
        "ERR_HTTP_CALLBACK_LIMIT",
        "native HTTP callback queue is full",
    )
}
pub fn with_default_http<T>(operation: impl FnOnce(&HttpServers<TsonicError>) -> T) -> T {
    DEFAULT_HTTP.with(operation)
}
