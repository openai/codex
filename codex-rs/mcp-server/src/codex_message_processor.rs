use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use codex_core::CodexConversation;
use codex_core::ConversationManager;
use codex_core::NewConversation;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::git_info::git_diff_to_remote;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::ReviewDecision;
use codex_login::AuthManager;
use codex_protocol::mcp_protocol::AuthMode;
use codex_protocol::mcp_protocol::GitDiffToRemoteResponse;
use mcp_types::JSONRPCErrorError;
use mcp_types::RequestId;
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tracing::error;
use uuid::Uuid;

use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use crate::json_to_toml::json_to_toml;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::OutgoingNotification;
use codex_core::protocol::InputItem as CoreInputItem;
use codex_core::protocol::Op;
use codex_login::CLIENT_ID;
use codex_login::ServerOptions as LoginServerOptions;
use codex_login::ShutdownHandle;
use codex_login::run_login_server;
use codex_protocol::mcp_protocol::APPLY_PATCH_APPROVAL_METHOD;
use codex_protocol::mcp_protocol::AddConversationListenerParams;
use codex_protocol::mcp_protocol::AddConversationSubscriptionResponse;
use codex_protocol::mcp_protocol::ApplyPatchApprovalParams;
use codex_protocol::mcp_protocol::ApplyPatchApprovalResponse;
use codex_protocol::mcp_protocol::AuthStatusChangeNotification;
use codex_protocol::mcp_protocol::ClientRequest;
use codex_protocol::mcp_protocol::ConversationId;
use codex_protocol::mcp_protocol::EXEC_COMMAND_APPROVAL_METHOD;
use codex_protocol::mcp_protocol::ExecCommandApprovalParams;
use codex_protocol::mcp_protocol::ExecCommandApprovalResponse;
use codex_protocol::mcp_protocol::InputItem as WireInputItem;
use codex_protocol::mcp_protocol::InterruptConversationParams;
use codex_protocol::mcp_protocol::InterruptConversationResponse;
use codex_protocol::mcp_protocol::LoginChatGptCompleteNotification;
use codex_protocol::mcp_protocol::LoginChatGptResponse;
use codex_protocol::mcp_protocol::NewConversationParams;
use codex_protocol::mcp_protocol::NewConversationResponse;
use codex_protocol::mcp_protocol::RemoveConversationListenerParams;
use codex_protocol::mcp_protocol::RemoveConversationSubscriptionResponse;
use codex_protocol::mcp_protocol::SendUserMessageParams;
use codex_protocol::mcp_protocol::SendUserMessageResponse;
use codex_protocol::mcp_protocol::SendUserTurnParams;
use codex_protocol::mcp_protocol::SendUserTurnResponse;
use codex_protocol::mcp_protocol::ServerNotification;

// Duration before a ChatGPT login attempt is abandoned.
const LOGIN_CHATGPT_TIMEOUT: Duration = Duration::from_secs(10 * 60);

struct ActiveLogin {
    shutdown_handle: ShutdownHandle,
    login_id: Uuid,
}

impl ActiveLogin {
    fn drop(&self) {
        self.shutdown_handle.shutdown();
    }
}

/// Handles JSON-RPC messages for Codex conversations.
pub(crate) struct CodexMessageProcessor {
    auth_manager: Arc<AuthManager>,
    conversation_manager: Arc<ConversationManager>,
    outgoing: Arc<OutgoingMessageSender>,
    codex_linux_sandbox_exe: Option<PathBuf>,
    config: Arc<Config>,
    conversation_listeners: HashMap<Uuid, oneshot::Sender<()>>,
    active_login: Arc<Mutex<Option<ActiveLogin>>>,
    // Queue of pending interrupt requests per conversation. We reply when TurnAborted arrives.
    pending_interrupts: Arc<Mutex<HashMap<Uuid, Vec<RequestId>>>>,
}

impl CodexMessageProcessor {
    pub fn new(
        auth_manager: Arc<AuthManager>,
        conversation_manager: Arc<ConversationManager>,
        outgoing: Arc<OutgoingMessageSender>,
        codex_linux_sandbox_exe: Option<PathBuf>,
        config: Arc<Config>,
    ) -> Self {
        Self {
            auth_manager,
            conversation_manager,
            outgoing,
            codex_linux_sandbox_exe,
            config,
            conversation_listeners: HashMap::new(),
            active_login: Arc::new(Mutex::new(None)),
            pending_interrupts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn process_request(&mut self, request: ClientRequest) {
        match request {
            ClientRequest::NewConversation { request_id, params } => {
                // Do not tokio::spawn() to process new_conversation()
                // asynchronously because we need to ensure the conversation is
                // created before processing any subsequent messages.
                self.process_new_conversation(request_id, params).await;
            }
            ClientRequest::SendUserMessage { request_id, params } => {
                self.send_user_message(request_id, params).await;
            }
            ClientRequest::SendUserTurn { request_id, params } => {
                self.send_user_turn(request_id, params).await;
            }
            ClientRequest::InterruptConversation { request_id, params } => {
                self.interrupt_conversation(request_id, params).await;
            }
            ClientRequest::AddConversationListener { request_id, params } => {
                self.add_conversation_listener(request_id, params).await;
            }
            ClientRequest::RemoveConversationListener { request_id, params } => {
                self.remove_conversation_listener(request_id, params).await;
            }
            ClientRequest::GitDiffToRemote { request_id, params } => {
                self.git_diff_to_origin(request_id, params.cwd).await;
            }
            ClientRequest::LoginChatGpt { request_id } => {
                self.login_chatgpt(request_id).await;
            }
            ClientRequest::CancelLoginChatGpt { request_id, params } => {
                self.cancel_login_chatgpt(request_id, params.login_id).await;
            }
            ClientRequest::LogoutChatGpt { request_id } => {
                self.logout_chatgpt(request_id).await;
            }
            ClientRequest::GetAuthStatus { request_id, params } => {
                self.get_auth_status(request_id, params).await;
            }
        }
    }

    async fn login_chatgpt(&mut self, request_id: RequestId) {
        let config = self.config.as_ref();

        let opts = LoginServerOptions {
            open_browser: false,
            ..LoginServerOptions::new(config.codex_home.clone(), CLIENT_ID.to_string())
        };

        enum LoginChatGptReply {
            Response(LoginChatGptResponse),
            Error(JSONRPCErrorError),
        }

        let reply = match run_login_server(opts) {
            Ok(server) => {
                let login_id = Uuid::new_v4();
                let shutdown_handle = server.cancel_handle();

                // Replace active login if present.
                {
                    let mut guard = self.active_login.lock().await;
                    if let Some(existing) = guard.take() {
                        existing.drop();
                    }
                    *guard = Some(ActiveLogin {
                        shutdown_handle: shutdown_handle.clone(),
                        login_id,
                    });
                }

                let response = LoginChatGptResponse {
                    login_id,
                    auth_url: server.auth_url.clone(),
                };

                // Spawn background task to monitor completion.
                let outgoing_clone = self.outgoing.clone();
                let active_login = self.active_login.clone();
                let auth_manager = self.auth_manager.clone();
                tokio::spawn(async move {
                    let (success, error_msg) = match tokio::time::timeout(
                        LOGIN_CHATGPT_TIMEOUT,
                        server.block_until_done(),
                    )
                    .await
                    {
                        Ok(Ok(())) => (true, None),
                        Ok(Err(err)) => (false, Some(format!("Login server error: {err}"))),
                        Err(_elapsed) => {
                            // Timeout: cancel server and report
                            shutdown_handle.shutdown();
                            (false, Some("Login timed out".to_string()))
                        }
                    };
                    let payload = LoginChatGptCompleteNotification {
                        login_id,
                        success,
                        error: error_msg,
                    };
                    outgoing_clone
                        .send_server_notification(ServerNotification::LoginChatGptComplete(payload))
                        .await;

                    // Send an auth status change notification.
                    if success {
                        // Update in-memory auth cache now that login completed.
                        auth_manager.reload();

                        // Notify clients with the actual current auth mode.
                        let current_auth_method = auth_manager.auth().map(|a| a.mode);
                        let payload = AuthStatusChangeNotification {
                            auth_method: current_auth_method,
                        };
                        outgoing_clone
                            .send_server_notification(ServerNotification::AuthStatusChange(payload))
                            .await;
                    }

                    // Clear the active login if it matches this attempt. It may have been replaced or cancelled.
                    let mut guard = active_login.lock().await;
                    if guard.as_ref().map(|l| l.login_id) == Some(login_id) {
                        *guard = None;
                    }
                });

                LoginChatGptReply::Response(response)
            }
            Err(err) => LoginChatGptReply::Error(JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to start login server: {err}"),
                data: None,
            }),
        };

        match reply {
            LoginChatGptReply::Response(resp) => {
                self.outgoing.send_response(request_id, resp).await
            }
            LoginChatGptReply::Error(err) => self.outgoing.send_error(request_id, err).await,
        }
    }

    async fn cancel_login_chatgpt(&mut self, request_id: RequestId, login_id: Uuid) {
        let mut guard = self.active_login.lock().await;
        if guard.as_ref().map(|l| l.login_id) == Some(login_id) {
            if let Some(active) = guard.take() {
                active.drop();
            }
            drop(guard);
            self.outgoing
                .send_response(
                    request_id,
                    codex_protocol::mcp_protocol::CancelLoginChatGptResponse {},
                )
                .await;
        } else {
            drop(guard);
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("login id not found: {login_id}"),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
        }
    }

    async fn logout_chatgpt(&mut self, request_id: RequestId) {
        {
            // Cancel any active login attempt.
            let mut guard = self.active_login.lock().await;
            if let Some(active) = guard.take() {
                active.drop();
            }
        }

        if let Err(err) = self.auth_manager.logout() {
            let error = JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("logout failed: {err}"),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        }

        self.outgoing
            .send_response(
                request_id,
                codex_protocol::mcp_protocol::LogoutChatGptResponse {},
            )
            .await;

        // Send auth status change notification reflecting the current auth mode
        // after logout (which may fall back to API key via env var).
        let current_auth_method = self.auth_manager.auth().map(|auth| auth.mode);
        let payload = AuthStatusChangeNotification {
            auth_method: current_auth_method,
        };
        self.outgoing
            .send_server_notification(ServerNotification::AuthStatusChange(payload))
            .await;
    }

    async fn get_auth_status(
        &self,
        request_id: RequestId,
        params: codex_protocol::mcp_protocol::GetAuthStatusParams,
    ) {
        let preferred_auth_method: AuthMode = self.auth_manager.preferred_auth_method();
        let include_token = params.include_token.unwrap_or(false);
        let do_refresh = params.refresh_token.unwrap_or(false);

        if do_refresh && let Err(err) = self.auth_manager.refresh_token().await {
            tracing::warn!("failed to refresh token while getting auth status: {err}");
        }

        let response = match self.auth_manager.auth() {
            Some(auth) => {
                let (reported_auth_method, token_opt) = match auth.get_token().await {
                    Ok(token) if !token.is_empty() => {
                        let tok = if include_token { Some(token) } else { None };
                        (Some(auth.mode), tok)
                    }
                    Ok(_) => (None, None),
                    Err(err) => {
                        tracing::warn!("failed to get token for auth status: {err}");
                        (None, None)
                    }
                };
                codex_protocol::mcp_protocol::GetAuthStatusResponse {
                    auth_method: reported_auth_method,
                    preferred_auth_method,
                    auth_token: token_opt,
                }
            }
            None => codex_protocol::mcp_protocol::GetAuthStatusResponse {
                auth_method: None,
                preferred_auth_method,
                auth_token: None,
            },
        };

        self.outgoing.send_response(request_id, response).await;
    }

    async fn process_new_conversation(&self, request_id: RequestId, params: NewConversationParams) {
        let config = match derive_config_from_params(params, self.codex_linux_sandbox_exe.clone()) {
            Ok(config) => config,
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("error deriving config: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        match self.conversation_manager.new_conversation(config).await {
            Ok(conversation_id) => {
                let NewConversation {
                    conversation_id,
                    session_configured,
                    ..
                } = conversation_id;
                let response = NewConversationResponse {
                    conversation_id: ConversationId(conversation_id),
                    model: session_configured.model,
                };
                self.outgoing.send_response(request_id, response).await;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("error creating conversation: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn send_user_message(&self, request_id: RequestId, params: SendUserMessageParams) {
        let SendUserMessageParams {
            conversation_id,
            items,
        } = params;
        let Ok(conversation) = self
            .conversation_manager
            .get_conversation(conversation_id.0)
            .await
        else {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("conversation not found: {conversation_id}"),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        };

        let mapped_items: Vec<CoreInputItem> = items
            .into_iter()
            .map(|item| match item {
                WireInputItem::Text { text } => CoreInputItem::Text { text },
                WireInputItem::Image { image_url } => CoreInputItem::Image { image_url },
                WireInputItem::LocalImage { path } => CoreInputItem::LocalImage { path },
            })
            .collect();

        // Submit user input to the conversation.
        let _ = conversation
            .submit(Op::UserInput {
                items: mapped_items,
            })
            .await;

        // Acknowledge with an empty result.
        self.outgoing
            .send_response(request_id, SendUserMessageResponse {})
            .await;
    }

    async fn send_user_turn(&self, request_id: RequestId, params: SendUserTurnParams) {
        let SendUserTurnParams {
            conversation_id,
            items,
            cwd,
            approval_policy,
            sandbox_policy,
            model,
            effort,
            summary,
        } = params;

        let Ok(conversation) = self
            .conversation_manager
            .get_conversation(conversation_id.0)
            .await
        else {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("conversation not found: {conversation_id}"),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        };

        let mapped_items: Vec<CoreInputItem> = items
            .into_iter()
            .map(|item| match item {
                WireInputItem::Text { text } => CoreInputItem::Text { text },
                WireInputItem::Image { image_url } => CoreInputItem::Image { image_url },
                WireInputItem::LocalImage { path } => CoreInputItem::LocalImage { path },
            })
            .collect();

        let _ = conversation
            .submit(Op::UserTurn {
                items: mapped_items,
                cwd,
                approval_policy,
                sandbox_policy,
                model,
                effort,
                summary,
            })
            .await;

        self.outgoing
            .send_response(request_id, SendUserTurnResponse {})
            .await;
    }

    async fn interrupt_conversation(
        &mut self,
        request_id: RequestId,
        params: InterruptConversationParams,
    ) {
        let InterruptConversationParams { conversation_id } = params;
        let Ok(conversation) = self
            .conversation_manager
            .get_conversation(conversation_id.0)
            .await
        else {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("conversation not found: {conversation_id}"),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        };

        // Record the pending interrupt so we can reply when TurnAborted arrives.
        {
            let mut map = self.pending_interrupts.lock().await;
            map.entry(conversation_id.0).or_default().push(request_id);
        }

        // Submit the interrupt; we'll respond upon TurnAborted.
        let _ = conversation.submit(Op::Interrupt).await;
    }

    async fn add_conversation_listener(
        &mut self,
        request_id: RequestId,
        params: AddConversationListenerParams,
    ) {
        let AddConversationListenerParams { conversation_id } = params;
        let Ok(conversation) = self
            .conversation_manager
            .get_conversation(conversation_id.0)
            .await
        else {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("conversation not found: {}", conversation_id.0),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        };

        let subscription_id = Uuid::new_v4();
        let (cancel_tx, mut cancel_rx) = oneshot::channel();
        self.conversation_listeners
            .insert(subscription_id, cancel_tx);
        let outgoing_for_task = self.outgoing.clone();
        let pending_interrupts = self.pending_interrupts.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut cancel_rx => {
                        // User has unsubscribed, so exit this task.
                        break;
                    }
                    event = conversation.next_event() => {
                        let event = match event {
                            Ok(event) => event,
                            Err(err) => {
                                tracing::warn!("conversation.next_event() failed with: {err}");
                                break;
                            }
                        };

                        // For now, we send a notification for every event,
                        // JSON-serializing the `Event` as-is, but we will move
                        // to creating a special enum for notifications with a
                        // stable wire format.
                        let method = format!("codex/event/{}", event.msg);
                        let mut params = match serde_json::to_value(event.clone()) {
                            Ok(serde_json::Value::Object(map)) => map,
                            Ok(_) => {
                                tracing::error!("event did not serialize to an object");
                                continue;
                            }
                            Err(err) => {
                                tracing::error!("failed to serialize event: {err}");
                                continue;
                            }
                        };
                        params.insert("conversationId".to_string(), conversation_id.to_string().into());

                        outgoing_for_task.send_notification(OutgoingNotification {
                            method,
                            params: Some(params.into()),
                        })
                        .await;

                        apply_bespoke_event_handling(event.clone(), conversation_id, conversation.clone(), outgoing_for_task.clone(), pending_interrupts.clone()).await;
                    }
                }
            }
        });
        let response = AddConversationSubscriptionResponse { subscription_id };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn remove_conversation_listener(
        &mut self,
        request_id: RequestId,
        params: RemoveConversationListenerParams,
    ) {
        let RemoveConversationListenerParams { subscription_id } = params;
        match self.conversation_listeners.remove(&subscription_id) {
            Some(sender) => {
                // Signal the spawned task to exit and acknowledge.
                let _ = sender.send(());
                let response = RemoveConversationSubscriptionResponse {};
                self.outgoing.send_response(request_id, response).await;
            }
            None => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("subscription not found: {subscription_id}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn git_diff_to_origin(&self, request_id: RequestId, cwd: PathBuf) {
        let diff = git_diff_to_remote(&cwd).await;
        match diff {
            Some(value) => {
                let response = GitDiffToRemoteResponse {
                    sha: value.sha,
                    diff: value.diff,
                };
                self.outgoing.send_response(request_id, response).await;
            }
            None => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("failed to compute git diff to remote for cwd: {cwd:?}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }
}

async fn apply_bespoke_event_handling(
    event: Event,
    conversation_id: ConversationId,
    conversation: Arc<CodexConversation>,
    outgoing: Arc<OutgoingMessageSender>,
    pending_interrupts: Arc<Mutex<HashMap<Uuid, Vec<RequestId>>>>,
) {
    let Event { id: event_id, msg } = event;
    match msg {
        EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
            call_id,
            changes,
            reason,
            grant_root,
        }) => {
            let params = ApplyPatchApprovalParams {
                conversation_id,
                call_id,
                file_changes: changes,
                reason,
                grant_root,
            };
            let value = serde_json::to_value(&params).unwrap_or_default();
            let rx = outgoing
                .send_request(APPLY_PATCH_APPROVAL_METHOD, Some(value))
                .await;
            // TODO(mbolin): Enforce a timeout so this task does not live indefinitely?
            tokio::spawn(async move {
                on_patch_approval_response(event_id, rx, conversation).await;
            });
        }
        EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
            call_id,
            command,
            cwd,
            reason,
        }) => {
            let params = ExecCommandApprovalParams {
                conversation_id,
                call_id,
                command,
                cwd,
                reason,
            };
            let value = serde_json::to_value(&params).unwrap_or_default();
            let rx = outgoing
                .send_request(EXEC_COMMAND_APPROVAL_METHOD, Some(value))
                .await;

            // TODO(mbolin): Enforce a timeout so this task does not live indefinitely?
            tokio::spawn(async move {
                on_exec_approval_response(event_id, rx, conversation).await;
            });
        }
        // If this is a TurnAborted, reply to any pending interrupt requests.
        EventMsg::TurnAborted(turn_aborted_event) => {
            let pending = {
                let mut map = pending_interrupts.lock().await;
                map.remove(&conversation_id.0).unwrap_or_default()
            };
            if !pending.is_empty() {
                let response = InterruptConversationResponse {
                    abort_reason: turn_aborted_event.reason,
                };
                for rid in pending {
                    outgoing.send_response(rid, response.clone()).await;
                }
            }
        }

        _ => {}
    }
}

fn derive_config_from_params(
    params: NewConversationParams,
    codex_linux_sandbox_exe: Option<PathBuf>,
) -> std::io::Result<Config> {
    let NewConversationParams {
        model,
        profile,
        cwd,
        approval_policy,
        sandbox: sandbox_mode,
        config: cli_overrides,
        base_instructions,
        include_plan_tool,
        include_apply_patch_tool,
    } = params;
    let overrides = ConfigOverrides {
        model,
        config_profile: profile,
        cwd: cwd.map(PathBuf::from),
        approval_policy,
        sandbox_mode,
        model_provider: None,
        codex_linux_sandbox_exe,
        base_instructions,
        include_plan_tool,
        include_apply_patch_tool,
        disable_response_storage: None,
        show_raw_agent_reasoning: None,
        tools_web_search_request: None,
    };

    let cli_overrides = cli_overrides
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| (k, json_to_toml(v)))
        .collect();

    Config::load_with_cli_overrides(cli_overrides, overrides)
}

async fn on_patch_approval_response(
    event_id: String,
    receiver: tokio::sync::oneshot::Receiver<mcp_types::Result>,
    codex: Arc<CodexConversation>,
) {
    let response = receiver.await;
    let value = match response {
        Ok(value) => value,
        Err(err) => {
            error!("request failed: {err:?}");
            if let Err(submit_err) = codex
                .submit(Op::PatchApproval {
                    id: event_id.clone(),
                    decision: ReviewDecision::Denied,
                })
                .await
            {
                error!("failed to submit denied PatchApproval after request failure: {submit_err}");
            }
            return;
        }
    };

    let response =
        serde_json::from_value::<ApplyPatchApprovalResponse>(value).unwrap_or_else(|err| {
            error!("failed to deserialize ApplyPatchApprovalResponse: {err}");
            ApplyPatchApprovalResponse {
                decision: ReviewDecision::Denied,
            }
        });

    if let Err(err) = codex
        .submit(Op::PatchApproval {
            id: event_id,
            decision: response.decision,
        })
        .await
    {
        error!("failed to submit PatchApproval: {err}");
    }
}

async fn on_exec_approval_response(
    event_id: String,
    receiver: tokio::sync::oneshot::Receiver<mcp_types::Result>,
    conversation: Arc<CodexConversation>,
) {
    let response = receiver.await;
    let value = match response {
        Ok(value) => value,
        Err(err) => {
            tracing::error!("request failed: {err:?}");
            return;
        }
    };

    // Try to deserialize `value` and then make the appropriate call to `codex`.
    let response =
        serde_json::from_value::<ExecCommandApprovalResponse>(value).unwrap_or_else(|err| {
            error!("failed to deserialize ExecCommandApprovalResponse: {err}");
            // If we cannot deserialize the response, we deny the request to be
            // conservative.
            ExecCommandApprovalResponse {
                decision: ReviewDecision::Denied,
            }
        });

    if let Err(err) = conversation
        .submit(Op::ExecApproval {
            id: event_id,
            decision: response.decision,
        })
        .await
    {
        error!("failed to submit ExecApproval: {err}");
    }
}
