use std::time::Duration;

use codex_analytics::GuardianReviewSessionKind;
use codex_analytics::GuardianToolCallCounts;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandSource;
use codex_protocol::protocol::TokenUsage;

#[derive(Debug, Clone)]
pub(crate) struct GuardianReviewSessionReport {
    pub(crate) guardian_thread_id: String,
    pub(crate) session_kind: GuardianReviewSessionKind,
    pub(crate) guardian_model: Option<String>,
    pub(crate) guardian_reasoning_effort: Option<String>,
    pub(crate) had_prior_review_context: bool,
    pub(crate) tool_call_counts: GuardianToolCallCounts,
    pub(crate) time_to_first_token_ms: Option<u64>,
    pub(crate) token_usage: Option<TokenUsage>,
}

pub(super) fn duration_millis_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

pub(super) fn guardian_event_records_time_to_first_token(event: &EventMsg) -> bool {
    matches!(
        event,
        EventMsg::AgentMessage(_)
            | EventMsg::AgentMessageDelta(_)
            | EventMsg::AgentReasoning(_)
            | EventMsg::AgentReasoningDelta(_)
            | EventMsg::AgentReasoningRawContent(_)
            | EventMsg::AgentReasoningRawContentDelta(_)
            | EventMsg::McpToolCallBegin(_)
            | EventMsg::ExecCommandBegin(_)
            | EventMsg::DynamicToolCallRequest(_)
            | EventMsg::PatchApplyBegin(_)
            | EventMsg::WebSearchBegin(_)
            | EventMsg::ImageGenerationBegin(_)
            | EventMsg::ViewImageToolCall(_)
    )
}

pub(super) fn record_guardian_tool_call_count(
    event: &EventMsg,
    counts: &mut GuardianToolCallCounts,
) {
    match event {
        EventMsg::ExecCommandBegin(begin) => match begin.source {
            ExecCommandSource::Agent | ExecCommandSource::UserShell => {
                counts.shell += 1;
            }
            ExecCommandSource::UnifiedExecStartup | ExecCommandSource::UnifiedExecInteraction => {
                counts.unified_exec += 1;
            }
        },
        EventMsg::McpToolCallBegin(_) => {
            counts.mcp += 1;
        }
        EventMsg::DynamicToolCallRequest(_) => {
            counts.dynamic += 1;
        }
        EventMsg::PatchApplyBegin(_) => {
            counts.apply_patch += 1;
        }
        EventMsg::WebSearchBegin(_) => {
            counts.web_search += 1;
        }
        EventMsg::ImageGenerationBegin(_) => {
            counts.image_generation += 1;
        }
        EventMsg::ViewImageToolCall(_) => {
            counts.view_image += 1;
        }
        EventMsg::Error(_)
        | EventMsg::Warning(_)
        | EventMsg::RealtimeConversationStarted(_)
        | EventMsg::RealtimeConversationRealtime(_)
        | EventMsg::RealtimeConversationClosed(_)
        | EventMsg::ModelReroute(_)
        | EventMsg::ContextCompacted(_)
        | EventMsg::ThreadRolledBack(_)
        | EventMsg::TurnStarted(_)
        | EventMsg::TurnComplete(_)
        | EventMsg::TokenCount(_)
        | EventMsg::AgentMessage(_)
        | EventMsg::UserMessage(_)
        | EventMsg::AgentMessageDelta(_)
        | EventMsg::AgentReasoning(_)
        | EventMsg::AgentReasoningDelta(_)
        | EventMsg::AgentReasoningRawContent(_)
        | EventMsg::AgentReasoningRawContentDelta(_)
        | EventMsg::AgentReasoningSectionBreak(_)
        | EventMsg::SessionConfigured(_)
        | EventMsg::ThreadNameUpdated(_)
        | EventMsg::McpStartupUpdate(_)
        | EventMsg::McpStartupComplete(_)
        | EventMsg::McpToolCallEnd(_)
        | EventMsg::WebSearchEnd(_)
        | EventMsg::ImageGenerationEnd(_)
        | EventMsg::ExecCommandOutputDelta(_)
        | EventMsg::TerminalInteraction(_)
        | EventMsg::ExecCommandEnd(_)
        | EventMsg::ExecApprovalRequest(_)
        | EventMsg::RequestPermissions(_)
        | EventMsg::RequestUserInput(_)
        | EventMsg::DynamicToolCallResponse(_)
        | EventMsg::ElicitationRequest(_)
        | EventMsg::ApplyPatchApprovalRequest(_)
        | EventMsg::GuardianAssessment(_)
        | EventMsg::DeprecationNotice(_)
        | EventMsg::BackgroundEvent(_)
        | EventMsg::UndoStarted(_)
        | EventMsg::UndoCompleted(_)
        | EventMsg::StreamError(_)
        | EventMsg::PatchApplyEnd(_)
        | EventMsg::TurnDiff(_)
        | EventMsg::GetHistoryEntryResponse(_)
        | EventMsg::McpListToolsResponse(_)
        | EventMsg::ListSkillsResponse(_)
        | EventMsg::SkillsUpdateAvailable
        | EventMsg::PlanUpdate(_)
        | EventMsg::TurnAborted(_)
        | EventMsg::ShutdownComplete
        | EventMsg::EnteredReviewMode(_)
        | EventMsg::ExitedReviewMode(_)
        | EventMsg::RawResponseItem(_)
        | EventMsg::ItemStarted(_)
        | EventMsg::ItemCompleted(_)
        | EventMsg::HookStarted(_)
        | EventMsg::HookCompleted(_)
        | EventMsg::AgentMessageContentDelta(_)
        | EventMsg::PlanDelta(_)
        | EventMsg::ReasoningContentDelta(_)
        | EventMsg::ReasoningRawContentDelta(_)
        | EventMsg::CollabAgentSpawnBegin(_)
        | EventMsg::CollabAgentSpawnEnd(_)
        | EventMsg::CollabAgentInteractionBegin(_)
        | EventMsg::CollabAgentInteractionEnd(_)
        | EventMsg::CollabWaitingBegin(_)
        | EventMsg::CollabWaitingEnd(_)
        | EventMsg::CollabCloseBegin(_)
        | EventMsg::CollabCloseEnd(_)
        | EventMsg::CollabResumeBegin(_)
        | EventMsg::CollabResumeEnd(_) => {}
    }
}
