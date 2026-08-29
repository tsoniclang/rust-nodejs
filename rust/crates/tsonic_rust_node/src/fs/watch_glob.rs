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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsWatchEvent {
    pub event_type: String,
    pub filename: String,
}

struct FsWatcherState {
    path: String,
    watcher: Option<notify::RecommendedWatcher>,
    receiver: Option<std::sync::mpsc::Receiver<notify::Result<notify::Event>>>,
    pending_events: std::collections::VecDeque<FsWatchEvent>,
    previous: Option<Stats>,
    stat_interval: Option<std::time::Duration>,
    last_stat_check: std::time::Instant,
    closed: bool,
    refed: bool,
}

#[derive(Clone)]
pub struct FsWatcher {
    state: std::rc::Rc<std::cell::RefCell<FsWatcherState>>,
}

impl std::fmt::Debug for FsWatcher {
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

impl FsWatcher {
    pub fn poll(&mut self) -> NodeResult<Option<FsWatchEvent>> {
        let mut state = self.state.borrow_mut();
        if state.closed {
            return Err(NodeError::new("ERR_WATCHER_CLOSED", "watcher is closed"));
        }
        if let Some(event) = state.pending_events.pop_front() {
            return Ok(Some(event));
        }
        let received = state
            .receiver
            .as_ref()
            .ok_or_else(|| NodeError::new(
                "ERR_INVALID_ARG_TYPE",
                "stat watchers do not expose filesystem event polling",
            ))?
            .try_recv();
        match received {
            Ok(Ok(event)) => {
                let event_type = watch_event_type(&event.kind).to_string();
                let filenames = watch_event_filenames(&state.path, &event.paths);
                state.pending_events.extend(filenames.into_iter().map(|filename| FsWatchEvent {
                    event_type: event_type.clone(),
                    filename,
                }));
                Ok(state.pending_events.pop_front())
            }
            Ok(Err(error)) => Err(NodeError::new("EIO", error.to_string())),
            Err(std::sync::mpsc::TryRecvError::Empty) => Ok(None),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(NodeError::new(
                "ERR_WATCHER_CLOSED",
                "filesystem notification channel is closed",
            )),
        }
    }

    pub fn close(&mut self) {
        let mut state = self.state.borrow_mut();
        state.closed = true;
        state.watcher = None;
    }

    pub fn ref_(&mut self) -> &mut Self {
        self.state.borrow_mut().refed = true;
        self
    }

    pub fn unref(&mut self) -> &mut Self {
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

pub fn watch(path: &str) -> NodeResult<FsWatcher> {
    watch_with_options(path, WatchOptions::default())
}

pub fn watch_with_options(path: &str, options: WatchOptions) -> NodeResult<FsWatcher> {
    if options.signal_aborted {
        return Err(NodeError::new("ABORT_ERR", "watch was aborted"));
    }
    use notify::Watcher;

    let (sender, receiver) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = sender.send(event);
    }).map_err(|error| NodeError::new("EIO", error.to_string()))?;
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
    Ok(FsWatcher {
        state: std::rc::Rc::new(std::cell::RefCell::new(FsWatcherState {
            path: path.to_string(),
            watcher: Some(watcher),
            receiver: Some(receiver),
            pending_events: std::collections::VecDeque::new(),
            previous: Some(stats_for_watch_path(path)),
            stat_interval: None,
            last_stat_check: std::time::Instant::now(),
            closed: false,
            refed: options.persistent,
        })),
    })
}

type RuntimeWatchCallback = tsonic_rust_runtime::Callable<
    (String, String),
    tsonic_rust_runtime::TsonicResult<()>,
>;
type RuntimeStatWatchCallback = tsonic_rust_runtime::Callable<
    (Stats, Stats),
    tsonic_rust_runtime::TsonicResult<()>,
>;

enum RuntimeWatcherCallback {
    Event(RuntimeWatchCallback),
    Stat(RuntimeStatWatchCallback),
}

struct RuntimeWatcher {
    state: std::rc::Rc<std::cell::RefCell<FsWatcherState>>,
    callback: RuntimeWatcherCallback,
}

thread_local! {
    static RUNTIME_WATCHERS: std::cell::RefCell<std::collections::BTreeMap<u64, RuntimeWatcher>> =
        std::cell::RefCell::new(std::collections::BTreeMap::new());
}

static NEXT_RUNTIME_WATCHER_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

pub fn watch_callable<E>(
    path: &str,
    callback: tsonic_rust_runtime::Callable<(String, String), Result<(), E>>,
) -> NodeResult<FsWatcher>
where
    E: std::fmt::Display + 'static,
{
    let watcher = watch(path)?;
    let callback = tsonic_rust_runtime::Callable::new(move |arguments| {
        callback
            .call(arguments)
            .map_err(crate::error::callback_runtime_error)
    });
    let id = NEXT_RUNTIME_WATCHER_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    RUNTIME_WATCHERS.with(|watchers| {
        watchers.borrow_mut().insert(id, RuntimeWatcher {
            state: std::rc::Rc::clone(&watcher.state),
            callback: RuntimeWatcherCallback::Event(callback),
        });
    });
    Ok(watcher)
}

pub(crate) fn has_refed_runtime_watchers() -> bool {
    RUNTIME_WATCHERS.with(|watchers| {
        watchers.borrow().values().any(|watcher| {
            let state = watcher.state.borrow();
            state.refed && !state.closed
        })
    })
}

pub(crate) fn poll_runtime_watchers() -> tsonic_rust_runtime::TsonicResult<bool> {
    let active = RUNTIME_WATCHERS.with(|watchers| {
        let mut watchers = watchers.borrow_mut();
        watchers.retain(|_, watcher| !watcher.state.borrow().closed);
        watchers
            .values()
            .map(|watcher| {
                let state = std::rc::Rc::clone(&watcher.state);
                (FsWatcher { state }, match &watcher.callback {
                    RuntimeWatcherCallback::Event(callback) => RuntimeWatcherCallback::Event(callback.clone()),
                    RuntimeWatcherCallback::Stat(callback) => RuntimeWatcherCallback::Stat(callback.clone()),
                })
            })
            .collect::<Vec<_>>()
    });
    let mut did_work = false;
    for (mut watcher, callback) in active {
        match callback {
            RuntimeWatcherCallback::Event(callback) => {
                while let Some(event) = watcher.poll().map_err(tsonic_rust_runtime::TsonicError::from)? {
                    did_work = true;
                    callback.call((event.event_type, event.filename))?;
                }
            }
            RuntimeWatcherCallback::Stat(callback) => {
                if let Some((current, previous)) = watcher
                    .poll_stat_change()
                    .map_err(tsonic_rust_runtime::TsonicError::from)?
                {
                    did_work = true;
                    callback.call((current, previous))?;
                }
            }
        }
    }
    Ok(did_work)
}

fn watch_event_type(kind: &notify::EventKind) -> &'static str {
    match kind {
        notify::EventKind::Create(_) | notify::EventKind::Remove(_) => "rename",
        notify::EventKind::Modify(notify::event::ModifyKind::Name(_)) => "rename",
        _ => "change",
    }
}

fn watch_event_filenames(path: &str, event_paths: &[std::path::PathBuf]) -> Vec<String> {
    if event_paths.is_empty() {
        return vec![path.to_string()];
    }
    let watched_path = std::path::Path::new(path);
    event_paths
        .iter()
        .map(|event_path| {
            event_path
                .strip_prefix(watched_path)
                .ok()
                .filter(|relative| !relative.as_os_str().is_empty())
                .map(|relative| relative.to_string_lossy().to_string())
                .or_else(|| {
                    event_path
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                })
                .unwrap_or_else(|| path.to_string())
        })
        .collect()
}

pub fn watch_file(path: &str) -> NodeResult<FsWatcher> {
    watch(path)
}

pub fn watch_file_with_options(path: &str, options: WatchFileOptions) -> NodeResult<FsWatcher> {
    if options.interval_ms == 0 {
        return Err(NodeError::new(
            "ERR_OUT_OF_RANGE",
            "watchFile interval must be greater than zero",
        ));
    }
    Ok(FsWatcher {
        state: std::rc::Rc::new(std::cell::RefCell::new(FsWatcherState {
            path: path.to_string(),
            watcher: None,
            receiver: None,
            pending_events: std::collections::VecDeque::new(),
            previous: Some(stats_for_watch_path(path)),
            stat_interval: Some(std::time::Duration::from_millis(options.interval_ms)),
            last_stat_check: std::time::Instant::now(),
            closed: false,
            refed: options.persistent,
        })),
    })
}

pub type StatWatcher = FsWatcher;

pub fn watch_file_callable<E>(
    path: &str,
    callback: tsonic_rust_runtime::Callable<(Stats, Stats), Result<(), E>>,
) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    watch_file_options_callable(path, WatchFileOptions::default(), callback)
}

pub fn watch_file_options_callable<E>(
    path: &str,
    options: WatchFileOptions,
    callback: tsonic_rust_runtime::Callable<(Stats, Stats), Result<(), E>>,
) -> NodeResult<()>
where
    E: std::fmt::Display + 'static,
{
    let watcher = watch_file_with_options(path, options)?;
    let callback = tsonic_rust_runtime::Callable::new(move |arguments| {
        callback
            .call(arguments)
            .map_err(crate::error::callback_runtime_error)
    });
    let id = NEXT_RUNTIME_WATCHER_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    RUNTIME_WATCHERS.with(|watchers| {
        watchers.borrow_mut().insert(id, RuntimeWatcher {
            state: std::rc::Rc::clone(&watcher.state),
            callback: RuntimeWatcherCallback::Stat(callback),
        });
    });
    Ok(())
}

pub fn unwatch_file(path: &str) {
    RUNTIME_WATCHERS.with(|watchers| {
        watchers.borrow_mut().retain(|_, watcher| {
            if watcher.state.borrow().path == path &&
                matches!(&watcher.callback, RuntimeWatcherCallback::Stat(_))
            {
                watcher.state.borrow_mut().closed = true;
                false
            } else {
                true
            }
        });
    });
}

impl FsWatcher {
    fn poll_stat_change(&mut self) -> NodeResult<Option<(Stats, Stats)>> {
        let mut state = self.state.borrow_mut();
        if state.closed {
            return Err(NodeError::new("ERR_WATCHER_CLOSED", "watcher is closed"));
        }
        let interval = state.stat_interval.ok_or_else(|| NodeError::new(
            "ERR_INVALID_ARG_TYPE",
            "filesystem event watchers do not expose stat polling",
        ))?;
        if state.last_stat_check.elapsed() < interval {
            return Ok(None);
        }
        state.last_stat_check = std::time::Instant::now();
        let current = stats_for_watch_path(&state.path);
        let previous = state.previous.clone().unwrap_or_else(empty_watch_stats);
        state.previous = Some(current.clone());
        Ok((current != previous).then_some((current, previous)))
    }
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

static NEXT_FD: AtomicI32 = AtomicI32::new(10);
static FILE_TABLE: OnceLock<Mutex<HashMap<i32, File>>> = OnceLock::new();
