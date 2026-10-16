use super::physical_child::WorkerChild;
use super::{io_error, status_code, Worker, WorkerState};
use crate::error::{NodeError, NodeResult};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

pub(super) struct WorkerPhysical {
    pub(super) child: WorkerChild,
    state: Mutex<PhysicalState>,
    change: AtomicU64,
    waiting: AtomicBool,
    readiness: Arc<mio::Waker>,
}

struct PhysicalState {
    reader_finished: bool,
    reader_error: Option<NodeError>,
    process: Option<NodeResult<i32>>,
    termination_requested: bool,
    waiters: WorkerCompletionWaiters,
}

pub(super) struct PhysicalSnapshot {
    pub(super) reader_finished: bool,
    pub(super) process_finished: bool,
    pub(super) termination_requested: bool,
    pub(super) exit_observed: bool,
    pub(super) change: u64,
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
                reader_finished: false,
                reader_error: None,
                process: None,
                termination_requested: false,
                waiters: WorkerCompletionWaiters::default(),
            }),
            change: AtomicU64::new(0),
            waiting: AtomicBool::new(false),
            readiness,
        })
    }

    pub(super) fn snapshot(&self) -> PhysicalSnapshot {
        let state = crate::sync::lock(&self.state);
        PhysicalSnapshot {
            reader_finished: state.reader_finished,
            process_finished: state.process.is_some(),
            termination_requested: state.termination_requested,
            exit_observed: state.process.as_ref().is_some_and(Result::is_ok),
            change: self.change.load(Ordering::Acquire),
        }
    }

    pub(super) fn reader_error(&self) -> Option<NodeError> {
        crate::sync::lock(&self.state).reader_error.clone()
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

    pub(super) fn published(&self) {
        self.change.fetch_add(1, Ordering::Release);
        if self.waiting.load(Ordering::Acquire) {
            let wake = {
                let mut state = crate::sync::lock(&self.state);
                let wake = state.waiters.take_wakers();
                self.waiting.store(false, Ordering::Release);
                wake
            };
            wake.wake_all();
        }
    }

    pub(super) fn finish_reader(&self, error: Option<NodeError>) {
        {
            let mut state = crate::sync::lock(&self.state);
            state.reader_finished = true;
            state.reader_error = error;
        }
        self.published();
        let _ = self.readiness.wake();
        let result = self.child.wait().map_err(io_error).and_then(status_code);
        crate::sync::lock(&self.state).process = Some(result);
        self.published();
        let _ = self.readiness.wake();
    }
}

impl<E: From<NodeError> + 'static> Worker<E> {
    pub async fn terminate(&self) -> NodeResult<i32> {
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
    registration: Option<usize>,
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
    type Output = NodeResult<i32>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let waiting = self.get_mut();
        let physical = &waiting.state.physical;
        loop {
            let before = physical.snapshot().change;
            {
                waiting.observed_unowned |= waiting.state.delivery.borrow().owner.is_none();
                waiting.state.ingest_signals(true)?;
                if physical.snapshot().process_finished {
                    let result = waiting
                        .state
                        .finish_native_observation(waiting.observed_unowned);
                    return Poll::Ready(result);
                }
            }
            let mut prepared = None;
            let changed = loop {
                let mut state = crate::sync::lock(&physical.state);
                let identity = waiting
                    .registration
                    .filter(|identity| state.waiters.get(*identity).is_some());
                let matches = identity.is_some_and(|identity| {
                    state
                        .waiters
                        .get(identity)
                        .is_some_and(|waker| waker.will_wake(context.waker()))
                });
                if !matches && prepared.is_none() {
                    drop(state);
                    prepared = Some(context.waker().clone());
                    continue;
                }
                let previous = if matches {
                    None
                } else if let Some(identity) = identity {
                    Some(state.waiters.replace(
                        identity,
                        prepared.take().expect("prepared replacement native waker"),
                    ))
                } else {
                    match state.waiters.register(&mut prepared) {
                        Ok(identity) => {
                            waiting.registration = Some(identity);
                            None
                        }
                        Err(error) => {
                            drop(state);
                            return Poll::Ready(Err(error));
                        }
                    }
                };
                physical.waiting.store(true, Ordering::Release);
                let changed =
                    before != physical.change.load(Ordering::Acquire) || state.process.is_some();
                drop(state);
                drop(previous);
                drop(prepared);
                break changed;
            };
            if !changed {
                return Poll::Pending;
            }
        }
    }
}

impl Drop for WorkerCompletion<'_> {
    fn drop(&mut self) {
        if let Some(identity) = self.registration {
            let physical = &self.state.physical;
            let removed = crate::sync::lock(&physical.state).waiters.remove(identity);
            drop(removed);
        }
    }
}
