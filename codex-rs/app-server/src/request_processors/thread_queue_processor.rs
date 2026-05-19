use super::*;

#[derive(Clone)]
pub(crate) struct ThreadQueueRequestProcessor {
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    state_db: StateDbHandle,
    thread_state_manager: ThreadStateManager,
    turn_processor: TurnRequestProcessor,
}

impl ThreadQueueRequestProcessor {
    pub(crate) fn new(
        thread_manager: Arc<ThreadManager>,
        outgoing: Arc<OutgoingMessageSender>,
        state_db: StateDbHandle,
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
        if self.thread_manager.get_thread(thread_id).await.is_err() {
            return Err(invalid_request(format!("thread not found: {thread_id}")));
        }
        TurnRequestProcessor::validate_v2_input_limit(&params.turn_start_params.input)?;
        let payload = serde_json::to_vec(&params.turn_start_params).map_err(|err| {
            internal_error(format!("failed to serialize queued turn payload: {err}"))
        })?;
        let record = self
            .state_db
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
        let queued_turns = self.list_visible_queued_turns(thread_id).await?;
        Ok(Some(ThreadQueueListResponse { queued_turns }.into()))
    }

    pub(crate) async fn thread_queue_delete(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadQueueDeleteParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let thread_id = parse_queue_thread_id(params.thread_id.as_str())?;
        let deleted = self
            .state_db
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
        let thread_id = parse_queue_thread_id(params.thread_id.as_str())?;
        let records = self
            .state_db
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

    pub(crate) async fn emit_resume_queue_snapshot_and_drain(&self, thread_id: ThreadId) {
        let failure = turn_error("queued turn dispatch was interrupted while app-server restarted");
        let failure_json = match serde_json::to_vec(&failure) {
            Ok(failure_json) => failure_json,
            Err(err) => {
                tracing::warn!("failed to serialize queued turn recovery failure: {err}");
                return;
            }
        };
        match self
            .state_db
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

    pub(crate) async fn drain_thread_queue_after_terminal_turn(&self, thread_id: ThreadId) {
        self.drain_thread_queue_if_idle(thread_id).await;
    }

    async fn drain_thread_queue_if_idle(&self, thread_id: ThreadId) {
        let Ok(thread) = self.thread_manager.get_thread(thread_id).await else {
            return;
        };
        if matches!(thread.agent_status().await, AgentStatus::Running) {
            return;
        }
        let thread_state = self.thread_state_manager.thread_state(thread_id).await;
        if thread_state.lock().await.active_turn_snapshot().is_some() {
            return;
        }
        let record = match self.state_db.claim_head_thread_queued_turn(thread_id).await {
            Ok(Some(record)) => record,
            Ok(None) => return,
            Err(err) => {
                tracing::warn!("failed to claim queued turn for thread {thread_id}: {err}");
                return;
            }
        };
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
        if let Err(err) = self.turn_processor.queued_turn_start(params).await {
            self.fail_dispatch(
                thread_id,
                record.queued_turn_id.as_str(),
                turn_error(format!(
                    "queued turn could not start: {message}",
                    message = err.message
                )),
            )
            .await;
            return;
        }
        match self
            .state_db
            .remove_dispatched_thread_queued_turn(record.queued_turn_id.as_str())
            .await
        {
            Ok(true) => self.emit_thread_queue_changed(thread_id).await,
            Ok(false) => tracing::warn!(
                "queued turn {} was accepted but its dispatch claim disappeared",
                record.queued_turn_id
            ),
            Err(err) => tracing::warn!(
                "failed to remove accepted queued turn {}: {err}",
                record.queued_turn_id
            ),
        }
    }

    async fn fail_dispatch(&self, thread_id: ThreadId, queued_turn_id: &str, error: TurnError) {
        let failure_json = match serde_json::to_vec(&error) {
            Ok(failure_json) => failure_json,
            Err(err) => {
                tracing::warn!("failed to serialize queued turn failure: {err}");
                return;
            }
        };
        match self
            .state_db
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
        self.outgoing
            .send_server_notification(ServerNotification::ThreadQueueChanged(
                ThreadQueueChangedNotification {
                    thread_id: thread_id.to_string(),
                    queued_turns,
                },
            ))
            .await;
    }

    async fn list_visible_queued_turns(
        &self,
        thread_id: ThreadId,
    ) -> Result<Vec<QueuedTurn>, JSONRPCErrorError> {
        self.state_db
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
