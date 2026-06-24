use super::*;

#[derive(Clone)]
pub(crate) struct ThreadCatalogSubscriptions {
    outgoing: Arc<OutgoingMessageSender>,
    connection_ids: Arc<Mutex<HashSet<ConnectionId>>>,
    revision: Arc<Mutex<u64>>,
}

impl ThreadCatalogSubscriptions {
    pub(crate) fn new(outgoing: Arc<OutgoingMessageSender>) -> Self {
        Self {
            outgoing,
            connection_ids: Arc::new(Mutex::new(HashSet::new())),
            revision: Arc::new(Mutex::new(0)),
        }
    }

    pub(super) async fn subscribe(
        &self,
        connection_id: ConnectionId,
    ) -> ThreadCatalogSubscribeResponse {
        let mut connection_ids = self.connection_ids.lock().await;
        connection_ids.insert(connection_id);
        let revision = *self.revision.lock().await;
        ThreadCatalogSubscribeResponse { revision }
    }

    pub(super) async fn unsubscribe(
        &self,
        connection_id: ConnectionId,
    ) -> ThreadCatalogUnsubscribeResponse {
        self.connection_ids.lock().await.remove(&connection_id);
        ThreadCatalogUnsubscribeResponse {}
    }

    pub(super) async fn connection_closed(&self, connection_id: ConnectionId) {
        self.connection_ids.lock().await.remove(&connection_id);
    }

    pub(super) async fn publish_thread_summary(&self, thread: ThreadSummary) {
        let connection_ids = self
            .connection_ids
            .lock()
            .await
            .iter()
            .copied()
            .collect::<Vec<_>>();
        if connection_ids.is_empty() {
            return;
        }
        let revision = {
            let mut revision = self.revision.lock().await;
            *revision = revision.saturating_add(1);
            *revision
        };
        self.outgoing
            .send_server_notification_to_connections(
                &connection_ids,
                ServerNotification::ThreadCatalogChanged(ThreadCatalogChangedNotification {
                    revision,
                    thread,
                }),
            )
            .await;
    }

    pub(super) async fn publish_thread_change(
        &self,
        thread_store: &Arc<dyn ThreadStore>,
        thread_id: ThreadId,
        fallback_provider: &str,
        fallback_cwd: &AbsolutePathBuf,
    ) {
        if self.connection_ids.lock().await.is_empty() {
            return;
        }
        let stored_thread = match thread_store
            .read_thread(StoreReadThreadParams {
                thread_id,
                include_archived: true,
                include_history: false,
            })
            .await
        {
            Ok(stored_thread) => stored_thread,
            Err(ThreadStoreError::ThreadNotFound { .. }) => return,
            Err(err) => {
                warn!("failed to read thread {thread_id} for catalog notification: {err}");
                return;
            }
        };
        let summary =
            thread_summary_from_stored_thread(stored_thread, fallback_provider, fallback_cwd);
        self.publish_thread_summary(summary).await;
    }
}
