use super::*;
use codex_protocol::protocol::validate_thread_goal_objective;

#[derive(Clone)]
pub(crate) struct ThreadGoalRequestProcessor {
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    config: Arc<Config>,
    thread_state_manager: ThreadStateManager,
    state_db: Option<StateDbHandle>,
}

impl ThreadGoalRequestProcessor {
    pub(crate) fn new(
        thread_manager: Arc<ThreadManager>,
        outgoing: Arc<OutgoingMessageSender>,
        config: Arc<Config>,
        thread_state_manager: ThreadStateManager,
        state_db: Option<StateDbHandle>,
    ) -> Self {
        Self {
            thread_manager,
            outgoing,
            config,
            thread_state_manager,
            state_db,
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

    pub(crate) async fn emit_resume_goal_snapshot_and_continue(
        &self,
        thread_id: ThreadId,
        thread: &CodexThread,
    ) {
        if !self.config.features.enabled(Feature::Goals) {
            return;
        }
        self.emit_thread_goal_snapshot(thread_id).await;
        // App-server owns resume response and snapshot ordering, so wait until
        // those are sent before letting core start goal continuation.
        if let Err(err) = thread.continue_active_goal_if_idle().await {
            tracing::warn!("failed to continue active goal after resume: {err}");
        }
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
        let running_thread = self.thread_manager.get_thread(thread_id).await.ok();
        let rollout_path = match running_thread.as_ref() {
            Some(thread) => thread.rollout_path().ok_or_else(|| {
                invalid_request(format!(
                    "ephemeral thread does not support goals: {thread_id}"
                ))
            })?,
            None => find_thread_path_by_id_str(
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
            Some(&state_db),
            rollout_path.as_path(),
            self.config.model_provider_id.as_str(),
            /*builder*/ None,
            &[],
            /*archived_only*/ None,
            /*new_thread_memory_mode*/ None,
        )
        .await;

        let listener_command_tx = {
            let thread_state = self.thread_state_manager.thread_state(thread_id).await;
            let thread_state = thread_state.lock().await;
            thread_state.listener_command_tx()
        };
        let status = params.status.map(thread_goal_status_to_state);
        let objective = params.objective.as_deref().map(str::trim);

        if let Some(objective) = objective {
            validate_thread_goal_objective(objective).map_err(invalid_request)?;
        }
        if objective.is_some() || params.token_budget.is_some() {
            validate_goal_budget(params.token_budget.flatten()).map_err(invalid_request)?;
        }

        if let Some(thread) = running_thread.as_ref() {
            thread.prepare_external_goal_mutation().await;
        }

        let (goal, previous_status) = (if let Some(objective) = objective {
            let existing_goal = state_db
                .get_thread_goal(thread_id)
                .await
                .map_err(|err| invalid_request(err.to_string()))?;
            if let Some(goal) = existing_goal.as_ref().filter(|goal| {
                goal.objective == objective
                    && goal.status != codex_state::ThreadGoalStatus::Complete
            }) {
                let previous_status = ExternalGoalPreviousStatus::Existing(goal.status);
                state_db
                    .update_thread_goal(
                        thread_id,
                        codex_state::ThreadGoalUpdate {
                            status,
                            token_budget: params.token_budget,
                            expected_goal_id: Some(goal.goal_id.clone()),
                        },
                    )
                    .await
                    .and_then(|goal| {
                        goal.ok_or_else(|| {
                            anyhow::anyhow!(
                                "cannot update goal for thread {thread_id}: no goal exists"
                            )
                        })
                    })
                    .map(|goal| (goal, previous_status))
            } else {
                let previous_status = ExternalGoalPreviousStatus::NewGoal;
                state_db
                    .replace_thread_goal(
                        thread_id,
                        objective,
                        status.unwrap_or(codex_state::ThreadGoalStatus::Active),
                        params.token_budget.flatten(),
                    )
                    .await
                    .map(|goal| (goal, previous_status))
            }
        } else {
            let existing_goal = state_db
                .get_thread_goal(thread_id)
                .await
                .map_err(|err| invalid_request(err.to_string()))?;
            let Some(existing_goal) = existing_goal else {
                return Err(invalid_request(format!(
                    "cannot update goal for thread {thread_id}: no goal exists"
                )));
            };
            let previous_status = ExternalGoalPreviousStatus::Existing(existing_goal.status);
            state_db
                .update_thread_goal(
                    thread_id,
                    codex_state::ThreadGoalUpdate {
                        status,
                        token_budget: params.token_budget,
                        expected_goal_id: None,
                    },
                )
                .await
                .and_then(|goal| {
                    goal.ok_or_else(|| {
                        anyhow::anyhow!("cannot update goal for thread {thread_id}: no goal exists")
                    })
                })
                .map(|goal| (goal, previous_status))
        })
        .map_err(|err| invalid_request(err.to_string()))?;
        let external_goal_set = ExternalGoalSet {
            goal: goal.clone(),
            previous_status,
        };
        let goal = api_thread_goal_from_state(goal);
        self.outgoing
            .send_response(
                request_id.clone(),
                ThreadGoalSetResponse { goal: goal.clone() },
            )
            .await;
        self.emit_thread_goal_updated_ordered(thread_id, goal, listener_command_tx)
            .await;
        if let Some(thread) = running_thread.as_ref() {
            thread.apply_external_goal_set(external_goal_set).await;
        }
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
        let goal = state_db
            .get_thread_goal(thread_id)
            .await
            .map_err(|err| internal_error(format!("failed to read thread goal: {err}")))?
            .map(api_thread_goal_from_state);
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
        let running_thread = self.thread_manager.get_thread(thread_id).await.ok();
        let rollout_path = match running_thread.as_ref() {
            Some(thread) => thread.rollout_path().ok_or_else(|| {
                invalid_request(format!(
                    "ephemeral thread does not support goals: {thread_id}"
                ))
            })?,
            None => find_thread_path_by_id_str(
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
            Some(&state_db),
            rollout_path.as_path(),
            self.config.model_provider_id.as_str(),
            /*builder*/ None,
            &[],
            /*archived_only*/ None,
            /*new_thread_memory_mode*/ None,
        )
        .await;

        if let Some(thread) = running_thread.as_ref() {
            thread.prepare_external_goal_mutation().await;
        }

        let listener_command_tx = {
            let thread_state = self.thread_state_manager.thread_state(thread_id).await;
            let thread_state = thread_state.lock().await;
            thread_state.listener_command_tx()
        };
        let cleared = state_db
            .delete_thread_goal(thread_id)
            .await
            .map_err(|err| internal_error(format!("failed to clear thread goal: {err}")))?;

        if cleared && let Some(thread) = running_thread.as_ref() {
            thread.apply_external_goal_clear().await;
        }

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
            find_thread_path_by_id_str(
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

fn validate_goal_budget(value: Option<i64>) -> Result<(), String> {
    if let Some(value) = value
        && value <= 0
    {
        return Err("goal budgets must be positive when provided".to_string());
    }
    Ok(())
}

fn thread_goal_status_to_state(status: ThreadGoalStatus) -> codex_state::ThreadGoalStatus {
    match status {
        ThreadGoalStatus::Active => codex_state::ThreadGoalStatus::Active,
        ThreadGoalStatus::Paused => codex_state::ThreadGoalStatus::Paused,
        ThreadGoalStatus::BudgetLimited => codex_state::ThreadGoalStatus::BudgetLimited,
        ThreadGoalStatus::Complete => codex_state::ThreadGoalStatus::Complete,
    }
}

fn thread_goal_status_from_state(status: codex_state::ThreadGoalStatus) -> ThreadGoalStatus {
    match status {
        codex_state::ThreadGoalStatus::Active => ThreadGoalStatus::Active,
        codex_state::ThreadGoalStatus::Paused => ThreadGoalStatus::Paused,
        codex_state::ThreadGoalStatus::BudgetLimited => ThreadGoalStatus::BudgetLimited,
        codex_state::ThreadGoalStatus::Complete => ThreadGoalStatus::Complete,
    }
}

pub(super) fn api_thread_goal_from_state(goal: codex_state::ThreadGoal) -> ThreadGoal {
    ThreadGoal {
        thread_id: goal.thread_id.to_string(),
        objective: goal.objective,
        status: thread_goal_status_from_state(goal.status),
        token_budget: goal.token_budget,
        tokens_used: goal.tokens_used,
        time_used_seconds: goal.time_used_seconds,
        created_at: goal.created_at.timestamp(),
        updated_at: goal.updated_at.timestamp(),
    }
}

fn parse_thread_id_for_request(thread_id: &str) -> Result<ThreadId, JSONRPCErrorError> {
    ThreadId::from_string(thread_id)
        .map_err(|err| invalid_request(format!("invalid thread id: {err}")))
}
