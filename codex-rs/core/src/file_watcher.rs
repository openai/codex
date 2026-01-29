//! Watches AGENTS and skill roots for changes and broadcasts coarse-grained
//! `FileWatcherEvent`s that higher-level components react to on the next turn.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

use notify::Event;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use tokio::runtime::Handle;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tracing::warn;

use crate::config::Config;
use crate::features::Feature;
use crate::project_doc::DEFAULT_PROJECT_DOC_FILENAME;
use crate::project_doc::LOCAL_PROJECT_DOC_FILENAME;
use crate::project_doc::project_doc_search_dirs;
use crate::skills::loader::skill_roots_from_layer_stack;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileWatcherEvent {
    AgentsChanged { paths: Vec<PathBuf> },
    SkillsChanged { paths: Vec<PathBuf> },
}

struct WatchState {
    skills_roots: HashSet<PathBuf>,
    agents_enabled: bool,
    skills_enabled: bool,
    agents_fallback_filenames: HashSet<String>,
}

struct FileWatcherInner {
    watcher: RecommendedWatcher,
    watched_paths: HashMap<PathBuf, RecursiveMode>,
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
            agents_enabled: false,
            skills_enabled: false,
            agents_fallback_filenames: HashSet::new(),
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
                agents_enabled: false,
                skills_enabled: false,
                agents_fallback_filenames: HashSet::new(),
            })),
            tx,
        }
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<FileWatcherEvent> {
        self.tx.subscribe()
    }

    pub(crate) fn register_config(&self, config: &Config) {
        let agents_enabled = config.features.enabled(Feature::LiveAgentsReload);
        let skills_enabled = config.features.enabled(Feature::LiveSkillsReload);

        {
            let mut state = match self.state.write() {
                Ok(state) => state,
                Err(err) => err.into_inner(),
            };
            state.agents_enabled = agents_enabled;
            state.skills_enabled = skills_enabled;
            state.agents_fallback_filenames = config
                .project_doc_fallback_filenames
                .iter()
                .filter(|name| !name.is_empty())
                .cloned()
                .collect();
            if !skills_enabled {
                state.skills_roots.clear();
            }
        }

        if agents_enabled {
            self.watch_agents_root(config.codex_home.clone());
        }

        if agents_enabled {
            match project_doc_search_dirs(config) {
                Ok(dirs) => {
                    for dir in dirs {
                        self.watch_path(dir, RecursiveMode::NonRecursive);
                    }
                }
                Err(err) => {
                    warn!("failed to determine AGENTS.md search dirs: {err}");
                }
            }
        }

        if skills_enabled {
            self.register_skills_root(config.codex_home.join("skills"));
            let roots = skill_roots_from_layer_stack(&config.config_layer_stack);
            for root in roots {
                self.register_skills_root(root.path);
            }
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
                while let Some(res) = raw_rx.recv().await {
                    match res {
                        Ok(event) => {
                            let (agents_paths, skills_paths) = classify_event(&event, &state);
                            if !agents_paths.is_empty() {
                                let _ = tx.send(FileWatcherEvent::AgentsChanged {
                                    paths: agents_paths,
                                });
                            }
                            if !skills_paths.is_empty() {
                                let _ = tx.send(FileWatcherEvent::SkillsChanged {
                                    paths: skills_paths,
                                });
                            }
                        }
                        Err(err) => {
                            warn!("file watcher error: {err}");
                        }
                    }
                }
            });
        } else {
            warn!("file watcher loop skipped: no Tokio runtime available");
        }
    }

    fn watch_agents_root(&self, root: PathBuf) {
        self.watch_path(root, RecursiveMode::NonRecursive);
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
        let Some(watch_path) = nearest_existing_ancestor(&path) else {
            return;
        };
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

fn classify_event(event: &Event, state: &RwLock<WatchState>) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut agents_paths = Vec::new();
    let mut skills_paths = Vec::new();
    let (agents_enabled, skills_enabled, skills_roots, agents_fallback_filenames) =
        match state.read() {
            Ok(state) => (
                state.agents_enabled,
                state.skills_enabled,
                state.skills_roots.clone(),
                state.agents_fallback_filenames.clone(),
            ),
            Err(err) => {
                let state = err.into_inner();
                (
                    state.agents_enabled,
                    state.skills_enabled,
                    state.skills_roots.clone(),
                    state.agents_fallback_filenames.clone(),
                )
            }
        };

    for path in &event.paths {
        if agents_enabled && is_agents_path(path, &agents_fallback_filenames) {
            agents_paths.push(path.clone());
        }
        if skills_enabled && is_skills_path(path, &skills_roots) {
            skills_paths.push(path.clone());
        }
    }

    (agents_paths, skills_paths)
}

fn is_agents_path(path: &Path, fallbacks: &HashSet<String>) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name == DEFAULT_PROJECT_DOC_FILENAME
        || name == LOCAL_PROJECT_DOC_FILENAME
        || fallbacks.contains(name)
}

fn is_skills_path(path: &Path, roots: &HashSet<PathBuf>) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}

fn nearest_existing_ancestor(path: &Path) -> Option<PathBuf> {
    let mut cursor = path;
    loop {
        if cursor.exists() {
            return Some(cursor.to_path_buf());
        }
        cursor = cursor.parent()?;
    }
}
