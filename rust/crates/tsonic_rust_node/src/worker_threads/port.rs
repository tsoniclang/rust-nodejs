use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::{Rc, Weak};
use std::sync::mpsc::{Receiver, TryRecvError};

use tsonic_rust_js::JsValue;
use tsonic_rust_runtime::dispatch_queue::{TaskReservation, TaskTicket};
use tsonic_rust_runtime::Callable;

use crate::error::{NodeError, NodeResult};
use crate::events::{EventEmitter, WeakEventEmitter};

use super::clone::{decode, encode, ClonedValue};
use super::protocol::{TransportEvent, WorkerFrameKind, WorkerTransport};
use super::WorkerResources;

const MAXIMUM_QUEUED_MESSAGES: usize = 1 << 16;

#[cfg(test)]
mod tests;

pub struct MessagePort<E: 'static = tsonic_rust_runtime::TsonicError> {
    state: Rc<RefCell<MessagePortState>>,
    emitter: EventEmitter<E>,
}

pub(super) struct MessagePortState {
    reservation: TaskReservation,
    peer: Option<Weak<RefCell<MessagePortState>>>,
    transport: Option<WorkerTransport>,
    incoming: Option<Receiver<TransportEvent>>,
    messages: VecDeque<(TaskTicket, ClonedValue)>,
    errors: VecDeque<(TaskTicket, String)>,
    started: bool,
    closed: bool,
    refed: bool,
    close_pending: Option<TaskTicket>,
}

enum PortSignal {
    Message(JsValue),
    Error(String),
    Close,
}

impl<E: 'static> Clone for MessagePort<E> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
            emitter: self.emitter.clone(),
        }
    }
}

impl<E: From<NodeError> + 'static> MessagePort<E> {
    fn local(resources: &WorkerResources<E>) -> NodeResult<Self> {
        let reservation = super::resources::reserve_resource()?;
        let value = Self {
            state: Rc::new(RefCell::new(MessagePortState {
                reservation,
                peer: None,
                transport: None,
                incoming: None,
                messages: VecDeque::new(),
                errors: VecDeque::new(),
                started: false,
                closed: false,
                refed: true,
                close_pending: None,
            })),
            emitter: EventEmitter::new(),
        };
        resources.register_port(&value);
        Ok(value)
    }

    pub(super) fn from_state(
        resources: &WorkerResources<E>,
        state: Rc<RefCell<MessagePortState>>,
    ) -> NodeResult<Self> {
        let value = Self {
            state,
            emitter: EventEmitter::new(),
        };
        resources.register_port(&value);
        Ok(value)
    }

    fn connect(&self, peer: &Self) {
        self.state.borrow_mut().peer = Some(Rc::downgrade(&peer.state));
    }

    pub fn post_message(&self, value: JsValue) -> NodeResult<()> {
        let payload = ClonedValue::from_js(&value)?;
        let state = self.state.borrow();
        if state.closed {
            return Err(NodeError::new(
                "ERR_CLOSED_MESSAGE_PORT",
                "message port is closed",
            ));
        }
        if let Some(transport) = &state.transport {
            return transport.send(WorkerFrameKind::Message, &encode(&payload)?);
        }
        let peer = state.peer.as_ref().and_then(Weak::upgrade).ok_or_else(|| {
            NodeError::new("ERR_CLOSED_MESSAGE_PORT", "message port peer is closed")
        })?;
        drop(state);
        let result = peer.borrow_mut().enqueue(payload);
        result
    }

    pub fn receive_message(&self) -> NodeResult<Option<JsValue>> {
        let mut state = self.state.borrow_mut();
        state.ingest_transport()?;
        Ok(state.messages.pop_front().map(|(_, value)| value.to_js()))
    }

    pub fn start(&self) {
        self.state.borrow_mut().started = true;
    }

    pub fn close(&self) -> NodeResult<()> {
        let transport = {
            let mut state = self.state.borrow_mut();
            if state.closed {
                return Ok(());
            }
            state.mark_closed()?;
            state.transport.take()
        };
        if let Some(transport) = transport {
            let _ = transport.send(WorkerFrameKind::Close, &[]);
            transport.close();
        }
        Ok(())
    }

    pub fn unref(&self) -> &Self {
        self.state.borrow_mut().refed = false;
        self
    }

    pub fn ref_chain(&self) -> &Self {
        let mut state = self.state.borrow_mut();
        if !state.closed {
            state.refed = true;
        }
        drop(state);
        self
    }

    pub fn has_ref(&self) -> bool {
        self.state.borrow().refed
    }

    pub fn on_callable(
        &self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.start();
        self.emitter.on_callable(event, listener)?;
        Ok(self)
    }

    pub fn on_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.start();
        self.emitter.on_callable1(event, listener)?;
        Ok(self)
    }

    pub fn once_callable(
        &self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.start();
        self.emitter.once_callable(event, listener)?;
        Ok(self)
    }

    pub fn once_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.start();
        self.emitter.once_callable1(event, listener)?;
        Ok(self)
    }

    pub fn off_callable(
        &self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.emitter.off_callable(event, listener)?;
        Ok(self)
    }

    pub fn off_callable1(
        &self,
        event: &JsValue,
        listener: &Callable<(JsValue,), Result<(), E>>,
    ) -> NodeResult<&Self> {
        self.emitter.off_callable1(event, listener)?;
        Ok(self)
    }

    pub(super) fn capture(&self) -> Result<Option<TaskTicket>, E> {
        let captured = {
            let mut state = self.state.borrow_mut();
            state.ingest_transport().map(|()| {
                if !state.started {
                    return None;
                }
                [
                    state.messages.back().map(|(ticket, _)| *ticket),
                    state.errors.back().map(|(ticket, _)| *ticket),
                    state.close_pending,
                ]
                .into_iter()
                .flatten()
                .max()
            })
        };
        captured.map_err(E::from)
    }

    pub(super) fn poll(&self, boundary: Option<TaskTicket>) -> Result<bool, E> {
        let Some(boundary) = boundary else {
            return Ok(false);
        };
        let mut dispatched = false;
        loop {
            let value = {
                let mut state = self.state.borrow_mut();
                if !state
                    .messages
                    .front()
                    .is_some_and(|(ticket, _)| *ticket <= boundary)
                {
                    break;
                }
                state.messages.pop_front().map(|(_, value)| value)
            };
            let Some(value) = value else {
                break;
            };
            self.dispatch_signal(PortSignal::Message(value.to_js()))?;
            dispatched = true;
        }
        loop {
            let error = {
                let mut state = self.state.borrow_mut();
                if !state
                    .errors
                    .front()
                    .is_some_and(|(ticket, _)| *ticket <= boundary)
                {
                    break;
                }
                state.errors.pop_front().map(|(_, error)| error)
            };
            let Some(error) = error else {
                break;
            };
            self.dispatch_signal(PortSignal::Error(error))?;
            dispatched = true;
        }
        if self
            .state
            .borrow()
            .close_pending
            .is_some_and(|ticket| ticket <= boundary)
        {
            self.state.borrow_mut().close_pending = None;
            self.dispatch_signal(PortSignal::Close)?;
            dispatched = true;
        }
        Ok(dispatched)
    }

    fn dispatch_signal(&self, signal: PortSignal) -> Result<(), E> {
        let converted;
        let (event, arguments): (&str, &[JsValue]) = match &signal {
            PortSignal::Message(value) => ("message", std::slice::from_ref(value)),
            PortSignal::Error(error) => {
                converted = JsValue::String(error.clone());
                ("error", std::slice::from_ref(&converted))
            }
            PortSignal::Close => ("close", &[]),
        };
        let emission = self.emitter.prepare_named_emission(event, arguments)?;
        emission.invoke(arguments)?;
        Ok(())
    }

    pub(super) fn is_refed_active(&self) -> bool {
        let state = self.state.borrow();
        state.started && state.refed && !state.closed
    }
}

impl MessagePortState {
    fn enqueue(&mut self, payload: ClonedValue) -> NodeResult<()> {
        if self.closed {
            return Err(NodeError::new(
                "ERR_CLOSED_MESSAGE_PORT",
                "message port is closed",
            ));
        }
        if self.messages.len() >= MAXIMUM_QUEUED_MESSAGES {
            return Err(NodeError::new(
                "ERR_WORKER_MESSAGE_QUEUE_LIMIT",
                "message port queue exceeds the finite message limit",
            ));
        }
        self.messages
            .push_back((super::resources::admit_signal()?, payload));
        Ok(())
    }

    fn push_error(&mut self, error: String) -> NodeResult<()> {
        self.errors
            .push_back((super::resources::admit_signal()?, error));
        Ok(())
    }

    fn mark_closed(&mut self) -> NodeResult<()> {
        if !self.closed {
            self.close_pending = Some(super::resources::admit_signal()?);
            self.closed = true;
            self.refed = false;
        }
        self.incoming = None;
        Ok(())
    }

    fn ingest_transport(&mut self) -> NodeResult<()> {
        while self.messages.len() + self.errors.len() < MAXIMUM_QUEUED_MESSAGES {
            let event = {
                let Some(incoming) = self.incoming.as_ref() else {
                    return Ok(());
                };
                incoming.try_recv()
            };
            match event {
                Ok(TransportEvent::Frame(frame)) => match frame.kind {
                    WorkerFrameKind::Message => match decode(&frame.payload) {
                        Ok(value) => {
                            if let Err(error) = self.enqueue(value) {
                                self.push_error(error.to_string())?;
                            }
                        }
                        Err(error) => self.push_error(error.to_string())?,
                    },
                    WorkerFrameKind::Error => {
                        self.push_error(String::from_utf8_lossy(&frame.payload).into_owned())?;
                    }
                    WorkerFrameKind::Close => {
                        self.mark_closed()?;
                        break;
                    }
                    _ => self.push_error("unexpected worker transport frame".to_string())?,
                },
                Ok(TransportEvent::Failure(error)) => {
                    self.push_error(error)?;
                    self.mark_closed()?;
                    break;
                }
                Ok(TransportEvent::End) => {
                    self.mark_closed()?;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.mark_closed()?;
                    break;
                }
            }
        }
        Ok(())
    }
}

pub struct MessageChannel<E: 'static = tsonic_rust_runtime::TsonicError> {
    pub port1: MessagePort<E>,
    pub port2: MessagePort<E>,
}

impl<E: 'static> Clone for MessageChannel<E> {
    fn clone(&self) -> Self {
        Self {
            port1: self.port1.clone(),
            port2: self.port2.clone(),
        }
    }
}

impl<E: From<NodeError> + 'static> MessageChannel<E> {
    pub fn new(resources: &WorkerResources<E>) -> NodeResult<Self> {
        let port1 = MessagePort::local(resources)?;
        let port2 = MessagePort::local(resources)?;
        port1.connect(&port2);
        port2.connect(&port1);
        Ok(Self { port1, port2 })
    }
}

pub(super) struct RuntimePort<E: 'static> {
    state: Weak<RefCell<MessagePortState>>,
    emitter: WeakEventEmitter<E>,
}

impl<E: 'static> Clone for RuntimePort<E> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            emitter: self.emitter.clone(),
        }
    }
}

impl<E: From<NodeError> + 'static> MessagePort<E> {
    pub(super) fn ticket(&self) -> TaskTicket {
        self.state.borrow().reservation.ticket()
    }

    pub(super) fn downgrade(&self) -> RuntimePort<E> {
        RuntimePort {
            state: Rc::downgrade(&self.state),
            emitter: self.emitter.downgrade(),
        }
    }
}

impl<E: 'static> RuntimePort<E> {
    pub(super) fn is_alive(&self) -> bool {
        self.state.strong_count() > 0 && self.emitter.is_alive()
    }

    pub(super) fn upgrade(&self) -> Option<MessagePort<E>> {
        Some(MessagePort {
            state: self.state.upgrade()?,
            emitter: self.emitter.upgrade()?,
        })
    }
}

pub(super) fn transport_state(
    transport: WorkerTransport,
    incoming: Receiver<TransportEvent>,
) -> NodeResult<Rc<RefCell<MessagePortState>>> {
    let reservation = super::resources::reserve_resource()?;
    Ok(Rc::new(RefCell::new(MessagePortState {
        reservation,
        peer: None,
        transport: Some(transport),
        incoming: Some(incoming),
        messages: VecDeque::new(),
        errors: VecDeque::new(),
        started: false,
        closed: false,
        refed: true,
        close_pending: None,
    })))
}
