use super::{io_error, Worker, WorkerState};
use crate::error::{NodeError, NodeResult};
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

impl<E: From<NodeError> + 'static> Worker<E> {
    pub async fn terminate(&self) -> NodeResult<i32> {
        {
            let mut state = self.state.borrow_mut();
            state.observe_exit()?;
            if state.exit_code.is_none() && !state.termination_requested {
                if let Err(error) = state.child.kill() {
                    state.observe_exit()?;
                    if state.exit_code.is_none() {
                        return Err(io_error(error));
                    }
                }
                state.termination_requested = true;
            }
        }
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
            waker
                .take()
                .expect("native completion retains its prepared waker"),
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
            .expect("a pending native completion owns its registration");
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

    pub(super) fn wake_all(self) {
        if let Some((_, waker)) = self.first {
            waker.wake();
        }
        for (_, waker) in self.additional {
            waker.wake();
        }
    }
}

pub(super) struct WorkerCompletion<'worker> {
    state: &'worker RefCell<WorkerState>,
    registration: Option<usize>,
}

impl<'worker> WorkerCompletion<'worker> {
    pub(super) fn new(state: &'worker RefCell<WorkerState>) -> Self {
        Self {
            state,
            registration: None,
        }
    }
}

impl Future for WorkerCompletion<'_> {
    type Output = NodeResult<i32>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let waiting = self.get_mut();
        {
            let state = waiting.state.borrow();
            if state.complete {
                return Poll::Ready(Ok(state
                    .exit_code
                    .expect("completed worker retains its native exit status")));
            }
            if waiting.registration.is_some_and(|identity| {
                state
                    .completion
                    .get(identity)
                    .is_some_and(|waker| waker.will_wake(context.waker()))
            }) {
                return Poll::Pending;
            }
        }
        let mut prepared = Some(context.waker().clone());
        let mut state = waiting.state.borrow_mut();
        if state.complete {
            return Poll::Ready(Ok(state
                .exit_code
                .expect("completed worker retains its native exit status")));
        }
        let previous = if let Some(identity) = waiting.registration {
            Some(state.completion.replace(
                identity,
                prepared.take().expect("prepared completion waker"),
            ))
        } else {
            let registered = state.completion.register(&mut prepared);
            match registered {
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
        drop(state);
        drop(previous);
        Poll::Pending
    }
}

impl Drop for WorkerCompletion<'_> {
    fn drop(&mut self) {
        if let Some(identity) = self.registration {
            let removed = self.state.borrow_mut().completion.remove(identity);
            drop(removed);
        }
    }
}
