use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingMessageSender;
use codex_app_server_protocol::FsChangedNotification;
use codex_app_server_protocol::FsUnwatchParams;
use codex_app_server_protocol::FsUnwatchResponse;
use codex_app_server_protocol::FsWatchParams;
use codex_app_server_protocol::FsWatchResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ServerNotification;
use codex_core::file_watcher::FileWatcher;
use codex_core::file_watcher::FileWatcherEvent;
use codex_core::file_watcher::FileWatcherSubscriber;
use codex_core::file_watcher::Receiver;
use codex_core::file_watcher::WatchPath;
use codex_core::file_watcher::WatchRegistration;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;
#[cfg(test)]
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::Instant;
use tracing::warn;
use uuid::Uuid;

const FS_CHANGED_NOTIFICATION_DEBOUNCE: Duration = Duration::from_millis(200);

struct DebouncedReceiver {
    rx: Receiver,
    interval: Duration,
    changed_paths: HashSet<PathBuf>,
    next_allowance: Option<Instant>,
}

impl DebouncedReceiver {
    fn new(rx: Receiver, interval: Duration) -> Self {
        Self {
            rx,
            interval,
            changed_paths: HashSet::new(),
            next_allowance: None,
        }
    }

    async fn recv(&mut self) -> Option<FileWatcherEvent> {
        while self.changed_paths.is_empty() {
            self.changed_paths.extend(self.rx.recv().await?.paths);
        }
        let next_allowance = *self
            .next_allowance
            .get_or_insert_with(|| Instant::now() + self.interval);

        loop {
            tokio::select! {
                event = self.rx.recv() => self.changed_paths.extend(event?.paths),
                _ = tokio::time::sleep_until(next_allowance) => break,
            }
        }

        Some(FileWatcherEvent {
            paths: self.changed_paths.drain().collect(),
        })
    }
}

#[derive(Clone)]
pub(crate) struct FsWatchManager {
    outgoing: Arc<OutgoingMessageSender>,
    file_watcher: Arc<FileWatcher>,
    state: Arc<AsyncMutex<FsWatchState>>,
}

#[derive(Default)]
struct FsWatchState {
    entries: HashMap<WatchKey, WatchEntry>,
}

struct WatchEntry {
    terminate_tx: oneshot::Sender<oneshot::Sender<()>>,
    _subscriber: FileWatcherSubscriber,
    _registration: WatchRegistration,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct WatchKey {
    connection_id: ConnectionId,
    watch_id: String,
}

impl FsWatchManager {
    pub(crate) fn new(outgoing: Arc<OutgoingMessageSender>) -> Self {
        let file_watcher = match FileWatcher::new() {
            Ok(file_watcher) => Arc::new(file_watcher),
            Err(err) => {
                warn!("filesystem watch manager falling back to noop core watcher: {err}");
                Arc::new(FileWatcher::noop())
            }
        };
        Self::new_with_file_watcher(outgoing, file_watcher)
    }

    fn new_with_file_watcher(
        outgoing: Arc<OutgoingMessageSender>,
        file_watcher: Arc<FileWatcher>,
    ) -> Self {
        Self {
            outgoing,
            file_watcher,
            state: Arc::new(AsyncMutex::new(FsWatchState::default())),
        }
    }

    pub(crate) async fn watch(
        &self,
        connection_id: ConnectionId,
        params: FsWatchParams,
    ) -> Result<FsWatchResponse, JSONRPCErrorError> {
        let watch_id = Uuid::now_v7().to_string();
        let outgoing = self.outgoing.clone();
        let (subscriber, rx) = self.file_watcher.add_subscriber();
        let watch_root = params.path.to_path_buf().clone();
        let registration = subscriber.register_paths(vec![WatchPath {
            path: params.path.to_path_buf(),
            recursive: false,
        }]);
        let (terminate_tx, terminate_rx) = oneshot::channel();

        self.state.lock().await.entries.insert(
            WatchKey {
                connection_id,
                watch_id: watch_id.clone(),
            },
            WatchEntry {
                terminate_tx,
                _subscriber: subscriber,
                _registration: registration,
            },
        );

        let task_watch_id = watch_id.clone();
        tokio::spawn(async move {
            let mut rx = DebouncedReceiver::new(rx, FS_CHANGED_NOTIFICATION_DEBOUNCE);
            tokio::pin!(terminate_rx);
            loop {
                let event = tokio::select! {
                    biased;
                    _ = &mut terminate_rx => break,
                    event = rx.recv() => match event {
                        Some(event) => event,
                        None => break,
                    },
                };
                let mut changed_paths = event
                    .paths
                    .into_iter()
                    .map(|path| AbsolutePathBuf::resolve_path_against_base(&path, &watch_root))
                    .collect::<Vec<_>>();
                changed_paths.sort_by(|left, right| left.as_path().cmp(right.as_path()));
                if !changed_paths.is_empty() {
                    outgoing
                        .send_server_notification_to_connection_and_wait(
                            connection_id,
                            ServerNotification::FsChanged(FsChangedNotification {
                                watch_id: task_watch_id.clone(),
                                changed_paths,
                            }),
                        )
                        .await;
                }
            }
        });

        Ok(FsWatchResponse {
            watch_id,
            path: params.path,
        })
    }

    pub(crate) async fn unwatch(
        &self,
        connection_id: ConnectionId,
        params: FsUnwatchParams,
    ) -> Result<FsUnwatchResponse, JSONRPCErrorError> {
        let watch_key = WatchKey {
            connection_id,
            watch_id: params.watch_id,
        };
        let entry = self.state.lock().await.entries.remove(&watch_key);
        if let Some(entry) = entry {
            // Wait for the oneshot to be destroyed by the task to ensure that no notifications
            // are send after the unwatch response.
            let (done_tx, done_rx) = oneshot::channel();
            let _ = entry.terminate_tx.send(done_tx);
            let _ = done_rx.await;
        }
        Ok(FsUnwatchResponse {})
    }

    pub(crate) async fn connection_closed(&self, connection_id: ConnectionId) {
        let mut state = self.state.lock().await;
        state
            .entries
            .extract_if(|key, _| key.connection_id == connection_id)
            .count();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use uuid::Version;

    fn absolute_path(path: PathBuf) -> AbsolutePathBuf {
        assert!(
            path.is_absolute(),
            "path must be absolute: {}",
            path.display()
        );
        AbsolutePathBuf::try_from(path).expect("path should be absolute")
    }

    fn manager_with_noop_watcher() -> FsWatchManager {
        const OUTGOING_BUFFER: usize = 1;
        let (tx, _rx) = mpsc::channel(OUTGOING_BUFFER);
        FsWatchManager::new_with_file_watcher(
            Arc::new(OutgoingMessageSender::new(tx)),
            Arc::new(FileWatcher::noop()),
        )
    }

    #[tokio::test]
    async fn watch_returns_a_v7_id_and_tracks_the_owner_scoped_entry() {
        let temp_dir = TempDir::new().expect("temp dir");
        let head_path = temp_dir.path().join("HEAD");
        std::fs::write(&head_path, "ref: refs/heads/main\n").expect("write HEAD");

        let manager = manager_with_noop_watcher();
        let path = absolute_path(head_path);
        let response = manager
            .watch(ConnectionId(1), FsWatchParams { path: path.clone() })
            .await
            .expect("watch should succeed");

        assert_eq!(response.path, path);
        let watch_id = Uuid::parse_str(&response.watch_id).expect("watch id should be a UUID");
        assert_eq!(watch_id.get_version(), Some(Version::SortRand));

        let state = manager.state.lock().await;
        assert_eq!(
            state.entries.keys().cloned().collect::<HashSet<_>>(),
            HashSet::from([WatchKey {
                connection_id: ConnectionId(1),
                watch_id: response.watch_id,
            }])
        );
    }

    #[tokio::test]
    async fn unwatch_is_scoped_to_the_connection_that_created_the_watch() {
        let temp_dir = TempDir::new().expect("temp dir");
        let head_path = temp_dir.path().join("HEAD");
        std::fs::write(&head_path, "ref: refs/heads/main\n").expect("write HEAD");

        let manager = manager_with_noop_watcher();
        let response = manager
            .watch(
                ConnectionId(1),
                FsWatchParams {
                    path: absolute_path(head_path),
                },
            )
            .await
            .expect("watch should succeed");
        let watch_key = WatchKey {
            connection_id: ConnectionId(1),
            watch_id: response.watch_id.clone(),
        };

        manager
            .unwatch(
                ConnectionId(2),
                FsUnwatchParams {
                    watch_id: response.watch_id.clone(),
                },
            )
            .await
            .expect("foreign unwatch should be a no-op");
        assert!(manager.state.lock().await.entries.contains_key(&watch_key));

        manager
            .unwatch(
                ConnectionId(1),
                FsUnwatchParams {
                    watch_id: response.watch_id,
                },
            )
            .await
            .expect("owner unwatch should succeed");
        assert!(!manager.state.lock().await.entries.contains_key(&watch_key));
    }

    #[tokio::test]
    async fn connection_closed_removes_only_that_connections_watches() {
        let temp_dir = TempDir::new().expect("temp dir");
        let head_path = temp_dir.path().join("HEAD");
        let fetch_head_path = temp_dir.path().join("FETCH_HEAD");
        let packed_refs_path = temp_dir.path().join("packed-refs");
        std::fs::write(&head_path, "ref: refs/heads/main\n").expect("write HEAD");
        std::fs::write(&fetch_head_path, "old-fetch\n").expect("write FETCH_HEAD");
        std::fs::write(&packed_refs_path, "refs\n").expect("write packed-refs");

        let manager = manager_with_noop_watcher();
        let response_1 = manager
            .watch(
                ConnectionId(1),
                FsWatchParams {
                    path: absolute_path(head_path),
                },
            )
            .await
            .expect("first watch should succeed");
        let response_2 = manager
            .watch(
                ConnectionId(1),
                FsWatchParams {
                    path: absolute_path(fetch_head_path),
                },
            )
            .await
            .expect("second watch should succeed");
        let response_3 = manager
            .watch(
                ConnectionId(2),
                FsWatchParams {
                    path: absolute_path(packed_refs_path),
                },
            )
            .await
            .expect("third watch should succeed");

        manager.connection_closed(ConnectionId(1)).await;

        assert_eq!(
            manager
                .state
                .lock()
                .await
                .entries
                .keys()
                .cloned()
                .collect::<HashSet<_>>(),
            HashSet::from([WatchKey {
                connection_id: ConnectionId(2),
                watch_id: response_3.watch_id,
            }])
        );
        assert_ne!(response_1.watch_id, response_2.watch_id);
    }
}
