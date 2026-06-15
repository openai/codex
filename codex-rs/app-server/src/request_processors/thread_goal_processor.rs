use super::*;
use codex_goal_extension::GoalObjectiveUpdate;
use codex_goal_extension::GoalService;
use codex_goal_extension::GoalServiceError;
use codex_goal_extension::GoalSetRequest;
use codex_goal_extension::GoalTokenBudgetUpdate;

#[derive(Clone)]
pub(crate) struct ThreadGoalRequestProcessor {
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    config: Arc<Config>,
    thread_state_manager: ThreadStateManager,
    state_db: Option<StateDbHandle>,
    goal_service: Arc<GoalService>,
}

impl ThreadGoalRequestProcessor {
    pub(crate) fn new(
        thread_manager: Arc<ThreadManager>,
        outgoing: Arc<OutgoingMessageSender>,
        config: Arc<Config>,
        thread_state_manager: ThreadStateManager,
        state_db: Option<StateDbHandle>,
        goal_service: Arc<GoalService>,
    ) -> Self {
        Self {
            thread_manager,
            outgoing,
            config,
            thread_state_manager,
            state_db,
            goal_service,
        }
    }

    pub(crate) async fn thread_goal_set(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadGoalSetParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.thread_goal_set_inner(request_id, params)
            .await
            .map(|()| None)
    }

    pub(crate) async fn thread_goal_get(
        &self,
        params: ThreadGoalGetParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.thread_goal_get_inner(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn thread_goal_clear(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadGoalClearParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.thread_goal_clear_inner(request_id, params)
            .await
            .map(|()| None)
    }

    pub(crate) async fn emit_resume_goal_snapshot(&self, thread_id: ThreadId) {
        if !self.config.features.enabled(Feature::Goals) {
            return;
        }
        self.emit_thread_goal_snapshot(thread_id).await;
    }

    pub(crate) async fn pending_resume_goal_state(
        &self,
        thread: &CodexThread,
    ) -> (bool, Option<StateDbHandle>) {
        let emit_thread_goal_update = self.config.features.enabled(Feature::Goals);
        let thread_goal_state_db = if emit_thread_goal_update {
            if let Some(state_db) = thread.state_db() {
                Some(state_db)
            } else {
                self.state_db.clone()
            }
        } else {
            None
        };
        (emit_thread_goal_update, thread_goal_state_db)
    }

    async fn thread_goal_set_inner(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadGoalSetParams,
    ) -> Result<(), JSONRPCErrorError> {
        if !self.config.features.enabled(Feature::Goals) {
            return Err(invalid_request("goals feature is disabled"));
        }

        let thread_id = parse_thread_id_for_request(params.thread_id.as_str())?;
        let state_db = self.state_db_for_materialized_thread(thread_id).await?;
        self.reconcile_thread_goal_rollout(thread_id, &state_db)
            .await?;

        let listener_command_tx = {
            let thread_state = self.thread_state_manager.thread_state(thread_id).await;
            let thread_state = thread_state.lock().await;
            thread_state.listener_command_tx()
        };
        let status = params.status.map(ThreadGoalStatus::to_core);
        let objective = params.objective.as_deref();

        let outcome = self
            .goal_service
            .set_thread_goal(
                &state_db,
                GoalSetRequest {
                    thread_id,
                    objective: objective
                        .map(GoalObjectiveUpdate::Set)
                        .unwrap_or(GoalObjectiveUpdate::Keep),
                    status,
                    token_budget: match params.token_budget {
                        Some(token_budget) => GoalTokenBudgetUpdate::Set(token_budget),
                        None => GoalTokenBudgetUpdate::Keep,
                    },
                },
            )
            .await
            .map_err(goal_service_error)?;
        let goal = ThreadGoal::from(outcome.goal.clone());
        self.outgoing
            .send_response(
                request_id.clone(),
                ThreadGoalSetResponse { goal: goal.clone() },
            )
            .await;
        self.emit_thread_goal_updated_ordered(thread_id, goal, listener_command_tx)
            .await;
        outcome.apply_runtime_effects(&self.goal_service).await;
        Ok(())
    }

    async fn thread_goal_get_inner(
        &self,
        params: ThreadGoalGetParams,
    ) -> Result<ThreadGoalGetResponse, JSONRPCErrorError> {
        if !self.config.features.enabled(Feature::Goals) {
            return Err(invalid_request("goals feature is disabled"));
        }

        let thread_id = parse_thread_id_for_request(params.thread_id.as_str())?;
        let state_db = self.state_db_for_materialized_thread(thread_id).await?;
        let goal = self
            .goal_service
            .get_thread_goal(&state_db, thread_id)
            .await
            .map_err(goal_service_error)?
            .map(ThreadGoal::from);
        Ok(ThreadGoalGetResponse { goal })
    }

    async fn thread_goal_clear_inner(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadGoalClearParams,
    ) -> Result<(), JSONRPCErrorError> {
        if !self.config.features.enabled(Feature::Goals) {
            return Err(invalid_request("goals feature is disabled"));
        }

        let thread_id = parse_thread_id_for_request(params.thread_id.as_str())?;
        let state_db = self.state_db_for_materialized_thread(thread_id).await?;
        self.reconcile_thread_goal_rollout(thread_id, &state_db)
            .await?;

        let listener_command_tx = {
            let thread_state = self.thread_state_manager.thread_state(thread_id).await;
            let thread_state = thread_state.lock().await;
            thread_state.listener_command_tx()
        };
        let cleared = self
            .goal_service
            .clear_thread_goal(&state_db, thread_id)
            .await
            .map_err(goal_service_error)?;

        self.outgoing
            .send_response(request_id, ThreadGoalClearResponse { cleared })
            .await;
        if cleared {
            self.emit_thread_goal_cleared_ordered(thread_id, listener_command_tx)
                .await;
        }
        Ok(())
    }

    async fn state_db_for_materialized_thread(
        &self,
        thread_id: ThreadId,
    ) -> Result<StateDbHandle, JSONRPCErrorError> {
        if let Ok(thread) = self.thread_manager.get_thread(thread_id).await {
            if thread.rollout_path().is_none() {
                return Err(invalid_request(format!(
                    "ephemeral thread does not support goals: {thread_id}"
                )));
            }
            if let Some(state_db) = thread.state_db() {
                return Ok(state_db);
            }
        } else {
            codex_rollout::find_thread_path_by_id_str(
                &self.config.codex_home,
                &thread_id.to_string(),
                self.state_db.as_deref(),
            )
            .await
            .map_err(|err| {
                internal_error(format!("failed to locate thread id {thread_id}: {err}"))
            })?
            .ok_or_else(|| invalid_request(format!("thread not found: {thread_id}")))?;
        }

        self.state_db
            .clone()
            .ok_or_else(|| internal_error("sqlite state db unavailable for thread goals"))
    }

    async fn reconcile_thread_goal_rollout(
        &self,
        thread_id: ThreadId,
        state_db: &StateDbHandle,
    ) -> Result<(), JSONRPCErrorError> {
        let running_thread = self.thread_manager.get_thread(thread_id).await.ok();
        let rollout_path = match running_thread.as_ref() {
            Some(thread) => thread.rollout_path().ok_or_else(|| {
                invalid_request(format!(
                    "ephemeral thread does not support goals: {thread_id}"
                ))
            })?,
            None => codex_rollout::find_thread_path_by_id_str(
                &self.config.codex_home,
                &thread_id.to_string(),
                self.state_db.as_deref(),
            )
            .await
            .map_err(|err| {
                internal_error(format!("failed to locate thread id {thread_id}: {err}"))
            })?
            .ok_or_else(|| invalid_request(format!("thread not found: {thread_id}")))?,
        };
        reconcile_rollout(
            Some(state_db),
            rollout_path.as_path(),
            self.config.model_provider_id.as_str(),
            /*builder*/ None,
            &[],
            /*archived_only*/ None,
            /*new_thread_memory_mode*/ None,
        )
        .await;
        Ok(())
    }

    async fn emit_thread_goal_snapshot(&self, thread_id: ThreadId) {
        let state_db = match self.state_db_for_materialized_thread(thread_id).await {
            Ok(state_db) => state_db,
            Err(err) => {
                warn!(
                    "failed to open state db before emitting thread goal resume snapshot for {thread_id}: {}",
                    err.message
                );
                return;
            }
        };
        let listener_command_tx = {
            let thread_state = self.thread_state_manager.thread_state(thread_id).await;
            let thread_state = thread_state.lock().await;
            thread_state.listener_command_tx()
        };
        if let Some(listener_command_tx) = listener_command_tx {
            let command = crate::thread_state::ThreadListenerCommand::EmitThreadGoalSnapshot {
                state_db: state_db.clone(),
            };
            if listener_command_tx.send(command).is_ok() {
                return;
            }
            warn!(
                "failed to enqueue thread goal snapshot for {thread_id}: listener command channel is closed"
            );
        }
        send_thread_goal_snapshot_notification(&self.outgoing, thread_id, &state_db).await;
    }

    async fn emit_thread_goal_updated_ordered(
        &self,
        thread_id: ThreadId,
        goal: ThreadGoal,
        listener_command_tx: Option<tokio::sync::mpsc::UnboundedSender<ThreadListenerCommand>>,
    ) {
        if let Some(listener_command_tx) = listener_command_tx {
            let command = crate::thread_state::ThreadListenerCommand::EmitThreadGoalUpdated {
                turn_id: None,
                goal: goal.clone(),
            };
            if listener_command_tx.send(command).is_ok() {
                return;
            }
            warn!(
                "failed to enqueue thread goal update for {thread_id}: listener command channel is closed"
            );
        }
        self.outgoing
            .send_server_notification(ServerNotification::ThreadGoalUpdated(
                ThreadGoalUpdatedNotification {
                    thread_id: thread_id.to_string(),
                    turn_id: None,
                    goal,
                },
            ))
            .await;
    }

    async fn emit_thread_goal_cleared_ordered(
        &self,
        thread_id: ThreadId,
        listener_command_tx: Option<tokio::sync::mpsc::UnboundedSender<ThreadListenerCommand>>,
    ) {
        if let Some(listener_command_tx) = listener_command_tx {
            let command = crate::thread_state::ThreadListenerCommand::EmitThreadGoalCleared;
            if listener_command_tx.send(command).is_ok() {
                return;
            }
            warn!(
                "failed to enqueue thread goal clear for {thread_id}: listener command channel is closed"
            );
        }
        self.outgoing
            .send_server_notification(ServerNotification::ThreadGoalCleared(
                ThreadGoalClearedNotification {
                    thread_id: thread_id.to_string(),
                },
            ))
            .await;
    }
}

pub(super) fn api_thread_goal_from_state(goal: codex_state::ThreadGoal) -> ThreadGoal {
    ThreadGoal {
        thread_id: goal.thread_id.to_string(),
        objective: goal.objective,
        status: api_thread_goal_status_from_state(goal.status),
        token_budget: goal.token_budget,
        tokens_used: goal.tokens_used,
        time_used_seconds: goal.time_used_seconds,
        created_at: goal.created_at.timestamp(),
        updated_at: goal.updated_at.timestamp(),
    }
}

fn api_thread_goal_status_from_state(status: codex_state::ThreadGoalStatus) -> ThreadGoalStatus {
    match status {
        codex_state::ThreadGoalStatus::Active => ThreadGoalStatus::Active,
        codex_state::ThreadGoalStatus::Paused => ThreadGoalStatus::Paused,
        codex_state::ThreadGoalStatus::Blocked => ThreadGoalStatus::Blocked,
        codex_state::ThreadGoalStatus::UsageLimited => ThreadGoalStatus::UsageLimited,
        codex_state::ThreadGoalStatus::BudgetLimited => ThreadGoalStatus::BudgetLimited,
        codex_state::ThreadGoalStatus::Complete => ThreadGoalStatus::Complete,
    }
}

fn goal_service_error(err: GoalServiceError) -> JSONRPCErrorError {
    match err {
        GoalServiceError::InvalidRequest(message) => invalid_request(message),
        GoalServiceError::Internal(message) => internal_error(message),
    }
}

fn parse_thread_id_for_request(thread_id: &str) -> Result<ThreadId, JSONRPCErrorError> {
    ThreadId::from_string(thread_id)
        .map_err(|err| invalid_request(format!("invalid thread id: {err}")))
}
