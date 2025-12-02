use crate::bespoke_event_handling::apply_bespoke_event_handling;
use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use crate::fuzzy_file_search::run_fuzzy_file_search;
use crate::models::supported_models;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::OutgoingNotification;
use chrono::DateTime;
use chrono::Utc;
use codex_app_server_protocol::Account;
use codex_app_server_protocol::AccountLoginCompletedNotification;
use codex_app_server_protocol::AccountUpdatedNotification;
use codex_app_server_protocol::AddConversationListenerParams;
use codex_app_server_protocol::AddConversationSubscriptionResponse;
use codex_app_server_protocol::ArchiveConversationParams;
use codex_app_server_protocol::ArchiveConversationResponse;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::AuthStatusChangeNotification;
use codex_app_server_protocol::CancelLoginAccountParams;
use codex_app_server_protocol::CancelLoginAccountResponse;
use codex_app_server_protocol::CancelLoginChatGptResponse;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CommandExecParams;
use codex_app_server_protocol::ConversationGitInfo;
use codex_app_server_protocol::ConversationSummary;
use codex_app_server_protocol::ExecOneOffCommandResponse;
use codex_app_server_protocol::FeedbackUploadParams;
use codex_app_server_protocol::FeedbackUploadResponse;
use codex_app_server_protocol::FuzzyFileSearchParams;
use codex_app_server_protocol::FuzzyFileSearchResponse;
use codex_app_server_protocol::GetAccountParams;
use codex_app_server_protocol::GetAccountRateLimitsResponse;
use codex_app_server_protocol::GetAccountResponse;
use codex_app_server_protocol::GetAuthStatusParams;
use codex_app_server_protocol::GetAuthStatusResponse;
use codex_app_server_protocol::GetConversationSummaryParams;
use codex_app_server_protocol::GetConversationSummaryResponse;
use codex_app_server_protocol::GetUserAgentResponse;
use codex_app_server_protocol::GetUserSavedConfigResponse;
use codex_app_server_protocol::GitDiffToRemoteResponse;
use codex_app_server_protocol::GitInfo as ApiGitInfo;
use codex_app_server_protocol::InputItem as WireInputItem;
use codex_app_server_protocol::InterruptConversationParams;
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
use codex_app_server_protocol::ResumeConversationParams;
use codex_app_server_protocol::ResumeConversationResponse;
use codex_app_server_protocol::ReviewDelivery as ApiReviewDelivery;
use codex_app_server_protocol::ReviewStartParams;
use codex_app_server_protocol::ReviewStartResponse;
use codex_app_server_protocol::ReviewTarget as ApiReviewTarget;
use codex_app_server_protocol::SandboxMode;
use codex_app_server_protocol::SendUserMessageParams;
use codex_app_server_protocol::SendUserMessageResponse;
use codex_app_server_protocol::SendUserTurnParams;
use codex_app_server_protocol::SendUserTurnResponse;
use codex_app_server_protocol::ServerNotification;
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
use codex_app_server_protocol::TurnError;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStartedNotification;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInfoResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_app_server_protocol::UserSavedConfig;
use codex_app_server_protocol::build_turns_from_event_msgs;
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
use codex_core::features::Feature;
use codex_core::find_conversation_path_by_id_str;
use codex_core::git_info::git_diff_to_remote;
use codex_core::parse_cursor;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDelivery as CoreReviewDelivery;
use codex_core::protocol::ReviewRequest;
use codex_core::protocol::ReviewTarget as CoreReviewTarget;
use codex_core::protocol::SessionConfiguredEvent;
use codex_core::read_head_for_summary;
use codex_feedback::CodexFeedback;
use codex_login::ServerOptions as LoginServerOptions;
use codex_login::ShutdownHandle;
use codex_login::run_login_server;
use codex_protocol::ConversationId;
use codex_protocol::config_types::ForcedLoginMethod;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::GitInfo as CoreGitInfo;
use codex_protocol::protocol::RateLimitSnapshot as CoreRateLimitSnapshot;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::USER_MESSAGE_BEGIN;
use codex_protocol::user_input::UserInput as CoreInputItem;
use codex_utils_json_to_toml::json_to_toml;
use std::collections::HashMap;
use std::collections::HashSet;
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
pub(crate) type PendingInterrupts = Arc<Mutex<HashMap<ConversationId, PendingInterruptQueue>>>;

/// Per-conversation accumulation of the latest states e.g. error message while a turn runs.
#[derive(Default, Clone)]
pub(crate) struct TurnSummary {
    pub(crate) file_change_started: HashSet<String>,
    pub(crate) last_error: Option<TurnError>,
}

pub(crate) type TurnSummaryStore = Arc<Mutex<HashMap<ConversationId, TurnSummary>>>;

// Duration before a ChatGPT login attempt is abandoned.
const LOGIN_CHATGPT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
struct ActiveLogin {
    shutdown_handle: ShutdownHandle,
    login_id: Uuid,
}

impl Drop for ActiveLogin {
    fn drop(&mut self) {
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
    turn_summary_store: TurnSummaryStore,
    pending_fuzzy_searches: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    feedback: CodexFeedback,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ApiVersion {
    V1,
    V2,
}

impl CodexMessageProcessor {
    async fn conversation_from_thread_id(
        &self,
        thread_id: &str,
    ) -> Result<(ConversationId, Arc<CodexConversation>), JSONRPCErrorError> {
        // Resolve conversation id from v2 thread id string.
        let conversation_id =
            ConversationId::from_string(thread_id).map_err(|err| JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("invalid thread id: {err}"),
                data: None,
            })?;

        let conversation = self
            .conversation_manager
            .get_conversation(conversation_id)
            .await
            .map_err(|_| JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("conversation not found: {conversation_id}"),
                data: None,
            })?;

        Ok((conversation_id, conversation))
    }
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
            turn_summary_store: Arc::new(Mutex::new(HashMap::new())),
            pending_fuzzy_searches: Arc::new(Mutex::new(HashMap::new())),
            feedback,
        }
    }

    fn review_request_from_target(
        target: ApiReviewTarget,
    ) -> Result<(ReviewRequest, String), JSONRPCErrorError> {
        fn invalid_request(message: String) -> JSONRPCErrorError {
            JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message,
                data: None,
            }
        }

        let cleaned_target = match target {
            ApiReviewTarget::UncommittedChanges => ApiReviewTarget::UncommittedChanges,
            ApiReviewTarget::BaseBranch { branch } => {
                let branch = branch.trim().to_string();
                if branch.is_empty() {
                    return Err(invalid_request("branch must not be empty".to_string()));
                }
                ApiReviewTarget::BaseBranch { branch }
            }
            ApiReviewTarget::Commit { sha, title } => {
                let sha = sha.trim().to_string();
                if sha.is_empty() {
                    return Err(invalid_request("sha must not be empty".to_string()));
                }
                let title = title
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty());
                ApiReviewTarget::Commit { sha, title }
            }
            ApiReviewTarget::Custom { instructions } => {
                let trimmed = instructions.trim().to_string();
                if trimmed.is_empty() {
                    return Err(invalid_request(
                        "instructions must not be empty".to_string(),
                    ));
                }
                ApiReviewTarget::Custom {
                    instructions: trimmed,
                }
            }
        };

        let core_target = match cleaned_target {
            ApiReviewTarget::UncommittedChanges => CoreReviewTarget::UncommittedChanges,
            ApiReviewTarget::BaseBranch { branch } => CoreReviewTarget::BaseBranch { branch },
            ApiReviewTarget::Commit { sha, title } => CoreReviewTarget::Commit { sha, title },
            ApiReviewTarget::Custom { instructions } => CoreReviewTarget::Custom { instructions },
        };

        let hint = codex_core::review_prompts::user_facing_hint(&core_target);
        let review_request = ReviewRequest {
            target: core_target,
            user_facing_hint: Some(hint.clone()),
        };

        Ok((review_request, hint))
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
            ClientRequest::ReviewStart { request_id, params } => {
                self.review_start(request_id, params).await;
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
            ClientRequest::GetAccount { request_id, params } => {
                self.get_account(request_id, params).await;
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
            ClientRequest::OneOffCommandExec { request_id, params } => {
                self.exec_one_off_command(request_id, params).await;
            }
            ClientRequest::ExecOneOffCommand { request_id, params } => {
                self.exec_one_off_command(request_id, params.into()).await;
            }
            ClientRequest::ConfigRead { .. }
            | ClientRequest::ConfigValueWrite { .. }
            | ClientRequest::ConfigBatchWrite { .. } => {
                warn!("Config request reached CodexMessageProcessor unexpectedly");
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
                drop(active);
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
                            drop(existing);
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
                            drop(existing);
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
                drop(active);
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
                drop(active);
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

    async fn refresh_token_if_requested(&self, do_refresh: bool) {
        if do_refresh && let Err(err) = self.auth_manager.refresh_token().await {
            tracing::warn!("failed to refresh token whilte getting account: {err}");
        }
    }

    async fn get_auth_status(&self, request_id: RequestId, params: GetAuthStatusParams) {
        let include_token = params.include_token.unwrap_or(false);
        let do_refresh = params.refresh_token.unwrap_or(false);

        self.refresh_token_if_requested(do_refresh).await;

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

    async fn get_account(&self, request_id: RequestId, params: GetAccountParams) {
        let do_refresh = params.refresh_token;

        self.refresh_token_if_requested(do_refresh).await;

        // Whether auth is required for the active model provider.
        let requires_openai_auth = self.config.model_provider.requires_openai_auth;

        if !requires_openai_auth {
            let response = GetAccountResponse {
                account: None,
                requires_openai_auth,
            };
            self.outgoing.send_response(request_id, response).await;
            return;
        }

        let account = match self.auth_manager.auth() {
            Some(auth) => Some(match auth.mode {
                AuthMode::ApiKey => Account::ApiKey {},
                AuthMode::ChatGPT => {
                    let email = auth.get_account_email();
                    let plan_type = auth.account_plan_type();

                    match (email, plan_type) {
                        (Some(email), Some(plan_type)) => Account::Chatgpt { email, plan_type },
                        _ => {
                            let error = JSONRPCErrorError {
                                code: INVALID_REQUEST_ERROR_CODE,
                                message:
                                    "email and plan type are required for chatgpt authentication"
                                        .to_string(),
                                data: None,
                            };
                            self.outgoing.send_error(request_id, error).await;
                            return;
                        }
                    }
                }
            }),
            None => None,
        };

        let response = GetAccountResponse {
            account,
            requires_openai_auth,
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

    async fn exec_one_off_command(&self, request_id: RequestId, params: CommandExecParams) {
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
        let timeout_ms = params
            .timeout_ms
            .and_then(|timeout_ms| u64::try_from(timeout_ms).ok());
        let exec_params = ExecParams {
            command: params.command,
            cwd,
            expiration: timeout_ms.into(),
            env,
            with_escalated_permissions: None,
            justification: None,
            arg0: None,
        };

        let effective_policy = params
            .sandbox_policy
            .map(|policy| policy.to_core())
            .unwrap_or_else(|| self.config.sandbox_policy.clone());

        let codex_linux_sandbox_exe = self.config.codex_linux_sandbox_exe.clone();
        let outgoing = self.outgoing.clone();
        let req_id = request_id;
        let sandbox_cwd = self.config.cwd.clone();

        tokio::spawn(async move {
            match codex_core::exec::process_exec_tool_call(
                exec_params,
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
            cwd: cwd.clone().map(PathBuf::from),
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

        // Persist windows sandbox feature.
        // TODO: persist default config in general.
        let mut cli_overrides = cli_overrides.unwrap_or_default();
        if cfg!(windows) && self.config.features.enabled(Feature::WindowsSandbox) {
            cli_overrides.insert(
                "features.enable_experimental_windows_sandbox".to_string(),
                serde_json::json!(true),
            );
        }

        let config = match derive_config_from_params(overrides, Some(cli_overrides)).await {
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
        let overrides = self.build_thread_config_overrides(
            params.model,
            params.model_provider,
            params.cwd,
            params.approval_policy,
            params.sandbox,
            params.base_instructions,
            params.developer_instructions,
        );

        let config = match derive_config_from_params(overrides, params.config).await {
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
                let NewConversation {
                    conversation_id,
                    session_configured,
                    ..
                } = new_conv;
                let rollout_path = session_configured.rollout_path.clone();
                let fallback_provider = self.config.model_provider_id.as_str();

                // A bit hacky, but the summary contains a lot of useful information for the thread
                // that unfortunately does not get returned from conversation_manager.new_conversation().
                let thread = match read_summary_from_rollout(
                    rollout_path.as_path(),
                    fallback_provider,
                )
                .await
                {
                    Ok(summary) => summary_to_thread(summary),
                    Err(err) => {
                        self.send_internal_error(
                            request_id,
                            format!(
                                "failed to load rollout `{}` for conversation {conversation_id}: {err}",
                                rollout_path.display()
                            ),
                        )
                        .await;
                        return;
                    }
                };

                let SessionConfiguredEvent {
                    model,
                    model_provider_id,
                    cwd,
                    approval_policy,
                    sandbox_policy,
                    ..
                } = session_configured;
                let response = ThreadStartResponse {
                    thread: thread.clone(),
                    model,
                    model_provider: model_provider_id,
                    cwd,
                    approval_policy: approval_policy.into(),
                    sandbox: sandbox_policy.into(),
                    reasoning_effort: session_configured.reasoning_effort,
                };

                // Auto-attach a conversation listener when starting a thread.
                // Use the same behavior as the v1 API with experimental_raw_events=false.
                if let Err(err) = self
                    .attach_conversation_listener(conversation_id, false, ApiVersion::V2)
                    .await
                {
                    tracing::warn!(
                        "failed to attach listener for conversation {}: {}",
                        conversation_id,
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

    #[allow(clippy::too_many_arguments)]
    fn build_thread_config_overrides(
        &self,
        model: Option<String>,
        model_provider: Option<String>,
        cwd: Option<String>,
        approval_policy: Option<codex_app_server_protocol::AskForApproval>,
        sandbox: Option<SandboxMode>,
        base_instructions: Option<String>,
        developer_instructions: Option<String>,
    ) -> ConfigOverrides {
        ConfigOverrides {
            model,
            model_provider,
            cwd: cwd.map(PathBuf::from),
            approval_policy: approval_policy
                .map(codex_app_server_protocol::AskForApproval::to_core),
            sandbox_mode: sandbox.map(SandboxMode::to_core),
            codex_linux_sandbox_exe: self.codex_linux_sandbox_exe.clone(),
            base_instructions,
            developer_instructions,
            ..Default::default()
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

        let data = summaries.into_iter().map(summary_to_thread).collect();

        let response = ThreadListResponse { data, next_cursor };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn thread_resume(&mut self, request_id: RequestId, params: ThreadResumeParams) {
        let ThreadResumeParams {
            thread_id,
            history,
            path,
            model,
            model_provider,
            cwd,
            approval_policy,
            sandbox,
            config: cli_overrides,
            base_instructions,
            developer_instructions,
        } = params;

        let overrides_requested = model.is_some()
            || model_provider.is_some()
            || cwd.is_some()
            || approval_policy.is_some()
            || sandbox.is_some()
            || cli_overrides.is_some()
            || base_instructions.is_some()
            || developer_instructions.is_some();

        let config = if overrides_requested {
            let overrides = self.build_thread_config_overrides(
                model,
                model_provider,
                cwd,
                approval_policy,
                sandbox,
                base_instructions,
                developer_instructions,
            );
            match derive_config_from_params(overrides, cli_overrides).await {
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
            }
        } else {
            self.config.as_ref().clone()
        };

        let conversation_history = if let Some(history) = history {
            if history.is_empty() {
                self.send_invalid_request_error(
                    request_id,
                    "history must not be empty".to_string(),
                )
                .await;
                return;
            }
            InitialHistory::Forked(history.into_iter().map(RolloutItem::ResponseItem).collect())
        } else if let Some(path) = path {
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
        } else {
            let existing_conversation_id = match ConversationId::from_string(&thread_id) {
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
                &existing_conversation_id.to_string(),
            )
            .await
            {
                Ok(Some(p)) => p,
                Ok(None) => {
                    self.send_invalid_request_error(
                        request_id,
                        format!("no rollout found for conversation id {existing_conversation_id}"),
                    )
                    .await;
                    return;
                }
                Err(err) => {
                    self.send_invalid_request_error(
                        request_id,
                        format!(
                            "failed to locate conversation id {existing_conversation_id}: {err}"
                        ),
                    )
                    .await;
                    return;
                }
            };

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
        };

        let fallback_model_provider = config.model_provider_id.clone();

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
                let SessionConfiguredEvent {
                    rollout_path,
                    initial_messages,
                    ..
                } = session_configured;
                // Auto-attach a conversation listener when resuming a thread.
                if let Err(err) = self
                    .attach_conversation_listener(conversation_id, false, ApiVersion::V2)
                    .await
                {
                    tracing::warn!(
                        "failed to attach listener for conversation {}: {}",
                        conversation_id,
                        err.message
                    );
                }

                let mut thread = match read_summary_from_rollout(
                    rollout_path.as_path(),
                    fallback_model_provider.as_str(),
                )
                .await
                {
                    Ok(summary) => summary_to_thread(summary),
                    Err(err) => {
                        self.send_internal_error(
                            request_id,
                            format!(
                                "failed to load rollout `{}` for conversation {conversation_id}: {err}",
                                rollout_path.display()
                            ),
                        )
                        .await;
                        return;
                    }
                };
                thread.turns = initial_messages
                    .as_deref()
                    .map_or_else(Vec::new, build_turns_from_event_msgs);

                let response = ThreadResumeResponse {
                    thread,
                    model: session_configured.model,
                    model_provider: session_configured.model_provider_id,
                    cwd: session_configured.cwd,
                    approval_policy: session_configured.approval_policy.into(),
                    sandbox: session_configured.sandbox_policy.into(),
                    reasoning_effort: session_configured.reasoning_effort,
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
            .filter_map(|it| {
                let session_meta_line = it.head.first().and_then(|first| {
                    serde_json::from_value::<SessionMetaLine>(first.clone()).ok()
                })?;
                extract_conversation_summary(
                    it.path,
                    &it.head,
                    &session_meta_line.meta,
                    session_meta_line.git.as_ref(),
                    fallback_provider.as_str(),
                )
            })
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

                // Persist windows sandbox feature.
                let mut cli_overrides = cli_overrides.unwrap_or_default();
                if cfg!(windows) && self.config.features.enabled(Feature::WindowsSandbox) {
                    cli_overrides.insert(
                        "features.enable_experimental_windows_sandbox".to_string(),
                        serde_json::json!(true),
                    );
                }

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

                derive_config_from_params(overrides, Some(cli_overrides)).await
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

    async fn send_internal_error(&self, request_id: RequestId, message: String) {
        let error = JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
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
        let (_, conversation) = match self.conversation_from_thread_id(&params.thread_id).await {
            Ok(v) => v,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

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
                    items: vec![],
                    error: None,
                    status: TurnStatus::InProgress,
                };

                let response = TurnStartResponse { turn: turn.clone() };
                self.outgoing.send_response(request_id, response).await;

                // Emit v2 turn/started notification.
                let notif = TurnStartedNotification {
                    thread_id: params.thread_id,
                    turn,
                };
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

    fn build_review_turn(turn_id: String, display_text: &str) -> Turn {
        let items = if display_text.is_empty() {
            Vec::new()
        } else {
            vec![ThreadItem::UserMessage {
                id: turn_id.clone(),
                content: vec![V2UserInput::Text {
                    text: display_text.to_string(),
                }],
            }]
        };

        Turn {
            id: turn_id,
            items,
            error: None,
            status: TurnStatus::InProgress,
        }
    }

    async fn emit_review_started(
        &self,
        request_id: &RequestId,
        turn: Turn,
        parent_thread_id: String,
        review_thread_id: String,
    ) {
        let response = ReviewStartResponse {
            turn: turn.clone(),
            review_thread_id,
        };
        self.outgoing
            .send_response(request_id.clone(), response)
            .await;

        let notif = TurnStartedNotification {
            thread_id: parent_thread_id,
            turn,
        };
        self.outgoing
            .send_server_notification(ServerNotification::TurnStarted(notif))
            .await;
    }

    async fn start_inline_review(
        &self,
        request_id: &RequestId,
        parent_conversation: Arc<CodexConversation>,
        review_request: ReviewRequest,
        display_text: &str,
        parent_thread_id: String,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        let turn_id = parent_conversation
            .submit(Op::Review { review_request })
            .await;

        match turn_id {
            Ok(turn_id) => {
                let turn = Self::build_review_turn(turn_id, display_text);
                self.emit_review_started(
                    request_id,
                    turn,
                    parent_thread_id.clone(),
                    parent_thread_id,
                )
                .await;
                Ok(())
            }
            Err(err) => Err(JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to start review: {err}"),
                data: None,
            }),
        }
    }

    async fn start_detached_review(
        &mut self,
        request_id: &RequestId,
        parent_conversation_id: ConversationId,
        review_request: ReviewRequest,
        display_text: &str,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        let rollout_path = find_conversation_path_by_id_str(
            &self.config.codex_home,
            &parent_conversation_id.to_string(),
        )
        .await
        .map_err(|err| JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
            message: format!("failed to locate conversation id {parent_conversation_id}: {err}"),
            data: None,
        })?
        .ok_or_else(|| JSONRPCErrorError {
            code: INVALID_REQUEST_ERROR_CODE,
            message: format!("no rollout found for conversation id {parent_conversation_id}"),
            data: None,
        })?;

        let mut config = self.config.as_ref().clone();
        config.model = self.config.review_model.clone();

        let NewConversation {
            conversation_id,
            conversation,
            session_configured,
            ..
        } = self
            .conversation_manager
            .fork_conversation(usize::MAX, config, rollout_path)
            .await
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("error creating detached review conversation: {err}"),
                data: None,
            })?;

        if let Err(err) = self
            .attach_conversation_listener(conversation_id, false, ApiVersion::V2)
            .await
        {
            tracing::warn!(
                "failed to attach listener for review conversation {}: {}",
                conversation_id,
                err.message
            );
        }

        let rollout_path = conversation.rollout_path();
        let fallback_provider = self.config.model_provider_id.as_str();
        match read_summary_from_rollout(rollout_path.as_path(), fallback_provider).await {
            Ok(summary) => {
                let thread = summary_to_thread(summary);
                let notif = ThreadStartedNotification { thread };
                self.outgoing
                    .send_server_notification(ServerNotification::ThreadStarted(notif))
                    .await;
            }
            Err(err) => {
                tracing::warn!(
                    "failed to load summary for review conversation {}: {}",
                    session_configured.session_id,
                    err
                );
            }
        }

        let turn_id = conversation
            .submit(Op::Review { review_request })
            .await
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to start detached review turn: {err}"),
                data: None,
            })?;

        let turn = Self::build_review_turn(turn_id, display_text);
        let review_thread_id = conversation_id.to_string();
        self.emit_review_started(request_id, turn, review_thread_id.clone(), review_thread_id)
            .await;

        Ok(())
    }

    async fn review_start(&mut self, request_id: RequestId, params: ReviewStartParams) {
        let ReviewStartParams {
            thread_id,
            target,
            delivery,
        } = params;
        let (parent_conversation_id, parent_conversation) =
            match self.conversation_from_thread_id(&thread_id).await {
                Ok(v) => v,
                Err(error) => {
                    self.outgoing.send_error(request_id, error).await;
                    return;
                }
            };

        let (review_request, display_text) = match Self::review_request_from_target(target) {
            Ok(value) => value,
            Err(err) => {
                self.outgoing.send_error(request_id, err).await;
                return;
            }
        };

        let delivery = delivery.unwrap_or(ApiReviewDelivery::Inline).to_core();
        match delivery {
            CoreReviewDelivery::Inline => {
                if let Err(err) = self
                    .start_inline_review(
                        &request_id,
                        parent_conversation,
                        review_request,
                        display_text.as_str(),
                        thread_id.clone(),
                    )
                    .await
                {
                    self.outgoing.send_error(request_id, err).await;
                }
            }
            CoreReviewDelivery::Detached => {
                if let Err(err) = self
                    .start_detached_review(
                        &request_id,
                        parent_conversation_id,
                        review_request,
                        display_text.as_str(),
                    )
                    .await
                {
                    self.outgoing.send_error(request_id, err).await;
                }
            }
        }
    }

    async fn turn_interrupt(&mut self, request_id: RequestId, params: TurnInterruptParams) {
        let TurnInterruptParams { thread_id, .. } = params;

        let (conversation_id, conversation) =
            match self.conversation_from_thread_id(&thread_id).await {
                Ok(v) => v,
                Err(error) => {
                    self.outgoing.send_error(request_id, error).await;
                    return;
                }
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
            .attach_conversation_listener(conversation_id, experimental_raw_events, ApiVersion::V1)
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
        api_version: ApiVersion,
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
        let turn_summary_store = self.turn_summary_store.clone();
        let api_version_for_task = api_version;
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
                            turn_summary_store.clone(),
                            api_version_for_task,
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
        let session_source = self.conversation_manager.session_source();

        let upload_result = tokio::task::spawn_blocking(move || {
            let rollout_path_ref = validated_rollout_path.as_deref();
            snapshot.upload_feedback(
                &classification,
                reason.as_deref(),
                include_logs,
                rollout_path_ref,
                Some(session_source),
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

    let session_meta_line =
        serde_json::from_value::<SessionMetaLine>(first.clone()).map_err(|_| {
            IoError::other(format!(
                "rollout at {} does not start with session metadata",
                path.display()
            ))
        })?;
    let SessionMetaLine {
        meta: session_meta,
        git,
    } = session_meta_line;

    if let Some(summary) = extract_conversation_summary(
        path.to_path_buf(),
        &head,
        &session_meta,
        git.as_ref(),
        fallback_provider,
    ) {
        return Ok(summary);
    }

    let timestamp = if session_meta.timestamp.is_empty() {
        None
    } else {
        Some(session_meta.timestamp.clone())
    };
    let model_provider = session_meta
        .model_provider
        .clone()
        .unwrap_or_else(|| fallback_provider.to_string());
    let git_info = git.as_ref().map(map_git_info);

    Ok(ConversationSummary {
        conversation_id: session_meta.id,
        timestamp,
        path: path.to_path_buf(),
        preview: String::new(),
        model_provider,
        cwd: session_meta.cwd,
        cli_version: session_meta.cli_version,
        source: session_meta.source,
        git_info,
    })
}

fn extract_conversation_summary(
    path: PathBuf,
    head: &[serde_json::Value],
    session_meta: &SessionMeta,
    git: Option<&CoreGitInfo>,
    fallback_provider: &str,
) -> Option<ConversationSummary> {
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
        .clone()
        .unwrap_or_else(|| fallback_provider.to_string());
    let git_info = git.map(map_git_info);

    Some(ConversationSummary {
        conversation_id,
        timestamp,
        path,
        preview: preview.to_string(),
        model_provider,
        cwd: session_meta.cwd.clone(),
        cli_version: session_meta.cli_version.clone(),
        source: session_meta.source.clone(),
        git_info,
    })
}

fn map_git_info(git_info: &CoreGitInfo) -> ConversationGitInfo {
    ConversationGitInfo {
        sha: git_info.commit_hash.clone(),
        branch: git_info.branch.clone(),
        origin_url: git_info.repository_url.clone(),
    }
}

fn parse_datetime(timestamp: Option<&str>) -> Option<DateTime<Utc>> {
    timestamp.and_then(|ts| {
        chrono::DateTime::parse_from_rfc3339(ts)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Utc))
    })
}

fn summary_to_thread(summary: ConversationSummary) -> Thread {
    let ConversationSummary {
        conversation_id,
        path,
        preview,
        timestamp,
        model_provider,
        cwd,
        cli_version,
        source,
        git_info,
    } = summary;

    let created_at = parse_datetime(timestamp.as_deref());
    let git_info = git_info.map(|info| ApiGitInfo {
        sha: info.sha,
        branch: info.branch,
        origin_url: info.origin_url,
    });

    Thread {
        id: conversation_id.to_string(),
        preview,
        model_provider,
        created_at: created_at.map(|dt| dt.timestamp()).unwrap_or(0),
        path,
        cwd,
        cli_version,
        source: source.into(),
        git_info,
        turns: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use codex_protocol::protocol::SessionSource;
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

        let session_meta = serde_json::from_value::<SessionMeta>(head[0].clone())?;

        let summary =
            extract_conversation_summary(path.clone(), &head, &session_meta, None, "test-provider")
                .expect("summary");

        let expected = ConversationSummary {
            conversation_id,
            timestamp,
            path,
            preview: "Count to 5".to_string(),
            model_provider: "test-provider".to_string(),
            cwd: PathBuf::from("/"),
            cli_version: "0.0.0".to_string(),
            source: SessionSource::VSCode,
            git_info: None,
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
            cwd: PathBuf::new(),
            cli_version: String::new(),
            source: SessionSource::VSCode,
            git_info: None,
        };

        assert_eq!(summary, expected);
        Ok(())
    }
}
