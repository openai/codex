//! Mapping from Codex protocol events into raw rollout-trace events.
//!
//! The session layer already emits protocol events for turn lifecycle, terminal
//! sessions, patch application, MCP calls, and collaboration tools. Rollout
//! tracing reuses those observations instead of adding another set of hooks in
//! `codex-core`: this module translates the protocol surface into the smaller
//! trace vocabulary and keeps the mapping isolated inside `codex-rollout-trace`.
//!
//! The long explicit `EventMsg` matches are intentional. Most protocol events
//! are not trace runtime boundaries, but spelling them out makes new protocol
//! variants a compile-time prompt to decide whether the trace should capture
//! them.

use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::ExecCommandSource;
use codex_protocol::protocol::ExecCommandStatus;
use codex_protocol::protocol::McpToolCallBeginEvent;
use codex_protocol::protocol::McpToolCallEndEvent;
use codex_protocol::protocol::PatchApplyBeginEvent;
use codex_protocol::protocol::PatchApplyEndEvent;
use codex_protocol::protocol::PatchApplyStatus;
use codex_protocol::protocol::TurnAbortReason;
use serde::Serialize;

use crate::AgentThreadId;
use crate::CodexTurnId;
use crate::ExecutionStatus;
use crate::RawTraceEventPayload;

pub(crate) struct CodexTurnTraceEvent {
    pub context_turn_id: CodexTurnId,
    pub payload: RawTraceEventPayload,
}

pub(crate) fn codex_turn_trace_event(
    thread_id: AgentThreadId,
    default_turn_id: &str,
    event: &EventMsg,
) -> Option<CodexTurnTraceEvent> {
    match event {
        EventMsg::TurnStarted(event) => {
            let codex_turn_id = event.turn_id.clone();
            Some(CodexTurnTraceEvent {
                context_turn_id: codex_turn_id.clone(),
                payload: RawTraceEventPayload::CodexTurnStarted {
                    codex_turn_id,
                    thread_id,
                },
            })
        }
        EventMsg::TurnComplete(event) => {
            let codex_turn_id = event.turn_id.clone();
            Some(CodexTurnTraceEvent {
                context_turn_id: codex_turn_id.clone(),
                payload: RawTraceEventPayload::CodexTurnEnded {
                    codex_turn_id,
                    status: ExecutionStatus::Completed,
                },
            })
        }
        EventMsg::TurnAborted(event) => {
            let codex_turn_id = event
                .turn_id
                .clone()
                .unwrap_or_else(|| default_turn_id.to_string());
            Some(CodexTurnTraceEvent {
                context_turn_id: codex_turn_id.clone(),
                payload: RawTraceEventPayload::CodexTurnEnded {
                    codex_turn_id,
                    status: execution_status_for_abort_reason(&event.reason),
                },
            })
        }
        _ => None,
    }
}

pub(crate) enum ToolRuntimeTraceEvent<'a> {
    Started {
        tool_call_id: &'a str,
        payload: ToolRuntimePayload<'a>,
    },
    Ended {
        tool_call_id: &'a str,
        status: ExecutionStatus,
        payload: ToolRuntimePayload<'a>,
    },
}

/// Borrowed protocol payload that should be persisted as tool runtime data.
///
/// The trace wants the exact protocol payload shape for E2E debugging, while
/// reducers consume the surrounding typed trace events. This enum lets the
/// recorder serialize the original event by reference, without first cloning it
/// or converting it through `serde_json::Value`.
pub(crate) enum ToolRuntimePayload<'a> {
    ExecCommandBegin(&'a ExecCommandBeginEvent),
    ExecCommandEnd(&'a ExecCommandEndEvent),
    PatchApplyBegin(&'a PatchApplyBeginEvent),
    PatchApplyEnd(&'a PatchApplyEndEvent),
    McpToolCallBegin(&'a McpToolCallBeginEvent),
    McpToolCallEnd(&'a McpToolCallEndEvent),
    CollabAgentSpawnBegin(&'a codex_protocol::protocol::CollabAgentSpawnBeginEvent),
    CollabAgentSpawnEnd(&'a codex_protocol::protocol::CollabAgentSpawnEndEvent),
    CollabAgentInteractionBegin(&'a codex_protocol::protocol::CollabAgentInteractionBeginEvent),
    CollabAgentInteractionEnd(&'a codex_protocol::protocol::CollabAgentInteractionEndEvent),
    CollabWaitingBegin(&'a codex_protocol::protocol::CollabWaitingBeginEvent),
    CollabWaitingEnd(&'a codex_protocol::protocol::CollabWaitingEndEvent),
    CollabCloseBegin(&'a codex_protocol::protocol::CollabCloseBeginEvent),
    CollabCloseEnd(&'a codex_protocol::protocol::CollabCloseEndEvent),
}

impl Serialize for ToolRuntimePayload<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ToolRuntimePayload::ExecCommandBegin(event) => event.serialize(serializer),
            ToolRuntimePayload::ExecCommandEnd(event) => event.serialize(serializer),
            ToolRuntimePayload::PatchApplyBegin(event) => event.serialize(serializer),
            ToolRuntimePayload::PatchApplyEnd(event) => event.serialize(serializer),
            ToolRuntimePayload::McpToolCallBegin(event) => event.serialize(serializer),
            ToolRuntimePayload::McpToolCallEnd(event) => event.serialize(serializer),
            ToolRuntimePayload::CollabAgentSpawnBegin(event) => event.serialize(serializer),
            ToolRuntimePayload::CollabAgentSpawnEnd(event) => event.serialize(serializer),
            ToolRuntimePayload::CollabAgentInteractionBegin(event) => event.serialize(serializer),
            ToolRuntimePayload::CollabAgentInteractionEnd(event) => event.serialize(serializer),
            ToolRuntimePayload::CollabWaitingBegin(event) => event.serialize(serializer),
            ToolRuntimePayload::CollabWaitingEnd(event) => event.serialize(serializer),
            ToolRuntimePayload::CollabCloseBegin(event) => event.serialize(serializer),
            ToolRuntimePayload::CollabCloseEnd(event) => event.serialize(serializer),
        }
    }
}

pub(crate) fn tool_runtime_trace_event(event: &EventMsg) -> Option<ToolRuntimeTraceEvent<'_>> {
    match event {
        EventMsg::ExecCommandBegin(event) if event.source != ExecCommandSource::UserShell => {
            Some(ToolRuntimeTraceEvent::Started {
                tool_call_id: &event.call_id,
                payload: ToolRuntimePayload::ExecCommandBegin(event),
            })
        }
        EventMsg::ExecCommandEnd(event) if event.source != ExecCommandSource::UserShell => {
            Some(ToolRuntimeTraceEvent::Ended {
                tool_call_id: &event.call_id,
                status: event.status.trace_execution_status(),
                payload: ToolRuntimePayload::ExecCommandEnd(event),
            })
        }
        EventMsg::PatchApplyBegin(event) => Some(ToolRuntimeTraceEvent::Started {
            tool_call_id: &event.call_id,
            payload: ToolRuntimePayload::PatchApplyBegin(event),
        }),
        EventMsg::PatchApplyEnd(event) => Some(ToolRuntimeTraceEvent::Ended {
            tool_call_id: &event.call_id,
            status: event.status.trace_execution_status(),
            payload: ToolRuntimePayload::PatchApplyEnd(event),
        }),
        EventMsg::McpToolCallBegin(event) => Some(ToolRuntimeTraceEvent::Started {
            tool_call_id: &event.call_id,
            payload: ToolRuntimePayload::McpToolCallBegin(event),
        }),
        EventMsg::McpToolCallEnd(event) => Some(ToolRuntimeTraceEvent::Ended {
            tool_call_id: &event.call_id,
            status: if event.result.is_ok() {
                ExecutionStatus::Completed
            } else {
                ExecutionStatus::Failed
            },
            payload: ToolRuntimePayload::McpToolCallEnd(event),
        }),
        EventMsg::CollabAgentSpawnBegin(event) => Some(ToolRuntimeTraceEvent::Started {
            tool_call_id: &event.call_id,
            payload: ToolRuntimePayload::CollabAgentSpawnBegin(event),
        }),
        EventMsg::CollabAgentSpawnEnd(event) => Some(ToolRuntimeTraceEvent::Ended {
            tool_call_id: &event.call_id,
            // A spawn end without a child thread id means the runtime boundary
            // finished without creating the requested child thread.
            status: if event.new_thread_id.is_some() {
                ExecutionStatus::Completed
            } else {
                ExecutionStatus::Failed
            },
            payload: ToolRuntimePayload::CollabAgentSpawnEnd(event),
        }),
        EventMsg::CollabAgentInteractionBegin(event) => Some(ToolRuntimeTraceEvent::Started {
            tool_call_id: &event.call_id,
            payload: ToolRuntimePayload::CollabAgentInteractionBegin(event),
        }),
        EventMsg::CollabAgentInteractionEnd(event) => Some(ToolRuntimeTraceEvent::Ended {
            tool_call_id: &event.call_id,
            status: ExecutionStatus::Completed,
            payload: ToolRuntimePayload::CollabAgentInteractionEnd(event),
        }),
        EventMsg::CollabWaitingBegin(event) => Some(ToolRuntimeTraceEvent::Started {
            tool_call_id: &event.call_id,
            payload: ToolRuntimePayload::CollabWaitingBegin(event),
        }),
        EventMsg::CollabWaitingEnd(event) => Some(ToolRuntimeTraceEvent::Ended {
            tool_call_id: &event.call_id,
            status: ExecutionStatus::Completed,
            payload: ToolRuntimePayload::CollabWaitingEnd(event),
        }),
        EventMsg::CollabCloseBegin(event) => Some(ToolRuntimeTraceEvent::Started {
            tool_call_id: &event.call_id,
            payload: ToolRuntimePayload::CollabCloseBegin(event),
        }),
        EventMsg::CollabCloseEnd(event) => Some(ToolRuntimeTraceEvent::Ended {
            tool_call_id: &event.call_id,
            status: ExecutionStatus::Completed,
            payload: ToolRuntimePayload::CollabCloseEnd(event),
        }),
        EventMsg::Error(_)
        | EventMsg::Warning(_)
        | EventMsg::GuardianWarning(_)
        | EventMsg::RealtimeConversationStarted(_)
        | EventMsg::RealtimeConversationRealtime(_)
        | EventMsg::RealtimeConversationClosed(_)
        | EventMsg::RealtimeConversationSdp(_)
        | EventMsg::ModelReroute(_)
        | EventMsg::ModelVerification(_)
        | EventMsg::ContextCompacted(_)
        | EventMsg::ThreadRolledBack(_)
        | EventMsg::ThreadGoalUpdated(_)
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
        | EventMsg::WebSearchBegin(_)
        | EventMsg::WebSearchEnd(_)
        | EventMsg::ImageGenerationBegin(_)
        | EventMsg::ImageGenerationEnd(_)
        | EventMsg::ExecCommandBegin(_)
        | EventMsg::ExecCommandOutputDelta(_)
        | EventMsg::TerminalInteraction(_)
        | EventMsg::ExecCommandEnd(_)
        | EventMsg::ViewImageToolCall(_)
        | EventMsg::ExecApprovalRequest(_)
        | EventMsg::RequestPermissions(_)
        | EventMsg::RequestUserInput(_)
        | EventMsg::DynamicToolCallRequest(_)
        | EventMsg::DynamicToolCallResponse(_)
        | EventMsg::ElicitationRequest(_)
        | EventMsg::ApplyPatchApprovalRequest(_)
        | EventMsg::GuardianAssessment(_)
        | EventMsg::DeprecationNotice(_)
        | EventMsg::BackgroundEvent(_)
        | EventMsg::UndoStarted(_)
        | EventMsg::UndoCompleted(_)
        | EventMsg::StreamError(_)
        | EventMsg::PatchApplyUpdated(_)
        | EventMsg::TurnDiff(_)
        | EventMsg::GetHistoryEntryResponse(_)
        | EventMsg::McpListToolsResponse(_)
        | EventMsg::ListSkillsResponse(_)
        | EventMsg::RealtimeConversationListVoicesResponse(_)
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
        | EventMsg::CollabResumeBegin(_)
        | EventMsg::CollabResumeEnd(_) => None,
    }
}

pub(crate) fn wrapped_protocol_event_type(event: &EventMsg) -> Option<&'static str> {
    match event {
        EventMsg::SessionConfigured(_) => Some("session_configured"),
        EventMsg::TurnStarted(_) => Some("turn_started"),
        EventMsg::TurnComplete(_) => Some("turn_complete"),
        EventMsg::TurnAborted(_) => Some("turn_aborted"),
        EventMsg::ThreadNameUpdated(_) => Some("thread_name_updated"),
        EventMsg::ThreadRolledBack(_) => Some("thread_rolled_back"),
        EventMsg::Error(_) => Some("error"),
        EventMsg::Warning(_) => Some("warning"),
        EventMsg::ShutdownComplete => Some("shutdown_complete"),
        EventMsg::GuardianWarning(_)
        | EventMsg::RealtimeConversationStarted(_)
        | EventMsg::RealtimeConversationRealtime(_)
        | EventMsg::RealtimeConversationClosed(_)
        | EventMsg::RealtimeConversationSdp(_)
        | EventMsg::ModelReroute(_)
        | EventMsg::ModelVerification(_)
        | EventMsg::ContextCompacted(_)
        | EventMsg::TokenCount(_)
        | EventMsg::AgentMessage(_)
        | EventMsg::UserMessage(_)
        | EventMsg::AgentMessageDelta(_)
        | EventMsg::AgentReasoning(_)
        | EventMsg::AgentReasoningDelta(_)
        | EventMsg::AgentReasoningRawContent(_)
        | EventMsg::AgentReasoningRawContentDelta(_)
        | EventMsg::AgentReasoningSectionBreak(_)
        | EventMsg::ThreadGoalUpdated(_)
        | EventMsg::McpStartupUpdate(_)
        | EventMsg::McpStartupComplete(_)
        | EventMsg::McpToolCallBegin(_)
        | EventMsg::McpToolCallEnd(_)
        | EventMsg::WebSearchBegin(_)
        | EventMsg::WebSearchEnd(_)
        | EventMsg::ImageGenerationBegin(_)
        | EventMsg::ImageGenerationEnd(_)
        | EventMsg::ExecCommandBegin(_)
        | EventMsg::ExecCommandOutputDelta(_)
        | EventMsg::TerminalInteraction(_)
        | EventMsg::ExecCommandEnd(_)
        | EventMsg::ViewImageToolCall(_)
        | EventMsg::ExecApprovalRequest(_)
        | EventMsg::RequestPermissions(_)
        | EventMsg::RequestUserInput(_)
        | EventMsg::DynamicToolCallRequest(_)
        | EventMsg::DynamicToolCallResponse(_)
        | EventMsg::ElicitationRequest(_)
        | EventMsg::ApplyPatchApprovalRequest(_)
        | EventMsg::GuardianAssessment(_)
        | EventMsg::DeprecationNotice(_)
        | EventMsg::BackgroundEvent(_)
        | EventMsg::UndoStarted(_)
        | EventMsg::UndoCompleted(_)
        | EventMsg::StreamError(_)
        | EventMsg::PatchApplyBegin(_)
        | EventMsg::PatchApplyUpdated(_)
        | EventMsg::PatchApplyEnd(_)
        | EventMsg::TurnDiff(_)
        | EventMsg::GetHistoryEntryResponse(_)
        | EventMsg::McpListToolsResponse(_)
        | EventMsg::ListSkillsResponse(_)
        | EventMsg::RealtimeConversationListVoicesResponse(_)
        | EventMsg::SkillsUpdateAvailable
        | EventMsg::PlanUpdate(_)
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
        | EventMsg::CollabResumeEnd(_) => None,
    }
}

trait TraceExecutionStatus {
    fn trace_execution_status(&self) -> ExecutionStatus;
}

impl TraceExecutionStatus for ExecCommandStatus {
    fn trace_execution_status(&self) -> ExecutionStatus {
        match self {
            ExecCommandStatus::Completed => ExecutionStatus::Completed,
            ExecCommandStatus::Failed => ExecutionStatus::Failed,
            ExecCommandStatus::Declined => ExecutionStatus::Cancelled,
        }
    }
}

impl TraceExecutionStatus for PatchApplyStatus {
    fn trace_execution_status(&self) -> ExecutionStatus {
        match self {
            PatchApplyStatus::Completed => ExecutionStatus::Completed,
            PatchApplyStatus::Failed => ExecutionStatus::Failed,
            PatchApplyStatus::Declined => ExecutionStatus::Cancelled,
        }
    }
}

fn execution_status_for_abort_reason(reason: &TurnAbortReason) -> ExecutionStatus {
    match reason {
        TurnAbortReason::Interrupted
        | TurnAbortReason::Replaced
        | TurnAbortReason::ReviewEnded
        | TurnAbortReason::BudgetLimited => ExecutionStatus::Cancelled,
    }
}
