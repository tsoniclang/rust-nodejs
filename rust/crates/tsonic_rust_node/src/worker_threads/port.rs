use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::{Rc, Weak};
use std::sync::mpsc::{Receiver, TryRecvError};

use tsonic_rust_js::{JsString, JsValue};
use tsonic_rust_runtime::Callable;

use crate::error::{NodeError, NodeResult};
use crate::events::EventEmitter;

use super::clone::{decode, encode, ClonedValue};
use super::protocol::{TransportEvent, WorkerFrameKind, WorkerTransport};

const MAXIMUM_QUEUED_MESSAGES: usize = 1 << 16;

#[derive(Clone)]
pub struct MessagePort {
    state: Rc<RefCell<MessagePortState>>,
    emitter: Rc<RefCell<EventEmitter>>,
}

struct MessagePortState {
    peer: Option<Weak<RefCell<MessagePortState>>>,
    transport: Option<WorkerTransport>,
    incoming: Option<Receiver<TransportEvent>>,
    messages: VecDeque<ClonedValue>,
    errors: VecDeque<String>,
    started: bool,
    closed: bool,
    refed: bool,
    close_pending: bool,
}

enum PortSignal {
    Message(JsValue),
    Error(String),
    Close,
}

impl MessagePort {
    fn local() -> Self {
        let value = Self {
            state: Rc::new(RefCell::new(MessagePortState {
                peer: None,
                transport: None,
                incoming: None,
                messages: VecDeque::new(),
                errors: VecDeque::new(),
                started: false,
                closed: false,
                refed: true,
                close_pending: false,
            })),
            emitter: Rc::new(RefCell::new(EventEmitter::new())),
        };
        register_port(&value);
        value
    }

    pub(crate) fn from_transport(
        transport: WorkerTransport,
        incoming: Receiver<TransportEvent>,
    ) -> Self {
        let value = Self {
            state: Rc::new(RefCell::new(MessagePortState {
                peer: None,
                transport: Some(transport),
                incoming: Some(incoming),
                messages: VecDeque::new(),
                errors: VecDeque::new(),
                started: false,
                closed: false,
                refed: true,
                close_pending: false,
            })),
            emitter: Rc::new(RefCell::new(EventEmitter::new())),
        };
        register_port(&value);
        value
    }

    fn connect(&self, peer: &Self) {
        self.state.borrow_mut().peer = Some(Rc::downgrade(&peer.state));
    }

    pub fn post_message(&self, value: JsValue) -> NodeResult<()> {
        let payload = ClonedValue::from_js(&value)?;
        let state = self.state.borrow();
        if state.closed {
            return Err(NodeError::new("ERR_CLOSED_MESSAGE_PORT", "message port is closed"));
        }
        if let Some(transport) = &state.transport {
            return transport.send(WorkerFrameKind::Message, &encode(&payload)?);
        }
        let peer = state.peer.as_ref().and_then(Weak::upgrade).ok_or_else(|| {
            NodeError::new("ERR_CLOSED_MESSAGE_PORT", "message port peer is closed")
        })?;
        drop(state);
        peer.borrow_mut().enqueue(payload)
    }

    pub fn receive_message(&self) -> Option<JsValue> {
        let mut state = self.state.borrow_mut();
        state.ingest_transport();
        state.messages.pop_front().map(|value| value.to_js())
    }

    pub fn start(&self) {
        self.state.borrow_mut().started = true;
    }

    pub fn close(&self) {
        let transport = {
            let mut state = self.state.borrow_mut();
            if state.closed {
                return;
            }
            state.closed = true;
            state.refed = false;
            state.close_pending = true;
            state.transport.take()
        };
        if let Some(transport) = transport {
            let _ = transport.send(WorkerFrameKind::Close, &[]);
            transport.close();
        }
    }

    pub fn unref(&mut self) -> &mut Self {
        self.state.borrow_mut().refed = false;
        self
    }

    pub fn ref_chain(&mut self) -> &mut Self {
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

    pub fn on_callable<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.start();
        self.emitter.borrow_mut().on_callable(event, listener)?;
        Ok(self)
    }

    pub fn on_callable1<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<JsValue, Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.start();
        self.emitter.borrow_mut().on_callable1(event, listener)?;
        Ok(self)
    }

    pub fn once_callable<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.start();
        self.emitter.borrow_mut().once_callable(event, listener)?;
        Ok(self)
    }

    pub fn once_callable1<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<JsValue, Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: std::fmt::Display + 'static,
    {
        self.start();
        self.emitter.borrow_mut().once_callable1(event, listener)?;
        Ok(self)
    }

    pub fn off_callable<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<(), Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: 'static,
    {
        self.emitter.borrow_mut().off_callable(event, listener)?;
        Ok(self)
    }

    pub fn off_callable1<E>(
        &mut self,
        event: &JsValue,
        listener: &Callable<JsValue, Result<(), E>>,
    ) -> NodeResult<&mut Self>
    where
        E: 'static,
    {
        self.emitter.borrow_mut().off_callable1(event, listener)?;
        Ok(self)
    }

    fn poll(&self) -> NodeResult<bool> {
        let signals = {
            let mut state = self.state.borrow_mut();
            state.ingest_transport();
            if !state.started {
                return Ok(false);
            }
            let mut signals = Vec::new();
            while let Some(value) = state.messages.pop_front() {
                signals.push(PortSignal::Message(value.to_js()));
            }
            while let Some(error) = state.errors.pop_front() {
                signals.push(PortSignal::Error(error));
            }
            if state.close_pending {
                state.close_pending = false;
                signals.push(PortSignal::Close);
            }
            signals
        };
        for signal in &signals {
            let emission = {
                let mut emitter = self.emitter.borrow_mut();
                match signal {
                    PortSignal::Message(value) => emitter.prepare_callable_emission(
                        &event_name("message"),
                        std::slice::from_ref(value),
                    )?,
                    PortSignal::Error(error) => emitter.prepare_callable_emission(
                        &event_name("error"),
                        &[JsValue::String(JsString::from_utf8(error))],
                    )?,
                    PortSignal::Close => {
                        emitter.prepare_callable_emission(&event_name("close"), &[])?
                    }
                }
            };
            emission.invoke()?;
        }
        Ok(!signals.is_empty())
    }

    fn is_refed_active(&self) -> bool {
        let state = self.state.borrow();
        state.started && state.refed && !state.closed
    }
}

impl MessagePortState {
    fn enqueue(&mut self, payload: ClonedValue) -> NodeResult<()> {
        if self.closed {
            return Err(NodeError::new("ERR_CLOSED_MESSAGE_PORT", "message port is closed"));
        }
        if self.messages.len() >= MAXIMUM_QUEUED_MESSAGES {
            return Err(NodeError::new(
                "ERR_WORKER_MESSAGE_QUEUE_LIMIT",
                "message port queue exceeds the finite message limit",
            ));
        }
        self.messages.push_back(payload);
        Ok(())
    }

    fn ingest_transport(&mut self) {
        loop {
            let event = {
                let Some(incoming) = self.incoming.as_ref() else {
                    return;
                };
                incoming.try_recv()
            };
            match event {
                Ok(TransportEvent::Frame(frame)) => match frame.kind {
                    WorkerFrameKind::Message => match decode(&frame.payload) {
                        Ok(value) => {
                            if let Err(error) = self.enqueue(value) {
                                self.errors.push_back(error.to_string());
                            }
                        }
                        Err(error) => self.errors.push_back(error.to_string()),
                    },
                    WorkerFrameKind::Error => {
                        self.errors.push_back(String::from_utf8_lossy(&frame.payload).into_owned());
                    }
                    WorkerFrameKind::Close => {
                        self.closed = true;
                        self.refed = false;
                        self.close_pending = true;
                    }
                    _ => self.errors.push_back("unexpected worker transport frame".to_string()),
                },
                Ok(TransportEvent::Failure(error)) => {
                    self.errors.push_back(error);
                    self.closed = true;
                    self.refed = false;
                    self.close_pending = true;
                }
                Ok(TransportEvent::End) => {
                    self.closed = true;
                    self.refed = false;
                    self.close_pending = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.closed = true;
                    self.refed = false;
                    self.close_pending = true;
                    break;
                }
            }
        }
    }
}

pub struct MessageChannel {
    pub port1: MessagePort,
    pub port2: MessagePort,
}

impl MessageChannel {
    pub fn new() -> Self {
        let port1 = MessagePort::local();
        let port2 = MessagePort::local();
        port1.connect(&port2);
        port2.connect(&port1);
        Self { port1, port2 }
    }
}

impl Default for MessageChannel {
    fn default() -> Self {
        Self::new()
    }
}

thread_local! {
    static PORTS: RefCell<Vec<(Weak<RefCell<MessagePortState>>, Weak<RefCell<EventEmitter>>)>> =
        const { RefCell::new(Vec::new()) };
}

fn register_port(port: &MessagePort) {
    PORTS.with(|ports| {
        ports.borrow_mut().push((
            Rc::downgrade(&port.state),
            Rc::downgrade(&port.emitter),
        ));
    });
}

pub(crate) fn poll_runtime_ports() -> tsonic_rust_runtime::TsonicResult<bool> {
    let ports = PORTS.with(|ports| {
        let mut ports = ports.borrow_mut();
        ports.retain(|(state, emitter)| state.strong_count() > 0 && emitter.strong_count() > 0);
        ports
            .iter()
            .filter_map(|(state, emitter)| Some((state.upgrade()?, emitter.upgrade()?)))
            .map(|(state, emitter)| MessagePort { state, emitter })
            .collect::<Vec<_>>()
    });
    let mut did_work = false;
    for port in ports {
        did_work |= port.poll().map_err(tsonic_rust_runtime::TsonicError::from)?;
    }
    Ok(did_work)
}

pub(crate) fn has_refed_runtime_ports() -> bool {
    PORTS.with(|ports| {
        ports.borrow().iter().any(|(state, emitter)| {
            let (Some(state), Some(emitter)) = (state.upgrade(), emitter.upgrade()) else {
                return false;
            };
            MessagePort { state, emitter }.is_refed_active()
        })
    })
}

fn event_name(value: &str) -> JsValue {
    JsValue::String(JsString::from_utf8(value))
}
