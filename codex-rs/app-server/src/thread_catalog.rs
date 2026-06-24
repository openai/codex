use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Weak;

use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadCatalogChangedNotification;
use codex_app_server_protocol::ThreadSummary;
use codex_thread_store::ReadThreadParams;
use codex_thread_store::ThreadCatalogChange;
use codex_thread_store::ThreadStore;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio::sync::Mutex;
use tokio::sync::broadcast::error::RecvError;
use tracing::warn;

use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingMessageSender;
use crate::request_processors::thread_from_stored_thread;

#[derive(Clone)]
pub(crate) struct ThreadCatalogSubscriptions {
    connection_ids: Arc<Mutex<HashSet<ConnectionId>>>,
}

impl ThreadCatalogSubscriptions {
    pub(crate) fn new(
        outgoing: Arc<OutgoingMessageSender>,
        thread_store: Arc<dyn ThreadStore>,
        fallback_provider: String,
        fallback_cwd: AbsolutePathBuf,
    ) -> Self {
        let connection_ids = Arc::new(Mutex::new(HashSet::new()));
        let changes = thread_store.subscribe_catalog_changes();
        tokio::spawn(run_catalog_listener(
            changes,
            Arc::downgrade(&thread_store),
            Arc::clone(&outgoing),
            Arc::clone(&connection_ids),
            fallback_provider,
            fallback_cwd,
        ));
        Self { connection_ids }
    }

    pub(crate) async fn subscribe(&self, connection_id: ConnectionId) {
        self.connection_ids.lock().await.insert(connection_id);
    }

    pub(crate) async fn connection_closed(&self, connection_id: ConnectionId) {
        self.connection_ids.lock().await.remove(&connection_id);
    }
}

async fn run_catalog_listener(
    mut changes: tokio::sync::broadcast::Receiver<ThreadCatalogChange>,
    thread_store: Weak<dyn ThreadStore>,
    outgoing: Arc<OutgoingMessageSender>,
    connection_ids: Arc<Mutex<HashSet<ConnectionId>>>,
    fallback_provider: String,
    fallback_cwd: AbsolutePathBuf,
) {
    loop {
        let change = match changes.recv().await {
            Ok(change) => Some(change),
            Err(RecvError::Lagged(skipped)) => {
                warn!("thread catalog listener lagged by {skipped} changes");
                None
            }
            Err(RecvError::Closed) => break,
        };
        let connection_ids = connection_ids
            .lock()
            .await
            .iter()
            .copied()
            .collect::<Vec<_>>();
        if connection_ids.is_empty() {
            continue;
        }

        let notification = match change {
            Some(ThreadCatalogChange::Upsert { thread_id }) => {
                let Some(thread_store) = thread_store.upgrade() else {
                    break;
                };
                match thread_store
                    .read_thread(ReadThreadParams {
                        thread_id,
                        include_archived: true,
                        include_history: false,
                    })
                    .await
                {
                    Ok(thread) => ServerNotification::ThreadCatalogChanged(
                        ThreadCatalogChangedNotification::Upsert {
                            thread: Box::new(thread_summary_from_stored_thread(
                                thread,
                                &fallback_provider,
                                &fallback_cwd,
                            )),
                        },
                    ),
                    Err(err) => {
                        warn!("failed to read thread {thread_id} after catalog change: {err}");
                        ServerNotification::ThreadCatalogChanged(
                            ThreadCatalogChangedNotification::Invalidate,
                        )
                    }
                }
            }
            Some(ThreadCatalogChange::Delete { thread_id }) => {
                ServerNotification::ThreadCatalogChanged(ThreadCatalogChangedNotification::Delete {
                    thread_id: thread_id.to_string(),
                })
            }
            None => ServerNotification::ThreadCatalogChanged(
                ThreadCatalogChangedNotification::Invalidate,
            ),
        };
        outgoing
            .send_server_notification_to_connections(&connection_ids, notification)
            .await;
    }
}

fn thread_summary_from_stored_thread(
    thread: codex_thread_store::StoredThread,
    fallback_provider: &str,
    fallback_cwd: &AbsolutePathBuf,
) -> ThreadSummary {
    let created_at_ms = thread.created_at.timestamp_millis();
    let updated_at_ms = thread.updated_at.timestamp_millis();
    let recency_at_ms = thread.recency_at.timestamp_millis();
    let archived_at = thread.archived_at.as_ref().map(chrono::DateTime::timestamp);
    let (thread, _) = thread_from_stored_thread(thread, fallback_provider, fallback_cwd);
    ThreadSummary {
        id: thread.id,
        forked_from_id: thread.forked_from_id,
        parent_thread_id: thread.parent_thread_id,
        preview: thread.preview,
        model_provider: thread.model_provider,
        created_at: thread.created_at,
        created_at_ms,
        updated_at: thread.updated_at,
        updated_at_ms,
        recency_at: thread.recency_at,
        recency_at_ms: Some(recency_at_ms),
        archived_at,
        cwd: thread.cwd,
        source: thread.source,
        thread_source: thread.thread_source,
        agent_nickname: thread.agent_nickname,
        agent_role: thread.agent_role,
        git_info: thread.git_info,
        name: thread.name,
    }
}
