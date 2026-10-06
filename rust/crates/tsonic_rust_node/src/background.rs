use std::cell::{OnceCell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::num::NonZeroUsize;
use std::sync::mpsc::{Receiver, SyncSender};

use tsonic_rust_runtime::dispatch_queue::{TaskBudget, TaskReservation, TaskTicket};
use tsonic_rust_runtime::TsonicError;

mod worker;
pub(crate) use worker::run;

const MAX_PENDING_BACKGROUND_WORK: usize = 16 * 1024;

trait BackgroundCompletion<TError> {
    fn complete(self: Box<Self>) -> Result<(), TError>;
}

struct TypedBackgroundCompletion<TResult, TCallback> {
    result_receiver: Receiver<crate::NodeResult<TResult>>,
    callback: TCallback,
}

impl<TResult, TCallback, TError> BackgroundCompletion<TError>
    for TypedBackgroundCompletion<TResult, TCallback>
where
    TResult: Send + 'static,
    TCallback: FnOnce(crate::NodeResult<TResult>) -> Result<(), TError> + 'static,
    TError: From<TsonicError>,
{
    fn complete(self: Box<Self>) -> Result<(), TError> {
        let Self {
            result_receiver,
            callback,
        } = *self;
        let result = result_receiver.recv().map_err(|_| {
            TError::from(TsonicError::from(crate::NodeError::new(
                "ERR_NODE_BACKGROUND_RESULT",
                "background work completed without its exact typed result",
            )))
        })?;
        callback(result)
    }
}

struct WorkCompletion {
    id: TaskTicket,
}

struct PendingCompletion<TError> {
    reservation: TaskReservation,
    callback: Box<dyn BackgroundCompletion<TError>>,
}

struct SourceThreadCompletions<TError> {
    sender: SyncSender<WorkCompletion>,
    receiver: Receiver<WorkCompletion>,
    in_flight: BTreeMap<TaskTicket, PendingCompletion<TError>>,
    ready: VecDeque<(u64, PendingCompletion<TError>)>,
    next_ready_ticket: u64,
}

impl<TError> SourceThreadCompletions<TError> {
    fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::sync_channel(MAX_PENDING_BACKGROUND_WORK);
        Self {
            sender,
            receiver,
            in_flight: BTreeMap::new(),
            ready: VecDeque::new(),
            next_ready_ticket: 0,
        }
    }

    fn pending(&self) -> usize {
        self.in_flight.len() + self.ready.len()
    }
}

pub struct BackgroundTasks<TError> {
    source: OnceCell<RefCell<SourceThreadCompletions<TError>>>,
}

impl<TError> Default for BackgroundTasks<TError> {
    fn default() -> Self {
        Self::new()
    }
}

impl<TError> BackgroundTasks<TError> {
    pub const fn new() -> Self {
        Self {
            source: OnceCell::new(),
        }
    }

    pub fn has_pending_work(&self) -> bool {
        self.source
            .get()
            .is_some_and(|source| source.borrow().pending() != 0)
    }
}

impl<TError: From<TsonicError>> BackgroundTasks<TError> {
    pub fn spawn<TResult>(
        &self,
        work: impl FnOnce() -> crate::NodeResult<TResult> + Send + 'static,
        completion: impl FnOnce(crate::NodeResult<TResult>) -> Result<(), TError> + 'static,
    ) -> crate::NodeResult<()>
    where
        TResult: Send + 'static,
    {
        let reservation = background_task_budget().reserve().map_err(|error| {
            crate::NodeError::new("ERR_NODE_BACKGROUND_WORK_LIMIT", error.to_string())
        })?;
        let wake = crate::readiness::waker()?;
        let id = reservation.ticket();
        let (result_sender, result_receiver) = std::sync::mpsc::sync_channel(1);
        let source = self
            .source
            .get_or_init(|| RefCell::new(SourceThreadCompletions::new()));
        let completion_sender = {
            let mut source = source.borrow_mut();
            source.in_flight.insert(
                id,
                PendingCompletion {
                    reservation,
                    callback: Box::new(TypedBackgroundCompletion {
                        result_receiver,
                        callback: completion,
                    }),
                },
            );
            source.sender.clone()
        };
        let submitted = worker::submit(move || {
            let result = worker::execute(work);
            let _ = result_sender.send(result);
            let _ = completion_sender.send(WorkCompletion { id });
            let _ = wake.wake();
        });
        if submitted.is_err() {
            source.borrow_mut().in_flight.remove(&id);
        }
        submitted
    }

    pub fn poll(&self) -> Result<bool, TError> {
        self.source.get().map_or(Ok(false), poll_completions)
    }
}

thread_local! {
    static BACKGROUND_TASK_BUDGET: OnceCell<TaskBudget> = const { OnceCell::new() };
    static SOURCE_THREAD_COMPLETIONS: BackgroundTasks<TsonicError> =
        const { BackgroundTasks::new() };
}

fn background_task_budget() -> TaskBudget {
    BACKGROUND_TASK_BUDGET.with(|budget| {
        budget
            .get_or_init(|| {
                TaskBudget::new(
                    NonZeroUsize::new(MAX_PENDING_BACKGROUND_WORK)
                        .expect("finite native background task limit"),
                )
            })
            .clone()
    })
}

pub(crate) fn spawn<TResult>(
    work: impl FnOnce() -> crate::NodeResult<TResult> + Send + 'static,
    completion: impl FnOnce(crate::NodeResult<TResult>) -> Result<(), TsonicError> + 'static,
) -> crate::NodeResult<()>
where
    TResult: Send + 'static,
{
    SOURCE_THREAD_COMPLETIONS.with(|source| source.spawn(work, completion))
}

pub(crate) fn poll() -> Result<bool, TsonicError> {
    SOURCE_THREAD_COMPLETIONS.with(BackgroundTasks::poll)
}

pub(crate) fn has_pending_work() -> bool {
    SOURCE_THREAD_COMPLETIONS.with(BackgroundTasks::has_pending_work)
}

fn poll_completions<TError: From<TsonicError>>(
    source: &RefCell<SourceThreadCompletions<TError>>,
) -> Result<bool, TError> {
    let boundary = {
        let mut source = source.borrow_mut();
        loop {
            match source.receiver.try_recv() {
                Ok(value) => {
                    let next = source.next_ready_ticket.checked_add(1).ok_or_else(|| {
                        TError::from(TsonicError::from(crate::NodeError::new(
                            "ERR_NODE_BACKGROUND_WORK_LIMIT",
                            "background ready ticket range is exhausted",
                        )))
                    })?;
                    let callback = source.in_flight.remove(&value.id).ok_or_else(|| {
                        TError::from(TsonicError::from(crate::NodeError::new(
                            "ERR_NODE_BACKGROUND_RESULT",
                            "background work completed without its exact callback",
                        )))
                    })?;
                    let ticket = source.next_ready_ticket;
                    source.next_ready_ticket = next;
                    source.ready.push_back((ticket, callback));
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Err(TError::from(TsonicError::from(crate::NodeError::new(
                        "ERR_NODE_BACKGROUND_WORKER",
                        "background completion channel is unavailable",
                    ))));
                }
            }
        }
        source.ready.back().map(|(ticket, _)| *ticket)
    };
    let Some(boundary) = boundary else {
        return Ok(false);
    };
    let mut did_work = false;
    loop {
        let callback = {
            let mut source = source.borrow_mut();
            if source
                .ready
                .front()
                .is_some_and(|(ticket, _)| *ticket <= boundary)
            {
                source.ready.pop_front().map(|(_, callback)| callback)
            } else {
                None
            }
        };
        let Some(PendingCompletion {
            reservation,
            callback,
        }) = callback
        else {
            break;
        };
        drop(reservation);
        callback.complete()?;
        did_work = true;
    }
    Ok(did_work)
}

#[cfg(test)]
mod tests;
