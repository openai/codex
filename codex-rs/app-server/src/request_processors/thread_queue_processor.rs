use super::*;

const THREAD_QUEUE_LIST_DEFAULT_LIMIT: usize = 25;
const THREAD_QUEUE_LIST_MAX_LIMIT: usize = 100;

#[derive(Clone)]
pub(crate) struct ThreadQueueRequestProcessor {
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    state_db: Option<StateDbHandle>,
    thread_state_manager: ThreadStateManager,
    turn_processor: TurnRequestProcessor,
}

impl ThreadQueueRequestProcessor {
    pub(crate) fn new(
        thread_manager: Arc<ThreadManager>,
        outgoing: Arc<OutgoingMessageSender>,
        state_db: Option<StateDbHandle>,
        thread_state_manager: ThreadStateManager,
        turn_processor: TurnRequestProcessor,
    ) -> Self {
        Self {
            thread_manager,
            outgoing,
            state_db,
            thread_state_manager,
            turn_processor,
        }
    }

    fn state_db(&self) -> Result<&StateDbHandle, JSONRPCErrorError> {
        self.state_db
            .as_ref()
            .ok_or_else(|| internal_error("queued turns require the app-server state db"))
    }

    pub(crate) async fn thread_queue_add(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadQueueAddParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let thread_id = parse_queue_thread_id(params.thread_id.as_str())?;
        if params.thread_id != params.turn_start_params.thread_id {
            return Err(invalid_request(
                "`threadId` must match `turnStartParams.threadId`",
            ));
        }
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
        TurnRequestProcessor::validate_v2_input_limit(&params.turn_start_params.input)?;
        let payload = serde_json::to_vec(&params.turn_start_params).map_err(|err| {
            internal_error(format!("failed to serialize queued turn payload: {err}"))
        })?;
        let record = self
            .state_db()?
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
        self.drain_thread_queue_if_idle(thread_id).await;
        Ok(None)
    }

    pub(crate) async fn thread_queue_list(
        &self,
        params: ThreadQueueListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
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
        let thread_id = parse_queue_thread_id(params.thread_id.as_str())?;
        let deleted = self
            .state_db()?
            .delete_thread_queued_turn(thread_id, params.queued_turn_id.as_str())
            .await
            .map_err(|err| internal_error(format!("failed to delete queued turn: {err}")))?;
        self.outgoing
            .send_response(request_id, ThreadQueueDeleteResponse { deleted })
            .await;
        if deleted {
            self.emit_thread_queue_changed(thread_id).await;
            self.drain_thread_queue_if_idle(thread_id).await;
        }
        Ok(None)
    }

    pub(crate) async fn thread_queue_reorder(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadQueueReorderParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let thread_id = parse_queue_thread_id(params.thread_id.as_str())?;
        let records = self
            .state_db()?
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
        self.drain_thread_queue_if_idle(thread_id).await;
        Ok(None)
    }

    pub(crate) async fn recover_resume_queue_snapshot_and_drain(&self, thread_id: ThreadId) {
        let Some(state_db) = self.state_db.as_ref() else {
            return;
        };
        let failure = turn_error("queued turn dispatch was interrupted while app-server restarted");
        let failure_json = match serde_json::to_vec(&failure) {
            Ok(failure_json) => failure_json,
            Err(err) => {
                tracing::warn!("failed to serialize queued turn recovery failure: {err}");
                return;
            }
        };
        match state_db
            .recover_dispatching_thread_queued_turns(thread_id, failure_json.as_slice())
            .await
        {
            Ok(_) => {}
            Err(err) => {
                tracing::warn!("failed to recover queued turns for thread {thread_id}: {err}");
                return;
            }
        }
        self.emit_thread_queue_changed(thread_id).await;
        self.drain_thread_queue_if_idle(thread_id).await;
    }

    pub(crate) async fn emit_resume_queue_snapshot_and_drain(&self, thread_id: ThreadId) {
        self.emit_thread_queue_changed(thread_id).await;
        self.drain_thread_queue_if_idle(thread_id).await;
    }

    pub(crate) async fn drain_thread_queue_after_terminal_turn(&self, thread_id: ThreadId) {
        self.drain_thread_queue_if_idle(thread_id).await;
    }

    pub(crate) async fn complete_dispatch_after_turn_started(
        &self,
        thread_id: ThreadId,
        turn_id: &str,
    ) {
        let Some(state_db) = self.state_db.as_ref() else {
            return;
        };
        match state_db
            .remove_dispatching_thread_queued_turn(thread_id, turn_id)
            .await
        {
            Ok(true) | Ok(false) => {}
            Err(err) => {
                tracing::warn!(
                    "failed to clear queued dispatch claim for thread {thread_id}: {err}"
                );
            }
        }
    }

    async fn drain_thread_queue_if_idle(&self, thread_id: ThreadId) {
        let Some(state_db) = self.state_db.as_ref() else {
            return;
        };
        let Ok(thread) = self.thread_manager.get_thread(thread_id).await else {
            return;
        };
        if matches!(thread.agent_status().await, AgentStatus::Running) {
            return;
        }
        let thread_state = self.thread_state_manager.thread_state(thread_id).await;
        {
            let thread_state = thread_state.lock().await;
            if thread_state.active_turn_snapshot().is_some()
                || !matches!(
                    thread_state.pending_turn_starts,
                    crate::thread_state::PendingTurnStarts::None
                )
            {
                return;
            }
        }
        let record = match state_db.claim_head_thread_queued_turn(thread_id).await {
            Ok(Some(record)) => record,
            Ok(None) => return,
            Err(err) => {
                tracing::warn!("failed to claim queued turn for thread {thread_id}: {err}");
                return;
            }
        };
        self.emit_thread_queue_changed(thread_id).await;
        let params = match serde_json::from_slice::<TurnStartParams>(
            record.turn_start_params_jsonb.as_slice(),
        ) {
            Ok(params) => params,
            Err(err) => {
                self.fail_dispatch(
                    thread_id,
                    record.queued_turn_id.as_str(),
                    turn_error(format!("queued turn payload could not be read: {err}")),
                )
                .await;
                return;
            }
        };
        match self.turn_processor.queued_turn_start(params).await {
            Ok(response) => {
                let turn_id = response.turn.id;
                match state_db
                    .set_dispatching_thread_queued_turn_turn_id(
                        record.queued_turn_id.as_str(),
                        turn_id.as_str(),
                    )
                    .await
                {
                    Ok(true) => {}
                    Ok(false) => {
                        tracing::warn!(
                            "queued turn {} lost its dispatch claim before turn {turn_id} was recorded",
                            record.queued_turn_id
                        );
                        return;
                    }
                    Err(err) => {
                        tracing::warn!(
                            "failed to record dispatch turn {turn_id} for queued turn {}: {err}",
                            record.queued_turn_id
                        );
                        return;
                    }
                }
                let thread_state = self.thread_state_manager.thread_state(thread_id).await;
                let should_complete_dispatch = {
                    let thread_state = thread_state.lock().await;
                    thread_state
                        .active_turn_snapshot()
                        .is_some_and(|turn| turn.id == turn_id)
                        || thread_state.last_terminal_turn_id.as_deref() == Some(turn_id.as_str())
                };
                if should_complete_dispatch {
                    self.complete_dispatch_after_turn_started(thread_id, turn_id.as_str())
                        .await;
                }
            }
            Err(err) => {
                self.fail_dispatch(
                    thread_id,
                    record.queued_turn_id.as_str(),
                    turn_error(format!(
                        "queued turn could not start: {message}",
                        message = err.message
                    )),
                )
                .await;
            }
        }
    }

    async fn fail_dispatch(&self, thread_id: ThreadId, queued_turn_id: &str, error: TurnError) {
        let Some(state_db) = self.state_db.as_ref() else {
            return;
        };
        let failure_json = match serde_json::to_vec(&error) {
            Ok(failure_json) => failure_json,
            Err(err) => {
                tracing::warn!("failed to serialize queued turn failure: {err}");
                return;
            }
        };
        match state_db
            .mark_thread_queued_turn_failed(queued_turn_id, failure_json.as_slice())
            .await
        {
            Ok(true) => self.emit_thread_queue_changed(thread_id).await,
            Ok(false) => tracing::warn!(
                "queued turn {queued_turn_id} could not be marked failed because its dispatch claim disappeared"
            ),
            Err(err) => tracing::warn!("failed to mark queued turn {queued_turn_id} failed: {err}"),
        }
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
            .list_visible_thread_queued_turns(thread_id)
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
    let turn_start_params = serde_json::from_slice(record.turn_start_params_jsonb.as_slice())
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
        turn_start_params,
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
