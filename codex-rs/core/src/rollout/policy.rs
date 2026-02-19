use crate::protocol::EventMsg;
use crate::protocol::RolloutItem;
use codex_protocol::models::ResponseItem;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EventPersistenceMode {
    #[default]
    Limited,
    Extended,
}

/// Whether a rollout `item` should be persisted in rollout files for the
/// provided persistence `mode`.
#[inline]
pub(crate) fn is_persisted_response_item(item: &RolloutItem, mode: EventPersistenceMode) -> bool {
    match item {
        RolloutItem::ResponseItem(item) => should_persist_response_item(item),
        RolloutItem::EventMsg(ev) => should_persist_event_msg(ev, mode),
        // Persist Codex executive markers so we can analyze flows (e.g., compaction, API turns).
        RolloutItem::Compacted(_) | RolloutItem::TurnContext(_) | RolloutItem::SessionMeta(_) => {
            true
        }
    }
}

/// Whether a `ResponseItem` should be persisted in rollout files.
#[inline]
pub(crate) fn should_persist_response_item(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::GhostSnapshot { .. }
        | ResponseItem::Compaction { .. } => true,
        ResponseItem::Other => false,
    }
}

/// Whether a `ResponseItem` should be persisted for the memories.
#[inline]
pub(crate) fn should_persist_response_item_for_memories(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { role, .. } => role != "developer",
        ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::WebSearchCall { .. } => true,
        ResponseItem::Reasoning { .. }
        | ResponseItem::GhostSnapshot { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::Other => false,
    }
}

/// Whether an `EventMsg` should be persisted in rollout files for the
/// provided persistence `mode`.
#[inline]
pub(crate) fn should_persist_event_msg(ev: &EventMsg, mode: EventPersistenceMode) -> bool {
    match mode {
        EventPersistenceMode::Limited => should_persist_event_msg_limited(ev),
        EventPersistenceMode::Extended => should_persist_event_msg_extended(ev),
    }
}

fn should_persist_event_msg_limited(ev: &EventMsg) -> bool {
    matches!(
        event_msg_persistence_mode(ev),
        Some(EventPersistenceMode::Limited)
    )
}

fn should_persist_event_msg_extended(ev: &EventMsg) -> bool {
    matches!(
        event_msg_persistence_mode(ev),
        Some(EventPersistenceMode::Limited) | Some(EventPersistenceMode::Extended)
    )
}

/// Returns the minimum persistence mode that includes this event.
/// `None` means the event should never be persisted.
fn event_msg_persistence_mode(ev: &EventMsg) -> Option<EventPersistenceMode> {
    match ev {
        EventMsg::UserMessage(_)
        | EventMsg::AgentMessage(_)
        | EventMsg::AgentReasoning(_)
        | EventMsg::AgentReasoningRawContent(_)
        | EventMsg::TokenCount(_)
        | EventMsg::ContextCompacted(_)
        | EventMsg::EnteredReviewMode(_)
        | EventMsg::ExitedReviewMode(_)
        | EventMsg::ThreadRolledBack(_)
        | EventMsg::UndoCompleted(_)
        | EventMsg::TurnAborted(_)
        | EventMsg::TurnStarted(_)
        | EventMsg::TurnComplete(_) => Some(EventPersistenceMode::Limited),
        EventMsg::ItemCompleted(event) => {
            // Plan items are derived from streaming tags and are not part of the
            // raw ResponseItem history, so we persist their completion to replay
            // them on resume without bloating rollouts with every item lifecycle.
            if matches!(event.item, codex_protocol::items::TurnItem::Plan(_)) {
                Some(EventPersistenceMode::Limited)
            } else {
                None
            }
        }
        EventMsg::Error(_)
        | EventMsg::WebSearchEnd(_)
        | EventMsg::ExecCommandEnd(_)
        | EventMsg::PatchApplyEnd(_)
        | EventMsg::McpToolCallEnd(_)
        | EventMsg::ViewImageToolCall(_)
        | EventMsg::CollabAgentSpawnEnd(_)
        | EventMsg::CollabAgentInteractionEnd(_)
        | EventMsg::CollabWaitingEnd(_)
        | EventMsg::CollabCloseEnd(_)
        | EventMsg::CollabResumeEnd(_) => Some(EventPersistenceMode::Extended),
        EventMsg::Warning(_)
        | EventMsg::ModelReroute(_)
        | EventMsg::AgentMessageDelta(_)
        | EventMsg::AgentReasoningDelta(_)
        | EventMsg::AgentReasoningRawContentDelta(_)
        | EventMsg::AgentReasoningSectionBreak(_)
        | EventMsg::RawResponseItem(_)
        | EventMsg::SessionConfigured(_)
        | EventMsg::ThreadNameUpdated(_)
        | EventMsg::McpToolCallBegin(_)
        | EventMsg::WebSearchBegin(_)
        | EventMsg::ExecCommandBegin(_)
        | EventMsg::TerminalInteraction(_)
        | EventMsg::ExecCommandOutputDelta(_)
        | EventMsg::ExecApprovalRequest(_)
        | EventMsg::RequestUserInput(_)
        | EventMsg::DynamicToolCallRequest(_)
        | EventMsg::ElicitationRequest(_)
        | EventMsg::ApplyPatchApprovalRequest(_)
        | EventMsg::BackgroundEvent(_)
        | EventMsg::StreamError(_)
        | EventMsg::PatchApplyBegin(_)
        | EventMsg::TurnDiff(_)
        | EventMsg::GetHistoryEntryResponse(_)
        | EventMsg::UndoStarted(_)
        | EventMsg::McpListToolsResponse(_)
        | EventMsg::McpStartupUpdate(_)
        | EventMsg::McpStartupComplete(_)
        | EventMsg::ListCustomPromptsResponse(_)
        | EventMsg::ListSkillsResponse(_)
        | EventMsg::ListRemoteSkillsResponse(_)
        | EventMsg::RemoteSkillDownloaded(_)
        | EventMsg::PlanUpdate(_)
        | EventMsg::ShutdownComplete
        | EventMsg::DeprecationNotice(_)
        | EventMsg::ItemStarted(_)
        | EventMsg::AgentMessageContentDelta(_)
        | EventMsg::PlanDelta(_)
        | EventMsg::ReasoningContentDelta(_)
        | EventMsg::ReasoningRawContentDelta(_)
        | EventMsg::SkillsUpdateAvailable
        | EventMsg::CollabAgentSpawnBegin(_)
        | EventMsg::CollabAgentInteractionBegin(_)
        | EventMsg::CollabWaitingBegin(_)
        | EventMsg::CollabCloseBegin(_)
        | EventMsg::CollabResumeBegin(_) => None,
    }
}
