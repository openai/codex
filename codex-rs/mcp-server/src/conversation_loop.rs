use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::exec_approval::handle_exec_approval_request;
use crate::mcp_protocol::CodexEventNotificationParams;
use crate::mcp_protocol::ConversationId;
use crate::mcp_protocol::InitialStateNotificationParams;
use crate::mcp_protocol::InitialStatePayload;
use crate::mcp_protocol::NotificationMeta;
use crate::outgoing_message::OutgoingMessageSender;
use crate::patch_approval::handle_patch_approval_request;
use codex_core::Codex;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::FileChange;
use mcp_types::RequestId;
use tokio::sync::watch::Receiver as WatchReceiver;
use tracing::error;
use uuid::Uuid;

/// Conversation event loop bridging Codex events to MCP notifications.
///
/// Semantics:
/// - Always buffers all Codex events to include in an InitialState snapshot when
///   streaming turns on.
/// - Streams notifications live when `streaming_enabled` is true.
/// - Defers exec/patch approval elicitations until streaming turns on so
///   the client first receives InitialState, then the corresponding requests.
pub async fn run_conversation_loop(
    codex: Arc<Codex>,
    outgoing: Arc<OutgoingMessageSender>,
    request_id: RequestId,
    mut stream_rx: WatchReceiver<bool>,
    session_id: Uuid,
    running_sessions: Arc<Mutex<HashSet<Uuid>>>,
) {
    let request_id_str = match &request_id {
        RequestId::String(s) => s.clone(),
        RequestId::Integer(n) => n.to_string(),
    };

    // Buffer all events to include in InitialState when streaming is enabled
    let mut buffered_events: Vec<CodexEventNotificationParams> = Vec::new();
    let mut streaming_enabled = *stream_rx.borrow();

    let mut pending_elicitations: Vec<PendingElicitation> = Vec::new();

    let ctx = LoopCtx {
        outgoing: outgoing.clone(),
        codex: codex.clone(),
        request_id: request_id.clone(),
        request_id_str: request_id_str.clone(),
    };

    loop {
        tokio::select! {
            res = codex.next_event() => {
                match res {
                    Ok(event) => {
                        // Always buffer the event
                        buffered_events.push(CodexEventNotificationParams { meta: None, msg: event.msg.clone() });
                        // Stream immediately if enabled
                        stream_event_if_enabled(streaming_enabled, &ctx, &event.msg).await;

                        match event.msg {
                    EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent { command, cwd, call_id, reason: _ }) => {
                        process_exec_request(
                            streaming_enabled,
                            &mut pending_elicitations,
                            command,
                            cwd,
                            call_id,
                            event.id.clone(),
                            &ctx,
                        )
                        .await;
                        continue;
                    }
                    EventMsg::Error(_) => {
                        error!("Codex runtime error");
                    }
                    EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent { call_id, reason, grant_root, changes }) => {
                        process_patch_request(
                            streaming_enabled,
                            &mut pending_elicitations,
                            PatchReq {
                                call_id,
                                reason,
                                grant_root,
                                changes,
                                event_id: event.id.clone(),
                            },
                            &ctx,
                        )
                        .await;
                        continue;
                    }
                    EventMsg::TaskComplete(_) => {
                        handle_task_complete(&running_sessions, &session_id).await;
                    }
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
                    EventMsg::TaskStarted
                    | EventMsg::TokenCount(_)
                    | EventMsg::AgentReasoning(_)
                    | EventMsg::McpToolCallBegin(_)
                    | EventMsg::McpToolCallEnd(_)
                    | EventMsg::ExecCommandBegin(_)
                    | EventMsg::ExecCommandEnd(_)
                    | EventMsg::BackgroundEvent(_)
                    | EventMsg::ExecCommandOutputDelta(_)
                    | EventMsg::PatchApplyBegin(_)
                    | EventMsg::PatchApplyEnd(_)
                    | EventMsg::GetHistoryEntryResponse(_)
                    | EventMsg::PlanUpdate(_)
                    | EventMsg::ShutdownComplete => {
                    }
                        }
                    }
                    Err(e) => {
                        error!("Codex runtime error: {e}");
                    }
                }
            },
            changed = stream_rx.changed() => {
                if changed.is_ok() {
                    let now = *stream_rx.borrow();
                    handle_stream_change(
                        now,
                        &mut streaming_enabled,
                        session_id,
                        &buffered_events,
                        &mut pending_elicitations,
                        &ctx,
                    )
                    .await;
                }
            }
        }
    }
}

/// Deferred elicitation requests to be sent after InitialState when
/// streaming is enabled. Preserves original event order (FIFO).
enum PendingElicitation {
    Exec {
        command: Vec<String>,
        cwd: PathBuf,
        event_id: String,
        call_id: String,
    },
    Patch {
        call_id: String,
        reason: Option<String>,
        grant_root: Option<PathBuf>,
        changes: HashMap<PathBuf, FileChange>,
        event_id: String,
    },
}

/// Immutable context shared across helper functions to avoid long
/// argument lists.
struct LoopCtx {
    outgoing: Arc<OutgoingMessageSender>,
    codex: Arc<Codex>,
    request_id: RequestId,
    request_id_str: String,
}

/// Snapshot of a patch approval request used to defer elicitation.
struct PatchReq {
    call_id: String,
    reason: Option<String>,
    grant_root: Option<PathBuf>,
    changes: HashMap<PathBuf, FileChange>,
    event_id: String,
}

/// Streams a single Codex event as an MCP notification if streaming is enabled.
async fn stream_event_if_enabled(streaming_enabled: bool, ctx: &LoopCtx, msg: &EventMsg) {
    if !streaming_enabled {
        return;
    }
    let method = msg.to_string();
    let params = CodexEventNotificationParams {
        meta: None,
        msg: msg.clone(),
    };
    if let Ok(params_val) = serde_json::to_value(&params) {
        ctx.outgoing
            .send_custom_notification(&method, params_val)
            .await;
    } else {
        error!("Failed to serialize event params");
    }
}

/// Handles an exec approval request. If streaming is disabled, defers the
/// elicitation until after InitialState; otherwise elicits immediately.
async fn process_exec_request(
    streaming_enabled: bool,
    pending: &mut Vec<PendingElicitation>,
    command: Vec<String>,
    cwd: PathBuf,
    call_id: String,
    event_id: String,
    ctx: &LoopCtx,
) {
    if streaming_enabled {
        handle_exec_approval_request(
            command,
            cwd,
            ctx.outgoing.clone(),
            ctx.codex.clone(),
            ctx.request_id.clone(),
            ctx.request_id_str.clone(),
            event_id,
            call_id,
        )
        .await;
    } else {
        pending.push(PendingElicitation::Exec {
            command,
            cwd,
            event_id,
            call_id,
        });
    }
}

/// Handles a patch approval request. If streaming is disabled, defers the
/// elicitation until after InitialState; otherwise elicits immediately.
async fn process_patch_request(
    streaming_enabled: bool,
    pending: &mut Vec<PendingElicitation>,
    req: PatchReq,
    ctx: &LoopCtx,
) {
    let PatchReq {
        call_id,
        reason,
        grant_root,
        changes,
        event_id,
    } = req;
    if streaming_enabled {
        handle_patch_approval_request(
            call_id,
            reason,
            grant_root,
            changes,
            ctx.outgoing.clone(),
            ctx.codex.clone(),
            ctx.request_id.clone(),
            ctx.request_id_str.clone(),
            event_id,
        )
        .await;
    } else {
        pending.push(PendingElicitation::Patch {
            call_id,
            reason,
            grant_root,
            changes,
            event_id,
        });
    }
}

/// Handles a streaming state change.
///
/// When enabling streaming:
/// 1) emits InitialState with all buffered events
/// 2) drains and sends any deferred elicitations
async fn handle_stream_change(
    now: bool,
    streaming_enabled: &mut bool,
    session_id: Uuid,
    buffered_events: &[CodexEventNotificationParams],
    pending: &mut Vec<PendingElicitation>,
    ctx: &LoopCtx,
) {
    if now && !*streaming_enabled {
        *streaming_enabled = true;
        emit_initial_state(ctx, session_id, buffered_events).await;
        drain_pending_elicitations(pending, ctx).await;
    } else if !now && *streaming_enabled {
        *streaming_enabled = false;
    }
}

/// Emits the InitialState snapshot to the client.
async fn emit_initial_state(
    ctx: &LoopCtx,
    session_id: Uuid,
    buffered_events: &[CodexEventNotificationParams],
) {
    let params = InitialStateNotificationParams {
        meta: Some(NotificationMeta {
            conversation_id: Some(ConversationId(session_id)),
            request_id: None,
        }),
        initial_state: InitialStatePayload {
            events: buffered_events.to_vec(),
        },
    };
    if let Ok(params_val) = serde_json::to_value(&params) {
        ctx.outgoing
            .send_custom_notification("notifications/initial_state", params_val)
            .await;
    } else {
        error!("Failed to serialize InitialState params");
    }
}

/// Sends any deferred exec/patch elicitations in FIFO order.
async fn drain_pending_elicitations(pending: &mut Vec<PendingElicitation>, ctx: &LoopCtx) {
    for item in pending.drain(..) {
        match item {
            PendingElicitation::Exec {
                command,
                cwd,
                event_id,
                call_id,
            } => {
                handle_exec_approval_request(
                    command,
                    cwd,
                    ctx.outgoing.clone(),
                    ctx.codex.clone(),
                    ctx.request_id.clone(),
                    ctx.request_id_str.clone(),
                    event_id,
                    call_id,
                )
                .await;
            }
            PendingElicitation::Patch {
                call_id,
                reason,
                grant_root,
                changes,
                event_id,
            } => {
                handle_patch_approval_request(
                    call_id,
                    reason,
                    grant_root,
                    changes,
                    ctx.outgoing.clone(),
                    ctx.codex.clone(),
                    ctx.request_id.clone(),
                    ctx.request_id_str.clone(),
                    event_id,
                )
                .await;
            }
        }
    }
}

/// Removes the session id from the shared running set when a task completes.
async fn handle_task_complete(running_sessions: &Arc<Mutex<HashSet<Uuid>>>, session_id: &Uuid) {
    let mut running_sessions = running_sessions.lock().await;
    running_sessions.remove(session_id);
}
