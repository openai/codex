use super::*;

#[derive(Clone)]
pub(crate) struct ThreadCatalogSubscriptions {
    outgoing: Arc<OutgoingMessageSender>,
    connection_ids: Arc<Mutex<HashSet<ConnectionId>>>,
}

impl ThreadCatalogSubscriptions {
    pub(crate) fn new(outgoing: Arc<OutgoingMessageSender>) -> Self {
        Self {
            outgoing,
            connection_ids: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub(super) async fn subscribe(
        &self,
        connection_id: ConnectionId,
    ) -> ThreadCatalogSubscribeResponse {
        self.connection_ids.lock().await.insert(connection_id);
        ThreadCatalogSubscribeResponse {}
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
        self.outgoing
            .send_server_notification_to_connections(
                &connection_ids,
                ServerNotification::ThreadCatalogChanged(ThreadCatalogChangedNotification {
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
