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
    receiver: std::sync::mpsc::Receiver<notify::Result<notify::Event>>,
    previous: Option<Stats>,
    pending_stat_event: bool,
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
        match state.receiver.try_recv() {
            Ok(Ok(event)) => Ok(Some(FsWatchEvent {
                event_type: watch_event_type(&event.kind).to_string(),
                filename: event
                    .paths
                    .first()
                    .and_then(|path| path.file_name())
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_else(|| state.path.clone()),
            })),
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
            receiver,
            previous: Some(stats_for_watch_path(path)),
            pending_stat_event: false,
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
            .filter_map(|watcher| {
                let state = std::rc::Rc::clone(&watcher.state);
                let refed = state.borrow().refed;
                refed.then(|| (FsWatcher { state }, match &watcher.callback {
                    RuntimeWatcherCallback::Event(callback) => RuntimeWatcherCallback::Event(callback.clone()),
                    RuntimeWatcherCallback::Stat(callback) => RuntimeWatcherCallback::Stat(callback.clone()),
                }))
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

pub fn watch_file(path: &str) -> NodeResult<FsWatcher> {
    watch(path)
}

pub fn watch_file_with_options(path: &str, options: WatchFileOptions) -> NodeResult<FsWatcher> {
    let watcher = watch(path)?;
    {
        let mut state = watcher.state.borrow_mut();
        state.refed = options.persistent;
        state.stat_interval = Some(std::time::Duration::from_millis(options.interval_ms));
    }
    Ok(watcher)
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
        loop {
            match state.receiver.try_recv() {
                Ok(Ok(_)) => state.pending_stat_event = true,
                Ok(Err(error)) => return Err(NodeError::new("EIO", error.to_string())),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return Err(NodeError::new(
                    "ERR_WATCHER_CLOSED",
                    "filesystem notification channel is closed",
                )),
            }
        }
        let interval = state.stat_interval.unwrap_or_default();
        if !state.pending_stat_event || state.last_stat_check.elapsed() < interval {
            return Ok(None);
        }
        state.pending_stat_event = false;
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
