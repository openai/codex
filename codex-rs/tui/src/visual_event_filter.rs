use codex_core::protocol::EventMsg;

/// Returns true when a protocol event should be forwarded into the visual transcript.
pub(crate) fn should_forward_visual_event(msg: &EventMsg) -> bool {
    matches!(
        msg,
        EventMsg::AgentReasoning(_)
            | EventMsg::AgentReasoningDelta(_)
            | EventMsg::AgentReasoningRawContent(_)
            | EventMsg::AgentReasoningRawContentDelta(_)
            | EventMsg::AgentReasoningSectionBreak(_)
            | EventMsg::ReasoningContentDelta(_)
            | EventMsg::ReasoningRawContentDelta(_)
            | EventMsg::TurnStarted(_)
            | EventMsg::TokenCount(_)
            | EventMsg::Warning(_)
            | EventMsg::Error(_)
            | EventMsg::PlanUpdate(_)
            | EventMsg::ExecCommandBegin(_)
            | EventMsg::ExecCommandOutputDelta(_)
            | EventMsg::TerminalInteraction(_)
            | EventMsg::ExecCommandEnd(_)
            | EventMsg::PatchApplyBegin(_)
            | EventMsg::PatchApplyEnd(_)
            | EventMsg::ViewImageToolCall(_)
            | EventMsg::McpToolCallBegin(_)
            | EventMsg::McpToolCallEnd(_)
            | EventMsg::WebSearchBegin(_)
            | EventMsg::WebSearchEnd(_)
            | EventMsg::BackgroundEvent(_)
            | EventMsg::StreamError(_)
            | EventMsg::TurnDiff(_)
            | EventMsg::ContextCompacted(_)
    )
}
