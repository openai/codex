use crate::protocol::EventMsg;
use crate::protocol::RolloutItem;
use codex_protocol::models::ResponseItem;

/// Whether a rollout `item` should be persisted in rollout files.
pub fn is_persisted_rollout_item(item: &RolloutItem) -> bool {
    match item {
        RolloutItem::ResponseItem(item) => should_persist_response_item(item),
        RolloutItem::InterAgentCommunication(_)
        | RolloutItem::InterAgentCommunicationMetadata { .. } => true,
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
            persisted.push(item.clone());
        }
    }
    persisted
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
        ResponseItem::AdditionalTools { .. }
        | ResponseItem::CompactionTrigger { .. }
        | ResponseItem::Other => false,
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
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::CompactionTrigger { .. }
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
        | EventMsg::SubAgentActivity(_) => true,
        EventMsg::ItemCompleted(event) => {
            // These items have no equivalent raw ResponseItem or legacy event,
            // so persist their completion for replay without retaining every
            // item lifecycle event.
            matches!(
                event.item,
                codex_protocol::items::TurnItem::Plan(_)
                    | codex_protocol::items::TurnItem::Sleep(_)
            )
        }
        EventMsg::Error(error) => error.affects_turn_status(),
        EventMsg::GuardianAssessment(event) => match event.status {
            codex_protocol::protocol::GuardianAssessmentStatus::InProgress => false,
            codex_protocol::protocol::GuardianAssessmentStatus::Approved
            | codex_protocol::protocol::GuardianAssessmentStatus::Denied
            | codex_protocol::protocol::GuardianAssessmentStatus::TimedOut
            | codex_protocol::protocol::GuardianAssessmentStatus::Aborted => true,
        },
        EventMsg::HookCompleted(_) => true,
        EventMsg::ExecCommandEnd(_)
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
        | EventMsg::SafetyBuffering(_)
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
mod tests {
    use super::*;
    use codex_protocol::protocol::CodexErrorInfo;
    use codex_protocol::protocol::ErrorEvent;
    use codex_protocol::protocol::GuardianAssessmentAction;
    use codex_protocol::protocol::GuardianAssessmentEvent;
    use codex_protocol::protocol::GuardianAssessmentStatus;
    use codex_protocol::protocol::HookCompletedEvent;
    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookExecutionMode;
    use codex_protocol::protocol::HookHandlerType;
    use codex_protocol::protocol::HookRunStatus;
    use codex_protocol::protocol::HookRunSummary;
    use codex_protocol::protocol::HookScope;
    use codex_protocol::protocol::HookSource;
    use codex_protocol::protocol::HookStartedEvent;
    use codex_protocol::protocol::NetworkApprovalProtocol;
    use pretty_assertions::assert_eq;

    #[test]
    fn persists_only_errors_that_define_terminal_turn_state() {
        let terminal_error = EventMsg::Error(ErrorEvent {
            message: "simulated failure".to_string(),
            codex_error_info: Some(CodexErrorInfo::InternalServerError),
        });
        let request_error = EventMsg::Error(ErrorEvent {
            message: "rollback failed".to_string(),
            codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
        });

        assert!(should_persist_event_msg(&terminal_error));
        assert!(!should_persist_event_msg(&request_error));
    }

    #[test]
    fn persists_only_terminal_guardian_assessments() {
        let mut assessment = GuardianAssessmentEvent {
            id: "review-1".to_string(),
            target_item_id: None,
            turn_id: "turn-1".to_string(),
            started_at_ms: 1,
            completed_at_ms: None,
            status: GuardianAssessmentStatus::InProgress,
            risk_level: None,
            user_authorization: None,
            rationale: None,
            decision_source: None,
            action: GuardianAssessmentAction::NetworkAccess {
                target: "https://example.com".to_string(),
                host: "example.com".to_string(),
                protocol: NetworkApprovalProtocol::Https,
                port: 443,
            },
        };

        for (status, expected) in [
            (GuardianAssessmentStatus::InProgress, false),
            (GuardianAssessmentStatus::Approved, true),
            (GuardianAssessmentStatus::Denied, true),
            (GuardianAssessmentStatus::TimedOut, true),
            (GuardianAssessmentStatus::Aborted, true),
        ] {
            assessment.status = status;
            assert_eq!(
                should_persist_event_msg(&EventMsg::GuardianAssessment(assessment.clone())),
                expected,
            );
        }
    }

    #[test]
    fn persists_completed_hook_events_but_not_started_events() {
        let run = HookRunSummary {
            id: "hook-1".to_string(),
            event_name: HookEventName::PostToolUse,
            handler_type: HookHandlerType::Command,
            execution_mode: HookExecutionMode::Sync,
            scope: HookScope::Turn,
            source_path: std::env::current_dir()
                .expect("current directory should be available")
                .try_into()
                .expect("current directory should be absolute"),
            source: HookSource::Project,
            display_order: 0,
            status: HookRunStatus::Completed,
            status_message: None,
            started_at: 1,
            completed_at: Some(2),
            duration_ms: Some(1_000),
            entries: Vec::new(),
        };

        assert!(!should_persist_event_msg(&EventMsg::HookStarted(
            HookStartedEvent {
                turn_id: Some("turn-1".to_string()),
                run: run.clone(),
            },
        )));
        assert!(should_persist_event_msg(&EventMsg::HookCompleted(
            HookCompletedEvent {
                turn_id: Some("turn-1".to_string()),
                run,
            },
        )));
    }
}
