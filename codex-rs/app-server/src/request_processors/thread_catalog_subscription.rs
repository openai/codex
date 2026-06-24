use super::*;

#[derive(Clone)]
pub(crate) struct ThreadCatalogSubscriptions {
    outgoing: Arc<OutgoingMessageSender>,
    state: Arc<Mutex<ThreadCatalogSubscriptionState>>,
    delivery_barrier: Arc<Semaphore>,
}

#[derive(Default)]
struct ThreadCatalogSubscriptionState {
    connection_ids: HashSet<ConnectionId>,
    revision: u64,
}

impl ThreadCatalogSubscriptions {
    pub(crate) fn new(outgoing: Arc<OutgoingMessageSender>) -> Self {
        Self {
            outgoing,
            state: Arc::new(Mutex::new(ThreadCatalogSubscriptionState::default())),
            delivery_barrier: Arc::new(Semaphore::new(1)),
        }
    }

    pub(super) async fn subscribe(
        &self,
        connection_id: ConnectionId,
    ) -> ThreadCatalogSubscribeResponse {
        let _delivery_barrier = self.acquire_delivery_barrier().await;
        let mut state = self.state.lock().await;
        state.connection_ids.insert(connection_id);
        ThreadCatalogSubscribeResponse {
            revision: state.revision,
        }
    }

    pub(super) async fn unsubscribe(
        &self,
        connection_id: ConnectionId,
    ) -> ThreadCatalogUnsubscribeResponse {
        let _delivery_barrier = self.acquire_delivery_barrier().await;
        self.state
            .lock()
            .await
            .connection_ids
            .remove(&connection_id);
        ThreadCatalogUnsubscribeResponse {}
    }

    pub(super) async fn connection_closed(&self, connection_id: ConnectionId) {
        let _delivery_barrier = self.acquire_delivery_barrier().await;
        self.state
            .lock()
            .await
            .connection_ids
            .remove(&connection_id);
    }

    pub(super) async fn publish_thread_summary(&self, thread: ThreadSummary) {
        let _delivery_barrier = self.acquire_delivery_barrier().await;
        let (connection_ids, revision) = {
            let mut state = self.state.lock().await;
            if state.connection_ids.is_empty() {
                return;
            }
            state.revision = state.revision.saturating_add(1);
            (
                state.connection_ids.iter().copied().collect::<Vec<_>>(),
                state.revision,
            )
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
        if self.state.lock().await.connection_ids.is_empty() {
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

    async fn acquire_delivery_barrier(&self) -> SemaphorePermit<'_> {
        match self.delivery_barrier.acquire().await {
            Ok(permit) => permit,
            Err(_) => unreachable!("catalog delivery semaphore is never closed"),
        }
    }
}
