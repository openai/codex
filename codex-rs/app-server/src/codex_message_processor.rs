use crate::bespoke_event_handling::apply_bespoke_event_handling;
use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use crate::fuzzy_file_search::FuzzyFileSearchSession;
use crate::fuzzy_file_search::run_fuzzy_file_search;
use crate::fuzzy_file_search::start_fuzzy_file_search_session;
use crate::models::supported_models;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::ConnectionRequestId;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::OutgoingNotification;
use crate::outgoing_message::ThreadScopedOutgoingMessageSender;
use crate::thread_status::ThreadWatchManager;
use chrono::DateTime;
use chrono::SecondsFormat;
use chrono::Utc;
use codex_app_server_protocol::Account;
use codex_app_server_protocol::AccountLoginCompletedNotification;
use codex_app_server_protocol::AccountUpdatedNotification;
use codex_app_server_protocol::AddConversationListenerParams;
use codex_app_server_protocol::AddConversationSubscriptionResponse;
use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::AppListUpdatedNotification;
use codex_app_server_protocol::AppsListParams;
use codex_app_server_protocol::AppsListResponse;
use codex_app_server_protocol::ArchiveConversationParams;
use codex_app_server_protocol::ArchiveConversationResponse;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::AuthStatusChangeNotification;
use codex_app_server_protocol::CancelLoginAccountParams;
use codex_app_server_protocol::CancelLoginAccountResponse;
use codex_app_server_protocol::CancelLoginAccountStatus;
use codex_app_server_protocol::CancelLoginChatGptResponse;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CollaborationModeListParams;
use codex_app_server_protocol::CollaborationModeListResponse;
use codex_app_server_protocol::CommandExecParams;
use codex_app_server_protocol::ConversationGitInfo;
use codex_app_server_protocol::ConversationSummary;
use codex_app_server_protocol::DynamicToolSpec as ApiDynamicToolSpec;
use codex_app_server_protocol::ExecOneOffCommandResponse;
use codex_app_server_protocol::ExperimentalFeature as ApiExperimentalFeature;
use codex_app_server_protocol::ExperimentalFeatureListParams;
use codex_app_server_protocol::ExperimentalFeatureListResponse;
use codex_app_server_protocol::ExperimentalFeatureStage as ApiExperimentalFeatureStage;
use codex_app_server_protocol::FeedbackUploadParams;
use codex_app_server_protocol::FeedbackUploadResponse;
use codex_app_server_protocol::ForkConversationParams;
use codex_app_server_protocol::ForkConversationResponse;
use codex_app_server_protocol::FuzzyFileSearchParams;
use codex_app_server_protocol::FuzzyFileSearchResponse;
use codex_app_server_protocol::FuzzyFileSearchSessionStartParams;
use codex_app_server_protocol::FuzzyFileSearchSessionStartResponse;
use codex_app_server_protocol::FuzzyFileSearchSessionStopParams;
use codex_app_server_protocol::FuzzyFileSearchSessionStopResponse;
use codex_app_server_protocol::FuzzyFileSearchSessionUpdateParams;
use codex_app_server_protocol::FuzzyFileSearchSessionUpdateResponse;
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
use codex_app_server_protocol::HazelnutScope as ApiHazelnutScope;
use codex_app_server_protocol::InputItem as WireInputItem;
use codex_app_server_protocol::InterruptConversationParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ListConversationsParams;
use codex_app_server_protocol::ListConversationsResponse;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::ListMcpServerStatusResponse;
use codex_app_server_protocol::LoginAccountParams;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::LoginApiKeyParams;
use codex_app_server_protocol::LoginApiKeyResponse;
use codex_app_server_protocol::LoginChatGptCompleteNotification;
use codex_app_server_protocol::LoginChatGptResponse;
use codex_app_server_protocol::LogoutAccountResponse;
use codex_app_server_protocol::LogoutChatGptResponse;
use codex_app_server_protocol::McpServerOauthLoginCompletedNotification;
use codex_app_server_protocol::McpServerOauthLoginParams;
use codex_app_server_protocol::McpServerOauthLoginResponse;
use codex_app_server_protocol::McpServerRefreshResponse;
use codex_app_server_protocol::McpServerStatus;
use codex_app_server_protocol::MockExperimentalMethodParams;
use codex_app_server_protocol::MockExperimentalMethodResponse;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelListResponse;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::NewConversationResponse;
use codex_app_server_protocol::ProductSurface as ApiProductSurface;
use codex_app_server_protocol::RemoveConversationListenerParams;
use codex_app_server_protocol::RemoveConversationSubscriptionResponse;
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
use codex_app_server_protocol::SkillsConfigWriteParams;
use codex_app_server_protocol::SkillsConfigWriteResponse;
use codex_app_server_protocol::SkillsListParams;
use codex_app_server_protocol::SkillsListResponse;
use codex_app_server_protocol::SkillsRemoteReadParams;
use codex_app_server_protocol::SkillsRemoteReadResponse;
use codex_app_server_protocol::SkillsRemoteWriteParams;
use codex_app_server_protocol::SkillsRemoteWriteResponse;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadArchivedNotification;
use codex_app_server_protocol::ThreadBackgroundTerminalsCleanParams;
use codex_app_server_protocol::ThreadBackgroundTerminalsCleanResponse;
use codex_app_server_protocol::ThreadCompactStartParams;
use codex_app_server_protocol::ThreadCompactStartResponse;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadRollbackParams;
use codex_app_server_protocol::ThreadSetNameParams;
use codex_app_server_protocol::ThreadSetNameResponse;
use codex_app_server_protocol::ThreadSortKey;
use codex_app_server_protocol::ThreadSourceKind;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStartedNotification;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::ThreadUnarchiveParams;
use codex_app_server_protocol::ThreadUnarchiveResponse;
use codex_app_server_protocol::ThreadUnarchivedNotification;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStartedNotification;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::TurnSteerParams;
use codex_app_server_protocol::TurnSteerResponse;
use codex_app_server_protocol::UserInfoResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_app_server_protocol::UserSavedConfig;
use codex_app_server_protocol::WindowsSandboxSetupCompletedNotification;
use codex_app_server_protocol::WindowsSandboxSetupMode;
use codex_app_server_protocol::WindowsSandboxSetupStartParams;
use codex_app_server_protocol::WindowsSandboxSetupStartResponse;
use codex_app_server_protocol::build_turns_from_rollout_items;
use codex_backend_client::Client as BackendClient;
use codex_chatgpt::connectors;
use codex_cloud_requirements::cloud_requirements_loader;
use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::CodexThread;
use codex_core::Cursor as RolloutCursor;
use codex_core::InitialHistory;
use codex_core::NewThread;
use codex_core::RolloutRecorder;
use codex_core::SessionMeta;
use codex_core::SteerInputError;
use codex_core::ThreadConfigSnapshot;
use codex_core::ThreadManager;
use codex_core::ThreadSortKey as CoreThreadSortKey;
use codex_core::auth::AuthMode as CoreAuthMode;
use codex_core::auth::CLIENT_ID;
use codex_core::auth::login_with_api_key;
use codex_core::auth::login_with_chatgpt_auth_tokens;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::ConfigService;
use codex_core::config::edit::ConfigEdit;
use codex_core::config::edit::ConfigEditsBuilder;
use codex_core::config::types::McpServerTransportConfig;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::default_client::get_codex_user_agent;
use codex_core::default_client::set_default_client_residency_requirement;
use codex_core::error::CodexErr;
use codex_core::exec::ExecParams;
use codex_core::exec_env::create_env;
use codex_core::features::FEATURES;
use codex_core::features::Feature;
use codex_core::features::Stage;
use codex_core::find_archived_thread_path_by_id_str;
use codex_core::find_thread_path_by_id_str;
use codex_core::git_info::git_diff_to_remote;
use codex_core::mcp::collect_mcp_snapshot;
use codex_core::mcp::group_tools_by_server;
use codex_core::parse_cursor;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDelivery as CoreReviewDelivery;
use codex_core::protocol::ReviewRequest;
use codex_core::protocol::ReviewTarget as CoreReviewTarget;
use codex_core::protocol::SessionConfiguredEvent;
use codex_core::read_head_for_summary;
use codex_core::read_session_meta_line;
use codex_core::rollout_date_parts;
use codex_core::sandboxing::SandboxPermissions;
use codex_core::skills::remote::export_remote_skill;
use codex_core::skills::remote::list_remote_skills;
use codex_core::state_db::StateDbHandle;
use codex_core::state_db::get_state_db;
use codex_core::windows_sandbox::WindowsSandboxLevelExt;
use codex_core::windows_sandbox::WindowsSandboxSetupMode as CoreWindowsSandboxSetupMode;
use codex_core::windows_sandbox::WindowsSandboxSetupRequest;
use codex_feedback::CodexFeedback;
use codex_login::ServerOptions as LoginServerOptions;
use codex_login::ShutdownHandle;
use codex_login::run_login_server;
use codex_protocol::ThreadId;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ForcedLoginMethod;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::dynamic_tools::DynamicToolSpec as CoreDynamicToolSpec;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::GitInfo as CoreGitInfo;
use codex_protocol::protocol::McpAuthStatus as CoreMcpAuthStatus;
use codex_protocol::protocol::McpServerRefreshConfig;
use codex_protocol::protocol::RateLimitSnapshot as CoreRateLimitSnapshot;
use codex_protocol::protocol::RemoteSkillHazelnutScope;
use codex_protocol::protocol::RemoteSkillProductSurface;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::USER_MESSAGE_BEGIN;
use codex_protocol::user_input::UserInput as CoreInputItem;
use codex_rmcp_client::perform_oauth_login_return_url;
use codex_utils_json_to_toml::json_to_toml;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::FileTimes;
use std::fs::OpenOptions;
use std::io::Error as IoError;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::SystemTime;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use tokio::sync::oneshot;
use toml::Value as TomlValue;
use tracing::error;
use tracing::info;
use tracing::warn;
use uuid::Uuid;

use crate::filters::compute_source_filters;
use crate::filters::source_kind_matches;
use crate::thread_state::ThreadState;
use crate::thread_state::ThreadStateManager;

const THREAD_LIST_DEFAULT_LIMIT: usize = 25;
const THREAD_LIST_MAX_LIMIT: usize = 100;

struct ThreadListFilters {
    model_providers: Option<Vec<String>>,
    source_kinds: Option<Vec<ThreadSourceKind>>,
    archived: bool,
    cwd: Option<PathBuf>,
}

// Duration before a ChatGPT login attempt is abandoned.
const LOGIN_CHATGPT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const APP_LIST_LOAD_TIMEOUT: Duration = Duration::from_secs(90);
struct ActiveLogin {
    shutdown_handle: ShutdownHandle,
    login_id: Uuid,
}

#[derive(Clone, Copy, Debug)]
enum CancelLoginError {
    NotFound(Uuid),
}

enum AppListLoadResult {
    Accessible(Result<Vec<AppInfo>, String>),
    Directory(Result<Vec<AppInfo>, String>),
}

fn convert_remote_scope(scope: ApiHazelnutScope) -> RemoteSkillHazelnutScope {
    match scope {
        ApiHazelnutScope::WorkspaceShared => RemoteSkillHazelnutScope::WorkspaceShared,
        ApiHazelnutScope::AllShared => RemoteSkillHazelnutScope::AllShared,
        ApiHazelnutScope::Personal => RemoteSkillHazelnutScope::Personal,
        ApiHazelnutScope::Example => RemoteSkillHazelnutScope::Example,
    }
}

fn convert_remote_product_surface(product_surface: ApiProductSurface) -> RemoteSkillProductSurface {
    match product_surface {
        ApiProductSurface::Chatgpt => RemoteSkillProductSurface::Chatgpt,
        ApiProductSurface::Codex => RemoteSkillProductSurface::Codex,
        ApiProductSurface::Api => RemoteSkillProductSurface::Api,
        ApiProductSurface::Atlas => RemoteSkillProductSurface::Atlas,
    }
}

impl Drop for ActiveLogin {
    fn drop(&mut self) {
        self.shutdown_handle.shutdown();
    }
}

/// Handles JSON-RPC messages for Codex threads (and legacy conversation APIs).
pub(crate) struct CodexMessageProcessor {
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    codex_linux_sandbox_exe: Option<PathBuf>,
    config: Arc<Config>,
    cli_overrides: Vec<(String, TomlValue)>,
    cloud_requirements: Arc<RwLock<CloudRequirementsLoader>>,
    active_login: Arc<Mutex<Option<ActiveLogin>>>,
    thread_state_manager: ThreadStateManager,
    thread_watch_manager: ThreadWatchManager,
    pending_fuzzy_searches: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    fuzzy_search_sessions: Arc<Mutex<HashMap<String, FuzzyFileSearchSession>>>,
    feedback: CodexFeedback,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ApiVersion {
    V1,
    #[default]
    V2,
}

pub(crate) struct CodexMessageProcessorArgs {
    pub(crate) auth_manager: Arc<AuthManager>,
    pub(crate) thread_manager: Arc<ThreadManager>,
    pub(crate) outgoing: Arc<OutgoingMessageSender>,
    pub(crate) codex_linux_sandbox_exe: Option<PathBuf>,
    pub(crate) config: Arc<Config>,
    pub(crate) cli_overrides: Vec<(String, TomlValue)>,
    pub(crate) cloud_requirements: Arc<RwLock<CloudRequirementsLoader>>,
    pub(crate) feedback: CodexFeedback,
}

impl CodexMessageProcessor {
    async fn load_thread(
        &self,
        thread_id: &str,
    ) -> Result<(ThreadId, Arc<CodexThread>), JSONRPCErrorError> {
        // Resolve the core conversation handle from a v2 thread id string.
        let thread_id = ThreadId::from_string(thread_id).map_err(|err| JSONRPCErrorError {
            code: INVALID_REQUEST_ERROR_CODE,
            message: format!("invalid thread id: {err}"),
            data: None,
        })?;

        let thread = self
            .thread_manager
            .get_thread(thread_id)
            .await
            .map_err(|_| JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("thread not found: {thread_id}"),
                data: None,
            })?;

        Ok((thread_id, thread))
    }
    pub fn new(args: CodexMessageProcessorArgs) -> Self {
        let CodexMessageProcessorArgs {
            auth_manager,
            thread_manager,
            outgoing,
            codex_linux_sandbox_exe,
            config,
            cli_overrides,
            cloud_requirements,
            feedback,
        } = args;
        Self {
            auth_manager,
            thread_manager,
            outgoing: outgoing.clone(),
            codex_linux_sandbox_exe,
            config,
            cli_overrides,
            cloud_requirements,
            active_login: Arc::new(Mutex::new(None)),
            thread_state_manager: ThreadStateManager::new(),
            thread_watch_manager: ThreadWatchManager::new_with_outgoing(outgoing),
            pending_fuzzy_searches: Arc::new(Mutex::new(HashMap::new())),
            fuzzy_search_sessions: Arc::new(Mutex::new(HashMap::new())),
            feedback,
        }
    }

    async fn load_latest_config(&self) -> Result<Config, JSONRPCErrorError> {
        let cloud_requirements = self.current_cloud_requirements();
        codex_core::config::ConfigBuilder::default()
            .cli_overrides(self.cli_overrides.clone())
            .cloud_requirements(cloud_requirements)
            .build()
            .await
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to reload config: {err}"),
                data: None,
            })
    }

    fn current_cloud_requirements(&self) -> CloudRequirementsLoader {
        self.cloud_requirements
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// If a client sends `developer_instructions: null` during a mode switch,
    /// use the built-in instructions for that mode.
    fn normalize_turn_start_collaboration_mode(
        &self,
        mut collaboration_mode: CollaborationMode,
    ) -> CollaborationMode {
        if collaboration_mode.settings.developer_instructions.is_none()
            && let Some(instructions) = self
                .thread_manager
                .list_collaboration_modes()
                .into_iter()
                .find(|preset| preset.mode == Some(collaboration_mode.mode))
                .and_then(|preset| preset.developer_instructions.flatten())
                .filter(|instructions| !instructions.is_empty())
        {
            collaboration_mode.settings.developer_instructions = Some(instructions);
        }

        collaboration_mode
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

    pub async fn process_request(&mut self, connection_id: ConnectionId, request: ClientRequest) {
        let to_connection_request_id = |request_id| ConnectionRequestId {
            connection_id,
            request_id,
        };

        match request {
            ClientRequest::Initialize { .. } => {
                panic!("Initialize should be handled in MessageProcessor");
            }
            // === v2 Thread/Turn APIs ===
            ClientRequest::ThreadStart { request_id, params } => {
                self.thread_start(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ThreadResume { request_id, params } => {
                self.thread_resume(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ThreadFork { request_id, params } => {
                self.thread_fork(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ThreadArchive { request_id, params } => {
                self.thread_archive(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ThreadSetName { request_id, params } => {
                self.thread_set_name(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ThreadUnarchive { request_id, params } => {
                self.thread_unarchive(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ThreadCompactStart { request_id, params } => {
                self.thread_compact_start(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ThreadBackgroundTerminalsClean { request_id, params } => {
                self.thread_background_terminals_clean(
                    to_connection_request_id(request_id),
                    params,
                )
                .await;
            }
            ClientRequest::ThreadRollback { request_id, params } => {
                self.thread_rollback(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ThreadList { request_id, params } => {
                self.thread_list(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ThreadLoadedList { request_id, params } => {
                self.thread_loaded_list(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ThreadRead { request_id, params } => {
                self.thread_read(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::SkillsList { request_id, params } => {
                self.skills_list(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::SkillsRemoteList { request_id, params } => {
                self.skills_remote_list(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::SkillsRemoteExport { request_id, params } => {
                self.skills_remote_export(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::AppsList { request_id, params } => {
                self.apps_list(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::SkillsConfigWrite { request_id, params } => {
                self.skills_config_write(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::TurnStart { request_id, params } => {
                self.turn_start(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::TurnSteer { request_id, params } => {
                self.turn_steer(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::TurnInterrupt { request_id, params } => {
                self.turn_interrupt(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ReviewStart { request_id, params } => {
                self.review_start(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::NewConversation { request_id, params } => {
                // Do not tokio::spawn() to process new_conversation()
                // asynchronously because we need to ensure the conversation is
                // created before processing any subsequent messages.
                self.process_new_conversation(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::GetConversationSummary { request_id, params } => {
                self.get_thread_summary(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ListConversations { request_id, params } => {
                self.handle_list_conversations(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ModelList { request_id, params } => {
                let outgoing = self.outgoing.clone();
                let thread_manager = self.thread_manager.clone();
                let request_id = to_connection_request_id(request_id);

                tokio::spawn(async move {
                    Self::list_models(outgoing, thread_manager, request_id, params).await;
                });
            }
            ClientRequest::ExperimentalFeatureList { request_id, params } => {
                self.experimental_feature_list(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::CollaborationModeList { request_id, params } => {
                let outgoing = self.outgoing.clone();
                let thread_manager = self.thread_manager.clone();
                let request_id = to_connection_request_id(request_id);

                tokio::spawn(async move {
                    Self::list_collaboration_modes(outgoing, thread_manager, request_id, params)
                        .await;
                });
            }
            ClientRequest::MockExperimentalMethod { request_id, params } => {
                self.mock_experimental_method(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::McpServerOauthLogin { request_id, params } => {
                self.mcp_server_oauth_login(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::McpServerRefresh { request_id, params } => {
                self.mcp_server_refresh(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::McpServerStatusList { request_id, params } => {
                self.list_mcp_server_status(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::WindowsSandboxSetupStart { request_id, params } => {
                self.windows_sandbox_setup_start(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::LoginAccount { request_id, params } => {
                self.login_v2(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::LogoutAccount {
                request_id,
                params: _,
            } => {
                self.logout_v2(to_connection_request_id(request_id)).await;
            }
            ClientRequest::CancelLoginAccount { request_id, params } => {
                self.cancel_login_v2(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::GetAccount { request_id, params } => {
                self.get_account(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ResumeConversation { request_id, params } => {
                self.handle_resume_conversation(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ForkConversation { request_id, params } => {
                self.handle_fork_conversation(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ArchiveConversation { request_id, params } => {
                self.archive_conversation(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::SendUserMessage { request_id, params } => {
                self.send_user_message(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::SendUserTurn { request_id, params } => {
                self.send_user_turn(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::InterruptConversation { request_id, params } => {
                self.interrupt_conversation(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::AddConversationListener { request_id, params } => {
                self.add_conversation_listener(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::RemoveConversationListener { request_id, params } => {
                self.remove_thread_listener(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::GitDiffToRemote { request_id, params } => {
                self.git_diff_to_origin(to_connection_request_id(request_id), params.cwd)
                    .await;
            }
            ClientRequest::LoginApiKey { request_id, params } => {
                self.login_api_key_v1(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::LoginChatGpt {
                request_id,
                params: _,
            } => {
                self.login_chatgpt_v1(to_connection_request_id(request_id))
                    .await;
            }
            ClientRequest::CancelLoginChatGpt { request_id, params } => {
                self.cancel_login_chatgpt(to_connection_request_id(request_id), params.login_id)
                    .await;
            }
            ClientRequest::LogoutChatGpt {
                request_id,
                params: _,
            } => {
                self.logout_v1(to_connection_request_id(request_id)).await;
            }
            ClientRequest::GetAuthStatus { request_id, params } => {
                self.get_auth_status(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::GetUserSavedConfig {
                request_id,
                params: _,
            } => {
                self.get_user_saved_config(to_connection_request_id(request_id))
                    .await;
            }
            ClientRequest::SetDefaultModel { request_id, params } => {
                self.set_default_model(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::GetUserAgent {
                request_id,
                params: _,
            } => {
                self.get_user_agent(to_connection_request_id(request_id))
                    .await;
            }
            ClientRequest::UserInfo {
                request_id,
                params: _,
            } => {
                self.get_user_info(to_connection_request_id(request_id))
                    .await;
            }
            ClientRequest::FuzzyFileSearch { request_id, params } => {
                self.fuzzy_file_search(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::FuzzyFileSearchSessionStart { request_id, params } => {
                self.fuzzy_file_search_session_start(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::FuzzyFileSearchSessionUpdate { request_id, params } => {
                self.fuzzy_file_search_session_update(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::FuzzyFileSearchSessionStop { request_id, params } => {
                self.fuzzy_file_search_session_stop(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::OneOffCommandExec { request_id, params } => {
                self.exec_one_off_command(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ExecOneOffCommand { request_id, params } => {
                self.exec_one_off_command(to_connection_request_id(request_id), params.into())
                    .await;
            }
            ClientRequest::ConfigRead { .. }
            | ClientRequest::ConfigValueWrite { .. }
            | ClientRequest::ConfigBatchWrite { .. } => {
                warn!("Config request reached CodexMessageProcessor unexpectedly");
            }
            ClientRequest::ConfigRequirementsRead { .. } => {
                warn!("ConfigRequirementsRead request reached CodexMessageProcessor unexpectedly");
            }
            ClientRequest::GetAccountRateLimits {
                request_id,
                params: _,
            } => {
                self.get_account_rate_limits(to_connection_request_id(request_id))
                    .await;
            }
            ClientRequest::FeedbackUpload { request_id, params } => {
                self.upload_feedback(to_connection_request_id(request_id), params)
                    .await;
            }
        }
    }

    async fn login_v2(&mut self, request_id: ConnectionRequestId, params: LoginAccountParams) {
        match params {
            LoginAccountParams::ApiKey { api_key } => {
                self.login_api_key_v2(request_id, LoginApiKeyParams { api_key })
                    .await;
            }
            LoginAccountParams::Chatgpt => {
                self.login_chatgpt_v2(request_id).await;
            }
            LoginAccountParams::ChatgptAuthTokens {
                access_token,
                chatgpt_account_id,
                chatgpt_plan_type,
            } => {
                self.login_chatgpt_auth_tokens(
                    request_id,
                    access_token,
                    chatgpt_account_id,
                    chatgpt_plan_type,
                )
                .await;
            }
        }
    }

    fn external_auth_active_error(&self) -> JSONRPCErrorError {
        JSONRPCErrorError {
            code: INVALID_REQUEST_ERROR_CODE,
            message: "External auth is active. Use account/login/start (chatgptAuthTokens) to update it or account/logout to clear it."
                .to_string(),
            data: None,
        }
    }

    async fn login_api_key_common(
        &mut self,
        params: &LoginApiKeyParams,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        if self.auth_manager.is_external_auth_active() {
            return Err(self.external_auth_active_error());
        }

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

    async fn login_api_key_v1(
        &mut self,
        request_id: ConnectionRequestId,
        params: LoginApiKeyParams,
    ) {
        match self.login_api_key_common(&params).await {
            Ok(()) => {
                self.outgoing
                    .send_response(request_id, LoginApiKeyResponse {})
                    .await;

                let payload = AuthStatusChangeNotification {
                    auth_method: self
                        .auth_manager
                        .auth_cached()
                        .as_ref()
                        .map(CodexAuth::api_auth_mode),
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

    async fn login_api_key_v2(
        &mut self,
        request_id: ConnectionRequestId,
        params: LoginApiKeyParams,
    ) {
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
                    auth_mode: self
                        .auth_manager
                        .auth_cached()
                        .as_ref()
                        .map(CodexAuth::api_auth_mode),
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

        if self.auth_manager.is_external_auth_active() {
            return Err(self.external_auth_active_error());
        }

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
    async fn login_chatgpt_v1(&mut self, request_id: ConnectionRequestId) {
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
                    let cloud_requirements = self.cloud_requirements.clone();
                    let chatgpt_base_url = self.config.chatgpt_base_url.clone();
                    let codex_home = self.config.codex_home.clone();
                    let cli_overrides = self.cli_overrides.clone();
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
                            replace_cloud_requirements_loader(
                                cloud_requirements.as_ref(),
                                auth_manager.clone(),
                                chatgpt_base_url,
                                codex_home,
                            );
                            sync_default_client_residency_requirement(
                                &cli_overrides,
                                cloud_requirements.as_ref(),
                            )
                            .await;

                            // Notify clients with the actual current auth mode.
                            let current_auth_method = auth_manager
                                .auth_cached()
                                .as_ref()
                                .map(CodexAuth::api_auth_mode);
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

    async fn login_chatgpt_v2(&mut self, request_id: ConnectionRequestId) {
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
                    let cloud_requirements = self.cloud_requirements.clone();
                    let chatgpt_base_url = self.config.chatgpt_base_url.clone();
                    let codex_home = self.config.codex_home.clone();
                    let cli_overrides = self.cli_overrides.clone();
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
                            replace_cloud_requirements_loader(
                                cloud_requirements.as_ref(),
                                auth_manager.clone(),
                                chatgpt_base_url,
                                codex_home,
                            );
                            sync_default_client_residency_requirement(
                                &cli_overrides,
                                cloud_requirements.as_ref(),
                            )
                            .await;

                            // Notify clients with the actual current auth mode.
                            let current_auth_method = auth_manager
                                .auth_cached()
                                .as_ref()
                                .map(CodexAuth::api_auth_mode);
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
    ) -> std::result::Result<(), CancelLoginError> {
        let mut guard = self.active_login.lock().await;
        if guard.as_ref().map(|l| l.login_id) == Some(login_id) {
            if let Some(active) = guard.take() {
                drop(active);
            }
            Ok(())
        } else {
            Err(CancelLoginError::NotFound(login_id))
        }
    }

    async fn cancel_login_chatgpt(&mut self, request_id: ConnectionRequestId, login_id: Uuid) {
        match self.cancel_login_chatgpt_common(login_id).await {
            Ok(()) => {
                self.outgoing
                    .send_response(request_id, CancelLoginChatGptResponse {})
                    .await;
            }
            Err(CancelLoginError::NotFound(missing_login_id)) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("login id not found: {missing_login_id}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn cancel_login_v2(
        &mut self,
        request_id: ConnectionRequestId,
        params: CancelLoginAccountParams,
    ) {
        let login_id = params.login_id;
        match Uuid::parse_str(&login_id) {
            Ok(uuid) => {
                let status = match self.cancel_login_chatgpt_common(uuid).await {
                    Ok(()) => CancelLoginAccountStatus::Canceled,
                    Err(CancelLoginError::NotFound(_)) => CancelLoginAccountStatus::NotFound,
                };
                let response = CancelLoginAccountResponse { status };
                self.outgoing.send_response(request_id, response).await;
            }
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

    async fn login_chatgpt_auth_tokens(
        &mut self,
        request_id: ConnectionRequestId,
        access_token: String,
        chatgpt_account_id: String,
        chatgpt_plan_type: Option<String>,
    ) {
        if matches!(
            self.config.forced_login_method,
            Some(ForcedLoginMethod::Api)
        ) {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: "External ChatGPT auth is disabled. Use API key login instead."
                    .to_string(),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        }

        // Cancel any active login attempt to avoid persisting managed auth state.
        {
            let mut guard = self.active_login.lock().await;
            if let Some(active) = guard.take() {
                drop(active);
            }
        }

        if let Some(expected_workspace) = self.config.forced_chatgpt_workspace_id.as_deref()
            && chatgpt_account_id != expected_workspace
        {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!(
                    "External auth must use workspace {expected_workspace}, but received {chatgpt_account_id:?}."
                ),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        }

        if let Err(err) = login_with_chatgpt_auth_tokens(
            &self.config.codex_home,
            &access_token,
            &chatgpt_account_id,
            chatgpt_plan_type.as_deref(),
        ) {
            let error = JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to set external auth: {err}"),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        }
        self.auth_manager.reload();
        replace_cloud_requirements_loader(
            self.cloud_requirements.as_ref(),
            self.auth_manager.clone(),
            self.config.chatgpt_base_url.clone(),
            self.config.codex_home.clone(),
        );
        sync_default_client_residency_requirement(
            &self.cli_overrides,
            self.cloud_requirements.as_ref(),
        )
        .await;

        self.outgoing
            .send_response(request_id, LoginAccountResponse::ChatgptAuthTokens {})
            .await;

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
            auth_mode: self.auth_manager.get_api_auth_mode(),
        };
        self.outgoing
            .send_server_notification(ServerNotification::AccountUpdated(payload_v2))
            .await;
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
        Ok(self
            .auth_manager
            .auth_cached()
            .as_ref()
            .map(CodexAuth::api_auth_mode))
    }

    async fn logout_v1(&mut self, request_id: ConnectionRequestId) {
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

    async fn logout_v2(&mut self, request_id: ConnectionRequestId) {
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
        if self.auth_manager.is_external_auth_active() {
            return;
        }
        if do_refresh && let Err(err) = self.auth_manager.refresh_token().await {
            tracing::warn!("failed to refresh token while getting account: {err}");
        }
    }

    async fn get_auth_status(&self, request_id: ConnectionRequestId, params: GetAuthStatusParams) {
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
            match self.auth_manager.auth().await {
                Some(auth) => {
                    let auth_mode = auth.api_auth_mode();
                    let (reported_auth_method, token_opt) = match auth.get_token() {
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

    async fn get_account(&self, request_id: ConnectionRequestId, params: GetAccountParams) {
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

        let account = match self.auth_manager.auth_cached() {
            Some(auth) => match auth.auth_mode() {
                CoreAuthMode::ApiKey => Some(Account::ApiKey {}),
                CoreAuthMode::Chatgpt => {
                    let email = auth.get_account_email();
                    let plan_type = auth.account_plan_type();

                    match (email, plan_type) {
                        (Some(email), Some(plan_type)) => {
                            Some(Account::Chatgpt { email, plan_type })
                        }
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
            },
            None => None,
        };

        let response = GetAccountResponse {
            account,
            requires_openai_auth,
        };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn get_user_agent(&self, request_id: ConnectionRequestId) {
        let user_agent = get_codex_user_agent();
        let response = GetUserAgentResponse { user_agent };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn get_account_rate_limits(&self, request_id: ConnectionRequestId) {
        match self.fetch_account_rate_limits().await {
            Ok((rate_limits, rate_limits_by_limit_id)) => {
                let response = GetAccountRateLimitsResponse {
                    rate_limits: rate_limits.into(),
                    rate_limits_by_limit_id: Some(
                        rate_limits_by_limit_id
                            .into_iter()
                            .map(|(limit_id, snapshot)| (limit_id, snapshot.into()))
                            .collect(),
                    ),
                };
                self.outgoing.send_response(request_id, response).await;
            }
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn fetch_account_rate_limits(
        &self,
    ) -> Result<
        (
            CoreRateLimitSnapshot,
            HashMap<String, CoreRateLimitSnapshot>,
        ),
        JSONRPCErrorError,
    > {
        let Some(auth) = self.auth_manager.auth().await else {
            return Err(JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: "codex account authentication required to read rate limits".to_string(),
                data: None,
            });
        };

        if !auth.is_chatgpt_auth() {
            return Err(JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: "chatgpt authentication required to read rate limits".to_string(),
                data: None,
            });
        }

        let client = BackendClient::from_auth(self.config.chatgpt_base_url.clone(), &auth)
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to construct backend client: {err}"),
                data: None,
            })?;

        let snapshots = client
            .get_rate_limits_many()
            .await
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to fetch codex rate limits: {err}"),
                data: None,
            })?;
        if snapshots.is_empty() {
            return Err(JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: "failed to fetch codex rate limits: no snapshots returned".to_string(),
                data: None,
            });
        }

        let rate_limits_by_limit_id: HashMap<String, CoreRateLimitSnapshot> = snapshots
            .iter()
            .cloned()
            .map(|snapshot| {
                let limit_id = snapshot
                    .limit_id
                    .clone()
                    .unwrap_or_else(|| "codex".to_string());
                (limit_id, snapshot)
            })
            .collect();

        let primary = snapshots
            .iter()
            .find(|snapshot| snapshot.limit_id.as_deref() == Some("codex"))
            .cloned()
            .unwrap_or_else(|| snapshots[0].clone());

        Ok((primary, rate_limits_by_limit_id))
    }

    async fn get_user_saved_config(&self, request_id: ConnectionRequestId) {
        let service = ConfigService::new_with_defaults(self.config.codex_home.clone());
        let user_saved_config: UserSavedConfig = match service.load_user_saved_config().await {
            Ok(config) => config,
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: err.to_string(),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let response = GetUserSavedConfigResponse {
            config: user_saved_config,
        };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn get_user_info(&self, request_id: ConnectionRequestId) {
        // Read alleged user email from cached auth (best-effort; not verified).
        let alleged_user_email = self
            .auth_manager
            .auth_cached()
            .and_then(|a| a.get_account_email());

        let response = UserInfoResponse { alleged_user_email };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn set_default_model(
        &self,
        request_id: ConnectionRequestId,
        params: SetDefaultModelParams,
    ) {
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

    async fn exec_one_off_command(
        &self,
        request_id: ConnectionRequestId,
        params: CommandExecParams,
    ) {
        tracing::debug!("ExecOneOffCommand params: {params:?}");

        let request = request_id.clone();

        if params.command.is_empty() {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: "command must not be empty".to_string(),
                data: None,
            };
            self.outgoing.send_error(request, error).await;
            return;
        }

        let cwd = params.cwd.unwrap_or_else(|| self.config.cwd.clone());
        let env = create_env(&self.config.permissions.shell_environment_policy, None);
        let timeout_ms = params
            .timeout_ms
            .and_then(|timeout_ms| u64::try_from(timeout_ms).ok());
        let managed_network_requirements_enabled =
            self.config.managed_network_requirements_enabled();
        let started_network_proxy = match self.config.permissions.network.as_ref() {
            Some(spec) => match spec
                .start_proxy(
                    self.config.permissions.sandbox_policy.get(),
                    None,
                    None,
                    managed_network_requirements_enabled,
                )
                .await
            {
                Ok(started) => Some(started),
                Err(err) => {
                    let error = JSONRPCErrorError {
                        code: INTERNAL_ERROR_CODE,
                        message: format!("failed to start managed network proxy: {err}"),
                        data: None,
                    };
                    self.outgoing.send_error(request, error).await;
                    return;
                }
            },
            None => None,
        };
        let windows_sandbox_level = WindowsSandboxLevel::from_config(&self.config);
        let exec_params = ExecParams {
            command: params.command,
            cwd,
            expiration: timeout_ms.into(),
            env,
            network: started_network_proxy
                .as_ref()
                .map(codex_core::config::StartedNetworkProxy::proxy),
            network_attempt_id: None,
            sandbox_permissions: SandboxPermissions::UseDefault,
            windows_sandbox_level,
            justification: None,
            arg0: None,
        };

        let requested_policy = params.sandbox_policy.map(|policy| policy.to_core());
        let effective_policy = match requested_policy {
            Some(policy) => match self.config.permissions.sandbox_policy.can_set(&policy) {
                Ok(()) => policy,
                Err(err) => {
                    let error = JSONRPCErrorError {
                        code: INVALID_REQUEST_ERROR_CODE,
                        message: format!("invalid sandbox policy: {err}"),
                        data: None,
                    };
                    self.outgoing.send_error(request, error).await;
                    return;
                }
            },
            None => self.config.permissions.sandbox_policy.get().clone(),
        };

        let codex_linux_sandbox_exe = self.config.codex_linux_sandbox_exe.clone();
        let outgoing = self.outgoing.clone();
        let request_for_task = request;
        let sandbox_cwd = self.config.cwd.clone();
        let started_network_proxy_for_task = started_network_proxy;
        let use_linux_sandbox_bwrap = self.config.features.enabled(Feature::UseLinuxSandboxBwrap);

        tokio::spawn(async move {
            let _started_network_proxy = started_network_proxy_for_task;
            match codex_core::exec::process_exec_tool_call(
                exec_params,
                &effective_policy,
                sandbox_cwd.as_path(),
                &codex_linux_sandbox_exe,
                use_linux_sandbox_bwrap,
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
                    outgoing.send_response(request_for_task, response).await;
                }
                Err(err) => {
                    let error = JSONRPCErrorError {
                        code: INTERNAL_ERROR_CODE,
                        message: format!("exec failed: {err}"),
                        data: None,
                    };
                    outgoing.send_error(request_for_task, error).await;
                }
            }
        });
    }

    async fn process_new_conversation(
        &mut self,
        request_id: ConnectionRequestId,
        params: NewConversationParams,
    ) {
        let NewConversationParams {
            model,
            model_provider,
            profile,
            cwd,
            approval_policy,
            sandbox: sandbox_mode,
            config: request_overrides,
            base_instructions,
            developer_instructions,
            compact_prompt,
            include_apply_patch_tool,
        } = params;

        let typesafe_overrides = ConfigOverrides {
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

        // Persist Windows sandbox mode.
        // TODO: persist default config in general.
        let mut request_overrides = request_overrides.unwrap_or_default();
        if cfg!(windows) {
            match WindowsSandboxLevel::from_config(&self.config) {
                WindowsSandboxLevel::Elevated => {
                    request_overrides
                        .insert("windows.sandbox".to_string(), serde_json::json!("elevated"));
                }
                WindowsSandboxLevel::RestrictedToken => {
                    request_overrides.insert(
                        "windows.sandbox".to_string(),
                        serde_json::json!("unelevated"),
                    );
                }
                WindowsSandboxLevel::Disabled => {}
            }
        }

        let cloud_requirements = self.current_cloud_requirements();
        let config = match derive_config_from_params(
            &self.cli_overrides,
            Some(request_overrides),
            typesafe_overrides,
            &cloud_requirements,
        )
        .await
        {
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

        match self.thread_manager.start_thread(config).await {
            Ok(new_thread) => {
                let NewThread {
                    thread_id,
                    session_configured,
                    ..
                } = new_thread;
                let rollout_path = match session_configured.rollout_path {
                    Some(path) => path,
                    None => {
                        let error = JSONRPCErrorError {
                            code: INTERNAL_ERROR_CODE,
                            message: "rollout path missing for v1 conversation".to_string(),
                            data: None,
                        };
                        self.outgoing.send_error(request_id, error).await;
                        return;
                    }
                };
                let response = NewConversationResponse {
                    conversation_id: thread_id,
                    model: session_configured.model,
                    reasoning_effort: session_configured.reasoning_effort,
                    rollout_path,
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

    async fn thread_start(&mut self, request_id: ConnectionRequestId, params: ThreadStartParams) {
        let ThreadStartParams {
            model,
            model_provider,
            cwd,
            approval_policy,
            sandbox,
            config,
            base_instructions,
            developer_instructions,
            dynamic_tools,
            mock_experimental_field: _mock_experimental_field,
            experimental_raw_events,
            personality,
            ephemeral,
            persist_extended_history,
        } = params;
        let mut typesafe_overrides = self.build_thread_config_overrides(
            model,
            model_provider,
            cwd,
            approval_policy,
            sandbox,
            base_instructions,
            developer_instructions,
            personality,
        );
        typesafe_overrides.ephemeral = ephemeral;

        let cloud_requirements = self.current_cloud_requirements();
        let config = match derive_config_from_params(
            &self.cli_overrides,
            config,
            typesafe_overrides,
            &cloud_requirements,
        )
        .await
        {
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

        let dynamic_tools = dynamic_tools.unwrap_or_default();
        let core_dynamic_tools = if dynamic_tools.is_empty() {
            Vec::new()
        } else {
            if let Err(message) = validate_dynamic_tools(&dynamic_tools) {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message,
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
            dynamic_tools
                .into_iter()
                .map(|tool| CoreDynamicToolSpec {
                    name: tool.name,
                    description: tool.description,
                    input_schema: tool.input_schema,
                })
                .collect()
        };

        match self
            .thread_manager
            .start_thread_with_tools(config, core_dynamic_tools, persist_extended_history)
            .await
        {
            Ok(new_conv) => {
                let NewThread {
                    thread_id,
                    thread,
                    session_configured,
                    ..
                } = new_conv;
                let config_snapshot = thread.config_snapshot().await;
                let mut thread = build_thread_from_snapshot(
                    thread_id,
                    &config_snapshot,
                    session_configured.rollout_path.clone(),
                );

                // Auto-attach a thread listener when starting a thread.
                // Use the same behavior as the v1 API, with opt-in support for raw item events.
                if let Err(err) = self
                    .ensure_conversation_listener(
                        thread_id,
                        request_id.connection_id,
                        experimental_raw_events,
                        ApiVersion::V2,
                    )
                    .await
                {
                    tracing::warn!(
                        "failed to attach listener for thread {}: {}",
                        thread_id,
                        err.message
                    );
                }

                self.thread_watch_manager
                    .upsert_thread(thread.clone())
                    .await;

                thread.status = self
                    .thread_watch_manager
                    .loaded_status_for_thread(&thread.id)
                    .await;

                let response = ThreadStartResponse {
                    thread: thread.clone(),
                    model: config_snapshot.model,
                    model_provider: config_snapshot.model_provider_id,
                    cwd: config_snapshot.cwd,
                    approval_policy: config_snapshot.approval_policy.into(),
                    sandbox: config_snapshot.sandbox_policy.into(),
                    reasoning_effort: config_snapshot.reasoning_effort,
                };

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
        personality: Option<Personality>,
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
            personality,
            ..Default::default()
        }
    }

    async fn thread_archive(
        &mut self,
        request_id: ConnectionRequestId,
        params: ThreadArchiveParams,
    ) {
        // TODO(jif) mostly rewrite this using sqlite after phase 1
        let thread_id = match ThreadId::from_string(&params.thread_id) {
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

        let rollout_path =
            match find_thread_path_by_id_str(&self.config.codex_home, &thread_id.to_string()).await
            {
                Ok(Some(p)) => p,
                Ok(None) => {
                    let error = JSONRPCErrorError {
                        code: INVALID_REQUEST_ERROR_CODE,
                        message: format!("no rollout found for thread id {thread_id}"),
                        data: None,
                    };
                    self.outgoing.send_error(request_id, error).await;
                    return;
                }
                Err(err) => {
                    let error = JSONRPCErrorError {
                        code: INVALID_REQUEST_ERROR_CODE,
                        message: format!("failed to locate thread id {thread_id}: {err}"),
                        data: None,
                    };
                    self.outgoing.send_error(request_id, error).await;
                    return;
                }
            };

        let thread_id_str = thread_id.to_string();
        match self.archive_thread_common(thread_id, &rollout_path).await {
            Ok(()) => {
                let response = ThreadArchiveResponse {};
                self.outgoing.send_response(request_id, response).await;
                let notification = ThreadArchivedNotification {
                    thread_id: thread_id_str,
                };
                self.outgoing
                    .send_server_notification(ServerNotification::ThreadArchived(notification))
                    .await;
            }
            Err(err) => {
                self.outgoing.send_error(request_id, err).await;
            }
        }
    }

    async fn thread_set_name(&self, request_id: ConnectionRequestId, params: ThreadSetNameParams) {
        let ThreadSetNameParams { thread_id, name } = params;
        let Some(name) = codex_core::util::normalize_thread_name(&name) else {
            self.send_invalid_request_error(
                request_id,
                "thread name must not be empty".to_string(),
            )
            .await;
            return;
        };

        let (_, thread) = match self.load_thread(&thread_id).await {
            Ok(v) => v,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        if let Err(err) = thread.submit(Op::SetThreadName { name }).await {
            self.send_internal_error(request_id, format!("failed to set thread name: {err}"))
                .await;
            return;
        }

        self.outgoing
            .send_response(request_id, ThreadSetNameResponse {})
            .await;
    }

    async fn thread_unarchive(
        &mut self,
        request_id: ConnectionRequestId,
        params: ThreadUnarchiveParams,
    ) {
        // TODO(jif) mostly rewrite this using sqlite after phase 1
        let thread_id = match ThreadId::from_string(&params.thread_id) {
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

        let archived_path = match find_archived_thread_path_by_id_str(
            &self.config.codex_home,
            &thread_id.to_string(),
        )
        .await
        {
            Ok(Some(path)) => path,
            Ok(None) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("no archived rollout found for thread id {thread_id}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("failed to locate archived thread id {thread_id}: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let rollout_path_display = archived_path.display().to_string();
        let fallback_provider = self.config.model_provider_id.clone();
        let state_db_ctx = get_state_db(&self.config, None).await;
        let archived_folder = self
            .config
            .codex_home
            .join(codex_core::ARCHIVED_SESSIONS_SUBDIR);

        let result: Result<Thread, JSONRPCErrorError> = async {
            let canonical_archived_dir = tokio::fs::canonicalize(&archived_folder).await.map_err(
                |err| JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!(
                        "failed to unarchive thread: unable to resolve archived directory: {err}"
                    ),
                    data: None,
                },
            )?;
            let canonical_rollout_path = tokio::fs::canonicalize(&archived_path).await;
            let canonical_rollout_path = if let Ok(path) = canonical_rollout_path
                && path.starts_with(&canonical_archived_dir)
            {
                path
            } else {
                return Err(JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!(
                        "rollout path `{rollout_path_display}` must be in archived directory"
                    ),
                    data: None,
                });
            };

            let required_suffix = format!("{thread_id}.jsonl");
            let Some(file_name) = canonical_rollout_path.file_name().map(OsStr::to_owned) else {
                return Err(JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("rollout path `{rollout_path_display}` missing file name"),
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
                        "rollout path `{rollout_path_display}` does not match thread id {thread_id}"
                    ),
                    data: None,
                });
            }

            let Some((year, month, day)) = rollout_date_parts(&file_name) else {
                return Err(JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!(
                        "rollout path `{rollout_path_display}` missing filename timestamp"
                    ),
                    data: None,
                });
            };

            let sessions_folder = self.config.codex_home.join(codex_core::SESSIONS_SUBDIR);
            let dest_dir = sessions_folder.join(year).join(month).join(day);
            let restored_path = dest_dir.join(&file_name);
            tokio::fs::create_dir_all(&dest_dir)
                .await
                .map_err(|err| JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to unarchive thread: {err}"),
                    data: None,
                })?;
            tokio::fs::rename(&canonical_rollout_path, &restored_path)
                .await
                .map_err(|err| JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to unarchive thread: {err}"),
                    data: None,
                })?;
            tokio::task::spawn_blocking({
                let restored_path = restored_path.clone();
                move || -> std::io::Result<()> {
                    let times = FileTimes::new().set_modified(SystemTime::now());
                    OpenOptions::new()
                        .append(true)
                        .open(&restored_path)?
                        .set_times(times)?;
                    Ok(())
                }
            })
            .await
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to update unarchived thread timestamp: {err}"),
                data: None,
            })?
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to update unarchived thread timestamp: {err}"),
                data: None,
            })?;
            if let Some(ctx) = state_db_ctx {
                let _ = ctx
                    .mark_unarchived(thread_id, restored_path.as_path())
                    .await;
            }
            let summary =
                read_summary_from_rollout(restored_path.as_path(), fallback_provider.as_str())
                    .await
                    .map_err(|err| JSONRPCErrorError {
                        code: INTERNAL_ERROR_CODE,
                        message: format!("failed to read unarchived thread: {err}"),
                        data: None,
                    })?;
            Ok(summary_to_thread(summary))
        }
        .await;

        match result {
            Ok(mut thread) => {
                thread.status = self
                    .thread_watch_manager
                    .loaded_status_for_thread(&thread.id)
                    .await;
                let thread_id = thread.id.clone();
                let response = ThreadUnarchiveResponse { thread };
                self.outgoing.send_response(request_id, response).await;
                let notification = ThreadUnarchivedNotification { thread_id };
                self.outgoing
                    .send_server_notification(ServerNotification::ThreadUnarchived(notification))
                    .await;
            }
            Err(err) => {
                self.outgoing.send_error(request_id, err).await;
            }
        }
    }

    async fn thread_rollback(
        &mut self,
        request_id: ConnectionRequestId,
        params: ThreadRollbackParams,
    ) {
        let ThreadRollbackParams {
            thread_id,
            num_turns,
        } = params;

        if num_turns == 0 {
            self.send_invalid_request_error(request_id, "numTurns must be >= 1".to_string())
                .await;
            return;
        }

        let (thread_id, thread) = match self.load_thread(&thread_id).await {
            Ok(v) => v,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let request = request_id.clone();

        let rollback_already_in_progress = {
            let thread_state = self.thread_state_manager.thread_state(thread_id);
            let mut thread_state = thread_state.lock().await;
            if thread_state.pending_rollbacks.is_some() {
                true
            } else {
                thread_state.pending_rollbacks = Some(request.clone());
                false
            }
        };
        if rollback_already_in_progress {
            self.send_invalid_request_error(
                request.clone(),
                "rollback already in progress for this thread".to_string(),
            )
            .await;
            return;
        }

        if let Err(err) = thread.submit(Op::ThreadRollback { num_turns }).await {
            // No ThreadRollback event will arrive if an error occurs.
            // Clean up and reply immediately.
            let thread_state = self.thread_state_manager.thread_state(thread_id);
            let mut thread_state = thread_state.lock().await;
            thread_state.pending_rollbacks = None;
            drop(thread_state);

            self.send_internal_error(request, format!("failed to start rollback: {err}"))
                .await;
        }
    }

    async fn thread_compact_start(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadCompactStartParams,
    ) {
        let ThreadCompactStartParams { thread_id } = params;

        let (_, thread) = match self.load_thread(&thread_id).await {
            Ok(v) => v,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        match thread.submit(Op::Compact).await {
            Ok(_) => {
                self.outgoing
                    .send_response(request_id, ThreadCompactStartResponse {})
                    .await;
            }
            Err(err) => {
                self.send_internal_error(request_id, format!("failed to start compaction: {err}"))
                    .await;
            }
        }
    }

    async fn thread_background_terminals_clean(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadBackgroundTerminalsCleanParams,
    ) {
        let ThreadBackgroundTerminalsCleanParams { thread_id } = params;

        let (_, thread) = match self.load_thread(&thread_id).await {
            Ok(v) => v,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        match thread.submit(Op::CleanBackgroundTerminals).await {
            Ok(_) => {
                self.outgoing
                    .send_response(request_id, ThreadBackgroundTerminalsCleanResponse {})
                    .await;
            }
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!("failed to clean background terminals: {err}"),
                )
                .await;
            }
        }
    }

    async fn thread_list(&self, request_id: ConnectionRequestId, params: ThreadListParams) {
        let ThreadListParams {
            cursor,
            limit,
            sort_key,
            model_providers,
            source_kinds,
            archived,
            cwd,
        } = params;

        let requested_page_size = limit
            .map(|value| value as usize)
            .unwrap_or(THREAD_LIST_DEFAULT_LIMIT)
            .clamp(1, THREAD_LIST_MAX_LIMIT);
        let core_sort_key = match sort_key.unwrap_or(ThreadSortKey::CreatedAt) {
            ThreadSortKey::CreatedAt => CoreThreadSortKey::CreatedAt,
            ThreadSortKey::UpdatedAt => CoreThreadSortKey::UpdatedAt,
        };
        let (summaries, next_cursor) = match self
            .list_threads_common(
                requested_page_size,
                cursor,
                core_sort_key,
                ThreadListFilters {
                    model_providers,
                    source_kinds,
                    archived: archived.unwrap_or(false),
                    cwd: cwd.map(PathBuf::from),
                },
            )
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
            .map(summary_to_thread)
            .collect::<Vec<_>>();
        let statuses = self
            .thread_watch_manager
            .loaded_statuses_for_threads(data.iter().map(|thread| thread.id.clone()).collect())
            .await;
        let data = data
            .into_iter()
            .map(|mut thread| {
                if let Some(status) = statuses.get(&thread.id) {
                    thread.status = status.clone();
                }
                thread
            })
            .collect();
        let response = ThreadListResponse { data, next_cursor };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn thread_loaded_list(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadLoadedListParams,
    ) {
        let ThreadLoadedListParams { cursor, limit } = params;
        let mut data = self
            .thread_manager
            .list_thread_ids()
            .await
            .into_iter()
            .map(|thread_id| thread_id.to_string())
            .collect::<Vec<_>>();

        if data.is_empty() {
            let response = ThreadLoadedListResponse {
                data,
                next_cursor: None,
            };
            self.outgoing.send_response(request_id, response).await;
            return;
        }

        data.sort();
        let total = data.len();
        let start = match cursor {
            Some(cursor) => {
                let cursor = match ThreadId::from_string(&cursor) {
                    Ok(id) => id.to_string(),
                    Err(_) => {
                        let error = JSONRPCErrorError {
                            code: INVALID_REQUEST_ERROR_CODE,
                            message: format!("invalid cursor: {cursor}"),
                            data: None,
                        };
                        self.outgoing.send_error(request_id, error).await;
                        return;
                    }
                };
                match data.binary_search(&cursor) {
                    Ok(idx) => idx + 1,
                    Err(idx) => idx,
                }
            }
            None => 0,
        };

        let effective_limit = limit.unwrap_or(total as u32).max(1) as usize;
        let end = start.saturating_add(effective_limit).min(total);
        let page = data[start..end].to_vec();
        let next_cursor = page.last().filter(|_| end < total).cloned();

        let response = ThreadLoadedListResponse {
            data: page,
            next_cursor,
        };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn thread_read(&mut self, request_id: ConnectionRequestId, params: ThreadReadParams) {
        let ThreadReadParams {
            thread_id,
            include_turns,
        } = params;

        let thread_uuid = match ThreadId::from_string(&thread_id) {
            Ok(id) => id,
            Err(err) => {
                self.send_invalid_request_error(request_id, format!("invalid thread id: {err}"))
                    .await;
                return;
            }
        };

        let db_summary = read_summary_from_state_db_by_thread_id(&self.config, thread_uuid).await;
        let mut rollout_path = db_summary.as_ref().map(|summary| summary.path.clone());
        if rollout_path.is_none() || include_turns {
            rollout_path =
                match find_thread_path_by_id_str(&self.config.codex_home, &thread_uuid.to_string())
                    .await
                {
                    Ok(Some(path)) => Some(path),
                    Ok(None) => {
                        if include_turns {
                            None
                        } else {
                            rollout_path
                        }
                    }
                    Err(err) => {
                        self.send_invalid_request_error(
                            request_id,
                            format!("failed to locate thread id {thread_uuid}: {err}"),
                        )
                        .await;
                        return;
                    }
                };
        }

        if include_turns && rollout_path.is_none() && db_summary.is_some() {
            self.send_internal_error(
                request_id,
                format!("failed to locate rollout for thread {thread_uuid}"),
            )
            .await;
            return;
        }

        let mut thread = if let Some(summary) = db_summary {
            summary_to_thread(summary)
        } else if let Some(rollout_path) = rollout_path.as_ref() {
            let fallback_provider = self.config.model_provider_id.as_str();
            match read_summary_from_rollout(rollout_path, fallback_provider).await {
                Ok(summary) => summary_to_thread(summary),
                Err(err) => {
                    self.send_internal_error(
                        request_id,
                        format!(
                            "failed to load rollout `{}` for thread {thread_uuid}: {err}",
                            rollout_path.display()
                        ),
                    )
                    .await;
                    return;
                }
            }
        } else {
            let Ok(thread) = self.thread_manager.get_thread(thread_uuid).await else {
                self.send_invalid_request_error(
                    request_id,
                    format!("thread not loaded: {thread_uuid}"),
                )
                .await;
                return;
            };
            let config_snapshot = thread.config_snapshot().await;
            let loaded_rollout_path = thread.rollout_path();
            if include_turns && loaded_rollout_path.is_none() {
                self.send_invalid_request_error(
                    request_id,
                    "ephemeral threads do not support includeTurns".to_string(),
                )
                .await;
                return;
            }
            if include_turns {
                rollout_path = loaded_rollout_path.clone();
            }
            build_thread_from_snapshot(thread_uuid, &config_snapshot, loaded_rollout_path)
        };

        if include_turns && let Some(rollout_path) = rollout_path.as_ref() {
            match read_rollout_items_from_rollout(rollout_path).await {
                Ok(items) => {
                    thread.turns = build_turns_from_rollout_items(&items);
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    self.send_invalid_request_error(
                        request_id,
                        format!(
                            "thread {thread_uuid} is not materialized yet; includeTurns is unavailable before first user message"
                        ),
                    )
                    .await;
                    return;
                }
                Err(err) => {
                    self.send_internal_error(
                        request_id,
                        format!(
                            "failed to load rollout `{}` for thread {thread_uuid}: {err}",
                            rollout_path.display()
                        ),
                    )
                    .await;
                    return;
                }
            }
        }

        thread.status = self
            .thread_watch_manager
            .loaded_status_for_thread(&thread.id)
            .await;
        let response = ThreadReadResponse { thread };
        self.outgoing.send_response(request_id, response).await;
    }

    pub(crate) fn thread_created_receiver(&self) -> broadcast::Receiver<ThreadId> {
        self.thread_manager.subscribe_thread_created()
    }

    pub(crate) async fn connection_closed(&mut self, connection_id: ConnectionId) {
        self.thread_state_manager
            .remove_connection(connection_id)
            .await;
    }

    /// Best-effort: ensure initialized connections are subscribed to this thread.
    pub(crate) async fn try_attach_thread_listener(
        &mut self,
        thread_id: ThreadId,
        connection_ids: Vec<ConnectionId>,
    ) {
        if let Ok(thread) = self.thread_manager.get_thread(thread_id).await {
            let config_snapshot = thread.config_snapshot().await;
            let loaded_thread =
                build_thread_from_snapshot(thread_id, &config_snapshot, thread.rollout_path());
            self.thread_watch_manager.upsert_thread(loaded_thread).await;
        }

        for connection_id in connection_ids {
            if let Err(err) = self
                .ensure_conversation_listener(thread_id, connection_id, false, ApiVersion::V2)
                .await
            {
                warn!(
                    "failed to auto-attach listener for thread {thread_id}: {message}",
                    message = err.message
                );
            }
        }
    }

    async fn thread_resume(&mut self, request_id: ConnectionRequestId, params: ThreadResumeParams) {
        if self
            .resume_running_thread(request_id.clone(), &params)
            .await
        {
            return;
        }

        let ThreadResumeParams {
            thread_id,
            history,
            path,
            model,
            model_provider,
            cwd,
            approval_policy,
            sandbox,
            config: request_overrides,
            base_instructions,
            developer_instructions,
            personality,
            persist_extended_history,
        } = params;

        let thread_history = if let Some(history) = history {
            let Some(thread_history) = self
                .resume_thread_from_history(request_id.clone(), history.as_slice())
                .await
            else {
                return;
            };
            thread_history
        } else {
            let Some(thread_history) = self
                .resume_thread_from_rollout(request_id.clone(), &thread_id, path.as_ref())
                .await
            else {
                return;
            };
            thread_history
        };

        let history_cwd = thread_history.session_cwd();
        let typesafe_overrides = self.build_thread_config_overrides(
            model,
            model_provider,
            cwd,
            approval_policy,
            sandbox,
            base_instructions,
            developer_instructions,
            personality,
        );

        // Derive a Config using the same logic as new conversation, honoring overrides if provided.
        let cloud_requirements = self.current_cloud_requirements();
        let config = match derive_config_for_cwd(
            &self.cli_overrides,
            request_overrides,
            typesafe_overrides,
            history_cwd,
            &cloud_requirements,
        )
        .await
        {
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

        let fallback_model_provider = config.model_provider_id.clone();

        match self
            .thread_manager
            .resume_thread_with_history(
                config,
                thread_history,
                self.auth_manager.clone(),
                persist_extended_history,
            )
            .await
        {
            Ok(NewThread {
                thread_id,
                session_configured,
                ..
            }) => {
                let SessionConfiguredEvent { rollout_path, .. } = session_configured;
                let Some(rollout_path) = rollout_path else {
                    self.send_internal_error(
                        request_id,
                        format!("rollout path missing for thread {thread_id}"),
                    )
                    .await;
                    return;
                };
                // Auto-attach a thread listener when resuming a thread.
                if let Err(err) = self
                    .ensure_conversation_listener(
                        thread_id,
                        request_id.connection_id,
                        false,
                        ApiVersion::V2,
                    )
                    .await
                {
                    tracing::warn!(
                        "failed to attach listener for thread {}: {}",
                        thread_id,
                        err.message
                    );
                }

                let Some(mut thread) = self
                    .load_thread_from_rollout_or_send_internal(
                        request_id.clone(),
                        thread_id,
                        rollout_path.as_path(),
                        fallback_model_provider.as_str(),
                    )
                    .await
                else {
                    return;
                };

                self.thread_watch_manager
                    .upsert_thread(thread.clone())
                    .await;

                thread.status = self
                    .thread_watch_manager
                    .loaded_status_for_thread(&thread.id)
                    .await;

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

    async fn resume_running_thread(
        &mut self,
        request_id: ConnectionRequestId,
        params: &ThreadResumeParams,
    ) -> bool {
        if let Ok(existing_thread_id) = ThreadId::from_string(&params.thread_id)
            && let Ok(existing_thread) = self.thread_manager.get_thread(existing_thread_id).await
        {
            if params.history.is_some() {
                self.send_invalid_request_error(
                    request_id,
                    format!(
                        "cannot resume thread {existing_thread_id} with history while it is already running"
                    ),
                )
                .await;
                return true;
            }

            let rollout_path = if let Some(path) = existing_thread.rollout_path() {
                if path.exists() {
                    path
                } else {
                    match find_thread_path_by_id_str(
                        &self.config.codex_home,
                        &existing_thread_id.to_string(),
                    )
                    .await
                    {
                        Ok(Some(path)) => path,
                        Ok(None) => {
                            self.send_invalid_request_error(
                                request_id,
                                format!("no rollout found for thread id {existing_thread_id}"),
                            )
                            .await;
                            return true;
                        }
                        Err(err) => {
                            self.send_invalid_request_error(
                                request_id,
                                format!("failed to locate thread id {existing_thread_id}: {err}"),
                            )
                            .await;
                            return true;
                        }
                    }
                }
            } else {
                match find_thread_path_by_id_str(
                    &self.config.codex_home,
                    &existing_thread_id.to_string(),
                )
                .await
                {
                    Ok(Some(path)) => path,
                    Ok(None) => {
                        self.send_invalid_request_error(
                            request_id,
                            format!("no rollout found for thread id {existing_thread_id}"),
                        )
                        .await;
                        return true;
                    }
                    Err(err) => {
                        self.send_invalid_request_error(
                            request_id,
                            format!("failed to locate thread id {existing_thread_id}: {err}"),
                        )
                        .await;
                        return true;
                    }
                }
            };

            if let Some(requested_path) = params.path.as_ref()
                && requested_path != &rollout_path
            {
                self.send_invalid_request_error(
                    request_id,
                    format!(
                        "cannot resume running thread {existing_thread_id} with mismatched path: requested `{}`, active `{}`",
                        requested_path.display(),
                        rollout_path.display()
                    ),
                )
                .await;
                return true;
            }

            if let Err(err) = self
                .ensure_conversation_listener(
                    existing_thread_id,
                    request_id.connection_id,
                    false,
                    ApiVersion::V2,
                )
                .await
            {
                tracing::warn!(
                    "failed to attach listener for thread {}: {}",
                    existing_thread_id,
                    err.message
                );
            }

            let config_snapshot = existing_thread.config_snapshot().await;
            let mismatch_details = collect_resume_override_mismatches(params, &config_snapshot);
            if !mismatch_details.is_empty() {
                tracing::warn!(
                    "thread/resume overrides ignored for running thread {}: {}",
                    existing_thread_id,
                    mismatch_details.join("; ")
                );
            }

            let Some(mut thread) = self
                .load_thread_from_rollout_or_send_internal(
                    request_id.clone(),
                    existing_thread_id,
                    rollout_path.as_path(),
                    config_snapshot.model_provider_id.as_str(),
                )
                .await
            else {
                return true;
            };

            let ThreadConfigSnapshot {
                model,
                model_provider_id,
                approval_policy,
                sandbox_policy,
                cwd,
                reasoning_effort,
                ..
            } = config_snapshot;
            thread.status = self
                .thread_watch_manager
                .loaded_status_for_thread(&thread.id)
                .await;
            let response = ThreadResumeResponse {
                thread,
                model,
                model_provider: model_provider_id,
                cwd,
                approval_policy: approval_policy.into(),
                sandbox: sandbox_policy.into(),
                reasoning_effort,
            };
            self.outgoing.send_response(request_id, response).await;
            return true;
        }
        false
    }

    async fn resume_thread_from_history(
        &self,
        request_id: ConnectionRequestId,
        history: &[ResponseItem],
    ) -> Option<InitialHistory> {
        if history.is_empty() {
            self.send_invalid_request_error(request_id, "history must not be empty".to_string())
                .await;
            return None;
        }
        Some(InitialHistory::Forked(
            history
                .iter()
                .cloned()
                .map(RolloutItem::ResponseItem)
                .collect(),
        ))
    }

    async fn resume_thread_from_rollout(
        &self,
        request_id: ConnectionRequestId,
        thread_id: &str,
        path: Option<&PathBuf>,
    ) -> Option<InitialHistory> {
        let rollout_path = if let Some(path) = path {
            path.clone()
        } else {
            let existing_thread_id = match ThreadId::from_string(thread_id) {
                Ok(id) => id,
                Err(err) => {
                    let error = JSONRPCErrorError {
                        code: INVALID_REQUEST_ERROR_CODE,
                        message: format!("invalid thread id: {err}"),
                        data: None,
                    };
                    self.outgoing.send_error(request_id, error).await;
                    return None;
                }
            };

            match find_thread_path_by_id_str(
                &self.config.codex_home,
                &existing_thread_id.to_string(),
            )
            .await
            {
                Ok(Some(path)) => path,
                Ok(None) => {
                    self.send_invalid_request_error(
                        request_id,
                        format!("no rollout found for thread id {existing_thread_id}"),
                    )
                    .await;
                    return None;
                }
                Err(err) => {
                    self.send_invalid_request_error(
                        request_id,
                        format!("failed to locate thread id {existing_thread_id}: {err}"),
                    )
                    .await;
                    return None;
                }
            }
        };

        match RolloutRecorder::get_rollout_history(&rollout_path).await {
            Ok(initial_history) => Some(initial_history),
            Err(err) => {
                self.send_invalid_request_error(
                    request_id,
                    format!("failed to load rollout `{}`: {err}", rollout_path.display()),
                )
                .await;
                None
            }
        }
    }

    async fn load_thread_from_rollout_or_send_internal(
        &self,
        request_id: ConnectionRequestId,
        thread_id: ThreadId,
        rollout_path: &Path,
        fallback_provider: &str,
    ) -> Option<Thread> {
        let mut thread = match read_summary_from_rollout(rollout_path, fallback_provider).await {
            Ok(summary) => summary_to_thread(summary),
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!(
                        "failed to load rollout `{}` for thread {thread_id}: {err}",
                        rollout_path.display()
                    ),
                )
                .await;
                return None;
            }
        };
        match read_rollout_items_from_rollout(rollout_path).await {
            Ok(items) => {
                thread.turns = build_turns_from_rollout_items(&items);
                Some(thread)
            }
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!(
                        "failed to load rollout `{}` for thread {thread_id}: {err}",
                        rollout_path.display()
                    ),
                )
                .await;
                None
            }
        }
    }

    async fn thread_fork(&mut self, request_id: ConnectionRequestId, params: ThreadForkParams) {
        let ThreadForkParams {
            thread_id,
            path,
            model,
            model_provider,
            cwd,
            approval_policy,
            sandbox,
            config: cli_overrides,
            base_instructions,
            developer_instructions,
            persist_extended_history,
        } = params;

        let (rollout_path, source_thread_id) = if let Some(path) = path {
            (path, None)
        } else {
            let existing_thread_id = match ThreadId::from_string(&thread_id) {
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

            match find_thread_path_by_id_str(
                &self.config.codex_home,
                &existing_thread_id.to_string(),
            )
            .await
            {
                Ok(Some(p)) => (p, Some(existing_thread_id)),
                Ok(None) => {
                    self.send_invalid_request_error(
                        request_id,
                        format!("no rollout found for thread id {existing_thread_id}"),
                    )
                    .await;
                    return;
                }
                Err(err) => {
                    self.send_invalid_request_error(
                        request_id,
                        format!("failed to locate thread id {existing_thread_id}: {err}"),
                    )
                    .await;
                    return;
                }
            }
        };

        let history_cwd =
            read_history_cwd_from_state_db(&self.config, source_thread_id, rollout_path.as_path())
                .await;

        // Persist Windows sandbox mode.
        let mut cli_overrides = cli_overrides.unwrap_or_default();
        if cfg!(windows) {
            match WindowsSandboxLevel::from_config(&self.config) {
                WindowsSandboxLevel::Elevated => {
                    cli_overrides
                        .insert("windows.sandbox".to_string(), serde_json::json!("elevated"));
                }
                WindowsSandboxLevel::RestrictedToken => {
                    cli_overrides.insert(
                        "windows.sandbox".to_string(),
                        serde_json::json!("unelevated"),
                    );
                }
                WindowsSandboxLevel::Disabled => {}
            }
        }
        let request_overrides = if cli_overrides.is_empty() {
            None
        } else {
            Some(cli_overrides)
        };
        let typesafe_overrides = self.build_thread_config_overrides(
            model,
            model_provider,
            cwd,
            approval_policy,
            sandbox,
            base_instructions,
            developer_instructions,
            None,
        );
        // Derive a Config using the same logic as new conversation, honoring overrides if provided.
        let cloud_requirements = self.current_cloud_requirements();
        let config = match derive_config_for_cwd(
            &self.cli_overrides,
            request_overrides,
            typesafe_overrides,
            history_cwd,
            &cloud_requirements,
        )
        .await
        {
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

        let fallback_model_provider = config.model_provider_id.clone();

        let NewThread {
            thread_id,
            session_configured,
            ..
        } = match self
            .thread_manager
            .fork_thread(
                usize::MAX,
                config,
                rollout_path.clone(),
                persist_extended_history,
            )
            .await
        {
            Ok(thread) => thread,
            Err(err) => {
                let (code, message) = match err {
                    CodexErr::Io(_) | CodexErr::Json(_) => (
                        INVALID_REQUEST_ERROR_CODE,
                        format!("failed to load rollout `{}`: {err}", rollout_path.display()),
                    ),
                    CodexErr::InvalidRequest(message) => (INVALID_REQUEST_ERROR_CODE, message),
                    _ => (INTERNAL_ERROR_CODE, format!("error forking thread: {err}")),
                };
                let error = JSONRPCErrorError {
                    code,
                    message,
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let SessionConfiguredEvent { rollout_path, .. } = session_configured;
        let Some(rollout_path) = rollout_path else {
            self.send_internal_error(
                request_id,
                format!("rollout path missing for thread {thread_id}"),
            )
            .await;
            return;
        };
        // Auto-attach a conversation listener when forking a thread.
        if let Err(err) = self
            .ensure_conversation_listener(
                thread_id,
                request_id.connection_id,
                false,
                ApiVersion::V2,
            )
            .await
        {
            tracing::warn!(
                "failed to attach listener for thread {}: {}",
                thread_id,
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
                        "failed to load rollout `{}` for thread {thread_id}: {err}",
                        rollout_path.display()
                    ),
                )
                .await;
                return;
            }
        };
        match read_rollout_items_from_rollout(rollout_path.as_path()).await {
            Ok(items) => {
                thread.turns = build_turns_from_rollout_items(&items);
            }
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!(
                        "failed to load rollout `{}` for thread {thread_id}: {err}",
                        rollout_path.display()
                    ),
                )
                .await;
                return;
            }
        }

        self.thread_watch_manager
            .upsert_thread(thread.clone())
            .await;

        thread.status = self
            .thread_watch_manager
            .loaded_status_for_thread(&thread.id)
            .await;

        let response = ThreadForkResponse {
            thread: thread.clone(),
            model: session_configured.model,
            model_provider: session_configured.model_provider_id,
            cwd: session_configured.cwd,
            approval_policy: session_configured.approval_policy.into(),
            sandbox: session_configured.sandbox_policy.into(),
            reasoning_effort: session_configured.reasoning_effort,
        };

        self.outgoing.send_response(request_id, response).await;

        let notif = ThreadStartedNotification { thread };
        self.outgoing
            .send_server_notification(ServerNotification::ThreadStarted(notif))
            .await;
    }

    async fn get_thread_summary(
        &self,
        request_id: ConnectionRequestId,
        params: GetConversationSummaryParams,
    ) {
        if let GetConversationSummaryParams::ThreadId { conversation_id } = &params
            && let Some(summary) =
                read_summary_from_state_db_by_thread_id(&self.config, *conversation_id).await
        {
            let response = GetConversationSummaryResponse { summary };
            self.outgoing.send_response(request_id, response).await;
            return;
        }

        let path = match params {
            GetConversationSummaryParams::RolloutPath { rollout_path } => {
                if rollout_path.is_relative() {
                    self.config.codex_home.join(&rollout_path)
                } else {
                    rollout_path
                }
            }
            GetConversationSummaryParams::ThreadId { conversation_id } => {
                match codex_core::find_thread_path_by_id_str(
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
        request_id: ConnectionRequestId,
        params: ListConversationsParams,
    ) {
        let ListConversationsParams {
            page_size,
            cursor,
            model_providers,
        } = params;
        let requested_page_size = page_size
            .unwrap_or(THREAD_LIST_DEFAULT_LIMIT)
            .clamp(1, THREAD_LIST_MAX_LIMIT);

        match self
            .list_threads_common(
                requested_page_size,
                cursor,
                CoreThreadSortKey::UpdatedAt,
                ThreadListFilters {
                    model_providers,
                    source_kinds: None,
                    archived: false,
                    cwd: None,
                },
            )
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

    async fn list_threads_common(
        &self,
        requested_page_size: usize,
        cursor: Option<String>,
        sort_key: CoreThreadSortKey,
        filters: ThreadListFilters,
    ) -> Result<(Vec<ConversationSummary>, Option<String>), JSONRPCErrorError> {
        let ThreadListFilters {
            model_providers,
            source_kinds,
            archived,
            cwd,
        } = filters;
        let mut cursor_obj: Option<RolloutCursor> = match cursor.as_ref() {
            Some(cursor_str) => {
                Some(parse_cursor(cursor_str).ok_or_else(|| JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("invalid cursor: {cursor_str}"),
                    data: None,
                })?)
            }
            None => None,
        };
        let mut last_cursor = cursor_obj.clone();
        let mut remaining = requested_page_size;
        let mut items = Vec::with_capacity(requested_page_size);
        let mut next_cursor: Option<String> = None;

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
        let (allowed_sources_vec, source_kind_filter) = compute_source_filters(source_kinds);
        let allowed_sources = allowed_sources_vec.as_slice();
        let state_db_ctx = get_state_db(&self.config, None).await;

        while remaining > 0 {
            let page_size = remaining.min(THREAD_LIST_MAX_LIMIT);
            let page = if archived {
                RolloutRecorder::list_archived_threads(
                    &self.config,
                    page_size,
                    cursor_obj.as_ref(),
                    sort_key,
                    allowed_sources,
                    model_provider_filter.as_deref(),
                    fallback_provider.as_str(),
                )
                .await
                .map_err(|err| JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to list threads: {err}"),
                    data: None,
                })?
            } else {
                RolloutRecorder::list_threads(
                    &self.config,
                    page_size,
                    cursor_obj.as_ref(),
                    sort_key,
                    allowed_sources,
                    model_provider_filter.as_deref(),
                    fallback_provider.as_str(),
                )
                .await
                .map_err(|err| JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to list threads: {err}"),
                    data: None,
                })?
            };

            let mut filtered = Vec::with_capacity(page.items.len());
            for it in page.items {
                let Some(summary) = summary_from_thread_list_item(
                    it,
                    fallback_provider.as_str(),
                    state_db_ctx.as_ref(),
                )
                .await
                else {
                    continue;
                };
                if source_kind_filter
                    .as_ref()
                    .is_none_or(|filter| source_kind_matches(&summary.source, filter))
                    && cwd
                        .as_ref()
                        .is_none_or(|expected_cwd| &summary.cwd == expected_cwd)
                {
                    filtered.push(summary);
                    if filtered.len() >= remaining {
                        break;
                    }
                }
            }
            items.extend(filtered);
            remaining = requested_page_size.saturating_sub(items.len());

            // Encode RolloutCursor into the JSON-RPC string form returned to clients.
            let next_cursor_value = page.next_cursor.clone();
            next_cursor = next_cursor_value
                .as_ref()
                .and_then(|cursor| serde_json::to_value(cursor).ok())
                .and_then(|value| value.as_str().map(str::to_owned));
            if remaining == 0 {
                break;
            }

            match next_cursor_value {
                Some(cursor_val) if remaining > 0 => {
                    // Break if our pagination would reuse the same cursor again; this avoids
                    // an infinite loop when filtering drops everything on the page.
                    if last_cursor.as_ref() == Some(&cursor_val) {
                        next_cursor = None;
                        break;
                    }
                    last_cursor = Some(cursor_val.clone());
                    cursor_obj = Some(cursor_val);
                }
                _ => break,
            }
        }

        Ok((items, next_cursor))
    }

    async fn list_models(
        outgoing: Arc<OutgoingMessageSender>,
        thread_manager: Arc<ThreadManager>,
        request_id: ConnectionRequestId,
        params: ModelListParams,
    ) {
        let ModelListParams {
            limit,
            cursor,
            include_hidden,
        } = params;
        let models = supported_models(thread_manager, include_hidden.unwrap_or(false)).await;
        let total = models.len();

        if total == 0 {
            let response = ModelListResponse {
                data: Vec::new(),
                next_cursor: None,
            };
            outgoing.send_response(request_id, response).await;
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
                    outgoing.send_error(request_id, error).await;
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
            outgoing.send_error(request_id, error).await;
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
        outgoing.send_response(request_id, response).await;
    }

    async fn list_collaboration_modes(
        outgoing: Arc<OutgoingMessageSender>,
        thread_manager: Arc<ThreadManager>,
        request_id: ConnectionRequestId,
        params: CollaborationModeListParams,
    ) {
        let CollaborationModeListParams {} = params;
        let items = thread_manager.list_collaboration_modes();
        let response = CollaborationModeListResponse { data: items };
        outgoing.send_response(request_id, response).await;
    }

    async fn experimental_feature_list(
        &self,
        request_id: ConnectionRequestId,
        params: ExperimentalFeatureListParams,
    ) {
        let ExperimentalFeatureListParams { cursor, limit } = params;
        let config = match self.load_latest_config().await {
            Ok(config) => config,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let data = FEATURES
            .iter()
            .map(|spec| {
                let (stage, display_name, description, announcement) = match spec.stage {
                    Stage::Experimental {
                        name,
                        menu_description,
                        announcement,
                    } => (
                        ApiExperimentalFeatureStage::Beta,
                        Some(name.to_string()),
                        Some(menu_description.to_string()),
                        Some(announcement.to_string()),
                    ),
                    Stage::UnderDevelopment => (
                        ApiExperimentalFeatureStage::UnderDevelopment,
                        None,
                        None,
                        None,
                    ),
                    Stage::Stable => (ApiExperimentalFeatureStage::Stable, None, None, None),
                    Stage::Deprecated => {
                        (ApiExperimentalFeatureStage::Deprecated, None, None, None)
                    }
                    Stage::Removed => (ApiExperimentalFeatureStage::Removed, None, None, None),
                };

                ApiExperimentalFeature {
                    name: spec.key.to_string(),
                    stage,
                    display_name,
                    description,
                    announcement,
                    enabled: config.features.enabled(spec.id),
                    default_enabled: spec.default_enabled,
                }
            })
            .collect::<Vec<_>>();

        let total = data.len();
        if total == 0 {
            self.outgoing
                .send_response(
                    request_id,
                    ExperimentalFeatureListResponse {
                        data: Vec::new(),
                        next_cursor: None,
                    },
                )
                .await;
            return;
        }

        // Clamp to 1 so limit=0 cannot return a non-advancing page.
        let effective_limit = limit.unwrap_or(total as u32).max(1) as usize;
        let effective_limit = effective_limit.min(total);
        let start = match cursor {
            Some(cursor) => match cursor.parse::<usize>() {
                Ok(idx) => idx,
                Err(_) => {
                    self.send_invalid_request_error(
                        request_id,
                        format!("invalid cursor: {cursor}"),
                    )
                    .await;
                    return;
                }
            },
            None => 0,
        };

        if start > total {
            self.send_invalid_request_error(
                request_id,
                format!("cursor {start} exceeds total feature flags {total}"),
            )
            .await;
            return;
        }

        let end = start.saturating_add(effective_limit).min(total);
        let data = data[start..end].to_vec();
        let next_cursor = if end < total {
            Some(end.to_string())
        } else {
            None
        };

        self.outgoing
            .send_response(
                request_id,
                ExperimentalFeatureListResponse { data, next_cursor },
            )
            .await;
    }

    async fn mock_experimental_method(
        &self,
        request_id: ConnectionRequestId,
        params: MockExperimentalMethodParams,
    ) {
        let MockExperimentalMethodParams { value } = params;
        let response = MockExperimentalMethodResponse { echoed: value };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn mcp_server_refresh(&self, request_id: ConnectionRequestId, _params: Option<()>) {
        let config = match self.load_latest_config().await {
            Ok(config) => config,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let mcp_servers = match serde_json::to_value(config.mcp_servers.get()) {
            Ok(value) => value,
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to serialize MCP servers: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let mcp_oauth_credentials_store_mode =
            match serde_json::to_value(config.mcp_oauth_credentials_store_mode) {
                Ok(value) => value,
                Err(err) => {
                    let error = JSONRPCErrorError {
                        code: INTERNAL_ERROR_CODE,
                        message: format!(
                            "failed to serialize MCP OAuth credentials store mode: {err}"
                        ),
                        data: None,
                    };
                    self.outgoing.send_error(request_id, error).await;
                    return;
                }
            };

        let refresh_config = McpServerRefreshConfig {
            mcp_servers,
            mcp_oauth_credentials_store_mode,
        };

        // Refresh requests are queued per thread; each thread rebuilds MCP connections on its next
        // active turn to avoid work for threads that never resume.
        let thread_manager = Arc::clone(&self.thread_manager);
        thread_manager.refresh_mcp_servers(refresh_config).await;
        let response = McpServerRefreshResponse {};
        self.outgoing.send_response(request_id, response).await;
    }

    async fn mcp_server_oauth_login(
        &self,
        request_id: ConnectionRequestId,
        params: McpServerOauthLoginParams,
    ) {
        let config = match self.load_latest_config().await {
            Ok(config) => config,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let McpServerOauthLoginParams {
            name,
            scopes,
            timeout_secs,
        } = params;

        let Some(server) = config.mcp_servers.get().get(&name) else {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("No MCP server named '{name}' found."),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        };

        let (url, http_headers, env_http_headers) = match &server.transport {
            McpServerTransportConfig::StreamableHttp {
                url,
                http_headers,
                env_http_headers,
                ..
            } => (url.clone(), http_headers.clone(), env_http_headers.clone()),
            _ => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: "OAuth login is only supported for streamable HTTP servers."
                        .to_string(),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let scopes = scopes.or_else(|| server.scopes.clone());

        match perform_oauth_login_return_url(
            &name,
            &url,
            config.mcp_oauth_credentials_store_mode,
            http_headers,
            env_http_headers,
            scopes.as_deref().unwrap_or_default(),
            timeout_secs,
            config.mcp_oauth_callback_port,
        )
        .await
        {
            Ok(handle) => {
                let authorization_url = handle.authorization_url().to_string();
                let notification_name = name.clone();
                let outgoing = Arc::clone(&self.outgoing);

                tokio::spawn(async move {
                    let (success, error) = match handle.wait().await {
                        Ok(()) => (true, None),
                        Err(err) => (false, Some(err.to_string())),
                    };

                    let notification = ServerNotification::McpServerOauthLoginCompleted(
                        McpServerOauthLoginCompletedNotification {
                            name: notification_name,
                            success,
                            error,
                        },
                    );
                    outgoing.send_server_notification(notification).await;
                });

                let response = McpServerOauthLoginResponse { authorization_url };
                self.outgoing.send_response(request_id, response).await;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to login to MCP server '{name}': {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn list_mcp_server_status(
        &self,
        request_id: ConnectionRequestId,
        params: ListMcpServerStatusParams,
    ) {
        let request = request_id.clone();

        let outgoing = Arc::clone(&self.outgoing);
        let config = match self.load_latest_config().await {
            Ok(config) => config,
            Err(error) => {
                self.outgoing.send_error(request, error).await;
                return;
            }
        };

        tokio::spawn(async move {
            Self::list_mcp_server_status_task(outgoing, request, params, config).await;
        });
    }

    async fn list_mcp_server_status_task(
        outgoing: Arc<OutgoingMessageSender>,
        request_id: ConnectionRequestId,
        params: ListMcpServerStatusParams,
        config: Config,
    ) {
        let snapshot = collect_mcp_snapshot(&config).await;

        let tools_by_server = group_tools_by_server(&snapshot.tools);

        let mut server_names: Vec<String> = config
            .mcp_servers
            .keys()
            .cloned()
            .chain(snapshot.auth_statuses.keys().cloned())
            .chain(snapshot.resources.keys().cloned())
            .chain(snapshot.resource_templates.keys().cloned())
            .collect();
        server_names.sort();
        server_names.dedup();

        let total = server_names.len();
        let limit = params.limit.unwrap_or(total as u32).max(1) as usize;
        let effective_limit = limit.min(total);
        let start = match params.cursor {
            Some(cursor) => match cursor.parse::<usize>() {
                Ok(idx) => idx,
                Err(_) => {
                    let error = JSONRPCErrorError {
                        code: INVALID_REQUEST_ERROR_CODE,
                        message: format!("invalid cursor: {cursor}"),
                        data: None,
                    };
                    outgoing.send_error(request_id, error).await;
                    return;
                }
            },
            None => 0,
        };

        if start > total {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("cursor {start} exceeds total MCP servers {total}"),
                data: None,
            };
            outgoing.send_error(request_id, error).await;
            return;
        }

        let end = start.saturating_add(effective_limit).min(total);

        let data: Vec<McpServerStatus> = server_names[start..end]
            .iter()
            .map(|name| McpServerStatus {
                name: name.clone(),
                tools: tools_by_server.get(name).cloned().unwrap_or_default(),
                resources: snapshot.resources.get(name).cloned().unwrap_or_default(),
                resource_templates: snapshot
                    .resource_templates
                    .get(name)
                    .cloned()
                    .unwrap_or_default(),
                auth_status: snapshot
                    .auth_statuses
                    .get(name)
                    .cloned()
                    .unwrap_or(CoreMcpAuthStatus::Unsupported)
                    .into(),
            })
            .collect();

        let next_cursor = if end < total {
            Some(end.to_string())
        } else {
            None
        };

        let response = ListMcpServerStatusResponse { data, next_cursor };

        outgoing.send_response(request_id, response).await;
    }

    async fn handle_resume_conversation(
        &self,
        request_id: ConnectionRequestId,
        params: ResumeConversationParams,
    ) {
        let ResumeConversationParams {
            path,
            conversation_id,
            history,
            overrides,
        } = params;

        let thread_history = if let Some(path) = path {
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
            match find_thread_path_by_id_str(&self.config.codex_home, &conversation_id.to_string())
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

        let history_cwd = thread_history.session_cwd();
        let (typesafe_overrides, request_overrides) = match overrides {
            Some(overrides) => {
                let NewConversationParams {
                    model,
                    model_provider,
                    profile,
                    cwd,
                    approval_policy,
                    sandbox: sandbox_mode,
                    config: request_overrides,
                    base_instructions,
                    developer_instructions,
                    compact_prompt,
                    include_apply_patch_tool,
                } = overrides;

                // Persist Windows sandbox mode.
                let mut request_overrides = request_overrides.unwrap_or_default();
                if cfg!(windows) {
                    match WindowsSandboxLevel::from_config(&self.config) {
                        WindowsSandboxLevel::Elevated => {
                            request_overrides.insert(
                                "windows.sandbox".to_string(),
                                serde_json::json!("elevated"),
                            );
                        }
                        WindowsSandboxLevel::RestrictedToken => {
                            request_overrides.insert(
                                "windows.sandbox".to_string(),
                                serde_json::json!("unelevated"),
                            );
                        }
                        WindowsSandboxLevel::Disabled => {}
                    }
                }

                let typesafe_overrides = ConfigOverrides {
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
                (typesafe_overrides, Some(request_overrides))
            }
            None => (
                ConfigOverrides {
                    codex_linux_sandbox_exe: self.codex_linux_sandbox_exe.clone(),
                    ..Default::default()
                },
                None,
            ),
        };

        let cloud_requirements = self.current_cloud_requirements();
        let config = match derive_config_for_cwd(
            &self.cli_overrides,
            request_overrides,
            typesafe_overrides,
            history_cwd,
            &cloud_requirements,
        )
        .await
        {
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

        match self
            .thread_manager
            .resume_thread_with_history(config, thread_history, self.auth_manager.clone(), false)
            .await
        {
            Ok(NewThread {
                thread_id,
                session_configured,
                ..
            }) => {
                let rollout_path = match session_configured.rollout_path.clone() {
                    Some(path) => path,
                    None => {
                        let error = JSONRPCErrorError {
                            code: INTERNAL_ERROR_CODE,
                            message: "rollout path missing for resumed conversation".to_string(),
                            data: None,
                        };
                        self.outgoing.send_error(request_id, error).await;
                        return;
                    }
                };
                self.outgoing
                    .send_server_notification(ServerNotification::SessionConfigured(
                        SessionConfiguredNotification {
                            session_id: session_configured.session_id,
                            model: session_configured.model.clone(),
                            reasoning_effort: session_configured.reasoning_effort,
                            history_log_id: session_configured.history_log_id,
                            history_entry_count: session_configured.history_entry_count,
                            initial_messages: session_configured.initial_messages.clone(),
                            rollout_path: rollout_path.clone(),
                        },
                    ))
                    .await;
                let initial_messages = session_configured
                    .initial_messages
                    .map(|msgs| msgs.into_iter().collect());

                // Reply with thread id + model and initial messages (when present)
                let response = ResumeConversationResponse {
                    conversation_id: thread_id,
                    model: session_configured.model.clone(),
                    initial_messages,
                    rollout_path,
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

    async fn handle_fork_conversation(
        &self,
        request_id: ConnectionRequestId,
        params: ForkConversationParams,
    ) {
        let ForkConversationParams {
            path,
            conversation_id,
            overrides,
        } = params;

        // Derive a Config using the same logic as new conversation, honoring overrides if provided.
        let (rollout_path, source_thread_id) = if let Some(path) = path {
            (path, None)
        } else if let Some(conversation_id) = conversation_id {
            match find_thread_path_by_id_str(&self.config.codex_home, &conversation_id.to_string())
                .await
            {
                Ok(Some(found_path)) => (found_path, Some(conversation_id)),
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
            self.send_invalid_request_error(
                request_id,
                "either path or conversation id must be provided".to_string(),
            )
            .await;
            return;
        };

        let history_cwd =
            read_history_cwd_from_state_db(&self.config, source_thread_id, rollout_path.as_path())
                .await;

        let (typesafe_overrides, request_overrides) = match overrides {
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

                // Persist Windows sandbox mode.
                let mut cli_overrides = cli_overrides.unwrap_or_default();
                if cfg!(windows) {
                    match WindowsSandboxLevel::from_config(&self.config) {
                        WindowsSandboxLevel::Elevated => {
                            cli_overrides.insert(
                                "windows.sandbox".to_string(),
                                serde_json::json!("elevated"),
                            );
                        }
                        WindowsSandboxLevel::RestrictedToken => {
                            cli_overrides.insert(
                                "windows.sandbox".to_string(),
                                serde_json::json!("unelevated"),
                            );
                        }
                        WindowsSandboxLevel::Disabled => {}
                    }
                }
                let request_overrides = if cli_overrides.is_empty() {
                    None
                } else {
                    Some(cli_overrides)
                };

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

                (overrides, request_overrides)
            }
            None => (
                ConfigOverrides {
                    codex_linux_sandbox_exe: self.codex_linux_sandbox_exe.clone(),
                    ..Default::default()
                },
                None,
            ),
        };

        let cloud_requirements = self.current_cloud_requirements();
        let config = match derive_config_for_cwd(
            &self.cli_overrides,
            request_overrides,
            typesafe_overrides,
            history_cwd,
            &cloud_requirements,
        )
        .await
        {
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

        let NewThread {
            thread_id,
            session_configured,
            ..
        } = match self
            .thread_manager
            .fork_thread(usize::MAX, config, rollout_path.clone(), false)
            .await
        {
            Ok(thread) => thread,
            Err(err) => {
                let (code, message) = match err {
                    CodexErr::Io(_) | CodexErr::Json(_) => (
                        INVALID_REQUEST_ERROR_CODE,
                        format!("failed to load rollout `{}`: {err}", rollout_path.display()),
                    ),
                    CodexErr::InvalidRequest(message) => (INVALID_REQUEST_ERROR_CODE, message),
                    _ => (
                        INTERNAL_ERROR_CODE,
                        format!("error forking conversation: {err}"),
                    ),
                };
                let error = JSONRPCErrorError {
                    code,
                    message,
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let rollout_path = match session_configured.rollout_path.clone() {
            Some(path) => path,
            None => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: "rollout path missing for forked conversation".to_string(),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        self.outgoing
            .send_server_notification(ServerNotification::SessionConfigured(
                SessionConfiguredNotification {
                    session_id: session_configured.session_id,
                    model: session_configured.model.clone(),
                    reasoning_effort: session_configured.reasoning_effort,
                    history_log_id: session_configured.history_log_id,
                    history_entry_count: session_configured.history_entry_count,
                    initial_messages: session_configured.initial_messages.clone(),
                    rollout_path: rollout_path.clone(),
                },
            ))
            .await;
        let initial_messages = session_configured
            .initial_messages
            .map(|msgs| msgs.into_iter().collect());

        // Reply with conversation id + model and initial messages (when present)
        let response = ForkConversationResponse {
            conversation_id: thread_id,
            model: session_configured.model.clone(),
            initial_messages,
            rollout_path,
        };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn send_invalid_request_error(&self, request_id: ConnectionRequestId, message: String) {
        let error = JSONRPCErrorError {
            code: INVALID_REQUEST_ERROR_CODE,
            message,
            data: None,
        };
        self.outgoing.send_error(request_id, error).await;
    }

    async fn send_internal_error(&self, request_id: ConnectionRequestId, message: String) {
        let error = JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
            message,
            data: None,
        };
        self.outgoing.send_error(request_id, error).await;
    }

    async fn archive_conversation(
        &mut self,
        request_id: ConnectionRequestId,
        params: ArchiveConversationParams,
    ) {
        let ArchiveConversationParams {
            conversation_id: thread_id,
            rollout_path,
        } = params;

        match self.archive_thread_common(thread_id, &rollout_path).await {
            Ok(()) => {
                tracing::info!("thread/archive succeeded for {thread_id}");
                let response = ArchiveConversationResponse {};
                self.outgoing.send_response(request_id, response).await;
            }
            Err(err) => {
                tracing::warn!("thread/archive failed for {thread_id}: {}", err.message);
                self.outgoing.send_error(request_id, err).await;
            }
        }
    }

    async fn archive_thread_common(
        &mut self,
        thread_id: ThreadId,
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
                        "failed to archive thread: unable to resolve sessions directory: {err}"
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

        // Verify file name matches thread id.
        let required_suffix = format!("{thread_id}.jsonl");
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
                    "rollout path `{}` does not match thread id {thread_id}",
                    rollout_path.display()
                ),
                data: None,
            });
        }

        let mut state_db_ctx = None;

        // If the thread is active, request shutdown and wait briefly.
        let removed_conversation = self.thread_manager.remove_thread(&thread_id).await;
        if let Some(conversation) = removed_conversation {
            if let Some(ctx) = conversation.state_db() {
                state_db_ctx = Some(ctx);
            }
            info!("thread {thread_id} was active; shutting down");
            // Request shutdown.
            match conversation.submit(Op::Shutdown).await {
                Ok(_) => {
                    // Poll agent status rather than consuming events so attached listeners do not block shutdown.
                    let wait_for_shutdown = async {
                        loop {
                            if matches!(conversation.agent_status().await, AgentStatus::Shutdown) {
                                break;
                            }
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                    };
                    if tokio::time::timeout(Duration::from_secs(10), wait_for_shutdown)
                        .await
                        .is_err()
                    {
                        warn!("thread {thread_id} shutdown timed out; proceeding with archive");
                    }
                }
                Err(err) => {
                    error!("failed to submit Shutdown to thread {thread_id}: {err}");
                }
            }
            self.thread_state_manager
                .remove_thread_state(thread_id)
                .await;
        }

        self.thread_watch_manager
            .remove_thread(&thread_id.to_string())
            .await;

        if state_db_ctx.is_none() {
            state_db_ctx = get_state_db(&self.config, None).await;
        }

        // Move the rollout file to archived.
        let result: std::io::Result<()> = async move {
            let archive_folder = self
                .config
                .codex_home
                .join(codex_core::ARCHIVED_SESSIONS_SUBDIR);
            tokio::fs::create_dir_all(&archive_folder).await?;
            let archived_path = archive_folder.join(&file_name);
            tokio::fs::rename(&canonical_rollout_path, &archived_path).await?;
            if let Some(ctx) = state_db_ctx {
                let _ = ctx
                    .mark_archived(thread_id, archived_path.as_path(), Utc::now())
                    .await;
            }
            Ok(())
        }
        .await;

        result.map_err(|err| JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
            message: format!("failed to archive thread: {err}"),
            data: None,
        })
    }

    async fn send_user_message(
        &self,
        request_id: ConnectionRequestId,
        params: SendUserMessageParams,
    ) {
        let SendUserMessageParams {
            conversation_id,
            items,
        } = params;
        let Ok(conversation) = self.thread_manager.get_thread(conversation_id).await else {
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
                WireInputItem::Text {
                    text,
                    text_elements,
                } => CoreInputItem::Text {
                    text,
                    text_elements: text_elements.into_iter().map(Into::into).collect(),
                },
                WireInputItem::Image { image_url } => CoreInputItem::Image { image_url },
                WireInputItem::LocalImage { path } => CoreInputItem::LocalImage { path },
            })
            .collect();

        // Submit user input to the conversation.
        let _ = conversation
            .submit(Op::UserInput {
                items: mapped_items,
                final_output_json_schema: None,
            })
            .await;

        // Acknowledge with an empty result.
        self.outgoing
            .send_response(request_id, SendUserMessageResponse {})
            .await;
    }

    async fn send_user_turn(&self, request_id: ConnectionRequestId, params: SendUserTurnParams) {
        let SendUserTurnParams {
            conversation_id,
            items,
            cwd,
            approval_policy,
            sandbox_policy,
            model,
            effort,
            summary,
            output_schema,
        } = params;

        let Ok(conversation) = self.thread_manager.get_thread(conversation_id).await else {
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
                WireInputItem::Text {
                    text,
                    text_elements,
                } => CoreInputItem::Text {
                    text,
                    text_elements: text_elements.into_iter().map(Into::into).collect(),
                },
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
                final_output_json_schema: output_schema,
                collaboration_mode: None,
                personality: None,
            })
            .await;

        self.outgoing
            .send_response(request_id, SendUserTurnResponse {})
            .await;
    }

    async fn apps_list(&self, request_id: ConnectionRequestId, params: AppsListParams) {
        let mut config = match self.load_latest_config().await {
            Ok(config) => config,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        if let Some(thread_id) = params.thread_id.as_deref() {
            let (_, thread) = match self.load_thread(thread_id).await {
                Ok(result) => result,
                Err(error) => {
                    self.outgoing.send_error(request_id, error).await;
                    return;
                }
            };

            if thread.enabled(Feature::Apps) {
                config.features.enable(Feature::Apps);
            } else {
                config.features.disable(Feature::Apps);
            }
        }

        if !config.features.enabled(Feature::Apps) {
            self.outgoing
                .send_response(
                    request_id,
                    AppsListResponse {
                        data: Vec::new(),
                        next_cursor: None,
                    },
                )
                .await;
            return;
        }

        let request = request_id.clone();
        let outgoing = Arc::clone(&self.outgoing);
        tokio::spawn(async move {
            Self::apps_list_task(outgoing, request, params, config).await;
        });
    }

    async fn apps_list_task(
        outgoing: Arc<OutgoingMessageSender>,
        request_id: ConnectionRequestId,
        params: AppsListParams,
        config: Config,
    ) {
        let AppsListParams {
            cursor,
            limit,
            thread_id: _,
            force_refetch,
        } = params;
        let start = match cursor {
            Some(cursor) => match cursor.parse::<usize>() {
                Ok(idx) => idx,
                Err(_) => {
                    let error = JSONRPCErrorError {
                        code: INVALID_REQUEST_ERROR_CODE,
                        message: format!("invalid cursor: {cursor}"),
                        data: None,
                    };
                    outgoing.send_error(request_id, error).await;
                    return;
                }
            },
            None => 0,
        };

        let (mut accessible_connectors, mut all_connectors) = tokio::join!(
            connectors::list_cached_accessible_connectors_from_mcp_tools(&config),
            connectors::list_cached_all_connectors(&config)
        );

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let accessible_config = config.clone();
        let accessible_tx = tx.clone();
        tokio::spawn(async move {
            let result = connectors::list_accessible_connectors_from_mcp_tools_with_options(
                &accessible_config,
                force_refetch,
            )
            .await
            .map_err(|err| format!("failed to load accessible apps: {err}"));
            let _ = accessible_tx.send(AppListLoadResult::Accessible(result));
        });

        let all_config = config.clone();
        tokio::spawn(async move {
            let result = connectors::list_all_connectors_with_options(&all_config, force_refetch)
                .await
                .map_err(|err| format!("failed to list apps: {err}"));
            let _ = tx.send(AppListLoadResult::Directory(result));
        });

        let app_list_deadline = tokio::time::Instant::now() + APP_LIST_LOAD_TIMEOUT;
        let mut accessible_loaded = false;
        let mut all_loaded = false;

        loop {
            let result = match tokio::time::timeout_at(app_list_deadline, rx.recv()).await {
                Ok(Some(result)) => result,
                Ok(None) => {
                    let error = JSONRPCErrorError {
                        code: INTERNAL_ERROR_CODE,
                        message: "failed to load app lists".to_string(),
                        data: None,
                    };
                    outgoing.send_error(request_id, error).await;
                    return;
                }
                Err(_) => {
                    let timeout_seconds = APP_LIST_LOAD_TIMEOUT.as_secs();
                    let error = JSONRPCErrorError {
                        code: INTERNAL_ERROR_CODE,
                        message: format!(
                            "timed out waiting for app lists after {timeout_seconds} seconds"
                        ),
                        data: None,
                    };
                    outgoing.send_error(request_id, error).await;
                    return;
                }
            };

            match result {
                AppListLoadResult::Accessible(Ok(connectors)) => {
                    accessible_connectors = Some(connectors);
                    accessible_loaded = true;
                }
                AppListLoadResult::Accessible(Err(err)) => {
                    let error = JSONRPCErrorError {
                        code: INTERNAL_ERROR_CODE,
                        message: err,
                        data: None,
                    };
                    outgoing.send_error(request_id, error).await;
                    return;
                }
                AppListLoadResult::Directory(Ok(connectors)) => {
                    all_connectors = Some(connectors);
                    all_loaded = true;
                }
                AppListLoadResult::Directory(Err(err)) => {
                    let error = JSONRPCErrorError {
                        code: INTERNAL_ERROR_CODE,
                        message: err,
                        data: None,
                    };
                    outgoing.send_error(request_id, error).await;
                    return;
                }
            }

            let merged = connectors::with_app_enabled_state(
                Self::merge_loaded_apps(
                    all_connectors.as_deref(),
                    accessible_connectors.as_deref(),
                ),
                &config,
            );
            Self::send_app_list_updated_notification(&outgoing, merged.clone()).await;

            if accessible_loaded && all_loaded {
                match Self::paginate_apps(merged.as_slice(), start, limit) {
                    Ok(response) => {
                        outgoing.send_response(request_id, response).await;
                        return;
                    }
                    Err(error) => {
                        outgoing.send_error(request_id, error).await;
                        return;
                    }
                }
            }
        }
    }

    fn merge_loaded_apps(
        all_connectors: Option<&[AppInfo]>,
        accessible_connectors: Option<&[AppInfo]>,
    ) -> Vec<AppInfo> {
        let all_connectors_loaded = all_connectors.is_some();
        let all = all_connectors.map_or_else(Vec::new, <[AppInfo]>::to_vec);
        let accessible = accessible_connectors.map_or_else(Vec::new, <[AppInfo]>::to_vec);
        connectors::merge_connectors_with_accessible(all, accessible, all_connectors_loaded)
    }

    fn paginate_apps(
        connectors: &[AppInfo],
        start: usize,
        limit: Option<u32>,
    ) -> Result<AppsListResponse, JSONRPCErrorError> {
        let total = connectors.len();
        if start > total {
            return Err(JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("cursor {start} exceeds total apps {total}"),
                data: None,
            });
        }

        let effective_limit = limit.unwrap_or(total as u32).max(1) as usize;
        let end = start.saturating_add(effective_limit).min(total);
        let data = connectors[start..end].to_vec();
        let next_cursor = if end < total {
            Some(end.to_string())
        } else {
            None
        };

        Ok(AppsListResponse { data, next_cursor })
    }

    async fn send_app_list_updated_notification(
        outgoing: &Arc<OutgoingMessageSender>,
        data: Vec<AppInfo>,
    ) {
        outgoing
            .send_server_notification(ServerNotification::AppListUpdated(
                AppListUpdatedNotification { data },
            ))
            .await;
    }

    async fn skills_list(&self, request_id: ConnectionRequestId, params: SkillsListParams) {
        let SkillsListParams {
            cwds,
            force_reload,
            per_cwd_extra_user_roots,
        } = params;
        let cwds = if cwds.is_empty() {
            vec![self.config.cwd.clone()]
        } else {
            cwds
        };
        let cwd_set: HashSet<PathBuf> = cwds.iter().cloned().collect();

        let mut extra_roots_by_cwd: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        for entry in per_cwd_extra_user_roots.unwrap_or_default() {
            if !cwd_set.contains(&entry.cwd) {
                warn!(
                    cwd = %entry.cwd.display(),
                    "ignoring per-cwd extra roots for cwd not present in skills/list cwds"
                );
                continue;
            }

            let mut valid_extra_roots = Vec::new();
            for root in entry.extra_user_roots {
                if !root.is_absolute() {
                    self.send_invalid_request_error(
                        request_id,
                        format!(
                            "skills/list perCwdExtraUserRoots extraUserRoots paths must be absolute: {}",
                            root.display()
                        ),
                    )
                    .await;
                    return;
                }
                valid_extra_roots.push(root);
            }
            extra_roots_by_cwd
                .entry(entry.cwd)
                .or_default()
                .extend(valid_extra_roots);
        }

        let skills_manager = self.thread_manager.skills_manager();
        let mut data = Vec::new();
        for cwd in cwds {
            let extra_roots = extra_roots_by_cwd
                .get(&cwd)
                .map_or(&[][..], std::vec::Vec::as_slice);
            let outcome = skills_manager
                .skills_for_cwd_with_extra_user_roots(&cwd, force_reload, extra_roots)
                .await;
            let errors = errors_to_info(&outcome.errors);
            let skills = skills_to_info(&outcome.skills, &outcome.disabled_paths);
            data.push(codex_app_server_protocol::SkillsListEntry {
                cwd,
                skills,
                errors,
            });
        }
        self.outgoing
            .send_response(request_id, SkillsListResponse { data })
            .await;
    }

    async fn skills_remote_list(
        &self,
        request_id: ConnectionRequestId,
        params: SkillsRemoteReadParams,
    ) {
        let hazelnut_scope = convert_remote_scope(params.hazelnut_scope);
        let product_surface = convert_remote_product_surface(params.product_surface);
        let enabled = if params.enabled { Some(true) } else { None };

        let auth = self.auth_manager.auth().await;
        match list_remote_skills(
            &self.config,
            auth.as_ref(),
            hazelnut_scope,
            product_surface,
            enabled,
        )
        .await
        {
            Ok(skills) => {
                let data = skills
                    .into_iter()
                    .map(|skill| codex_app_server_protocol::RemoteSkillSummary {
                        id: skill.id,
                        name: skill.name,
                        description: skill.description,
                    })
                    .collect();
                self.outgoing
                    .send_response(request_id, SkillsRemoteReadResponse { data })
                    .await;
            }
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!("failed to list remote skills: {err}"),
                )
                .await;
            }
        }
    }

    async fn skills_remote_export(
        &self,
        request_id: ConnectionRequestId,
        params: SkillsRemoteWriteParams,
    ) {
        let SkillsRemoteWriteParams { hazelnut_id } = params;
        let auth = self.auth_manager.auth().await;
        let response = export_remote_skill(&self.config, auth.as_ref(), hazelnut_id.as_str()).await;

        match response {
            Ok(downloaded) => {
                self.outgoing
                    .send_response(
                        request_id,
                        SkillsRemoteWriteResponse {
                            id: downloaded.id,
                            path: downloaded.path,
                        },
                    )
                    .await;
            }
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!("failed to download remote skill: {err}"),
                )
                .await;
            }
        }
    }

    async fn skills_config_write(
        &self,
        request_id: ConnectionRequestId,
        params: SkillsConfigWriteParams,
    ) {
        let SkillsConfigWriteParams { path, enabled } = params;
        let edits = vec![ConfigEdit::SetSkillConfig { path, enabled }];
        let result = ConfigEditsBuilder::new(&self.config.codex_home)
            .with_edits(edits)
            .apply()
            .await;

        match result {
            Ok(()) => {
                self.thread_manager.skills_manager().clear_cache();
                self.outgoing
                    .send_response(
                        request_id,
                        SkillsConfigWriteResponse {
                            effective_enabled: enabled,
                        },
                    )
                    .await;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to update skill settings: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn interrupt_conversation(
        &mut self,
        request_id: ConnectionRequestId,
        params: InterruptConversationParams,
    ) {
        let InterruptConversationParams { conversation_id } = params;
        let Ok(conversation) = self.thread_manager.get_thread(conversation_id).await else {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("conversation not found: {conversation_id}"),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        };

        let request = request_id.clone();

        // Record the pending interrupt so we can reply when TurnAborted arrives.
        {
            let pending_interrupts = self.thread_state_manager.thread_state(conversation_id);
            let mut thread_state = pending_interrupts.lock().await;
            thread_state
                .pending_interrupts
                .push((request, ApiVersion::V1));
        }

        // Submit the interrupt; we'll respond upon TurnAborted.
        let _ = conversation.submit(Op::Interrupt).await;
    }

    async fn turn_start(&self, request_id: ConnectionRequestId, params: TurnStartParams) {
        let (_, thread) = match self.load_thread(&params.thread_id).await {
            Ok(v) => v,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let collaboration_mode = params
            .collaboration_mode
            .map(|mode| self.normalize_turn_start_collaboration_mode(mode));

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
            || params.summary.is_some()
            || collaboration_mode.is_some()
            || params.personality.is_some();

        // If any overrides are provided, update the session turn context first.
        if has_any_overrides {
            let _ = thread
                .submit(Op::OverrideTurnContext {
                    cwd: params.cwd,
                    approval_policy: params.approval_policy.map(AskForApproval::to_core),
                    sandbox_policy: params.sandbox_policy.map(|p| p.to_core()),
                    windows_sandbox_level: None,
                    model: params.model,
                    effort: params.effort.map(Some),
                    summary: params.summary,
                    collaboration_mode,
                    personality: params.personality,
                })
                .await;
        }

        // Start the turn by submitting the user input. Return its submission id as turn_id.
        let turn_id = thread
            .submit(Op::UserInput {
                items: mapped_items,
                final_output_json_schema: params.output_schema,
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

    async fn turn_steer(&self, request_id: ConnectionRequestId, params: TurnSteerParams) {
        let (_, thread) = match self.load_thread(&params.thread_id).await {
            Ok(v) => v,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        if params.expected_turn_id.is_empty() {
            self.send_invalid_request_error(
                request_id,
                "expectedTurnId must not be empty".to_string(),
            )
            .await;
            return;
        }

        let mapped_items: Vec<CoreInputItem> = params
            .input
            .into_iter()
            .map(V2UserInput::into_core)
            .collect();

        match thread
            .steer_input(mapped_items, Some(&params.expected_turn_id))
            .await
        {
            Ok(turn_id) => {
                let response = TurnSteerResponse { turn_id };
                self.outgoing.send_response(request_id, response).await;
            }
            Err(err) => {
                let (code, message) = match err {
                    SteerInputError::NoActiveTurn(_) => (
                        INVALID_REQUEST_ERROR_CODE,
                        "no active turn to steer".to_string(),
                    ),
                    SteerInputError::ExpectedTurnMismatch { expected, actual } => (
                        INVALID_REQUEST_ERROR_CODE,
                        format!("expected active turn id `{expected}` but found `{actual}`"),
                    ),
                    SteerInputError::EmptyInput => (
                        INVALID_REQUEST_ERROR_CODE,
                        "input must not be empty".to_string(),
                    ),
                };
                let error = JSONRPCErrorError {
                    code,
                    message,
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
                    // Review prompt display text is synthesized; no UI element ranges to preserve.
                    text_elements: Vec::new(),
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
        request_id: &ConnectionRequestId,
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
        request_id: &ConnectionRequestId,
        parent_thread: Arc<CodexThread>,
        review_request: ReviewRequest,
        display_text: &str,
        parent_thread_id: String,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        let turn_id = parent_thread.submit(Op::Review { review_request }).await;

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
        request_id: &ConnectionRequestId,
        parent_thread_id: ThreadId,
        parent_thread: Arc<CodexThread>,
        review_request: ReviewRequest,
        display_text: &str,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        let rollout_path = if let Some(path) = parent_thread.rollout_path() {
            path
        } else {
            find_thread_path_by_id_str(&self.config.codex_home, &parent_thread_id.to_string())
                .await
                .map_err(|err| JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to locate thread id {parent_thread_id}: {err}"),
                    data: None,
                })?
                .ok_or_else(|| JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("no rollout found for thread id {parent_thread_id}"),
                    data: None,
                })?
        };

        let mut config = self.config.as_ref().clone();
        if let Some(review_model) = &config.review_model {
            config.model = Some(review_model.clone());
        }

        let NewThread {
            thread_id,
            thread: review_thread,
            session_configured,
            ..
        } = self
            .thread_manager
            .fork_thread(usize::MAX, config, rollout_path, false)
            .await
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("error creating detached review thread: {err}"),
                data: None,
            })?;

        if let Err(err) = self
            .ensure_conversation_listener(
                thread_id,
                request_id.connection_id,
                false,
                ApiVersion::V2,
            )
            .await
        {
            tracing::warn!(
                "failed to attach listener for review thread {}: {}",
                thread_id,
                err.message
            );
        }

        let fallback_provider = self.config.model_provider_id.as_str();
        if let Some(rollout_path) = review_thread.rollout_path() {
            match read_summary_from_rollout(rollout_path.as_path(), fallback_provider).await {
                Ok(summary) => {
                    let mut thread = summary_to_thread(summary);
                    self.thread_watch_manager
                        .upsert_thread(thread.clone())
                        .await;
                    thread.status = self
                        .thread_watch_manager
                        .loaded_status_for_thread(&thread.id)
                        .await;
                    let notif = ThreadStartedNotification { thread };
                    self.outgoing
                        .send_server_notification(ServerNotification::ThreadStarted(notif))
                        .await;
                }
                Err(err) => {
                    tracing::warn!(
                        "failed to load summary for review thread {}: {}",
                        session_configured.session_id,
                        err
                    );
                }
            }
        } else {
            tracing::warn!(
                "review thread {} has no rollout path",
                session_configured.session_id
            );
        }

        let turn_id = review_thread
            .submit(Op::Review { review_request })
            .await
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to start detached review turn: {err}"),
                data: None,
            })?;

        let turn = Self::build_review_turn(turn_id, display_text);
        let review_thread_id = thread_id.to_string();
        self.emit_review_started(request_id, turn, review_thread_id.clone(), review_thread_id)
            .await;

        Ok(())
    }

    async fn review_start(&mut self, request_id: ConnectionRequestId, params: ReviewStartParams) {
        let ReviewStartParams {
            thread_id,
            target,
            delivery,
        } = params;
        let (parent_thread_id, parent_thread) = match self.load_thread(&thread_id).await {
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
                        parent_thread,
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
                        parent_thread_id,
                        parent_thread,
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

    async fn turn_interrupt(
        &mut self,
        request_id: ConnectionRequestId,
        params: TurnInterruptParams,
    ) {
        let TurnInterruptParams { thread_id, .. } = params;

        let (thread_uuid, thread) = match self.load_thread(&thread_id).await {
            Ok(v) => v,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let request = request_id.clone();

        // Record the pending interrupt so we can reply when TurnAborted arrives.
        {
            let thread_state = self.thread_state_manager.thread_state(thread_uuid);
            let mut thread_state = thread_state.lock().await;
            thread_state
                .pending_interrupts
                .push((request, ApiVersion::V2));
        }

        // Submit the interrupt; we'll respond upon TurnAborted.
        let _ = thread.submit(Op::Interrupt).await;
    }

    async fn add_conversation_listener(
        &mut self,
        request_id: ConnectionRequestId,
        params: AddConversationListenerParams,
    ) {
        let AddConversationListenerParams {
            conversation_id,
            experimental_raw_events,
        } = params;
        let conversation = match self.thread_manager.get_thread(conversation_id).await {
            Ok(conv) => conv,
            Err(_) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("thread not found: {conversation_id}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };
        let subscription_id = Uuid::new_v4();
        let thread_state = self
            .thread_state_manager
            .set_listener(
                subscription_id,
                conversation_id,
                request_id.connection_id,
                experimental_raw_events,
            )
            .await;
        self.ensure_listener_task_running(
            conversation_id,
            conversation,
            thread_state,
            ApiVersion::V1,
        )
        .await;

        let response = AddConversationSubscriptionResponse { subscription_id };
        self.outgoing.send_response(request_id, response).await;
    }

    async fn remove_thread_listener(
        &mut self,
        request_id: ConnectionRequestId,
        params: RemoveConversationListenerParams,
    ) {
        let RemoveConversationListenerParams { subscription_id } = params;
        match self
            .thread_state_manager
            .remove_listener(subscription_id)
            .await
        {
            Some(thread_id) => {
                info!("removed listener for thread {thread_id}");
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

    async fn ensure_conversation_listener(
        &mut self,
        conversation_id: ThreadId,
        connection_id: ConnectionId,
        raw_events_enabled: bool,
        api_version: ApiVersion,
    ) -> Result<(), JSONRPCErrorError> {
        let conversation = match self.thread_manager.get_thread(conversation_id).await {
            Ok(conv) => conv,
            Err(_) => {
                return Err(JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("thread not found: {conversation_id}"),
                    data: None,
                });
            }
        };
        let thread_state = self
            .thread_state_manager
            .ensure_connection_subscribed(conversation_id, connection_id, raw_events_enabled)
            .await;
        self.ensure_listener_task_running(conversation_id, conversation, thread_state, api_version)
            .await;
        Ok(())
    }

    async fn ensure_listener_task_running(
        &self,
        conversation_id: ThreadId,
        conversation: Arc<CodexThread>,
        thread_state: Arc<Mutex<ThreadState>>,
        api_version: ApiVersion,
    ) {
        let (cancel_tx, mut cancel_rx) = oneshot::channel();
        {
            let mut thread_state = thread_state.lock().await;
            if thread_state.listener_matches(&conversation) {
                return;
            }
            thread_state.set_listener(cancel_tx, &conversation);
        }
        let outgoing_for_task = self.outgoing.clone();
        let thread_manager = self.thread_manager.clone();
        let thread_watch_manager = self.thread_watch_manager.clone();
        let fallback_model_provider = self.config.model_provider_id.clone();
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
                                tracing::warn!("thread.next_event() failed with: {err}");
                                break;
                            }
                        };

                        // For now, we send a notification for every event,
                        // JSON-serializing the `Event` as-is, but these should
                        // be migrated to be variants of `ServerNotification`
                        // instead.
                        let event_formatted = match &event.msg {
                            EventMsg::TurnStarted(_) => "task_started",
                            EventMsg::TurnComplete(_) => "task_complete",
                            _ => &event.msg.to_string(),
                        };
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
                        let (subscribed_connection_ids, raw_events_enabled) = {
                            let thread_state = thread_state.lock().await;
                            (
                                thread_state.subscribed_connection_ids(),
                                thread_state.experimental_raw_events,
                            )
                        };
                        if let EventMsg::RawResponseItem(_) = &event.msg && !raw_events_enabled {
                            continue;
                        }

                        if !subscribed_connection_ids.is_empty() {
                            outgoing_for_task
                                .send_notification_to_connections(
                                    &subscribed_connection_ids,
                                    OutgoingNotification {
                                        method: format!("codex/event/{event_formatted}"),
                                        params: Some(params.into()),
                                    },
                                )
                                .await;
                        }

                        let thread_outgoing = ThreadScopedOutgoingMessageSender::new(
                            outgoing_for_task.clone(),
                            subscribed_connection_ids,
                        );
                        apply_bespoke_event_handling(
                            event.clone(),
                            conversation_id,
                            conversation.clone(),
                            thread_manager.clone(),
                            thread_outgoing,
                            thread_state.clone(),
                            thread_watch_manager.clone(),
                            api_version,
                            fallback_model_provider.clone(),
                        )
                        .await;
                    }
                }
            }
        });
    }
    async fn git_diff_to_origin(&self, request_id: ConnectionRequestId, cwd: PathBuf) {
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

    async fn fuzzy_file_search(
        &mut self,
        request_id: ConnectionRequestId,
        params: FuzzyFileSearchParams,
    ) {
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

    async fn fuzzy_file_search_session_start(
        &mut self,
        request_id: ConnectionRequestId,
        params: FuzzyFileSearchSessionStartParams,
    ) {
        let FuzzyFileSearchSessionStartParams { session_id, roots } = params;
        if session_id.is_empty() {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: "sessionId must not be empty".to_string(),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        }

        let session =
            start_fuzzy_file_search_session(session_id.clone(), roots, self.outgoing.clone());
        match session {
            Ok(session) => {
                let mut sessions = self.fuzzy_search_sessions.lock().await;
                sessions.insert(session_id, session);
                self.outgoing
                    .send_response(request_id, FuzzyFileSearchSessionStartResponse {})
                    .await;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to start fuzzy file search session: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn fuzzy_file_search_session_update(
        &mut self,
        request_id: ConnectionRequestId,
        params: FuzzyFileSearchSessionUpdateParams,
    ) {
        let FuzzyFileSearchSessionUpdateParams { session_id, query } = params;
        let found = {
            let sessions = self.fuzzy_search_sessions.lock().await;
            if let Some(session) = sessions.get(&session_id) {
                session.update_query(query);
                true
            } else {
                false
            }
        };
        if !found {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("fuzzy file search session not found: {session_id}"),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        }

        self.outgoing
            .send_response(request_id, FuzzyFileSearchSessionUpdateResponse {})
            .await;
    }

    async fn fuzzy_file_search_session_stop(
        &mut self,
        request_id: ConnectionRequestId,
        params: FuzzyFileSearchSessionStopParams,
    ) {
        let FuzzyFileSearchSessionStopParams { session_id } = params;
        {
            let mut sessions = self.fuzzy_search_sessions.lock().await;
            sessions.remove(&session_id);
        }

        self.outgoing
            .send_response(request_id, FuzzyFileSearchSessionStopResponse {})
            .await;
    }

    async fn upload_feedback(&self, request_id: ConnectionRequestId, params: FeedbackUploadParams) {
        if !self.config.feedback_enabled {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: "sending feedback is disabled by configuration".to_string(),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        }

        let FeedbackUploadParams {
            classification,
            reason,
            thread_id,
            include_logs,
        } = params;

        let conversation_id = match thread_id.as_deref() {
            Some(thread_id) => match ThreadId::from_string(thread_id) {
                Ok(conversation_id) => Some(conversation_id),
                Err(err) => {
                    let error = JSONRPCErrorError {
                        code: INVALID_REQUEST_ERROR_CODE,
                        message: format!("invalid thread id: {err}"),
                        data: None,
                    };
                    self.outgoing.send_error(request_id, error).await;
                    return;
                }
            },
            None => None,
        };

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
        let session_source = self.thread_manager.session_source();

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

    async fn windows_sandbox_setup_start(
        &mut self,
        request_id: ConnectionRequestId,
        params: WindowsSandboxSetupStartParams,
    ) {
        self.outgoing
            .send_response(
                request_id.clone(),
                WindowsSandboxSetupStartResponse { started: true },
            )
            .await;

        let mode = match params.mode {
            WindowsSandboxSetupMode::Elevated => CoreWindowsSandboxSetupMode::Elevated,
            WindowsSandboxSetupMode::Unelevated => CoreWindowsSandboxSetupMode::Unelevated,
        };
        let config = Arc::clone(&self.config);
        let outgoing = ThreadScopedOutgoingMessageSender::new(
            Arc::clone(&self.outgoing),
            vec![request_id.connection_id],
        );

        tokio::spawn(async move {
            let setup_request = WindowsSandboxSetupRequest {
                mode,
                policy: config.permissions.sandbox_policy.get().clone(),
                policy_cwd: config.cwd.clone(),
                command_cwd: config.cwd.clone(),
                env_map: std::env::vars().collect(),
                codex_home: config.codex_home.clone(),
                active_profile: config.active_profile.clone(),
            };
            let setup_result =
                codex_core::windows_sandbox::run_windows_sandbox_setup(setup_request).await;
            let notification = WindowsSandboxSetupCompletedNotification {
                mode: match mode {
                    CoreWindowsSandboxSetupMode::Elevated => WindowsSandboxSetupMode::Elevated,
                    CoreWindowsSandboxSetupMode::Unelevated => WindowsSandboxSetupMode::Unelevated,
                },
                success: setup_result.is_ok(),
                error: setup_result.err().map(|err| err.to_string()),
            };
            outgoing
                .send_server_notification(ServerNotification::WindowsSandboxSetupCompleted(
                    notification,
                ))
                .await;
        });
    }

    async fn resolve_rollout_path(&self, conversation_id: ThreadId) -> Option<PathBuf> {
        match self.thread_manager.get_thread(conversation_id).await {
            Ok(conv) => conv.rollout_path(),
            Err(_) => None,
        }
    }
}

fn collect_resume_override_mismatches(
    request: &ThreadResumeParams,
    config_snapshot: &ThreadConfigSnapshot,
) -> Vec<String> {
    let mut mismatch_details = Vec::new();

    if let Some(requested_model) = request.model.as_deref()
        && requested_model != config_snapshot.model
    {
        mismatch_details.push(format!(
            "model requested={requested_model} active={}",
            config_snapshot.model
        ));
    }
    if let Some(requested_provider) = request.model_provider.as_deref()
        && requested_provider != config_snapshot.model_provider_id
    {
        mismatch_details.push(format!(
            "model_provider requested={requested_provider} active={}",
            config_snapshot.model_provider_id
        ));
    }
    if let Some(requested_cwd) = request.cwd.as_deref() {
        let requested_cwd_path = std::path::PathBuf::from(requested_cwd);
        if requested_cwd_path != config_snapshot.cwd {
            mismatch_details.push(format!(
                "cwd requested={} active={}",
                requested_cwd_path.display(),
                config_snapshot.cwd.display()
            ));
        }
    }
    if let Some(requested_approval) = request.approval_policy.as_ref() {
        let active_approval: AskForApproval = config_snapshot.approval_policy.into();
        if requested_approval != &active_approval {
            mismatch_details.push(format!(
                "approval_policy requested={requested_approval:?} active={active_approval:?}"
            ));
        }
    }
    if let Some(requested_sandbox) = request.sandbox.as_ref() {
        let sandbox_matches = matches!(
            (requested_sandbox, &config_snapshot.sandbox_policy),
            (
                SandboxMode::ReadOnly,
                codex_protocol::protocol::SandboxPolicy::ReadOnly { .. }
            ) | (
                SandboxMode::WorkspaceWrite,
                codex_protocol::protocol::SandboxPolicy::WorkspaceWrite { .. }
            ) | (
                SandboxMode::DangerFullAccess,
                codex_protocol::protocol::SandboxPolicy::DangerFullAccess
            ) | (
                SandboxMode::DangerFullAccess,
                codex_protocol::protocol::SandboxPolicy::ExternalSandbox { .. }
            )
        );
        if !sandbox_matches {
            mismatch_details.push(format!(
                "sandbox requested={requested_sandbox:?} active={:?}",
                config_snapshot.sandbox_policy
            ));
        }
    }
    if let Some(requested_personality) = request.personality.as_ref()
        && config_snapshot.personality.as_ref() != Some(requested_personality)
    {
        mismatch_details.push(format!(
            "personality requested={requested_personality:?} active={:?}",
            config_snapshot.personality
        ));
    }

    if request.config.is_some() {
        mismatch_details
            .push("config overrides were provided and ignored while running".to_string());
    }
    if request.base_instructions.is_some() {
        mismatch_details
            .push("baseInstructions override was provided and ignored while running".to_string());
    }
    if request.developer_instructions.is_some() {
        mismatch_details.push(
            "developerInstructions override was provided and ignored while running".to_string(),
        );
    }
    if request.persist_extended_history {
        mismatch_details.push(
            "persistExtendedHistory override was provided and ignored while running".to_string(),
        );
    }

    mismatch_details
}

fn skills_to_info(
    skills: &[codex_core::skills::SkillMetadata],
    disabled_paths: &std::collections::HashSet<PathBuf>,
) -> Vec<codex_app_server_protocol::SkillMetadata> {
    skills
        .iter()
        .map(|skill| {
            let enabled = !disabled_paths.contains(&skill.path);
            codex_app_server_protocol::SkillMetadata {
                name: skill.name.clone(),
                description: skill.description.clone(),
                short_description: skill.short_description.clone(),
                interface: skill.interface.clone().map(|interface| {
                    codex_app_server_protocol::SkillInterface {
                        display_name: interface.display_name,
                        short_description: interface.short_description,
                        icon_small: interface.icon_small,
                        icon_large: interface.icon_large,
                        brand_color: interface.brand_color,
                        default_prompt: interface.default_prompt,
                    }
                }),
                dependencies: skill.dependencies.clone().map(|dependencies| {
                    codex_app_server_protocol::SkillDependencies {
                        tools: dependencies
                            .tools
                            .into_iter()
                            .map(|tool| codex_app_server_protocol::SkillToolDependency {
                                r#type: tool.r#type,
                                value: tool.value,
                                description: tool.description,
                                transport: tool.transport,
                                command: tool.command,
                                url: tool.url,
                            })
                            .collect(),
                    }
                }),
                path: skill.path.clone(),
                scope: skill.scope.into(),
                enabled,
            }
        })
        .collect()
}

fn errors_to_info(
    errors: &[codex_core::skills::SkillError],
) -> Vec<codex_app_server_protocol::SkillErrorInfo> {
    errors
        .iter()
        .map(|err| codex_app_server_protocol::SkillErrorInfo {
            path: err.path.clone(),
            message: err.message.clone(),
        })
        .collect()
}

fn validate_dynamic_tools(tools: &[ApiDynamicToolSpec]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for tool in tools {
        let name = tool.name.trim();
        if name.is_empty() {
            return Err("dynamic tool name must not be empty".to_string());
        }
        if name != tool.name {
            return Err(format!(
                "dynamic tool name has leading/trailing whitespace: {}",
                tool.name
            ));
        }
        if name == "mcp" || name.starts_with("mcp__") {
            return Err(format!("dynamic tool name is reserved: {name}"));
        }
        if !seen.insert(name.to_string()) {
            return Err(format!("duplicate dynamic tool name: {name}"));
        }

        if let Err(err) = codex_core::parse_tool_input_schema(&tool.input_schema) {
            return Err(format!(
                "dynamic tool input schema is not supported for {name}: {err}"
            ));
        }
    }
    Ok(())
}

fn replace_cloud_requirements_loader(
    cloud_requirements: &RwLock<CloudRequirementsLoader>,
    auth_manager: Arc<AuthManager>,
    chatgpt_base_url: String,
    codex_home: PathBuf,
) {
    let loader = cloud_requirements_loader(auth_manager, chatgpt_base_url, codex_home);
    if let Ok(mut guard) = cloud_requirements.write() {
        *guard = loader;
    } else {
        warn!("failed to update cloud requirements loader");
    }
}

async fn sync_default_client_residency_requirement(
    cli_overrides: &[(String, TomlValue)],
    cloud_requirements: &RwLock<CloudRequirementsLoader>,
) {
    let loader = cloud_requirements
        .read()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    match codex_core::config::ConfigBuilder::default()
        .cli_overrides(cli_overrides.to_vec())
        .cloud_requirements(loader)
        .build()
        .await
    {
        Ok(config) => set_default_client_residency_requirement(config.enforce_residency.value()),
        Err(err) => warn!(
            error = %err,
            "failed to sync default client residency requirement after auth refresh"
        ),
    }
}

/// Derive the effective [`Config`] by layering three override sources.
///
/// Precedence (lowest to highest):
/// - `cli_overrides`: process-wide startup `--config` flags.
/// - `request_overrides`: per-request dotted-path overrides (`params.config`), converted JSON->TOML.
/// - `typesafe_overrides`: Request objects such as `NewThreadParams` and
///   `ThreadStartParams` support a limited set of _explicit_ config overrides, so
///   `typesafe_overrides` is a `ConfigOverrides` derived from the respective request object.
///   Because the overrides are defined explicitly in the `*Params`, this takes priority over
///   the more general "bag of config options" provided by `cli_overrides` and `request_overrides`.
async fn derive_config_from_params(
    cli_overrides: &[(String, TomlValue)],
    request_overrides: Option<HashMap<String, serde_json::Value>>,
    typesafe_overrides: ConfigOverrides,
    cloud_requirements: &CloudRequirementsLoader,
) -> std::io::Result<Config> {
    let merged_cli_overrides = cli_overrides
        .iter()
        .cloned()
        .chain(
            request_overrides
                .unwrap_or_default()
                .into_iter()
                .map(|(k, v)| (k, json_to_toml(v))),
        )
        .collect::<Vec<_>>();

    codex_core::config::ConfigBuilder::default()
        .cli_overrides(merged_cli_overrides)
        .harness_overrides(typesafe_overrides)
        .cloud_requirements(cloud_requirements.clone())
        .build()
        .await
}

async fn derive_config_for_cwd(
    cli_overrides: &[(String, TomlValue)],
    request_overrides: Option<HashMap<String, serde_json::Value>>,
    typesafe_overrides: ConfigOverrides,
    cwd: Option<PathBuf>,
    cloud_requirements: &CloudRequirementsLoader,
) -> std::io::Result<Config> {
    let merged_cli_overrides = cli_overrides
        .iter()
        .cloned()
        .chain(
            request_overrides
                .unwrap_or_default()
                .into_iter()
                .map(|(k, v)| (k, json_to_toml(v))),
        )
        .collect::<Vec<_>>();

    codex_core::config::ConfigBuilder::default()
        .cli_overrides(merged_cli_overrides)
        .harness_overrides(typesafe_overrides)
        .fallback_cwd(cwd)
        .cloud_requirements(cloud_requirements.clone())
        .build()
        .await
}

async fn read_history_cwd_from_state_db(
    config: &Config,
    thread_id: Option<ThreadId>,
    rollout_path: &Path,
) -> Option<PathBuf> {
    if let Some(state_db_ctx) = get_state_db(config, None).await
        && let Some(thread_id) = thread_id
        && let Ok(Some(metadata)) = state_db_ctx.get_thread(thread_id).await
    {
        return Some(metadata.cwd);
    }

    match read_session_meta_line(rollout_path).await {
        Ok(meta_line) => Some(meta_line.meta.cwd),
        Err(err) => {
            let rollout_path = rollout_path.display();
            warn!("failed to read session metadata from rollout {rollout_path}: {err}");
            None
        }
    }
}

async fn read_summary_from_state_db_by_thread_id(
    config: &Config,
    thread_id: ThreadId,
) -> Option<ConversationSummary> {
    let state_db_ctx = get_state_db(config, None).await;
    read_summary_from_state_db_context_by_thread_id(state_db_ctx.as_ref(), thread_id).await
}

async fn read_summary_from_state_db_context_by_thread_id(
    state_db_ctx: Option<&StateDbHandle>,
    thread_id: ThreadId,
) -> Option<ConversationSummary> {
    let state_db_ctx = state_db_ctx?;

    let metadata = match state_db_ctx.get_thread(thread_id).await {
        Ok(Some(metadata)) => metadata,
        Ok(None) | Err(_) => return None,
    };
    Some(summary_from_state_db_metadata(
        metadata.id,
        metadata.rollout_path,
        metadata.first_user_message,
        metadata
            .created_at
            .to_rfc3339_opts(SecondsFormat::Secs, true),
        metadata
            .updated_at
            .to_rfc3339_opts(SecondsFormat::Secs, true),
        metadata.model_provider,
        metadata.cwd,
        metadata.cli_version,
        metadata.source,
        metadata.git_sha,
        metadata.git_branch,
        metadata.git_origin_url,
    ))
}

async fn summary_from_thread_list_item(
    it: codex_core::ThreadItem,
    fallback_provider: &str,
    state_db_ctx: Option<&StateDbHandle>,
) -> Option<ConversationSummary> {
    if let Some(thread_id) = it.thread_id {
        let timestamp = it.created_at.clone();
        let updated_at = it.updated_at.clone().or_else(|| timestamp.clone());
        let model_provider = it
            .model_provider
            .clone()
            .unwrap_or_else(|| fallback_provider.to_string());
        let cwd = it.cwd?;
        let cli_version = it.cli_version.unwrap_or_default();
        let source = it
            .source
            .unwrap_or(codex_protocol::protocol::SessionSource::Unknown);
        return Some(ConversationSummary {
            conversation_id: thread_id,
            path: it.path,
            preview: it.first_user_message.unwrap_or_default(),
            timestamp,
            updated_at,
            model_provider,
            cwd,
            cli_version,
            source,
            git_info: if it.git_sha.is_none()
                && it.git_branch.is_none()
                && it.git_origin_url.is_none()
            {
                None
            } else {
                Some(ConversationGitInfo {
                    sha: it.git_sha,
                    branch: it.git_branch,
                    origin_url: it.git_origin_url,
                })
            },
        });
    }
    if let Some(thread_id) = thread_id_from_rollout_path(it.path.as_path()) {
        return read_summary_from_state_db_context_by_thread_id(state_db_ctx, thread_id).await;
    }
    None
}

fn thread_id_from_rollout_path(path: &Path) -> Option<ThreadId> {
    let file_name = path.file_name()?.to_str()?;
    let stem = file_name.strip_suffix(".jsonl")?;
    if stem.len() < 37 {
        return None;
    }
    let uuid_start = stem.len().saturating_sub(36);
    if !stem[..uuid_start].ends_with('-') {
        return None;
    }
    ThreadId::from_string(&stem[uuid_start..]).ok()
}

#[allow(clippy::too_many_arguments)]
fn summary_from_state_db_metadata(
    conversation_id: ThreadId,
    path: PathBuf,
    first_user_message: Option<String>,
    timestamp: String,
    updated_at: String,
    model_provider: String,
    cwd: PathBuf,
    cli_version: String,
    source: String,
    git_sha: Option<String>,
    git_branch: Option<String>,
    git_origin_url: Option<String>,
) -> ConversationSummary {
    let preview = first_user_message.unwrap_or_default();
    let source = serde_json::from_value(serde_json::Value::String(source))
        .unwrap_or(codex_protocol::protocol::SessionSource::Unknown);
    let git_info = if git_sha.is_none() && git_branch.is_none() && git_origin_url.is_none() {
        None
    } else {
        Some(ConversationGitInfo {
            sha: git_sha,
            branch: git_branch,
            origin_url: git_origin_url,
        })
    };
    ConversationSummary {
        conversation_id,
        path,
        preview,
        timestamp: Some(timestamp),
        updated_at: Some(updated_at),
        model_provider,
        cwd,
        cli_version,
        source,
        git_info,
    }
}

pub(crate) async fn read_summary_from_rollout(
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

    let created_at = if session_meta.timestamp.is_empty() {
        None
    } else {
        Some(session_meta.timestamp.as_str())
    };
    let updated_at = read_updated_at(path, created_at).await;
    if let Some(summary) = extract_conversation_summary(
        path.to_path_buf(),
        &head,
        &session_meta,
        git.as_ref(),
        fallback_provider,
        updated_at.clone(),
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
    let updated_at = updated_at.or_else(|| timestamp.clone());

    Ok(ConversationSummary {
        conversation_id: session_meta.id,
        timestamp,
        updated_at,
        path: path.to_path_buf(),
        preview: String::new(),
        model_provider,
        cwd: session_meta.cwd,
        cli_version: session_meta.cli_version,
        source: session_meta.source,
        git_info,
    })
}

pub(crate) async fn read_rollout_items_from_rollout(
    path: &Path,
) -> std::io::Result<Vec<RolloutItem>> {
    let items = match RolloutRecorder::get_rollout_history(path).await? {
        InitialHistory::New => Vec::new(),
        InitialHistory::Forked(items) => items,
        InitialHistory::Resumed(resumed) => resumed.history,
    };

    Ok(items)
}

fn extract_conversation_summary(
    path: PathBuf,
    head: &[serde_json::Value],
    session_meta: &SessionMeta,
    git: Option<&CoreGitInfo>,
    fallback_provider: &str,
    updated_at: Option<String>,
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
    let updated_at = updated_at.or_else(|| timestamp.clone());

    Some(ConversationSummary {
        conversation_id,
        timestamp,
        updated_at,
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

async fn read_updated_at(path: &Path, created_at: Option<&str>) -> Option<String> {
    let updated_at = tokio::fs::metadata(path)
        .await
        .ok()
        .and_then(|meta| meta.modified().ok())
        .map(|modified| {
            let updated_at: DateTime<Utc> = modified.into();
            updated_at.to_rfc3339_opts(SecondsFormat::Secs, true)
        });
    updated_at.or_else(|| created_at.map(str::to_string))
}

fn build_thread_from_snapshot(
    thread_id: ThreadId,
    config_snapshot: &ThreadConfigSnapshot,
    path: Option<PathBuf>,
) -> Thread {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    Thread {
        id: thread_id.to_string(),
        preview: String::new(),
        model_provider: config_snapshot.model_provider_id.clone(),
        created_at: now,
        updated_at: now,
        status: ThreadStatus::NotLoaded,
        path,
        cwd: config_snapshot.cwd.clone(),
        cli_version: env!("CARGO_PKG_VERSION").to_string(),
        source: config_snapshot.session_source.clone().into(),
        git_info: None,
        turns: Vec::new(),
    }
}

pub(crate) fn summary_to_thread(summary: ConversationSummary) -> Thread {
    let ConversationSummary {
        conversation_id,
        path,
        preview,
        timestamp,
        updated_at,
        model_provider,
        cwd,
        cli_version,
        source,
        git_info,
    } = summary;

    let created_at = parse_datetime(timestamp.as_deref());
    let updated_at = parse_datetime(updated_at.as_deref()).or(created_at);
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
        updated_at: updated_at.map(|dt| dt.timestamp()).unwrap_or(0),
        status: ThreadStatus::NotLoaded,
        path: Some(path),
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
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn validate_dynamic_tools_rejects_unsupported_input_schema() {
        let tools = vec![ApiDynamicToolSpec {
            name: "my_tool".to_string(),
            description: "test".to_string(),
            input_schema: json!({"type": "null"}),
        }];
        let err = validate_dynamic_tools(&tools).expect_err("invalid schema");
        assert!(err.contains("my_tool"), "unexpected error: {err}");
    }

    #[test]
    fn validate_dynamic_tools_accepts_sanitizable_input_schema() {
        let tools = vec![ApiDynamicToolSpec {
            name: "my_tool".to_string(),
            description: "test".to_string(),
            // Missing `type` is common; core sanitizes these to a supported schema.
            input_schema: json!({"properties": {}}),
        }];
        validate_dynamic_tools(&tools).expect("valid schema");
    }

    #[test]
    fn extract_conversation_summary_prefers_plain_user_messages() -> Result<()> {
        let conversation_id = ThreadId::from_string("3f941c35-29b3-493b-b0a4-e25800d9aeb0")?;
        let timestamp = Some("2025-09-05T16:53:11.850Z".to_string());
        let path = PathBuf::from("rollout.jsonl");

        let head = vec![
            json!({
                "id": conversation_id.to_string(),
                "timestamp": timestamp,
                "cwd": "/",
                "originator": "codex",
                "cli_version": "0.0.0",
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

        let summary = extract_conversation_summary(
            path.clone(),
            &head,
            &session_meta,
            None,
            "test-provider",
            timestamp.clone(),
        )
        .expect("summary");

        let expected = ConversationSummary {
            conversation_id,
            timestamp: timestamp.clone(),
            updated_at: timestamp,
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
        use std::fs::FileTimes;

        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().join("rollout.jsonl");

        let conversation_id = ThreadId::from_string("bfd12a78-5900-467b-9bc5-d3d35df08191")?;
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
        let parsed = chrono::DateTime::parse_from_rfc3339(&timestamp)?.with_timezone(&Utc);
        let times = FileTimes::new().set_modified(parsed.into());
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)?
            .set_times(times)?;

        let summary = read_summary_from_rollout(path.as_path(), "fallback").await?;

        let expected = ConversationSummary {
            conversation_id,
            timestamp: Some(timestamp.clone()),
            updated_at: Some("2025-09-05T16:53:11Z".to_string()),
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

    #[tokio::test]
    async fn removing_one_listener_does_not_cancel_other_subscriptions_for_same_thread()
    -> Result<()> {
        let mut manager = ThreadStateManager::new();
        let thread_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let listener_a = Uuid::new_v4();
        let listener_b = Uuid::new_v4();
        let connection_a = ConnectionId(1);
        let connection_b = ConnectionId(2);
        let (cancel_tx, mut cancel_rx) = oneshot::channel();

        manager
            .set_listener(listener_a, thread_id, connection_a, false)
            .await;
        manager
            .set_listener(listener_b, thread_id, connection_b, false)
            .await;
        {
            let state = manager.thread_state(thread_id);
            state.lock().await.cancel_tx = Some(cancel_tx);
        }

        assert_eq!(manager.remove_listener(listener_a).await, Some(thread_id));
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut cancel_rx)
                .await
                .is_err()
        );
        assert_eq!(manager.remove_listener(listener_b).await, Some(thread_id));
        assert_eq!(cancel_rx.await, Ok(()));
        Ok(())
    }

    #[tokio::test]
    async fn removing_listener_unsubscribes_its_connection() -> Result<()> {
        let mut manager = ThreadStateManager::new();
        let thread_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let listener_a = Uuid::new_v4();
        let listener_b = Uuid::new_v4();
        let connection_a = ConnectionId(1);
        let connection_b = ConnectionId(2);

        manager
            .set_listener(listener_a, thread_id, connection_a, false)
            .await;
        manager
            .set_listener(listener_b, thread_id, connection_b, false)
            .await;

        assert_eq!(manager.remove_listener(listener_a).await, Some(thread_id));
        let state = manager.thread_state(thread_id);
        let subscribed_connection_ids = state.lock().await.subscribed_connection_ids();
        assert_eq!(subscribed_connection_ids, vec![connection_b]);
        Ok(())
    }

    #[tokio::test]
    async fn set_listener_uses_last_write_for_raw_events() -> Result<()> {
        let mut manager = ThreadStateManager::new();
        let thread_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let listener_a = Uuid::new_v4();
        let listener_b = Uuid::new_v4();
        let connection_a = ConnectionId(1);
        let connection_b = ConnectionId(2);

        manager
            .set_listener(listener_a, thread_id, connection_a, true)
            .await;
        {
            let state = manager.thread_state(thread_id);
            assert!(state.lock().await.experimental_raw_events);
        }
        manager
            .set_listener(listener_b, thread_id, connection_b, false)
            .await;
        let state = manager.thread_state(thread_id);
        assert!(!state.lock().await.experimental_raw_events);
        Ok(())
    }

    #[tokio::test]
    async fn removing_connection_clears_subscription_and_listener_when_last_subscriber()
    -> Result<()> {
        let mut manager = ThreadStateManager::new();
        let thread_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let listener = Uuid::new_v4();
        let connection = ConnectionId(1);
        let (cancel_tx, cancel_rx) = oneshot::channel();

        manager
            .set_listener(listener, thread_id, connection, false)
            .await;
        {
            let state = manager.thread_state(thread_id);
            state.lock().await.cancel_tx = Some(cancel_tx);
        }

        manager.remove_connection(connection).await;
        assert_eq!(cancel_rx.await, Ok(()));
        assert_eq!(manager.remove_listener(listener).await, None);

        let state = manager.thread_state(thread_id);
        assert!(state.lock().await.subscribed_connection_ids().is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn removing_auto_attached_connection_preserves_listener_for_other_connections()
    -> Result<()> {
        let mut manager = ThreadStateManager::new();
        let thread_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let connection_a = ConnectionId(1);
        let connection_b = ConnectionId(2);
        let (cancel_tx, mut cancel_rx) = oneshot::channel();

        manager
            .ensure_connection_subscribed(thread_id, connection_a, false)
            .await;
        manager
            .ensure_connection_subscribed(thread_id, connection_b, false)
            .await;
        {
            let state = manager.thread_state(thread_id);
            state.lock().await.cancel_tx = Some(cancel_tx);
        }

        manager.remove_connection(connection_a).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut cancel_rx)
                .await
                .is_err()
        );

        let state = manager.thread_state(thread_id);
        assert_eq!(
            state.lock().await.subscribed_connection_ids(),
            vec![connection_b]
        );
        Ok(())
    }
}
