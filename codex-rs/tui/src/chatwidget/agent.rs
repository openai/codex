use std::collections::HashMap;
use std::sync::Arc;

use crate::agent_command::AgentCommand;
use crate::app_event::ThreadBootstrap;
use codex_app_server::EmbeddedSessionClient;
use codex_app_server::EmbeddedSessionMessage;
use codex_app_server_protocol as app_proto;
use codex_app_server_protocol::RequestId as JsonRpcRequestId;
use codex_core::config::Config;
use codex_protocol::ThreadId;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;
use tokio::sync::broadcast::error::RecvError as BroadcastRecvError;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::unbounded_channel;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PendingServerRequestKey {
    ExecApproval(String),
    PatchApproval(String),
    UserInput(String),
    DynamicTool(String),
    McpElicitation {
        server_name: String,
        request_id: codex_protocol::mcp::RequestId,
    },
}

#[derive(Debug, Default)]
struct PendingServerRequests {
    by_key: HashMap<PendingServerRequestKey, JsonRpcRequestId>,
}

impl PendingServerRequests {
    fn insert(&mut self, request_id: JsonRpcRequestId, request: &app_proto::ServerRequest) {
        let Some(key) = pending_key_for_server_request(request) else {
            return;
        };
        self.by_key.insert(key, request_id);
    }

    fn take(&mut self, key: PendingServerRequestKey) -> Option<JsonRpcRequestId> {
        self.by_key.remove(&key)
    }
}

async fn take_pending_request_id(
    pending_server_requests: &Mutex<PendingServerRequests>,
    key: PendingServerRequestKey,
) -> Option<JsonRpcRequestId> {
    let request_id = pending_server_requests.lock().await.take(key.clone());
    if request_id.is_none() {
        tracing::warn!(?key, "pending app-server request not found for response");
    }
    request_id
}

async fn respond_if_pending<T: Serialize>(
    app_server_client: &EmbeddedSessionClient,
    pending_server_requests: &Mutex<PendingServerRequests>,
    key: PendingServerRequestKey,
    response: T,
) -> std::result::Result<(), app_proto::JSONRPCErrorError> {
    let Some(request_id) = take_pending_request_id(pending_server_requests, key).await else {
        return Ok(());
    };
    app_server_client.respond(request_id, response).await
}

fn pending_key_for_server_request(
    request: &app_proto::ServerRequest,
) -> Option<PendingServerRequestKey> {
    match request {
        app_proto::ServerRequest::CommandExecutionRequestApproval { params, .. } => {
            Some(PendingServerRequestKey::ExecApproval(
                params
                    .approval_id
                    .clone()
                    .unwrap_or_else(|| params.item_id.clone()),
            ))
        }
        app_proto::ServerRequest::FileChangeRequestApproval { params, .. } => Some(
            PendingServerRequestKey::PatchApproval(params.item_id.clone()),
        ),
        app_proto::ServerRequest::ToolRequestUserInput { params, .. } => {
            Some(PendingServerRequestKey::UserInput(params.item_id.clone()))
        }
        app_proto::ServerRequest::DynamicToolCall { params, .. } => {
            Some(PendingServerRequestKey::DynamicTool(params.call_id.clone()))
        }
        app_proto::ServerRequest::McpElicitationRequest { params, .. } => {
            Some(PendingServerRequestKey::McpElicitation {
                server_name: params.server_name.clone(),
                request_id: match &params.request_id {
                    app_proto::RequestId::String(value) => {
                        codex_protocol::mcp::RequestId::String(value.clone())
                    }
                    app_proto::RequestId::Integer(value) => {
                        codex_protocol::mcp::RequestId::Integer(*value)
                    }
                },
            })
        }
        _ => None,
    }
}

fn server_request_thread_id(request: &app_proto::ServerRequest) -> Option<&str> {
    match request {
        app_proto::ServerRequest::CommandExecutionRequestApproval { params, .. } => {
            Some(params.thread_id.as_str())
        }
        app_proto::ServerRequest::FileChangeRequestApproval { params, .. } => {
            Some(params.thread_id.as_str())
        }
        app_proto::ServerRequest::ToolRequestUserInput { params, .. } => {
            Some(params.thread_id.as_str())
        }
        app_proto::ServerRequest::DynamicToolCall { params, .. } => Some(params.thread_id.as_str()),
        app_proto::ServerRequest::McpElicitationRequest { params, .. } => {
            Some(params.thread_id.as_str())
        }
        app_proto::ServerRequest::ChatgptAuthTokensRefresh { .. }
        | app_proto::ServerRequest::ApplyPatchApproval { .. }
        | app_proto::ServerRequest::ExecCommandApproval { .. } => None,
    }
}

fn convert_via_serde<T, U>(value: T) -> Result<U, serde_json::Error>
where
    T: Serialize,
    U: DeserializeOwned,
{
    serde_json::from_value(serde_json::to_value(value)?)
}

fn convert_exec_approval_decision(
    decision: codex_protocol::protocol::ReviewDecision,
) -> Option<app_proto::CommandExecutionApprovalDecision> {
    match decision {
        codex_protocol::protocol::ReviewDecision::Approved => {
            Some(app_proto::CommandExecutionApprovalDecision::Accept)
        }
        codex_protocol::protocol::ReviewDecision::ApprovedForSession => {
            Some(app_proto::CommandExecutionApprovalDecision::AcceptForSession)
        }
        codex_protocol::protocol::ReviewDecision::ApprovedExecpolicyAmendment {
            proposed_execpolicy_amendment,
        } => match convert_via_serde(proposed_execpolicy_amendment) {
            Ok(execpolicy_amendment) => Some(
                app_proto::CommandExecutionApprovalDecision::AcceptWithExecpolicyAmendment {
                    execpolicy_amendment,
                },
            ),
            Err(err) => {
                tracing::error!("failed to convert execpolicy amendment: {err}");
                None
            }
        },
        codex_protocol::protocol::ReviewDecision::Denied => {
            Some(app_proto::CommandExecutionApprovalDecision::Decline)
        }
        codex_protocol::protocol::ReviewDecision::Abort => {
            Some(app_proto::CommandExecutionApprovalDecision::Cancel)
        }
    }
}

fn convert_patch_approval_decision(
    decision: codex_protocol::protocol::ReviewDecision,
) -> Option<app_proto::FileChangeApprovalDecision> {
    match decision {
        codex_protocol::protocol::ReviewDecision::Approved => {
            Some(app_proto::FileChangeApprovalDecision::Accept)
        }
        codex_protocol::protocol::ReviewDecision::ApprovedForSession => {
            Some(app_proto::FileChangeApprovalDecision::AcceptForSession)
        }
        codex_protocol::protocol::ReviewDecision::Denied => {
            Some(app_proto::FileChangeApprovalDecision::Decline)
        }
        codex_protocol::protocol::ReviewDecision::Abort => {
            Some(app_proto::FileChangeApprovalDecision::Cancel)
        }
        codex_protocol::protocol::ReviewDecision::ApprovedExecpolicyAmendment { .. } => {
            tracing::warn!("ignoring execpolicy amendment decision for patch approval");
            Some(app_proto::FileChangeApprovalDecision::Accept)
        }
    }
}

fn convert_elicitation_decision(
    decision: codex_protocol::approvals::ElicitationAction,
) -> app_proto::McpElicitationDecision {
    match decision {
        codex_protocol::approvals::ElicitationAction::Accept => {
            app_proto::McpElicitationDecision::Accept
        }
        codex_protocol::approvals::ElicitationAction::Decline => {
            app_proto::McpElicitationDecision::Decline
        }
        codex_protocol::approvals::ElicitationAction::Cancel => {
            app_proto::McpElicitationDecision::Cancel
        }
    }
}

fn convert_user_input_response(
    response: codex_protocol::request_user_input::RequestUserInputResponse,
) -> Result<app_proto::ToolRequestUserInputResponse, serde_json::Error> {
    convert_via_serde(response)
}

fn convert_dynamic_tool_response(
    response: codex_protocol::dynamic_tools::DynamicToolResponse,
) -> Result<app_proto::DynamicToolCallResponse, serde_json::Error> {
    convert_via_serde(response)
}

fn convert_review_target(
    target: codex_protocol::protocol::ReviewTarget,
) -> Result<app_proto::ReviewTarget, serde_json::Error> {
    convert_via_serde(target)
}

fn build_thread_bootstrap(
    thread: app_proto::Thread,
    model: String,
    model_provider_id: String,
    cwd: std::path::PathBuf,
    approval_policy: app_proto::AskForApproval,
    sandbox_policy: app_proto::SandboxPolicy,
    reasoning_effort: Option<codex_protocol::openai_models::ReasoningEffort>,
    history_log_id: u64,
    history_entry_count: usize,
    forked_from_id: Option<String>,
    network_proxy: Option<app_proto::SessionNetworkProxyRuntime>,
) -> Result<ThreadBootstrap, String> {
    let thread_id = ThreadId::try_from(thread.id.clone())
        .map_err(|err| format!("invalid thread id {}: {err}", thread.id))?;
    let forked_from_id = match forked_from_id {
        Some(forked_from_id) => Some(
            ThreadId::try_from(forked_from_id.clone())
                .map_err(|err| format!("invalid forked thread id {forked_from_id}: {err}"))?,
        ),
        None => None,
    };
    let network_proxy =
        network_proxy.map(
            |network_proxy| codex_protocol::protocol::SessionNetworkProxyRuntime {
                http_addr: network_proxy.http_addr,
                socks_addr: network_proxy.socks_addr,
                admin_addr: network_proxy.admin_addr,
            },
        );
    Ok(ThreadBootstrap {
        thread_id,
        rollout_path: thread.path.clone(),
        thread,
        model,
        model_provider_id,
        approval_policy: approval_policy.to_core(),
        sandbox_policy: sandbox_policy.to_core(),
        cwd,
        reasoning_effort,
        history_log_id,
        history_entry_count,
        forked_from_id,
        network_proxy,
    })
}

fn thread_bootstrap_from_start(
    response: app_proto::ThreadStartResponse,
) -> Result<ThreadBootstrap, String> {
    let app_proto::ThreadStartResponse {
        thread,
        model,
        model_provider,
        cwd,
        approval_policy,
        sandbox,
        reasoning_effort,
        history_log_id,
        history_entry_count,
        forked_from_thread_id,
        network_proxy,
    } = response;
    build_thread_bootstrap(
        thread,
        model,
        model_provider,
        cwd,
        approval_policy,
        sandbox,
        reasoning_effort,
        history_log_id,
        history_entry_count,
        forked_from_thread_id,
        network_proxy,
    )
}

fn spawn_server_request_tracker(
    app_server_client: Arc<EmbeddedSessionClient>,
    app_event_tx: AppEventSender,
    thread_id: ThreadId,
    pending_server_requests: Arc<Mutex<PendingServerRequests>>,
) {
    let thread_id_str = thread_id.to_string();
    tokio::spawn(async move {
        let mut rx = app_server_client.subscribe();
        loop {
            match rx.recv().await {
                Ok(EmbeddedSessionMessage::Request(request)) => {
                    if server_request_thread_id(&request.request) != Some(thread_id_str.as_str()) {
                        continue;
                    }
                    pending_server_requests
                        .lock()
                        .await
                        .insert(request.request_id, &request.request);
                    app_event_tx.send(AppEvent::AppServerRequest {
                        thread_id,
                        request: request.request,
                    });
                }
                Ok(EmbeddedSessionMessage::Notification(
                    app_proto::ServerNotification::ThreadShutdownCompleted(notification),
                )) => {
                    if notification.thread_id == thread_id_str {
                        break;
                    }
                }
                Ok(EmbeddedSessionMessage::Notification(_)) => {}
                Err(BroadcastRecvError::Lagged(skipped)) => {
                    tracing::warn!(
                        %thread_id,
                        skipped,
                        "server-request tracker lagged on app-server broadcast"
                    );
                }
                Err(BroadcastRecvError::Closed) => break,
            }
        }
    });
}

fn spawn_active_turn_tracker(
    app_server_client: Arc<EmbeddedSessionClient>,
    thread_id: ThreadId,
    active_turn_id: Arc<Mutex<Option<String>>>,
) {
    let thread_id = thread_id.to_string();
    tokio::spawn(async move {
        let mut rx = app_server_client.subscribe();
        loop {
            match rx.recv().await {
                Ok(EmbeddedSessionMessage::Notification(
                    app_proto::ServerNotification::TurnStarted(notification),
                )) => {
                    if notification.thread_id != thread_id {
                        continue;
                    }
                    *active_turn_id.lock().await = Some(notification.turn.id);
                }
                Ok(EmbeddedSessionMessage::Notification(
                    app_proto::ServerNotification::TurnCompleted(notification),
                )) => {
                    if notification.thread_id != thread_id {
                        continue;
                    }
                    let mut guard = active_turn_id.lock().await;
                    if guard.as_deref() == Some(notification.turn.id.as_str()) {
                        *guard = None;
                    }
                }
                Ok(EmbeddedSessionMessage::Notification(
                    app_proto::ServerNotification::ThreadShutdownCompleted(notification),
                )) => {
                    if notification.thread_id != thread_id {
                        continue;
                    }
                    *active_turn_id.lock().await = None;
                    break;
                }
                Ok(EmbeddedSessionMessage::Notification(_))
                | Ok(EmbeddedSessionMessage::Request(_)) => {}
                Err(BroadcastRecvError::Lagged(skipped)) => {
                    *active_turn_id.lock().await = None;
                    tracing::warn!(
                        thread_id = %thread_id,
                        skipped,
                        "active-turn tracker lagged on app-server broadcast; cleared active turn"
                    );
                }
                Err(BroadcastRecvError::Closed) => break,
            }
        }
    });
}

async fn run_command_loop(
    app_server_client: Arc<EmbeddedSessionClient>,
    app_event_tx: AppEventSender,
    thread_id: ThreadId,
    pending_server_requests: Arc<Mutex<PendingServerRequests>>,
    active_turn_id: Arc<Mutex<Option<String>>>,
    mut codex_op_rx: tokio::sync::mpsc::UnboundedReceiver<AgentCommand>,
) {
    let thread_id_str = thread_id.to_string();
    while let Some(command) = codex_op_rx.recv().await {
        let result = match command {
            AgentCommand::Interrupt => {
                let turn_id = active_turn_id.lock().await.clone();
                if let Some(turn_id) = turn_id {
                    app_server_client
                        .request::<app_proto::TurnInterruptResponse, _>(|request_id| {
                            app_proto::ClientRequest::TurnInterrupt {
                                request_id,
                                params: app_proto::TurnInterruptParams {
                                    thread_id: thread_id_str.clone(),
                                    turn_id,
                                },
                            }
                        })
                        .await
                        .map(|_| ())
                } else {
                    tracing::warn!("interrupt requested with no active turn id");
                    Ok(())
                }
            }
            AgentCommand::CleanBackgroundTerminals => app_server_client
                .request::<app_proto::ThreadBackgroundTerminalsCleanResponse, _>(|request_id| {
                    app_proto::ClientRequest::ThreadBackgroundTerminalsClean {
                        request_id,
                        params: app_proto::ThreadBackgroundTerminalsCleanParams {
                            thread_id: thread_id_str.clone(),
                        },
                    }
                })
                .await
                .map(|_| ()),
            AgentCommand::UserTurn {
                items,
                cwd,
                approval_policy,
                sandbox_policy,
                model,
                effort,
                summary,
                final_output_json_schema,
                collaboration_mode,
                personality,
            } => app_server_client
                .request::<app_proto::TurnStartResponse, _>(|request_id| {
                    app_proto::ClientRequest::TurnStart {
                        request_id,
                        params: app_proto::TurnStartParams {
                            thread_id: thread_id_str.clone(),
                            input: items.into_iter().map(Into::into).collect(),
                            cwd: Some(cwd),
                            approval_policy: Some(approval_policy.into()),
                            sandbox_policy: Some(sandbox_policy.into()),
                            model: Some(model),
                            effort,
                            summary: Some(summary),
                            personality,
                            output_schema: final_output_json_schema,
                            collaboration_mode,
                        },
                    }
                })
                .await
                .map(|_| ()),
            AgentCommand::OverrideTurnContext {
                cwd,
                approval_policy,
                sandbox_policy,
                windows_sandbox_level,
                model,
                effort,
                summary,
                collaboration_mode,
                personality,
            } => app_server_client
                .request::<app_proto::ThreadContextUpdateResponse, _>(|request_id| {
                    app_proto::ClientRequest::ThreadContextUpdate {
                        request_id,
                        params: app_proto::ThreadContextUpdateParams {
                            thread_id: thread_id_str.clone(),
                            cwd,
                            approval_policy: approval_policy.map(Into::into),
                            sandbox_policy: sandbox_policy.map(Into::into),
                            windows_sandbox_level,
                            model,
                            effort,
                            summary,
                            personality,
                            collaboration_mode,
                        },
                    }
                })
                .await
                .map(|_| ()),
            AgentCommand::ExecApproval {
                id,
                turn_id: _,
                decision,
            } => {
                let Some(decision) = convert_exec_approval_decision(decision) else {
                    continue;
                };
                respond_if_pending(
                    &app_server_client,
                    &pending_server_requests,
                    PendingServerRequestKey::ExecApproval(id),
                    app_proto::CommandExecutionRequestApprovalResponse { decision },
                )
                .await
            }
            AgentCommand::PatchApproval { id, decision } => {
                let Some(decision) = convert_patch_approval_decision(decision) else {
                    continue;
                };
                respond_if_pending(
                    &app_server_client,
                    &pending_server_requests,
                    PendingServerRequestKey::PatchApproval(id),
                    app_proto::FileChangeRequestApprovalResponse { decision },
                )
                .await
            }
            AgentCommand::ResolveElicitation {
                server_name,
                request_id,
                decision,
            } => {
                respond_if_pending(
                    &app_server_client,
                    &pending_server_requests,
                    PendingServerRequestKey::McpElicitation {
                        server_name,
                        request_id,
                    },
                    app_proto::McpElicitationRequestResponse {
                        decision: convert_elicitation_decision(decision),
                    },
                )
                .await
            }
            AgentCommand::UserInputAnswer { id, response } => {
                let Some(request_id) = take_pending_request_id(
                    &pending_server_requests,
                    PendingServerRequestKey::UserInput(id),
                )
                .await
                else {
                    continue;
                };
                match convert_user_input_response(response) {
                    Ok(response) => app_server_client.respond(request_id, response).await,
                    Err(err) => Err(app_proto::JSONRPCErrorError {
                        code: -32603,
                        message: format!("failed to convert request_user_input response: {err}"),
                        data: None,
                    }),
                }
            }
            AgentCommand::DynamicToolResponse { id, response } => {
                let Some(request_id) = take_pending_request_id(
                    &pending_server_requests,
                    PendingServerRequestKey::DynamicTool(id),
                )
                .await
                else {
                    continue;
                };
                match convert_dynamic_tool_response(response) {
                    Ok(response) => app_server_client.respond(request_id, response).await,
                    Err(err) => Err(app_proto::JSONRPCErrorError {
                        code: -32603,
                        message: format!("failed to convert dynamic tool response: {err}"),
                        data: None,
                    }),
                }
            }
            AgentCommand::AddToHistory { text } => app_server_client
                .request::<app_proto::HistoryAppendResponse, _>(|request_id| {
                    app_proto::ClientRequest::HistoryAppend {
                        request_id,
                        params: app_proto::HistoryAppendParams {
                            thread_id: thread_id_str.clone(),
                            text,
                        },
                    }
                })
                .await
                .map(|_| ()),
            AgentCommand::GetHistoryEntryRequest { offset, log_id } => {
                let response = app_server_client
                    .request::<app_proto::HistoryEntryReadResponse, _>(|request_id| {
                        app_proto::ClientRequest::HistoryEntryRead {
                            request_id,
                            params: app_proto::HistoryEntryReadParams { log_id, offset },
                        }
                    })
                    .await;
                response.map(|response| {
                    app_event_tx.send(AppEvent::HistoryEntryReadResponse(response));
                })
            }
            AgentCommand::ListMcpTools => {
                let response = app_server_client
                    .request::<app_proto::McpToolsListResponse, _>(|request_id| {
                        app_proto::ClientRequest::McpToolsList {
                            request_id,
                            params: app_proto::McpToolsListParams {},
                        }
                    })
                    .await;
                response.map(|response| {
                    app_event_tx.send(AppEvent::McpToolsListResponse(response));
                })
            }
            AgentCommand::ReloadUserConfig => app_server_client
                .request::<app_proto::ThreadConfigReloadResponse, _>(|request_id| {
                    app_proto::ClientRequest::ThreadConfigReload {
                        request_id,
                        params: app_proto::ThreadConfigReloadParams {
                            thread_id: thread_id_str.clone(),
                        },
                    }
                })
                .await
                .map(|_| ()),
            AgentCommand::ListCustomPrompts => {
                let response = app_server_client
                    .request::<app_proto::CustomPromptListResponse, _>(|request_id| {
                        app_proto::ClientRequest::CustomPromptList {
                            request_id,
                            params: app_proto::CustomPromptListParams {},
                        }
                    })
                    .await;
                response
                    .map(|response| app_event_tx.send(AppEvent::CustomPromptListResponse(response)))
            }
            AgentCommand::ListSkills { cwds, force_reload } => {
                let response = app_server_client
                    .request::<app_proto::SkillsListResponse, _>(|request_id| {
                        app_proto::ClientRequest::SkillsList {
                            request_id,
                            params: app_proto::SkillsListParams {
                                cwds,
                                force_reload,
                                per_cwd_extra_user_roots: None,
                            },
                        }
                    })
                    .await;
                response.map(|response| app_event_tx.send(AppEvent::SkillsListResponse(response)))
            }
            AgentCommand::Compact => app_server_client
                .request::<app_proto::ThreadCompactStartResponse, _>(|request_id| {
                    app_proto::ClientRequest::ThreadCompactStart {
                        request_id,
                        params: app_proto::ThreadCompactStartParams {
                            thread_id: thread_id_str.clone(),
                        },
                    }
                })
                .await
                .map(|_| ()),
            AgentCommand::DropMemories => app_server_client
                .request::<app_proto::ThreadMemoriesDropResponse, _>(|request_id| {
                    app_proto::ClientRequest::ThreadMemoriesDrop {
                        request_id,
                        params: app_proto::ThreadMemoriesDropParams {
                            thread_id: thread_id_str.clone(),
                        },
                    }
                })
                .await
                .map(|_| ()),
            AgentCommand::UpdateMemories => app_server_client
                .request::<app_proto::ThreadMemoriesUpdateResponse, _>(|request_id| {
                    app_proto::ClientRequest::ThreadMemoriesUpdate {
                        request_id,
                        params: app_proto::ThreadMemoriesUpdateParams {
                            thread_id: thread_id_str.clone(),
                        },
                    }
                })
                .await
                .map(|_| ()),
            AgentCommand::SetThreadName { name } => app_server_client
                .request::<app_proto::ThreadSetNameResponse, _>(|request_id| {
                    app_proto::ClientRequest::ThreadSetName {
                        request_id,
                        params: app_proto::ThreadSetNameParams {
                            thread_id: thread_id_str.clone(),
                            name,
                        },
                    }
                })
                .await
                .map(|_| ()),
            AgentCommand::Undo => app_server_client
                .request::<app_proto::ThreadUndoResponse, _>(|request_id| {
                    app_proto::ClientRequest::ThreadUndo {
                        request_id,
                        params: app_proto::ThreadUndoParams {
                            thread_id: thread_id_str.clone(),
                        },
                    }
                })
                .await
                .map(|_| ()),
            AgentCommand::ThreadRollback { num_turns } => app_server_client
                .request::<app_proto::ThreadRollbackResponse, _>(|request_id| {
                    app_proto::ClientRequest::ThreadRollback {
                        request_id,
                        params: app_proto::ThreadRollbackParams {
                            thread_id: thread_id_str.clone(),
                            num_turns,
                        },
                    }
                })
                .await
                .map(|_| {
                    app_event_tx.send(AppEvent::ThreadRollbackCompleted {
                        thread_id,
                        num_turns,
                    });
                }),
            AgentCommand::Review { review_request } => {
                let target = match convert_review_target(review_request.target) {
                    Ok(target) => target,
                    Err(err) => {
                        tracing::error!("failed to convert review target: {err}");
                        continue;
                    }
                };
                app_server_client
                    .request::<app_proto::ReviewStartResponse, _>(|request_id| {
                        app_proto::ClientRequest::ReviewStart {
                            request_id,
                            params: app_proto::ReviewStartParams {
                                thread_id: thread_id_str.clone(),
                                target,
                                delivery: None,
                            },
                        }
                    })
                    .await
                    .map(|_| ())
            }
            AgentCommand::Shutdown => app_server_client
                .request::<app_proto::ThreadShutdownResponse, _>(|request_id| {
                    app_proto::ClientRequest::ThreadShutdown {
                        request_id,
                        params: app_proto::ThreadShutdownParams {
                            thread_id: thread_id_str.clone(),
                        },
                    }
                })
                .await
                .map(|_| ()),
            AgentCommand::RunUserShellCommand { command } => app_server_client
                .request::<app_proto::ThreadUserShellCommandRunResponse, _>(|request_id| {
                    app_proto::ClientRequest::ThreadUserShellCommandRun {
                        request_id,
                        params: app_proto::ThreadUserShellCommandRunParams {
                            thread_id: thread_id_str.clone(),
                            command,
                        },
                    }
                })
                .await
                .map(|_| ()),
        };

        if let Err(err) = result {
            tracing::error!("failed to handle agent command via app-server: {err:?}");
        }
    }
}

fn sandbox_mode_from_policy(
    policy: &codex_protocol::protocol::SandboxPolicy,
) -> Option<app_proto::SandboxMode> {
    match policy {
        codex_protocol::protocol::SandboxPolicy::ReadOnly { .. } => {
            Some(app_proto::SandboxMode::ReadOnly)
        }
        codex_protocol::protocol::SandboxPolicy::WorkspaceWrite { .. } => {
            Some(app_proto::SandboxMode::WorkspaceWrite)
        }
        codex_protocol::protocol::SandboxPolicy::DangerFullAccess => {
            Some(app_proto::SandboxMode::DangerFullAccess)
        }
        codex_protocol::protocol::SandboxPolicy::ExternalSandbox { .. } => None,
    }
}

fn thread_start_params_from_config(config: &Config) -> app_proto::ThreadStartParams {
    app_proto::ThreadStartParams {
        model: config.model.clone(),
        model_provider: Some(config.model_provider_id.clone()),
        cwd: Some(config.cwd.to_string_lossy().to_string()),
        approval_policy: Some(app_proto::AskForApproval::from(
            *config.permissions.approval_policy.get(),
        )),
        sandbox: sandbox_mode_from_policy(config.permissions.sandbox_policy.get()),
        personality: config.personality,
        ..Default::default()
    }
}

/// Spawn the agent bootstrapper and command forwarding loop.
pub(crate) fn spawn_agent(
    config: Config,
    app_event_tx: AppEventSender,
    app_server_client: Arc<EmbeddedSessionClient>,
) -> UnboundedSender<AgentCommand> {
    let (codex_op_tx, codex_op_rx) = unbounded_channel::<AgentCommand>();

    let app_event_tx_clone = app_event_tx;
    tokio::spawn(async move {
        let response = match app_server_client
            .thread_start(thread_start_params_from_config(&config))
            .await
        {
            Ok(response) => response,
            Err(err) => {
                let message = format!("Failed to initialize codex: {err:?}");
                tracing::error!("{message}");
                app_event_tx_clone.send(AppEvent::FatalExitRequest(message));
                return;
            }
        };

        let bootstrap = match thread_bootstrap_from_start(response) {
            Ok(bootstrap) => bootstrap,
            Err(err) => {
                let message = format!("Failed to initialize codex: {err}");
                tracing::error!("{message}");
                app_event_tx_clone.send(AppEvent::FatalExitRequest(message));
                return;
            }
        };
        let thread_id = bootstrap.thread_id;
        let pending_server_requests = Arc::new(Mutex::new(PendingServerRequests::default()));
        let active_turn_id = Arc::new(Mutex::new(None));
        spawn_server_request_tracker(
            app_server_client.clone(),
            app_event_tx_clone.clone(),
            thread_id,
            pending_server_requests.clone(),
        );
        spawn_active_turn_tracker(app_server_client.clone(), thread_id, active_turn_id.clone());
        tokio::spawn(run_command_loop(
            app_server_client.clone(),
            app_event_tx_clone.clone(),
            thread_id,
            pending_server_requests,
            active_turn_id,
            codex_op_rx,
        ));
        app_event_tx_clone.send(AppEvent::ThreadBootstrapped(bootstrap));
    });

    codex_op_tx
}

/// Spawn agent loops for an existing thread (e.g., a forked thread).
pub(crate) fn spawn_agent_from_existing(
    thread_id: ThreadId,
    app_event_tx: AppEventSender,
    app_server_client: Arc<EmbeddedSessionClient>,
) -> UnboundedSender<AgentCommand> {
    let (codex_op_tx, codex_op_rx) = unbounded_channel::<AgentCommand>();

    let app_event_tx_clone = app_event_tx;
    tokio::spawn(async move {
        let pending_server_requests = Arc::new(Mutex::new(PendingServerRequests::default()));
        let active_turn_id = Arc::new(Mutex::new(None));
        spawn_server_request_tracker(
            app_server_client.clone(),
            app_event_tx_clone.clone(),
            thread_id,
            pending_server_requests.clone(),
        );
        spawn_active_turn_tracker(app_server_client.clone(), thread_id, active_turn_id.clone());
        tokio::spawn(run_command_loop(
            app_server_client,
            app_event_tx_clone.clone(),
            thread_id,
            pending_server_requests,
            active_turn_id,
            codex_op_rx,
        ));
    });

    codex_op_tx
}

/// Spawn a command-forwarding loop for an existing thread without subscribing to events.
pub(crate) fn spawn_op_forwarder(
    app_event_tx: AppEventSender,
    thread_id: ThreadId,
    app_server_client: Arc<EmbeddedSessionClient>,
) -> UnboundedSender<AgentCommand> {
    let (codex_op_tx, codex_op_rx) = unbounded_channel::<AgentCommand>();
    let pending_server_requests = Arc::new(Mutex::new(PendingServerRequests::default()));
    let active_turn_id = Arc::new(Mutex::new(None));
    spawn_server_request_tracker(
        app_server_client.clone(),
        app_event_tx.clone(),
        thread_id,
        pending_server_requests.clone(),
    );
    spawn_active_turn_tracker(app_server_client.clone(), thread_id, active_turn_id.clone());

    tokio::spawn(run_command_loop(
        app_server_client,
        app_event_tx,
        thread_id,
        pending_server_requests,
        active_turn_id,
        codex_op_rx,
    ));

    codex_op_tx
}
