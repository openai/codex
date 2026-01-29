use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use codex_app_server_protocol::ConfigLayerSource;
use codex_core::config::Config;
use codex_core::config_loader::ConfigLayerStackOrdering;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use notify::Config as NotifyConfig;
use notify::Event as NotifyEvent;
use notify::EventKind;
use notify::PollWatcher;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::unbounded_channel;
use tokio::task::JoinHandle;
use tokio::time;
use tokio::time::Instant;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

const DEBOUNCE_WINDOW: Duration = Duration::from_millis(250);
const POLL_INTERVAL: Duration = Duration::from_secs(2);
const SKILLS_DIR: &str = "skills";
const SKILLS_WATCH_ID: &str = "skills-watch";

pub(crate) struct SkillsWatcher {
    _watcher: SkillsWatcherHandle,
    _task: JoinHandle<()>,
}

enum SkillsWatcherHandle {
    Recommended(RecommendedWatcher),
    Poll(PollWatcher),
}

impl SkillsWatcherHandle {
    fn watch(&mut self, path: &Path) -> notify::Result<()> {
        match self {
            Self::Recommended(watcher) => watcher.watch(path, RecursiveMode::Recursive),
            Self::Poll(watcher) => watcher.watch(path, RecursiveMode::Recursive),
        }
    }
}

impl SkillsWatcher {
    pub(crate) fn start(config: &Config, app_event_tx: AppEventSender) -> Option<Self> {
        let watch_roots = skill_watch_roots(config);
        if watch_roots.is_empty() {
            return None;
        }

        let (event_tx, mut event_rx) = unbounded_channel::<notify::Result<NotifyEvent>>();
        let mut watcher = match build_recommended_watcher(event_tx.clone()) {
            Ok(watcher) => SkillsWatcherHandle::Recommended(watcher),
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "failed to start native skill watcher, falling back to polling"
                );
                match build_poll_watcher(event_tx) {
                    Ok(watcher) => SkillsWatcherHandle::Poll(watcher),
                    Err(err) => {
                        tracing::warn!(error = %err, "failed to start polling skill watcher");
                        return None;
                    }
                }
            }
        };

        let mut watched_any = false;
        for root in &watch_roots {
            if !root.is_dir() {
                continue;
            }
            match watcher.watch(root.as_path()) {
                Ok(()) => watched_any = true,
                Err(err) => {
                    let root_display = root.display();
                    tracing::warn!(error = %err, "failed to watch skill root {root_display}");
                }
            }
        }

        if !watched_any {
            return None;
        }

        let skills_roots: Vec<PathBuf> = watch_roots
            .iter()
            .map(|root| root.join(SKILLS_DIR))
            .collect();

        let task = tokio::spawn(async move {
            let mut pending = false;
            let mut next_emit = Instant::now();

            loop {
                tokio::select! {
                    maybe_event = event_rx.recv() => {
                        let Some(event) = maybe_event else {
                            break;
                        };
                        match event {
                            Ok(event) => {
                                if is_skill_event(&event, &skills_roots) {
                                    pending = true;
                                    next_emit = Instant::now() + DEBOUNCE_WINDOW;
                                }
                            }
                            Err(err) => {
                                tracing::warn!(error = %err, "skill watcher error");
                            }
                        }
                    }
                    _ = time::sleep_until(next_emit), if pending => {
                        pending = false;
                        let event = Event {
                            id: SKILLS_WATCH_ID.to_string(),
                            msg: EventMsg::SkillsUpdateAvailable,
                        };
                        app_event_tx.send(AppEvent::CodexEvent(event));
                    }
                }
            }
        });

        Some(Self {
            _watcher: watcher,
            _task: task,
        })
    }
}

fn build_recommended_watcher(
    event_tx: UnboundedSender<notify::Result<NotifyEvent>>,
) -> notify::Result<RecommendedWatcher> {
    notify::recommended_watcher(move |result| {
        let _ = event_tx.send(result);
    })
}

fn build_poll_watcher(
    event_tx: UnboundedSender<notify::Result<NotifyEvent>>,
) -> notify::Result<PollWatcher> {
    PollWatcher::new(
        move |result| {
            let _ = event_tx.send(result);
        },
        NotifyConfig::default().with_poll_interval(POLL_INTERVAL),
    )
}

fn skill_watch_roots(config: &Config) -> Vec<PathBuf> {
    let mut roots = HashSet::new();
    for layer in config
        .config_layer_stack
        .get_layers(ConfigLayerStackOrdering::HighestPrecedenceFirst, true)
    {
        if !matches!(
            layer.name,
            ConfigLayerSource::Project { .. } | ConfigLayerSource::User { .. }
        ) {
            continue;
        }
        if let Some(folder) = layer.config_folder() {
            roots.insert(normalize_root(folder.into_path_buf()));
        }
    }
    roots.insert(normalize_root(config.codex_home.clone()));
    roots.into_iter().collect()
}

fn normalize_root(path: PathBuf) -> PathBuf {
    dunce::canonicalize(&path).unwrap_or(path)
}

fn is_skill_event(event: &NotifyEvent, skills_roots: &[PathBuf]) -> bool {
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    event
        .paths
        .iter()
        .any(|path| is_skills_root_path(path, skills_roots))
}

fn is_skills_root_path(path: &Path, skills_roots: &[PathBuf]) -> bool {
    if skills_roots.iter().any(|root| path.starts_with(root)) {
        return true;
    }
    path.components()
        .any(|component| component.as_os_str() == SKILLS_DIR)
}
