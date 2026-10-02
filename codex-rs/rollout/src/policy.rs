use std::borrow::Cow;

use crate::RolloutItem;
use crate::protocol::EventMsg;
use codex_extension_items::ExtensionItem;
use codex_protocol::items::CommandExecutionItem;
use codex_protocol::items::McpToolCallItem;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::SubAgentActivityKind;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_utils_output_truncation::truncate_mcp_tool_result;
use codex_utils_string::truncate_middle_with_marker;

const PERSISTED_MCP_RESULT_MAX_BYTES: usize = 64 * 1024;
pub(super) const PERSISTED_COMMAND_OUTPUT_MAX_BYTES: usize = 64 * 1024;
const PERSISTED_COMMAND_OUTPUT_TRUNCATION_MARKER: &str =
    "\n... command output truncated for persistence ...\n";

/// Returns the authoritative durable representation of a rollout item.
pub fn persisted_rollout_item(
    item: &RolloutItem,
    history_mode: ThreadHistoryMode,
) -> Option<Cow<'_, RolloutItem>> {
    match item {
        RolloutItem::ResponseItem(response_item) => {
            should_persist_response_item(&response_item.item).then_some(Cow::Borrowed(item))
        }
        RolloutItem::InterAgentCommunication(_)
        | RolloutItem::InterAgentCommunicationMetadata { .. } => Some(Cow::Borrowed(item)),
        RolloutItem::EventMsg(ev) => {
            persisted_event_msg(ev, history_mode).map(|event| match event {
                Cow::Borrowed(_) => Cow::Borrowed(item),
                Cow::Owned(event) => Cow::Owned(RolloutItem::EventMsg(event)),
            })
        }
        RolloutItem::RealtimeItem(_) => {
            matches!(history_mode, ThreadHistoryMode::Paginated).then_some(Cow::Borrowed(item))
        }
        // Persist Codex executive markers so we can analyze flows (e.g., compaction, API turns).
        RolloutItem::Compacted(_)
        | RolloutItem::TurnContext(_)
        | RolloutItem::TokenUsageRecord(_)
        | RolloutItem::WorldState(_)
        | RolloutItem::RetainedContext(_)
        | RolloutItem::SecurityRiskScore(_)
        | RolloutItem::SessionMeta(_) => Some(Cow::Borrowed(item)),
    }
}

/// Return the rollout items that should be persisted for a live append.
pub fn persisted_rollout_items(
    items: &[RolloutItem],
    history_mode: ThreadHistoryMode,
) -> Vec<RolloutItem> {
    items
        .iter()
        .filter_map(|item| persisted_rollout_item(item, history_mode).map(Cow::into_owned))
        .collect()
}

/// Return the owned rollout items that should be persisted for a live append.
pub fn into_persisted_rollout_items(
    items: Vec<RolloutItem>,
    history_mode: ThreadHistoryMode,
) -> Vec<RolloutItem> {
    items
        .into_iter()
        .filter_map(|item| match persisted_rollout_item(&item, history_mode) {
            Some(Cow::Borrowed(_)) => Some(item),
            Some(Cow::Owned(item)) => Some(item),
            None => None,
        })
        .collect()
}

/// Whether a `ResponseItem` should be persisted in rollout files.
#[inline]
pub fn should_persist_response_item(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::AdditionalTools { .. }
        | ResponseItem::Message { .. }
        | ResponseItem::AgentMessage { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::ToolSearchOutput { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::ConfigurationUpdate { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::ContextCompaction { .. } => true,
        ResponseItem::CompactionTrigger { .. } | ResponseItem::Other => false,
    }
}

/// Whether a `ResponseItem` should be persisted for the memories.
#[inline]
pub fn should_persist_response_item_for_memories(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { role, .. } => role != "developer",
        ResponseItem::AgentMessage { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::ToolSearchOutput { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::WebSearchCall { .. } => true,
        ResponseItem::AdditionalTools { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::ConfigurationUpdate { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::CompactionTrigger { .. }
        | ResponseItem::ContextCompaction { .. }
        | ResponseItem::Other => false,
    }
}

fn persisted_event_msg(
    ev: &EventMsg,
    history_mode: ThreadHistoryMode,
) -> Option<Cow<'_, EventMsg>> {
    match ev {
        EventMsg::ItemCompleted(event) => persisted_item_completed_event(ev, event, history_mode),
        EventMsg::TokenCount(_)
        | EventMsg::ThreadGoalUpdated(_)
        | EventMsg::ThreadRolledBack(_)
        | EventMsg::TurnAborted(_)
        | EventMsg::TurnStarted(_)
        | EventMsg::TurnComplete(_)
        | EventMsg::ThreadSettingsApplied(_) => Some(Cow::Borrowed(ev)),

        // Only persist these legacy events when the thread's history mode is Legacy.
        // New, paginated rollouts persist ItemCompleted events with TurnItems.
        EventMsg::UserMessage(_)
        | EventMsg::AgentMessage(_)
        | EventMsg::AgentReasoning(_)
        | EventMsg::AgentReasoningRawContent(_)
        | EventMsg::EnteredReviewMode(_)
        | EventMsg::ExitedReviewMode(_)
        | EventMsg::PatchApplyEnd(_)
        | EventMsg::ContextCompacted(_)
        | EventMsg::McpToolCallEnd(_)
        | EventMsg::WebSearchEnd(_)
        | EventMsg::ImageGenerationEnd(_) => {
            matches!(history_mode, ThreadHistoryMode::Legacy).then_some(Cow::Borrowed(ev))
        }
        EventMsg::SubAgentActivity(event) => (matches!(history_mode, ThreadHistoryMode::Legacy)
            && event.kind != SubAgentActivityKind::Completed)
            .then_some(Cow::Borrowed(ev)),

        // Transient, non-durable events.
        EventMsg::Error(_)
        | EventMsg::ThreadQueueChanged(_)
        | EventMsg::GuardianAssessment(_)
        | EventMsg::ExecCommandEnd(_)
        | EventMsg::ViewImageToolCall(_)
        | EventMsg::CollabAgentSpawnEnd(_)
        | EventMsg::CollabAgentInteractionEnd(_)
        | EventMsg::CollabWaitingEnd(_)
        | EventMsg::CollabCloseEnd(_)
        | EventMsg::CollabResumeEnd(_)
        | EventMsg::DynamicToolCallRequest(_)
        | EventMsg::DynamicToolCallResponse(_)
        | EventMsg::Warning(_)
        | EventMsg::AuthRecoveryStarted(_)
        | EventMsg::AuthRecoveryCompleted(_)
        | EventMsg::GuardianWarning(_)
        | EventMsg::RealtimeConversationStarted(_)
        | EventMsg::RealtimeConversationSdp(_)
        | EventMsg::RealtimeConversationRealtime(_)
        | EventMsg::RealtimeConversationClosed(_)
        | EventMsg::SafetyBuffering(_)
        | EventMsg::ModelReroute(_)
        | EventMsg::ModelVerification(_)
        | EventMsg::TurnModerationMetadata(_)
        | EventMsg::AgentReasoningSectionBreak(_)
        | EventMsg::RawResponseItem(_)
        | EventMsg::RawResponseCompleted(_)
        | EventMsg::SessionConfigured(_)
        | EventMsg::EnvironmentConnected(_)
        | EventMsg::EnvironmentDisconnected(_)
        | EventMsg::McpToolCallBegin(_)
        | EventMsg::ExecCommandBegin(_)
        | EventMsg::TerminalInteraction(_)
        | EventMsg::ExecCommandOutputDelta(_)
        | EventMsg::ExecApprovalRequest(_)
        | EventMsg::RequestPermissions(_)
        | EventMsg::RequestUserInput(_)
        | EventMsg::ElicitationRequest(_)
        | EventMsg::ApplyPatchApprovalRequest(_)
        | EventMsg::StreamError(_)
        | EventMsg::PatchApplyBegin(_)
        | EventMsg::PatchApplyUpdated(_)
        | EventMsg::TurnDiff(_)
        | EventMsg::RealtimeConversationListVoicesResponse(_)
        | EventMsg::McpStartupUpdate(_)
        | EventMsg::McpStartupComplete(_)
        | EventMsg::WebSearchBegin(_)
        | EventMsg::PlanUpdate(_)
        | EventMsg::ShutdownComplete
        | EventMsg::DeprecationNotice(_)
        | EventMsg::ItemStarted(_)
        | EventMsg::HookStarted(_)
        | EventMsg::HookCompleted(_)
        | EventMsg::AgentMessageContentDelta(_)
        | EventMsg::PlanDelta(_)
        | EventMsg::ReasoningContentDelta(_)
        | EventMsg::ReasoningRawContentDelta(_)
        | EventMsg::ImageGenerationBegin(_)
        | EventMsg::CollabAgentSpawnBegin(_)
        | EventMsg::CollabAgentInteractionBegin(_)
        | EventMsg::CollabWaitingBegin(_)
        | EventMsg::CollabCloseBegin(_)
        | EventMsg::CollabResumeBegin(_) => None,
    }
}

/// Returns the persisted representation of an `ItemCompleted` event.
fn persisted_item_completed_event<'a>(
    ev: &'a EventMsg,
    event: &ItemCompletedEvent,
    history_mode: ThreadHistoryMode,
) -> Option<Cow<'a, EventMsg>> {
    match history_mode {
        ThreadHistoryMode::Legacy => {
            // Legacy rollouts keep only items with no lossless raw ResponseItem or legacy
            // equivalent.
            (matches!(
                event.item,
                TurnItem::FunctionCallOutput(_)
                    | TurnItem::Plan(_)
                    | TurnItem::Extension(ExtensionItem::Sleep(_))
            ) || matches!(
                &event.item,
                TurnItem::SubAgentActivity(item)
                    if item.kind == SubAgentActivityKind::Completed
            ))
            .then_some(Cow::Borrowed(ev))
        }
        ThreadHistoryMode::Paginated => match &event.item {
            TurnItem::CommandExecution(CommandExecutionItem {
                aggregated_output: Some(aggregated_output),
                ..
            }) if aggregated_output.len() > PERSISTED_COMMAND_OUTPUT_MAX_BYTES => {
                let aggregated_output = truncate_middle_with_marker(
                    aggregated_output,
                    PERSISTED_COMMAND_OUTPUT_MAX_BYTES,
                    PERSISTED_COMMAND_OUTPUT_TRUNCATION_MARKER,
                );
                let mut event = event.clone();
                if let TurnItem::CommandExecution(command) = &mut event.item {
                    command.aggregated_output = Some(aggregated_output);
                }
                Some(Cow::Owned(EventMsg::ItemCompleted(event)))
            }
            TurnItem::McpToolCall(McpToolCallItem {
                result: Some(result),
                ..
            }) => {
                let Cow::Owned(result) =
                    truncate_mcp_tool_result(result, PERSISTED_MCP_RESULT_MAX_BYTES)
                else {
                    return Some(Cow::Borrowed(ev));
                };
                let mut event = event.clone();
                if let TurnItem::McpToolCall(tool_call) = &mut event.item {
                    tool_call.result = Some(result);
                }
                Some(Cow::Owned(EventMsg::ItemCompleted(event)))
            }
            _ => Some(Cow::Borrowed(ev)),
        },
    }
}
