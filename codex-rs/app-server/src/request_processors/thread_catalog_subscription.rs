use super::*;

#[derive(Clone)]
pub(crate) struct ThreadCatalogSubscriptions {
    outgoing: Arc<OutgoingMessageSender>,
    connection_ids: Arc<Mutex<HashSet<ConnectionId>>>,
    ephemeral_threads: Arc<Mutex<HashMap<String, ThreadSummary>>>,
}

impl ThreadCatalogSubscriptions {
    pub(crate) fn new(outgoing: Arc<OutgoingMessageSender>) -> Self {
        Self {
            outgoing,
            connection_ids: Arc::new(Mutex::new(HashSet::new())),
            ephemeral_threads: Arc::new(Mutex::new(HashMap::new())),
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
        if thread.ephemeral {
            self.ephemeral_threads
                .lock()
                .await
                .insert(thread.id.clone(), thread.clone());
        }
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

    pub(super) async fn update_ephemeral_thread_summary(
        &self,
        thread_id: ThreadId,
        event: &EventMsg,
    ) -> Option<ThreadSummary> {
        let mut threads = self.ephemeral_threads.lock().await;
        let thread = threads.get_mut(&thread_id.to_string())?;
        match event {
            EventMsg::UserMessage(user) if thread.preview.is_empty() => {
                thread.preview = user.message.clone();
            }
            EventMsg::UserMessage(_)
            | EventMsg::ThreadSettingsApplied(_)
            | EventMsg::ThreadRolledBack(_)
            | EventMsg::TurnComplete(_)
            | EventMsg::TurnAborted(_) => {}
            _ => return None,
        }
        let updated_at_ms =
            i64::try_from(time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
                .ok()?;
        thread.updated_at = updated_at_ms / 1_000;
        thread.updated_at_ms = updated_at_ms;
        Some(thread.clone())
    }
}
