use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use crate::fuzzy_file_search::run_fuzzy_file_search;
use crate::models::supported_models;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::OutgoingNotification;
use codex_app_server_protocol::AccountLoginCompletedNotification;
use codex_app_server_protocol::AccountRateLimitsUpdatedNotification;
use codex_app_server_protocol::AccountUpdatedNotification;
use codex_app_server_protocol::AddConversationListenerParams;
use codex_app_server_protocol::AddConversationSubscriptionResponse;
use codex_app_server_protocol::ApplyPatchApprovalParams;
use codex_app_server_protocol::ApplyPatchApprovalResponse;
use codex_app_server_protocol::ArchiveConversationParams;
use codex_app_server_protocol::ArchiveConversationResponse;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::AuthStatusChangeNotification;
use codex_app_server_protocol::CancelLoginAccountParams;
use codex_app_server_protocol::CancelLoginAccountResponse;
use codex_app_server_protocol::CancelLoginChatGptResponse;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConversationSummary;
use codex_app_server_protocol::ExecCommandApprovalParams;
use codex_app_server_protocol::ExecCommandApprovalResponse;
use codex_app_server_protocol::ExecOneOffCommandParams;
use codex_app_server_protocol::ExecOneOffCommandResponse;
use codex_app_server_protocol::FeedbackUploadParams;
use codex_app_server_protocol::FeedbackUploadResponse;
use codex_app_server_protocol::FuzzyFileSearchParams;
use codex_app_server_protocol::FuzzyFileSearchResponse;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::GetAuthStatusParams;
use codex_app_server_protocol::GetAuthStatusResponse;
use codex_app_server_protocol::GetConversationSummaryParams;
use codex_app_server_protocol::GetConversationSummaryResponse;
use codex_app_server_protocol::GetUserAgentResponse;
use codex_app_server_protocol::GetUserSavedConfigResponse;
use codex_app_server_protocol::GitDiffToRemoteResponse;
use codex_app_server_protocol::InputItem as WireInputItem;
use codex_app_server_protocol::InterruptConversationParams;
use codex_app_server_protocol::InterruptConversationResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ListConversationsParams;
use codex_app_server_protocol::ListConversationsResponse;
use codex_app_server_protocol::LoginAccountParams;
use codex_app_server_protocol::LoginApiKeyParams;
use codex_app_server_protocol::LoginApiKeyResponse;
use codex_app_server_protocol::LoginChatGptCompleteNotification;
use codex_app_server_protocol::LoginChatGptResponse;
use codex_app_server_protocol::LogoutAccountResponse;
use codex_app_server_protocol::LogoutChatGptResponse;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelListResponse;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::NewConversationResponse;
use codex_app_server_protocol::RemoveConversationListenerParams;
use codex_app_server_protocol::RemoveConversationSubscriptionResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::Result as JsonRpcResult;
use codex_app_server_protocol::ResumeConversationParams;
use codex_app_server_protocol::ResumeConversationResponse;
use codex_app_server_protocol::SandboxMode;
use codex_app_server_protocol::SendUserMessageParams;
use codex_app_server_protocol::SendUserMessageResponse;
use codex_app_server_protocol::SendUserTurnParams;
use codex_app_server_protocol::SendUserTurnResponse;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequestPayload;
use codex_app_server_protocol::SessionConfiguredNotification;
use codex_app_server_protocol::SetDefaultModelParams;
use codex_app_server_protocol::SetDefaultModelResponse;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStartedNotification;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStartedNotification;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInfoResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_app_server_protocol::UserSavedConfig;
use codex_backend_client::Client as BackendClient;
use codex_core::AuthManager;
use codex_core::CodexConversation;
use codex_core::ConversationManager;
use codex_core::Cursor as RolloutCursor;
use codex_core::INTERACTIVE_SESSION_SOURCES;
use codex_core::InitialHistory;
use codex_core::NewConversation;
use codex_core::RolloutRecorder;
use codex_core::SessionMeta;
use codex_core::auth::CLIENT_ID;
use codex_core::auth::login_with_api_key;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::ConfigToml;
use codex_core::config::edit::ConfigEditsBuilder;
use codex_core::config_loader::load_config_as_toml;
use codex_core::default_client::get_codex_user_agent;
use codex_core::exec::ExecParams;
use codex_core::exec_env::create_env;
use codex_core::find_conversation_path_by_id_str;
use codex_core::get_platform_sandbox;
use codex_core::git_info::git_diff_to_remote;
use codex_core::parse_cursor;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use codex_core::read_head_for_summary;
use codex_feedback::CodexFeedback;
use codex_login::ServerOptions as LoginServerOptions;
use codex_login::ShutdownHandle;
use codex_login::run_login_server;
use codex_protocol::ConversationId;
use codex_protocol::config_types::ForcedLoginMethod;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RateLimitSnapshot as CoreRateLimitSnapshot;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::USER_MESSAGE_BEGIN;
use codex_protocol::user_input::UserInput as CoreInputItem;
use codex_utils_json_to_toml::json_to_toml;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::Error as IoError;
use std::path::Path;
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

type PendingInterruptQueue = Vec<(RequestId, ApiVersion)>;
type PendingInterrupts = Arc<Mutex<HashMap<ConversationId, PendingInterruptQueue>>>;

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
    pending_interrupts: PendingInterrupts,
    pending_fuzzy_searches: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    feedback: CodexFeedback,
}

#[derive(Clone, Copy, Debug)]
enum ApiVersion {
    V1,
    V2,
}

impl CodexMessageProcessor {
    pub fn new(
        auth_manager: Arc<AuthManager>,
        conversation_manager: Arc<ConversationManager>,
        outgoing: Arc<OutgoingMessageSender>,
        codex_linux_sandbox_exe: Option<PathBuf>,
        config: Arc<Config>,
        feedback: CodexFeedback,
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
            feedback,
        }
    }

    pub async fn process_request(&mut self, request: ClientRequest) {
        match request {
            ClientRequest::Initialize { .. } => {
                panic!("Initialize should be handled in MessageProcessor");
            }
            // === v2 Thread/Turn APIs ===
            ClientRequest::ThreadStart { request_id, params } => {
                self.thread_start(request_id, params).await;
            }
            ClientRequest::ThreadResume { request_id, params } => {
                self.thread_resume(request_id, params).await;
            }
            ClientRequest::ThreadArchive { request_id, params } => {
                self.thread_archive(request_id, params).await;
            }
            ClientRequest::ThreadList { request_id, params } => {
                self.thread_list(request_id, params).await;
            }
            ClientRequest::ThreadCompact {
                request_id,
                params: _,
            } => {
                self.send_unimplemented_error(request_id, "thread/compact")
                    .await;
            }
            ClientRequest::TurnStart { request_id, params } => {
                self.turn_start(request_id, params).await;
            }
            ClientRequest::TurnInterrupt { request_id, params } => {
                self.turn_interrupt(request_id, params).await;
            }
            ClientRequest::NewConversation { request_id, params } => {
                // Do not tokio::spawn() to process new_conversation()
                // asynchronously because we need to ensure the conversation is
                // created before processing any subsequent messages.
                self.process_new_conversation(request_id, params).await;
            }
            ClientRequest::GetConversationSummary { request_id, params } => {
                self.get_conversation_summary(request_id, params).await;
            }
            ClientRequest::ListConversations { request_id, params } => {
                self.handle_list_conversations(request_id, params).await;
            }
            ClientRequest::ModelList { request_id, params } => {
                self.list_models(request_id, params).await;
            }
            ClientRequest::LoginAccount { request_id, params } => {
                self.login_v2(request_id, params).await;
            }
            ClientRequest::LogoutAccount {
                request_id,
                params: _,
            } => {
                self.logout_v2(request_id).await;
            }
            ClientRequest::CancelLoginAccount { request_id, params } => {
                self.cancel_login_v2(request_id, params).await;
            }
            ClientRequest::GetAccount {
                request_id,
                params: _,
            } => {
                self.send_unimplemented_error(request_id, "account/read")
                    .await;
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
                self.login_api_key_v1(request_id, params).await;
            }
            ClientRequest::LoginChatGpt {
                request_id,
                params: _,
            } => {
                self.login_chatgpt_v1(request_id).await;
            }
            ClientRequest::CancelLoginChatGpt { request_id, params } => {
                self.cancel_login_chatgpt(request_id, params.login_id).await;
            }
            ClientRequest::LogoutChatGpt {
                request_id,
                params: _,
            } => {
                self.logout_v1(request_id).await;
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
            ClientRequest::GetAccountRateLimits {
                request_id,
                params: _,
            } => {
                self.get_account_rate_limits(request_id).await;
            }
            ClientRequest::FeedbackUpload { request_id, params } => {
                self.upload_feedback(request_id, params).await;
            }
        }
    }

    async fn send_unimplemented_error(&self, request_id: RequestId, method: &str) {
        let error = JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
            message: format!("{method} is not implemented yet"),
            data: None,
        };
        self.outgoing.send_error(request_id, error).await;
    }

    async fn login_v2(&mut self, request_id: RequestId, params: LoginAccountParams) {
        match params {
            LoginAccountParams::ApiKey { api_key } => {
                self.login_api_key_v2(request_id, LoginApiKeyParams { api_key })
                    .await;
            }
            LoginAccountParams::Chatgpt => {
                self.login_chatgpt_v2(request_id).await;
            }
        }
    }

    async fn login_api_key_common(
        &mut self,
        params: &LoginApiKeyParams,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        if matches!(
            self.config.forced_login_method,
            Some(ForcedLoginMethod::Chatgpt)
        ) {
            return Err(JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: "API key login is disabled. Use ChatGPT login instead.".to_string(),
                data: None,
            });
        }

        // Cancel any active login attempt.
        {
            let mut guard = self.active_login.lock().await;
            if let Some(active) = guard.take() {
                active.drop();
            }
        }

        match login_with_api_key(
            &self.config.codex_home,
            &params.api_key,
            self.config.cli_auth_credentials_store_mode,
        ) {
            Ok(()) => {
                self.auth_manager.reload();
                Ok(())
            }
            Err(err) => Err(JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to save api key: {err}"),
                data: None,
            }),
        }
    }

    async fn login_api_key_v1(&mut self, request_id: RequestId, params: LoginApiKeyParams) {
        match self.login_api_key_common(&params).await {
            Ok(()) => {
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
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn login_api_key_v2(&mut self, request_id: RequestId, params: LoginApiKeyParams) {
        match self.login_api_key_common(&params).await {
            Ok(()) => {
                let response = codex_app_server_protocol::LoginAccountResponse::ApiKey {};
                self.outgoing.send_response(request_id, response).await;

                let payload_login_completed = AccountLoginCompletedNotification {
                    login_id: None,
                    success: true,
                    error: None,
                };
                self.outgoing
                    .send_server_notification(ServerNotification::AccountLoginCompleted(
                        payload_login_completed,
                    ))
                    .await;

                let payload_v2 = AccountUpdatedNotification {
                    auth_mode: self.auth_manager.auth().map(|auth| auth.mode),
                };
                self.outgoing
                    .send_server_notification(ServerNotification::AccountUpdated(payload_v2))
                    .await;
            }
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    // Build options for a ChatGPT login attempt; performs validation.
    async fn login_chatgpt_common(
        &self,
    ) -> std::result::Result<LoginServerOptions, JSONRPCErrorError> {
        let config = self.config.as_ref();

        if matches!(config.forced_login_method, Some(ForcedLoginMethod::Api)) {
            return Err(JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: "ChatGPT login is disabled. Use API key login instead.".to_string(),
                data: None,
            });
        }

        Ok(LoginServerOptions {
            open_browser: false,
            ..LoginServerOptions::new(
                config.codex_home.clone(),
                CLIENT_ID.to_string(),
                config.forced_chatgpt_workspace_id.clone(),
                config.cli_auth_credentials_store_mode,
            )
        })
    }

    // Deprecated in favor of login_chatgpt_v2.
    async fn login_chatgpt_v1(&mut self, request_id: RequestId) {
        match self.login_chatgpt_common().await {
            Ok(opts) => match run_login_server(opts) {
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

                    // Spawn background task to monitor completion.
                    let outgoing_clone = self.outgoing.clone();
                    let active_login = self.active_login.clone();
                    let auth_manager = self.auth_manager.clone();
                    let auth_url = server.auth_url.clone();
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
                                shutdown_handle.shutdown();
                                (false, Some("Login timed out".to_string()))
                            }
                        };

                        let payload = LoginChatGptCompleteNotification {
                            login_id,
                            success,
                            error: error_msg.clone(),
                        };
                        outgoing_clone
                            .send_server_notification(ServerNotification::LoginChatGptComplete(
                                payload,
                            ))
                            .await;

                        if success {
                            auth_manager.reload();

                            // Notify clients with the actual current auth mode.
                            let current_auth_method = auth_manager.auth().map(|a| a.mode);
                            let payload = AuthStatusChangeNotification {
                                auth_method: current_auth_method,
                            };
                            outgoing_clone
                                .send_server_notification(ServerNotification::AuthStatusChange(
                                    payload,
                                ))
                                .await;
                        }

                        // Clear the active login if it matches this attempt. It may have been replaced or cancelled.
                        let mut guard = active_login.lock().await;
                        if guard.as_ref().map(|l| l.login_id) == Some(login_id) {
                            *guard = None;
                        }
                    });

                    let response = LoginChatGptResponse { login_id, auth_url };
                    self.outgoing.send_response(request_id, response).await;
                }
                Err(err) => {
                    let error = JSONRPCErrorError {
                        code: INTERNAL_ERROR_CODE,
                        message: format!("failed to start login server: {err}"),
                        data: None,
                    };
                    self.outgoing.send_error(request_id, error).await;
                }
            },
            Err(err) => {
                self.outgoing.send_error(request_id, err).await;
            }
        }
    }

    async fn login_chatgpt_v2(&mut self, request_id: RequestId) {
        match self.login_chatgpt_common().await {
            Ok(opts) => match run_login_server(opts) {
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

                    // Spawn background task to monitor completion.
                    let outgoing_clone = self.outgoing.clone();
                    let active_login = self.active_login.clone();
                    let auth_manager = self.auth_manager.clone();
                    let auth_url = server.auth_url.clone();
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
                                shutdown_handle.shutdown();
                                (false, Some("Login timed out".to_string()))
                            }
                        };

                        let payload_v2 = AccountLoginCompletedNotification {
                            login_id: Some(login_id.to_string()),
                            success,
                            error: error_msg,
                        };
                        outgoing_clone
                            .send_server_notification(ServerNotification::AccountLoginCompleted(
                                payload_v2,
                            ))
                            .await;

                        if success {
                            auth_manager.reload();

                            // Notify clients with the actual current auth mode.
                            let current_auth_method = auth_manager.auth().map(|a| a.mode);
                            let payload_v2 = AccountUpdatedNotification {
                                auth_mode: current_auth_method,
                            };
                            outgoing_clone
                                .send_server_notification(ServerNotification::AccountUpdated(
                                    payload_v2,
                                ))
                                .await;
                        }

                        // Clear the active login if it matches this attempt. It may have been replaced or cancelled.
                        let mut guard = active_login.lock().await;
                        if guard.as_ref().map(|l| l.login_id) == Some(login_id) {
                            *guard = None;
                        }
                    });

                    let response = codex_app_server_protocol::LoginAccountResponse::Chatgpt {
                        login_id: login_id.to_string(),
                        auth_url,
                    };
                    self.outgoing.send_response(request_id, response).await;
                }
                Err(err) => {
                    let error = JSONRPCErrorError {
                        code: INTERNAL_ERROR_CODE,
                        message: format!("failed to start login server: {err}"),
                        data: None,
                    };
                    self.outgoing.send_error(request_id, error).await;
                }
            },
            Err(err) => {
                self.outgoing.send_error(request_id, err).await;
            }
        }
    }

    async fn cancel_login_chatgpt_common(
        &mut self,
        login_id: Uuid,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        let mut guard = self.active_login.lock().await;
        if guard.as_ref().map(|l| l.login_id) == Some(login_id) {
            if let Some(active) = guard.take() {
                active.drop();
            }
            Ok(())
        } else {
            Err(JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("login id not found: {login_id}"),
                data: None,
            })
        }
    }

    async fn cancel_login_chatgpt(&mut self, request_id: RequestId, login_id: Uuid) {
        match self.cancel_login_chatgpt_common(login_id).await {
            Ok(()) => {
                self.outgoing
                    .send_response(request_id, CancelLoginChatGptResponse {})
                    .await;
            }
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn cancel_login_v2(&mut self, request_id: RequestId, params: CancelLoginAccountParams) {
        let login_id = params.login_id;
        match Uuid::parse_str(&login_id) {
            Ok(uuid) => match self.cancel_login_chatgpt_common(uuid).await {
                Ok(()) => {
                    self.outgoing
                        .send_response(request_id, CancelLoginAccountResponse {})
                        .await;
                }
                Err(error) => {
                    self.outgoing.send_error(request_id, error).await;
                }
            },
            Err(_) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("invalid login id: {login_id}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn logout_common(&mut self) -> std::result::Result<Option<AuthMode>, JSONRPCErrorError> {
        // Cancel any active login attempt.
        {
            let mut guard = self.active_login.lock().await;
            if let Some(active) = guard.take() {
                active.drop();
            }
        }

        if let Err(err) = self.auth_manager.logout() {
            return Err(JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("logout failed: {err}"),
                data: None,
            });
        }

        // Reflect the current auth method after logout (likely None).
        Ok(self.auth_manager.auth().map(|auth| auth.mode))
    }

    async fn logout_v1(&mut self, request_id: RequestId) {
        match self.logout_common().await {
            Ok(current_auth_method) => {
                self.outgoing
                    .send_response(request_id, LogoutChatGptResponse {})
                    .await;

                let payload = AuthStatusChangeNotification {
                    auth_method: current_auth_method,
                };
                self.outgoing
                    .send_server_notification(ServerNotification::AuthStatusChange(payload))
                    .await;
            }
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn logout_v2(&mut self, request_id: RequestId) {
        match self.logout_common().await {
            Ok(current_auth_method) => {
                self.outgoing
                    .send_response(request_id, LogoutAccountResponse {})
                    .await;

                let payload_v2 = AccountUpdatedNotification {
                    auth_mode: current_auth_method,
                };
                self.outgoing
                    .send_server_notification(ServerNotification::AccountUpdated(payload_v2))
                    .await;
            }
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn get_auth_status(&self, request_id: RequestId, params: GetAuthStatusParams) {
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
            GetAuthStatusResponse {
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
                    GetAuthStatusResponse {
                        auth_method: reported_auth_method,
                        auth_token: token_opt,
                        requires_openai_auth: Some(true),
                    }
                }
                None => GetAuthStatusResponse {
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

    async fn get_account_rate_limits(&self, request_id: RequestId) {
        match self.fetch_account_rate_limits().await {
            Ok(rate_limits) => {
                let response = GetAccountRateLimitsResponse {
                    rate_limits: rate_limits.into(),
                };
                self.outgoing.send_response(request_id, response).await;
            }
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn fetch_account_rate_limits(&self) -> Result<CoreRateLimitSnapshot, JSONRPCErrorError> {
        let Some(auth) = self.auth_manager.auth() else {
            return Err(JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: "codex account authentication required to read rate limits".to_string(),
                data: None,
            });
        };

        if auth.mode != AuthMode::ChatGPT {
            return Err(JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: "chatgpt authentication required to read rate limits".to_string(),
                data: None,
            });
        }

        let client = BackendClient::from_auth(self.config.chatgpt_base_url.clone(), &auth)
            .await
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to construct backend client: {err}"),
                data: None,
            })?;

        client
            .get_rate_limits()
            .await
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to fetch codex rate limits: {err}"),
                data: None,
            })
    }

    async fn get_user_saved_config(&self, request_id: RequestId) {
        let toml_value = match load_config_as_toml(&self.config.codex_home).await {
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
        // Read alleged user email from cached auth (best-effort; not verified).
        let alleged_user_email = self.auth_manager.auth().and_then(|a| a.get_account_email());

        let response = UserInfoResponse { alleged_user_email };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn set_default_model(&self, request_id: RequestId, params: SetDefaultModelParams) {
        let SetDefaultModelParams {
            model,
            reasoning_effort,
        } = params;

        match ConfigEditsBuilder::new(&self.config.codex_home)
            .with_profile(self.config.active_profile.as_deref())
            .set_model(model.as_deref(), reasoning_effort)
            .apply()
            .await
        {
            Ok(()) => {
                let response = SetDefaultModelResponse {};
                self.outgoing.send_response(request_id, response).await;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to persist model selection: {err}"),
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
            arg0: None,
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
        let NewConversationParams {
            model,
            model_provider,
            profile,
            cwd,
            approval_policy,
            sandbox: sandbox_mode,
            config: cli_overrides,
            base_instructions,
            developer_instructions,
            compact_prompt,
            include_apply_patch_tool,
        } = params;

        let overrides = ConfigOverrides {
            model,
            config_profile: profile,
            cwd: cwd.map(PathBuf::from),
            approval_policy,
            sandbox_mode,
            model_provider,
            codex_linux_sandbox_exe: self.codex_linux_sandbox_exe.clone(),
            base_instructions,
            developer_instructions,
            compact_prompt,
            include_apply_patch_tool,
            ..Default::default()
        };

        let config = match derive_config_from_params(overrides, cli_overrides).await {
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

    async fn thread_start(&mut self, request_id: RequestId, params: ThreadStartParams) {
        // Build ConfigOverrides directly from ThreadStartParams for config derivation.
        let cli_overrides = params.config;
        let overrides = ConfigOverrides {
            model: params.model,
            cwd: params.cwd.map(PathBuf::from),
            approval_policy: params.approval_policy.map(AskForApproval::to_core),
            sandbox_mode: params.sandbox.map(SandboxMode::to_core),
            model_provider: params.model_provider,
            codex_linux_sandbox_exe: self.codex_linux_sandbox_exe.clone(),
            base_instructions: params.base_instructions,
            developer_instructions: params.developer_instructions,
            ..Default::default()
        };

        let config = match derive_config_from_params(overrides, cli_overrides).await {
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
            Ok(new_conv) => {
                let thread = Thread {
                    id: new_conv.conversation_id.to_string(),
                };

                let response = ThreadStartResponse {
                    thread: thread.clone(),
                };

                // Auto-attach a conversation listener when starting a thread.
                // Use the same behavior as the v1 API with experimental_raw_events=false.
                if let Err(err) = self
                    .attach_conversation_listener(new_conv.conversation_id, false)
                    .await
                {
                    tracing::warn!(
                        "failed to attach listener for conversation {}: {}",
                        new_conv.conversation_id,
                        err.message
                    );
                }

                self.outgoing.send_response(request_id, response).await;

                let notif = ThreadStartedNotification { thread };
                self.outgoing
                    .send_server_notification(ServerNotification::ThreadStarted(notif))
                    .await;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("error creating thread: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn thread_archive(&mut self, request_id: RequestId, params: ThreadArchiveParams) {
        let conversation_id = match ConversationId::from_string(&params.thread_id) {
            Ok(id) => id,
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("invalid thread id: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let rollout_path = match find_conversation_path_by_id_str(
            &self.config.codex_home,
            &conversation_id.to_string(),
        )
        .await
        {
            Ok(Some(p)) => p,
            Ok(None) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("no rollout found for conversation id {conversation_id}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("failed to locate conversation id {conversation_id}: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        match self
            .archive_conversation_common(conversation_id, &rollout_path)
            .await
        {
            Ok(()) => {
                let response = ThreadArchiveResponse {};
                self.outgoing.send_response(request_id, response).await;
            }
            Err(err) => {
                self.outgoing.send_error(request_id, err).await;
            }
        }
    }

    async fn thread_list(&self, request_id: RequestId, params: ThreadListParams) {
        let ThreadListParams {
            cursor,
            limit,
            model_providers,
        } = params;

        let page_size = limit.unwrap_or(25).max(1) as usize;

        let (summaries, next_cursor) = match self
            .list_conversations_common(page_size, cursor, model_providers)
            .await
        {
            Ok(r) => r,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let data = summaries
            .into_iter()
            .map(|s| Thread {
                id: s.conversation_id.to_string(),
            })
            .collect();

        let response = ThreadListResponse { data, next_cursor };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn thread_resume(&mut self, request_id: RequestId, params: ThreadResumeParams) {
        let conversation_id = match ConversationId::from_string(&params.thread_id) {
            Ok(id) => id,
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("invalid thread id: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let path = match find_conversation_path_by_id_str(
            &self.config.codex_home,
            &conversation_id.to_string(),
        )
        .await
        {
            Ok(Some(p)) => p,
            Ok(None) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("no rollout found for conversation id {conversation_id}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("failed to locate conversation id {conversation_id}: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let fallback_provider = self.config.model_provider_id.as_str();
        let summary = match read_summary_from_rollout(&path, fallback_provider).await {
            Ok(s) => s,
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("failed to load rollout `{}`: {err}", path.display()),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let initial_history = match RolloutRecorder::get_rollout_history(&summary.path).await {
            Ok(initial_history) => initial_history,
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!(
                        "failed to load rollout `{}` for conversation {conversation_id}: {err}",
                        summary.path.display()
                    ),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        match self
            .conversation_manager
            .resume_conversation_with_history(
                self.config.as_ref().clone(),
                initial_history,
                self.auth_manager.clone(),
            )
            .await
        {
            Ok(_) => {
                // Auto-attach a conversation listener when resuming a thread.
                if let Err(err) = self
                    .attach_conversation_listener(conversation_id, false)
                    .await
                {
                    tracing::warn!(
                        "failed to attach listener for conversation {}: {}",
                        conversation_id,
                        err.message
                    );
                }

                let response = ThreadResumeResponse {
                    thread: Thread {
                        id: conversation_id.to_string(),
                    },
                };
                self.outgoing.send_response(request_id, response).await;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("error resuming thread: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn get_conversation_summary(
        &self,
        request_id: RequestId,
        params: GetConversationSummaryParams,
    ) {
        let path = match params {
            GetConversationSummaryParams::RolloutPath { rollout_path } => {
                if rollout_path.is_relative() {
                    self.config.codex_home.join(&rollout_path)
                } else {
                    rollout_path
                }
            }
            GetConversationSummaryParams::ConversationId { conversation_id } => {
                match codex_core::find_conversation_path_by_id_str(
                    &self.config.codex_home,
                    &conversation_id.to_string(),
                )
                .await
                {
                    Ok(Some(p)) => p,
                    _ => {
                        let error = JSONRPCErrorError {
                            code: INVALID_REQUEST_ERROR_CODE,
                            message: format!(
                                "no rollout found for conversation id {conversation_id}"
                            ),
                            data: None,
                        };
                        self.outgoing.send_error(request_id, error).await;
                        return;
                    }
                }
            }
        };

        let fallback_provider = self.config.model_provider_id.as_str();

        match read_summary_from_rollout(&path, fallback_provider).await {
            Ok(summary) => {
                let response = GetConversationSummaryResponse { summary };
                self.outgoing.send_response(request_id, response).await;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!(
                        "failed to load conversation summary from {}: {}",
                        path.display(),
                        err
                    ),
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
        let ListConversationsParams {
            page_size,
            cursor,
            model_providers,
        } = params;
        let page_size = page_size.unwrap_or(25).max(1);

        match self
            .list_conversations_common(page_size, cursor, model_providers)
            .await
        {
            Ok((items, next_cursor)) => {
                let response = ListConversationsResponse { items, next_cursor };
                self.outgoing.send_response(request_id, response).await;
            }
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
            }
        };
    }

    async fn list_conversations_common(
        &self,
        page_size: usize,
        cursor: Option<String>,
        model_providers: Option<Vec<String>>,
    ) -> Result<(Vec<ConversationSummary>, Option<String>), JSONRPCErrorError> {
        let cursor_obj: Option<RolloutCursor> = cursor.as_ref().and_then(|s| parse_cursor(s));
        let cursor_ref = cursor_obj.as_ref();

        let model_provider_filter = match model_providers {
            Some(providers) => {
                if providers.is_empty() {
                    None
                } else {
                    Some(providers)
                }
            }
            None => Some(vec![self.config.model_provider_id.clone()]),
        };
        let fallback_provider = self.config.model_provider_id.clone();

        let page = match RolloutRecorder::list_conversations(
            &self.config.codex_home,
            page_size,
            cursor_ref,
            INTERACTIVE_SESSION_SOURCES,
            model_provider_filter.as_deref(),
            fallback_provider.as_str(),
        )
        .await
        {
            Ok(p) => p,
            Err(err) => {
                return Err(JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to list conversations: {err}"),
                    data: None,
                });
            }
        };

        let items = page
            .items
            .into_iter()
            .filter_map(|it| extract_conversation_summary(it.path, &it.head, &fallback_provider))
            .collect::<Vec<_>>();

        // Encode next_cursor as a plain string
        let next_cursor = page
            .next_cursor
            .and_then(|cursor| serde_json::to_value(&cursor).ok())
            .and_then(|value| value.as_str().map(str::to_owned));

        Ok((items, next_cursor))
    }

    async fn list_models(&self, request_id: RequestId, params: ModelListParams) {
        let ModelListParams { limit, cursor } = params;
        let auth_mode = self.auth_manager.auth().map(|auth| auth.mode);
        let models = supported_models(auth_mode);
        let total = models.len();

        if total == 0 {
            let response = ModelListResponse {
                data: Vec::new(),
                next_cursor: None,
            };
            self.outgoing.send_response(request_id, response).await;
            return;
        }

        let effective_limit = limit.unwrap_or(total as u32).max(1) as usize;
        let effective_limit = effective_limit.min(total);
        let start = match cursor {
            Some(cursor) => match cursor.parse::<usize>() {
                Ok(idx) => idx,
                Err(_) => {
                    let error = JSONRPCErrorError {
                        code: INVALID_REQUEST_ERROR_CODE,
                        message: format!("invalid cursor: {cursor}"),
                        data: None,
                    };
                    self.outgoing.send_error(request_id, error).await;
                    return;
                }
            },
            None => 0,
        };

        if start > total {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("cursor {start} exceeds total models {total}"),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        }

        let end = start.saturating_add(effective_limit).min(total);
        let items = models[start..end].to_vec();
        let next_cursor = if end < total {
            Some(end.to_string())
        } else {
            None
        };
        let response = ModelListResponse {
            data: items,
            next_cursor,
        };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn handle_resume_conversation(
        &self,
        request_id: RequestId,
        params: ResumeConversationParams,
    ) {
        let ResumeConversationParams {
            path,
            conversation_id,
            history,
            overrides,
        } = params;

        // Derive a Config using the same logic as new conversation, honoring overrides if provided.
        let config = match overrides {
            Some(overrides) => {
                let NewConversationParams {
                    model,
                    model_provider,
                    profile,
                    cwd,
                    approval_policy,
                    sandbox: sandbox_mode,
                    config: cli_overrides,
                    base_instructions,
                    developer_instructions,
                    compact_prompt,
                    include_apply_patch_tool,
                } = overrides;

                let overrides = ConfigOverrides {
                    model,
                    config_profile: profile,
                    cwd: cwd.map(PathBuf::from),
                    approval_policy,
                    sandbox_mode,
                    model_provider,
                    codex_linux_sandbox_exe: self.codex_linux_sandbox_exe.clone(),
                    base_instructions,
                    developer_instructions,
                    compact_prompt,
                    include_apply_patch_tool,
                    ..Default::default()
                };

                derive_config_from_params(overrides, cli_overrides).await
            }
            None => Ok(self.config.as_ref().clone()),
        };
        let config = match config {
            Ok(cfg) => cfg,
            Err(err) => {
                self.send_invalid_request_error(
                    request_id,
                    format!("error deriving config: {err}"),
                )
                .await;
                return;
            }
        };

        let conversation_history = if let Some(path) = path {
            match RolloutRecorder::get_rollout_history(&path).await {
                Ok(initial_history) => initial_history,
                Err(err) => {
                    self.send_invalid_request_error(
                        request_id,
                        format!("failed to load rollout `{}`: {err}", path.display()),
                    )
                    .await;
                    return;
                }
            }
        } else if let Some(conversation_id) = conversation_id {
            match find_conversation_path_by_id_str(
                &self.config.codex_home,
                &conversation_id.to_string(),
            )
            .await
            {
                Ok(Some(found_path)) => {
                    match RolloutRecorder::get_rollout_history(&found_path).await {
                        Ok(initial_history) => initial_history,
                        Err(err) => {
                            self.send_invalid_request_error(
                                request_id,
                                format!(
                                    "failed to load rollout `{}` for conversation {conversation_id}: {err}",
                                    found_path.display()
                                ),
                            ).await;
                            return;
                        }
                    }
                }
                Ok(None) => {
                    self.send_invalid_request_error(
                        request_id,
                        format!("no rollout found for conversation id {conversation_id}"),
                    )
                    .await;
                    return;
                }
                Err(err) => {
                    self.send_invalid_request_error(
                        request_id,
                        format!("failed to locate conversation id {conversation_id}: {err}"),
                    )
                    .await;
                    return;
                }
            }
        } else {
            match history {
                Some(history) if !history.is_empty() => InitialHistory::Forked(
                    history.into_iter().map(RolloutItem::ResponseItem).collect(),
                ),
                Some(_) | None => {
                    self.send_invalid_request_error(
                        request_id,
                        "either path, conversation id or non empty history must be provided"
                            .to_string(),
                    )
                    .await;
                    return;
                }
            }
        };

        match self
            .conversation_manager
            .resume_conversation_with_history(
                config,
                conversation_history,
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
                let initial_messages = session_configured
                    .initial_messages
                    .map(|msgs| msgs.into_iter().collect());

                // Reply with conversation id + model and initial messages (when present)
                let response = ResumeConversationResponse {
                    conversation_id,
                    model: session_configured.model.clone(),
                    initial_messages,
                    rollout_path: session_configured.rollout_path.clone(),
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

    async fn send_invalid_request_error(&self, request_id: RequestId, message: String) {
        let error = JSONRPCErrorError {
            code: INVALID_REQUEST_ERROR_CODE,
            message,
            data: None,
        };
        self.outgoing.send_error(request_id, error).await;
    }

    async fn archive_conversation(
        &mut self,
        request_id: RequestId,
        params: ArchiveConversationParams,
    ) {
        let ArchiveConversationParams {
            conversation_id,
            rollout_path,
        } = params;

        match self
            .archive_conversation_common(conversation_id, &rollout_path)
            .await
        {
            Ok(()) => {
                tracing::info!("thread/archive succeeded for {conversation_id}");
                let response = ArchiveConversationResponse {};
                self.outgoing.send_response(request_id, response).await;
            }
            Err(err) => {
                tracing::warn!(
                    "thread/archive failed for {conversation_id}: {}",
                    err.message
                );
                self.outgoing.send_error(request_id, err).await;
            }
        }
    }

    async fn archive_conversation_common(
        &mut self,
        conversation_id: ConversationId,
        rollout_path: &Path,
    ) -> Result<(), JSONRPCErrorError> {
        // Verify rollout_path is under sessions dir.
        let rollout_folder = self.config.codex_home.join(codex_core::SESSIONS_SUBDIR);

        let canonical_sessions_dir = match tokio::fs::canonicalize(&rollout_folder).await {
            Ok(path) => path,
            Err(err) => {
                return Err(JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!(
                        "failed to archive conversation: unable to resolve sessions directory: {err}"
                    ),
                    data: None,
                });
            }
        };
        let canonical_rollout_path = tokio::fs::canonicalize(rollout_path).await;
        let canonical_rollout_path = if let Ok(path) = canonical_rollout_path
            && path.starts_with(&canonical_sessions_dir)
        {
            path
        } else {
            return Err(JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!(
                    "rollout path `{}` must be in sessions directory",
                    rollout_path.display()
                ),
                data: None,
            });
        };

        // Verify file name matches conversation id.
        let required_suffix = format!("{conversation_id}.jsonl");
        let Some(file_name) = canonical_rollout_path.file_name().map(OsStr::to_owned) else {
            return Err(JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!(
                    "rollout path `{}` missing file name",
                    rollout_path.display()
                ),
                data: None,
            });
        };
        if !file_name
            .to_string_lossy()
            .ends_with(required_suffix.as_str())
        {
            return Err(JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!(
                    "rollout path `{}` does not match conversation id {conversation_id}",
                    rollout_path.display()
                ),
                data: None,
            });
        }

        // If the conversation is active, request shutdown and wait briefly.
        if let Some(conversation) = self
            .conversation_manager
            .remove_conversation(&conversation_id)
            .await
        {
            info!("conversation {conversation_id} was active; shutting down");
            let conversation_clone = conversation.clone();
            let notify = Arc::new(tokio::sync::Notify::new());
            let notify_clone = notify.clone();

            // Establish the listener for ShutdownComplete before submitting
            // Shutdown so it is not missed.
            let is_shutdown = tokio::spawn(async move {
                // Create the notified future outside the loop to avoid losing notifications.
                let notified = notify_clone.notified();
                tokio::pin!(notified);
                loop {
                    select! {
                        _ = &mut notified => { break; }
                        event = conversation_clone.next_event() => {
                            match event {
                                Ok(event) => {
                                    if matches!(event.msg, EventMsg::ShutdownComplete) { break; }
                                }
                                // Break on errors to avoid tight loops when the agent loop has exited.
                                Err(_) => { break; }
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
                            // Wake any waiter; use notify_waiters to avoid missing the signal.
                            notify.notify_waiters();
                            // Perhaps we lost a shutdown race, so let's continue to
                            // clean up the .jsonl file.
                        }
                    }
                }
                Err(err) => {
                    error!("failed to submit Shutdown to conversation {conversation_id}: {err}");
                    notify.notify_waiters();
                }
            }
        }

        // Move the rollout file to archived.
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

        result.map_err(|err| JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
            message: format!("failed to archive conversation: {err}"),
            data: None,
        })
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
            map.entry(conversation_id)
                .or_default()
                .push((request_id, ApiVersion::V1));
        }

        // Submit the interrupt; we'll respond upon TurnAborted.
        let _ = conversation.submit(Op::Interrupt).await;
    }

    async fn turn_start(&self, request_id: RequestId, params: TurnStartParams) {
        // Resolve conversation id from v2 thread id string.
        let conversation_id = match ConversationId::from_string(&params.thread_id) {
            Ok(id) => id,
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("invalid thread id: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

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

        // Keep a copy of v2 inputs for the notification payload.
        let v2_inputs_for_notif = params.input.clone();

        // Map v2 input items to core input items.
        let mapped_items: Vec<CoreInputItem> = params
            .input
            .into_iter()
            .map(V2UserInput::into_core)
            .collect();

        let has_any_overrides = params.cwd.is_some()
            || params.approval_policy.is_some()
            || params.sandbox_policy.is_some()
            || params.model.is_some()
            || params.effort.is_some()
            || params.summary.is_some();

        // If any overrides are provided, update the session turn context first.
        if has_any_overrides {
            let _ = conversation
                .submit(Op::OverrideTurnContext {
                    cwd: params.cwd,
                    approval_policy: params.approval_policy.map(AskForApproval::to_core),
                    sandbox_policy: params.sandbox_policy.map(|p| p.to_core()),
                    model: params.model,
                    effort: params.effort.map(Some),
                    summary: params.summary,
                })
                .await;
        }

        // Start the turn by submitting the user input. Return its submission id as turn_id.
        let turn_id = conversation
            .submit(Op::UserInput {
                items: mapped_items,
            })
            .await;

        match turn_id {
            Ok(turn_id) => {
                let turn = Turn {
                    id: turn_id.clone(),
                    items: vec![ThreadItem::UserMessage {
                        id: turn_id,
                        content: v2_inputs_for_notif,
                    }],
                    status: TurnStatus::InProgress,
                    error: None,
                };

                let response = TurnStartResponse { turn: turn.clone() };
                self.outgoing.send_response(request_id, response).await;

                // Emit v2 turn/started notification.
                let notif = TurnStartedNotification { turn };
                self.outgoing
                    .send_server_notification(ServerNotification::TurnStarted(notif))
                    .await;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to start turn: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn turn_interrupt(&mut self, request_id: RequestId, params: TurnInterruptParams) {
        let TurnInterruptParams { thread_id, .. } = params;

        // Resolve conversation id from v2 thread id string.
        let conversation_id = match ConversationId::from_string(&thread_id) {
            Ok(id) => id,
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("invalid thread id: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

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
            map.entry(conversation_id)
                .or_default()
                .push((request_id, ApiVersion::V2));
        }

        // Submit the interrupt; we'll respond upon TurnAborted.
        let _ = conversation.submit(Op::Interrupt).await;
    }

    async fn add_conversation_listener(
        &mut self,
        request_id: RequestId,
        params: AddConversationListenerParams,
    ) {
        let AddConversationListenerParams {
            conversation_id,
            experimental_raw_events,
        } = params;
        match self
            .attach_conversation_listener(conversation_id, experimental_raw_events)
            .await
        {
            Ok(subscription_id) => {
                let response = AddConversationSubscriptionResponse { subscription_id };
                self.outgoing.send_response(request_id, response).await;
            }
            Err(err) => {
                self.outgoing.send_error(request_id, err).await;
            }
        }
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

    async fn attach_conversation_listener(
        &mut self,
        conversation_id: ConversationId,
        experimental_raw_events: bool,
    ) -> Result<Uuid, JSONRPCErrorError> {
        let conversation = match self
            .conversation_manager
            .get_conversation(conversation_id)
            .await
        {
            Ok(conv) => conv,
            Err(_) => {
                return Err(JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("conversation not found: {conversation_id}"),
                    data: None,
                });
            }
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

                        if let EventMsg::RawResponseItem(_) = &event.msg
                            && !experimental_raw_events {
                                continue;
                            }

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
                        params.insert(
                            "conversationId".to_string(),
                            conversation_id.to_string().into(),
                        );

                        outgoing_for_task
                            .send_notification(OutgoingNotification {
                                method,
                                params: Some(params.into()),
                            })
                            .await;

                        apply_bespoke_event_handling(
                            event.clone(),
                            conversation_id,
                            conversation.clone(),
                            outgoing_for_task.clone(),
                            pending_interrupts.clone(),
                        )
                        .await;
                    }
                }
            }
        });
        Ok(subscription_id)
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

    async fn upload_feedback(&self, request_id: RequestId, params: FeedbackUploadParams) {
        let FeedbackUploadParams {
            classification,
            reason,
            conversation_id,
            include_logs,
        } = params;

        let snapshot = self.feedback.snapshot(conversation_id);
        let thread_id = snapshot.thread_id.clone();

        let validated_rollout_path = if include_logs {
            match conversation_id {
                Some(conv_id) => self.resolve_rollout_path(conv_id).await,
                None => None,
            }
        } else {
            None
        };

        let upload_result = tokio::task::spawn_blocking(move || {
            let rollout_path_ref = validated_rollout_path.as_deref();
            snapshot.upload_feedback(
                &classification,
                reason.as_deref(),
                include_logs,
                rollout_path_ref,
            )
        })
        .await;

        let upload_result = match upload_result {
            Ok(result) => result,
            Err(join_err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to upload feedback: {join_err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        match upload_result {
            Ok(()) => {
                let response = FeedbackUploadResponse { thread_id };
                self.outgoing.send_response(request_id, response).await;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to upload feedback: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn resolve_rollout_path(&self, conversation_id: ConversationId) -> Option<PathBuf> {
        match self
            .conversation_manager
            .get_conversation(conversation_id)
            .await
        {
            Ok(conv) => Some(conv.rollout_path()),
            Err(_) => None,
        }
    }
}

async fn apply_bespoke_event_handling(
    event: Event,
    conversation_id: ConversationId,
    conversation: Arc<CodexConversation>,
    outgoing: Arc<OutgoingMessageSender>,
    pending_interrupts: PendingInterrupts,
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
            risk,
            parsed_cmd,
        }) => {
            let params = ExecCommandApprovalParams {
                conversation_id,
                call_id,
                command,
                cwd,
                reason,
                risk,
                parsed_cmd,
            };
            let rx = outgoing
                .send_request(ServerRequestPayload::ExecCommandApproval(params))
                .await;

            // TODO(mbolin): Enforce a timeout so this task does not live indefinitely?
            tokio::spawn(async move {
                on_exec_approval_response(event_id, rx, conversation).await;
            });
        }
        EventMsg::TokenCount(token_count_event) => {
            if let Some(rate_limits) = token_count_event.rate_limits {
                outgoing
                    .send_server_notification(ServerNotification::AccountRateLimitsUpdated(
                        AccountRateLimitsUpdatedNotification {
                            rate_limits: rate_limits.into(),
                        },
                    ))
                    .await;
            }
        }
        // If this is a TurnAborted, reply to any pending interrupt requests.
        EventMsg::TurnAborted(turn_aborted_event) => {
            let pending = {
                let mut map = pending_interrupts.lock().await;
                map.remove(&conversation_id).unwrap_or_default()
            };
            if !pending.is_empty() {
                for (rid, ver) in pending {
                    match ver {
                        ApiVersion::V1 => {
                            let response = InterruptConversationResponse {
                                abort_reason: turn_aborted_event.reason.clone(),
                            };
                            outgoing.send_response(rid, response).await;
                        }
                        ApiVersion::V2 => {
                            let response = TurnInterruptResponse {};
                            outgoing.send_response(rid, response).await;
                        }
                    }
                }
            }
        }

        _ => {}
    }
}

async fn derive_config_from_params(
    overrides: ConfigOverrides,
    cli_overrides: Option<std::collections::HashMap<String, serde_json::Value>>,
) -> std::io::Result<Config> {
    let cli_overrides = cli_overrides
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| (k, json_to_toml(v)))
        .collect();

    Config::load_with_cli_overrides(cli_overrides, overrides).await
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

async fn read_summary_from_rollout(
    path: &Path,
    fallback_provider: &str,
) -> std::io::Result<ConversationSummary> {
    let head = read_head_for_summary(path).await?;

    let Some(first) = head.first() else {
        return Err(IoError::other(format!(
            "rollout at {} is empty",
            path.display()
        )));
    };

    let session_meta = serde_json::from_value::<SessionMeta>(first.clone()).map_err(|_| {
        IoError::other(format!(
            "rollout at {} does not start with session metadata",
            path.display()
        ))
    })?;

    if let Some(summary) =
        extract_conversation_summary(path.to_path_buf(), &head, fallback_provider)
    {
        return Ok(summary);
    }

    let timestamp = if session_meta.timestamp.is_empty() {
        None
    } else {
        Some(session_meta.timestamp.clone())
    };
    let model_provider = session_meta
        .model_provider
        .unwrap_or_else(|| fallback_provider.to_string());

    Ok(ConversationSummary {
        conversation_id: session_meta.id,
        timestamp,
        path: path.to_path_buf(),
        preview: String::new(),
        model_provider,
    })
}

fn extract_conversation_summary(
    path: PathBuf,
    head: &[serde_json::Value],
    fallback_provider: &str,
) -> Option<ConversationSummary> {
    let session_meta = match head.first() {
        Some(first_line) => serde_json::from_value::<SessionMeta>(first_line.clone()).ok()?,
        None => return None,
    };

    let preview = head
        .iter()
        .filter_map(|value| serde_json::from_value::<ResponseItem>(value.clone()).ok())
        .find_map(|item| match codex_core::parse_turn_item(&item) {
            Some(TurnItem::UserMessage(user)) => Some(user.message()),
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
    let conversation_id = session_meta.id;
    let model_provider = session_meta
        .model_provider
        .unwrap_or_else(|| fallback_provider.to_string());

    Some(ConversationSummary {
        conversation_id,
        timestamp,
        path,
        preview: preview.to_string(),
        model_provider,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tempfile::TempDir;

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
                "instructions": null,
                "model_provider": "test-provider"
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

        let summary =
            extract_conversation_summary(path.clone(), &head, "test-provider").expect("summary");

        let expected = ConversationSummary {
            conversation_id,
            timestamp,
            path,
            preview: "Count to 5".to_string(),
            model_provider: "test-provider".to_string(),
        };

        assert_eq!(summary, expected);
        Ok(())
    }

    #[tokio::test]
    async fn read_summary_from_rollout_returns_empty_preview_when_no_user_message() -> Result<()> {
        use codex_protocol::protocol::RolloutItem;
        use codex_protocol::protocol::RolloutLine;
        use codex_protocol::protocol::SessionMetaLine;
        use std::fs;

        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().join("rollout.jsonl");

        let conversation_id = ConversationId::from_string("bfd12a78-5900-467b-9bc5-d3d35df08191")?;
        let timestamp = "2025-09-05T16:53:11.850Z".to_string();

        let session_meta = SessionMeta {
            id: conversation_id,
            timestamp: timestamp.clone(),
            model_provider: None,
            ..SessionMeta::default()
        };

        let line = RolloutLine {
            timestamp: timestamp.clone(),
            item: RolloutItem::SessionMeta(SessionMetaLine {
                meta: session_meta.clone(),
                git: None,
            }),
        };

        fs::write(&path, format!("{}\n", serde_json::to_string(&line)?))?;

        let summary = read_summary_from_rollout(path.as_path(), "fallback").await?;

        let expected = ConversationSummary {
            conversation_id,
            timestamp: Some(timestamp),
            path: path.clone(),
            preview: String::new(),
            model_provider: "fallback".to_string(),
        };

        assert_eq!(summary, expected);
        Ok(())
    }
}
