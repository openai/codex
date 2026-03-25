//! Skills-specific watcher built on top of the generic [`FileWatcher`].

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::runtime::Handle;
use tokio::sync::broadcast;
use tracing::warn;

use crate::SkillsManager;
use crate::config::Config;
use crate::file_watcher::FileWatcher;
use crate::file_watcher::FileWatcherSubscriber;
use crate::file_watcher::Receiver;
use crate::file_watcher::ThrottledWatchReceiver;
use crate::file_watcher::WatchPath;
use crate::file_watcher::WatchRegistration;
use crate::plugins::PluginsManager;
use crate::skills_load_input_from_config;

#[cfg(not(test))]
const WATCHER_THROTTLE_INTERVAL: Duration = Duration::from_secs(10);
#[cfg(test)]
const WATCHER_THROTTLE_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillsWatcherEvent {
    SkillsChanged { paths: Vec<PathBuf> },
}

pub(crate) struct SkillsWatcher {
    subscriber: FileWatcherSubscriber,
    tx: broadcast::Sender<SkillsWatcherEvent>,
}

impl SkillsWatcher {
    pub(crate) fn new(file_watcher: &Arc<FileWatcher>) -> Self {
        let (subscriber, rx) = file_watcher.add_subscriber();
        let (tx, _) = broadcast::channel(128);
        let skills_watcher = Self {
            subscriber,
            tx: tx.clone(),
        };
        Self::spawn_event_loop(rx, tx);
        skills_watcher
    }

    pub(crate) fn noop() -> Self {
        Self::new(&Arc::new(FileWatcher::noop()))
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<SkillsWatcherEvent> {
        self.tx.subscribe()
    }

    pub(crate) fn register_config(
        &self,
        config: &Config,
        skills_manager: &SkillsManager,
        plugins_manager: &PluginsManager,
    ) -> WatchRegistration {
        let plugin_outcome = plugins_manager.plugins_for_config(config);
        let effective_skill_roots = plugin_outcome.effective_skill_roots();
        let skills_input = skills_load_input_from_config(config, effective_skill_roots);
        let roots = skills_manager
            .skill_roots_for_config(&skills_input)
            .into_iter()
            .map(|root| WatchPath {
                path: root.path,
                recursive: true,
            })
            .collect();
        self.subscriber.register_paths(roots)
    }

    fn spawn_event_loop(rx: Receiver, tx: broadcast::Sender<SkillsWatcherEvent>) {
        let mut rx = ThrottledWatchReceiver::new(rx, WATCHER_THROTTLE_INTERVAL);
        if let Ok(handle) = Handle::try_current() {
            handle.spawn(async move {
                while let Some(event) = rx.recv().await {
                    let _ = tx.send(SkillsWatcherEvent::SkillsChanged { paths: event.paths });
                }
            });
        } else {
            warn!("skills watcher listener skipped: no Tokio runtime available");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tokio::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn forwards_file_watcher_events() {
        let file_watcher = Arc::new(FileWatcher::noop());
        let skills_watcher = SkillsWatcher::new(&file_watcher);
        let mut rx = skills_watcher.subscribe();
        let _registration = skills_watcher
            .subscriber
            .register_path(PathBuf::from("/tmp/skill"), true);

        file_watcher
            .send_paths_for_test(vec![PathBuf::from("/tmp/skill/SKILL.md")])
            .await;

        let event = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("skills watcher event")
            .expect("broadcast recv");
        assert_eq!(
            event,
            SkillsWatcherEvent::SkillsChanged {
                paths: vec![PathBuf::from("/tmp/skill/SKILL.md")],
            }
        );
    }
}
