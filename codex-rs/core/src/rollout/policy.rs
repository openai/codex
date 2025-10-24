use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;

/// Whether a `ResponseItem` should be persisted in rollout files.
#[inline]
pub(crate) fn is_persisted_response_item(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::CustomToolCallOutput { .. } => true,
        ResponseItem::WebSearchCall { .. } | ResponseItem::Other => false,
    }
}

pub(crate) fn is_persisted_event(event: &Event) -> bool {
    match event.msg {
        EventMsg::ExecApprovalRequest(_)
        | EventMsg::ApplyPatchApprovalRequest(_)
        | EventMsg::AgentReasoningDelta(_)
        | EventMsg::AgentReasoningRawContentDelta(_)
        | EventMsg::ExecCommandOutputDelta(_)
        | EventMsg::GetHistoryEntryResponse(_)
        | EventMsg::AgentMessageDelta(_)
        | EventMsg::TaskStarted(_)
        | EventMsg::TaskComplete(_)
        | EventMsg::McpToolCallBegin(_)
        | EventMsg::McpToolCallEnd(_)
        | EventMsg::WebSearchBegin(_)
        | EventMsg::WebSearchEnd(_)
        | EventMsg::ExecCommandBegin(_)
        | EventMsg::ExecCommandEnd(_)
        | EventMsg::PatchApplyBegin(_)
        | EventMsg::PatchApplyEnd(_)
        | EventMsg::TurnDiff(_)
        | EventMsg::BackgroundEvent(_)
        | EventMsg::McpListToolsResponse(_)
        | EventMsg::ListCustomPromptsResponse(_)
        | EventMsg::ShutdownComplete
        | EventMsg::ConversationHistory(_)
        | EventMsg::PlanUpdate(_)
        | EventMsg::TurnAborted(_)
        | EventMsg::StreamError(_)
        | EventMsg::Error(_)
        | EventMsg::AgentReasoningSectionBreak(_)
        | EventMsg::SessionConfigured(_) => false,
        EventMsg::UserMessage(_)
        | EventMsg::AgentMessage(_)
        | EventMsg::AgentReasoning(_)
        | EventMsg::AgentReasoningRawContent(_)
        | EventMsg::TokenCount(_) => true,
    }
}
