use crate::fs_api::invalid_request;
use crate::fs_api::map_io_error;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingMessageSender;
use codex_app_server_protocol::FsChangedNotification;
use codex_app_server_protocol::FsUnwatchParams;
use codex_app_server_protocol::FsUnwatchResponse;
use codex_app_server_protocol::FsWatchParams;
use codex_app_server_protocol::FsWatchResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ServerNotification;
use codex_utils_absolute_path::AbsolutePathBuf;
use notify::Event;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::hash::Hasher;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::warn;
use uuid::Uuid;

const FS_CHANGED_NOTIFICATION_DEBOUNCE: Duration = Duration::from_millis(200);

#[derive(Clone)]
pub(crate) struct FsWatchManager {
    outgoing: Arc<OutgoingMessageSender>,
    state: Arc<AsyncMutex<FsWatchState>>,
}

#[derive(Default)]
struct FsWatchState {
    entries: HashMap<WatchPathKey, FsWatchEntry>,
    watch_index: HashMap<WatchKey, WatchPathKey>,
}

struct FsWatchEntry {
    subscriptions: Arc<AsyncMutex<HashMap<WatchKey, FsWatchSubscription>>>,
    cancel: CancellationToken,
    _watcher: RecommendedWatcher,
}

impl Drop for FsWatchEntry {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[derive(Clone)]
struct FsWatchSubscription {
    path: AbsolutePathBuf,
    watch: SubscriptionWatch,
    pending_changed_paths: HashSet<AbsolutePathBuf>,
    notification_tx: mpsc::Sender<()>,
}

#[derive(Clone)]
enum SubscriptionWatch {
    File {
        filter_path: AbsolutePathBuf,
        last_observed_state: Option<ObservedPathState>,
    },
    Directory {
        last_observed_state: ObservedDirectoryState,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct WatchKey {
    connection_id: ConnectionId,
    watch_id: String,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct WatchPathKey {
    path: PathBuf,
}

struct ResolvedFsWatch {
    path: AbsolutePathBuf,
    watch_path_key: WatchPathKey,
    filter_path: Option<AbsolutePathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObservedPathState {
    is_directory: bool,
    is_file: bool,
    len: u64,
    modified_at: Option<SystemTime>,
    content_fingerprint: Option<u64>,
}

impl ObservedPathState {
    async fn observe(path: &AbsolutePathBuf, metadata: std::fs::Metadata) -> io::Result<Self> {
        let content_fingerprint = if cfg!(windows) && metadata.is_file() {
            // Same-size rewrites can land in the same timestamp bucket on Windows temp volumes.
            // Keep a content fingerprint so ambiguous notify events still identify the changed file.
            let bytes = tokio::fs::read(path).await?;
            let mut hasher = DefaultHasher::new();
            bytes.hash(&mut hasher);
            Some(hasher.finish())
        } else {
            None
        };

        Ok(Self {
            is_directory: metadata.is_dir(),
            is_file: metadata.is_file(),
            len: metadata.len(),
            modified_at: metadata.modified().ok(),
            content_fingerprint,
        })
    }
}

type ObservedDirectoryState = HashMap<AbsolutePathBuf, ObservedPathState>;

impl FsWatchManager {
    pub(crate) fn new(outgoing: Arc<OutgoingMessageSender>) -> Self {
        Self {
            outgoing,
            state: Arc::new(AsyncMutex::new(FsWatchState::default())),
        }
    }

    pub(crate) async fn watch(
        &self,
        connection_id: ConnectionId,
        params: FsWatchParams,
    ) -> Result<FsWatchResponse, JSONRPCErrorError> {
        let ResolvedFsWatch {
            path,
            watch_path_key,
            filter_path,
        } = resolve_fs_watch(params).await?;
        let watch_id = Uuid::now_v7().to_string();
        let watch_key = WatchKey {
            connection_id,
            watch_id: watch_id.clone(),
        };
        let (notification_tx, notification_rx) = mpsc::channel(1);
        let watch = match filter_path {
            Some(filter_path) => SubscriptionWatch::File {
                filter_path,
                last_observed_state: observe_path_state(&path).await.map_err(map_io_error)?,
            },
            None => SubscriptionWatch::Directory {
                last_observed_state: observe_directory_state(&path).await.map_err(map_io_error)?,
            },
        };
        let subscription = FsWatchSubscription {
            path: path.clone(),
            watch,
            pending_changed_paths: HashSet::new(),
            notification_tx,
        };

        let mut maybe_watch_task = None;
        let (subscriptions, notification_rx) = {
            let mut state = self.state.lock().await;
            let entry = match state.entries.entry(watch_path_key.clone()) {
                Entry::Occupied(entry) => entry,
                Entry::Vacant(entry) => {
                    let (raw_tx, raw_rx) = mpsc::unbounded_channel();
                    let mut watcher = notify::recommended_watcher(move |res| {
                        let _ = raw_tx.send(res);
                    })
                    .map_err(map_notify_error)?;
                    watcher
                        .watch(&watch_path_key.path, RecursiveMode::NonRecursive)
                        .map_err(map_notify_error)?;

                    let subscriptions = Arc::new(AsyncMutex::new(HashMap::new()));
                    let cancel = CancellationToken::new();
                    maybe_watch_task = Some((watch_path_key.clone(), cancel.clone(), raw_rx));
                    entry.insert_entry(FsWatchEntry {
                        subscriptions,
                        cancel,
                        _watcher: watcher,
                    })
                }
            };

            let subscriptions = entry.get().subscriptions.clone();
            state.watch_index.insert(watch_key.clone(), watch_path_key);
            subscriptions
                .lock()
                .await
                .insert(watch_key.clone(), subscription);
            (subscriptions, notification_rx)
        };

        if let Some((watch_path_key, cancel, raw_rx)) = maybe_watch_task {
            self.spawn_watch_task(watch_path_key, subscriptions.clone(), cancel, raw_rx);
        }
        self.spawn_notification_task(watch_key, subscriptions, notification_rx);

        Ok(FsWatchResponse { watch_id, path })
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
        let mut state = self.state.lock().await;
        let Some(watch_path_key) = state.watch_index.remove(&watch_key) else {
            return Ok(FsUnwatchResponse {});
        };

        if let Some(subscriptions) = state
            .entries
            .get(&watch_path_key)
            .map(|entry| entry.subscriptions.clone())
        {
            let mut subscriptions = subscriptions.lock().await;
            subscriptions.remove(&watch_key);
            if subscriptions.is_empty() {
                state.entries.remove(&watch_path_key);
            }
        }
        Ok(FsUnwatchResponse {})
    }

    pub(crate) async fn connection_closed(&self, connection_id: ConnectionId) {
        let mut state = self.state.lock().await;
        let mut empty_keys = Vec::new();
        let mut removed_watch_keys = Vec::new();

        for (watch_key, entry) in &state.entries {
            let mut subscriptions = entry.subscriptions.lock().await;
            removed_watch_keys.extend(
                subscriptions
                    .extract_if(|watch_id, _| watch_id.connection_id == connection_id)
                    .map(|(watch_id, _)| watch_id),
            );
            if subscriptions.is_empty() {
                empty_keys.push(watch_key.clone());
            }
        }

        for watch_key in removed_watch_keys {
            state.watch_index.remove(&watch_key);
        }
        for watch_key in empty_keys {
            state.entries.remove(&watch_key);
        }
    }

    fn spawn_watch_task(
        &self,
        watch_path_key: WatchPathKey,
        subscriptions: Arc<AsyncMutex<HashMap<WatchKey, FsWatchSubscription>>>,
        cancel: CancellationToken,
        mut raw_rx: mpsc::UnboundedReceiver<notify::Result<Event>>,
    ) {
        tokio::spawn(async move {
            loop {
                let raw_event = tokio::select! {
                    _ = cancel.cancelled() => break,
                    raw_event = raw_rx.recv() => raw_event,
                };
                match raw_event {
                    Some(Ok(event)) => {
                        if !should_process_event(&event) {
                            continue;
                        }
                        let mut subscriptions = subscriptions.lock().await;
                        update_notifications_for_event(
                            &mut subscriptions,
                            &watch_path_key.path,
                            &event,
                        )
                        .await;
                    }
                    Some(Err(err)) => {
                        warn!(
                            "filesystem watch error for {}: {err}",
                            watch_path_key.path.display()
                        );
                    }
                    None => break,
                }
            }
        });
    }

    fn spawn_notification_task(
        &self,
        watch_key: WatchKey,
        subscriptions: Arc<AsyncMutex<HashMap<WatchKey, FsWatchSubscription>>>,
        mut notification_rx: mpsc::Receiver<()>,
    ) {
        let outgoing = self.outgoing.clone();
        tokio::spawn(async move {
            while notification_rx.recv().await.is_some() {
                tokio::time::sleep(FS_CHANGED_NOTIFICATION_DEBOUNCE).await;

                let changed_paths = {
                    let mut subscriptions = subscriptions.lock().await;
                    let Some(subscription) = subscriptions.get_mut(&watch_key) else {
                        return;
                    };
                    while notification_rx.try_recv().is_ok() {}
                    if subscription.pending_changed_paths.is_empty() {
                        continue;
                    }
                    std::mem::take(&mut subscription.pending_changed_paths)
                };

                for changed_path in changed_paths {
                    // It is okay if client unwatches in the meantime, each notification has an explicit watch id.
                    outgoing
                        .send_server_notification_to_connections(
                            &[watch_key.connection_id],
                            ServerNotification::FsChanged(FsChangedNotification {
                                watch_id: watch_key.watch_id.clone(),
                                changed_path,
                            }),
                        )
                        .await;
                }
            }
        });
    }
}

async fn update_notifications_for_event(
    subscriptions: &mut HashMap<WatchKey, FsWatchSubscription>,
    watch_root: &Path,
    event: &Event,
) {
    let event_paths = event
        .paths
        .iter()
        .filter_map(
            |path| match AbsolutePathBuf::resolve_path_against_base(path, watch_root) {
                Ok(path) => Some(path),
                Err(err) => {
                    warn!(
                        "failed to normalize watch event path ({}) for {}: {err}",
                        path.display(),
                        watch_root.display()
                    );
                    None
                }
            },
        )
        .collect::<Vec<_>>();
    let event_is_ambiguous =
        event_paths.is_empty() || event_paths.iter().all(|path| path.as_path() == watch_root);

    for subscription in subscriptions.values_mut() {
        let mut changed_paths_were_mutated = false;
        match &mut subscription.watch {
            SubscriptionWatch::File {
                filter_path,
                last_observed_state,
            } => {
                let is_relevant = if event_is_ambiguous {
                    match observe_path_state(&subscription.path).await {
                        Ok(next_state) => {
                            let changed = next_state != *last_observed_state;
                            *last_observed_state = next_state;
                            changed
                        }
                        Err(err) => {
                            warn!(
                                "failed to inspect watched file state for {}: {err}",
                                subscription.path.display()
                            );
                            false
                        }
                    }
                } else {
                    let is_relevant = event_paths
                        .iter()
                        .any(|path| path_matches_filter(path, filter_path, watch_root));
                    if is_relevant {
                        match observe_path_state(&subscription.path).await {
                            Ok(next_state) => {
                                *last_observed_state = next_state;
                            }
                            Err(err) => {
                                warn!(
                                    "failed to refresh watched file state for {}: {err}",
                                    subscription.path.display()
                                );
                            }
                        }
                    }
                    is_relevant
                };

                if is_relevant
                    && subscription
                        .pending_changed_paths
                        .insert(subscription.path.clone())
                {
                    changed_paths_were_mutated = true;
                }
            }
            SubscriptionWatch::Directory {
                last_observed_state,
            } => {
                match directory_changed_paths(
                    &subscription.path,
                    last_observed_state,
                    watch_root,
                    &event_paths,
                    event_is_ambiguous,
                )
                .await
                {
                    Ok(changed_paths) => {
                        for changed_path in changed_paths {
                            changed_paths_were_mutated |=
                                subscription.pending_changed_paths.insert(changed_path);
                        }
                    }
                    Err(err) => {
                        warn!(
                            "failed to inspect watched directory state for {}: {err}",
                            subscription.path.display()
                        );
                    }
                }
            }
        }

        if changed_paths_were_mutated {
            let _ = subscription.notification_tx.try_send(());
        }
    }
}

async fn observe_path_state(path: &AbsolutePathBuf) -> io::Result<Option<ObservedPathState>> {
    match tokio::fs::metadata(path).await {
        Ok(metadata) => Ok(Some(ObservedPathState::observe(path, metadata).await?)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

async fn observe_directory_state(path: &AbsolutePathBuf) -> io::Result<ObservedDirectoryState> {
    let mut state = HashMap::new();
    let mut read_dir = match tokio::fs::read_dir(path).await {
        Ok(read_dir) => read_dir,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(state),
        Err(err) => return Err(err),
    };

    while let Some(entry) = read_dir.next_entry().await? {
        let child_path = AbsolutePathBuf::from_absolute_path(entry.path())?;
        match tokio::fs::symlink_metadata(&child_path).await {
            Ok(metadata) => {
                let observed = ObservedPathState::observe(&child_path, metadata).await?;
                state.insert(child_path, observed);
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
    }

    Ok(state)
}

async fn directory_changed_paths(
    path: &AbsolutePathBuf,
    last_observed_state: &mut ObservedDirectoryState,
    watch_root: &Path,
    event_paths: &[AbsolutePathBuf],
    event_is_ambiguous: bool,
) -> io::Result<HashSet<AbsolutePathBuf>> {
    let next_state = observe_directory_state(path).await?;
    let explicit_paths = event_paths
        .iter()
        .filter(|path| path.as_path() != watch_root && path.as_path().starts_with(watch_root))
        .cloned()
        .collect::<HashSet<_>>();
    let changed_paths = if event_is_ambiguous || explicit_paths.is_empty() {
        diff_directory_state(last_observed_state, &next_state)
    } else {
        explicit_paths
    };
    *last_observed_state = next_state;
    Ok(changed_paths)
}

fn diff_directory_state(
    previous_state: &ObservedDirectoryState,
    next_state: &ObservedDirectoryState,
) -> HashSet<AbsolutePathBuf> {
    let mut candidate_paths = HashSet::new();
    candidate_paths.extend(previous_state.keys().cloned());
    candidate_paths.extend(next_state.keys().cloned());

    candidate_paths
        .into_iter()
        .filter(|path| previous_state.get(path) != next_state.get(path))
        .collect()
}

fn path_matches_filter(
    changed_path: &AbsolutePathBuf,
    filter_path: &AbsolutePathBuf,
    watch_root: &Path,
) -> bool {
    changed_path.as_path() == filter_path.as_path()
        || (changed_path.as_path().parent() == Some(watch_root)
            && changed_path.as_path().file_name() == filter_path.as_path().file_name())
}

async fn resolve_fs_watch(params: FsWatchParams) -> Result<ResolvedFsWatch, JSONRPCErrorError> {
    let requested_path = params.path.into_path_buf();
    match tokio::fs::metadata(&requested_path).await {
        Ok(metadata) => {
            let requested_path = tokio::fs::canonicalize(&requested_path)
                .await
                .map_err(map_io_error)?;
            let requested_path = AbsolutePathBuf::try_from(requested_path).map_err(map_io_error)?;

            if metadata.is_dir() {
                return Ok(ResolvedFsWatch {
                    path: requested_path.clone(),
                    watch_path_key: WatchPathKey {
                        path: requested_path.to_path_buf(),
                    },
                    filter_path: None,
                });
            }

            let watch_root = requested_path
                .parent()
                .ok_or_else(|| {
                    invalid_request("fs/watch requires path to include a parent directory")
                })?
                .to_path_buf();
            return Ok(ResolvedFsWatch {
                path: requested_path.clone(),
                watch_path_key: WatchPathKey { path: watch_root },
                filter_path: Some(requested_path),
            });
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(map_io_error(err)),
    }

    let file_name = requested_path
        .as_path()
        .file_name()
        .ok_or_else(|| invalid_request("fs/watch requires path to include a file name"))?;
    let parent = requested_path
        .parent()
        .ok_or_else(|| invalid_request("fs/watch requires path to include a parent directory"))?;
    let watch_root = tokio::fs::canonicalize(parent)
        .await
        .map_err(map_io_error)?;
    let path = watch_root.join(file_name);
    let path = AbsolutePathBuf::try_from(path).map_err(map_io_error)?;
    Ok(ResolvedFsWatch {
        path: path.clone(),
        watch_path_key: WatchPathKey { path: watch_root },
        filter_path: Some(path),
    })
}

fn should_process_event(event: &Event) -> bool {
    match event.kind {
        EventKind::Access(_) => false,
        EventKind::Any
        | EventKind::Create(_)
        | EventKind::Modify(_)
        | EventKind::Remove(_)
        | EventKind::Other => true,
    }
}

fn map_notify_error(err: notify::Error) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: crate::error_code::INTERNAL_ERROR_CODE,
        message: err.to_string(),
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outgoing_message::OutgoingEnvelope;
    use crate::outgoing_message::OutgoingMessage;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn absolute_path(path: PathBuf) -> AbsolutePathBuf {
        assert!(
            path.is_absolute(),
            "path must be absolute: {}",
            path.display()
        );
        AbsolutePathBuf::try_from(path).expect("path should be absolute")
    }

    async fn file_subscription(path: &Path) -> (FsWatchSubscription, mpsc::Receiver<()>) {
        let path = absolute_path(path.to_path_buf());
        let (notification_tx, notification_rx) = mpsc::channel(1);
        (
            FsWatchSubscription {
                path: path.clone(),
                watch: SubscriptionWatch::File {
                    filter_path: path.clone(),
                    last_observed_state: observe_path_state(&path)
                        .await
                        .expect("should capture file state"),
                },
                pending_changed_paths: HashSet::new(),
                notification_tx,
            },
            notification_rx,
        )
    }

    async fn directory_subscription(path: &Path) -> (FsWatchSubscription, mpsc::Receiver<()>) {
        let path = absolute_path(path.to_path_buf());
        let (notification_tx, notification_rx) = mpsc::channel(1);
        (
            FsWatchSubscription {
                path: path.clone(),
                watch: SubscriptionWatch::Directory {
                    last_observed_state: observe_directory_state(&path)
                        .await
                        .expect("should capture directory state"),
                },
                pending_changed_paths: HashSet::new(),
                notification_tx,
            },
            notification_rx,
        )
    }

    fn expect_fs_changed_notification(
        envelope: OutgoingEnvelope,
    ) -> (ConnectionId, FsChangedNotification) {
        match envelope {
            OutgoingEnvelope::ToConnection {
                connection_id,
                message:
                    OutgoingMessage::AppServerNotification(ServerNotification::FsChanged(notification)),
            } => (connection_id, notification),
            envelope => panic!("expected fs/changed notification, got {envelope:?}"),
        }
    }

    #[tokio::test]
    async fn ambiguous_watch_root_event_notifies_only_the_file_that_changed() {
        let temp_dir = TempDir::new().expect("temp dir");
        let watch_root = temp_dir.path();
        let head_path = watch_root.join("HEAD");
        let fetch_head_path = watch_root.join("FETCH_HEAD");
        std::fs::write(&head_path, "old-head\n").expect("write HEAD");
        std::fs::write(&fetch_head_path, "old-fetch\n").expect("write FETCH_HEAD");

        let mut subscriptions = HashMap::from([
            (
                WatchKey {
                    connection_id: ConnectionId(1),
                    watch_id: "head".to_string(),
                },
                file_subscription(&head_path).await.0,
            ),
            (
                WatchKey {
                    connection_id: ConnectionId(2),
                    watch_id: "fetch".to_string(),
                },
                file_subscription(&fetch_head_path).await.0,
            ),
        ]);

        std::fs::write(&head_path, "new-head\n").expect("update HEAD");

        update_notifications_for_event(
            &mut subscriptions,
            watch_root,
            &Event::new(EventKind::Modify(notify::event::ModifyKind::Any))
                .add_path(watch_root.to_path_buf()),
        )
        .await;

        assert_eq!(
            subscriptions
                .get(&WatchKey {
                    connection_id: ConnectionId(1),
                    watch_id: "head".to_string(),
                })
                .expect("head subscription should exist")
                .pending_changed_paths,
            HashSet::from([absolute_path(head_path)])
        );
    }

    #[tokio::test]
    async fn relative_file_event_path_is_resolved_against_watch_root() {
        let temp_dir = TempDir::new().expect("temp dir");
        let watch_root = temp_dir.path();
        let head_path = watch_root.join("HEAD");
        std::fs::write(&head_path, "old-head\n").expect("write HEAD");

        let watch_key = WatchKey {
            connection_id: ConnectionId(1),
            watch_id: "head".to_string(),
        };
        let (subscription, notification_rx) = file_subscription(&head_path).await;
        let subscriptions = Arc::new(AsyncMutex::new(HashMap::from([(
            watch_key.clone(),
            subscription,
        )])));
        let (tx, mut rx) = mpsc::channel(1);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));
        let manager = FsWatchManager::new(outgoing);
        manager.spawn_notification_task(watch_key, subscriptions.clone(), notification_rx);

        std::fs::write(&head_path, "new-head\n").expect("update HEAD");

        {
            let mut subscriptions_guard = subscriptions.lock().await;
            update_notifications_for_event(
                &mut subscriptions_guard,
                watch_root,
                &Event::new(EventKind::Modify(notify::event::ModifyKind::Any))
                    .add_path(PathBuf::from("HEAD")),
            )
            .await;
        }

        let (connection_id, notification) = expect_fs_changed_notification(
            tokio::time::timeout(Duration::from_secs(1), rx.recv())
                .await
                .expect("notification should arrive")
                .expect("channel should remain open"),
        );
        assert_eq!(connection_id, ConnectionId(1));
        assert_eq!(
            notification,
            FsChangedNotification {
                watch_id: "head".to_string(),
                changed_path: absolute_path(head_path),
            }
        );
    }

    #[tokio::test]
    async fn ambiguous_empty_paths_event_notifies_only_the_file_that_changed() {
        let temp_dir = TempDir::new().expect("temp dir");
        let watch_root = temp_dir.path();
        let head_path = watch_root.join("HEAD");
        let fetch_head_path = watch_root.join("FETCH_HEAD");
        std::fs::write(&head_path, "old-head\n").expect("write HEAD");
        std::fs::write(&fetch_head_path, "old-fetch\n").expect("write FETCH_HEAD");

        let mut subscriptions = HashMap::from([
            (
                WatchKey {
                    connection_id: ConnectionId(1),
                    watch_id: "head".to_string(),
                },
                file_subscription(&head_path).await.0,
            ),
            (
                WatchKey {
                    connection_id: ConnectionId(2),
                    watch_id: "fetch".to_string(),
                },
                file_subscription(&fetch_head_path).await.0,
            ),
        ]);

        std::fs::write(&fetch_head_path, "new-fetch\n").expect("update FETCH_HEAD");

        update_notifications_for_event(
            &mut subscriptions,
            watch_root,
            &Event::new(EventKind::Modify(notify::event::ModifyKind::Any)),
        )
        .await;

        assert_eq!(
            subscriptions
                .get(&WatchKey {
                    connection_id: ConnectionId(2),
                    watch_id: "fetch".to_string(),
                })
                .expect("fetch subscription should exist")
                .pending_changed_paths,
            HashSet::from([absolute_path(fetch_head_path)])
        );
    }

    #[tokio::test]
    async fn ambiguous_empty_paths_event_notifies_only_the_directory_child_that_changed() {
        let temp_dir = TempDir::new().expect("temp dir");
        let watch_root = temp_dir.path();
        let head_path = watch_root.join("HEAD");
        let fetch_head_path = watch_root.join("FETCH_HEAD");
        std::fs::write(&head_path, "old-head\n").expect("write HEAD");
        std::fs::write(&fetch_head_path, "old-fetch\n").expect("write FETCH_HEAD");

        let mut subscriptions = HashMap::from([(
            WatchKey {
                connection_id: ConnectionId(1),
                watch_id: "git-dir".to_string(),
            },
            directory_subscription(watch_root).await.0,
        )]);

        std::fs::write(&fetch_head_path, "new-fetch\n").expect("update FETCH_HEAD");

        update_notifications_for_event(
            &mut subscriptions,
            watch_root,
            &Event::new(EventKind::Modify(notify::event::ModifyKind::Any)),
        )
        .await;

        assert_eq!(
            subscriptions
                .get(&WatchKey {
                    connection_id: ConnectionId(1),
                    watch_id: "git-dir".to_string(),
                })
                .expect("directory subscription should exist")
                .pending_changed_paths,
            HashSet::from([absolute_path(fetch_head_path)])
        );
    }

    #[test]
    fn diff_directory_state_detects_same_length_rewrite_when_content_changes() {
        let temp_dir = TempDir::new().expect("temp dir");
        let fetch_head_path = absolute_path(temp_dir.path().join("FETCH_HEAD"));
        let previous_state = HashMap::from([(
            fetch_head_path.clone(),
            ObservedPathState {
                is_directory: false,
                is_file: true,
                len: 10,
                modified_at: None,
                content_fingerprint: Some(1),
            },
        )]);
        let next_state = HashMap::from([(
            fetch_head_path.clone(),
            ObservedPathState {
                is_directory: false,
                is_file: true,
                len: 10,
                modified_at: None,
                content_fingerprint: Some(2),
            },
        )]);

        assert_eq!(
            diff_directory_state(&previous_state, &next_state),
            HashSet::from([fetch_head_path])
        );
    }

    #[tokio::test]
    async fn relative_directory_event_path_is_resolved_against_watch_root() {
        let temp_dir = TempDir::new().expect("temp dir");
        let watch_root = temp_dir.path();
        let fetch_head_path = watch_root.join("FETCH_HEAD");
        std::fs::write(&fetch_head_path, "old-fetch\n").expect("write FETCH_HEAD");

        let mut subscriptions = HashMap::from([(
            WatchKey {
                connection_id: ConnectionId(1),
                watch_id: "git-dir".to_string(),
            },
            directory_subscription(watch_root).await.0,
        )]);

        std::fs::write(&fetch_head_path, "new-fetch\n").expect("update FETCH_HEAD");

        update_notifications_for_event(
            &mut subscriptions,
            watch_root,
            &Event::new(EventKind::Modify(notify::event::ModifyKind::Any))
                .add_path(PathBuf::from("FETCH_HEAD")),
        )
        .await;

        assert_eq!(
            subscriptions
                .get(&WatchKey {
                    connection_id: ConnectionId(1),
                    watch_id: "git-dir".to_string(),
                })
                .expect("directory subscription should exist")
                .pending_changed_paths,
            HashSet::from([absolute_path(fetch_head_path)])
        );
    }

    #[tokio::test]
    async fn directory_watch_ignores_paths_outside_the_watched_directory() {
        let temp_dir = TempDir::new().expect("temp dir");
        let outside_dir = TempDir::new().expect("outside dir");
        let watch_root = temp_dir.path();
        let fetch_head_path = watch_root.join("FETCH_HEAD");
        let outside_path = outside_dir.path().join("FETCH_HEAD");
        std::fs::write(&fetch_head_path, "old-fetch\n").expect("write FETCH_HEAD");
        std::fs::write(&outside_path, "outside\n").expect("write outside path");

        let mut subscriptions = HashMap::from([(
            WatchKey {
                connection_id: ConnectionId(1),
                watch_id: "git-dir".to_string(),
            },
            directory_subscription(watch_root).await.0,
        )]);

        update_notifications_for_event(
            &mut subscriptions,
            watch_root,
            &Event::new(EventKind::Modify(notify::event::ModifyKind::Name(
                notify::event::RenameMode::Both,
            )))
            .add_path(fetch_head_path.clone())
            .add_path(outside_path),
        )
        .await;

        assert_eq!(
            subscriptions
                .get(&WatchKey {
                    connection_id: ConnectionId(1),
                    watch_id: "git-dir".to_string(),
                })
                .expect("directory subscription should exist")
                .pending_changed_paths,
            HashSet::from([absolute_path(fetch_head_path)])
        );
    }

    #[tokio::test]
    async fn blocked_sender_coalesces_same_path_notifications_per_watch_key() {
        let temp_dir = TempDir::new().expect("temp dir");
        let watch_root = temp_dir.path();
        let head_path = watch_root.join("HEAD");
        std::fs::write(&head_path, "old-head\n").expect("write HEAD");

        let watch_key = WatchKey {
            connection_id: ConnectionId(1),
            watch_id: "head".to_string(),
        };
        let (subscription, notification_rx) = file_subscription(&head_path).await;
        let subscriptions = Arc::new(AsyncMutex::new(HashMap::from([(
            watch_key.clone(),
            subscription,
        )])));
        let (tx, mut rx) = mpsc::channel(1);
        let outgoing = Arc::new(OutgoingMessageSender::new(tx));
        let manager = FsWatchManager::new(outgoing);
        manager.spawn_notification_task(watch_key.clone(), subscriptions.clone(), notification_rx);

        {
            let mut subscriptions_guard = subscriptions.lock().await;
            update_notifications_for_event(
                &mut subscriptions_guard,
                watch_root,
                &Event::new(EventKind::Modify(notify::event::ModifyKind::Any))
                    .add_path(head_path.clone()),
            )
            .await;
        }

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let pending_is_empty = {
                    let subscriptions_guard = subscriptions.lock().await;
                    subscriptions_guard
                        .get(&watch_key)
                        .expect("head subscription should exist")
                        .pending_changed_paths
                        .is_empty()
                };
                if pending_is_empty {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("first batch should be drained into the sender");

        {
            let mut subscriptions_guard = subscriptions.lock().await;
            update_notifications_for_event(
                &mut subscriptions_guard,
                watch_root,
                &Event::new(EventKind::Modify(notify::event::ModifyKind::Any))
                    .add_path(head_path.clone()),
            )
            .await;
        }

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let pending_is_empty = {
                    let subscriptions_guard = subscriptions.lock().await;
                    subscriptions_guard
                        .get(&watch_key)
                        .expect("head subscription should exist")
                        .pending_changed_paths
                        .is_empty()
                };
                if pending_is_empty {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("second batch should be waiting on the blocked sender");

        {
            let mut subscriptions_guard = subscriptions.lock().await;
            update_notifications_for_event(
                &mut subscriptions_guard,
                watch_root,
                &Event::new(EventKind::Modify(notify::event::ModifyKind::Any))
                    .add_path(head_path.clone()),
            )
            .await;
        }
        {
            let mut subscriptions_guard = subscriptions.lock().await;
            update_notifications_for_event(
                &mut subscriptions_guard,
                watch_root,
                &Event::new(EventKind::Modify(notify::event::ModifyKind::Any))
                    .add_path(head_path.clone()),
            )
            .await;
        }

        let (connection_id, notification) = expect_fs_changed_notification(
            tokio::time::timeout(Duration::from_secs(1), rx.recv())
                .await
                .expect("first notification should arrive")
                .expect("channel should remain open"),
        );
        assert_eq!(connection_id, ConnectionId(1));
        assert_eq!(
            notification,
            FsChangedNotification {
                watch_id: "head".to_string(),
                changed_path: absolute_path(head_path.clone()),
            }
        );

        let (connection_id, notification) = expect_fs_changed_notification(
            tokio::time::timeout(Duration::from_secs(1), rx.recv())
                .await
                .expect("blocked notification should arrive")
                .expect("channel should remain open"),
        );
        assert_eq!(connection_id, ConnectionId(1));
        assert_eq!(
            notification,
            FsChangedNotification {
                watch_id: "head".to_string(),
                changed_path: absolute_path(head_path.clone()),
            }
        );

        let (connection_id, notification) = expect_fs_changed_notification(
            tokio::time::timeout(Duration::from_secs(1), rx.recv())
                .await
                .expect("coalesced follow-up notification should arrive")
                .expect("channel should remain open"),
        );
        assert_eq!(connection_id, ConnectionId(1));
        assert_eq!(
            notification,
            FsChangedNotification {
                watch_id: "head".to_string(),
                changed_path: absolute_path(head_path),
            }
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(150), rx.recv())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn unwatch_is_scoped_to_the_connection_that_created_the_watch() {
        let temp_dir = TempDir::new().expect("temp dir");
        let head_path = temp_dir.path().join("HEAD");
        std::fs::write(&head_path, "ref: refs/heads/main\n").expect("write HEAD");

        let (tx, _rx) = mpsc::channel(1);
        let manager = FsWatchManager::new(Arc::new(OutgoingMessageSender::new(tx)));
        let response = manager
            .watch(
                ConnectionId(1),
                FsWatchParams {
                    path: absolute_path(head_path.clone()),
                },
            )
            .await
            .expect("watch should succeed");

        manager
            .unwatch(
                ConnectionId(2),
                FsUnwatchParams {
                    watch_id: response.watch_id.clone(),
                },
            )
            .await
            .expect("foreign unwatch should be a no-op");

        let watch_path_key = WatchPathKey {
            path: head_path
                .parent()
                .expect("watched file should have parent")
                .canonicalize()
                .expect("canonicalize watch root"),
        };
        let watch_key = WatchKey {
            connection_id: ConnectionId(1),
            watch_id: response.watch_id.clone(),
        };
        let state = manager.state.lock().await;
        let entry = state
            .entries
            .get(&watch_path_key)
            .expect("watch entry should remain");
        assert_eq!(state.watch_index.get(&watch_key), Some(&watch_path_key));
        let subscriptions = entry.subscriptions.clone();
        drop(state);
        assert!(subscriptions.lock().await.contains_key(&watch_key));

        manager
            .unwatch(
                ConnectionId(1),
                FsUnwatchParams {
                    watch_id: response.watch_id,
                },
            )
            .await
            .expect("owner unwatch should succeed");
    }
}
