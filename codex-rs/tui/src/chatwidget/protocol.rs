use super::*;

impl ChatWidget {
    pub(crate) fn handle_server_notification(
        &mut self,
        notification: ServerNotification,
        replay_kind: Option<ReplayKind>,
    ) {
        // Reject misrouted child updates before shared notification handling mutates parent state.
        if let ServerNotification::McpServerStatusUpdated(notification) = &notification
            && let (Some(notification_thread_id), Some(thread_id)) =
                (notification.thread_id.as_deref(), self.thread_id())
            && notification_thread_id != thread_id.to_string()
        {
            return;
        }

        if replay_kind != Some(ReplayKind::ResumeInitialMessages)
            && !self.recover_resumed_reasoning(&notification)
        {
            return;
        }
        let was_replaying_turn_completion = self.thread_usage.replaying_turn_completion;
        self.thread_usage.replaying_turn_completion = replay_kind.is_some();
        let from_replay = replay_kind.is_some();
        let is_resume_initial_replay =
            matches!(replay_kind, Some(ReplayKind::ResumeInitialMessages));
        let is_retry_error = matches!(
            &notification,
            ServerNotification::Error(ErrorNotification {
                will_retry: true,
                ..
            })
        );
        if !is_resume_initial_replay && !is_retry_error {
            self.restore_retry_status_header_if_present();
        }
        match notification {
            ServerNotification::ThreadTokenUsageUpdated(notification) => {
                self.set_token_info(Some(token_usage_info_from_app_server(
                    notification.token_usage,
                )));
            }
            ServerNotification::ThreadNameUpdated(notification) => {
                match ThreadId::from_string(&notification.thread_id) {
                    Ok(thread_id) => {
                        self.on_thread_name_updated(thread_id, notification.thread_name)
                    }
                    Err(err) => {
                        tracing::warn!(
                            thread_id = notification.thread_id,
                            error = %err,
                            "ignoring app-server ThreadNameUpdated with invalid thread_id"
                        );
                    }
                }
            }
            ServerNotification::ThreadGoalUpdated(notification) => {
                self.on_thread_goal_updated(notification.goal, notification.turn_id);
            }
            ServerNotification::ThreadGoalCleared(notification) => {
                self.on_thread_goal_cleared(notification.thread_id.as_str());
            }
            ServerNotification::ThreadSettingsUpdated(notification) => {
                self.on_thread_settings_updated(notification);
            }
            ServerNotification::TurnStarted(notification) => {
                if from_replay {
                    self.restore_realtime_transcripts_before_turn(&notification.turn.id);
                } else {
                    self.anchor_realtime_transcripts_before_turn(&notification.turn.id);
                }
                if replay_kind.is_none() {
                    self.clear_misalignment_for_new_turn(
                        &notification.turn.id,
                        MisalignmentTurnSource::ServerNotification,
                    );
                }
                self.turn_lifecycle.last_turn_id = Some(notification.turn.id);
                self.last_non_retry_error = None;
                if !matches!(replay_kind, Some(ReplayKind::ResumeInitialMessages)) {
                    self.warning_display_state.startup_complete = true;
                    self.on_task_started();
                }
            }
            ServerNotification::TurnCompleted(notification) => {
                self.restore_realtime_transcripts_before_turn(&notification.turn.id);
                self.handle_turn_completed_notification(notification, replay_kind);
            }
            ServerNotification::ItemStarted(notification) => {
                self.handle_item_started_notification(notification, replay_kind);
            }
            ServerNotification::ItemCompleted(notification) => {
                self.handle_item_completed_notification(notification, replay_kind);
            }
            ServerNotification::AgentMessageDelta(notification) => {
                self.restore_realtime_transcripts_before_turn(&notification.turn_id);
                if !self.is_realtime_delegated_reasoning_turn(&notification.turn_id)
                    && (from_replay
                        || !self.is_realtime_delegated_agent_item(
                            &notification.turn_id,
                            &notification.item_id,
                        ))
                {
                    self.on_agent_message_delta(notification.delta);
                }
            }
            ServerNotification::PlanDelta(notification) => {
                self.restore_realtime_transcripts_before_turn(&notification.turn_id);
                self.on_plan_delta(notification.delta);
            }
            ServerNotification::ReasoningSummaryTextDelta(notification) => {
                if !self.is_realtime_delegated_reasoning_item(
                    &notification.turn_id,
                    &notification.item_id,
                ) && self.status_state.reasoning_item_id.as_deref()
                    == Some(&notification.item_id)
                {
                    self.on_agent_reasoning_delta(notification.delta);
                }
            }
            ServerNotification::ReasoningTextDelta(notification) => {
                if self.config.show_raw_agent_reasoning
                    && !self.is_realtime_delegated_reasoning_item(
                        &notification.turn_id,
                        &notification.item_id,
                    )
                    && self.status_state.reasoning_item_id.as_deref() == Some(&notification.item_id)
                {
                    self.on_agent_reasoning_delta(notification.delta);
                }
            }
            ServerNotification::ReasoningSummaryPartAdded(notification) => {
                if !self.is_realtime_delegated_reasoning_item(
                    &notification.turn_id,
                    &notification.item_id,
                ) && self.status_state.reasoning_item_id.as_deref()
                    == Some(&notification.item_id)
                {
                    self.on_reasoning_section_break();
                }
            }
            ServerNotification::TerminalInteraction(notification) => {
                self.on_terminal_interaction(notification.process_id, notification.stdin)
            }
            ServerNotification::CommandExecutionOutputDelta(notification) => {
                self.on_exec_command_output_delta(&notification.item_id, &notification.delta);
            }
            ServerNotification::FileChangeOutputDelta(notification) => {
                self.on_patch_apply_output_delta(notification.item_id, notification.delta);
            }
            ServerNotification::TurnDiffUpdated(notification) => {
                self.on_turn_diff(notification.diff)
            }
            ServerNotification::TurnPlanUpdated(notification) => {
                self.on_plan_update(UpdatePlanArgs {
                    explanation: notification.explanation,
                    plan: notification
                        .plan
                        .into_iter()
                        .map(|step| UpdatePlanItemArg {
                            step: step.step,
                            status: match step.status {
                                TurnPlanStepStatus::Pending => UpdatePlanItemStatus::Pending,
                                TurnPlanStepStatus::InProgress => UpdatePlanItemStatus::InProgress,
                                TurnPlanStepStatus::Completed => UpdatePlanItemStatus::Completed,
                            },
                        })
                        .collect(),
                })
            }
            ServerNotification::HookStarted(notification) => {
                self.on_hook_started(notification.run);
            }
            ServerNotification::HookCompleted(notification) => {
                self.on_hook_completed(notification.run);
            }
            ServerNotification::Error(notification) => {
                if notification.will_retry {
                    if !from_replay {
                        self.on_stream_error(
                            notification.error.message,
                            notification.error.additional_details,
                        );
                    }
                } else {
                    self.last_non_retry_error = Some((
                        notification.turn_id.clone(),
                        notification.error.message.clone(),
                    ));
                    if !from_replay
                        && notification.error.codex_error_info
                            == Some(AppServerCodexErrorInfo::MisalignmentPolicyViolation)
                    {
                        self.on_misalignment_error(
                            Some(notification.turn_id),
                            notification.error.misalignment,
                        );
                    } else {
                        self.handle_non_retry_error(
                            notification.error.message,
                            notification.error.codex_error_info,
                        );
                    }
                }
            }
            ServerNotification::SkillsChanged(_) => {
                self.refresh_skills_for_current_cwd(/*force_reload*/ true);
            }
            ServerNotification::ModelRerouted(_) => {}
            ServerNotification::ModelVerification(notification) => {
                self.on_app_server_model_verification(&notification.verifications)
            }
            ServerNotification::ModelSafetyBufferingUpdated(notification) => {
                self.on_model_safety_buffering_updated(notification, replay_kind)
            }
            ServerNotification::AuthRecoveryStarted(notification) => {
                self.add_info_message(notification.message, /*hint*/ None)
            }
            ServerNotification::AuthRecoveryCompleted(notification) => {
                self.add_plain_history_lines(vec![
                    vec!["✓ ".green(), notification.message.into()].into(),
                ]);
            }
            ServerNotification::Warning(notification) => self.on_warning(notification.message),
            ServerNotification::GuardianWarning(notification) => {
                if !notification
                    .message
                    .starts_with("Automatic approval review approved (")
                {
                    self.on_warning(notification.message);
                }
            }
            ServerNotification::StrictReviewRequired(_) => {
                self.app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                    history_cell::new_warning_event(
                        "This request requires additional safety checks, some tool calls might take extra time"
                            .to_string(),
                    ),
                )));
                self.request_redraw();
            }
            ServerNotification::DeprecationNotice(notification) => {
                self.on_deprecation_notice(notification.summary, notification.details)
            }
            ServerNotification::ConfigWarning(notification) => {
                let message = notification
                    .details
                    .map(|details| format!("{}: {details}", notification.summary))
                    .unwrap_or(notification.summary);
                if self.warning_display_state.startup_complete {
                    self.on_warning(message);
                } else if self.warning_display_state.should_display(&message) {
                    self.warning_display_state
                        .startup_config_warnings
                        .insert(message.clone());
                    self.add_to_history(history_cell::StartupWarningsCell::new(vec![message]));
                    self.request_redraw();
                }
            }
            ServerNotification::McpServerStatusUpdated(notification) => {
                self.on_mcp_server_status_updated(notification)
            }
            ServerNotification::ItemGuardianApprovalReviewStarted(notification) => {
                self.on_guardian_review_notification(
                    notification.review_id,
                    notification.turn_id,
                    notification.started_at_ms,
                    notification.review,
                    /*completion*/ None,
                    notification.action,
                );
            }
            ServerNotification::ItemGuardianApprovalReviewCompleted(notification) => {
                self.on_guardian_review_notification(
                    notification.review_id,
                    notification.turn_id,
                    notification.started_at_ms,
                    notification.review,
                    Some((notification.completed_at_ms, notification.decision_source)),
                    notification.action,
                );
            }
            ServerNotification::ThreadClosed(_) => {
                if !from_replay {
                    self.on_shutdown_complete();
                }
            }
            ServerNotification::ThreadRealtimeSdp(notification) => {
                if !from_replay {
                    self.on_realtime_conversation_sdp(notification.sdp);
                }
            }
            ServerNotification::ThreadRealtimeStarted(_) => {
                if !from_replay {
                    self.on_realtime_conversation_started();
                }
            }
            ServerNotification::ThreadRealtimeTranscriptDelta(notification) => {
                if !from_replay {
                    self.on_realtime_transcript_delta(notification.role, notification.delta);
                }
            }
            ServerNotification::ThreadRealtimeTranscriptDone(notification) => {
                if !from_replay {
                    self.on_realtime_transcript_done(notification.role, notification.text);
                }
            }
            ServerNotification::ThreadRealtimeError(notification) => {
                if !from_replay {
                    self.on_realtime_error(notification.message);
                }
            }
            ServerNotification::ThreadRealtimeClosed(notification) => {
                if !from_replay {
                    self.on_realtime_conversation_closed(notification.reason);
                }
            }
            ServerNotification::ServerRequestResolved(_)
            | ServerNotification::AccountUpdated(_)
            | ServerNotification::AccountRateLimitsUpdated(_)
            | ServerNotification::ThreadStarted(_)
            | ServerNotification::ThreadStatusChanged(_)
            | ServerNotification::ThreadReverted(_)
            | ServerNotification::ThreadQueueChanged(_)
            | ServerNotification::ThreadArchived(_)
            | ServerNotification::ThreadDeleted(_)
            | ServerNotification::ThreadUnarchived(_)
            | ServerNotification::ThreadAttachmentUpdated(_)
            | ServerNotification::RawResponseItemCompleted(_)
            | ServerNotification::RawResponseCompleted(_)
            | ServerNotification::CommandExecOutputDelta(_)
            | ServerNotification::ProcessOutputDelta(_)
            | ServerNotification::ProcessExited(_)
            | ServerNotification::McpServerEventStream(_)
            | ServerNotification::FileChangePatchUpdated(_)
            | ServerNotification::McpToolCallProgress(_)
            | ServerNotification::McpServerOauthLoginCompleted(_)
            | ServerNotification::AppListUpdated(_)
            | ServerNotification::EnvironmentConnected(_)
            | ServerNotification::EnvironmentDisconnected(_)
            | ServerNotification::RemoteControlStatusChanged(_)
            | ServerNotification::ExternalAgentConfigImportProgress(_)
            | ServerNotification::ExternalAgentConfigImportCompleted(_)
            | ServerNotification::FsChanged(_)
            | ServerNotification::TurnModerationMetadata(_)
            | ServerNotification::FuzzyFileSearchSessionUpdated(_)
            | ServerNotification::FuzzyFileSearchSessionCompleted(_)
            | ServerNotification::ThreadRealtimeItemAdded(_)
            | ServerNotification::ThreadRealtimeItemStarted(_)
            | ServerNotification::ThreadRealtimeItemTranscriptDelta(_)
            | ServerNotification::ThreadRealtimeItemCompleted(_)
            | ServerNotification::ThreadRealtimeOutputAudioDelta(_)
            | ServerNotification::WindowsWorldWritableWarning(_)
            | ServerNotification::WindowsSandboxSetupCompleted(_)
            | ServerNotification::AccountLoginCompleted(_)
            | ServerNotification::ProjectChanged(_)
            | ServerNotification::ThreadProjectUpdated(_) => {}
            ServerNotification::ContextCompacted(_) => {}
        }
        // Tool and hook activity can recreate a hidden row with its default
        // heading. Restore the selected status before that row is rendered.
        if self
            .bottom_pane
            .status_widget()
            .is_some_and(|status| status.header() != self.status_state.current_status.header)
        {
            self.bottom_pane.update_status(
                self.status_state.current_status.header.clone(),
                self.status_state.current_status.details.clone(),
                StatusDetailsCapitalization::Preserve,
                self.status_state.current_status.details_max_lines,
            );
        }
        self.thread_usage.replaying_turn_completion = was_replaying_turn_completion;
    }

    pub(super) fn handle_turn_completed_notification(
        &mut self,
        notification: TurnCompletedNotification,
        replay_kind: Option<ReplayKind>,
    ) {
        // User-message dedupe only suppresses the app-server echo of a prompt
        // this TUI already rendered locally. Once that turn ends, another
        // client can submit the same text and it still needs its own user cell.
        self.last_rendered_user_message_display = None;
        let was_replaying_turn_completion = self.thread_usage.replaying_turn_completion;
        self.thread_usage.replaying_turn_completion = replay_kind.is_some();
        match notification.turn.status {
            TurnStatus::Completed => {
                let last_agent_message =
                    notification
                        .turn
                        .items
                        .iter()
                        .rev()
                        .find_map(|item| match item {
                            ThreadItem::AgentMessage {
                                id,
                                text,
                                phase: Some(MessagePhase::FinalAnswer) | None,
                                ..
                            } if !self
                                .is_realtime_delegated_reasoning_turn(&notification.turn.id)
                                || !realtime::is_private_realtime_agent_item(item) =>
                            {
                                Some((item.clone(), id.clone(), text.clone()))
                            }
                            _ => None,
                        });
                if let Some((item, id, _)) = &last_agent_message
                    && self
                        .transcript
                        .last_completed_agent_message
                        .as_ref()
                        .is_none_or(|(turn_id, item_id)| {
                            turn_id != &notification.turn.id || item_id != id
                        })
                {
                    self.handle_thread_item(
                        item.clone(),
                        notification.turn.id.clone(),
                        replay_kind
                            .map_or(ThreadItemRenderSource::Live, ThreadItemRenderSource::Replay),
                    );
                }
                if replay_kind.is_none()
                    && let Some((item, _, _)) = &last_agent_message
                {
                    self.speak_completed_realtime_delegation(&notification.turn.id, item);
                }
                self.last_non_retry_error = None;
                let completion = self.completion_cell(&notification.turn, replay_kind);
                self.on_task_complete(
                    last_agent_message.map(|(_, _, text)| text),
                    completion,
                    replay_kind.is_some(),
                );
            }
            TurnStatus::Interrupted => {
                self.last_non_retry_error = None;
                let reason = if self
                    .turn_lifecycle
                    .take_budget_limited(notification.turn.id.as_str())
                {
                    TurnAbortReason::BudgetLimited
                } else {
                    TurnAbortReason::Interrupted
                };
                self.on_interrupted_turn(reason);
            }
            TurnStatus::Failed => {
                if let Some(error) = notification.turn.error {
                    if replay_kind.is_none()
                        && error.codex_error_info
                            == Some(AppServerCodexErrorInfo::MisalignmentPolicyViolation)
                    {
                        self.on_misalignment_error(
                            Some(notification.turn.id.clone()),
                            error.misalignment,
                        );
                        self.last_non_retry_error = None;
                    } else if self.last_non_retry_error.as_ref()
                        == Some(&(notification.turn.id.clone(), error.message.clone()))
                    {
                        self.last_non_retry_error = None;
                    } else {
                        self.handle_non_retry_error(error.message, error.codex_error_info);
                    }
                } else {
                    self.last_non_retry_error = None;
                    self.finalize_turn();
                    self.request_redraw();
                    self.maybe_send_next_queued_input();
                }
            }
            TurnStatus::InProgress => {}
        }
        if replay_kind.is_none() {
            self.finish_realtime_turn(&notification.turn.id);
        }
        self.thread_usage.replaying_turn_completion = was_replaying_turn_completion;
    }

    fn handle_item_started_notification(
        &mut self,
        notification: ItemStartedNotification,
        replay_kind: Option<ReplayKind>,
    ) {
        self.restore_realtime_transcripts_before_turn(&notification.turn_id);
        match notification.item {
            ThreadItem::UserMessage { content, .. } if replay_kind.is_none() => {
                self.note_realtime_user_item_started(&notification.turn_id, &content);
            }
            ThreadItem::UserMessage { content, .. }
                if realtime::realtime_delegation_input(&content).is_some() =>
            {
                self.remember_realtime_delegated_reasoning_turn(&notification.turn_id);
            }
            ThreadItem::AgentMessage { id, .. } if replay_kind.is_none() => {
                self.is_realtime_delegated_agent_item(&notification.turn_id, &id);
            }
            ThreadItem::Reasoning { id, .. } => {
                if replay_kind.is_none()
                    && !self.is_realtime_delegated_reasoning_turn(&notification.turn_id)
                {
                    // A later voice handoff can steer this turn without making an
                    // already-started typed reasoning item private.
                    self.realtime_conversation
                        .agent_items
                        .entry((notification.turn_id.clone(), id.clone()))
                        .or_insert(realtime::RealtimeAgentItemOrigin::Typed);
                }
                if !matches!(replay_kind, Some(ReplayKind::ResumeInitialMessages))
                    && !self.is_realtime_delegated_reasoning_item(&notification.turn_id, &id)
                {
                    self.on_reasoning_item_started(id);
                }
            }
            ThreadItem::ContextCompaction { id }
                if !matches!(replay_kind, Some(ReplayKind::ResumeInitialMessages)) =>
            {
                // Buffered starts reconstruct an in-flight compaction when switching tasks.
                let elapsed = if replay_kind == Some(ReplayKind::ThreadSnapshot) {
                    let elapsed_ms = chrono::Utc::now()
                        .timestamp_millis()
                        .saturating_sub(notification.started_at_ms)
                        .max(0);
                    Duration::from_millis(elapsed_ms as u64)
                } else {
                    Duration::ZERO
                };
                self.on_context_compaction_started(id, elapsed);
            }
            item @ ThreadItem::CommandExecution { .. } => self.on_command_execution_started(item),
            ThreadItem::FileChange { id: _, changes, .. } => {
                self.on_patch_apply_begin(file_update_changes_to_display(changes));
            }
            item @ ThreadItem::McpToolCall { .. } => self.on_mcp_tool_call_started(item),
            ThreadItem::WebSearch(item) => {
                self.on_web_search_begin(item.id);
            }
            ThreadItem::ImageGeneration(_) => {
                self.on_image_generation_begin();
            }
            ThreadItem::CollabAgentToolCall {
                id,
                tool,
                status,
                sender_thread_id,
                receiver_thread_ids,
                prompt,
                model,
                reasoning_effort,
                agents_states,
            } => self.on_collab_agent_tool_call(ThreadItem::CollabAgentToolCall {
                id,
                tool,
                status,
                sender_thread_id,
                receiver_thread_ids,
                prompt,
                model,
                reasoning_effort,
                agents_states,
            }),
            ThreadItem::EnteredReviewMode { review, .. } if replay_kind.is_none() => {
                self.enter_review_mode_with_hint(review, /*from_replay*/ false);
            }
            _ => {}
        }
    }

    fn handle_item_completed_notification(
        &mut self,
        notification: ItemCompletedNotification,
        replay_kind: Option<ReplayKind>,
    ) {
        self.restore_realtime_transcripts_before_turn(&notification.turn_id);
        if replay_kind.is_none()
            && self.is_realtime_delegated_reasoning_turn(&notification.turn_id)
            && realtime::is_private_realtime_agent_item(&notification.item)
            && !matches!(&notification.item, ThreadItem::AgentMessage { id, .. } | ThreadItem::Reasoning { id, .. }
            if matches!(
                self.realtime_conversation.agent_items.get(&(notification.turn_id.clone(), id.clone())),
                Some(realtime::RealtimeAgentItemOrigin::Typed)
            ))
        {
            return;
        }
        // Buffered live notifications can introduce questions; historical turn replay cannot.
        if replay_kind == Some(ReplayKind::ThreadSnapshot)
            && let ThreadItem::AgentMessage {
                id,
                questions: Some(questions),
                ..
            } = &notification.item
        {
            self.add_async_questions(id, questions);
        }
        if replay_kind.is_none()
            && let ThreadItem::Reasoning { id, .. } = &notification.item
            && self.status_state.reasoning_item_id.as_ref() != Some(id)
        {
            return;
        }
        match notification.item {
            item @ ThreadItem::CommandExecution { .. } => self.on_command_execution_completed(item),
            item => self.handle_thread_item(
                item,
                notification.turn_id,
                replay_kind.map_or(ThreadItemRenderSource::Live, ThreadItemRenderSource::Replay),
            ),
        }
    }
}
