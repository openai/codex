use super::*;

const THREAD_QUEUE_LIST_DEFAULT_LIMIT: usize = 25;
const THREAD_QUEUE_LIST_MAX_LIMIT: usize = 100;

#[derive(Clone)]
pub(crate) struct ThreadQueueRequestProcessor {
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    config_manager: ConfigManager,
    state_db: Option<StateDbHandle>,
    thread_state_manager: ThreadStateManager,
}

impl ThreadQueueRequestProcessor {
    pub(crate) fn new(
        thread_manager: Arc<ThreadManager>,
        outgoing: Arc<OutgoingMessageSender>,
        config_manager: ConfigManager,
        state_db: Option<StateDbHandle>,
        thread_state_manager: ThreadStateManager,
    ) -> Self {
        Self {
            thread_manager,
            outgoing,
            config_manager,
            state_db,
            thread_state_manager,
        }
    }

    fn state_db(&self) -> Result<&StateDbHandle, JSONRPCErrorError> {
        self.state_db
            .as_ref()
            .ok_or_else(|| internal_error("queued turns require the app-server state db"))
    }

    async fn require_enabled(&self) -> Result<(), JSONRPCErrorError> {
        let config = self
            .config_manager
            .load_latest_config(/*fallback_cwd*/ None)
            .await
            .map_err(|err| internal_error(format!("failed to load app-server config: {err}")))?;
        if config.features.enabled(Feature::AppServerQueue) {
            Ok(())
        } else {
            Err(invalid_request("app-server queue feature is disabled"))
        }
    }

    pub(crate) async fn thread_queue_add(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadQueueAddParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.require_enabled().await?;
        let thread_id = parse_queue_thread_id(params.thread_id.as_str())?;
        let thread = self
            .thread_manager
            .get_thread(thread_id)
            .await
            .map_err(|_| invalid_request(format!("thread not found: {thread_id}")))?;
        if thread.config_snapshot().await.ephemeral {
            return Err(invalid_request(format!(
                "ephemeral thread does not support queued turns: {thread_id}"
            )));
        }
        let submission = TurnSubmission::from(params.submission);
        TurnRequestProcessor::validate_v2_input_limit(&submission.input)?;
        let payload = serde_json::to_vec(&submission).map_err(|err| {
            internal_error(format!("failed to serialize queued turn payload: {err}"))
        })?;
        let record = self
            .state_db()?
            .thread_queue()
            .append_thread_queued_turn(thread_id, payload.as_slice())
            .await
            .map_err(|err| internal_error(format!("failed to add queued turn: {err}")))?;
        let queued_turn = queued_turn_from_state(record)?;
        self.outgoing
            .send_response(
                request_id,
                ThreadQueueAddResponse {
                    queued_turn: queued_turn.clone(),
                },
            )
            .await;
        self.emit_thread_queue_changed(thread_id).await;
        Ok(None)
    }

    pub(crate) async fn thread_queue_list(
        &self,
        params: ThreadQueueListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.require_enabled().await?;
        let thread_id = parse_queue_thread_id(params.thread_id.as_str())?;
        let start = match params.cursor {
            Some(cursor) => cursor
                .parse::<usize>()
                .map_err(|_| invalid_request(format!("invalid cursor: {cursor}")))?,
            None => 0,
        };
        let limit = params
            .limit
            .unwrap_or(THREAD_QUEUE_LIST_DEFAULT_LIMIT as u32)
            .clamp(1, THREAD_QUEUE_LIST_MAX_LIMIT as u32) as usize;
        let records = self
            .state_db()?
            .thread_queue()
            .list_visible_thread_queued_turns_page(thread_id, start, limit.saturating_add(1))
            .await
            .map_err(|err| internal_error(format!("failed to read queued turns: {err}")))?;
        let has_next_page = records.len() > limit;
        let data = records
            .into_iter()
            .take(limit)
            .map(queued_turn_from_state)
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = has_next_page.then(|| start.saturating_add(limit).to_string());
        Ok(Some(ThreadQueueListResponse { data, next_cursor }.into()))
    }

    pub(crate) async fn thread_queue_delete(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadQueueDeleteParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.require_enabled().await?;
        let thread_id = parse_queue_thread_id(params.thread_id.as_str())?;
        let deleted = self
            .state_db()?
            .thread_queue()
            .delete_thread_queued_turn(thread_id, params.queued_turn_id.as_str())
            .await
            .map_err(|err| internal_error(format!("failed to delete queued turn: {err}")))?;
        self.outgoing
            .send_response(request_id, ThreadQueueDeleteResponse { deleted })
            .await;
        if deleted {
            self.emit_thread_queue_changed(thread_id).await;
        }
        Ok(None)
    }

    pub(crate) async fn thread_queue_reorder(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadQueueReorderParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.require_enabled().await?;
        let thread_id = parse_queue_thread_id(params.thread_id.as_str())?;
        let records = self
            .state_db()?
            .thread_queue()
            .reorder_thread_queued_turns(thread_id, params.queued_turn_ids.as_slice())
            .await
            .map_err(|err| invalid_request(format!("failed to reorder queued turns: {err}")))?;
        let queued_turns = records
            .into_iter()
            .map(queued_turn_from_state)
            .collect::<Result<Vec<_>, _>>()?;
        self.outgoing
            .send_response(
                request_id,
                ThreadQueueReorderResponse {
                    queued_turns: queued_turns.clone(),
                },
            )
            .await;
        self.send_thread_queue_changed(thread_id, queued_turns)
            .await;
        Ok(None)
    }

    pub(crate) async fn emit_resume_queue_snapshot(&self, thread_id: ThreadId) {
        if self.require_enabled().await.is_err() {
            return;
        }
        self.emit_thread_queue_changed(thread_id).await;
    }

    async fn emit_thread_queue_changed(&self, thread_id: ThreadId) {
        match self.list_visible_queued_turns(thread_id).await {
            Ok(queued_turns) => {
                self.send_thread_queue_changed(thread_id, queued_turns)
                    .await;
            }
            Err(err) => {
                tracing::warn!("failed to read queue snapshot for thread {thread_id}: {err:?}");
            }
        }
    }

    async fn send_thread_queue_changed(&self, thread_id: ThreadId, queued_turns: Vec<QueuedTurn>) {
        let subscribed_connection_ids = self
            .thread_state_manager
            .subscribed_connection_ids(thread_id)
            .await;
        self.outgoing
            .send_server_notification_to_connections(
                subscribed_connection_ids.as_slice(),
                ServerNotification::ThreadQueueChanged(ThreadQueueChangedNotification {
                    thread_id: thread_id.to_string(),
                    queued_turns,
                }),
            )
            .await;
    }

    async fn list_visible_queued_turns(
        &self,
        thread_id: ThreadId,
    ) -> Result<Vec<QueuedTurn>, JSONRPCErrorError> {
        self.state_db()?
            .thread_queue()
            .list_visible_thread_queued_turns_page(
                thread_id,
                /*offset*/ 0,
                THREAD_QUEUE_LIST_MAX_LIMIT,
            )
            .await
            .map_err(|err| internal_error(format!("failed to read queued turns: {err}")))?
            .into_iter()
            .map(queued_turn_from_state)
            .collect()
    }
}

fn parse_queue_thread_id(thread_id: &str) -> Result<ThreadId, JSONRPCErrorError> {
    ThreadId::from_string(thread_id)
        .map_err(|err| invalid_request(format!("invalid thread id: {err}")))
}

fn queued_turn_from_state(
    record: codex_state::ThreadQueuedTurn,
) -> Result<QueuedTurn, JSONRPCErrorError> {
    let submission = serde_json::from_slice(record.turn_submission_jsonb.as_slice())
        .map_err(|err| internal_error(format!("failed to read queued turn payload: {err}")))?;
    let status = match record.state {
        codex_state::ThreadQueuedTurnState::Pending => QueuedTurnStatus::Pending,
        codex_state::ThreadQueuedTurnState::Failed => {
            let error = record
                .failure_jsonb
                .as_deref()
                .map(serde_json::from_slice)
                .transpose()
                .map_err(|err| {
                    internal_error(format!("failed to read queued turn failure: {err}"))
                })?
                .unwrap_or_else(|| turn_error("queued turn dispatch failed"));
            QueuedTurnStatus::Failed { error }
        }
        codex_state::ThreadQueuedTurnState::Dispatching => {
            return Err(internal_error(
                "dispatching queued turns are not client-visible",
            ));
        }
    };
    Ok(QueuedTurn {
        id: record.queued_turn_id,
        submission,
        status,
    })
}

fn turn_error(message: impl Into<String>) -> TurnError {
    TurnError {
        message: message.into(),
        codex_error_info: None,
        additional_details: None,
    }
}
