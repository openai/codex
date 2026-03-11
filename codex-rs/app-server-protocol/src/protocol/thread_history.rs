use crate::protocol::v2::AutomaticApprovalReview;
use crate::protocol::v2::CollabAgentState;
use crate::protocol::v2::CollabAgentTool;
use crate::protocol::v2::CollabAgentToolCallStatus;
use crate::protocol::v2::CommandAction;
use crate::protocol::v2::CommandExecutionStatus;
use crate::protocol::v2::DynamicToolCallOutputContentItem;
use crate::protocol::v2::DynamicToolCallStatus;
use crate::protocol::v2::FileUpdateChange;
use crate::protocol::v2::ItemApprovalPendingKind;
use crate::protocol::v2::ItemApprovalResolvedBy;
use crate::protocol::v2::ItemApprovalState;
use crate::protocol::v2::ItemApprovalStatus;
use crate::protocol::v2::McpToolCallError;
use crate::protocol::v2::McpToolCallResult;
use crate::protocol::v2::McpToolCallStatus;
use crate::protocol::v2::PatchApplyStatus;
use crate::protocol::v2::PatchChangeKind;
use crate::protocol::v2::ThreadItem;
use crate::protocol::v2::Turn;
use crate::protocol::v2::TurnError as V2TurnError;
use crate::protocol::v2::TurnError;
use crate::protocol::v2::TurnStatus;
use crate::protocol::v2::UserInput;
use crate::protocol::v2::WebSearchAction;
use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::models::MessagePhase;
use codex_protocol::protocol::AgentReasoningEvent;
use codex_protocol::protocol::AgentReasoningRawContentEvent;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
use codex_protocol::protocol::CompactedItem;
use codex_protocol::protocol::ContextCompactedEvent;
use codex_protocol::protocol::DynamicToolCallResponseEvent;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::ImageGenerationBeginEvent;
use codex_protocol::protocol::ImageGenerationEndEvent;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ItemStartedEvent;
use codex_protocol::protocol::McpToolCallBeginEvent;
use codex_protocol::protocol::McpToolCallEndEvent;
use codex_protocol::protocol::PatchApplyBeginEvent;
use codex_protocol::protocol::PatchApplyEndEvent;
use codex_protocol::protocol::RequestUserInputEvent;
use codex_protocol::protocol::ReviewOutputEvent;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadRolledBackEvent;
use codex_protocol::protocol::TurnAbortedEvent;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use codex_protocol::protocol::ViewImageToolCallEvent;
use codex_protocol::protocol::WebSearchBeginEvent;
use codex_protocol::protocol::WebSearchEndEvent;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use tracing::warn;
use uuid::Uuid;

#[cfg(test)]
use codex_protocol::protocol::ExecCommandStatus as CoreExecCommandStatus;
#[cfg(test)]
use codex_protocol::protocol::PatchApplyStatus as CorePatchApplyStatus;

/// Convert persisted [`RolloutItem`] entries into a sequence of [`Turn`] values.
///
/// When available, this uses `TurnContext.turn_id` as the canonical turn id so
/// resumed/rebuilt thread history preserves the original turn identifiers.
pub fn build_turns_from_rollout_items(items: &[RolloutItem]) -> Vec<Turn> {
    let mut builder = ThreadHistoryBuilder::new();
    for item in items {
        builder.handle_rollout_item(item);
    }
    builder.finish()
}

pub struct ThreadHistoryBuilder {
    turns: Vec<Turn>,
    current_turn: Option<PendingTurn>,
    next_item_index: i64,
    guardian_network_access_item_ids: HashSet<(Option<String>, String)>,
}

impl Default for ThreadHistoryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadHistoryBuilder {
    pub fn new() -> Self {
        Self {
            turns: Vec::new(),
            current_turn: None,
            next_item_index: 1,
            guardian_network_access_item_ids: HashSet::new(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn finish(mut self) -> Vec<Turn> {
        self.finish_current_turn();
        self.turns
    }

    pub fn active_turn_snapshot(&self) -> Option<Turn> {
        self.current_turn
            .as_ref()
            .map(Turn::from)
            .or_else(|| self.turns.last().cloned())
    }

    pub fn item_snapshot(&self, turn_id: Option<&str>, item_id: &str) -> Option<ThreadItem> {
        self.find_item(turn_id, item_id).cloned()
    }

    pub fn has_active_turn(&self) -> bool {
        self.current_turn.is_some()
    }

    /// Shared reducer for persisted rollout replay and in-memory current-turn
    /// tracking used by running thread resume/rejoin.
    ///
    /// This function should handle all EventMsg variants that can be persisted in a rollout file.
    /// See `should_persist_event_msg` in `codex-rs/core/rollout/policy.rs`.
    pub fn handle_event(&mut self, event: &EventMsg) {
        match event {
            EventMsg::UserMessage(payload) => self.handle_user_message(payload),
            EventMsg::AgentMessage(payload) => {
                self.handle_agent_message(payload.message.clone(), payload.phase.clone())
            }
            EventMsg::AgentReasoning(payload) => self.handle_agent_reasoning(payload),
            EventMsg::AgentReasoningRawContent(payload) => {
                self.handle_agent_reasoning_raw_content(payload)
            }
            EventMsg::WebSearchBegin(payload) => self.handle_web_search_begin(payload),
            EventMsg::WebSearchEnd(payload) => self.handle_web_search_end(payload),
            EventMsg::ExecCommandBegin(payload) => self.handle_exec_command_begin(payload),
            EventMsg::ExecCommandEnd(payload) => self.handle_exec_command_end(payload),
            EventMsg::ExecApprovalRequest(payload) => self.handle_exec_approval_request(payload),
            EventMsg::ApplyPatchApprovalRequest(payload) => {
                self.handle_apply_patch_approval_request(payload)
            }
            EventMsg::PatchApplyBegin(payload) => self.handle_patch_apply_begin(payload),
            EventMsg::PatchApplyEnd(payload) => self.handle_patch_apply_end(payload),
            EventMsg::DynamicToolCallRequest(payload) => {
                self.handle_dynamic_tool_call_request(payload)
            }
            EventMsg::DynamicToolCallResponse(payload) => {
                self.handle_dynamic_tool_call_response(payload)
            }
            EventMsg::McpToolCallBegin(payload) => self.handle_mcp_tool_call_begin(payload),
            EventMsg::McpToolCallEnd(payload) => self.handle_mcp_tool_call_end(payload),
            EventMsg::RequestUserInput(payload) => self.handle_request_user_input(payload),
            EventMsg::ElicitationRequest(payload) => self.handle_elicitation_request(payload),
            EventMsg::ViewImageToolCall(payload) => self.handle_view_image_tool_call(payload),
            EventMsg::ImageGenerationBegin(payload) => self.handle_image_generation_begin(payload),
            EventMsg::ImageGenerationEnd(payload) => self.handle_image_generation_end(payload),
            EventMsg::GuardianAssessment(payload) => self.handle_guardian_assessment(payload),
            EventMsg::CollabAgentSpawnBegin(payload) => {
                self.handle_collab_agent_spawn_begin(payload)
            }
            EventMsg::CollabAgentSpawnEnd(payload) => self.handle_collab_agent_spawn_end(payload),
            EventMsg::CollabAgentInteractionBegin(payload) => {
                self.handle_collab_agent_interaction_begin(payload)
            }
            EventMsg::CollabAgentInteractionEnd(payload) => {
                self.handle_collab_agent_interaction_end(payload)
            }
            EventMsg::CollabWaitingBegin(payload) => self.handle_collab_waiting_begin(payload),
            EventMsg::CollabWaitingEnd(payload) => self.handle_collab_waiting_end(payload),
            EventMsg::CollabCloseBegin(payload) => self.handle_collab_close_begin(payload),
            EventMsg::CollabCloseEnd(payload) => self.handle_collab_close_end(payload),
            EventMsg::CollabResumeBegin(payload) => self.handle_collab_resume_begin(payload),
            EventMsg::CollabResumeEnd(payload) => self.handle_collab_resume_end(payload),
            EventMsg::ContextCompacted(payload) => self.handle_context_compacted(payload),
            EventMsg::EnteredReviewMode(payload) => self.handle_entered_review_mode(payload),
            EventMsg::ExitedReviewMode(payload) => self.handle_exited_review_mode(payload),
            EventMsg::ItemStarted(payload) => self.handle_item_started(payload),
            EventMsg::ItemCompleted(payload) => self.handle_item_completed(payload),
            EventMsg::HookStarted(_) | EventMsg::HookCompleted(_) => {}
            EventMsg::Error(payload) => self.handle_error(payload),
            EventMsg::TokenCount(_) => {}
            EventMsg::ThreadRolledBack(payload) => self.handle_thread_rollback(payload),
            EventMsg::UndoCompleted(_) => {}
            EventMsg::TurnAborted(payload) => self.handle_turn_aborted(payload),
            EventMsg::TurnStarted(payload) => self.handle_turn_started(payload),
            EventMsg::TurnComplete(payload) => self.handle_turn_complete(payload),
            _ => {}
        }
    }

    pub fn handle_rollout_item(&mut self, item: &RolloutItem) {
        match item {
            RolloutItem::EventMsg(event) => self.handle_event(event),
            RolloutItem::Compacted(payload) => self.handle_compacted(payload),
            RolloutItem::TurnContext(_)
            | RolloutItem::SessionMeta(_)
            | RolloutItem::ResponseItem(_) => {}
        }
    }

    fn handle_user_message(&mut self, payload: &UserMessageEvent) {
        // User messages should stay in explicitly opened turns. For backward
        // compatibility with older streams that did not open turns explicitly,
        // close any implicit/inactive turn and start a fresh one for this input.
        if let Some(turn) = self.current_turn.as_ref()
            && !turn.opened_explicitly
            && !(turn.saw_compaction && turn.items.is_empty())
        {
            self.finish_current_turn();
        }
        let mut turn = self
            .current_turn
            .take()
            .unwrap_or_else(|| self.new_turn(None));
        let id = self.next_item_id();
        let content = self.build_user_inputs(payload);
        turn.items.push(ThreadItem::UserMessage { id, content });
        self.current_turn = Some(turn);
    }

    fn handle_agent_message(&mut self, text: String, phase: Option<MessagePhase>) {
        if text.is_empty() {
            return;
        }

        let id = self.next_item_id();
        self.ensure_turn()
            .items
            .push(ThreadItem::AgentMessage { id, text, phase });
    }

    fn handle_agent_reasoning(&mut self, payload: &AgentReasoningEvent) {
        if payload.text.is_empty() {
            return;
        }

        // If the last item is a reasoning item, add the new text to the summary.
        if let Some(ThreadItem::Reasoning { summary, .. }) = self.ensure_turn().items.last_mut() {
            summary.push(payload.text.clone());
            return;
        }

        // Otherwise, create a new reasoning item.
        let id = self.next_item_id();
        self.ensure_turn().items.push(ThreadItem::Reasoning {
            id,
            summary: vec![payload.text.clone()],
            content: Vec::new(),
        });
    }

    fn handle_agent_reasoning_raw_content(&mut self, payload: &AgentReasoningRawContentEvent) {
        if payload.text.is_empty() {
            return;
        }

        // If the last item is a reasoning item, add the new text to the content.
        if let Some(ThreadItem::Reasoning { content, .. }) = self.ensure_turn().items.last_mut() {
            content.push(payload.text.clone());
            return;
        }

        // Otherwise, create a new reasoning item.
        let id = self.next_item_id();
        self.ensure_turn().items.push(ThreadItem::Reasoning {
            id,
            summary: Vec::new(),
            content: vec![payload.text.clone()],
        });
    }

    fn handle_item_started(&mut self, payload: &ItemStartedEvent) {
        match &payload.item {
            codex_protocol::items::TurnItem::Plan(plan) => {
                if plan.text.is_empty() {
                    return;
                }
                self.upsert_item_in_turn_id(
                    &payload.turn_id,
                    ThreadItem::from(payload.item.clone()),
                );
            }
            codex_protocol::items::TurnItem::UserMessage(_)
            | codex_protocol::items::TurnItem::AgentMessage(_)
            | codex_protocol::items::TurnItem::Reasoning(_)
            | codex_protocol::items::TurnItem::WebSearch(_)
            | codex_protocol::items::TurnItem::ImageGeneration(_)
            | codex_protocol::items::TurnItem::ContextCompaction(_) => {}
        }
    }

    fn handle_item_completed(&mut self, payload: &ItemCompletedEvent) {
        match &payload.item {
            codex_protocol::items::TurnItem::Plan(plan) => {
                if plan.text.is_empty() {
                    return;
                }
                self.upsert_item_in_turn_id(
                    &payload.turn_id,
                    ThreadItem::from(payload.item.clone()),
                );
            }
            codex_protocol::items::TurnItem::UserMessage(_)
            | codex_protocol::items::TurnItem::AgentMessage(_)
            | codex_protocol::items::TurnItem::Reasoning(_)
            | codex_protocol::items::TurnItem::WebSearch(_)
            | codex_protocol::items::TurnItem::ImageGeneration(_)
            | codex_protocol::items::TurnItem::ContextCompaction(_) => {}
        }
    }

    fn handle_web_search_begin(&mut self, payload: &WebSearchBeginEvent) {
        let item = ThreadItem::WebSearch {
            id: payload.call_id.clone(),
            query: String::new(),
            action: None,
        };
        self.upsert_item_in_current_turn(item);
    }

    fn handle_web_search_end(&mut self, payload: &WebSearchEndEvent) {
        let item = ThreadItem::WebSearch {
            id: payload.call_id.clone(),
            query: payload.query.clone(),
            action: Some(WebSearchAction::from(payload.action.clone())),
        };
        self.upsert_item_in_current_turn(item);
    }

    fn handle_exec_command_begin(&mut self, payload: &ExecCommandBeginEvent) {
        let command = shlex::try_join(payload.command.iter().map(String::as_str))
            .unwrap_or_else(|_| payload.command.join(" "));
        let command_actions = payload
            .parsed_cmd
            .iter()
            .cloned()
            .map(CommandAction::from)
            .collect();
        let approval = approved_on_item_start(
            self.existing_item_approval(Some(payload.turn_id.as_str()), &payload.call_id)
                .as_ref(),
        );
        let item = ThreadItem::CommandExecution {
            id: payload.call_id.clone(),
            command,
            cwd: payload.cwd.clone(),
            process_id: payload.process_id.clone(),
            status: CommandExecutionStatus::InProgress,
            command_actions,
            aggregated_output: None,
            exit_code: None,
            duration_ms: None,
            approval,
        };
        self.upsert_item_in_turn_id(&payload.turn_id, item);
    }

    fn handle_exec_command_end(&mut self, payload: &ExecCommandEndEvent) {
        let status: CommandExecutionStatus = (&payload.status).into();
        let duration_ms = i64::try_from(payload.duration.as_millis()).unwrap_or(i64::MAX);
        let aggregated_output = if payload.aggregated_output.is_empty() {
            None
        } else {
            Some(payload.aggregated_output.clone())
        };
        let command = shlex::try_join(payload.command.iter().map(String::as_str))
            .unwrap_or_else(|_| payload.command.join(" "));
        let command_actions = payload
            .parsed_cmd
            .iter()
            .cloned()
            .map(CommandAction::from)
            .collect();
        let approval = resolved_item_approval(
            self.existing_item_approval(Some(payload.turn_id.as_str()), &payload.call_id)
                .as_ref(),
            status == CommandExecutionStatus::Declined,
        );
        let item = ThreadItem::CommandExecution {
            id: payload.call_id.clone(),
            command,
            cwd: payload.cwd.clone(),
            process_id: payload.process_id.clone(),
            status,
            command_actions,
            aggregated_output,
            exit_code: Some(payload.exit_code),
            duration_ms: Some(duration_ms),
            approval,
        };
        // Command completions can arrive out of order. Unified exec may return
        // while a PTY is still running, then emit ExecCommandEnd later from a
        // background exit watcher when that process finally exits. By then, a
        // newer user turn may already have started. Route by event turn_id so
        // replay preserves the original turn association.
        self.upsert_item_in_turn_id(&payload.turn_id, item);
    }

    fn handle_exec_approval_request(&mut self, payload: &ExecApprovalRequestEvent) {
        let command = shlex::try_join(payload.command.iter().map(String::as_str))
            .unwrap_or_else(|_| payload.command.join(" "));
        let command_actions = payload
            .parsed_cmd
            .iter()
            .cloned()
            .map(CommandAction::from)
            .collect();
        let item = ThreadItem::CommandExecution {
            id: payload.call_id.clone(),
            command,
            cwd: payload.cwd.clone(),
            process_id: None,
            status: CommandExecutionStatus::InProgress,
            command_actions,
            aggregated_output: None,
            exit_code: None,
            duration_ms: None,
            approval: Some(pending_manual_approval_state()),
        };
        self.upsert_item_in_turn_id(&payload.turn_id, item);
    }

    fn handle_apply_patch_approval_request(&mut self, payload: &ApplyPatchApprovalRequestEvent) {
        let item = ThreadItem::FileChange {
            id: payload.call_id.clone(),
            changes: convert_patch_changes(&payload.changes),
            status: PatchApplyStatus::InProgress,
            approval: Some(pending_manual_approval_state()),
        };
        if payload.turn_id.is_empty() {
            self.upsert_item_in_current_turn(item);
        } else {
            self.upsert_item_in_turn_id(&payload.turn_id, item);
        }
    }

    fn handle_patch_apply_begin(&mut self, payload: &PatchApplyBeginEvent) {
        let approval = approved_on_item_start(
            self.existing_item_approval(Some(payload.turn_id.as_str()), &payload.call_id)
                .as_ref(),
        );
        let item = ThreadItem::FileChange {
            id: payload.call_id.clone(),
            changes: convert_patch_changes(&payload.changes),
            status: PatchApplyStatus::InProgress,
            approval,
        };
        if payload.turn_id.is_empty() {
            self.upsert_item_in_current_turn(item);
        } else {
            self.upsert_item_in_turn_id(&payload.turn_id, item);
        }
    }

    fn handle_patch_apply_end(&mut self, payload: &PatchApplyEndEvent) {
        let status: PatchApplyStatus = (&payload.status).into();
        let approval = resolved_item_approval(
            self.existing_item_approval(Some(payload.turn_id.as_str()), &payload.call_id)
                .as_ref(),
            status == PatchApplyStatus::Declined,
        );
        let item = ThreadItem::FileChange {
            id: payload.call_id.clone(),
            changes: convert_patch_changes(&payload.changes),
            status,
            approval,
        };
        if payload.turn_id.is_empty() {
            self.upsert_item_in_current_turn(item);
        } else {
            self.upsert_item_in_turn_id(&payload.turn_id, item);
        }
    }

    fn handle_dynamic_tool_call_request(
        &mut self,
        payload: &codex_protocol::dynamic_tools::DynamicToolCallRequest,
    ) {
        let item = ThreadItem::DynamicToolCall {
            id: payload.call_id.clone(),
            tool: payload.tool.clone(),
            arguments: payload.arguments.clone(),
            status: DynamicToolCallStatus::InProgress,
            content_items: None,
            success: None,
            duration_ms: None,
        };
        if payload.turn_id.is_empty() {
            self.upsert_item_in_current_turn(item);
        } else {
            self.upsert_item_in_turn_id(&payload.turn_id, item);
        }
    }

    fn handle_dynamic_tool_call_response(&mut self, payload: &DynamicToolCallResponseEvent) {
        let status = if payload.success {
            DynamicToolCallStatus::Completed
        } else {
            DynamicToolCallStatus::Failed
        };
        let duration_ms = i64::try_from(payload.duration.as_millis()).ok();
        let item = ThreadItem::DynamicToolCall {
            id: payload.call_id.clone(),
            tool: payload.tool.clone(),
            arguments: payload.arguments.clone(),
            status,
            content_items: Some(convert_dynamic_tool_content_items(&payload.content_items)),
            success: Some(payload.success),
            duration_ms,
        };
        if payload.turn_id.is_empty() {
            self.upsert_item_in_current_turn(item);
        } else {
            self.upsert_item_in_turn_id(&payload.turn_id, item);
        }
    }

    fn handle_mcp_tool_call_begin(&mut self, payload: &McpToolCallBeginEvent) {
        let approval =
            approved_on_item_start(self.existing_item_approval(None, &payload.call_id).as_ref());
        let item = ThreadItem::McpToolCall {
            id: payload.call_id.clone(),
            server: payload.invocation.server.clone(),
            tool: payload.invocation.tool.clone(),
            status: McpToolCallStatus::InProgress,
            arguments: payload
                .invocation
                .arguments
                .clone()
                .unwrap_or(serde_json::Value::Null),
            result: None,
            error: None,
            duration_ms: None,
            approval,
        };
        self.upsert_item_in_current_turn(item);
    }

    fn handle_mcp_tool_call_end(&mut self, payload: &McpToolCallEndEvent) {
        let status = if payload.is_success() {
            McpToolCallStatus::Completed
        } else if payload
            .result
            .as_ref()
            .err()
            .is_some_and(|message| is_declined_mcp_tool_call_message(message))
        {
            McpToolCallStatus::Declined
        } else {
            McpToolCallStatus::Failed
        };
        let duration_ms = i64::try_from(payload.duration.as_millis()).ok();
        let (result, error) = match &payload.result {
            Ok(value) => (
                Some(McpToolCallResult {
                    content: value.content.clone(),
                    structured_content: value.structured_content.clone(),
                }),
                None,
            ),
            Err(message) => (
                None,
                Some(McpToolCallError {
                    message: message.clone(),
                }),
            ),
        };
        let approval = resolved_item_approval(
            self.existing_item_approval(None, &payload.call_id).as_ref(),
            status == McpToolCallStatus::Declined,
        );
        let item = ThreadItem::McpToolCall {
            id: payload.call_id.clone(),
            server: payload.invocation.server.clone(),
            tool: payload.invocation.tool.clone(),
            status,
            arguments: payload
                .invocation
                .arguments
                .clone()
                .unwrap_or(serde_json::Value::Null),
            result,
            error,
            duration_ms,
            approval,
        };
        self.upsert_item_in_current_turn(item);
    }

    fn handle_request_user_input(&mut self, payload: &RequestUserInputEvent) {
        self.update_mcp_tool_call_approval(
            Some(payload.turn_id.as_str()),
            &payload.call_id,
            pending_manual_approval_state(),
        );
    }

    fn handle_elicitation_request(&mut self, payload: &ElicitationRequestEvent) {
        let request_id = payload.id.to_string();
        let Some(call_id) = request_id.strip_prefix("mcp_tool_call_approval_") else {
            return;
        };
        self.update_mcp_tool_call_approval(
            payload.turn_id.as_deref(),
            call_id,
            pending_manual_approval_state(),
        );
    }

    fn handle_view_image_tool_call(&mut self, payload: &ViewImageToolCallEvent) {
        let item = ThreadItem::ImageView {
            id: payload.call_id.clone(),
            path: payload.path.to_string_lossy().into_owned(),
        };
        self.upsert_item_in_current_turn(item);
    }

    fn handle_image_generation_begin(&mut self, payload: &ImageGenerationBeginEvent) {
        let item = ThreadItem::ImageGeneration {
            id: payload.call_id.clone(),
            status: String::new(),
            revised_prompt: None,
            result: String::new(),
        };
        self.upsert_item_in_current_turn(item);
    }

    fn handle_image_generation_end(&mut self, payload: &ImageGenerationEndEvent) {
        let item = ThreadItem::ImageGeneration {
            id: payload.call_id.clone(),
            status: payload.status.clone(),
            revised_prompt: payload.revised_prompt.clone(),
            result: payload.result.clone(),
        };
        self.upsert_item_in_current_turn(item);
    }

    fn handle_guardian_assessment(&mut self, payload: &GuardianAssessmentEvent) {
        let approval = automatic_approval_state(payload);
        let turn_id = (!payload.turn_id.is_empty()).then_some(payload.turn_id.as_str());
        if self.find_item(turn_id, &payload.id).is_none()
            && let Some(action) = payload.action.as_ref()
            && let Some(item) =
                thread_item_from_guardian_assessment_action(&payload.id, action, approval.clone())
        {
            if action
                .get("tool")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|tool| tool == "network_access")
            {
                self.guardian_network_access_item_ids
                    .insert((turn_id.map(str::to_owned), payload.id.clone()));
            }
            if let Some(turn_id) = turn_id {
                self.upsert_item_in_turn_id(turn_id, item);
            } else {
                self.upsert_item_in_current_turn(item);
            }
            return;
        }
        self.update_item_approval(
            turn_id,
            &payload.id,
            approval,
            matches!(
                payload.status,
                codex_protocol::protocol::GuardianAssessmentStatus::Denied
            ),
        );
    }

    fn handle_collab_agent_spawn_begin(
        &mut self,
        payload: &codex_protocol::protocol::CollabAgentSpawnBeginEvent,
    ) {
        let item = ThreadItem::CollabAgentToolCall {
            id: payload.call_id.clone(),
            tool: CollabAgentTool::SpawnAgent,
            status: CollabAgentToolCallStatus::InProgress,
            sender_thread_id: payload.sender_thread_id.to_string(),
            receiver_thread_ids: Vec::new(),
            prompt: Some(payload.prompt.clone()),
            model: Some(payload.model.clone()),
            reasoning_effort: Some(payload.reasoning_effort),
            agents_states: HashMap::new(),
        };
        self.upsert_item_in_current_turn(item);
    }

    fn handle_collab_agent_spawn_end(
        &mut self,
        payload: &codex_protocol::protocol::CollabAgentSpawnEndEvent,
    ) {
        let has_receiver = payload.new_thread_id.is_some();
        let status = match &payload.status {
            AgentStatus::Errored(_) | AgentStatus::NotFound => CollabAgentToolCallStatus::Failed,
            _ if has_receiver => CollabAgentToolCallStatus::Completed,
            _ => CollabAgentToolCallStatus::Failed,
        };
        let (receiver_thread_ids, agents_states) = match &payload.new_thread_id {
            Some(id) => {
                let receiver_id = id.to_string();
                let received_status = CollabAgentState::from(payload.status.clone());
                (
                    vec![receiver_id.clone()],
                    [(receiver_id, received_status)].into_iter().collect(),
                )
            }
            None => (Vec::new(), HashMap::new()),
        };
        self.upsert_item_in_current_turn(ThreadItem::CollabAgentToolCall {
            id: payload.call_id.clone(),
            tool: CollabAgentTool::SpawnAgent,
            status,
            sender_thread_id: payload.sender_thread_id.to_string(),
            receiver_thread_ids,
            prompt: Some(payload.prompt.clone()),
            model: Some(payload.model.clone()),
            reasoning_effort: Some(payload.reasoning_effort),
            agents_states,
        });
    }

    fn handle_collab_agent_interaction_begin(
        &mut self,
        payload: &codex_protocol::protocol::CollabAgentInteractionBeginEvent,
    ) {
        let item = ThreadItem::CollabAgentToolCall {
            id: payload.call_id.clone(),
            tool: CollabAgentTool::SendInput,
            status: CollabAgentToolCallStatus::InProgress,
            sender_thread_id: payload.sender_thread_id.to_string(),
            receiver_thread_ids: vec![payload.receiver_thread_id.to_string()],
            prompt: Some(payload.prompt.clone()),
            model: None,
            reasoning_effort: None,
            agents_states: HashMap::new(),
        };
        self.upsert_item_in_current_turn(item);
    }

    fn handle_collab_agent_interaction_end(
        &mut self,
        payload: &codex_protocol::protocol::CollabAgentInteractionEndEvent,
    ) {
        let status = match &payload.status {
            AgentStatus::Errored(_) | AgentStatus::NotFound => CollabAgentToolCallStatus::Failed,
            _ => CollabAgentToolCallStatus::Completed,
        };
        let receiver_id = payload.receiver_thread_id.to_string();
        let received_status = CollabAgentState::from(payload.status.clone());
        self.upsert_item_in_current_turn(ThreadItem::CollabAgentToolCall {
            id: payload.call_id.clone(),
            tool: CollabAgentTool::SendInput,
            status,
            sender_thread_id: payload.sender_thread_id.to_string(),
            receiver_thread_ids: vec![receiver_id.clone()],
            prompt: Some(payload.prompt.clone()),
            model: None,
            reasoning_effort: None,
            agents_states: [(receiver_id, received_status)].into_iter().collect(),
        });
    }

    fn handle_collab_waiting_begin(
        &mut self,
        payload: &codex_protocol::protocol::CollabWaitingBeginEvent,
    ) {
        let item = ThreadItem::CollabAgentToolCall {
            id: payload.call_id.clone(),
            tool: CollabAgentTool::Wait,
            status: CollabAgentToolCallStatus::InProgress,
            sender_thread_id: payload.sender_thread_id.to_string(),
            receiver_thread_ids: payload
                .receiver_thread_ids
                .iter()
                .map(ToString::to_string)
                .collect(),
            prompt: None,
            model: None,
            reasoning_effort: None,
            agents_states: HashMap::new(),
        };
        self.upsert_item_in_current_turn(item);
    }

    fn handle_collab_waiting_end(
        &mut self,
        payload: &codex_protocol::protocol::CollabWaitingEndEvent,
    ) {
        let status = if payload
            .statuses
            .values()
            .any(|status| matches!(status, AgentStatus::Errored(_) | AgentStatus::NotFound))
        {
            CollabAgentToolCallStatus::Failed
        } else {
            CollabAgentToolCallStatus::Completed
        };
        let mut receiver_thread_ids: Vec<String> =
            payload.statuses.keys().map(ToString::to_string).collect();
        receiver_thread_ids.sort();
        let agents_states = payload
            .statuses
            .iter()
            .map(|(id, status)| (id.to_string(), CollabAgentState::from(status.clone())))
            .collect();
        self.upsert_item_in_current_turn(ThreadItem::CollabAgentToolCall {
            id: payload.call_id.clone(),
            tool: CollabAgentTool::Wait,
            status,
            sender_thread_id: payload.sender_thread_id.to_string(),
            receiver_thread_ids,
            prompt: None,
            model: None,
            reasoning_effort: None,
            agents_states,
        });
    }

    fn handle_collab_close_begin(
        &mut self,
        payload: &codex_protocol::protocol::CollabCloseBeginEvent,
    ) {
        let item = ThreadItem::CollabAgentToolCall {
            id: payload.call_id.clone(),
            tool: CollabAgentTool::CloseAgent,
            status: CollabAgentToolCallStatus::InProgress,
            sender_thread_id: payload.sender_thread_id.to_string(),
            receiver_thread_ids: vec![payload.receiver_thread_id.to_string()],
            prompt: None,
            model: None,
            reasoning_effort: None,
            agents_states: HashMap::new(),
        };
        self.upsert_item_in_current_turn(item);
    }

    fn handle_collab_close_end(&mut self, payload: &codex_protocol::protocol::CollabCloseEndEvent) {
        let status = match &payload.status {
            AgentStatus::Errored(_) | AgentStatus::NotFound => CollabAgentToolCallStatus::Failed,
            _ => CollabAgentToolCallStatus::Completed,
        };
        let receiver_id = payload.receiver_thread_id.to_string();
        let agents_states = [(
            receiver_id.clone(),
            CollabAgentState::from(payload.status.clone()),
        )]
        .into_iter()
        .collect();
        self.upsert_item_in_current_turn(ThreadItem::CollabAgentToolCall {
            id: payload.call_id.clone(),
            tool: CollabAgentTool::CloseAgent,
            status,
            sender_thread_id: payload.sender_thread_id.to_string(),
            receiver_thread_ids: vec![receiver_id],
            prompt: None,
            model: None,
            reasoning_effort: None,
            agents_states,
        });
    }

    fn handle_collab_resume_begin(
        &mut self,
        payload: &codex_protocol::protocol::CollabResumeBeginEvent,
    ) {
        let item = ThreadItem::CollabAgentToolCall {
            id: payload.call_id.clone(),
            tool: CollabAgentTool::ResumeAgent,
            status: CollabAgentToolCallStatus::InProgress,
            sender_thread_id: payload.sender_thread_id.to_string(),
            receiver_thread_ids: vec![payload.receiver_thread_id.to_string()],
            prompt: None,
            model: None,
            reasoning_effort: None,
            agents_states: HashMap::new(),
        };
        self.upsert_item_in_current_turn(item);
    }

    fn handle_collab_resume_end(
        &mut self,
        payload: &codex_protocol::protocol::CollabResumeEndEvent,
    ) {
        let status = match &payload.status {
            AgentStatus::Errored(_) | AgentStatus::NotFound => CollabAgentToolCallStatus::Failed,
            _ => CollabAgentToolCallStatus::Completed,
        };
        let receiver_id = payload.receiver_thread_id.to_string();
        let agents_states = [(
            receiver_id.clone(),
            CollabAgentState::from(payload.status.clone()),
        )]
        .into_iter()
        .collect();
        self.upsert_item_in_current_turn(ThreadItem::CollabAgentToolCall {
            id: payload.call_id.clone(),
            tool: CollabAgentTool::ResumeAgent,
            status,
            sender_thread_id: payload.sender_thread_id.to_string(),
            receiver_thread_ids: vec![receiver_id],
            prompt: None,
            model: None,
            reasoning_effort: None,
            agents_states,
        });
    }

    fn handle_context_compacted(&mut self, _payload: &ContextCompactedEvent) {
        let id = self.next_item_id();
        self.ensure_turn()
            .items
            .push(ThreadItem::ContextCompaction { id });
    }

    fn handle_entered_review_mode(&mut self, payload: &codex_protocol::protocol::ReviewRequest) {
        let review = payload
            .user_facing_hint
            .clone()
            .unwrap_or_else(|| "Review requested.".to_string());
        let id = self.next_item_id();
        self.ensure_turn()
            .items
            .push(ThreadItem::EnteredReviewMode { id, review });
    }

    fn handle_exited_review_mode(
        &mut self,
        payload: &codex_protocol::protocol::ExitedReviewModeEvent,
    ) {
        let review = payload
            .review_output
            .as_ref()
            .map(render_review_output_text)
            .unwrap_or_else(|| REVIEW_FALLBACK_MESSAGE.to_string());
        let id = self.next_item_id();
        self.ensure_turn()
            .items
            .push(ThreadItem::ExitedReviewMode { id, review });
    }

    fn handle_error(&mut self, payload: &ErrorEvent) {
        if !payload.affects_turn_status() {
            return;
        }
        let Some(turn) = self.current_turn.as_mut() else {
            return;
        };
        turn.status = TurnStatus::Failed;
        turn.error = Some(V2TurnError {
            message: payload.message.clone(),
            codex_error_info: payload.codex_error_info.clone().map(Into::into),
            additional_details: None,
        });
    }

    fn handle_turn_aborted(&mut self, payload: &TurnAbortedEvent) {
        if let Some(turn_id) = payload.turn_id.as_deref() {
            // Prefer an exact ID match so we interrupt the turn explicitly targeted by the event.
            if let Some(turn) = self.current_turn.as_mut().filter(|turn| turn.id == turn_id) {
                turn.status = TurnStatus::Interrupted;
                return;
            }

            if let Some(turn) = self.turns.iter_mut().find(|turn| turn.id == turn_id) {
                turn.status = TurnStatus::Interrupted;
                return;
            }
        }

        // If the event has no ID (or refers to an unknown turn), fall back to the active turn.
        if let Some(turn) = self.current_turn.as_mut() {
            turn.status = TurnStatus::Interrupted;
        }
    }

    fn handle_turn_started(&mut self, payload: &TurnStartedEvent) {
        self.finish_current_turn();
        self.current_turn = Some(
            self.new_turn(Some(payload.turn_id.clone()))
                .with_status(TurnStatus::InProgress)
                .opened_explicitly(),
        );
    }

    fn handle_turn_complete(&mut self, payload: &TurnCompleteEvent) {
        let mark_completed = |status: &mut TurnStatus| {
            if matches!(*status, TurnStatus::Completed | TurnStatus::InProgress) {
                *status = TurnStatus::Completed;
            }
        };

        // Prefer an exact ID match from the active turn and then close it.
        if let Some(current_turn) = self
            .current_turn
            .as_mut()
            .filter(|turn| turn.id == payload.turn_id)
        {
            mark_completed(&mut current_turn.status);
            self.finish_current_turn();
            return;
        }

        if let Some(turn) = self
            .turns
            .iter_mut()
            .find(|turn| turn.id == payload.turn_id)
        {
            mark_completed(&mut turn.status);
            return;
        }

        // If the completion event cannot be matched, apply it to the active turn.
        if let Some(current_turn) = self.current_turn.as_mut() {
            mark_completed(&mut current_turn.status);
            self.finish_current_turn();
        }
    }

    /// Marks the current turn as containing a persisted compaction marker.
    ///
    /// This keeps compaction-only legacy turns from being dropped by
    /// `finish_current_turn` when they have no renderable items and were not
    /// explicitly opened.
    fn handle_compacted(&mut self, _payload: &CompactedItem) {
        self.ensure_turn().saw_compaction = true;
    }

    fn handle_thread_rollback(&mut self, payload: &ThreadRolledBackEvent) {
        self.finish_current_turn();

        let n = usize::try_from(payload.num_turns).unwrap_or(usize::MAX);
        if n >= self.turns.len() {
            self.turns.clear();
        } else {
            self.turns.truncate(self.turns.len().saturating_sub(n));
        }

        let item_count: usize = self.turns.iter().map(|t| t.items.len()).sum();
        self.next_item_index = i64::try_from(item_count.saturating_add(1)).unwrap_or(i64::MAX);
    }

    fn finish_current_turn(&mut self) {
        if let Some(turn) = self.current_turn.take() {
            if turn.items.is_empty() && !turn.opened_explicitly && !turn.saw_compaction {
                return;
            }
            self.turns.push(turn.into());
        }
    }

    fn new_turn(&mut self, id: Option<String>) -> PendingTurn {
        PendingTurn {
            id: id.unwrap_or_else(|| Uuid::now_v7().to_string()),
            items: Vec::new(),
            error: None,
            status: TurnStatus::Completed,
            opened_explicitly: false,
            saw_compaction: false,
        }
    }

    fn ensure_turn(&mut self) -> &mut PendingTurn {
        if self.current_turn.is_none() {
            let turn = self.new_turn(None);
            return self.current_turn.insert(turn);
        }

        if let Some(turn) = self.current_turn.as_mut() {
            return turn;
        }

        unreachable!("current turn must exist after initialization");
    }

    fn upsert_item_in_turn_id(&mut self, turn_id: &str, item: ThreadItem) {
        if let Some(turn) = self.current_turn.as_mut()
            && turn.id == turn_id
        {
            upsert_turn_item(&mut turn.items, item);
            return;
        }

        if let Some(turn) = self.turns.iter_mut().find(|turn| turn.id == turn_id) {
            upsert_turn_item(&mut turn.items, item);
            return;
        }

        warn!(
            item_id = item.id(),
            "dropping turn-scoped item for unknown turn id `{turn_id}`"
        );
    }

    fn upsert_item_in_current_turn(&mut self, item: ThreadItem) {
        let turn = self.ensure_turn();
        upsert_turn_item(&mut turn.items, item);
    }

    fn existing_item_approval(
        &self,
        turn_id: Option<&str>,
        item_id: &str,
    ) -> Option<ItemApprovalState> {
        self.find_item(turn_id, item_id)
            .and_then(thread_item_approval)
            .cloned()
    }

    fn find_item(&self, turn_id: Option<&str>, item_id: &str) -> Option<&ThreadItem> {
        if let Some(turn_id) = turn_id {
            if let Some(turn) = self.current_turn.as_ref()
                && turn.id == turn_id
            {
                return turn.items.iter().find(|item| item.id() == item_id);
            }

            return self
                .turns
                .iter()
                .find(|turn| turn.id == turn_id)
                .and_then(|turn| turn.items.iter().find(|item| item.id() == item_id));
        }

        self.current_turn
            .as_ref()
            .and_then(|turn| turn.items.iter().find(|item| item.id() == item_id))
            .or_else(|| {
                self.turns
                    .iter()
                    .rev()
                    .find_map(|turn| turn.items.iter().find(|item| item.id() == item_id))
            })
    }

    fn update_mcp_tool_call_approval(
        &mut self,
        turn_id: Option<&str>,
        item_id: &str,
        approval: ItemApprovalState,
    ) {
        self.update_item_approval(turn_id, item_id, approval, false);
    }

    fn update_item_approval(
        &mut self,
        turn_id: Option<&str>,
        item_id: &str,
        approval: ItemApprovalState,
        mark_declined: bool,
    ) {
        let approved = approval.status == ItemApprovalStatus::Approved;
        let synthetic_network_access = self
            .guardian_network_access_item_ids
            .contains(&(turn_id.map(str::to_owned), item_id.to_string()));
        let update = |item: &mut ThreadItem| match item {
            ThreadItem::CommandExecution {
                status,
                approval: existing_approval,
                ..
            } => {
                if mark_declined {
                    *status = CommandExecutionStatus::Declined;
                } else if approved && synthetic_network_access {
                    *status = CommandExecutionStatus::Completed;
                }
                *existing_approval = Some(approval);
            }
            ThreadItem::FileChange {
                status,
                approval: existing_approval,
                ..
            } => {
                if mark_declined {
                    *status = PatchApplyStatus::Declined;
                }
                *existing_approval = Some(approval);
            }
            ThreadItem::McpToolCall {
                status,
                approval: existing_approval,
                ..
            } => {
                if mark_declined {
                    *status = McpToolCallStatus::Declined;
                }
                *existing_approval = Some(approval);
            }
            _ => {}
        };

        if let Some(turn_id) = turn_id {
            if let Some(turn) = self.current_turn.as_mut()
                && turn.id == turn_id
                && let Some(item) = turn.items.iter_mut().find(|item| item.id() == item_id)
            {
                update(item);
                return;
            }

            if let Some(turn) = self.turns.iter_mut().find(|turn| turn.id == turn_id)
                && let Some(item) = turn.items.iter_mut().find(|item| item.id() == item_id)
            {
                update(item);
                return;
            }
            return;
        }

        if let Some(turn) = self.current_turn.as_mut()
            && let Some(item) = turn.items.iter_mut().find(|item| item.id() == item_id)
        {
            update(item);
            return;
        }

        if let Some(turn) = self
            .turns
            .iter_mut()
            .rev()
            .find(|turn| turn.items.iter().any(|item| item.id() == item_id))
            && let Some(item) = turn.items.iter_mut().find(|item| item.id() == item_id)
        {
            update(item);
        }
    }

    fn next_item_id(&mut self) -> String {
        let id = format!("item-{}", self.next_item_index);
        self.next_item_index += 1;
        id
    }

    fn build_user_inputs(&self, payload: &UserMessageEvent) -> Vec<UserInput> {
        let mut content = Vec::new();
        if !payload.message.trim().is_empty() {
            content.push(UserInput::Text {
                text: payload.message.clone(),
                text_elements: payload
                    .text_elements
                    .iter()
                    .cloned()
                    .map(Into::into)
                    .collect(),
            });
        }
        if let Some(images) = &payload.images {
            for image in images {
                content.push(UserInput::Image { url: image.clone() });
            }
        }
        for path in &payload.local_images {
            content.push(UserInput::LocalImage { path: path.clone() });
        }
        content
    }
}

const REVIEW_FALLBACK_MESSAGE: &str = "Reviewer failed to output a response.";

fn render_review_output_text(output: &ReviewOutputEvent) -> String {
    let explanation = output.overall_explanation.trim();
    if explanation.is_empty() {
        REVIEW_FALLBACK_MESSAGE.to_string()
    } else {
        explanation.to_string()
    }
}

pub fn convert_patch_changes(
    changes: &HashMap<std::path::PathBuf, codex_protocol::protocol::FileChange>,
) -> Vec<FileUpdateChange> {
    let mut converted: Vec<FileUpdateChange> = changes
        .iter()
        .map(|(path, change)| FileUpdateChange {
            path: path.to_string_lossy().into_owned(),
            kind: map_patch_change_kind(change),
            diff: format_file_change_diff(change),
        })
        .collect();
    converted.sort_by(|a, b| a.path.cmp(&b.path));
    converted
}

fn convert_dynamic_tool_content_items(
    items: &[codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem],
) -> Vec<DynamicToolCallOutputContentItem> {
    items
        .iter()
        .cloned()
        .map(|item| match item {
            codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem::InputText { text } => {
                DynamicToolCallOutputContentItem::InputText { text }
            }
            codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem::InputImage {
                image_url,
            } => DynamicToolCallOutputContentItem::InputImage { image_url },
        })
        .collect()
}

fn map_patch_change_kind(change: &codex_protocol::protocol::FileChange) -> PatchChangeKind {
    match change {
        codex_protocol::protocol::FileChange::Add { .. } => PatchChangeKind::Add,
        codex_protocol::protocol::FileChange::Delete { .. } => PatchChangeKind::Delete,
        codex_protocol::protocol::FileChange::Update { move_path, .. } => PatchChangeKind::Update {
            move_path: move_path.clone(),
        },
    }
}

fn format_file_change_diff(change: &codex_protocol::protocol::FileChange) -> String {
    match change {
        codex_protocol::protocol::FileChange::Add { content } => content.clone(),
        codex_protocol::protocol::FileChange::Delete { content } => content.clone(),
        codex_protocol::protocol::FileChange::Update {
            unified_diff,
            move_path,
        } => {
            if let Some(path) = move_path {
                format!("{unified_diff}\n\nMoved to: {}", path.display())
            } else {
                unified_diff.clone()
            }
        }
    }
}

fn upsert_turn_item(items: &mut Vec<ThreadItem>, item: ThreadItem) {
    if let Some(existing_item) = items
        .iter_mut()
        .find(|existing_item| existing_item.id() == item.id())
    {
        *existing_item = preserve_existing_approval(existing_item, item);
        return;
    }
    items.push(item);
}

fn preserve_existing_approval(existing_item: &ThreadItem, item: ThreadItem) -> ThreadItem {
    match (existing_item, item) {
        (
            ThreadItem::CommandExecution {
                approval: existing_approval,
                ..
            },
            ThreadItem::CommandExecution {
                id,
                command,
                cwd,
                process_id,
                status,
                command_actions,
                aggregated_output,
                exit_code,
                duration_ms,
                approval,
            },
        ) => ThreadItem::CommandExecution {
            id,
            command,
            cwd,
            process_id,
            status,
            command_actions,
            aggregated_output,
            exit_code,
            duration_ms,
            approval: approval.or_else(|| existing_approval.clone()),
        },
        (
            ThreadItem::FileChange {
                approval: existing_approval,
                ..
            },
            ThreadItem::FileChange {
                id,
                changes,
                status,
                approval,
            },
        ) => ThreadItem::FileChange {
            id,
            changes,
            status,
            approval: approval.or_else(|| existing_approval.clone()),
        },
        (
            ThreadItem::McpToolCall {
                approval: existing_approval,
                ..
            },
            ThreadItem::McpToolCall {
                id,
                server,
                tool,
                status,
                arguments,
                result,
                error,
                duration_ms,
                approval,
            },
        ) => ThreadItem::McpToolCall {
            id,
            server,
            tool,
            status,
            arguments,
            result,
            error,
            duration_ms,
            approval: approval.or_else(|| existing_approval.clone()),
        },
        (_, item) => item,
    }
}

fn thread_item_approval(item: &ThreadItem) -> Option<&ItemApprovalState> {
    match item {
        ThreadItem::CommandExecution { approval, .. }
        | ThreadItem::FileChange { approval, .. }
        | ThreadItem::McpToolCall { approval, .. } => approval.as_ref(),
        ThreadItem::UserMessage { .. }
        | ThreadItem::AgentMessage { .. }
        | ThreadItem::Plan { .. }
        | ThreadItem::Reasoning { .. }
        | ThreadItem::DynamicToolCall { .. }
        | ThreadItem::CollabAgentToolCall { .. }
        | ThreadItem::WebSearch { .. }
        | ThreadItem::ImageView { .. }
        | ThreadItem::ImageGeneration { .. }
        | ThreadItem::EnteredReviewMode { .. }
        | ThreadItem::ExitedReviewMode { .. }
        | ThreadItem::ContextCompaction { .. } => None,
    }
}

fn pending_manual_approval_state() -> ItemApprovalState {
    ItemApprovalState {
        status: ItemApprovalStatus::Pending,
        pending_kind: Some(ItemApprovalPendingKind::ManualRequest),
        resolved_by: None,
        automatic_review: None,
    }
}

fn approved_on_item_start(existing: Option<&ItemApprovalState>) -> Option<ItemApprovalState> {
    match existing {
        Some(ItemApprovalState {
            status: ItemApprovalStatus::Pending,
            pending_kind: Some(ItemApprovalPendingKind::ManualRequest),
            ..
        }) => Some(ItemApprovalState {
            status: ItemApprovalStatus::Approved,
            pending_kind: None,
            resolved_by: Some(ItemApprovalResolvedBy::User),
            automatic_review: None,
        }),
        Some(existing) => Some(existing.clone()),
        None => None,
    }
}

fn resolved_item_approval(
    existing: Option<&ItemApprovalState>,
    declined: bool,
) -> Option<ItemApprovalState> {
    match existing {
        Some(ItemApprovalState {
            status: ItemApprovalStatus::Pending,
            pending_kind: Some(ItemApprovalPendingKind::ManualRequest),
            ..
        }) => Some(ItemApprovalState {
            status: if declined {
                ItemApprovalStatus::Declined
            } else {
                ItemApprovalStatus::Approved
            },
            pending_kind: None,
            resolved_by: Some(ItemApprovalResolvedBy::User),
            automatic_review: None,
        }),
        Some(existing) => Some(existing.clone()),
        None => None,
    }
}

fn automatic_approval_state(payload: &GuardianAssessmentEvent) -> ItemApprovalState {
    match payload.status {
        codex_protocol::protocol::GuardianAssessmentStatus::InProgress => ItemApprovalState {
            status: ItemApprovalStatus::Pending,
            pending_kind: Some(ItemApprovalPendingKind::AutomaticReview),
            resolved_by: None,
            automatic_review: Some(AutomaticApprovalReview::from_core_review_status(
                payload.status,
                payload.risk_score,
                payload.risk_level,
                payload.rationale.clone(),
            )),
        },
        codex_protocol::protocol::GuardianAssessmentStatus::Approved => ItemApprovalState {
            status: ItemApprovalStatus::Approved,
            pending_kind: None,
            resolved_by: Some(ItemApprovalResolvedBy::Automatic),
            automatic_review: Some(AutomaticApprovalReview::from_core_review_status(
                payload.status,
                payload.risk_score,
                payload.risk_level,
                payload.rationale.clone(),
            )),
        },
        codex_protocol::protocol::GuardianAssessmentStatus::Denied => ItemApprovalState {
            status: ItemApprovalStatus::Declined,
            pending_kind: None,
            resolved_by: Some(ItemApprovalResolvedBy::Automatic),
            automatic_review: Some(AutomaticApprovalReview::from_core_review_status(
                payload.status,
                payload.risk_score,
                payload.risk_level,
                payload.rationale.clone(),
            )),
        },
    }
}

fn thread_item_from_guardian_assessment_action(
    item_id: &str,
    action: &serde_json::Value,
    approval: ItemApprovalState,
) -> Option<ThreadItem> {
    let status = approval.status;
    let tool = action.get("tool")?.as_str()?;
    match tool {
        "shell" | "exec_command" => Some(ThreadItem::CommandExecution {
            id: item_id.to_string(),
            command: guardian_action_command(action)?,
            cwd: action
                .get("cwd")
                .and_then(serde_json::Value::as_str)
                .map(PathBuf::from)
                .unwrap_or_default(),
            process_id: None,
            status: if status == ItemApprovalStatus::Declined {
                CommandExecutionStatus::Declined
            } else {
                CommandExecutionStatus::InProgress
            },
            command_actions: Vec::new(),
            aggregated_output: None,
            exit_code: None,
            duration_ms: None,
            approval: Some(approval),
        }),
        "network_access" => Some(ThreadItem::CommandExecution {
            id: item_id.to_string(),
            command: format!("network access {}", action.get("target")?.as_str()?),
            cwd: PathBuf::new(),
            process_id: None,
            status: match status {
                ItemApprovalStatus::Pending => CommandExecutionStatus::InProgress,
                ItemApprovalStatus::Approved => CommandExecutionStatus::Completed,
                ItemApprovalStatus::Declined => CommandExecutionStatus::Declined,
                ItemApprovalStatus::Cancelled => CommandExecutionStatus::Declined,
            },
            command_actions: Vec::new(),
            aggregated_output: None,
            exit_code: None,
            duration_ms: None,
            approval: Some(approval),
        }),
        "apply_patch" => Some(ThreadItem::FileChange {
            id: item_id.to_string(),
            changes: guardian_action_changes(action),
            status: if status == ItemApprovalStatus::Declined {
                PatchApplyStatus::Declined
            } else {
                PatchApplyStatus::InProgress
            },
            approval: Some(approval),
        }),
        "mcp_tool_call" => Some(ThreadItem::McpToolCall {
            id: item_id.to_string(),
            server: action
                .get("server")
                .and_then(serde_json::Value::as_str)?
                .to_string(),
            tool: action
                .get("tool_name")
                .and_then(serde_json::Value::as_str)?
                .to_string(),
            status: if status == ItemApprovalStatus::Declined {
                McpToolCallStatus::Declined
            } else {
                McpToolCallStatus::InProgress
            },
            arguments: action
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
            result: None,
            error: None,
            duration_ms: None,
            approval: Some(approval),
        }),
        _ => None,
    }
}

fn guardian_action_command(action: &serde_json::Value) -> Option<String> {
    if let Some(command) = action.get("command") {
        return match command {
            serde_json::Value::String(command) => Some(command.clone()),
            serde_json::Value::Array(command) => {
                let args = command
                    .iter()
                    .map(serde_json::Value::as_str)
                    .collect::<Option<Vec<_>>>()?;
                shlex::try_join(args.iter().copied())
                    .ok()
                    .or_else(|| Some(args.join(" ")))
            }
            _ => None,
        };
    }

    let program = action.get("program")?.as_str()?;
    let argv = action
        .get("argv")?
        .as_array()?
        .iter()
        .map(serde_json::Value::as_str)
        .collect::<Option<Vec<_>>>()?;
    let args = std::iter::once(program)
        .chain(argv.iter().skip(1).copied())
        .collect::<Vec<_>>();
    shlex::try_join(args.iter().copied())
        .ok()
        .or_else(|| Some(args.join(" ")))
}

fn guardian_action_changes(action: &serde_json::Value) -> Vec<FileUpdateChange> {
    if let Some(changes) = action.get("changes").and_then(serde_json::Value::as_array) {
        return changes
            .iter()
            .filter_map(|change| {
                let path = change.get("path")?.as_str()?.to_string();
                let diff = change.get("diff")?.as_str()?.to_string();
                let kind = match change.get("kind")?.as_str()? {
                    "add" => PatchChangeKind::Add,
                    "delete" => PatchChangeKind::Delete,
                    "update" => PatchChangeKind::Update {
                        move_path: change
                            .get("move_path")
                            .and_then(serde_json::Value::as_str)
                            .map(PathBuf::from),
                    },
                    _ => return None,
                };
                Some(FileUpdateChange { path, kind, diff })
            })
            .collect();
    }

    action
        .get("files")
        .and_then(serde_json::Value::as_array)
        .map(|files| {
            files
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(|path| FileUpdateChange {
                    path: path.to_string(),
                    kind: PatchChangeKind::Update { move_path: None },
                    diff: String::new(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn is_declined_mcp_tool_call_message(message: &str) -> bool {
    matches!(
        message,
        "user rejected MCP tool call" | "user cancelled MCP tool call"
    )
}

struct PendingTurn {
    id: String,
    items: Vec<ThreadItem>,
    error: Option<TurnError>,
    status: TurnStatus,
    /// True when this turn originated from an explicit `turn_started`/`turn_complete`
    /// boundary, so we preserve it even if it has no renderable items.
    opened_explicitly: bool,
    /// True when this turn includes a persisted `RolloutItem::Compacted`, which
    /// should keep the turn from being dropped even without normal items.
    saw_compaction: bool,
}

impl PendingTurn {
    fn opened_explicitly(mut self) -> Self {
        self.opened_explicitly = true;
        self
    }

    fn with_status(mut self, status: TurnStatus) -> Self {
        self.status = status;
        self
    }
}

impl From<PendingTurn> for Turn {
    fn from(value: PendingTurn) -> Self {
        Self {
            id: value.id,
            items: value.items,
            error: value.error,
            status: value.status,
        }
    }
}

impl From<&PendingTurn> for Turn {
    fn from(value: &PendingTurn) -> Self {
        Self {
            id: value.id.clone(),
            items: value.items.clone(),
            error: value.error.clone(),
            status: value.status.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::v2::AutomaticApprovalReviewStatus;
    use crate::protocol::v2::RiskLevel;
    use codex_protocol::ThreadId;
    use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem as CoreDynamicToolCallOutputContentItem;
    use codex_protocol::items::TurnItem as CoreTurnItem;
    use codex_protocol::items::UserMessageItem as CoreUserMessageItem;
    use codex_protocol::models::MessagePhase as CoreMessagePhase;
    use codex_protocol::models::WebSearchAction as CoreWebSearchAction;
    use codex_protocol::parse_command::ParsedCommand;
    use codex_protocol::protocol::AgentMessageEvent;
    use codex_protocol::protocol::AgentReasoningEvent;
    use codex_protocol::protocol::AgentReasoningRawContentEvent;
    use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
    use codex_protocol::protocol::CodexErrorInfo;
    use codex_protocol::protocol::CompactedItem;
    use codex_protocol::protocol::DynamicToolCallResponseEvent;
    use codex_protocol::protocol::ExecCommandEndEvent;
    use codex_protocol::protocol::ExecCommandSource;
    use codex_protocol::protocol::ItemStartedEvent;
    use codex_protocol::protocol::McpInvocation;
    use codex_protocol::protocol::McpToolCallEndEvent;
    use codex_protocol::protocol::PatchApplyBeginEvent;
    use codex_protocol::protocol::ThreadRolledBackEvent;
    use codex_protocol::protocol::TurnAbortReason;
    use codex_protocol::protocol::TurnAbortedEvent;
    use codex_protocol::protocol::TurnCompleteEvent;
    use codex_protocol::protocol::TurnStartedEvent;
    use codex_protocol::protocol::UserMessageEvent;
    use codex_protocol::protocol::WebSearchEndEvent;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use std::time::Duration;
    use uuid::Uuid;

    #[test]
    fn builds_multiple_turns_with_reasoning_items() {
        let events = vec![
            EventMsg::UserMessage(UserMessageEvent {
                message: "First turn".into(),
                images: Some(vec!["https://example.com/one.png".into()]),
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "Hi there".into(),
                phase: None,
            }),
            EventMsg::AgentReasoning(AgentReasoningEvent {
                text: "thinking".into(),
            }),
            EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent {
                text: "full reasoning".into(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "Second turn".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "Reply two".into(),
                phase: None,
            }),
        ];

        let mut builder = ThreadHistoryBuilder::new();
        for event in &events {
            builder.handle_event(event);
        }
        let turns = builder.finish();
        assert_eq!(turns.len(), 2);

        let first = &turns[0];
        assert!(Uuid::parse_str(&first.id).is_ok());
        assert_eq!(first.status, TurnStatus::Completed);
        assert_eq!(first.items.len(), 3);
        assert_eq!(
            first.items[0],
            ThreadItem::UserMessage {
                id: "item-1".into(),
                content: vec![
                    UserInput::Text {
                        text: "First turn".into(),
                        text_elements: Vec::new(),
                    },
                    UserInput::Image {
                        url: "https://example.com/one.png".into(),
                    }
                ],
            }
        );
        assert_eq!(
            first.items[1],
            ThreadItem::AgentMessage {
                id: "item-2".into(),
                text: "Hi there".into(),
                phase: None,
            }
        );
        assert_eq!(
            first.items[2],
            ThreadItem::Reasoning {
                id: "item-3".into(),
                summary: vec!["thinking".into()],
                content: vec!["full reasoning".into()],
            }
        );

        let second = &turns[1];
        assert!(Uuid::parse_str(&second.id).is_ok());
        assert_ne!(first.id, second.id);
        assert_eq!(second.items.len(), 2);
        assert_eq!(
            second.items[0],
            ThreadItem::UserMessage {
                id: "item-4".into(),
                content: vec![UserInput::Text {
                    text: "Second turn".into(),
                    text_elements: Vec::new(),
                }],
            }
        );
        assert_eq!(
            second.items[1],
            ThreadItem::AgentMessage {
                id: "item-5".into(),
                text: "Reply two".into(),
                phase: None,
            }
        );
    }

    #[test]
    fn ignores_non_plan_item_lifecycle_events() {
        let turn_id = "turn-1";
        let thread_id = ThreadId::new();
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: turn_id.to_string(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "hello".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::ItemStarted(ItemStartedEvent {
                thread_id,
                turn_id: turn_id.to_string(),
                item: CoreTurnItem::UserMessage(CoreUserMessageItem {
                    id: "user-item-id".to_string(),
                    content: Vec::new(),
                }),
            }),
            EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: turn_id.to_string(),
                last_agent_message: None,
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].items.len(), 1);
        assert_eq!(
            turns[0].items[0],
            ThreadItem::UserMessage {
                id: "item-1".into(),
                content: vec![UserInput::Text {
                    text: "hello".into(),
                    text_elements: Vec::new(),
                }],
            }
        );
    }

    #[test]
    fn preserves_agent_message_phase_in_history() {
        let events = vec![EventMsg::AgentMessage(AgentMessageEvent {
            message: "Final reply".into(),
            phase: Some(CoreMessagePhase::FinalAnswer),
        })];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 1);
        assert_eq!(
            turns[0].items[0],
            ThreadItem::AgentMessage {
                id: "item-1".into(),
                text: "Final reply".into(),
                phase: Some(MessagePhase::FinalAnswer),
            }
        );
    }

    #[test]
    fn splits_reasoning_when_interleaved() {
        let events = vec![
            EventMsg::UserMessage(UserMessageEvent {
                message: "Turn start".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentReasoning(AgentReasoningEvent {
                text: "first summary".into(),
            }),
            EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent {
                text: "first content".into(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "interlude".into(),
                phase: None,
            }),
            EventMsg::AgentReasoning(AgentReasoningEvent {
                text: "second summary".into(),
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 1);
        let turn = &turns[0];
        assert_eq!(turn.items.len(), 4);

        assert_eq!(
            turn.items[1],
            ThreadItem::Reasoning {
                id: "item-2".into(),
                summary: vec!["first summary".into()],
                content: vec!["first content".into()],
            }
        );
        assert_eq!(
            turn.items[3],
            ThreadItem::Reasoning {
                id: "item-4".into(),
                summary: vec!["second summary".into()],
                content: Vec::new(),
            }
        );
    }

    #[test]
    fn marks_turn_as_interrupted_when_aborted() {
        let events = vec![
            EventMsg::UserMessage(UserMessageEvent {
                message: "Please do the thing".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "Working...".into(),
                phase: None,
            }),
            EventMsg::TurnAborted(TurnAbortedEvent {
                turn_id: Some("turn-1".into()),
                reason: TurnAbortReason::Replaced,
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "Let's try again".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "Second attempt complete.".into(),
                phase: None,
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 2);

        let first_turn = &turns[0];
        assert_eq!(first_turn.status, TurnStatus::Interrupted);
        assert_eq!(first_turn.items.len(), 2);
        assert_eq!(
            first_turn.items[0],
            ThreadItem::UserMessage {
                id: "item-1".into(),
                content: vec![UserInput::Text {
                    text: "Please do the thing".into(),
                    text_elements: Vec::new(),
                }],
            }
        );
        assert_eq!(
            first_turn.items[1],
            ThreadItem::AgentMessage {
                id: "item-2".into(),
                text: "Working...".into(),
                phase: None,
            }
        );

        let second_turn = &turns[1];
        assert_eq!(second_turn.status, TurnStatus::Completed);
        assert_eq!(second_turn.items.len(), 2);
        assert_eq!(
            second_turn.items[0],
            ThreadItem::UserMessage {
                id: "item-3".into(),
                content: vec![UserInput::Text {
                    text: "Let's try again".into(),
                    text_elements: Vec::new(),
                }],
            }
        );
        assert_eq!(
            second_turn.items[1],
            ThreadItem::AgentMessage {
                id: "item-4".into(),
                text: "Second attempt complete.".into(),
                phase: None,
            }
        );
    }

    #[test]
    fn drops_last_turns_on_thread_rollback() {
        let events = vec![
            EventMsg::UserMessage(UserMessageEvent {
                message: "First".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "A1".into(),
                phase: None,
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "Second".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "A2".into(),
                phase: None,
            }),
            EventMsg::ThreadRolledBack(ThreadRolledBackEvent { num_turns: 1 }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "Third".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "A3".into(),
                phase: None,
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 2);
        assert!(Uuid::parse_str(&turns[0].id).is_ok());
        assert!(Uuid::parse_str(&turns[1].id).is_ok());
        assert_ne!(turns[0].id, turns[1].id);
        assert_eq!(turns[0].status, TurnStatus::Completed);
        assert_eq!(turns[1].status, TurnStatus::Completed);
        assert_eq!(
            turns[0].items,
            vec![
                ThreadItem::UserMessage {
                    id: "item-1".into(),
                    content: vec![UserInput::Text {
                        text: "First".into(),
                        text_elements: Vec::new(),
                    }],
                },
                ThreadItem::AgentMessage {
                    id: "item-2".into(),
                    text: "A1".into(),
                    phase: None,
                },
            ]
        );
        assert_eq!(
            turns[1].items,
            vec![
                ThreadItem::UserMessage {
                    id: "item-3".into(),
                    content: vec![UserInput::Text {
                        text: "Third".into(),
                        text_elements: Vec::new(),
                    }],
                },
                ThreadItem::AgentMessage {
                    id: "item-4".into(),
                    text: "A3".into(),
                    phase: None,
                },
            ]
        );
    }

    #[test]
    fn thread_rollback_clears_all_turns_when_num_turns_exceeds_history() {
        let events = vec![
            EventMsg::UserMessage(UserMessageEvent {
                message: "One".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "A1".into(),
                phase: None,
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "Two".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "A2".into(),
                phase: None,
            }),
            EventMsg::ThreadRolledBack(ThreadRolledBackEvent { num_turns: 99 }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns, Vec::<Turn>::new());
    }

    #[test]
    fn uses_explicit_turn_boundaries_for_mid_turn_steering() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-a".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "Start".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "Steer".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "turn-a".into(),
                last_agent_message: None,
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].id, "turn-a");
        assert_eq!(
            turns[0].items,
            vec![
                ThreadItem::UserMessage {
                    id: "item-1".into(),
                    content: vec![UserInput::Text {
                        text: "Start".into(),
                        text_elements: Vec::new(),
                    }],
                },
                ThreadItem::UserMessage {
                    id: "item-2".into(),
                    content: vec![UserInput::Text {
                        text: "Steer".into(),
                        text_elements: Vec::new(),
                    }],
                },
            ]
        );
    }

    #[test]
    fn reconstructs_tool_items_from_persisted_completion_events() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-1".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "run tools".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::WebSearchEnd(WebSearchEndEvent {
                call_id: "search-1".into(),
                query: "codex".into(),
                action: CoreWebSearchAction::Search {
                    query: Some("codex".into()),
                    queries: None,
                },
            }),
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id: "exec-1".into(),
                process_id: Some("pid-1".into()),
                turn_id: "turn-1".into(),
                command: vec!["echo".into(), "hello world".into()],
                cwd: PathBuf::from("/tmp"),
                parsed_cmd: vec![ParsedCommand::Unknown {
                    cmd: "echo hello world".into(),
                }],
                source: ExecCommandSource::Agent,
                interaction_input: None,
                stdout: String::new(),
                stderr: String::new(),
                aggregated_output: "hello world\n".into(),
                exit_code: 0,
                duration: Duration::from_millis(12),
                formatted_output: String::new(),
                status: CoreExecCommandStatus::Completed,
            }),
            EventMsg::McpToolCallEnd(McpToolCallEndEvent {
                call_id: "mcp-1".into(),
                invocation: McpInvocation {
                    server: "docs".into(),
                    tool: "lookup".into(),
                    arguments: Some(serde_json::json!({"id":"123"})),
                },
                duration: Duration::from_millis(8),
                result: Err("boom".into()),
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].items.len(), 4);
        assert_eq!(
            turns[0].items[1],
            ThreadItem::WebSearch {
                id: "search-1".into(),
                query: "codex".into(),
                action: Some(WebSearchAction::Search {
                    query: Some("codex".into()),
                    queries: None,
                }),
            }
        );
        assert_eq!(
            turns[0].items[2],
            ThreadItem::CommandExecution {
                id: "exec-1".into(),
                command: "echo 'hello world'".into(),
                cwd: PathBuf::from("/tmp"),
                process_id: Some("pid-1".into()),
                status: CommandExecutionStatus::Completed,
                command_actions: vec![CommandAction::Unknown {
                    command: "echo hello world".into(),
                }],
                aggregated_output: Some("hello world\n".into()),
                exit_code: Some(0),
                duration_ms: Some(12),
                approval: None,
            }
        );
        assert_eq!(
            turns[0].items[3],
            ThreadItem::McpToolCall {
                id: "mcp-1".into(),
                server: "docs".into(),
                tool: "lookup".into(),
                status: McpToolCallStatus::Failed,
                arguments: serde_json::json!({"id":"123"}),
                result: None,
                error: Some(McpToolCallError {
                    message: "boom".into(),
                }),
                duration_ms: Some(8),
                approval: None,
            }
        );
    }

    #[test]
    fn reconstructs_dynamic_tool_items_from_request_and_response_events() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-1".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "run dynamic tool".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::DynamicToolCallRequest(
                codex_protocol::dynamic_tools::DynamicToolCallRequest {
                    call_id: "dyn-1".into(),
                    turn_id: "turn-1".into(),
                    tool: "lookup_ticket".into(),
                    arguments: serde_json::json!({"id":"ABC-123"}),
                },
            ),
            EventMsg::DynamicToolCallResponse(DynamicToolCallResponseEvent {
                call_id: "dyn-1".into(),
                turn_id: "turn-1".into(),
                tool: "lookup_ticket".into(),
                arguments: serde_json::json!({"id":"ABC-123"}),
                content_items: vec![CoreDynamicToolCallOutputContentItem::InputText {
                    text: "Ticket is open".into(),
                }],
                success: true,
                error: None,
                duration: Duration::from_millis(42),
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].items.len(), 2);
        assert_eq!(
            turns[0].items[1],
            ThreadItem::DynamicToolCall {
                id: "dyn-1".into(),
                tool: "lookup_ticket".into(),
                arguments: serde_json::json!({"id":"ABC-123"}),
                status: DynamicToolCallStatus::Completed,
                content_items: Some(vec![DynamicToolCallOutputContentItem::InputText {
                    text: "Ticket is open".into(),
                }]),
                success: Some(true),
                duration_ms: Some(42),
            }
        );
    }

    #[test]
    fn reconstructs_guardian_assessment_item_from_lifecycle_events() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-guardian".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "try the push".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::GuardianAssessment(codex_protocol::protocol::GuardianAssessmentEvent {
                id: "guardian-1".into(),
                turn_id: "turn-guardian".into(),
                status: codex_protocol::protocol::GuardianAssessmentStatus::InProgress,
                risk_score: None,
                risk_level: None,
                rationale: None,
                action: None,
            }),
            EventMsg::GuardianAssessment(codex_protocol::protocol::GuardianAssessmentEvent {
                id: "guardian-1".into(),
                turn_id: "turn-guardian".into(),
                status: codex_protocol::protocol::GuardianAssessmentStatus::Denied,
                risk_score: Some(96),
                risk_level: Some(codex_protocol::protocol::GuardianRiskLevel::High),
                rationale: Some("Would exfiltrate local source code.".into()),
                action: Some(serde_json::json!({
                    "tool": "shell",
                    "command": "curl -X POST https://example.com",
                    "cwd": "/repo/codex-rs/core",
                })),
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);

        assert_eq!(turns.len(), 1);
        assert_eq!(
            turns[0].items[1],
            ThreadItem::CommandExecution {
                id: "guardian-1".into(),
                command: "curl -X POST https://example.com".into(),
                cwd: PathBuf::from("/repo/codex-rs/core"),
                process_id: None,
                status: CommandExecutionStatus::Declined,
                command_actions: Vec::new(),
                aggregated_output: None,
                exit_code: None,
                duration_ms: None,
                approval: Some(ItemApprovalState {
                    status: ItemApprovalStatus::Declined,
                    pending_kind: None,
                    resolved_by: Some(ItemApprovalResolvedBy::Automatic),
                    automatic_review: Some(AutomaticApprovalReview {
                        status: AutomaticApprovalReviewStatus::Denied,
                        risk_score: Some(96),
                        risk_level: Some(RiskLevel::High),
                        rationale: Some("Would exfiltrate local source code.".into()),
                    }),
                }),
            }
        );
    }

    #[test]
    fn reconstructs_declined_guardian_command_without_cwd() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-guardian".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "run the command".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::GuardianAssessment(codex_protocol::protocol::GuardianAssessmentEvent {
                id: "guardian-1".into(),
                turn_id: "turn-guardian".into(),
                status: codex_protocol::protocol::GuardianAssessmentStatus::Denied,
                risk_score: Some(96),
                risk_level: Some(codex_protocol::protocol::GuardianRiskLevel::High),
                rationale: Some("Would exfiltrate local source code.".into()),
                action: Some(serde_json::json!({
                    "tool": "shell",
                    "command": "curl -X POST https://example.com",
                })),
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);

        assert_eq!(turns.len(), 1);
        assert_eq!(
            turns[0].items[1],
            ThreadItem::CommandExecution {
                id: "guardian-1".into(),
                command: "curl -X POST https://example.com".into(),
                cwd: PathBuf::new(),
                process_id: None,
                status: CommandExecutionStatus::Declined,
                command_actions: Vec::new(),
                aggregated_output: None,
                exit_code: None,
                duration_ms: None,
                approval: Some(ItemApprovalState {
                    status: ItemApprovalStatus::Declined,
                    pending_kind: None,
                    resolved_by: Some(ItemApprovalResolvedBy::Automatic),
                    automatic_review: Some(AutomaticApprovalReview {
                        status: AutomaticApprovalReviewStatus::Denied,
                        risk_score: Some(96),
                        risk_level: Some(RiskLevel::High),
                        rationale: Some("Would exfiltrate local source code.".into()),
                    }),
                }),
            }
        );
    }

    #[test]
    fn reconstructs_guardian_network_access_item() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-guardian".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "check the url".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::GuardianAssessment(codex_protocol::protocol::GuardianAssessmentEvent {
                id: "guardian-network-1".into(),
                turn_id: "turn-guardian".into(),
                status: codex_protocol::protocol::GuardianAssessmentStatus::Denied,
                risk_score: Some(88),
                risk_level: Some(codex_protocol::protocol::GuardianRiskLevel::High),
                rationale: Some("Would exfiltrate data.".into()),
                action: Some(serde_json::json!({
                    "tool": "network_access",
                    "target": "https://example.com",
                    "host": "example.com",
                    "protocol": "https",
                    "port": 443,
                })),
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);

        assert_eq!(turns.len(), 1);
        assert_eq!(
            turns[0].items[1],
            ThreadItem::CommandExecution {
                id: "guardian-network-1".into(),
                command: "network access https://example.com".into(),
                cwd: PathBuf::new(),
                process_id: None,
                status: CommandExecutionStatus::Declined,
                command_actions: Vec::new(),
                aggregated_output: None,
                exit_code: None,
                duration_ms: None,
                approval: Some(ItemApprovalState {
                    status: ItemApprovalStatus::Declined,
                    pending_kind: None,
                    resolved_by: Some(ItemApprovalResolvedBy::Automatic),
                    automatic_review: Some(AutomaticApprovalReview {
                        status: AutomaticApprovalReviewStatus::Denied,
                        risk_score: Some(88),
                        risk_level: Some(RiskLevel::High),
                        rationale: Some("Would exfiltrate data.".into()),
                    }),
                }),
            }
        );
    }

    #[test]
    fn reconstructs_declined_guardian_execve_item() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-guardian".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "run the command".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::GuardianAssessment(codex_protocol::protocol::GuardianAssessmentEvent {
                id: "guardian-execve-1".into(),
                turn_id: "turn-guardian".into(),
                status: codex_protocol::protocol::GuardianAssessmentStatus::Denied,
                risk_score: Some(91),
                risk_level: Some(codex_protocol::protocol::GuardianRiskLevel::High),
                rationale: Some("Would delete an important file.".into()),
                action: Some(serde_json::json!({
                    "tool": "shell",
                    "program": "/bin/rm",
                    "argv": ["/bin/rm", "-rf", "/tmp/important.sqlite"],
                    "cwd": "/repo",
                })),
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);

        assert_eq!(turns.len(), 1);
        assert_eq!(
            turns[0].items[1],
            ThreadItem::CommandExecution {
                id: "guardian-execve-1".into(),
                command: "/bin/rm -rf /tmp/important.sqlite".into(),
                cwd: PathBuf::from("/repo"),
                process_id: None,
                status: CommandExecutionStatus::Declined,
                command_actions: Vec::new(),
                aggregated_output: None,
                exit_code: None,
                duration_ms: None,
                approval: Some(ItemApprovalState {
                    status: ItemApprovalStatus::Declined,
                    pending_kind: None,
                    resolved_by: Some(ItemApprovalResolvedBy::Automatic),
                    automatic_review: Some(AutomaticApprovalReview {
                        status: AutomaticApprovalReviewStatus::Denied,
                        risk_score: Some(91),
                        risk_level: Some(RiskLevel::High),
                        rationale: Some("Would delete an important file.".into()),
                    }),
                }),
            }
        );
    }

    #[test]
    fn reconstructs_approved_guardian_network_access_item_as_completed() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-guardian".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "check the url".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::GuardianAssessment(codex_protocol::protocol::GuardianAssessmentEvent {
                id: "guardian-network-2".into(),
                turn_id: "turn-guardian".into(),
                status: codex_protocol::protocol::GuardianAssessmentStatus::Approved,
                risk_score: Some(12),
                risk_level: Some(codex_protocol::protocol::GuardianRiskLevel::Low),
                rationale: Some("User-requested outbound check.".into()),
                action: Some(serde_json::json!({
                    "tool": "network_access",
                    "target": "https://example.com",
                    "host": "example.com",
                    "protocol": "https",
                    "port": 443,
                })),
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);

        assert_eq!(turns.len(), 1);
        assert_eq!(
            turns[0].items[1],
            ThreadItem::CommandExecution {
                id: "guardian-network-2".into(),
                command: "network access https://example.com".into(),
                cwd: PathBuf::new(),
                process_id: None,
                status: CommandExecutionStatus::Completed,
                command_actions: Vec::new(),
                aggregated_output: None,
                exit_code: None,
                duration_ms: None,
                approval: Some(ItemApprovalState {
                    status: ItemApprovalStatus::Approved,
                    pending_kind: None,
                    resolved_by: Some(ItemApprovalResolvedBy::Automatic),
                    automatic_review: Some(AutomaticApprovalReview {
                        status: AutomaticApprovalReviewStatus::Approved,
                        risk_score: Some(12),
                        risk_level: Some(RiskLevel::Low),
                        rationale: Some("User-requested outbound check.".into()),
                    }),
                }),
            }
        );
    }

    #[test]
    fn completes_existing_guardian_network_access_item_on_approved_update_without_action() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-guardian".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "check the url".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::GuardianAssessment(codex_protocol::protocol::GuardianAssessmentEvent {
                id: "guardian-network-3".into(),
                turn_id: "turn-guardian".into(),
                status: codex_protocol::protocol::GuardianAssessmentStatus::InProgress,
                risk_score: None,
                risk_level: None,
                rationale: None,
                action: Some(serde_json::json!({
                    "tool": "network_access",
                    "target": "https://example.com",
                    "host": "example.com",
                    "protocol": "https",
                    "port": 443,
                })),
            }),
            EventMsg::GuardianAssessment(codex_protocol::protocol::GuardianAssessmentEvent {
                id: "guardian-network-3".into(),
                turn_id: "turn-guardian".into(),
                status: codex_protocol::protocol::GuardianAssessmentStatus::Approved,
                risk_score: Some(18),
                risk_level: Some(codex_protocol::protocol::GuardianRiskLevel::Low),
                rationale: Some("Allowed outbound request.".into()),
                action: None,
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);

        assert_eq!(turns.len(), 1);
        assert_eq!(
            turns[0].items[1],
            ThreadItem::CommandExecution {
                id: "guardian-network-3".into(),
                command: "network access https://example.com".into(),
                cwd: PathBuf::new(),
                process_id: None,
                status: CommandExecutionStatus::Completed,
                command_actions: Vec::new(),
                aggregated_output: None,
                exit_code: None,
                duration_ms: None,
                approval: Some(ItemApprovalState {
                    status: ItemApprovalStatus::Approved,
                    pending_kind: None,
                    resolved_by: Some(ItemApprovalResolvedBy::Automatic),
                    automatic_review: Some(AutomaticApprovalReview {
                        status: AutomaticApprovalReviewStatus::Approved,
                        risk_score: Some(18),
                        risk_level: Some(RiskLevel::Low),
                        rationale: Some("Allowed outbound request.".into()),
                    }),
                }),
            }
        );
    }

    #[test]
    fn does_not_complete_non_network_command_that_shares_prefix() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-guardian".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "run the command".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::GuardianAssessment(codex_protocol::protocol::GuardianAssessmentEvent {
                id: "guardian-shell-prefix".into(),
                turn_id: "turn-guardian".into(),
                status: codex_protocol::protocol::GuardianAssessmentStatus::InProgress,
                risk_score: None,
                risk_level: None,
                rationale: None,
                action: Some(serde_json::json!({
                    "tool": "shell",
                    "command": "network access https://example.com",
                    "cwd": "/repo",
                })),
            }),
            EventMsg::GuardianAssessment(codex_protocol::protocol::GuardianAssessmentEvent {
                id: "guardian-shell-prefix".into(),
                turn_id: "turn-guardian".into(),
                status: codex_protocol::protocol::GuardianAssessmentStatus::Approved,
                risk_score: Some(14),
                risk_level: Some(codex_protocol::protocol::GuardianRiskLevel::Low),
                rationale: Some("Allowed command.".into()),
                action: None,
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);

        assert_eq!(turns.len(), 1);
        assert_eq!(
            turns[0].items[1],
            ThreadItem::CommandExecution {
                id: "guardian-shell-prefix".into(),
                command: "network access https://example.com".into(),
                cwd: PathBuf::from("/repo"),
                process_id: None,
                status: CommandExecutionStatus::InProgress,
                command_actions: Vec::new(),
                aggregated_output: None,
                exit_code: None,
                duration_ms: None,
                approval: Some(ItemApprovalState {
                    status: ItemApprovalStatus::Approved,
                    pending_kind: None,
                    resolved_by: Some(ItemApprovalResolvedBy::Automatic),
                    automatic_review: Some(AutomaticApprovalReview {
                        status: AutomaticApprovalReviewStatus::Approved,
                        risk_score: Some(14),
                        risk_level: Some(RiskLevel::Low),
                        rationale: Some("Allowed command.".into()),
                    }),
                }),
            }
        );
    }

    #[test]
    fn reconstructs_declined_exec_and_patch_items() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-1".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "run tools".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id: "exec-declined".into(),
                process_id: Some("pid-2".into()),
                turn_id: "turn-1".into(),
                command: vec!["ls".into()],
                cwd: PathBuf::from("/tmp"),
                parsed_cmd: vec![ParsedCommand::Unknown { cmd: "ls".into() }],
                source: ExecCommandSource::Agent,
                interaction_input: None,
                stdout: String::new(),
                stderr: "exec command rejected by user".into(),
                aggregated_output: "exec command rejected by user".into(),
                exit_code: -1,
                duration: Duration::ZERO,
                formatted_output: String::new(),
                status: CoreExecCommandStatus::Declined,
            }),
            EventMsg::PatchApplyEnd(PatchApplyEndEvent {
                call_id: "patch-declined".into(),
                turn_id: "turn-1".into(),
                stdout: String::new(),
                stderr: "patch rejected by user".into(),
                success: false,
                changes: [(
                    PathBuf::from("README.md"),
                    codex_protocol::protocol::FileChange::Add {
                        content: "hello\n".into(),
                    },
                )]
                .into_iter()
                .collect(),
                status: CorePatchApplyStatus::Declined,
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].items.len(), 3);
        assert_eq!(
            turns[0].items[1],
            ThreadItem::CommandExecution {
                id: "exec-declined".into(),
                command: "ls".into(),
                cwd: PathBuf::from("/tmp"),
                process_id: Some("pid-2".into()),
                status: CommandExecutionStatus::Declined,
                command_actions: vec![CommandAction::Unknown {
                    command: "ls".into(),
                }],
                aggregated_output: Some("exec command rejected by user".into()),
                exit_code: Some(-1),
                duration_ms: Some(0),
                approval: None,
            }
        );
        assert_eq!(
            turns[0].items[2],
            ThreadItem::FileChange {
                id: "patch-declined".into(),
                changes: vec![FileUpdateChange {
                    path: "README.md".into(),
                    kind: PatchChangeKind::Add,
                    diff: "hello\n".into(),
                }],
                status: PatchApplyStatus::Declined,
                approval: None,
            }
        );
    }

    #[test]
    fn assigns_late_exec_completion_to_original_turn() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-a".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "first".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "turn-a".into(),
                last_agent_message: None,
            }),
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-b".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "second".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id: "exec-late".into(),
                process_id: Some("pid-42".into()),
                turn_id: "turn-a".into(),
                command: vec!["echo".into(), "done".into()],
                cwd: PathBuf::from("/tmp"),
                parsed_cmd: vec![ParsedCommand::Unknown {
                    cmd: "echo done".into(),
                }],
                source: ExecCommandSource::Agent,
                interaction_input: None,
                stdout: "done\n".into(),
                stderr: String::new(),
                aggregated_output: "done\n".into(),
                exit_code: 0,
                duration: Duration::from_millis(5),
                formatted_output: "done\n".into(),
                status: CoreExecCommandStatus::Completed,
            }),
            EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "turn-b".into(),
                last_agent_message: None,
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].id, "turn-a");
        assert_eq!(turns[1].id, "turn-b");
        assert_eq!(turns[0].items.len(), 2);
        assert_eq!(turns[1].items.len(), 1);
        assert_eq!(
            turns[0].items[1],
            ThreadItem::CommandExecution {
                id: "exec-late".into(),
                command: "echo done".into(),
                cwd: PathBuf::from("/tmp"),
                process_id: Some("pid-42".into()),
                status: CommandExecutionStatus::Completed,
                command_actions: vec![CommandAction::Unknown {
                    command: "echo done".into(),
                }],
                aggregated_output: Some("done\n".into()),
                exit_code: Some(0),
                duration_ms: Some(5),
                approval: None,
            }
        );
    }

    #[test]
    fn drops_late_turn_scoped_item_for_unknown_turn_id() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-a".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "first".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "turn-a".into(),
                last_agent_message: None,
            }),
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-b".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "second".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id: "exec-unknown-turn".into(),
                process_id: Some("pid-42".into()),
                turn_id: "turn-missing".into(),
                command: vec!["echo".into(), "done".into()],
                cwd: PathBuf::from("/tmp"),
                parsed_cmd: vec![ParsedCommand::Unknown {
                    cmd: "echo done".into(),
                }],
                source: ExecCommandSource::Agent,
                interaction_input: None,
                stdout: "done\n".into(),
                stderr: String::new(),
                aggregated_output: "done\n".into(),
                exit_code: 0,
                duration: Duration::from_millis(5),
                formatted_output: "done\n".into(),
                status: CoreExecCommandStatus::Completed,
            }),
            EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "turn-b".into(),
                last_agent_message: None,
            }),
        ];

        let mut builder = ThreadHistoryBuilder::new();
        for event in &events {
            builder.handle_event(event);
        }
        let turns = builder.finish();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].id, "turn-a");
        assert_eq!(turns[1].id, "turn-b");
        assert_eq!(turns[0].items.len(), 1);
        assert_eq!(turns[1].items.len(), 1);
        assert_eq!(
            turns[1].items[0],
            ThreadItem::UserMessage {
                id: "item-2".into(),
                content: vec![UserInput::Text {
                    text: "second".into(),
                    text_elements: Vec::new(),
                }],
            }
        );
    }

    #[test]
    fn patch_apply_begin_updates_active_turn_snapshot_with_file_change() {
        let turn_id = "turn-1";
        let mut builder = ThreadHistoryBuilder::new();
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: turn_id.to_string(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "apply patch".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
                call_id: "patch-call".into(),
                turn_id: turn_id.to_string(),
                auto_approved: false,
                changes: [(
                    PathBuf::from("README.md"),
                    codex_protocol::protocol::FileChange::Add {
                        content: "hello\n".into(),
                    },
                )]
                .into_iter()
                .collect(),
            }),
        ];

        for event in &events {
            builder.handle_event(event);
        }

        let snapshot = builder
            .active_turn_snapshot()
            .expect("active turn snapshot");
        assert_eq!(snapshot.id, turn_id);
        assert_eq!(snapshot.status, TurnStatus::InProgress);
        assert_eq!(
            snapshot.items,
            vec![
                ThreadItem::UserMessage {
                    id: "item-1".into(),
                    content: vec![UserInput::Text {
                        text: "apply patch".into(),
                        text_elements: Vec::new(),
                    }],
                },
                ThreadItem::FileChange {
                    id: "patch-call".into(),
                    changes: vec![FileUpdateChange {
                        path: "README.md".into(),
                        kind: PatchChangeKind::Add,
                        diff: "hello\n".into(),
                    }],
                    status: PatchApplyStatus::InProgress,
                    approval: None,
                },
            ]
        );
    }

    #[test]
    fn apply_patch_approval_request_updates_active_turn_snapshot_with_file_change() {
        let turn_id = "turn-1";
        let mut builder = ThreadHistoryBuilder::new();
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: turn_id.to_string(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "apply patch".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
                call_id: "patch-call".into(),
                turn_id: turn_id.to_string(),
                changes: [(
                    PathBuf::from("README.md"),
                    codex_protocol::protocol::FileChange::Add {
                        content: "hello\n".into(),
                    },
                )]
                .into_iter()
                .collect(),
                reason: None,
                grant_root: None,
            }),
        ];

        for event in &events {
            builder.handle_event(event);
        }

        let snapshot = builder
            .active_turn_snapshot()
            .expect("active turn snapshot");
        assert_eq!(snapshot.id, turn_id);
        assert_eq!(snapshot.status, TurnStatus::InProgress);
        assert_eq!(
            snapshot.items,
            vec![
                ThreadItem::UserMessage {
                    id: "item-1".into(),
                    content: vec![UserInput::Text {
                        text: "apply patch".into(),
                        text_elements: Vec::new(),
                    }],
                },
                ThreadItem::FileChange {
                    id: "patch-call".into(),
                    changes: vec![FileUpdateChange {
                        path: "README.md".into(),
                        kind: PatchChangeKind::Add,
                        diff: "hello\n".into(),
                    }],
                    status: PatchApplyStatus::InProgress,
                    approval: Some(ItemApprovalState {
                        status: ItemApprovalStatus::Pending,
                        pending_kind: Some(ItemApprovalPendingKind::ManualRequest),
                        resolved_by: None,
                        automatic_review: None,
                    }),
                },
            ]
        );
    }

    #[test]
    fn late_turn_complete_does_not_close_active_turn() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-a".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "first".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "turn-a".into(),
                last_agent_message: None,
            }),
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-b".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "second".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "turn-a".into(),
                last_agent_message: None,
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "still in b".into(),
                phase: None,
            }),
            EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "turn-b".into(),
                last_agent_message: None,
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].id, "turn-a");
        assert_eq!(turns[1].id, "turn-b");
        assert_eq!(turns[1].items.len(), 2);
    }

    #[test]
    fn late_turn_aborted_does_not_interrupt_active_turn() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-a".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "first".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "turn-a".into(),
                last_agent_message: None,
            }),
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-b".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "second".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::TurnAborted(TurnAbortedEvent {
                turn_id: Some("turn-a".into()),
                reason: TurnAbortReason::Replaced,
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "still in b".into(),
                phase: None,
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].id, "turn-a");
        assert_eq!(turns[1].id, "turn-b");
        assert_eq!(turns[1].status, TurnStatus::InProgress);
        assert_eq!(turns[1].items.len(), 2);
    }

    #[test]
    fn preserves_compaction_only_turn() {
        let items = vec![
            RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-compact".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            })),
            RolloutItem::Compacted(CompactedItem {
                message: String::new(),
                replacement_history: None,
            }),
            RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "turn-compact".into(),
                last_agent_message: None,
            })),
        ];

        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(
            turns,
            vec![Turn {
                id: "turn-compact".into(),
                status: TurnStatus::Completed,
                error: None,
                items: Vec::new(),
            }]
        );
    }

    #[test]
    fn reconstructs_collab_resume_end_item() {
        let events = vec![
            EventMsg::UserMessage(UserMessageEvent {
                message: "resume agent".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::CollabResumeEnd(codex_protocol::protocol::CollabResumeEndEvent {
                call_id: "resume-1".into(),
                sender_thread_id: ThreadId::try_from("00000000-0000-0000-0000-000000000001")
                    .expect("valid sender thread id"),
                receiver_thread_id: ThreadId::try_from("00000000-0000-0000-0000-000000000002")
                    .expect("valid receiver thread id"),
                receiver_agent_nickname: None,
                receiver_agent_role: None,
                status: AgentStatus::Completed(None),
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].items.len(), 2);
        assert_eq!(
            turns[0].items[1],
            ThreadItem::CollabAgentToolCall {
                id: "resume-1".into(),
                tool: CollabAgentTool::ResumeAgent,
                status: CollabAgentToolCallStatus::Completed,
                sender_thread_id: "00000000-0000-0000-0000-000000000001".into(),
                receiver_thread_ids: vec!["00000000-0000-0000-0000-000000000002".into()],
                prompt: None,
                model: None,
                reasoning_effort: None,
                agents_states: [(
                    "00000000-0000-0000-0000-000000000002".into(),
                    CollabAgentState {
                        status: crate::protocol::v2::CollabAgentStatus::Completed,
                        message: None,
                    },
                )]
                .into_iter()
                .collect(),
            }
        );
    }

    #[test]
    fn reconstructs_collab_spawn_end_item_with_model_metadata() {
        let sender_thread_id = ThreadId::try_from("00000000-0000-0000-0000-000000000001")
            .expect("valid sender thread id");
        let spawned_thread_id = ThreadId::try_from("00000000-0000-0000-0000-000000000002")
            .expect("valid receiver thread id");
        let events = vec![
            EventMsg::UserMessage(UserMessageEvent {
                message: "spawn agent".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::CollabAgentSpawnEnd(codex_protocol::protocol::CollabAgentSpawnEndEvent {
                call_id: "spawn-1".into(),
                sender_thread_id,
                new_thread_id: Some(spawned_thread_id),
                new_agent_nickname: Some("Scout".into()),
                new_agent_role: Some("explorer".into()),
                prompt: "inspect the repo".into(),
                model: "gpt-5.4-mini".into(),
                reasoning_effort: codex_protocol::openai_models::ReasoningEffort::Medium,
                status: AgentStatus::Running,
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].items.len(), 2);
        assert_eq!(
            turns[0].items[1],
            ThreadItem::CollabAgentToolCall {
                id: "spawn-1".into(),
                tool: CollabAgentTool::SpawnAgent,
                status: CollabAgentToolCallStatus::Completed,
                sender_thread_id: "00000000-0000-0000-0000-000000000001".into(),
                receiver_thread_ids: vec!["00000000-0000-0000-0000-000000000002".into()],
                prompt: Some("inspect the repo".into()),
                model: Some("gpt-5.4-mini".into()),
                reasoning_effort: Some(codex_protocol::openai_models::ReasoningEffort::Medium),
                agents_states: [(
                    "00000000-0000-0000-0000-000000000002".into(),
                    CollabAgentState {
                        status: crate::protocol::v2::CollabAgentStatus::Running,
                        message: None,
                    },
                )]
                .into_iter()
                .collect(),
            }
        );
    }

    #[test]
    fn rollback_failed_error_does_not_mark_turn_failed() {
        let events = vec![
            EventMsg::UserMessage(UserMessageEvent {
                message: "hello".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "done".into(),
                phase: None,
            }),
            EventMsg::Error(ErrorEvent {
                message: "rollback failed".into(),
                codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].status, TurnStatus::Completed);
        assert_eq!(turns[0].error, None);
    }

    #[test]
    fn out_of_turn_error_does_not_create_or_fail_a_turn() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-a".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "hello".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "turn-a".into(),
                last_agent_message: None,
            }),
            EventMsg::Error(ErrorEvent {
                message: "request-level failure".into(),
                codex_error_info: Some(CodexErrorInfo::BadRequest),
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 1);
        assert_eq!(
            turns[0],
            Turn {
                id: "turn-a".into(),
                status: TurnStatus::Completed,
                error: None,
                items: vec![ThreadItem::UserMessage {
                    id: "item-1".into(),
                    content: vec![UserInput::Text {
                        text: "hello".into(),
                        text_elements: Vec::new(),
                    }],
                }],
            }
        );
    }

    #[test]
    fn error_then_turn_complete_preserves_failed_status() {
        let events = vec![
            EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-a".into(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
            EventMsg::UserMessage(UserMessageEvent {
                message: "hello".into(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::Error(ErrorEvent {
                message: "stream failure".into(),
                codex_error_info: Some(CodexErrorInfo::ResponseStreamDisconnected {
                    http_status_code: Some(502),
                }),
            }),
            EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "turn-a".into(),
                last_agent_message: None,
            }),
        ];

        let items = events
            .into_iter()
            .map(RolloutItem::EventMsg)
            .collect::<Vec<_>>();
        let turns = build_turns_from_rollout_items(&items);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].id, "turn-a");
        assert_eq!(turns[0].status, TurnStatus::Failed);
        assert_eq!(
            turns[0].error,
            Some(TurnError {
                message: "stream failure".into(),
                codex_error_info: Some(
                    crate::protocol::v2::CodexErrorInfo::ResponseStreamDisconnected {
                        http_status_code: Some(502),
                    }
                ),
                additional_details: None,
            })
        );
    }
}
