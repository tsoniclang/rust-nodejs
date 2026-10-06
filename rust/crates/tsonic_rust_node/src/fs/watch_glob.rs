pub fn glob_sync(pattern: &str) -> NodeResult<Vec<String>> {
    let mut matches = glob::glob(pattern)
        .map_err(|error| NodeError::new("ERR_INVALID_ARG_VALUE", error.to_string()))?
        .map(|entry| {
            entry
                .map(|path| path.to_string_lossy().to_string())
                .map_err(|error| NodeError::new("EIO", error.to_string()))
        })
        .collect::<NodeResult<Vec<_>>>()?;
    matches.sort();
    Ok(matches)
}

use std::sync::atomic::{AtomicBool, Ordering as WatchOrdering};
use std::sync::{mpsc, Arc};
use tsonic_rust_runtime::dispatch_queue::{TaskReservation, TaskTicket};
use tsonic_rust_runtime::Callable;

mod watchers;
pub use watchers::{with_default_watchers, Watchers};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsWatchEvent {
    pub event_type: String,
    pub filename: String,
}

struct WatchInput {
    receiver: mpsc::Receiver<notify::Result<notify::Event>>,
    overflowed: Arc<AtomicBool>,
}

enum WatchCallback<E> {
    Event(Callable<(String, String), Result<(), E>>),
    Stat(Callable<(Stats, Stats), Result<(), E>>),
}

struct FsWatcherState<E> {
    reservation: Option<TaskReservation>,
    path: String,
    watcher: Option<notify::RecommendedWatcher>,
    input: Option<WatchInput>,
    pending_events: VecDeque<(TaskTicket, FsWatchEvent)>,
    pending_notification: Option<(notify::Event, usize)>,
    pending_stat: Option<(TaskTicket, Stats, Stats)>,
    maximum_events: usize,
    previous: Option<Stats>,
    stat_interval: Option<std::time::Duration>,
    last_stat_check: std::time::Instant,
    callback: Option<WatchCallback<E>>,
    closed: bool,
    refed: bool,
}

impl<E> Drop for FsWatcherState<E> {
    fn drop(&mut self) {
        self.input.take();
        self.watcher.take();
    }
}

pub struct FsWatcher<E: 'static = tsonic_rust_runtime::TsonicError> {
    state: Rc<RefCell<FsWatcherState<E>>>,
}

impl<E: 'static> Clone for FsWatcher<E> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
}

impl<E: 'static> std::fmt::Debug for FsWatcher<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.borrow();
        formatter
            .debug_struct("FsWatcher")
            .field("path", &state.path)
            .field("closed", &state.closed)
            .field("refed", &state.refed)
            .finish()
    }
}

impl<E: 'static> FsWatcher<E> {
    pub fn poll(&self) -> NodeResult<Option<FsWatchEvent>> {
        let mut state = self.state.borrow_mut();
        if state.closed {
            return Err(NodeError::new("ERR_WATCHER_CLOSED", "watcher is closed"));
        }
        if state.input.is_none() {
            return Err(NodeError::new(
                "ERR_INVALID_ARG_TYPE",
                "stat watchers do not expose filesystem event polling",
            ));
        }
        if state.pending_events.is_empty() {
            state.ingest_events()?;
        }
        Ok(state.pending_events.pop_front().map(|(_, event)| event))
    }

    pub fn close(&self) {
        close_watcher_state(&self.state);
    }

    pub fn ref_(&self) -> &Self {
        self.state.borrow_mut().refed = true;
        self
    }

    pub fn unref(&self) -> &Self {
        self.state.borrow_mut().refed = false;
        self
    }

    pub fn has_ref(&self) -> bool {
        self.state.borrow().refed
    }

    pub fn closed(&self) -> bool {
        self.state.borrow().closed
    }
}

fn close_watcher_state<E>(owner: &RefCell<FsWatcherState<E>>) {
    let (input, watcher, reservation, callback) = {
        let mut state = owner.borrow_mut();
        state.closed = true;
        state.refed = false;
        state.pending_events.clear();
        state.pending_notification = None;
        state.pending_stat = None;
        (
            state.input.take(),
            state.watcher.take(),
            state.reservation.take(),
            state.callback.take(),
        )
    };
    drop(input);
    drop(watcher);
    drop(reservation);
    drop(callback);
}

impl<E> FsWatcherState<E> {
    fn ingest_events(&mut self) -> NodeResult<()> {
        let input = self.input.as_ref().ok_or_else(|| {
            NodeError::new(
                "ERR_INVALID_ARG_TYPE",
                "stat watchers do not expose filesystem event polling",
            )
        })?;
        if input.overflowed.swap(false, WatchOrdering::AcqRel) {
            return Err(watchers::queue_limit());
        }
        while self.pending_events.len() < self.maximum_events {
            if let Some((event, next_path)) = &mut self.pending_notification {
                let ticket = watchers::admit_event()?;
                let filename = watch_event_filename(&self.path, event.paths.get(*next_path));
                self.pending_events.push_back((
                    ticket,
                    FsWatchEvent {
                        event_type: watch_event_type(&event.kind).to_owned(),
                        filename,
                    },
                ));
                *next_path += 1;
                if *next_path >= event.paths.len().max(1) {
                    self.pending_notification = None;
                }
                continue;
            }
            match input.receiver.try_recv() {
                Ok(Ok(event)) => self.pending_notification = Some((event, 0)),
                Ok(Err(error)) => return Err(NodeError::new("EIO", error.to_string())),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(NodeError::new(
                        "ERR_WATCHER_CLOSED",
                        "filesystem notification channel is closed",
                    ))
                }
            }
        }
        Ok(())
    }

    fn capture_stat(&mut self) -> NodeResult<()> {
        if self.pending_stat.is_some() {
            return Ok(());
        }
        let interval = self.stat_interval.ok_or_else(|| {
            NodeError::new(
                "ERR_INVALID_ARG_TYPE",
                "filesystem event watchers do not expose stat polling",
            )
        })?;
        if self.last_stat_check.elapsed() < interval {
            return Ok(());
        }
        self.last_stat_check = std::time::Instant::now();
        let current = stats_for_watch_path(&self.path);
        let previous = self
            .previous
            .as_ref()
            .cloned()
            .unwrap_or_else(empty_watch_stats);
        if current != previous {
            let ticket = watchers::admit_event()?;
            self.previous = Some(current.clone());
            self.pending_stat = Some((ticket, current, previous));
        }
        Ok(())
    }
}

pub fn watch<E: From<NodeError> + 'static>(
    roots: &Watchers<E>,
    path: &str,
) -> NodeResult<FsWatcher<E>> {
    watch_with_options(roots, path, WatchOptions::default())
}

pub fn watch_with_options<E: From<NodeError> + 'static>(
    roots: &Watchers<E>,
    path: &str,
    options: WatchOptions,
) -> NodeResult<FsWatcher<E>> {
    if options.signal_aborted {
        return Err(NodeError::new("ABORT_ERR", "watch was aborted"));
    }
    if options.max_queue == 0 || options.max_queue > watchers::MAXIMUM_PENDING_EVENTS {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            "watch queue must fit the finite native event budget",
        ));
    }
    use notify::Watcher as _;
    let reservation = watchers::reserve_resource()?;
    let (sender, receiver) = mpsc::sync_channel(options.max_queue);
    let overflowed = Arc::new(AtomicBool::new(false));
    let native_overflow = Arc::clone(&overflowed);
    let wake = crate::readiness::waker()?;
    let mut watcher = notify::recommended_watcher(move |event| {
        if let Err(mpsc::TrySendError::Full(_)) = sender.try_send(event) {
            native_overflow.store(true, WatchOrdering::Release);
        }
        let _ = wake.wake();
    })
    .map_err(|error| NodeError::new("EIO", error.to_string()))?;
    watcher
        .watch(
            std::path::Path::new(path),
            if options.recursive {
                notify::RecursiveMode::Recursive
            } else {
                notify::RecursiveMode::NonRecursive
            },
        )
        .map_err(|error| NodeError::new("EIO", error.to_string()))?;
    let value = FsWatcher {
        state: Rc::new(RefCell::new(FsWatcherState {
            reservation: Some(reservation),
            path: path.to_owned(),
            watcher: Some(watcher),
            input: Some(WatchInput {
                receiver,
                overflowed,
            }),
            pending_events: VecDeque::new(),
            pending_notification: None,
            pending_stat: None,
            maximum_events: options.max_queue,
            previous: Some(stats_for_watch_path(path)),
            stat_interval: None,
            last_stat_check: std::time::Instant::now(),
            callback: None,
            closed: false,
            refed: options.persistent,
        })),
    };
    roots.register_handle(&value);
    Ok(value)
}

pub fn watch_callable<E: From<NodeError> + 'static>(
    roots: &Watchers<E>,
    path: &str,
    callback: Callable<(String, String), Result<(), E>>,
) -> NodeResult<FsWatcher<E>> {
    let watcher = watch(roots, path)?;
    watcher.state.borrow_mut().callback = Some(WatchCallback::Event(callback));
    Ok(watcher)
}

fn watch_event_type(kind: &notify::EventKind) -> &'static str {
    match kind {
        notify::EventKind::Create(_) | notify::EventKind::Remove(_) => "rename",
        notify::EventKind::Modify(notify::event::ModifyKind::Name(_)) => "rename",
        _ => "change",
    }
}

fn watch_event_filename(path: &str, event_path: Option<&std::path::PathBuf>) -> String {
    let watched_path = std::path::Path::new(path);
    event_path
        .and_then(|event_path| {
            event_path
                .strip_prefix(watched_path)
                .ok()
                .filter(|relative| !relative.as_os_str().is_empty())
                .map(|relative| relative.to_string_lossy().into_owned())
                .or_else(|| {
                    event_path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
        })
        .unwrap_or_else(|| path.to_owned())
}

pub fn watch_file<E: From<NodeError> + 'static>(
    roots: &Watchers<E>,
    path: &str,
) -> NodeResult<FsWatcher<E>> {
    watch_file_with_options(roots, path, WatchFileOptions::default())
}

pub fn watch_file_with_options<E: From<NodeError> + 'static>(
    roots: &Watchers<E>,
    path: &str,
    options: WatchFileOptions,
) -> NodeResult<FsWatcher<E>> {
    if options.interval_ms == 0 {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            "watchFile interval must be greater than zero",
        ));
    }
    let value = FsWatcher {
        state: Rc::new(RefCell::new(FsWatcherState {
            reservation: Some(watchers::reserve_resource()?),
            path: path.to_owned(),
            watcher: None,
            input: None,
            pending_events: VecDeque::new(),
            pending_notification: None,
            pending_stat: None,
            maximum_events: watchers::MAXIMUM_PENDING_EVENTS,
            previous: Some(stats_for_watch_path(path)),
            stat_interval: Some(std::time::Duration::from_millis(options.interval_ms)),
            last_stat_check: std::time::Instant::now(),
            callback: None,
            closed: false,
            refed: options.persistent,
        })),
    };
    roots.register_handle(&value);
    Ok(value)
}

pub type StatWatcher<E = tsonic_rust_runtime::TsonicError> = FsWatcher<E>;

pub fn watch_file_callable<E: From<NodeError> + 'static>(
    roots: &Watchers<E>,
    path: &str,
    callback: Callable<(Stats, Stats), Result<(), E>>,
) -> NodeResult<()> {
    watch_file_options_callable(roots, path, WatchFileOptions::default(), callback)
}

pub fn watch_file_options_callable<E: From<NodeError> + 'static>(
    roots: &Watchers<E>,
    path: &str,
    options: WatchFileOptions,
    callback: Callable<(Stats, Stats), Result<(), E>>,
) -> NodeResult<()> {
    let watcher = watch_file_with_options(roots, path, options)?;
    watcher.state.borrow_mut().callback = Some(WatchCallback::Stat(callback));
    roots.retain_stat(&watcher);
    Ok(())
}

pub fn unwatch_file(path: &str) {
    watchers::unwatch_file(path);
}

fn stats_for_watch_path(path: &str) -> Stats {
    fs::metadata(path)
        .map(|metadata| stats_from_metadata(&metadata))
        .unwrap_or_else(|_| empty_watch_stats())
}

fn empty_watch_stats() -> Stats {
    Stats {
        size: 0,
        dev: 0,
        ino: 0,
        mode: 0,
        nlink: 0,
        uid: 0,
        gid: 0,
        rdev: 0,
        blksize: 0,
        blocks: 0,
        atime_ms: 0.0,
        mtime_ms: 0.0,
        ctime_ms: 0.0,
        birthtime_ms: 0.0,
        is_file: false,
        is_directory: false,
        is_symbolic_link: false,
        is_block_device: false,
        is_character_device: false,
        is_fifo: false,
        is_socket: false,
    }
}

#[cfg(not(unix))]
static NEXT_FD: AtomicI32 = AtomicI32::new(10);
static FILE_TABLE: OnceLock<Mutex<HashMap<i32, File>>> = OnceLock::new();
