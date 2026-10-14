use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::num::NonZeroUsize;
use std::rc::{Rc, Weak};
use std::sync::mpsc::{Receiver, SyncSender};

use tsonic_rust_runtime::dispatch::{DispatchGeneration, DispatchGenerationId, DispatchOwner};
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
    TError: From<crate::NodeError>,
{
    fn complete(self: Box<Self>) -> Result<(), TError> {
        let Self {
            result_receiver,
            callback,
        } = *self;
        let result = result_receiver.recv().map_err(|_| {
            TError::from(crate::NodeError::new(
                "ERR_NODE_BACKGROUND_RESULT",
                "background work completed without its exact typed result",
            ))
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
    generation: DispatchGeneration,
    channel: Option<(SyncSender<WorkCompletion>, Receiver<WorkCompletion>)>,
    in_flight: BTreeMap<TaskTicket, PendingCompletion<TError>>,
    ready: VecDeque<(u64, PendingCompletion<TError>)>,
}

impl<TError> SourceThreadCompletions<TError> {
    fn new() -> Self {
        Self {
            generation: DispatchGeneration::new_closed(),
            channel: None,
            in_flight: BTreeMap::new(),
            ready: VecDeque::new(),
        }
    }

    fn pending(&self) -> usize {
        self.in_flight.len() + self.ready.len()
    }

    fn completion_sender(&mut self) -> SyncSender<WorkCompletion> {
        self.channel
            .get_or_insert_with(|| std::sync::mpsc::sync_channel(MAX_PENDING_BACKGROUND_WORK))
            .0
            .clone()
    }
}

pub struct BackgroundTasks<TError> {
    source: OnceCell<Rc<RefCell<SourceThreadCompletions<TError>>>>,
    owner: Cell<Option<DispatchOwner>>,
}

pub struct BackgroundHandle<TError> {
    source: Weak<RefCell<SourceThreadCompletions<TError>>>,
    generation: Option<DispatchGenerationId>,
}

impl<TError> Clone for BackgroundHandle<TError> {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            generation: self.generation,
        }
    }
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
            owner: Cell::new(None),
        }
    }

    pub fn has_pending_work(&self) -> bool {
        self.source
            .get()
            .is_some_and(|source| source.borrow().pending() != 0)
    }

    pub fn handle(&self) -> BackgroundHandle<TError> {
        let source = self.scheduling_source();
        BackgroundHandle {
            source: Rc::downgrade(source),
            generation: source.borrow().generation.capture(),
        }
    }

    fn scheduling_source(&self) -> &Rc<RefCell<SourceThreadCompletions<TError>>> {
        self.source.get_or_init(|| {
            let source = SourceThreadCompletions::new();
            if let Some(owner) = self.owner.get() {
                assert!(
                    source.generation.enter(owner),
                    "a fresh native completion source accepts its owner"
                );
            }
            Rc::new(RefCell::new(source))
        })
    }
}

impl<TError: From<crate::NodeError>> BackgroundTasks<TError> {
    pub fn spawn<TResult>(
        &self,
        work: impl FnOnce() -> crate::NodeResult<TResult> + Send + 'static,
        completion: impl FnOnce(crate::NodeResult<TResult>) -> Result<(), TError> + 'static,
    ) -> crate::NodeResult<()>
    where
        TResult: Send + 'static,
    {
        let reservation = reserve_background_task()?;
        spawn_on_source(self.scheduling_source(), reservation, work, completion)
    }

    pub fn poll(&self) -> Result<bool, TError> {
        self.source
            .get()
            .map_or(Ok(false), |source| poll_completions(source))
    }
}

impl<TError: From<crate::NodeError>> BackgroundHandle<TError> {
    pub fn spawn<TResult>(
        &self,
        work: impl FnOnce() -> crate::NodeResult<TResult> + Send + 'static,
        completion: impl FnOnce(crate::NodeResult<TResult>) -> Result<(), TError> + 'static,
    ) -> crate::NodeResult<()>
    where
        TResult: Send + 'static,
    {
        let source = self.source.upgrade().ok_or_else(|| {
            crate::NodeError::new(
                "ERR_NODE_BACKGROUND_CLOSED",
                "background callback owner has been released",
            )
        })?;
        if !source.borrow().generation.accepts(self.generation) {
            return Err(crate::NodeError::new(
                "ERR_NODE_BACKGROUND_CLOSED",
                "background callback owner has been released",
            ));
        }
        let reservation = reserve_background_task()?;
        spawn_on_source(&source, reservation, work, completion)
    }
}

fn reserve_background_task() -> crate::NodeResult<TaskReservation> {
    tsonic_rust_js::event_loop::ensure_running().map_err(crate::NodeError::from)?;
    background_task_budget()
        .reserve()
        .map_err(|error| crate::NodeError::new("ERR_NODE_BACKGROUND_WORK_LIMIT", error.to_string()))
}

fn spawn_on_source<TResult, TError: From<crate::NodeError>>(
    source: &RefCell<SourceThreadCompletions<TError>>,
    reservation: TaskReservation,
    work: impl FnOnce() -> crate::NodeResult<TResult> + Send + 'static,
    completion: impl FnOnce(crate::NodeResult<TResult>) -> Result<(), TError> + 'static,
) -> crate::NodeResult<()>
where
    TResult: Send + 'static,
{
    let wake = tsonic_rust_js::event_loop::dispatch_handle().map_err(crate::NodeError::from)?;
    let owner = tsonic_rust_js::event_loop::dispatch_owner().map_err(crate::NodeError::from)?;
    let id = reservation.ticket();
    let (result_sender, result_receiver) = std::sync::mpsc::sync_channel(1);
    let completion_sender = {
        let mut source = source.borrow_mut();
        if !source.generation.owns(owner) {
            return Err(crate::NodeError::new(
                "ERR_NODE_BACKGROUND_CLOSED",
                "background callback owner has been released",
            ));
        }
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
        source.completion_sender()
    };
    let submitted = worker::submit(move || {
        let result = worker::execute(work);
        let _ = result_sender.send(result);
        let _ = completion_sender.send(WorkCompletion { id });
        let _ = wake.wake();
    });
    if submitted.is_err() {
        let removed = source.borrow_mut().in_flight.remove(&id);
        drop(removed);
    }
    submitted
}

thread_local! {
    static NEXT_COMPLETION_TICKET: Cell<u64> = const { Cell::new(0) };
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

#[cfg(test)]
pub(crate) fn poll() -> Result<bool, TsonicError> {
    SOURCE_THREAD_COMPLETIONS.with(BackgroundTasks::poll)
}

pub(crate) fn has_pending_work() -> bool {
    SOURCE_THREAD_COMPLETIONS.with(BackgroundTasks::has_pending_work)
}

fn poll_completions<TError: From<crate::NodeError>>(
    source: &RefCell<SourceThreadCompletions<TError>>,
) -> Result<bool, TError> {
    let boundary = publish_completions(source).map_err(TError::from)?;
    let Some(boundary) = boundary else {
        return Ok(false);
    };
    let mut did_work = false;
    loop {
        if !poll_next_completion(source, boundary)? {
            break;
        }
        did_work = true;
    }
    Ok(did_work)
}

fn poll_next_completion<TError: From<crate::NodeError>>(
    source: &RefCell<SourceThreadCompletions<TError>>,
    boundary: u64,
) -> Result<bool, TError> {
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
        return Ok(false);
    };
    drop(reservation);
    callback.complete()?;
    Ok(true)
}

fn publish_completions<TError>(
    source: &RefCell<SourceThreadCompletions<TError>>,
) -> crate::NodeResult<Option<u64>> {
    let mut source = source.borrow_mut();
    loop {
        let Some((_, receiver)) = &source.channel else {
            break;
        };
        match receiver.try_recv() {
            Ok(value) => {
                let ticket = NEXT_COMPLETION_TICKET.with(|sequence| {
                    let ticket = sequence.get();
                    let next = ticket.checked_add(1).ok_or_else(|| {
                        crate::NodeError::new(
                            "ERR_NODE_BACKGROUND_WORK_LIMIT",
                            "background ready ticket range is exhausted",
                        )
                    })?;
                    sequence.set(next);
                    Ok::<_, crate::NodeError>(ticket)
                })?;
                let callback = source.in_flight.remove(&value.id).ok_or_else(|| {
                    crate::NodeError::new(
                        "ERR_NODE_BACKGROUND_RESULT",
                        "background work completed without its exact callback",
                    )
                })?;
                source.ready.push_back((ticket, callback));
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                return Err(crate::NodeError::new(
                    "ERR_NODE_BACKGROUND_WORKER",
                    "background completion channel is unavailable",
                ));
            }
        }
    }
    Ok(source.ready.back().map(|(ticket, _)| *ticket))
}

impl<TError: From<crate::NodeError>> tsonic_rust_runtime::dispatch::DispatchContexts
    for BackgroundTasks<TError>
{
    type Error = TError;
    type Frontier = Option<u64>;

    fn enter(&self, owner: DispatchOwner) -> bool {
        if !owner.claim(&self.owner) {
            return false;
        }
        self.source
            .get()
            .is_none_or(|source| source.borrow().generation.enter(owner))
    }

    fn close(&self, owner: DispatchOwner) {
        if !owner.release(&self.owner) {
            return;
        }
        if let Some(source) = self.source.get() {
            let retired = {
                let mut source = source.borrow_mut();
                if !source.generation.close(owner) {
                    return;
                }
                (
                    source.channel.take(),
                    std::mem::take(&mut source.in_flight),
                    std::mem::take(&mut source.ready),
                )
            };
            drop(retired);
        }
    }

    fn prepare(
        &self,
        phase: tsonic_rust_runtime::dispatch::DispatchPhase,
    ) -> Result<Self::Frontier, TError> {
        if phase != crate::phases::BACKGROUND {
            return Ok(None);
        }
        self.source.get().map_or(Ok(None), |source| {
            publish_completions(source).map_err(TError::from)
        })
    }

    fn next_ready(&self, frontier: &Self::Frontier) -> Option<u64> {
        let boundary = (*frontier)?;
        self.source.get().and_then(|source| {
            source
                .borrow()
                .ready
                .front()
                .and_then(|(ticket, _)| (*ticket <= boundary).then_some(*ticket))
        })
    }

    fn poll_next(&self, frontier: &Self::Frontier) -> Result<bool, TError> {
        match (self.source.get(), frontier) {
            (Some(source), Some(boundary)) => poll_next_completion(source, *boundary),
            _ => Ok(false),
        }
    }

    fn has_work(&self) -> bool {
        self.has_pending_work()
    }

    fn next_delay(&self) -> Option<std::time::Duration> {
        None
    }
}

pub(crate) fn with_default<TValue>(
    callback: impl FnOnce(&BackgroundTasks<TsonicError>) -> TValue,
) -> TValue {
    SOURCE_THREAD_COMPLETIONS.with(callback)
}

#[cfg(test)]
mod tests;
