//! Watches subscribed files or directories and routes coarse-grained change
//! notifications to the subscribers that own matching watched paths.

use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use notify::Event;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use tokio::runtime::Handle;
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio::time::sleep_until;
use tracing::warn;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Coalesced file change notification for a subscriber.
pub struct FileWatcherEvent {
    /// Changed paths delivered in sorted order with duplicates removed.
    pub paths: Vec<PathBuf>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
/// Path subscription registered by a [`FileWatcherSubscriber`].
pub struct WatchPath {
    /// Root path to watch.
    pub path: PathBuf,
    /// Whether events below `path` should match recursively.
    pub recursive: bool,
}

type SubscriberId = u64;

#[derive(Default)]
struct WatchState {
    next_subscriber_id: SubscriberId,
    path_ref_counts: HashMap<PathBuf, PathWatchCounts>,
    subscribers: HashMap<SubscriberId, SubscriberState>,
}

struct SubscriberState {
    watched_paths: HashMap<SubscriberWatch, SubscriberWatchState>,
    tx: WatchSender,
}

/// Per-subscriber watch paths split by the role each path serves.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct SubscriberWatch {
    /// Original path requested by the subscriber. Notifications are reported
    /// in this namespace so clients do not see canonicalization artifacts.
    requested: WatchPath,
    /// Canonical equivalent of `requested` used to match backend events.
    /// Some backends report canonical paths such as `/private/var/...` even
    /// when the watch was registered through `/var/...`.
    matched: WatchPath,
    /// Existing path passed to the OS watcher and used for ref-counting. This
    /// is usually `requested`, but missing targets use an existing ancestor.
    actual: WatchPath,
}

struct SubscriberWatchState {
    count: usize,
    last_exists: bool,
}

/// Receives coalesced change notifications for a single subscriber.
pub struct Receiver {
    inner: Arc<ReceiverInner>,
}

struct WatchSender {
    inner: Arc<ReceiverInner>,
}

struct ReceiverInner {
    changed_paths: AsyncMutex<BTreeSet<PathBuf>>,
    notify: Notify,
    sender_count: AtomicUsize,
}

impl Receiver {
    /// Waits for the next batch of changed paths, or returns `None` once the
    /// corresponding subscriber has been removed and no more events can arrive.
    pub async fn recv(&mut self) -> Option<FileWatcherEvent> {
        loop {
            let notified = self.inner.notify.notified();
            {
                let mut changed_paths = self.inner.changed_paths.lock().await;
                if !changed_paths.is_empty() {
                    return Some(FileWatcherEvent {
                        paths: std::mem::take(&mut *changed_paths).into_iter().collect(),
                    });
                }
                if self.inner.sender_count.load(Ordering::Acquire) == 0 {
                    return None;
                }
            }
            notified.await;
        }
    }
}

impl WatchSender {
    async fn add_changed_paths(&self, paths: &[PathBuf]) {
        if paths.is_empty() {
            return;
        }

        let mut changed_paths = self.inner.changed_paths.lock().await;
        let previous_len = changed_paths.len();
        changed_paths.extend(paths.iter().cloned());
        if changed_paths.len() != previous_len {
            self.inner.notify.notify_one();
        }
    }
}

impl Clone for WatchSender {
    fn clone(&self) -> Self {
        self.inner.sender_count.fetch_add(1, Ordering::Relaxed);
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Drop for WatchSender {
    fn drop(&mut self) {
        if self.inner.sender_count.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.inner.notify.notify_waiters();
        }
    }
}

fn watch_channel() -> (WatchSender, Receiver) {
    let inner = Arc::new(ReceiverInner {
        changed_paths: AsyncMutex::new(BTreeSet::new()),
        notify: Notify::new(),
        sender_count: AtomicUsize::new(1),
    });
    (
        WatchSender {
            inner: Arc::clone(&inner),
        },
        Receiver { inner },
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PathWatchCounts {
    non_recursive: usize,
    recursive: usize,
}

impl PathWatchCounts {
    fn increment(&mut self, recursive: bool, amount: usize) {
        if recursive {
            self.recursive += amount;
        } else {
            self.non_recursive += amount;
        }
    }

    fn decrement(&mut self, recursive: bool, amount: usize) {
        if recursive {
            self.recursive = self.recursive.saturating_sub(amount);
        } else {
            self.non_recursive = self.non_recursive.saturating_sub(amount);
        }
    }

    fn effective_mode(self) -> Option<RecursiveMode> {
        if self.recursive > 0 {
            Some(RecursiveMode::Recursive)
        } else if self.non_recursive > 0 {
            Some(RecursiveMode::NonRecursive)
        } else {
            None
        }
    }

    fn is_empty(self) -> bool {
        self.non_recursive == 0 && self.recursive == 0
    }
}

struct FileWatcherInner {
    watcher: RecommendedWatcher,
    watched_paths: HashMap<PathBuf, RecursiveMode>,
}

/// Coalesces bursts of watch notifications and emits at most once per interval.
pub struct ThrottledWatchReceiver {
    rx: Receiver,
    interval: Duration,
    next_allowed: Option<Instant>,
}

impl ThrottledWatchReceiver {
    /// Creates a throttling wrapper around a raw watcher [`Receiver`].
    pub fn new(rx: Receiver, interval: Duration) -> Self {
        Self {
            rx,
            interval,
            next_allowed: None,
        }
    }

    /// Receives the next event, enforcing the configured minimum delay after
    /// the previous emission.
    pub async fn recv(&mut self) -> Option<FileWatcherEvent> {
        if let Some(next_allowed) = self.next_allowed {
            sleep_until(next_allowed).await;
        }

        let event = self.rx.recv().await;
        if event.is_some() {
            self.next_allowed = Some(Instant::now() + self.interval);
        }
        event
    }
}

/// Handle used to register watched paths for one logical consumer.
pub struct FileWatcherSubscriber {
    id: SubscriberId,
    file_watcher: Arc<FileWatcher>,
}

impl FileWatcherSubscriber {
    /// Registers the provided paths for this subscriber and returns an RAII
    /// guard that unregisters them on drop.
    pub fn register_paths(&self, watched_paths: Vec<WatchPath>) -> WatchRegistration {
        let watched_paths = dedupe_watched_paths(watched_paths)
            .into_iter()
            .map(|requested| {
                let (actual, matched) = actual_watch_path(&requested);
                SubscriberWatch {
                    requested,
                    matched,
                    actual,
                }
            })
            .collect::<Vec<_>>();
        self.file_watcher.register_paths(self.id, &watched_paths);

        WatchRegistration {
            file_watcher: Arc::downgrade(&self.file_watcher),
            subscriber_id: self.id,
            watched_paths,
        }
    }

    #[cfg(test)]
    pub(crate) fn register_path(&self, path: PathBuf, recursive: bool) -> WatchRegistration {
        self.register_paths(vec![WatchPath { path, recursive }])
    }
}

impl Drop for FileWatcherSubscriber {
    fn drop(&mut self) {
        self.file_watcher.remove_subscriber(self.id);
    }
}

/// RAII guard for a set of active path registrations.
pub struct WatchRegistration {
    file_watcher: std::sync::Weak<FileWatcher>,
    subscriber_id: SubscriberId,
    watched_paths: Vec<SubscriberWatch>,
}

impl Default for WatchRegistration {
    fn default() -> Self {
        Self {
            file_watcher: std::sync::Weak::new(),
            subscriber_id: 0,
            watched_paths: Vec::new(),
        }
    }
}

impl Drop for WatchRegistration {
    fn drop(&mut self) {
        if let Some(file_watcher) = self.file_watcher.upgrade() {
            file_watcher.unregister_paths(self.subscriber_id, &self.watched_paths);
        }
    }
}

/// Multi-subscriber file watcher built on top of `notify`.
pub struct FileWatcher {
    inner: Option<Mutex<FileWatcherInner>>,
    state: Arc<RwLock<WatchState>>,
}

impl FileWatcher {
    /// Creates a live filesystem watcher and starts its background event loop
    /// on the current Tokio runtime.
    pub fn new() -> notify::Result<Self> {
        let (raw_tx, raw_rx) = mpsc::unbounded_channel();
        let raw_tx_clone = raw_tx;
        let watcher = notify::recommended_watcher(move |res| {
            let _ = raw_tx_clone.send(res);
        })?;
        let inner = FileWatcherInner {
            watcher,
            watched_paths: HashMap::new(),
        };
        let state = Arc::new(RwLock::new(WatchState::default()));
        let file_watcher = Self {
            inner: Some(Mutex::new(inner)),
            state,
        };
        file_watcher.spawn_event_loop(raw_rx);
        Ok(file_watcher)
    }

    /// Creates an inert watcher that only supports test-driven synthetic
    /// notifications.
    pub fn noop() -> Self {
        Self {
            inner: None,
            state: Arc::new(RwLock::new(WatchState::default())),
        }
    }

    /// Adds a new subscriber and returns both its registration handle and its
    /// dedicated event receiver.
    pub fn add_subscriber(self: &Arc<Self>) -> (FileWatcherSubscriber, Receiver) {
        let (tx, rx) = watch_channel();
        let mut state = self
            .state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let subscriber_id = state.next_subscriber_id;
        state.next_subscriber_id += 1;
        state.subscribers.insert(
            subscriber_id,
            SubscriberState {
                watched_paths: HashMap::new(),
                tx,
            },
        );

        let subscriber = FileWatcherSubscriber {
            id: subscriber_id,
            file_watcher: self.clone(),
        };
        (subscriber, rx)
    }

    fn register_paths(&self, subscriber_id: SubscriberId, watched_paths: &[SubscriberWatch]) {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut inner_guard: Option<std::sync::MutexGuard<'_, FileWatcherInner>> = None;

        for subscriber_watch in watched_paths {
            {
                let Some(subscriber) = state.subscribers.get_mut(&subscriber_id) else {
                    return;
                };
                subscriber
                    .watched_paths
                    .entry(subscriber_watch.clone())
                    .and_modify(|state| state.count += 1)
                    .or_insert_with(|| SubscriberWatchState {
                        count: 1,
                        last_exists: subscriber_watch.matched.path.exists(),
                    });
            }

            let counts = state
                .path_ref_counts
                .entry(subscriber_watch.actual.path.clone())
                .or_default();
            let previous_mode = counts.effective_mode();
            counts.increment(subscriber_watch.actual.recursive, /*amount*/ 1);
            let next_mode = counts.effective_mode();
            if previous_mode != next_mode {
                self.reconfigure_watch(&subscriber_watch.actual.path, next_mode, &mut inner_guard);
            }
        }
    }

    // Bridge `notify`'s callback-based events into the Tokio runtime and
    // notify the matching subscribers.
    fn spawn_event_loop(&self, mut raw_rx: mpsc::UnboundedReceiver<notify::Result<Event>>) {
        if let Ok(handle) = Handle::try_current() {
            let state = Arc::clone(&self.state);
            handle.spawn(async move {
                loop {
                    match raw_rx.recv().await {
                        Some(Ok(event)) => {
                            if !is_mutating_event(&event) {
                                continue;
                            }
                            if event.paths.is_empty() {
                                continue;
                            }
                            Self::notify_subscribers(&state, &event.paths).await;
                        }
                        Some(Err(err)) => {
                            warn!("file watcher error: {err}");
                        }
                        None => break,
                    }
                }
            });
        } else {
            warn!("file watcher loop skipped: no Tokio runtime available");
        }
    }

    fn unregister_paths(&self, subscriber_id: SubscriberId, watched_paths: &[SubscriberWatch]) {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut inner_guard: Option<std::sync::MutexGuard<'_, FileWatcherInner>> = None;

        for subscriber_watch in watched_paths {
            {
                let Some(subscriber) = state.subscribers.get_mut(&subscriber_id) else {
                    return;
                };
                let Some(subscriber_watch_state) =
                    subscriber.watched_paths.get_mut(subscriber_watch)
                else {
                    continue;
                };
                subscriber_watch_state.count = subscriber_watch_state.count.saturating_sub(1);
                if subscriber_watch_state.count == 0 {
                    subscriber.watched_paths.remove(subscriber_watch);
                }
            }
            let Some(counts) = state.path_ref_counts.get_mut(&subscriber_watch.actual.path) else {
                continue;
            };
            let previous_mode = counts.effective_mode();
            counts.decrement(subscriber_watch.actual.recursive, /*amount*/ 1);
            let next_mode = counts.effective_mode();
            if counts.is_empty() {
                state.path_ref_counts.remove(&subscriber_watch.actual.path);
            }
            if previous_mode != next_mode {
                self.reconfigure_watch(&subscriber_watch.actual.path, next_mode, &mut inner_guard);
            }
        }
    }

    fn remove_subscriber(&self, subscriber_id: SubscriberId) {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(subscriber) = state.subscribers.remove(&subscriber_id) else {
            return;
        };

        let mut inner_guard: Option<std::sync::MutexGuard<'_, FileWatcherInner>> = None;
        for (subscriber_watch, subscriber_watch_state) in subscriber.watched_paths {
            let Some(path_counts) = state.path_ref_counts.get_mut(&subscriber_watch.actual.path)
            else {
                continue;
            };
            let previous_mode = path_counts.effective_mode();
            path_counts.decrement(subscriber_watch.actual.recursive, subscriber_watch_state.count);
            let next_mode = path_counts.effective_mode();
            if path_counts.is_empty() {
                state.path_ref_counts.remove(&subscriber_watch.actual.path);
            }
            if previous_mode != next_mode {
                self.reconfigure_watch(&subscriber_watch.actual.path, next_mode, &mut inner_guard);
            }
        }
    }

    fn reconfigure_watch<'a>(
        &'a self,
        path: &Path,
        next_mode: Option<RecursiveMode>,
        inner_guard: &mut Option<std::sync::MutexGuard<'a, FileWatcherInner>>,
    ) {
        let Some(inner) = &self.inner else {
            return;
        };
        if inner_guard.is_none() {
            let guard = inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *inner_guard = Some(guard);
        }
        let Some(guard) = inner_guard.as_mut() else {
            return;
        };

        let existing_mode = guard.watched_paths.get(path).copied();
        if existing_mode == next_mode {
            return;
        }

        if existing_mode.is_some() {
            if let Err(err) = guard.watcher.unwatch(path) {
                warn!("failed to unwatch {}: {err}", path.display());
            }
            guard.watched_paths.remove(path);
        }

        let Some(next_mode) = next_mode else {
            return;
        };
        if !path.exists() {
            return;
        }

        if let Err(err) = guard.watcher.watch(path, next_mode) {
            warn!("failed to watch {}: {err}", path.display());
            return;
        }
        guard.watched_paths.insert(path.to_path_buf(), next_mode);
    }

    async fn notify_subscribers(state: &RwLock<WatchState>, event_paths: &[PathBuf]) {
        let subscribers_to_notify: Vec<(WatchSender, Vec<PathBuf>)> = {
            let mut state = state
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state
                .subscribers
                .values_mut()
                .filter_map(|subscriber| {
                    let mut changed_paths = Vec::new();
                    for event_path in event_paths {
                        for (subscriber_watch, subscriber_watch_state) in
                            &mut subscriber.watched_paths
                        {
                            if let Some(path) = changed_path_for_event(
                                subscriber_watch,
                                subscriber_watch_state,
                                event_path,
                            ) {
                                changed_paths.push(path);
                            }
                        }
                    }
                    (!changed_paths.is_empty()).then_some((subscriber.tx.clone(), changed_paths))
                })
                .collect()
        };

        for (subscriber, changed_paths) in subscribers_to_notify {
            subscriber.add_changed_paths(&changed_paths).await;
        }
    }

    #[cfg(test)]
    pub(crate) async fn send_paths_for_test(&self, paths: Vec<PathBuf>) {
        Self::notify_subscribers(&self.state, &paths).await;
    }

    #[cfg(test)]
    pub(crate) fn spawn_event_loop_for_test(
        &self,
        raw_rx: mpsc::UnboundedReceiver<notify::Result<Event>>,
    ) {
        self.spawn_event_loop(raw_rx);
    }

    #[cfg(test)]
    pub(crate) fn watch_counts_for_test(&self, path: &Path) -> Option<(usize, usize)> {
        let state = self
            .state
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state
            .path_ref_counts
            .get(path)
            .map(|counts| (counts.non_recursive, counts.recursive))
    }
}

fn is_mutating_event(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}

fn dedupe_watched_paths(mut watched_paths: Vec<WatchPath>) -> Vec<WatchPath> {
    watched_paths.sort_unstable_by(|a, b| {
        a.path
            .as_os_str()
            .cmp(b.path.as_os_str())
            .then(a.recursive.cmp(&b.recursive))
    });
    watched_paths.dedup();
    watched_paths
}

/// Returns the actual OS watch path and canonical match path for a request.
///
/// Missing targets are watched through the nearest existing ancestor. If that
/// ancestor is not the direct parent, the actual watch becomes recursive so
/// creation events below the ancestor can still reach the subscriber.
fn actual_watch_path(requested: &WatchPath) -> (WatchPath, WatchPath) {
    if requested.path.exists() {
        let matched_path = requested
            .path
            .canonicalize()
            .unwrap_or_else(|_| requested.path.clone());
        let actual = requested.clone();
        let matched = WatchPath {
            path: matched_path,
            recursive: requested.recursive,
        };
        return (actual, matched);
    }

    let requested_parent = requested.path.parent();
    let mut ancestor = requested_parent;
    while let Some(path) = ancestor {
        if path.exists() {
            let actual_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            let matched_path = requested
                .path
                .strip_prefix(path)
                .map(|suffix| actual_path.join(suffix))
                .unwrap_or_else(|_| requested.path.clone());
            let actual = WatchPath {
                path: path.to_path_buf(),
                recursive: requested.recursive || Some(path) != requested_parent,
            };
            let matched = WatchPath {
                path: matched_path,
                recursive: requested.recursive,
            };
            return (actual, matched);
        }
        ancestor = path.parent();
    }

    (requested.clone(), requested.clone())
}

/// Converts one raw backend event path into the subscriber-visible path.
///
/// Matching first uses the canonical path namespace reported by many OS
/// backends, then falls back to the originally requested namespace for
/// synthetic tests and backends that preserve the input spelling.
fn changed_path_for_event(
    subscriber_watch: &SubscriberWatch,
    subscriber_watch_state: &mut SubscriberWatchState,
    event_path: &Path,
) -> Option<PathBuf> {
    if let Some(path) = changed_path_for_matched_path(
        subscriber_watch,
        subscriber_watch_state,
        &subscriber_watch.matched,
        event_path,
    ) {
        return Some(path);
    }
    if subscriber_watch.matched.path == subscriber_watch.requested.path {
        return None;
    }
    changed_path_for_matched_path(
        subscriber_watch,
        subscriber_watch_state,
        &subscriber_watch.requested,
        event_path,
    )
}

/// Applies the watch matching rules in one path namespace and maps any emitted
/// path back into the subscriber's requested namespace.
fn changed_path_for_matched_path(
    subscriber_watch: &SubscriberWatch,
    subscriber_watch_state: &mut SubscriberWatchState,
    matched: &WatchPath,
    event_path: &Path,
) -> Option<PathBuf> {
    let requested = &subscriber_watch.requested;
    if event_path == matched.path {
        subscriber_watch_state.last_exists = matched.path.exists();
        return Some(requested.path.clone());
    }
    if matched.path.starts_with(event_path) {
        if subscriber_watch.actual.path != matched.path {
            let now_exists = matched.path.exists();
            let should_notify = now_exists || subscriber_watch_state.last_exists;
            subscriber_watch_state.last_exists = now_exists;
            return should_notify.then(|| requested.path.clone());
        }
        subscriber_watch_state.last_exists = matched.path.exists();
        return Some(event_path.to_path_buf());
    }
    if !event_path.starts_with(&matched.path) {
        return None;
    }
    if !(matched.recursive || event_path.parent() == Some(matched.path.as_path())) {
        return None;
    }
    subscriber_watch_state.last_exists = matched.path.exists();
    Some(
        event_path
            .strip_prefix(&matched.path)
            .map(|suffix| requested.path.join(suffix))
            .unwrap_or_else(|_| event_path.to_path_buf()),
    )
}

#[cfg(test)]
#[path = "file_watcher_tests.rs"]
mod tests;
