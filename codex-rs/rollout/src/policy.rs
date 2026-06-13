use crate::protocol::EventMsg;
use crate::protocol::RolloutItem;
use codex_protocol::models::ResponseItem;
use codex_utils_string::truncate_middle_chars;

const PERSISTED_EXEC_AGGREGATED_OUTPUT_MAX_BYTES: usize = 10_000;

fn truncate_persisted_exec_output(output: &str) -> String {
    let mut retained_bytes = PERSISTED_EXEC_AGGREGATED_OUTPUT_MAX_BYTES;
    loop {
        let truncated = truncate_middle_chars(output, retained_bytes);
        if truncated.len() <= PERSISTED_EXEC_AGGREGATED_OUTPUT_MAX_BYTES {
            return truncated;
        }
        retained_bytes = retained_bytes
            .saturating_sub(truncated.len() - PERSISTED_EXEC_AGGREGATED_OUTPUT_MAX_BYTES);
    }
}

/// Whether a rollout `item` should be persisted in rollout files.
pub fn is_persisted_rollout_item(item: &RolloutItem) -> bool {
    match item {
        RolloutItem::ResponseItem(item) => should_persist_response_item(item),
        RolloutItem::InterAgentCommunication(_) => true,
        RolloutItem::EventMsg(ev) => should_persist_event_msg(ev),
        // Persist Codex executive markers so we can analyze flows (e.g., compaction, API turns).
        RolloutItem::Compacted(_) | RolloutItem::TurnContext(_) | RolloutItem::SessionMeta(_) => {
            true
        }
    }
}

/// Return the canonical rollout items that should be persisted for a live append.
pub fn persisted_rollout_items(items: &[RolloutItem]) -> Vec<RolloutItem> {
    let mut persisted = Vec::new();
    for item in items {
        if is_persisted_rollout_item(item) {
            persisted.push(sanitize_rollout_item_for_persistence(item.clone()));
        }
    }
    persisted
}

fn sanitize_rollout_item_for_persistence(item: RolloutItem) -> RolloutItem {
    match item {
        RolloutItem::EventMsg(EventMsg::ExecCommandEnd(mut event)) => {
            // Rebuilt app-server history needs the aggregate, while the per-stream and
            // model-formatted copies would only duplicate potentially large output.
            event.aggregated_output = truncate_persisted_exec_output(&event.aggregated_output);
            event.stdout.clear();
            event.stderr.clear();
            event.formatted_output.clear();
            RolloutItem::EventMsg(EventMsg::ExecCommandEnd(event))
        }
        _ => item,
    }
}

/// Whether a `ResponseItem` should be persisted in rollout files.
#[inline]
pub fn should_persist_response_item(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { .. }
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
        | ResponseItem::Compaction { .. }
        | ResponseItem::ContextCompaction { .. } => true,
        ResponseItem::CompactionTrigger => false,
        ResponseItem::Other => false,
    }
}

/// Whether a `ResponseItem` should be persisted for the memories.
#[inline]
pub fn should_persist_response_item_for_memories(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { role, .. } => role != "developer",
        ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::ToolSearchOutput { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::WebSearchCall { .. } => true,
        ResponseItem::AgentMessage { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::CompactionTrigger
        | ResponseItem::ContextCompaction { .. }
        | ResponseItem::Other => false,
    }
}

/// Whether an `EventMsg` should be persisted in rollout files.
#[inline]
pub fn should_persist_event_msg(ev: &EventMsg) -> bool {
    match ev {
        EventMsg::UserMessage(_)
        | EventMsg::AgentMessage(_)
        | EventMsg::AgentReasoning(_)
        | EventMsg::AgentReasoningRawContent(_)
        | EventMsg::PatchApplyEnd(_)
        | EventMsg::TokenCount(_)
        | EventMsg::ThreadGoalUpdated(_)
        | EventMsg::ContextCompacted(_)
        | EventMsg::EnteredReviewMode(_)
        | EventMsg::ExitedReviewMode(_)
        | EventMsg::McpToolCallEnd(_)
        | EventMsg::ThreadRolledBack(_)
        | EventMsg::TurnAborted(_)
        | EventMsg::TurnStarted(_)
        | EventMsg::TurnComplete(_)
        | EventMsg::WebSearchEnd(_)
        | EventMsg::ImageGenerationEnd(_)
        | EventMsg::ExecCommandEnd(_)
        | EventMsg::SubAgentActivity(_) => true,
        EventMsg::ItemCompleted(event) => {
            // Plan items are derived from streaming tags and are not part of the
            // raw ResponseItem history, so we persist their completion to replay
            // them on resume without bloating rollouts with every item lifecycle.
            matches!(event.item, codex_protocol::items::TurnItem::Plan(_))
        }
        EventMsg::Error(_)
        | EventMsg::GuardianAssessment(_)
        | EventMsg::ViewImageToolCall(_)
        | EventMsg::CollabAgentSpawnEnd(_)
        | EventMsg::CollabAgentInteractionEnd(_)
        | EventMsg::CollabWaitingEnd(_)
        | EventMsg::CollabCloseEnd(_)
        | EventMsg::CollabResumeEnd(_)
        | EventMsg::DynamicToolCallRequest(_)
        | EventMsg::DynamicToolCallResponse(_)
        | EventMsg::Warning(_)
        | EventMsg::GuardianWarning(_)
        | EventMsg::RealtimeConversationStarted(_)
        | EventMsg::RealtimeConversationSdp(_)
        | EventMsg::RealtimeConversationRealtime(_)
        | EventMsg::RealtimeConversationClosed(_)
        | EventMsg::ModelReroute(_)
        | EventMsg::ModelVerification(_)
        | EventMsg::TurnModerationMetadata(_)
        | EventMsg::AgentReasoningSectionBreak(_)
        | EventMsg::RawResponseItem(_)
        | EventMsg::SessionConfigured(_)
        | EventMsg::ThreadSettingsApplied(_)
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
        | EventMsg::CollabResumeBegin(_) => false,
    }
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
