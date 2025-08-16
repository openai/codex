use std::sync::Arc;

use crate::exec_approval::handle_exec_approval_request;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::OutgoingNotificationMeta;
use crate::patch_approval::handle_patch_approval_request;
use codex_core::CodexConversation;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use mcp_types::RequestId;
use tracing::error;

pub async fn run_conversation_loop(
    codex: Arc<CodexConversation>,
    outgoing: Arc<OutgoingMessageSender>,
    request_id: RequestId,
) {
    let request_id_str = match &request_id {
        RequestId::String(s) => s.clone(),
        RequestId::Integer(n) => n.to_string(),
    };

    // Stream events until the task needs to pause for user interaction or
    // completes.
    loop {
        match codex.next_event().await {
            Ok(event) => {
                outgoing
                    .send_event_as_notification(
                        &event,
                        Some(OutgoingNotificationMeta::new(Some(request_id.clone()))),
                    )
                    .await;

                match event.msg {
                    EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                        command,
                        cwd,
                        call_id,
                        reason: _,
                    }) => {
                        handle_exec_approval_request(
                            command,
                            cwd,
                            outgoing.clone(),
                            codex.clone(),
                            request_id.clone(),
                            request_id_str.clone(),
                            event.id.clone(),
                            call_id,
                        )
                        .await;
                    }
                    EventMsg::Error(_) => {
                        error!("Codex runtime error");
                    }
                    EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
                        call_id,
                        reason,
                        grant_root,
                        changes,
                    }) => {
                        handle_patch_approval_request(
                            call_id,
                            reason,
                            grant_root,
                            changes,
                            outgoing.clone(),
                            codex.clone(),
                            request_id.clone(),
                            request_id_str.clone(),
                            event.id.clone(),
                        )
                        .await;
                    }
                    EventMsg::TaskComplete(_) => {}
                    EventMsg::SessionConfigured(_) => {
                        tracing::error!("unexpected SessionConfigured event");
                    }
                    EventMsg::AgentMessageDelta(_) => {
                        // TODO: think how we want to support this in the MCP
                    }
                    EventMsg::AgentReasoningDelta(_) => {
                        // TODO: think how we want to support this in the MCP
                    }
                    EventMsg::AgentMessage(AgentMessageEvent { .. }) => {
                        // TODO: think how we want to support this in the MCP
                    }
                    EventMsg::AgentReasoningRawContent(_)
                    | EventMsg::AgentReasoningRawContentDelta(_)
                    | EventMsg::TaskStarted
                    | EventMsg::TokenCount(_)
                    | EventMsg::AgentReasoning(_)
                    | EventMsg::AgentReasoningSectionBreak(_)
                    | EventMsg::McpToolCallBegin(_)
                    | EventMsg::McpToolCallEnd(_)
                    | EventMsg::ExecCommandBegin(_)
                    | EventMsg::ExecCommandEnd(_)
                    | EventMsg::TurnDiff(_)
                    | EventMsg::BackgroundEvent(_)
                    | EventMsg::ExecCommandOutputDelta(_)
                    | EventMsg::PatchApplyBegin(_)
                    | EventMsg::PatchApplyEnd(_)
                    | EventMsg::GetHistoryEntryResponse(_)
                    | EventMsg::PlanUpdate(_)
                    | EventMsg::TurnAborted(_)
                    | EventMsg::ShutdownComplete => {
                        // For now, we do not do anything extra for these
                        // events. Note that
                        // send(codex_event_to_notification(&event)) above has
                        // already dispatched these events as notifications,
                        // though we may want to do give different treatment to
                        // individual events in the future.
                    }
                }
            }
            Err(e) => {
                error!("Codex runtime error: {e}");
            }
        }
    }
}
