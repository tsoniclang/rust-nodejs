use super::physical_child::WorkerChild;
use super::{
    io_error, status_code, Worker, WorkerCompletionError, WorkerFrame, WorkerState,
    MAXIMUM_QUEUED_SIGNALS,
};
use crate::error::{NodeError, NodeResult};
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Waker};
use tsonic_rust_runtime::dispatch_queue::TaskTicket;

pub(super) struct WorkerPhysical {
    pub(super) child: WorkerChild,
    state: Mutex<PhysicalState>,
    readiness: Arc<mio::Waker>,
    capacity: Condvar,
}

struct PhysicalState {
    frames: VecDeque<QueuedFrame>,
    capacity_waiters: usize,
    consumer_open: bool,
    callback_active: bool,
    reader_finished: bool,
    reader_error: Option<NodeError>,
    reader_ticket: Option<TaskTicket>,
    reader_consumed: bool,
    exit_ticket: Option<TaskTicket>,
    exit_consumed: bool,
    process: Option<NodeResult<i32>>,
    termination_requested: bool,
    native_waiters: WorkerCompletionWaiters,
    owned_waiters: WorkerCompletionWaiters,
}

struct QueuedFrame {
    ticket: Option<TaskTicket>,
    frame: WorkerFrame,
}

pub(super) enum PhysicalSignal {
    Frame(WorkerFrame),
    ReaderEnd(Option<NodeError>),
    Exit(NodeResult<i32>),
}

pub(super) struct PhysicalSnapshot {
    #[cfg(test)]
    pub(super) reader_finished: bool,
    #[cfg(test)]
    pub(super) callback_active: bool,
    pub(super) process_finished: bool,
}

impl WorkerPhysical {
    pub(super) fn new(child: WorkerChild) -> NodeResult<Self> {
        let readiness = match crate::readiness::waker() {
            Ok(readiness) => readiness,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        Ok(Self {
            child,
            state: Mutex::new(PhysicalState {
                frames: VecDeque::new(),
                capacity_waiters: 0,
                consumer_open: true,
                callback_active: false,
                reader_finished: false,
                reader_error: None,
                reader_ticket: None,
                reader_consumed: false,
                exit_ticket: None,
                exit_consumed: false,
                process: None,
                termination_requested: false,
                native_waiters: WorkerCompletionWaiters::default(),
                owned_waiters: WorkerCompletionWaiters::default(),
            }),
            readiness,
            capacity: Condvar::new(),
        })
    }

    pub(super) fn enqueue_frame(&self, frame: WorkerFrame) -> bool {
        let mut state = crate::sync::lock(&self.state);
        while state.frames.len() == MAXIMUM_QUEUED_SIGNALS && state.consumer_open {
            state.capacity_waiters += 1;
            state = self
                .capacity
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.capacity_waiters -= 1;
        }
        if !state.consumer_open || state.reader_finished {
            drop(state);
            drop(frame);
            return false;
        }
        state.frames.push_back(QueuedFrame {
            ticket: None,
            frame,
        });
        let notify_callbacks = state.callback_active;
        let wake = state.native_waiters.take_wakers();
        drop(state);
        wake.wake_all();
        if notify_callbacks {
            let _ = self.readiness.wake();
        }
        true
    }

    pub(super) fn admit_frames(&self) -> NodeResult<Option<TaskTicket>> {
        let mut state = crate::sync::lock(&self.state);
        let unadmitted = state
            .frames
            .iter()
            .rev()
            .take_while(|queued| queued.ticket.is_none())
            .count();
        let first = state.frames.len() - unadmitted;
        for queued in state.frames.range_mut(first..) {
            if queued.ticket.is_none() {
                queued.ticket = Some(super::super::resources::admit_signal()?);
            }
        }
        if state.reader_finished && !state.reader_consumed && state.reader_ticket.is_none() {
            state.reader_ticket = Some(super::super::resources::admit_signal()?);
        }
        if state.process.is_some() && !state.exit_consumed && state.exit_ticket.is_none() {
            state.exit_ticket = Some(super::super::resources::admit_signal()?);
        }
        Ok(state
            .exit_ticket
            .filter(|_| !state.exit_consumed)
            .or(state.reader_ticket.filter(|_| !state.reader_consumed))
            .or_else(|| state.frames.back().and_then(|frame| frame.ticket)))
    }

    pub(super) fn take_signal(
        &self,
        boundary: Option<TaskTicket>,
        include_exit: bool,
    ) -> Option<PhysicalSignal> {
        let mut state = crate::sync::lock(&self.state);
        let admitted = |ticket: Option<TaskTicket>| {
            boundary.is_none_or(|boundary| ticket.is_some_and(|ticket| ticket <= boundary))
        };
        let signal = if let Some(front) = state.frames.front() {
            if !admitted(front.ticket) {
                return None;
            }
            PhysicalSignal::Frame(
                state
                    .frames
                    .pop_front()
                    .expect("checked native FIFO head")
                    .frame,
            )
        } else if state.reader_finished && !state.reader_consumed {
            if !admitted(state.reader_ticket) {
                return None;
            }
            state.reader_consumed = true;
            PhysicalSignal::ReaderEnd(state.reader_error.take())
        } else if include_exit && state.reader_consumed && !state.exit_consumed {
            if !admitted(state.exit_ticket) {
                return None;
            }
            let result = state.process.clone()?;
            state.exit_consumed = true;
            PhysicalSignal::Exit(result)
        } else {
            return None;
        };
        let notify = state.capacity_waiters != 0;
        drop(state);
        if notify {
            self.capacity.notify_one();
        }
        Some(signal)
    }

    #[cfg(test)]
    pub(super) fn native_drained(&self) -> bool {
        let state = crate::sync::lock(&self.state);
        state.frames.is_empty() && state.reader_consumed
    }

    pub(super) fn consume_exit(&self) {
        crate::sync::lock(&self.state).exit_consumed = true;
    }

    pub(super) fn close_consumer(&self) {
        let (discarded, notify) = {
            let mut state = crate::sync::lock(&self.state);
            state.consumer_open = false;
            state.callback_active = false;
            (
                std::mem::take(&mut state.frames),
                state.capacity_waiters != 0,
            )
        };
        if notify {
            self.capacity.notify_all();
        }
        drop(discarded);
    }

    #[cfg(test)]
    pub(super) fn queued_len(&self) -> usize {
        crate::sync::lock(&self.state).frames.len()
    }

    #[cfg(test)]
    pub(super) fn capacity_waiters(&self) -> usize {
        crate::sync::lock(&self.state).capacity_waiters
    }

    #[cfg(test)]
    pub(super) fn exit_admitted(&self) -> bool {
        let state = crate::sync::lock(&self.state);
        state.exit_ticket.is_some() && !state.exit_consumed
    }

    #[cfg(test)]
    pub(super) fn has_callback_admission(&self) -> bool {
        let state = crate::sync::lock(&self.state);
        state.frames.iter().any(|frame| frame.ticket.is_some())
            || state.reader_ticket.is_some()
            || state.exit_ticket.is_some()
    }

    pub(super) fn snapshot(&self) -> PhysicalSnapshot {
        let state = crate::sync::lock(&self.state);
        PhysicalSnapshot {
            #[cfg(test)]
            reader_finished: state.reader_finished,
            #[cfg(test)]
            callback_active: state.callback_active,
            process_finished: state.process.is_some(),
        }
    }

    pub(super) fn suppresses_native_io(&self) -> bool {
        let state = crate::sync::lock(&self.state);
        state.termination_requested || state.process.as_ref().is_some_and(Result::is_ok)
    }

    pub(super) fn result(&self) -> Option<NodeResult<i32>> {
        crate::sync::lock(&self.state).process.clone()
    }

    pub(super) fn request_termination(&self) -> NodeResult<()> {
        {
            let mut state = crate::sync::lock(&self.state);
            if state.process.is_some() || state.termination_requested {
                return Ok(());
            }
            state.termination_requested = true;
        }
        if let Err(error) = self.child.kill() {
            let mut state = crate::sync::lock(&self.state);
            if state.process.is_some() {
                return Ok(());
            }
            state.termination_requested = false;
            return Err(io_error(error));
        }
        Ok(())
    }

    pub(super) fn owner_changed(&self, active: bool) {
        let (native, owned) = {
            let mut state = crate::sync::lock(&self.state);
            state.callback_active = active;
            (
                state.native_waiters.take_wakers(),
                state.owned_waiters.take_wakers(),
            )
        };
        native.wake_all();
        owned.wake_all();
        if active {
            let _ = self.readiness.wake();
        }
    }

    pub(super) fn finish_reader(&self, error: Option<NodeError>) {
        let (wake, notify_callbacks) = {
            let mut state = crate::sync::lock(&self.state);
            state.reader_finished = true;
            state.reader_error = error;
            (state.native_waiters.take_wakers(), state.callback_active)
        };
        wake.wake_all();
        if notify_callbacks {
            let _ = self.readiness.wake();
        }
        let result = self.child.wait().map_err(io_error).and_then(status_code);
        let (native, owned, notify_callbacks) = {
            let mut state = crate::sync::lock(&self.state);
            state.process = Some(result);
            (
                state.native_waiters.take_wakers(),
                state.owned_waiters.take_wakers(),
                state.callback_active,
            )
        };
        native.wake_all();
        owned.wake_all();
        if notify_callbacks {
            let _ = self.readiness.wake();
        }
    }
}

impl<E: From<NodeError> + 'static> Worker<E> {
    pub async fn terminate(&self) -> Result<i32, WorkerCompletionError> {
        self.state.physical.request_termination()?;
        WorkerCompletion::new(&self.state).await
    }
}

#[derive(Default)]
pub(super) struct WorkerCompletionWaiters {
    next: usize,
    first: Option<(usize, Waker)>,
    additional: Vec<(usize, Waker)>,
}

impl WorkerCompletionWaiters {
    fn get(&self, identity: usize) -> Option<&Waker> {
        self.first
            .as_ref()
            .filter(|(selected, _)| *selected == identity)
            .or_else(|| {
                self.additional
                    .iter()
                    .find(|(selected, _)| *selected == identity)
            })
            .map(|(_, waker)| waker)
    }

    fn register(&mut self, waker: &mut Option<Waker>) -> NodeResult<usize> {
        let identity = self.next.checked_add(1).ok_or_else(|| {
            NodeError::new(
                "ERR_WORKER_COMPLETION_LIMIT",
                "native worker completion identities are exhausted",
            )
        })?;
        let first = self.first.is_none() && self.additional.is_empty();
        if !first {
            self.additional.try_reserve(1).map_err(|_| {
                NodeError::new(
                    "ERR_WORKER_COMPLETION_LIMIT",
                    "native worker completion storage cannot be allocated",
                )
            })?;
        }
        let selected = (
            identity,
            waker.take().expect("prepared native completion waker"),
        );
        if first {
            self.first = Some(selected);
        } else {
            self.additional.push(selected);
        }
        self.next = identity;
        Ok(identity)
    }

    fn replace(&mut self, identity: usize, waker: Waker) -> Waker {
        let selected = self
            .first
            .as_mut()
            .filter(|(selected, _)| *selected == identity)
            .or_else(|| {
                self.additional
                    .iter_mut()
                    .find(|(selected, _)| *selected == identity)
            })
            .expect("registered native completion waiter");
        std::mem::replace(&mut selected.1, waker)
    }

    fn remove(&mut self, identity: usize) -> Option<Waker> {
        if self
            .first
            .as_ref()
            .is_some_and(|(selected, _)| *selected == identity)
        {
            return self.first.take().map(|(_, waker)| waker);
        }
        self.additional
            .iter()
            .position(|(selected, _)| *selected == identity)
            .map(|index| self.additional.remove(index).1)
    }

    fn take_wakers(&mut self) -> CompletionWakes {
        CompletionWakes {
            first: self.first.take().map(|(_, waker)| waker),
            additional: std::mem::take(&mut self.additional),
        }
    }
}

struct CompletionWakes {
    first: Option<Waker>,
    additional: Vec<(usize, Waker)>,
}

impl CompletionWakes {
    fn wake_all(self) {
        if let Some(waker) = self.first {
            waker.wake();
        }
        for (_, waker) in self.additional {
            waker.wake();
        }
    }
}

pub(super) struct WorkerCompletion<'worker> {
    state: &'worker WorkerState,
    registration: Option<(bool, usize)>,
    observed_unowned: bool,
}

impl<'worker> WorkerCompletion<'worker> {
    pub(super) fn new(state: &'worker WorkerState) -> Self {
        Self {
            state,
            registration: None,
            observed_unowned: false,
        }
    }
}

impl Future for WorkerCompletion<'_> {
    type Output = Result<i32, WorkerCompletionError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let waiting = self.get_mut();
        let physical = &waiting.state.physical;
        let mut prepared = None;
        loop {
            waiting.observed_unowned |= waiting.state.delivery.borrow().owner.is_none();
            waiting.state.ingest_signals(true)?;
            #[cfg(test)]
            super::completion_tests::at_observation_gap();
            let owned = waiting.state.delivery.borrow().owner.is_some();
            let mut state = crate::sync::lock(&physical.state);
            let drained = state.frames.is_empty() && state.reader_consumed;
            if state.process.is_some() && (owned || drained) {
                drop(state);
                drop(prepared);
                return Poll::Ready(
                    waiting
                        .state
                        .finish_native_observation(waiting.observed_unowned),
                );
            }
            if !owned
                && (!state.frames.is_empty() || (state.reader_finished && !state.reader_consumed))
            {
                drop(state);
                continue;
            }
            let waiters = if owned {
                &mut state.owned_waiters
            } else {
                &mut state.native_waiters
            };
            let identity = waiting
                .registration
                .filter(|(registered_owned, identity)| {
                    *registered_owned == owned && waiters.get(*identity).is_some()
                })
                .map(|(_, identity)| identity);
            let matches = identity.is_some_and(|identity| {
                waiters
                    .get(identity)
                    .is_some_and(|waker| waker.will_wake(context.waker()))
            });
            if !matches && prepared.is_none() {
                drop(state);
                #[cfg(test)]
                super::completion_tests::at_waker_preparation();
                prepared = Some(context.waker().clone());
                continue;
            }
            let previous = if matches {
                None
            } else if let Some(identity) = identity {
                Some(waiters.replace(
                    identity,
                    prepared.take().expect("prepared replacement native waker"),
                ))
            } else {
                match waiters.register(&mut prepared) {
                    Ok(identity) => {
                        waiting.registration = Some((owned, identity));
                        None
                    }
                    Err(error) => {
                        drop(state);
                        drop(prepared);
                        return Poll::Ready(Err(error.into()));
                    }
                }
            };
            drop(state);
            drop(previous);
            drop(prepared);
            return Poll::Pending;
        }
    }
}

impl Drop for WorkerCompletion<'_> {
    fn drop(&mut self) {
        if let Some((owned, identity)) = self.registration {
            let physical = &self.state.physical;
            let removed = {
                let mut state = crate::sync::lock(&physical.state);
                if owned {
                    state.owned_waiters.remove(identity)
                } else {
                    state.native_waiters.remove(identity)
                }
            };
            drop(removed);
        }
    }
}
