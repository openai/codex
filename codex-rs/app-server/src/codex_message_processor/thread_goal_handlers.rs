use super::*;

impl CodexMessageProcessor {
    pub(super) async fn thread_goal_set(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadGoalSetParams,
    ) {
        if !self.config.features.enabled(Feature::Goals) {
            self.send_invalid_request_error(request_id, "goals feature is disabled".to_string())
                .await;
            return;
        }

        let thread_id = match parse_thread_id_for_request(params.thread_id.as_str()) {
            Ok(thread_id) => thread_id,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };
        let state_db = match self.state_db_for_materialized_thread(thread_id).await {
            Ok(state_db) => state_db,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };
        let running_thread = self.thread_manager.get_thread(thread_id).await.ok();
        let rollout_path = match running_thread.as_ref() {
            Some(thread) => match thread.rollout_path() {
                Some(path) => path,
                None => {
                    self.send_invalid_request_error(
                        request_id,
                        format!("ephemeral thread does not support goals: {thread_id}"),
                    )
                    .await;
                    return;
                }
            },
            None => {
                match find_thread_path_by_id_str(&self.config.codex_home, &thread_id.to_string())
                    .await
                {
                    Ok(Some(path)) => path,
                    Ok(None) => {
                        self.send_invalid_request_error(
                            request_id,
                            format!("thread not found: {thread_id}"),
                        )
                        .await;
                        return;
                    }
                    Err(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to locate thread id {thread_id}: {err}"),
                        )
                        .await;
                        return;
                    }
                }
            }
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
        let status = params.status.map(thread_goal_status_to_state);

        let goal = if let Some(objective) = params.objective {
            let objective = objective.trim();
            if objective.is_empty() {
                self.send_invalid_request_error(
                    request_id,
                    "goal objective must not be empty".to_string(),
                )
                .await;
                return;
            }
            if let Err(message) = validate_goal_budget(params.token_budget.flatten()) {
                self.send_invalid_request_error(request_id, message).await;
                return;
            }
            match state_db.get_thread_goal(thread_id).await {
                Ok(goal) => {
                    let same_incomplete_goal = goal.as_ref().is_some_and(|goal| {
                        goal.objective == objective
                            && goal.status != codex_state::ThreadGoalStatus::Complete
                    });
                    if same_incomplete_goal {
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
                                    anyhow::anyhow!(
                                        "cannot update goal for thread {thread_id}: no goal exists"
                                    )
                                })
                            })
                    } else {
                        state_db
                            .replace_thread_goal(
                                thread_id,
                                objective,
                                status.unwrap_or(codex_state::ThreadGoalStatus::Active),
                                params.token_budget.flatten(),
                            )
                            .await
                    }
                }
                Err(err) => Err(err),
            }
        } else {
            if let Some(token_budget) = params.token_budget
                && let Err(message) = validate_goal_budget(token_budget)
            {
                self.send_invalid_request_error(request_id, message).await;
                return;
            }
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
        };

        let goal = match goal {
            Ok(goal) => goal,
            Err(err) => {
                self.send_invalid_request_error(request_id, err.to_string())
                    .await;
                return;
            }
        };
        let goal_status = goal.status;
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
            thread.apply_external_goal_set(goal_status).await;
        }
    }

    pub(super) async fn thread_goal_get(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadGoalGetParams,
    ) {
        if !self.config.features.enabled(Feature::Goals) {
            self.send_invalid_request_error(request_id, "goals feature is disabled".to_string())
                .await;
            return;
        }

        let thread_id = match parse_thread_id_for_request(params.thread_id.as_str()) {
            Ok(thread_id) => thread_id,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };
        let state_db = match self.state_db_for_materialized_thread(thread_id).await {
            Ok(state_db) => state_db,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };
        let goal = match state_db.get_thread_goal(thread_id).await {
            Ok(goal) => goal.map(api_thread_goal_from_state),
            Err(err) => {
                self.send_internal_error(request_id, format!("failed to read thread goal: {err}"))
                    .await;
                return;
            }
        };
        self.outgoing
            .send_response(request_id, ThreadGoalGetResponse { goal })
            .await;
    }

    pub(super) async fn thread_goal_clear(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadGoalClearParams,
    ) {
        if !self.config.features.enabled(Feature::Goals) {
            self.send_invalid_request_error(request_id, "goals feature is disabled".to_string())
                .await;
            return;
        }

        let thread_id = match parse_thread_id_for_request(params.thread_id.as_str()) {
            Ok(thread_id) => thread_id,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };
        let state_db = match self.state_db_for_materialized_thread(thread_id).await {
            Ok(state_db) => state_db,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };
        let running_thread = self.thread_manager.get_thread(thread_id).await.ok();
        let rollout_path = match running_thread.as_ref() {
            Some(thread) => match thread.rollout_path() {
                Some(path) => path,
                None => {
                    self.send_invalid_request_error(
                        request_id,
                        format!("ephemeral thread does not support goals: {thread_id}"),
                    )
                    .await;
                    return;
                }
            },
            None => {
                match find_thread_path_by_id_str(&self.config.codex_home, &thread_id.to_string())
                    .await
                {
                    Ok(Some(path)) => path,
                    Ok(None) => {
                        self.send_invalid_request_error(
                            request_id,
                            format!("thread not found: {thread_id}"),
                        )
                        .await;
                        return;
                    }
                    Err(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to locate thread id {thread_id}: {err}"),
                        )
                        .await;
                        return;
                    }
                }
            }
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
        let cleared = match state_db.delete_thread_goal(thread_id).await {
            Ok(cleared) => cleared,
            Err(err) => {
                self.send_internal_error(request_id, format!("failed to clear thread goal: {err}"))
                    .await;
                return;
            }
        };

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
            match find_thread_path_by_id_str(&self.config.codex_home, &thread_id.to_string()).await
            {
                Ok(Some(_)) => {}
                Ok(None) => {
                    return Err(invalid_request(format!("thread not found: {thread_id}")));
                }
                Err(err) => {
                    return Err(internal_error(format!(
                        "failed to locate thread id {thread_id}: {err}"
                    )));
                }
            }
        }

        open_state_db_for_direct_thread_lookup(&self.config)
            .await
            .ok_or_else(|| internal_error("sqlite state db unavailable for thread goals"))
    }

    pub(super) async fn emit_thread_goal_snapshot(&self, thread_id: ThreadId) {
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
        match state_db.get_thread_goal(thread_id).await {
            Ok(Some(goal)) => {
                self.emit_thread_goal_updated_ordered(
                    thread_id,
                    api_thread_goal_from_state(goal),
                    listener_command_tx,
                )
                .await;
            }
            Ok(None) => {
                self.emit_thread_goal_cleared_ordered(thread_id, listener_command_tx)
                    .await;
            }
            Err(err) => {
                warn!(
                    "failed to read thread goal before emitting resume snapshot for {thread_id}: {err}"
                );
            }
        }
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
