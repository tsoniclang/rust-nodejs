use std::cell::{OnceCell, RefCell};
use std::time::Duration;

use crate::runtime_resources::{
    NativeResourceBudget, ResourceFrontier, RuntimeResource, RuntimeResources,
};
use crate::stream::{Readable, WeakReadable};
use crate::{NodeError, NodeResult};
use tsonic_rust_runtime::dispatch::{DispatchContexts, DispatchPhase};
use tsonic_rust_runtime::dispatch_queue::{TaskHandle, TaskTicket};

const MAXIMUM_STDIN_RESOURCES: usize = 1024;

thread_local! {
    static STDIN_BUDGET: NativeResourceBudget = const { NativeResourceBudget::new(MAXIMUM_STDIN_RESOURCES) };
}

pub struct StdinResources<E: 'static> {
    stream: RefCell<Option<Readable<E>>>,
    resources: OnceCell<RuntimeResources<StdinResource<E>>>,
}

impl<E: 'static> Default for StdinResources<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: 'static> StdinResources<E> {
    pub const fn new() -> Self {
        Self {
            stream: RefCell::new(None),
            resources: OnceCell::new(),
        }
    }
}

impl<E: From<NodeError> + 'static> StdinResources<E> {
    pub(crate) fn stream(
        &self,
        background: &crate::background::BackgroundTasks<E>,
        tasks: TaskHandle<E>,
    ) -> NodeResult<Readable<E>> {
        if let Some(stream) = self.stream.borrow().as_ref() {
            return Ok(stream.clone());
        }
        #[cfg(target_os = "linux")]
        {
            use std::io::Seek;
            let reservation = STDIN_BUDGET.with(NativeResourceBudget::reserve)?;
            let metadata = nix::sys::stat::fstat(std::io::stdin())
                .map_err(|error| NodeError::new("ERR_NODE_STDIN", error.to_string()))?;
            let kind = nix::sys::stat::SFlag::from_bits_truncate(metadata.st_mode)
                & nix::sys::stat::SFlag::S_IFMT;
            let stream = if kind == nix::sys::stat::SFlag::S_IFREG {
                let descriptor = nix::unistd::dup(std::io::stdin())
                    .map_err(|error| NodeError::new("ERR_NODE_STDIN", error.to_string()))?;
                let mut file = std::fs::File::from(descriptor);
                let start = file
                    .stream_position()
                    .map_err(|error| NodeError::new("ERR_NODE_STDIN", error.to_string()))?;
                let options = crate::fs::ReadStreamOptions {
                    start: Some(start),
                    ..Default::default()
                };
                let stream = crate::fs::ReadStream::from_file(
                    background,
                    tasks,
                    "stdin".to_owned(),
                    file,
                    &options,
                )?;
                let readable = stream.readable_handle();
                readable.mark_stdin(reservation);
                readable
            } else {
                let mut input = crate::readiness::Stdin::open()?;
                let readable = Readable::new(tasks);
                let ticket = reservation.ticket();
                input.retain_lease(reservation);
                readable.attach_stdin(input);
                self.resources
                    .get_or_init(|| RuntimeResources::new(MAXIMUM_STDIN_RESOURCES))
                    .register(
                        ticket,
                        StdinResource {
                            readable: readable.downgrade(),
                        },
                    );
                readable
            };
            self.stream.replace(Some(stream.clone()));
            Ok(stream)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (background, tasks);
            Err(NodeError::new(
                "ERR_NODE_STDIN_UNSUPPORTED",
                "native stdin readiness is currently supported on Linux only",
            ))
        }
    }
}

impl<E: From<NodeError> + 'static> DispatchContexts for StdinResources<E> {
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
        self.resources.get().is_some_and(RuntimeResources::has_work)
    }

    fn next_delay(&self) -> Option<Duration> {
        None
    }
}

struct StdinResource<E: 'static> {
    readable: WeakReadable<E>,
}

impl<E: 'static> Clone for StdinResource<E> {
    fn clone(&self) -> Self {
        Self {
            readable: self.readable.clone(),
        }
    }
}

impl<E: From<NodeError> + 'static> RuntimeResource for StdinResource<E> {
    type Error = E;

    fn is_alive(&self) -> bool {
        self.readable
            .upgrade()
            .is_some_and(|stream| !stream.destroyed() && !stream.is_ended())
    }

    fn phase(&self) -> DispatchPhase {
        DispatchPhase::RuntimeTasks
    }

    fn capture(&self) -> Result<Option<TaskTicket>, E> {
        if let Some(stream) = self.readable.upgrade() {
            stream.prepare_stdin()?;
        }
        Ok(None)
    }

    fn poll(&self, _boundary: Option<TaskTicket>) -> Result<bool, E> {
        self.readable
            .upgrade()
            .map_or(Ok(false), |stream| stream.poll_stdin())
    }

    fn has_work(&self) -> bool {
        self.readable
            .upgrade()
            .is_some_and(|stream| stream.wants_stdin_input())
    }

    fn next_delay(&self) -> Option<Duration> {
        None
    }
}
