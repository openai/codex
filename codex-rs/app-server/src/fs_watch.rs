use crate::fs_api::invalid_request;
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
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;
#[cfg(test)]
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tracing::warn;

const FS_CHANGED_NOTIFICATION_DEBOUNCE: Duration = Duration::from_millis(200);

// FileWatcher can emit multiple low-level events for one user-visible edit
// (for example, write + rename + metadata updates). App-server clients only
// need a coarse "something under this watch changed" signal, so this receiver
// batches paths until the debounce window closes and then emits one sorted
// notification payload.
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
    _registration: WatchRegistrationGuard,
}

// A watch entry owns both halves of the core watcher subscription:
// - `_subscriber` keeps the per-client receiver alive.
// - `_registration` keeps the path registered with the shared FileWatcher.
// Dropping the entry unregisters the path and closes the receiver.
enum WatchRegistrationGuard {
    Core {
        _subscriber: FileWatcherSubscriber,
        _registration: WatchRegistration,
    },
    #[cfg(test)]
    Synthetic,
}

enum FsWatchEventReceiver {
    Core(DebouncedReceiver),
    #[cfg(test)]
    Synthetic(mpsc::Receiver<FileWatcherEvent>),
}

impl FsWatchEventReceiver {
    async fn recv(&mut self) -> Option<FileWatcherEvent> {
        match self {
            Self::Core(rx) => rx.recv().await,
            #[cfg(test)]
            Self::Synthetic(rx) => rx.recv().await,
        }
    }
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
        let outgoing = self.outgoing.clone();
        let (subscriber, rx) = self.file_watcher.add_subscriber();
        // fs/watch registers the requested path as non-recursive. The core
        // watcher owns the platform-specific matching rules for which backend
        // event paths are forwarded to this app-server watch.
        let registration = subscriber.register_paths(vec![WatchPath {
            path: params.path.to_path_buf(),
            recursive: false,
        }]);
        self.watch_with_event_receiver(
            connection_id,
            params,
            FsWatchEventReceiver::Core(DebouncedReceiver::new(
                rx,
                FS_CHANGED_NOTIFICATION_DEBOUNCE,
            )),
            WatchRegistrationGuard::Core {
                _subscriber: subscriber,
                _registration: registration,
            },
            outgoing,
        )
        .await
    }

    async fn watch_with_event_receiver(
        &self,
        connection_id: ConnectionId,
        params: FsWatchParams,
        rx: FsWatchEventReceiver,
        registration: WatchRegistrationGuard,
        outgoing: Arc<OutgoingMessageSender>,
    ) -> Result<FsWatchResponse, JSONRPCErrorError> {
        let watch_id = params.watch_id;
        let watch_key = WatchKey {
            connection_id,
            watch_id: watch_id.clone(),
        };
        let watch_root = params.path.clone();
        let (terminate_tx, terminate_rx) = oneshot::channel();
        match self.state.lock().await.entries.entry(watch_key) {
            Entry::Occupied(_) => {
                return Err(invalid_request(format!(
                    "watchId already exists: {watch_id}"
                )));
            }
            Entry::Vacant(entry) => {
                entry.insert(WatchEntry {
                    terminate_tx,
                    _registration: registration,
                });
            }
        }

        let task_watch_id = watch_id.clone();
        spawn_watch_forwarding_task(
            outgoing,
            connection_id,
            task_watch_id,
            watch_root,
            rx,
            terminate_rx,
        );

        Ok(FsWatchResponse { path: params.path })
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
            // are sent after the unwatch response.
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

fn spawn_watch_forwarding_task(
    outgoing: Arc<OutgoingMessageSender>,
    connection_id: ConnectionId,
    watch_id: String,
    watch_root: AbsolutePathBuf,
    mut rx: FsWatchEventReceiver,
    terminate_rx: oneshot::Receiver<oneshot::Sender<()>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        // Each watch owns one forwarding task. It exits when the client
        // unwatches, the connection closes and drops the entry, or the
        // underlying FileWatcher subscriber closes.
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
            if let Some(notification) = fs_changed_notification(&watch_id, &watch_root, event) {
                outgoing
                    .send_server_notification_to_connection_and_wait(
                        connection_id,
                        ServerNotification::FsChanged(notification),
                    )
                    .await;
            }
        }
    })
}

fn fs_changed_notification(
    watch_id: &str,
    watch_root: &AbsolutePathBuf,
    event: FileWatcherEvent,
) -> Option<FsChangedNotification> {
    // Absolute backend paths are preserved by AbsolutePathBuf::join, while
    // relative synthetic/test paths are resolved against the logical watch
    // root. Sort so repeated event batches are stable.
    let mut changed_paths = event
        .paths
        .into_iter()
        .map(|path| watch_root.join(path))
        .collect::<Vec<_>>();
    changed_paths.sort_by(|left, right| left.as_path().cmp(right.as_path()));
    (!changed_paths.is_empty()).then(|| FsChangedNotification {
        watch_id: watch_id.to_string(),
        changed_paths,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outgoing_message::OutgoingEnvelope;
    use crate::outgoing_message::OutgoingMessage;
    use pretty_assertions::assert_eq;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::sync::mpsc::error::TryRecvError;
    use tokio::time::timeout;

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

    fn manager_with_outgoing_rx() -> (FsWatchManager, mpsc::Receiver<OutgoingEnvelope>) {
        const OUTGOING_BUFFER: usize = 4;
        let (tx, rx) = mpsc::channel(OUTGOING_BUFFER);
        (
            FsWatchManager::new_with_file_watcher(
                Arc::new(OutgoingMessageSender::new(tx)),
                Arc::new(FileWatcher::noop()),
            ),
            rx,
        )
    }

    async fn watch_with_synthetic_events(
        manager: &FsWatchManager,
        connection_id: ConnectionId,
        params: FsWatchParams,
    ) -> Result<(FsWatchResponse, mpsc::Sender<FileWatcherEvent>), JSONRPCErrorError> {
        let outgoing = manager.outgoing.clone();
        let (event_tx, event_rx) = mpsc::channel(4);
        let response = manager
            .watch_with_event_receiver(
                connection_id,
                params,
                FsWatchEventReceiver::Synthetic(event_rx),
                WatchRegistrationGuard::Synthetic,
                outgoing,
            )
            .await?;

        Ok((response, event_tx))
    }

    async fn recv_fs_changed_notification(
        rx: &mut mpsc::Receiver<OutgoingEnvelope>,
        expected_connection_id: ConnectionId,
    ) -> FsChangedNotification {
        let envelope = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("should receive outgoing envelope before timeout")
            .expect("outgoing channel should be open");
        let OutgoingEnvelope::ToConnection {
            connection_id,
            message,
            write_complete_tx,
        } = envelope
        else {
            panic!("expected targeted outgoing envelope");
        };
        assert_eq!(connection_id, expected_connection_id);
        let OutgoingMessage::AppServerNotification(ServerNotification::FsChanged(notification)) =
            message
        else {
            panic!("expected fs/changed app-server notification");
        };
        write_complete_tx
            .expect("write completion sender should be attached")
            .send(())
            .expect("forwarding task should still be waiting for write completion");
        notification
    }

    #[tokio::test]
    async fn watch_uses_client_id_and_tracks_the_owner_scoped_entry() {
        let temp_dir = TempDir::new().expect("temp dir");
        let head_path = temp_dir.path().join("HEAD");
        std::fs::write(&head_path, "ref: refs/heads/main\n").expect("write HEAD");

        let manager = manager_with_noop_watcher();
        let path = absolute_path(head_path);
        let watch_id = "watch-head".to_string();
        let response = manager
            .watch(
                ConnectionId(1),
                FsWatchParams {
                    watch_id: watch_id.clone(),
                    path: path.clone(),
                },
            )
            .await
            .expect("watch should succeed");

        assert_eq!(response.path, path);

        let state = manager.state.lock().await;
        assert_eq!(
            state.entries.keys().cloned().collect::<HashSet<_>>(),
            HashSet::from([WatchKey {
                connection_id: ConnectionId(1),
                watch_id,
            }])
        );
    }

    #[tokio::test]
    async fn synthetic_watch_forwards_fs_changed_notification_to_connection() {
        let temp_dir = TempDir::new().expect("temp dir");
        let git_dir = temp_dir.path().join(".git");
        std::fs::create_dir(&git_dir).expect("create .git dir");
        let fetch_head_path = git_dir.join("FETCH_HEAD");

        let (manager, mut rx) = manager_with_outgoing_rx();
        let watch_root = absolute_path(git_dir);
        let (response, event_tx) = watch_with_synthetic_events(
            &manager,
            ConnectionId(7),
            FsWatchParams {
                watch_id: "watch-git-dir".to_string(),
                path: watch_root.clone(),
            },
        )
        .await
        .expect("synthetic watch should succeed");
        assert_eq!(response.path, watch_root);

        event_tx
            .send(FileWatcherEvent {
                paths: vec![fetch_head_path.clone()],
            })
            .await
            .expect("watch forwarding task should receive synthetic event");
        let notification = recv_fs_changed_notification(&mut rx, ConnectionId(7)).await;

        assert_eq!(
            notification,
            FsChangedNotification {
                watch_id: "watch-git-dir".to_string(),
                changed_paths: vec![absolute_path(fetch_head_path)],
            }
        );
    }

    #[tokio::test]
    async fn synthetic_unwatch_stops_forwarding_before_response_returns() {
        let temp_dir = TempDir::new().expect("temp dir");
        let git_dir = temp_dir.path().join(".git");
        std::fs::create_dir(&git_dir).expect("create .git dir");
        let fetch_head_path = git_dir.join("FETCH_HEAD");
        let packed_refs_path = git_dir.join("packed-refs");

        let (manager, mut rx) = manager_with_outgoing_rx();
        let (_response, event_tx) = watch_with_synthetic_events(
            &manager,
            ConnectionId(7),
            FsWatchParams {
                watch_id: "watch-git-dir".to_string(),
                path: absolute_path(git_dir),
            },
        )
        .await
        .expect("synthetic watch should succeed");

        event_tx
            .send(FileWatcherEvent {
                paths: vec![fetch_head_path],
            })
            .await
            .expect("watch forwarding task should receive synthetic event");
        let _ = recv_fs_changed_notification(&mut rx, ConnectionId(7)).await;

        manager
            .unwatch(
                ConnectionId(7),
                FsUnwatchParams {
                    watch_id: "watch-git-dir".to_string(),
                },
            )
            .await
            .expect("unwatch should succeed");
        let send_result = event_tx
            .send(FileWatcherEvent {
                paths: vec![packed_refs_path],
            })
            .await;
        assert!(send_result.is_err());

        assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn fs_changed_notification_reports_file_events_for_the_watch_id() {
        let temp_dir = TempDir::new().expect("temp dir");
        let head_path = temp_dir.path().join("HEAD");
        let watch_root = absolute_path(head_path.clone());

        let notification = fs_changed_notification(
            "watch-head",
            &watch_root,
            FileWatcherEvent {
                paths: vec![head_path.clone()],
            },
        )
        .expect("file event should produce fs/changed notification");

        assert_eq!(
            notification,
            FsChangedNotification {
                watch_id: "watch-head".to_string(),
                changed_paths: vec![absolute_path(head_path)],
            }
        );
    }

    #[test]
    fn fs_changed_notification_reports_directory_child_events() {
        let temp_dir = TempDir::new().expect("temp dir");
        let git_dir = temp_dir.path().join(".git");
        let fetch_head = git_dir.join("FETCH_HEAD");
        let watch_root = absolute_path(git_dir);

        let notification = fs_changed_notification(
            "watch-git-dir",
            &watch_root,
            FileWatcherEvent {
                paths: vec![fetch_head.clone()],
            },
        )
        .expect("child event should produce fs/changed notification");

        assert_eq!(
            notification,
            FsChangedNotification {
                watch_id: "watch-git-dir".to_string(),
                changed_paths: vec![absolute_path(fetch_head)],
            }
        );
    }

    #[test]
    fn fs_changed_notification_ignores_empty_events() {
        let temp_dir = TempDir::new().expect("temp dir");
        let watch_root = absolute_path(temp_dir.path().join(".git"));

        assert_eq!(
            fs_changed_notification(
                "watch-git-dir",
                &watch_root,
                FileWatcherEvent { paths: Vec::new() },
            ),
            None
        );
    }

    #[tokio::test]
    async fn unwatch_is_scoped_to_the_connection_that_created_the_watch() {
        let temp_dir = TempDir::new().expect("temp dir");
        let head_path = temp_dir.path().join("HEAD");
        std::fs::write(&head_path, "ref: refs/heads/main\n").expect("write HEAD");

        let manager = manager_with_noop_watcher();
        manager
            .watch(
                ConnectionId(1),
                FsWatchParams {
                    watch_id: "watch-head".to_string(),
                    path: absolute_path(head_path),
                },
            )
            .await
            .expect("watch should succeed");
        let watch_key = WatchKey {
            connection_id: ConnectionId(1),
            watch_id: "watch-head".to_string(),
        };

        manager
            .unwatch(
                ConnectionId(2),
                FsUnwatchParams {
                    watch_id: "watch-head".to_string(),
                },
            )
            .await
            .expect("foreign unwatch should be a no-op");
        assert!(manager.state.lock().await.entries.contains_key(&watch_key));

        manager
            .unwatch(
                ConnectionId(1),
                FsUnwatchParams {
                    watch_id: "watch-head".to_string(),
                },
            )
            .await
            .expect("owner unwatch should succeed");
        assert!(!manager.state.lock().await.entries.contains_key(&watch_key));
    }

    #[tokio::test]
    async fn watch_rejects_duplicate_id_for_the_same_connection() {
        let temp_dir = TempDir::new().expect("temp dir");
        let head_path = temp_dir.path().join("HEAD");
        let fetch_head_path = temp_dir.path().join("FETCH_HEAD");
        std::fs::write(&head_path, "ref: refs/heads/main\n").expect("write HEAD");
        std::fs::write(&fetch_head_path, "old-fetch\n").expect("write FETCH_HEAD");

        let manager = manager_with_noop_watcher();
        manager
            .watch(
                ConnectionId(1),
                FsWatchParams {
                    watch_id: "watch-head".to_string(),
                    path: absolute_path(head_path),
                },
            )
            .await
            .expect("first watch should succeed");

        let error = manager
            .watch(
                ConnectionId(1),
                FsWatchParams {
                    watch_id: "watch-head".to_string(),
                    path: absolute_path(fetch_head_path),
                },
            )
            .await
            .expect_err("duplicate watch should fail");

        assert_eq!(error.message, "watchId already exists: watch-head");
        assert_eq!(manager.state.lock().await.entries.len(), 1);
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
        let response = manager
            .watch(
                ConnectionId(1),
                FsWatchParams {
                    watch_id: "watch-head".to_string(),
                    path: absolute_path(head_path.clone()),
                },
            )
            .await
            .expect("first watch should succeed");
        manager
            .watch(
                ConnectionId(1),
                FsWatchParams {
                    watch_id: "watch-fetch-head".to_string(),
                    path: absolute_path(fetch_head_path),
                },
            )
            .await
            .expect("second watch should succeed");
        manager
            .watch(
                ConnectionId(2),
                FsWatchParams {
                    watch_id: "watch-packed-refs".to_string(),
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
                watch_id: "watch-packed-refs".to_string(),
            }])
        );
        assert_eq!(response.path, absolute_path(head_path));
    }
}
