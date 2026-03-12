//! Watches skill roots for changes and broadcasts coarse-grained
//! `FileWatcherEvent`s that higher-level components react to on the next turn.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::time::Duration;

use notify::Event;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use tokio::runtime::Handle;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio::time::sleep_until;
use tracing::warn;

use crate::config::Config;
use crate::skills::SkillsManager;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileWatcherEvent {
    SkillsChanged { paths: Vec<PathBuf> },
}

struct WatchState {
    skills_root_ref_counts: HashMap<PathBuf, usize>,
}

struct FileWatcherInner {
    watcher: RecommendedWatcher,
    watched_paths: HashMap<PathBuf, RecursiveMode>,
}

const WATCHER_THROTTLE_INTERVAL: Duration = Duration::from_secs(10);

/// Coalesces bursts of paths and emits at most once per interval.
struct ThrottledPaths {
    pending: HashSet<PathBuf>,
    next_allowed_at: Instant,
}

impl ThrottledPaths {
    fn new(now: Instant) -> Self {
        Self {
            pending: HashSet::new(),
            next_allowed_at: now,
        }
    }

    fn add(&mut self, paths: Vec<PathBuf>) {
        self.pending.extend(paths);
    }

    fn next_deadline(&self, now: Instant) -> Option<Instant> {
        (!self.pending.is_empty() && now < self.next_allowed_at).then_some(self.next_allowed_at)
    }

    fn take_ready(&mut self, now: Instant) -> Option<Vec<PathBuf>> {
        if self.pending.is_empty() || now < self.next_allowed_at {
            return None;
        }
        Some(self.take_with_next_allowed(now))
    }

    fn take_pending(&mut self, now: Instant) -> Option<Vec<PathBuf>> {
        if self.pending.is_empty() {
            return None;
        }
        Some(self.take_with_next_allowed(now))
    }

    fn take_with_next_allowed(&mut self, now: Instant) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self.pending.drain().collect();
        paths.sort_unstable_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
        self.next_allowed_at = now + WATCHER_THROTTLE_INTERVAL;
        paths
    }
}

pub(crate) struct FileWatcher {
    inner: Option<Mutex<FileWatcherInner>>,
    state: Arc<RwLock<WatchState>>,
    tx: broadcast::Sender<FileWatcherEvent>,
}

pub(crate) struct WatchRegistration {
    file_watcher: std::sync::Weak<FileWatcher>,
    roots: Vec<PathBuf>,
}

impl Drop for WatchRegistration {
    fn drop(&mut self) {
        if let Some(file_watcher) = self.file_watcher.upgrade() {
            file_watcher.unregister_roots(&self.roots);
        }
    }
}

impl FileWatcher {
    pub(crate) fn new(_codex_home: PathBuf) -> notify::Result<Self> {
        let (raw_tx, raw_rx) = mpsc::unbounded_channel();
        let raw_tx_clone = raw_tx;
        let watcher = notify::recommended_watcher(move |res| {
            let _ = raw_tx_clone.send(res);
        })?;
        let inner = FileWatcherInner {
            watcher,
            watched_paths: HashMap::new(),
        };
        let (tx, _) = broadcast::channel(128);
        let state = Arc::new(RwLock::new(WatchState {
            skills_root_ref_counts: HashMap::new(),
        }));
        let file_watcher = Self {
            inner: Some(Mutex::new(inner)),
            state: Arc::clone(&state),
            tx: tx.clone(),
        };
        file_watcher.spawn_event_loop(raw_rx, state, tx);
        Ok(file_watcher)
    }

    pub(crate) fn noop() -> Self {
        let (tx, _) = broadcast::channel(1);
        Self {
            inner: None,
            state: Arc::new(RwLock::new(WatchState {
                skills_root_ref_counts: HashMap::new(),
            })),
            tx,
        }
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<FileWatcherEvent> {
        self.tx.subscribe()
    }

    pub(crate) fn register_config(
        self: &Arc<Self>,
        config: &Config,
        skills_manager: &SkillsManager,
    ) -> WatchRegistration {
        let deduped_roots: HashSet<PathBuf> = skills_manager
            .skill_roots_for_config(config)
            .into_iter()
            .map(|root| root.path)
            .collect();
        let mut registered_roots: Vec<PathBuf> = deduped_roots.into_iter().collect();
        registered_roots.sort_unstable_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
        for root in &registered_roots {
            self.register_skills_root(root.clone());
        }

        WatchRegistration {
            file_watcher: Arc::downgrade(self),
            roots: registered_roots,
        }
    }

    // Bridge `notify`'s callback-based events into the Tokio runtime and
    // broadcast coarse-grained change signals to subscribers.
    fn spawn_event_loop(
        &self,
        mut raw_rx: mpsc::UnboundedReceiver<notify::Result<Event>>,
        state: Arc<RwLock<WatchState>>,
        tx: broadcast::Sender<FileWatcherEvent>,
    ) {
        if let Ok(handle) = Handle::try_current() {
            handle.spawn(async move {
                let now = Instant::now();
                let mut skills = ThrottledPaths::new(now);

                loop {
                    let now = Instant::now();
                    let next_deadline = skills.next_deadline(now);
                    let timer_deadline = next_deadline
                        .unwrap_or_else(|| now + Duration::from_secs(60 * 60 * 24 * 365));
                    let timer = sleep_until(timer_deadline);
                    tokio::pin!(timer);

                    tokio::select! {
                        res = raw_rx.recv() => {
                            match res {
                                Some(Ok(event)) => {
                                    let skills_paths = classify_event(&event, &state);
                                    let now = Instant::now();
                                    skills.add(skills_paths);

                                    if let Some(paths) = skills.take_ready(now) {
                                        let _ = tx.send(FileWatcherEvent::SkillsChanged { paths });
                                    }
                                }
                                Some(Err(err)) => {
                                    warn!("file watcher error: {err}");
                                }
                                None => {
                                    // Flush any pending changes before shutdown so subscribers
                                    // see the latest state.
                                    let now = Instant::now();
                                    if let Some(paths) = skills.take_pending(now) {
                                        let _ = tx.send(FileWatcherEvent::SkillsChanged { paths });
                                    }
                                    break;
                                }
                            }
                        }
                        _ = &mut timer => {
                            let now = Instant::now();
                            if let Some(paths) = skills.take_ready(now) {
                                let _ = tx.send(FileWatcherEvent::SkillsChanged { paths });
                            }
                        }
                    }
                }
            });
        } else {
            warn!("file watcher loop skipped: no Tokio runtime available");
        }
    }

    fn register_skills_root(&self, root: PathBuf) {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = state
            .skills_root_ref_counts
            .entry(root.clone())
            .or_insert(0);
        *count += 1;
        if *count == 1 {
            self.watch_path(root, RecursiveMode::Recursive);
        }
    }

    fn unregister_roots(&self, roots: &[PathBuf]) {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut inner_guard: Option<std::sync::MutexGuard<'_, FileWatcherInner>> = None;

        for root in roots {
            let mut should_unwatch = false;
            if let Some(count) = state.skills_root_ref_counts.get_mut(root) {
                if *count > 1 {
                    *count -= 1;
                } else {
                    state.skills_root_ref_counts.remove(root);
                    should_unwatch = true;
                }
            }

            if !should_unwatch {
                continue;
            }
            let Some(inner) = &self.inner else {
                continue;
            };
            if inner_guard.is_none() {
                let guard = inner
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                inner_guard = Some(guard);
            }

            let Some(guard) = inner_guard.as_mut() else {
                continue;
            };
            if guard.watched_paths.remove(root).is_none() {
                continue;
            }
            if let Err(err) = guard.watcher.unwatch(root) {
                warn!("failed to unwatch {}: {err}", root.display());
            }
        }
    }

    fn watch_path(&self, path: PathBuf, mode: RecursiveMode) {
        let Some(inner) = &self.inner else {
            return;
        };
        if !path.exists() {
            return;
        }
        let watch_path = path;
        let mut guard = inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(existing) = guard.watched_paths.get(&watch_path) {
            if *existing == RecursiveMode::Recursive || *existing == mode {
                return;
            }
            if let Err(err) = guard.watcher.unwatch(&watch_path) {
                warn!("failed to unwatch {}: {err}", watch_path.display());
            }
        }
        if let Err(err) = guard.watcher.watch(&watch_path, mode) {
            warn!("failed to watch {}: {err}", watch_path.display());
            return;
        }
        guard.watched_paths.insert(watch_path, mode);
    }
}

fn classify_event(event: &Event, state: &RwLock<WatchState>) -> Vec<PathBuf> {
    if !matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) {
        return Vec::new();
    }

    let mut skills_paths = Vec::new();
    let skills_roots = match state.read() {
        Ok(state) => state
            .skills_root_ref_counts
            .keys()
            .cloned()
            .collect::<HashSet<_>>(),
        Err(err) => {
            let state = err.into_inner();
            state
                .skills_root_ref_counts
                .keys()
                .cloned()
                .collect::<HashSet<_>>()
        }
    };

    for path in &event.paths {
        if is_skills_path(path, &skills_roots) {
            skills_paths.push(path.clone());
        }
    }

    skills_paths
}

fn is_skills_path(path: &Path, roots: &HashSet<PathBuf>) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}

#[cfg(test)]
#[path = "file_watcher_tests.rs"]
mod tests;
