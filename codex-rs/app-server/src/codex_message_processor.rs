use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use crate::fuzzy_file_search::run_fuzzy_file_search;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::OutgoingNotification;
use codex_app_server_protocol::AddConversationListenerParams;
use codex_app_server_protocol::AddConversationSubscriptionResponse;
use codex_app_server_protocol::ApplyPatchApprovalParams;
use codex_app_server_protocol::ApplyPatchApprovalResponse;
use codex_app_server_protocol::ArchiveConversationParams;
use codex_app_server_protocol::ArchiveConversationResponse;
use codex_app_server_protocol::AuthStatusChangeNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConversationSummary;
use codex_app_server_protocol::ExecCommandApprovalParams;
use codex_app_server_protocol::ExecCommandApprovalResponse;
use codex_app_server_protocol::ExecOneOffCommandParams;
use codex_app_server_protocol::ExecOneOffCommandResponse;
use codex_app_server_protocol::FuzzyFileSearchParams;
use codex_app_server_protocol::FuzzyFileSearchResponse;
use codex_app_server_protocol::GetUserAgentResponse;
use codex_app_server_protocol::GetUserSavedConfigResponse;
use codex_app_server_protocol::GitDiffToRemoteResponse;
use codex_app_server_protocol::InputItem as WireInputItem;
use codex_app_server_protocol::InterruptConversationParams;
use codex_app_server_protocol::InterruptConversationResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ListConversationsParams;
use codex_app_server_protocol::ListConversationsResponse;
use codex_app_server_protocol::LoginApiKeyParams;
use codex_app_server_protocol::LoginApiKeyResponse;
use codex_app_server_protocol::LoginChatGptCompleteNotification;
use codex_app_server_protocol::LoginChatGptResponse;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::NewConversationResponse;
use codex_app_server_protocol::RemoveConversationListenerParams;
use codex_app_server_protocol::RemoveConversationSubscriptionResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::Result as JsonRpcResult;
use codex_app_server_protocol::ResumeConversationParams;
use codex_app_server_protocol::SendUserMessageParams;
use codex_app_server_protocol::SendUserMessageResponse;
use codex_app_server_protocol::SendUserTurnParams;
use codex_app_server_protocol::SendUserTurnResponse;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequestPayload;
use codex_app_server_protocol::SessionConfiguredNotification;
use codex_app_server_protocol::SetDefaultModelParams;
use codex_app_server_protocol::SetDefaultModelResponse;
use codex_app_server_protocol::UserInfoResponse;
use codex_app_server_protocol::UserSavedConfig;
use codex_core::AuthManager;
use codex_core::CodexConversation;
use codex_core::ConversationManager;
use codex_core::Cursor as RolloutCursor;
use codex_core::INTERACTIVE_SESSION_SOURCES;
use codex_core::NewConversation;
use codex_core::RolloutRecorder;
use codex_core::SessionMeta;
use codex_core::auth::CLIENT_ID;
use codex_core::auth::get_auth_file;
use codex_core::auth::login_with_api_key;
use codex_core::auth::try_read_auth_json;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::ConfigToml;
use codex_core::config::load_config_as_toml;
use codex_core::config_edit::CONFIG_KEY_EFFORT;
use codex_core::config_edit::CONFIG_KEY_MODEL;
use codex_core::config_edit::persist_overrides_and_clear_if_none;
use codex_core::default_client::get_codex_user_agent;
use codex_core::exec::ExecParams;
use codex_core::exec_env::create_env;
use codex_core::get_platform_sandbox;
use codex_core::git_info::git_diff_to_remote;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::InputItem as CoreInputItem;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use codex_login::ServerOptions as LoginServerOptions;
use codex_login::ShutdownHandle;
use codex_login::run_login_server;
use codex_protocol::ConversationId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::InputMessageKind;
use codex_protocol::protocol::USER_MESSAGE_BEGIN;
use codex_utils_json_to_toml::json_to_toml;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::select;
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tracing::error;
use tracing::info;
use tracing::warn;
use uuid::Uuid;

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
    pending_interrupts: Arc<Mutex<HashMap<ConversationId, Vec<RequestId>>>>,
    pending_fuzzy_searches: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
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
            pending_fuzzy_searches: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn process_request(&mut self, request: ClientRequest) {
        match request {
            ClientRequest::Initialize { .. } => {
                panic!("Initialize should be handled in MessageProcessor");
            }
            ClientRequest::NewConversation { request_id, params } => {
                // Do not tokio::spawn() to process new_conversation()
                // asynchronously because we need to ensure the conversation is
                // created before processing any subsequent messages.
                self.process_new_conversation(request_id, params).await;
            }
            ClientRequest::ListConversations { request_id, params } => {
                self.handle_list_conversations(request_id, params).await;
            }
            ClientRequest::ResumeConversation { request_id, params } => {
                self.handle_resume_conversation(request_id, params).await;
            }
            ClientRequest::ArchiveConversation { request_id, params } => {
                self.archive_conversation(request_id, params).await;
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
            ClientRequest::LoginApiKey { request_id, params } => {
                self.login_api_key(request_id, params).await;
            }
            ClientRequest::LoginChatGpt {
                request_id,
                params: _,
            } => {
                self.login_chatgpt(request_id).await;
            }
            ClientRequest::CancelLoginChatGpt { request_id, params } => {
                self.cancel_login_chatgpt(request_id, params.login_id).await;
            }
            ClientRequest::LogoutChatGpt {
                request_id,
                params: _,
            } => {
                self.logout_chatgpt(request_id).await;
            }
            ClientRequest::GetAuthStatus { request_id, params } => {
                self.get_auth_status(request_id, params).await;
            }
            ClientRequest::GetUserSavedConfig {
                request_id,
                params: _,
            } => {
                self.get_user_saved_config(request_id).await;
            }
            ClientRequest::SetDefaultModel { request_id, params } => {
                self.set_default_model(request_id, params).await;
            }
            ClientRequest::GetUserAgent {
                request_id,
                params: _,
            } => {
                self.get_user_agent(request_id).await;
            }
            ClientRequest::UserInfo {
                request_id,
                params: _,
            } => {
                self.get_user_info(request_id).await;
            }
            ClientRequest::FuzzyFileSearch { request_id, params } => {
                self.fuzzy_file_search(request_id, params).await;
            }
            ClientRequest::ExecOneOffCommand { request_id, params } => {
                self.exec_one_off_command(request_id, params).await;
            }
        }
    }

    async fn login_api_key(&mut self, request_id: RequestId, params: LoginApiKeyParams) {
        {
            let mut guard = self.active_login.lock().await;
            if let Some(active) = guard.take() {
                active.drop();
            }
        }

        match login_with_api_key(&self.config.codex_home, &params.api_key) {
            Ok(()) => {
                self.auth_manager.reload();
                self.outgoing
                    .send_response(request_id, LoginApiKeyResponse {})
                    .await;

                let payload = AuthStatusChangeNotification {
                    auth_method: self.auth_manager.auth().map(|auth| auth.mode),
                };
                self.outgoing
                    .send_server_notification(ServerNotification::AuthStatusChange(payload))
                    .await;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to save api key: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
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
                    codex_app_server_protocol::CancelLoginChatGptResponse {},
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
                codex_app_server_protocol::LogoutChatGptResponse {},
            )
            .await;

        // Send auth status change notification reflecting the current auth mode
        // after logout.
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
        params: codex_app_server_protocol::GetAuthStatusParams,
    ) {
        let include_token = params.include_token.unwrap_or(false);
        let do_refresh = params.refresh_token.unwrap_or(false);

        if do_refresh && let Err(err) = self.auth_manager.refresh_token().await {
            tracing::warn!("failed to refresh token while getting auth status: {err}");
        }

        // Determine whether auth is required based on the active model provider.
        // If a custom provider is configured with `requires_openai_auth == false`,
        // then no auth step is required; otherwise, default to requiring auth.
        let requires_openai_auth = self.config.model_provider.requires_openai_auth;

        let response = if !requires_openai_auth {
            codex_app_server_protocol::GetAuthStatusResponse {
                auth_method: None,
                auth_token: None,
                requires_openai_auth: Some(false),
            }
        } else {
            match self.auth_manager.auth() {
                Some(auth) => {
                    let auth_mode = auth.mode;
                    let (reported_auth_method, token_opt) = match auth.get_token().await {
                        Ok(token) if !token.is_empty() => {
                            let tok = if include_token { Some(token) } else { None };
                            (Some(auth_mode), tok)
                        }
                        Ok(_) => (None, None),
                        Err(err) => {
                            tracing::warn!("failed to get token for auth status: {err}");
                            (None, None)
                        }
                    };
                    codex_app_server_protocol::GetAuthStatusResponse {
                        auth_method: reported_auth_method,
                        auth_token: token_opt,
                        requires_openai_auth: Some(true),
                    }
                }
                None => codex_app_server_protocol::GetAuthStatusResponse {
                    auth_method: None,
                    auth_token: None,
                    requires_openai_auth: Some(true),
                },
            }
        };

        self.outgoing.send_response(request_id, response).await;
    }

    async fn get_user_agent(&self, request_id: RequestId) {
        let user_agent = get_codex_user_agent();
        let response = GetUserAgentResponse { user_agent };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn get_user_saved_config(&self, request_id: RequestId) {
        let toml_value = match load_config_as_toml(&self.config.codex_home) {
            Ok(val) => val,
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to load config.toml: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let cfg: ConfigToml = match toml_value.try_into() {
            Ok(cfg) => cfg,
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to parse config.toml: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let user_saved_config: UserSavedConfig = cfg.into();

        let response = GetUserSavedConfigResponse {
            config: user_saved_config,
        };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn get_user_info(&self, request_id: RequestId) {
        // Read alleged user email from auth.json (best-effort; not verified).
        let auth_path = get_auth_file(&self.config.codex_home);
        let alleged_user_email = match try_read_auth_json(&auth_path) {
            Ok(auth) => auth.tokens.and_then(|t| t.id_token.email),
            Err(_) => None,
        };

        let response = UserInfoResponse { alleged_user_email };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn set_default_model(&self, request_id: RequestId, params: SetDefaultModelParams) {
        let SetDefaultModelParams {
            model,
            reasoning_effort,
        } = params;
        let effort_str = reasoning_effort.map(|effort| effort.to_string());

        let overrides: [(&[&str], Option<&str>); 2] = [
            (&[CONFIG_KEY_MODEL], model.as_deref()),
            (&[CONFIG_KEY_EFFORT], effort_str.as_deref()),
        ];

        match persist_overrides_and_clear_if_none(
            &self.config.codex_home,
            self.config.active_profile.as_deref(),
            &overrides,
        )
        .await
        {
            Ok(()) => {
                let response = SetDefaultModelResponse {};
                self.outgoing.send_response(request_id, response).await;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to persist overrides: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn exec_one_off_command(&self, request_id: RequestId, params: ExecOneOffCommandParams) {
        tracing::debug!("ExecOneOffCommand params: {params:?}");

        if params.command.is_empty() {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: "command must not be empty".to_string(),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        }

        let cwd = params.cwd.unwrap_or_else(|| self.config.cwd.clone());
        let env = create_env(&self.config.shell_environment_policy);
        let timeout_ms = params.timeout_ms;
        let exec_params = ExecParams {
            command: params.command,
            cwd,
            timeout_ms,
            env,
            with_escalated_permissions: None,
            justification: None,
        };

        let effective_policy = params
            .sandbox_policy
            .unwrap_or_else(|| self.config.sandbox_policy.clone());

        let sandbox_type = match &effective_policy {
            codex_core::protocol::SandboxPolicy::DangerFullAccess => {
                codex_core::exec::SandboxType::None
            }
            _ => get_platform_sandbox().unwrap_or(codex_core::exec::SandboxType::None),
        };
        tracing::debug!("Sandbox type: {sandbox_type:?}");
        let codex_linux_sandbox_exe = self.config.codex_linux_sandbox_exe.clone();
        let outgoing = self.outgoing.clone();
        let req_id = request_id;
        let sandbox_cwd = self.config.cwd.clone();

        tokio::spawn(async move {
            match codex_core::exec::process_exec_tool_call(
                exec_params,
                sandbox_type,
                &effective_policy,
                sandbox_cwd.as_path(),
                &codex_linux_sandbox_exe,
                None,
            )
            .await
            {
                Ok(output) => {
                    let response = ExecOneOffCommandResponse {
                        exit_code: output.exit_code,
                        stdout: output.stdout.text,
                        stderr: output.stderr.text,
                    };
                    outgoing.send_response(req_id, response).await;
                }
                Err(err) => {
                    let error = JSONRPCErrorError {
                        code: INTERNAL_ERROR_CODE,
                        message: format!("exec failed: {err}"),
                        data: None,
                    };
                    outgoing.send_error(req_id, error).await;
                }
            }
        });
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
                    conversation_id,
                    model: session_configured.model,
                    reasoning_effort: session_configured.reasoning_effort,
                    rollout_path: session_configured.rollout_path,
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

    async fn handle_list_conversations(
        &self,
        request_id: RequestId,
        params: ListConversationsParams,
    ) {
        let page_size = params.page_size.unwrap_or(25);
        // Decode the optional cursor string to a Cursor via serde (Cursor implements Deserialize from string)
        let cursor_obj: Option<RolloutCursor> = match params.cursor {
            Some(s) => serde_json::from_str::<RolloutCursor>(&format!("\"{s}\"")).ok(),
            None => None,
        };
        let cursor_ref = cursor_obj.as_ref();

        let page = match RolloutRecorder::list_conversations(
            &self.config.codex_home,
            page_size,
            cursor_ref,
            INTERACTIVE_SESSION_SOURCES,
        )
        .await
        {
            Ok(p) => p,
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to list conversations: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let items = page
            .items
            .into_iter()
            .filter_map(|it| extract_conversation_summary(it.path, &it.head))
            .collect();

        // Encode next_cursor as a plain string
        let next_cursor = match page.next_cursor {
            Some(c) => match serde_json::to_value(&c) {
                Ok(serde_json::Value::String(s)) => Some(s),
                _ => None,
            },
            None => None,
        };

        let response = ListConversationsResponse { items, next_cursor };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn handle_resume_conversation(
        &self,
        request_id: RequestId,
        params: ResumeConversationParams,
    ) {
        // Derive a Config using the same logic as new conversation, honoring overrides if provided.
        let config = match params.overrides {
            Some(overrides) => {
                derive_config_from_params(overrides, self.codex_linux_sandbox_exe.clone())
            }
            None => Ok(self.config.as_ref().clone()),
        };
        let config = match config {
            Ok(cfg) => cfg,
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

        match self
            .conversation_manager
            .resume_conversation_from_rollout(
                config,
                params.path.clone(),
                self.auth_manager.clone(),
            )
            .await
        {
            Ok(NewConversation {
                conversation_id,
                session_configured,
                ..
            }) => {
                self.outgoing
                    .send_server_notification(ServerNotification::SessionConfigured(
                        SessionConfiguredNotification {
                            session_id: session_configured.session_id,
                            model: session_configured.model.clone(),
                            reasoning_effort: session_configured.reasoning_effort,
                            history_log_id: session_configured.history_log_id,
                            history_entry_count: session_configured.history_entry_count,
                            initial_messages: session_configured.initial_messages.clone(),
                            rollout_path: session_configured.rollout_path.clone(),
                        },
                    ))
                    .await;
                let initial_messages = session_configured.initial_messages.map(|msgs| {
                    msgs.into_iter()
                        .filter(|event| {
                            // Don't send non-plain user messages (like user instructions
                            // or environment context) back so they don't get rendered.
                            if let EventMsg::UserMessage(user_message) = event {
                                return matches!(user_message.kind, Some(InputMessageKind::Plain));
                            }
                            true
                        })
                        .collect()
                });

                // Reply with conversation id + model and initial messages (when present)
                let response = codex_app_server_protocol::ResumeConversationResponse {
                    conversation_id,
                    model: session_configured.model.clone(),
                    initial_messages,
                };
                self.outgoing.send_response(request_id, response).await;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("error resuming conversation: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn archive_conversation(&self, request_id: RequestId, params: ArchiveConversationParams) {
        let ArchiveConversationParams {
            conversation_id,
            rollout_path,
        } = params;

        // Verify that the rollout path is in the sessions directory or else
        // a malicious client could specify an arbitrary path.
        let rollout_folder = self.config.codex_home.join(codex_core::SESSIONS_SUBDIR);
        let canonical_rollout_path = tokio::fs::canonicalize(&rollout_path).await;
        let canonical_rollout_path = if let Ok(path) = canonical_rollout_path
            && path.starts_with(&rollout_folder)
        {
            path
        } else {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!(
                    "rollout path `{}` must be in sessions directory",
                    rollout_path.display()
                ),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        };

        let required_suffix = format!("{conversation_id}.jsonl");
        let Some(file_name) = canonical_rollout_path.file_name().map(OsStr::to_owned) else {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!(
                    "rollout path `{}` missing file name",
                    rollout_path.display()
                ),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        };

        if !file_name
            .to_string_lossy()
            .ends_with(required_suffix.as_str())
        {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!(
                    "rollout path `{}` does not match conversation id {conversation_id}",
                    rollout_path.display()
                ),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        }

        let removed_conversation = self
            .conversation_manager
            .remove_conversation(&conversation_id)
            .await;
        if let Some(conversation) = removed_conversation {
            info!("conversation {conversation_id} was active; shutting down");
            let conversation_clone = conversation.clone();
            let notify = Arc::new(tokio::sync::Notify::new());
            let notify_clone = notify.clone();

            // Establish the listener for ShutdownComplete before submitting
            // Shutdown so it is not missed.
            let is_shutdown = tokio::spawn(async move {
                loop {
                    select! {
                        _ = notify_clone.notified() => {
                            break;
                        }
                        event = conversation_clone.next_event() => {
                            if let Ok(event) = event && matches!(event.msg, EventMsg::ShutdownComplete) {
                                break;
                            }
                        }
                    }
                }
            });

            // Request shutdown.
            match conversation.submit(Op::Shutdown).await {
                Ok(_) => {
                    // Successfully submitted Shutdown; wait before proceeding.
                    select! {
                        _ = is_shutdown => {
                            // Normal shutdown: proceed with archive.
                        }
                        _ = tokio::time::sleep(Duration::from_secs(10)) => {
                            warn!("conversation {conversation_id} shutdown timed out; proceeding with archive");
                            notify.notify_one();
                        }
                    }
                }
                Err(err) => {
                    error!("failed to submit Shutdown to conversation {conversation_id}: {err}");
                    notify.notify_one();
                    // Perhaps we lost a shutdown race, so let's continue to
                    // clean up the .jsonl file.
                }
            }
        }

        // Move the .jsonl file to the archived sessions subdir.
        let result: std::io::Result<()> = async {
            let archive_folder = self
                .config
                .codex_home
                .join(codex_core::ARCHIVED_SESSIONS_SUBDIR);
            tokio::fs::create_dir_all(&archive_folder).await?;
            tokio::fs::rename(&canonical_rollout_path, &archive_folder.join(&file_name)).await?;
            Ok(())
        }
        .await;

        match result {
            Ok(()) => {
                let response = ArchiveConversationResponse {};
                self.outgoing.send_response(request_id, response).await;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to archive conversation: {err}"),
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
            .get_conversation(conversation_id)
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
            .get_conversation(conversation_id)
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
                final_output_json_schema: None,
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
            .get_conversation(conversation_id)
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
            map.entry(conversation_id).or_default().push(request_id);
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
            .get_conversation(conversation_id)
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
                        // JSON-serializing the `Event` as-is, but these should
                        // be migrated to be variants of `ServerNotification`
                        // instead.
                        let method = format!("codex/event/{}", event.msg);
                        let mut params = match serde_json::to_value(event.clone()) {
                            Ok(serde_json::Value::Object(map)) => map,
                            Ok(_) => {
                                error!("event did not serialize to an object");
                                continue;
                            }
                            Err(err) => {
                                error!("failed to serialize event: {err}");
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

    async fn fuzzy_file_search(&mut self, request_id: RequestId, params: FuzzyFileSearchParams) {
        let FuzzyFileSearchParams {
            query,
            roots,
            cancellation_token,
        } = params;

        let cancel_flag = match cancellation_token.clone() {
            Some(token) => {
                let mut pending_fuzzy_searches = self.pending_fuzzy_searches.lock().await;
                // if a cancellation_token is provided and a pending_request exists for
                // that token, cancel it
                if let Some(existing) = pending_fuzzy_searches.get(&token) {
                    existing.store(true, Ordering::Relaxed);
                }
                let flag = Arc::new(AtomicBool::new(false));
                pending_fuzzy_searches.insert(token.clone(), flag.clone());
                flag
            }
            None => Arc::new(AtomicBool::new(false)),
        };

        let results = match query.as_str() {
            "" => vec![],
            _ => run_fuzzy_file_search(query, roots, cancel_flag.clone()).await,
        };

        if let Some(token) = cancellation_token {
            let mut pending_fuzzy_searches = self.pending_fuzzy_searches.lock().await;
            if let Some(current_flag) = pending_fuzzy_searches.get(&token)
                && Arc::ptr_eq(current_flag, &cancel_flag)
            {
                pending_fuzzy_searches.remove(&token);
            }
        }

        let response = FuzzyFileSearchResponse { files: results };
        self.outgoing.send_response(request_id, response).await;
    }
}

async fn apply_bespoke_event_handling(
    event: Event,
    conversation_id: ConversationId,
    conversation: Arc<CodexConversation>,
    outgoing: Arc<OutgoingMessageSender>,
    pending_interrupts: Arc<Mutex<HashMap<ConversationId, Vec<RequestId>>>>,
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
            let rx = outgoing
                .send_request(ServerRequestPayload::ApplyPatchApproval(params))
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
            let rx = outgoing
                .send_request(ServerRequestPayload::ExecCommandApproval(params))
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
                map.remove(&conversation_id).unwrap_or_default()
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
        review_model: None,
        config_profile: profile,
        cwd: cwd.map(PathBuf::from),
        approval_policy,
        sandbox_mode,
        model_provider: None,
        codex_linux_sandbox_exe,
        base_instructions,
        include_plan_tool,
        include_apply_patch_tool,
        include_view_image_tool: None,
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
    receiver: oneshot::Receiver<JsonRpcResult>,
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
    receiver: oneshot::Receiver<JsonRpcResult>,
    conversation: Arc<CodexConversation>,
) {
    let response = receiver.await;
    let value = match response {
        Ok(value) => value,
        Err(err) => {
            error!("request failed: {err:?}");
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

fn extract_conversation_summary(
    path: PathBuf,
    head: &[serde_json::Value],
) -> Option<ConversationSummary> {
    let session_meta = match head.first() {
        Some(first_line) => serde_json::from_value::<SessionMeta>(first_line.clone()).ok()?,
        None => return None,
    };

    let preview = head
        .iter()
        .filter_map(|value| serde_json::from_value::<ResponseItem>(value.clone()).ok())
        .find_map(|item| match item {
            ResponseItem::Message { content, .. } => {
                content.into_iter().find_map(|content| match content {
                    ContentItem::InputText { text } => {
                        match InputMessageKind::from(("user", &text)) {
                            InputMessageKind::Plain => Some(text),
                            _ => None,
                        }
                    }
                    _ => None,
                })
            }
            _ => None,
        })?;

    let preview = match preview.find(USER_MESSAGE_BEGIN) {
        Some(idx) => preview[idx + USER_MESSAGE_BEGIN.len()..].trim(),
        None => preview.as_str(),
    };

    let timestamp = if session_meta.timestamp.is_empty() {
        None
    } else {
        Some(session_meta.timestamp.clone())
    };

    Some(ConversationSummary {
        conversation_id: session_meta.id,
        timestamp,
        path,
        preview: preview.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn extract_conversation_summary_prefers_plain_user_messages() -> Result<()> {
        let conversation_id = ConversationId::from_string("3f941c35-29b3-493b-b0a4-e25800d9aeb0")?;
        let timestamp = Some("2025-09-05T16:53:11.850Z".to_string());
        let path = PathBuf::from("rollout.jsonl");

        let head = vec![
            json!({
                "id": conversation_id.to_string(),
                "timestamp": timestamp,
                "cwd": "/",
                "originator": "codex",
                "cli_version": "0.0.0",
                "instructions": null
            }),
            json!({
                "type": "message",
                "role": "user",
                "content": [{
                    "type": "input_text",
                    "text": "<user_instructions>\n<AGENTS.md contents>\n</user_instructions>".to_string(),
                }],
            }),
            json!({
                "type": "message",
                "role": "user",
                "content": [{
                    "type": "input_text",
                    "text": format!("<prior context> {USER_MESSAGE_BEGIN}Count to 5"),
                }],
            }),
        ];

        let summary = extract_conversation_summary(path.clone(), &head).expect("summary");

        assert_eq!(summary.conversation_id, conversation_id);
        assert_eq!(
            summary.timestamp,
            Some("2025-09-05T16:53:11.850Z".to_string())
        );
        assert_eq!(summary.path, path);
        assert_eq!(summary.preview, "Count to 5");
        Ok(())
    }
}
