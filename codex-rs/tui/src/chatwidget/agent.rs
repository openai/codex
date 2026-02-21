use std::collections::HashMap;
use std::sync::Arc;

use codex_app_server::EmbeddedSessionClient;
use codex_app_server::EmbeddedSessionClientArgs;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::DynamicToolCallOutputContentItem as V2DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallResponse as V2DynamicToolCallResponse;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::FileChangeRequestApprovalResponse;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadSubmitOpParams;
use codex_app_server_protocol::ToolRequestUserInputAnswer;
use codex_app_server_protocol::ToolRequestUserInputResponse;
use codex_core::AuthManager;
use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_feedback::CodexFeedback;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem as CoreDynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolResponse as CoreDynamicToolResponse;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::unbounded_channel;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

#[derive(Default)]
struct PendingServerRequests {
    exec_approval_by_id: HashMap<String, AppServerRequestId>,
    patch_approval_by_item_id: HashMap<String, AppServerRequestId>,
    user_input_by_turn_id: HashMap<String, AppServerRequestId>,
    dynamic_tool_by_call_id: HashMap<String, AppServerRequestId>,
}

/// Spawn the agent bootstrapper and op forwarding loop, returning the
/// `UnboundedSender<Op>` used by the UI to submit operations.
pub(crate) fn spawn_agent(
    config: Config,
    app_event_tx: AppEventSender,
    server: Arc<ThreadManager>,
    auth_manager: Arc<AuthManager>,
    feedback: CodexFeedback,
) -> UnboundedSender<Op> {
    let (codex_op_tx, mut codex_op_rx) = unbounded_channel::<Op>();

    tokio::spawn(async move {
        let mut client = EmbeddedSessionClient::spawn(EmbeddedSessionClientArgs {
            auth_manager,
            thread_manager: server,
            codex_linux_sandbox_exe: config.codex_linux_sandbox_exe.clone(),
            config: config.clone(),
            cli_overrides: Vec::new(),
            feedback,
        });

        let mut next_request_id = 1_i64;
        let start_request_id = next_request_id_value(&mut next_request_id);
        let start_request = ClientRequest::ThreadStart {
            request_id: start_request_id.clone(),
            params: thread_start_params_from_config(&config),
        };
        if let Err(err) = client.send_request(start_request).await {
            let message = format!("Failed to initialize codex app-server session: {err}");
            tracing::error!("{message}");
            app_event_tx.send(AppEvent::FatalExitRequest(message));
            return;
        }

        let mut thread_id: Option<String> = None;
        let mut pending = PendingServerRequests::default();

        while thread_id.is_none() {
            let Some(message) = client.recv().await else {
                let message =
                    "Embedded app-server session closed before thread/start completed".to_string();
                tracing::error!("{message}");
                app_event_tx.send(AppEvent::FatalExitRequest(message));
                return;
            };
            if handle_app_server_message(
                &app_event_tx,
                &start_request_id,
                Some(&mut thread_id),
                &mut pending,
                message,
            ) {
                return;
            }
        }

        let Some(thread_id) = thread_id else {
            let message = "thread/start did not return a thread id".to_string();
            tracing::error!("{message}");
            app_event_tx.send(AppEvent::FatalExitRequest(message));
            return;
        };

        loop {
            tokio::select! {
                maybe_op = codex_op_rx.recv() => {
                    let Some(op) = maybe_op else {
                        break;
                    };

                    let handled = match try_handle_server_request_reply_op(&mut client, &mut pending, &op).await {
                        Ok(handled) => handled,
                        Err(err) => {
                            tracing::error!("failed to answer app-server server request: {err}");
                            false
                        }
                    };
                    if handled {
                        continue;
                    }
                    if is_server_request_reply_op(&op) {
                        tracing::warn!("dropping reply op without pending app-server request: {op:?}");
                        continue;
                    }

                    if let Err(err) = submit_thread_op(&client, &thread_id, &op, &mut next_request_id).await {
                        tracing::error!("failed to submit op via app-server: {err}");
                    }
                }
                maybe_message = client.recv() => {
                    let Some(message) = maybe_message else {
                        break;
                    };
                    if handle_app_server_message(
                        &app_event_tx,
                        &start_request_id,
                        None,
                        &mut pending,
                        message,
                    ) {
                        break;
                    }
                }
            }
        }
    });

    codex_op_tx
}

fn handle_app_server_message(
    app_event_tx: &AppEventSender,
    start_request_id: &AppServerRequestId,
    thread_id: Option<&mut Option<String>>,
    pending: &mut PendingServerRequests,
    message: JSONRPCMessage,
) -> bool {
    match message {
        JSONRPCMessage::Notification(notification) => {
            if let Some(event) = decode_raw_codex_event_notification(notification) {
                let is_shutdown_complete = matches!(event.msg, EventMsg::ShutdownComplete);
                app_event_tx.send(AppEvent::CodexEvent(event));
                if is_shutdown_complete {
                    return true;
                }
            }
        }
        JSONRPCMessage::Request(request) => {
            if let Ok(server_request) = ServerRequest::try_from(request) {
                match server_request {
                    ServerRequest::CommandExecutionRequestApproval { request_id, params } => {
                        let key = params.approval_id.unwrap_or(params.item_id);
                        pending.exec_approval_by_id.insert(key, request_id);
                    }
                    ServerRequest::FileChangeRequestApproval { request_id, params } => {
                        pending
                            .patch_approval_by_item_id
                            .insert(params.item_id, request_id);
                    }
                    ServerRequest::ToolRequestUserInput { request_id, params } => {
                        pending
                            .user_input_by_turn_id
                            .insert(params.turn_id, request_id);
                    }
                    ServerRequest::DynamicToolCall { request_id, params } => {
                        pending
                            .dynamic_tool_by_call_id
                            .insert(params.call_id, request_id);
                    }
                    _ => {}
                }
            }
        }
        JSONRPCMessage::Response(response) => {
            if &response.id == start_request_id {
                match serde_json::from_value::<ThreadStartResponse>(response.result) {
                    Ok(parsed) => {
                        if let Some(thread_id) = thread_id {
                            *thread_id = Some(parsed.thread.id);
                        }
                    }
                    Err(err) => {
                        tracing::error!("failed to decode thread/start response: {err}");
                    }
                }
            }
        }
        JSONRPCMessage::Error(error) => {
            if &error.id == start_request_id {
                let message = format!(
                    "thread/start failed: code={} message={}",
                    error.error.code, error.error.message
                );
                tracing::error!("{message}");
                app_event_tx.send(AppEvent::FatalExitRequest(message));
                return true;
            }
        }
    }

    false
}

fn decode_raw_codex_event_notification(
    notification: codex_app_server_protocol::JSONRPCNotification,
) -> Option<Event> {
    if !notification.method.starts_with("codex/event/") {
        return None;
    }
    let params = notification.params?;
    match serde_json::from_value::<Event>(params) {
        Ok(event) => Some(event),
        Err(err) => {
            tracing::warn!("failed to decode raw codex event notification: {err}");
            None
        }
    }
}

fn thread_start_params_from_config(config: &Config) -> ThreadStartParams {
    ThreadStartParams {
        model: config.model.clone(),
        model_provider: Some(config.model_provider_id.clone()),
        cwd: Some(config.cwd.display().to_string()),
        approval_policy: Some((*config.permissions.approval_policy.get()).into()),
        personality: config.personality,
        experimental_raw_events: true,
        ..Default::default()
    }
}

async fn submit_thread_op(
    client: &EmbeddedSessionClient,
    thread_id: &str,
    op: &Op,
    next_request_id: &mut i64,
) -> std::io::Result<()> {
    let request = ClientRequest::ThreadSubmitOp {
        request_id: next_request_id_value(next_request_id),
        params: ThreadSubmitOpParams {
            thread_id: thread_id.to_string(),
            op: serde_json::to_value(op).map_err(std::io::Error::other)?,
        },
    };
    client.send_request(request).await
}

fn next_request_id_value(next_request_id: &mut i64) -> AppServerRequestId {
    let value = *next_request_id;
    *next_request_id += 1;
    AppServerRequestId::Integer(value)
}

fn is_server_request_reply_op(op: &Op) -> bool {
    matches!(
        op,
        Op::ExecApproval { .. }
            | Op::PatchApproval { .. }
            | Op::UserInputAnswer { .. }
            | Op::DynamicToolResponse { .. }
    )
}

async fn try_handle_server_request_reply_op(
    client: &mut EmbeddedSessionClient,
    pending: &mut PendingServerRequests,
    op: &Op,
) -> std::io::Result<bool> {
    match op {
        Op::ExecApproval { id, decision, .. } => {
            if let Some(request_id) = pending.exec_approval_by_id.remove(id) {
                let response = CommandExecutionRequestApprovalResponse {
                    decision: map_exec_approval_decision(decision.clone()),
                };
                send_jsonrpc_response(client, request_id, response).await?;
                return Ok(true);
            }
        }
        Op::PatchApproval { id, decision } => {
            if let Some(request_id) = pending.patch_approval_by_item_id.remove(id) {
                let response = FileChangeRequestApprovalResponse {
                    decision: map_file_change_approval_decision(decision.clone()),
                };
                send_jsonrpc_response(client, request_id, response).await?;
                return Ok(true);
            }
        }
        Op::UserInputAnswer { id, response } => {
            if let Some(request_id) = pending.user_input_by_turn_id.remove(id) {
                let response = ToolRequestUserInputResponse {
                    answers: response
                        .answers
                        .iter()
                        .map(|(question_id, answer)| {
                            (
                                question_id.clone(),
                                ToolRequestUserInputAnswer {
                                    answers: answer.answers.clone(),
                                },
                            )
                        })
                        .collect(),
                };
                send_jsonrpc_response(client, request_id, response).await?;
                return Ok(true);
            }
        }
        Op::DynamicToolResponse { id, response } => {
            if let Some(request_id) = pending.dynamic_tool_by_call_id.remove(id) {
                let response = v2_dynamic_tool_response_from_core(response.clone());
                send_jsonrpc_response(client, request_id, response).await?;
                return Ok(true);
            }
        }
        _ => {}
    }

    Ok(false)
}

async fn send_jsonrpc_response<T: serde::Serialize>(
    client: &mut EmbeddedSessionClient,
    request_id: AppServerRequestId,
    response: T,
) -> std::io::Result<()> {
    let result = serde_json::to_value(response).map_err(std::io::Error::other)?;
    client
        .send_response(JSONRPCResponse {
            id: request_id,
            result,
        })
        .await
}

fn map_exec_approval_decision(decision: ReviewDecision) -> CommandExecutionApprovalDecision {
    match decision {
        ReviewDecision::Approved => CommandExecutionApprovalDecision::Accept,
        ReviewDecision::ApprovedForSession => CommandExecutionApprovalDecision::AcceptForSession,
        ReviewDecision::ApprovedExecpolicyAmendment {
            proposed_execpolicy_amendment,
        } => CommandExecutionApprovalDecision::AcceptWithExecpolicyAmendment {
            execpolicy_amendment: proposed_execpolicy_amendment.into(),
        },
        ReviewDecision::Denied => CommandExecutionApprovalDecision::Decline,
        ReviewDecision::Abort => CommandExecutionApprovalDecision::Cancel,
    }
}

fn map_file_change_approval_decision(decision: ReviewDecision) -> FileChangeApprovalDecision {
    match decision {
        ReviewDecision::Approved => FileChangeApprovalDecision::Accept,
        ReviewDecision::ApprovedForSession => FileChangeApprovalDecision::AcceptForSession,
        ReviewDecision::Denied => FileChangeApprovalDecision::Decline,
        ReviewDecision::Abort => FileChangeApprovalDecision::Cancel,
        ReviewDecision::ApprovedExecpolicyAmendment { .. } => FileChangeApprovalDecision::Accept,
    }
}

fn v2_dynamic_tool_response_from_core(
    response: CoreDynamicToolResponse,
) -> V2DynamicToolCallResponse {
    V2DynamicToolCallResponse {
        content_items: response
            .content_items
            .into_iter()
            .map(|item| match item {
                CoreDynamicToolCallOutputContentItem::InputText { text } => {
                    V2DynamicToolCallOutputContentItem::InputText { text }
                }
                CoreDynamicToolCallOutputContentItem::InputImage { image_url } => {
                    V2DynamicToolCallOutputContentItem::InputImage { image_url }
                }
            })
            .collect(),
        success: response.success,
    }
}

/// Spawn agent loops for an existing thread (e.g., a forked thread).
/// Sends the provided `SessionConfiguredEvent` immediately, then forwards subsequent
/// events and accepts Ops for submission.
pub(crate) fn spawn_agent_from_existing(
    thread: std::sync::Arc<CodexThread>,
    session_configured: codex_protocol::protocol::SessionConfiguredEvent,
    app_event_tx: AppEventSender,
) -> UnboundedSender<Op> {
    let (codex_op_tx, mut codex_op_rx) = unbounded_channel::<Op>();

    let app_event_tx_clone = app_event_tx;
    tokio::spawn(async move {
        // Forward the captured `SessionConfigured` event so it can be rendered in the UI.
        let ev = codex_protocol::protocol::Event {
            id: "".to_string(),
            msg: codex_protocol::protocol::EventMsg::SessionConfigured(session_configured),
        };
        app_event_tx_clone.send(AppEvent::CodexEvent(ev));

        let thread_clone = thread.clone();
        tokio::spawn(async move {
            while let Some(op) = codex_op_rx.recv().await {
                let id = thread_clone.submit(op).await;
                if let Err(e) = id {
                    tracing::error!("failed to submit op: {e}");
                }
            }
        });

        while let Ok(event) = thread.next_event().await {
            let is_shutdown_complete = matches!(event.msg, EventMsg::ShutdownComplete);
            app_event_tx_clone.send(AppEvent::CodexEvent(event));
            if is_shutdown_complete {
                // ShutdownComplete is terminal for a thread; drop this receiver task so
                // the Arc<CodexThread> can be released and thread resources can clean up.
                break;
            }
        }
    });

    codex_op_tx
}

/// Spawn an op-forwarding loop for an existing thread without subscribing to events.
pub(crate) fn spawn_op_forwarder(thread: std::sync::Arc<CodexThread>) -> UnboundedSender<Op> {
    let (codex_op_tx, mut codex_op_rx) = unbounded_channel::<Op>();

    tokio::spawn(async move {
        while let Some(op) = codex_op_rx.recv().await {
            if let Err(e) = thread.submit(op).await {
                tracing::error!("failed to submit op: {e}");
            }
        }
    });

    codex_op_tx
}
