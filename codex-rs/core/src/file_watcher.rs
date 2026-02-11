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
use crate::skills::loader::skill_roots_from_layer_stack_with_agents;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileWatcherEvent {
    SkillsChanged { paths: Vec<PathBuf> },
}

struct WatchState {
    skills_roots: HashSet<PathBuf>,
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
            skills_roots: HashSet::new(),
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
                skills_roots: HashSet::new(),
            })),
            tx,
        }
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<FileWatcherEvent> {
        self.tx.subscribe()
    }

    pub(crate) fn register_config(&self, config: &Config) {
        let roots =
            skill_roots_from_layer_stack_with_agents(&config.config_layer_stack, &config.cwd);
        for root in roots {
            self.register_skills_root(root.path);
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
        {
            let mut state = match self.state.write() {
                Ok(state) => state,
                Err(err) => err.into_inner(),
            };
            state.skills_roots.insert(root.clone());
        }
        self.watch_path(root, RecursiveMode::Recursive);
    }

    fn watch_path(&self, path: PathBuf, mode: RecursiveMode) {
        let Some(inner) = &self.inner else {
            return;
        };
        if !path.exists() {
            return;
        }
        let watch_path = path;
        let mut guard = match inner.lock() {
            Ok(guard) => guard,
            Err(err) => err.into_inner(),
        };
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
        Ok(state) => state.skills_roots.clone(),
        Err(err) => {
            let state = err.into_inner();
            state.skills_roots.clone()
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
mod tests {
    use super::*;
    use notify::EventKind;
    use notify::event::AccessKind;
    use notify::event::AccessMode;
    use notify::event::CreateKind;
    use notify::event::ModifyKind;
    use notify::event::RemoveKind;
    use pretty_assertions::assert_eq;
    use tokio::time::timeout;

    fn path(name: &str) -> PathBuf {
        PathBuf::from(name)
    }

    fn notify_event(kind: EventKind, paths: Vec<PathBuf>) -> Event {
        let mut event = Event::new(kind);
        for path in paths {
            event = event.add_path(path);
        }
        event
    }

    #[test]
    fn throttles_and_coalesces_within_interval() {
        let start = Instant::now();
        let mut throttled = ThrottledPaths::new(start);

        throttled.add(vec![path("a")]);
        let first = throttled.take_ready(start).expect("first emit");
        assert_eq!(first, vec![path("a")]);

        throttled.add(vec![path("b"), path("c")]);
        assert_eq!(throttled.take_ready(start), None);

        let second = throttled
            .take_ready(start + WATCHER_THROTTLE_INTERVAL)
            .expect("coalesced emit");
        assert_eq!(second, vec![path("b"), path("c")]);
    }

    #[test]
    fn flushes_pending_on_shutdown() {
        let start = Instant::now();
        let mut throttled = ThrottledPaths::new(start);

        throttled.add(vec![path("a")]);
        let _ = throttled.take_ready(start).expect("first emit");

        throttled.add(vec![path("b")]);
        assert_eq!(throttled.take_ready(start), None);

        let flushed = throttled
            .take_pending(start)
            .expect("shutdown flush emits pending paths");
        assert_eq!(flushed, vec![path("b")]);
    }

    #[test]
    fn classify_event_filters_to_skills_roots() {
        let root = path("/tmp/skills");
        let state = RwLock::new(WatchState {
            skills_roots: HashSet::from([root.clone()]),
        });
        let event = notify_event(
            EventKind::Create(CreateKind::Any),
            vec![
                root.join("demo/SKILL.md"),
                path("/tmp/other/not-a-skill.txt"),
            ],
        );

        let classified = classify_event(&event, &state);
        assert_eq!(classified, vec![root.join("demo/SKILL.md")]);
    }

    #[test]
    fn classify_event_supports_multiple_roots_without_prefix_false_positives() {
        let root_a = path("/tmp/skills");
        let root_b = path("/tmp/workspace/.codex/skills");
        let state = RwLock::new(WatchState {
            skills_roots: HashSet::from([root_a.clone(), root_b.clone()]),
        });
        let event = notify_event(
            EventKind::Modify(ModifyKind::Any),
            vec![
                root_a.join("alpha/SKILL.md"),
                path("/tmp/skills-extra/not-under-skills.txt"),
                root_b.join("beta/SKILL.md"),
            ],
        );

        let classified = classify_event(&event, &state);
        assert_eq!(
            classified,
            vec![root_a.join("alpha/SKILL.md"), root_b.join("beta/SKILL.md")]
        );
    }

    #[test]
    fn classify_event_ignores_non_mutating_event_kinds() {
        let root = path("/tmp/skills");
        let state = RwLock::new(WatchState {
            skills_roots: HashSet::from([root.clone()]),
        });
        let path = root.join("demo/SKILL.md");

        let access_event = notify_event(
            EventKind::Access(AccessKind::Open(AccessMode::Any)),
            vec![path.clone()],
        );
        assert_eq!(classify_event(&access_event, &state), Vec::<PathBuf>::new());

        let any_event = notify_event(EventKind::Any, vec![path.clone()]);
        assert_eq!(classify_event(&any_event, &state), Vec::<PathBuf>::new());

        let other_event = notify_event(EventKind::Other, vec![path]);
        assert_eq!(classify_event(&other_event, &state), Vec::<PathBuf>::new());
    }

    #[test]
    fn register_skills_root_dedupes_state_entries() {
        let watcher = FileWatcher::noop();
        let root = path("/tmp/skills");
        watcher.register_skills_root(root.clone());
        watcher.register_skills_root(root);
        watcher.register_skills_root(path("/tmp/other-skills"));

        let state = watcher.state.read().expect("state lock");
        assert_eq!(state.skills_roots.len(), 2);
    }

    #[tokio::test]
    async fn spawn_event_loop_flushes_pending_changes_on_shutdown() {
        let watcher = FileWatcher::noop();
        let root = path("/tmp/skills");
        {
            let mut state = watcher.state.write().expect("state lock");
            state.skills_roots.insert(root.clone());
        }

        let (raw_tx, raw_rx) = mpsc::unbounded_channel();
        let (tx, mut rx) = broadcast::channel(8);
        watcher.spawn_event_loop(raw_rx, Arc::clone(&watcher.state), tx);

        raw_tx
            .send(Ok(notify_event(
                EventKind::Create(CreateKind::File),
                vec![root.join("a/SKILL.md")],
            )))
            .expect("send first event");
        let first = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("first watcher event")
            .expect("broadcast recv first");
        assert_eq!(
            first,
            FileWatcherEvent::SkillsChanged {
                paths: vec![root.join("a/SKILL.md")]
            }
        );

        raw_tx
            .send(Ok(notify_event(
                EventKind::Remove(RemoveKind::File),
                vec![root.join("b/SKILL.md")],
            )))
            .expect("send second event");
        drop(raw_tx);

        let second = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("second watcher event")
            .expect("broadcast recv second");
        assert_eq!(
            second,
            FileWatcherEvent::SkillsChanged {
                paths: vec![root.join("b/SKILL.md")]
            }
        );
    }
}
