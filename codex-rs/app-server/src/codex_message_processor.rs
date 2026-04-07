use crate::bespoke_event_handling::apply_bespoke_event_handling;
use crate::command_exec::CommandExecManager;
use crate::command_exec::StartCommandExecParams;
use crate::config_api::apply_runtime_feature_enablement;
use crate::error_code::INPUT_TOO_LARGE_ERROR_CODE;
use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_PARAMS_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use crate::fuzzy_file_search::FuzzyFileSearchSession;
use crate::fuzzy_file_search::run_fuzzy_file_search;
use crate::fuzzy_file_search::start_fuzzy_file_search_session;
use crate::models::supported_models;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::ConnectionRequestId;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::RequestContext;
use crate::outgoing_message::ThreadScopedOutgoingMessageSender;
use crate::thread_status::ThreadWatchManager;
use crate::thread_status::resolve_thread_status;
use chrono::DateTime;
use chrono::SecondsFormat;
use chrono::Utc;
use codex_analytics::AnalyticsEventsClient;
use codex_app_server_protocol::Account;
use codex_app_server_protocol::AccountLoginCompletedNotification;
use codex_app_server_protocol::AccountUpdatedNotification;
use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::AppsListParams;
use codex_app_server_protocol::AppsListResponse;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::AuthMode as CoreAuthMode;
use codex_app_server_protocol::CancelLoginAccountParams;
use codex_app_server_protocol::CancelLoginAccountResponse;
use codex_app_server_protocol::CancelLoginAccountStatus;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::CodexErrorInfo as AppServerCodexErrorInfo;
use codex_app_server_protocol::CollaborationModeListParams;
use codex_app_server_protocol::CollaborationModeListResponse;
use codex_app_server_protocol::CommandExecParams;
use codex_app_server_protocol::CommandExecResizeParams;
use codex_app_server_protocol::CommandExecTerminateParams;
use codex_app_server_protocol::CommandExecWriteParams;
use codex_app_server_protocol::ConversationGitInfo;
use codex_app_server_protocol::ConversationSummary;
use codex_app_server_protocol::DynamicToolSpec as ApiDynamicToolSpec;
use codex_app_server_protocol::ExperimentalFeature as ApiExperimentalFeature;
use codex_app_server_protocol::ExperimentalFeatureListParams;
use codex_app_server_protocol::ExperimentalFeatureListResponse;
use codex_app_server_protocol::ExperimentalFeatureStage as ApiExperimentalFeatureStage;
use codex_app_server_protocol::FeedbackUploadParams;
use codex_app_server_protocol::FeedbackUploadResponse;
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
use codex_app_server_protocol::GitDiffToRemoteResponse;
use codex_app_server_protocol::GitInfo as ApiGitInfo;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::ListMcpServerStatusResponse;
use codex_app_server_protocol::LoginAccountParams;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::LoginApiKeyParams;
use codex_app_server_protocol::LogoutAccountResponse;
use codex_app_server_protocol::MarketplaceInterface;
use codex_app_server_protocol::McpResourceReadParams;
use codex_app_server_protocol::McpResourceReadResponse;
use codex_app_server_protocol::McpServerOauthLoginCompletedNotification;
use codex_app_server_protocol::McpServerOauthLoginParams;
use codex_app_server_protocol::McpServerOauthLoginResponse;
use codex_app_server_protocol::McpServerRefreshResponse;
use codex_app_server_protocol::McpServerStatus;
use codex_app_server_protocol::McpServerStatusDetail;
use codex_app_server_protocol::MockExperimentalMethodParams;
use codex_app_server_protocol::MockExperimentalMethodResponse;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelListResponse;
use codex_app_server_protocol::PluginDetail;
use codex_app_server_protocol::PluginInstallParams;
use codex_app_server_protocol::PluginInstallResponse;
use codex_app_server_protocol::PluginInterface;
use codex_app_server_protocol::PluginListParams;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::PluginMarketplaceEntry;
use codex_app_server_protocol::PluginReadParams;
use codex_app_server_protocol::PluginReadResponse;
use codex_app_server_protocol::PluginSource;
use codex_app_server_protocol::PluginSummary;
use codex_app_server_protocol::PluginUninstallParams;
use codex_app_server_protocol::PluginUninstallResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ReviewDelivery as ApiReviewDelivery;
use codex_app_server_protocol::ReviewStartParams;
use codex_app_server_protocol::ReviewStartResponse;
use codex_app_server_protocol::ReviewTarget as ApiReviewTarget;
use codex_app_server_protocol::SandboxMode;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequestResolvedNotification;
use codex_app_server_protocol::SkillSummary;
use codex_app_server_protocol::SkillsConfigWriteParams;
use codex_app_server_protocol::SkillsConfigWriteResponse;
use codex_app_server_protocol::SkillsListParams;
use codex_app_server_protocol::SkillsListResponse;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadArchivedNotification;
use codex_app_server_protocol::ThreadBackgroundTerminalsCleanParams;
use codex_app_server_protocol::ThreadBackgroundTerminalsCleanResponse;
use codex_app_server_protocol::ThreadClosedNotification;
use codex_app_server_protocol::ThreadCompactStartParams;
use codex_app_server_protocol::ThreadCompactStartResponse;
use codex_app_server_protocol::ThreadDecrementElicitationParams;
use codex_app_server_protocol::ThreadDecrementElicitationResponse;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadIncrementElicitationParams;
use codex_app_server_protocol::ThreadIncrementElicitationResponse;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadMetadataGitInfoUpdateParams;
use codex_app_server_protocol::ThreadMetadataUpdateParams;
use codex_app_server_protocol::ThreadMetadataUpdateResponse;
use codex_app_server_protocol::ThreadNameUpdatedNotification;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadRealtimeAppendAudioParams;
use codex_app_server_protocol::ThreadRealtimeAppendAudioResponse;
use codex_app_server_protocol::ThreadRealtimeAppendTextParams;
use codex_app_server_protocol::ThreadRealtimeAppendTextResponse;
use codex_app_server_protocol::ThreadRealtimeStartParams;
use codex_app_server_protocol::ThreadRealtimeStartResponse;
use codex_app_server_protocol::ThreadRealtimeStopParams;
use codex_app_server_protocol::ThreadRealtimeStopResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadRollbackParams;
use codex_app_server_protocol::ThreadSetNameParams;
use codex_app_server_protocol::ThreadSetNameResponse;
use codex_app_server_protocol::ThreadShellCommandParams;
use codex_app_server_protocol::ThreadShellCommandResponse;
use codex_app_server_protocol::ThreadSortKey;
use codex_app_server_protocol::ThreadSourceKind;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStartedNotification;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::ThreadUnarchiveParams;
use codex_app_server_protocol::ThreadUnarchiveResponse;
use codex_app_server_protocol::ThreadUnarchivedNotification;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::ThreadUnsubscribeStatus;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnError;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::TurnSteerParams;
use codex_app_server_protocol::TurnSteerResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_app_server_protocol::WindowsSandboxSetupCompletedNotification;
use codex_app_server_protocol::WindowsSandboxSetupMode;
use codex_app_server_protocol::WindowsSandboxSetupStartParams;
use codex_app_server_protocol::WindowsSandboxSetupStartResponse;
use codex_app_server_protocol::build_turns_from_rollout_items;
use codex_arg0::Arg0DispatchPaths;
use codex_backend_client::Client as BackendClient;
use codex_chatgpt::connectors;
use codex_cloud_requirements::cloud_requirements_loader;
use codex_config::types::McpServerTransportConfig;
use codex_core::CodexThread;
use codex_core::Cursor as RolloutCursor;
use codex_core::ForkSnapshot;
use codex_core::NewThread;
use codex_core::RolloutRecorder;
use codex_core::SessionMeta;
use codex_core::SteerInputError;
use codex_core::ThreadConfigSnapshot;
use codex_core::ThreadManager;
use codex_core::ThreadSortKey as CoreThreadSortKey;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::NetworkProxyAuditMetadata;
use codex_core::config::edit::ConfigEdit;
use codex_core::config::edit::ConfigEditsBuilder;
use codex_core::config_loader::CloudRequirementsLoadError;
use codex_core::config_loader::CloudRequirementsLoadErrorCode;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::LoaderOverrides;
use codex_core::config_loader::load_config_layers_state;
use codex_core::exec::ExecCapturePolicy;
use codex_core::exec::ExecExpiration;
use codex_core::exec::ExecParams;
use codex_core::exec_env::create_env;
use codex_core::find_archived_thread_path_by_id_str;
use codex_core::find_thread_name_by_id;
use codex_core::find_thread_names_by_ids;
use codex_core::find_thread_path_by_id_str;
use codex_core::parse_cursor;
use codex_core::plugins::MarketplaceError;
use codex_core::plugins::MarketplacePluginSource;
use codex_core::plugins::OPENAI_CURATED_MARKETPLACE_NAME;
use codex_core::plugins::PluginInstallError as CorePluginInstallError;
use codex_core::plugins::PluginInstallRequest;
use codex_core::plugins::PluginReadRequest;
use codex_core::plugins::PluginUninstallError as CorePluginUninstallError;
use codex_core::plugins::load_plugin_apps;
use codex_core::plugins::load_plugin_mcp_servers;
use codex_core::read_head_for_summary;
use codex_core::read_session_meta_line;
use codex_core::rollout_date_parts;
use codex_core::sandboxing::SandboxPermissions;
use codex_core::windows_sandbox::WindowsSandboxLevelExt;
use codex_core::windows_sandbox::WindowsSandboxSetupMode as CoreWindowsSandboxSetupMode;
use codex_core::windows_sandbox::WindowsSandboxSetupRequest;
use codex_features::FEATURES;
use codex_features::Feature;
use codex_features::Stage;
use codex_feedback::CodexFeedback;
use codex_git_utils::git_diff_to_remote;
use codex_git_utils::resolve_root_git_project_for_trust;
use codex_login::AuthManager;
use codex_login::CLIENT_ID;
use codex_login::CodexAuth;
use codex_login::ServerOptions as LoginServerOptions;
use codex_login::ShutdownHandle;
use codex_login::auth::login_with_chatgpt_auth_tokens;
use codex_login::complete_device_code_login;
use codex_login::default_client::set_default_client_residency_requirement;
use codex_login::login_with_api_key;
use codex_login::request_device_code;
use codex_login::run_login_server;
use codex_mcp::mcp::McpSnapshotDetail;
use codex_mcp::mcp::auth::discover_supported_scopes;
use codex_mcp::mcp::auth::resolve_oauth_scopes;
use codex_mcp::mcp::collect_mcp_snapshot_with_detail;
use codex_mcp::mcp::effective_mcp_servers;
use codex_mcp::mcp::qualified_mcp_tool_name_prefix;
use codex_models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_protocol::ThreadId;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ForcedLoginMethod;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::TrustLevel;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::dynamic_tools::DynamicToolSpec as CoreDynamicToolSpec;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::ConversationAudioParams;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::ConversationTextParams;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GitInfo as CoreGitInfo;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::McpAuthStatus as CoreMcpAuthStatus;
use codex_protocol::protocol::McpServerRefreshConfig;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RateLimitSnapshot as CoreRateLimitSnapshot;
use codex_protocol::protocol::ReviewDelivery as CoreReviewDelivery;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::ReviewTarget as CoreReviewTarget;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::USER_MESSAGE_BEGIN;
use codex_protocol::protocol::W3cTraceContext;
use codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS;
use codex_protocol::user_input::UserInput as CoreInputItem;
use codex_rmcp_client::perform_oauth_login_return_url;
use codex_rollout::state_db::StateDbHandle;
use codex_rollout::state_db::get_state_db;
use codex_rollout::state_db::reconcile_rollout;
use codex_state::StateRuntime;
use codex_state::ThreadMetadata;
use codex_state::ThreadMetadataBuilder;
use codex_state::log_db::LogDbLayer;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_json_to_toml::json_to_toml;
use codex_utils_pty::DEFAULT_OUTPUT_BYTES_CAP;
use std::collections::BTreeMap;
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
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use toml::Value as TomlValue;
use tracing::Instrument;
use tracing::error;
use tracing::info;
use tracing::warn;
use uuid::Uuid;

#[cfg(test)]
use codex_app_server_protocol::ServerRequest;

mod apps_list_helpers;
mod plugin_app_helpers;
mod plugin_mcp_oauth;

use crate::filters::compute_source_filters;
use crate::filters::source_kind_matches;
use crate::thread_state::ThreadListenerCommand;
use crate::thread_state::ThreadState;
use crate::thread_state::ThreadStateManager;

const THREAD_LIST_DEFAULT_LIMIT: usize = 25;
const THREAD_LIST_MAX_LIMIT: usize = 100;

struct ThreadListFilters {
    model_providers: Option<Vec<String>>,
    source_kinds: Option<Vec<ThreadSourceKind>>,
    archived: bool,
    cwd: Option<PathBuf>,
    search_term: Option<String>,
}

// Duration before a browser ChatGPT login attempt is abandoned.
const LOGIN_CHATGPT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const LOGIN_ISSUER_OVERRIDE_ENV_VAR: &str = "CODEX_APP_SERVER_LOGIN_ISSUER";
const APP_LIST_LOAD_TIMEOUT: Duration = Duration::from_secs(90);

enum ActiveLogin {
    Browser {
        shutdown_handle: ShutdownHandle,
        login_id: Uuid,
    },
    DeviceCode {
        cancel: CancellationToken,
        login_id: Uuid,
    },
}

impl ActiveLogin {
    fn login_id(&self) -> Uuid {
        match self {
            ActiveLogin::Browser { login_id, .. } | ActiveLogin::DeviceCode { login_id, .. } => {
                *login_id
            }
        }
    }

    fn cancel(&self) {
        match self {
            ActiveLogin::Browser {
                shutdown_handle, ..
            } => shutdown_handle.shutdown(),
            ActiveLogin::DeviceCode { cancel, .. } => cancel.cancel(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum CancelLoginError {
    NotFound,
}

enum AppListLoadResult {
    Accessible(Result<Vec<AppInfo>, String>),
    Directory(Result<Vec<AppInfo>, String>),
}

enum ThreadShutdownResult {
    Complete,
    SubmitFailed,
    TimedOut,
}

impl Drop for ActiveLogin {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Handles JSON-RPC messages for Codex threads (and legacy conversation APIs).
pub(crate) struct CodexMessageProcessor {
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    analytics_events_client: AnalyticsEventsClient,
    arg0_paths: Arg0DispatchPaths,
    config: Arc<Config>,
    cli_overrides: Arc<RwLock<Vec<(String, TomlValue)>>>,
    runtime_feature_enablement: Arc<RwLock<BTreeMap<String, bool>>>,
    cloud_requirements: Arc<RwLock<CloudRequirementsLoader>>,
    active_login: Arc<Mutex<Option<ActiveLogin>>>,
    pending_thread_unloads: Arc<Mutex<HashSet<ThreadId>>>,
    thread_state_manager: ThreadStateManager,
    thread_watch_manager: ThreadWatchManager,
    command_exec_manager: CommandExecManager,
    pending_fuzzy_searches: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    fuzzy_search_sessions: Arc<Mutex<HashMap<String, FuzzyFileSearchSession>>>,
    background_tasks: TaskTracker,
    feedback: CodexFeedback,
    log_db: Option<LogDbLayer>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ApiVersion {
    #[allow(dead_code)]
    V1,
    #[default]
    V2,
}

#[derive(Clone)]
struct ListenerTaskContext {
    thread_manager: Arc<ThreadManager>,
    thread_state_manager: ThreadStateManager,
    outgoing: Arc<OutgoingMessageSender>,
    analytics_events_client: AnalyticsEventsClient,
    general_analytics_enabled: bool,
    thread_watch_manager: ThreadWatchManager,
    fallback_model_provider: String,
    codex_home: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnsureConversationListenerResult {
    Attached,
    ConnectionClosed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RefreshTokenRequestOutcome {
    NotAttemptedOrSucceeded,
    FailedTransiently,
    FailedPermanently,
}

pub(crate) struct CodexMessageProcessorArgs {
    pub(crate) auth_manager: Arc<AuthManager>,
    pub(crate) thread_manager: Arc<ThreadManager>,
    pub(crate) outgoing: Arc<OutgoingMessageSender>,
    pub(crate) analytics_events_client: AnalyticsEventsClient,
    pub(crate) arg0_paths: Arg0DispatchPaths,
    pub(crate) config: Arc<Config>,
    pub(crate) cli_overrides: Arc<RwLock<Vec<(String, TomlValue)>>>,
    pub(crate) runtime_feature_enablement: Arc<RwLock<BTreeMap<String, bool>>>,
    pub(crate) cloud_requirements: Arc<RwLock<CloudRequirementsLoader>>,
    pub(crate) feedback: CodexFeedback,
    pub(crate) log_db: Option<LogDbLayer>,
}

impl CodexMessageProcessor {
    pub(crate) fn handle_config_mutation(&self) {
        self.clear_plugin_related_caches();
    }

    fn clear_plugin_related_caches(&self) {
        self.thread_manager.plugins_manager().clear_cache();
        self.thread_manager.skills_manager().clear_cache();
    }

    fn current_account_updated_notification(&self) -> AccountUpdatedNotification {
        let auth = self.auth_manager.auth_cached();
        AccountUpdatedNotification {
            auth_mode: auth.as_ref().map(CodexAuth::api_auth_mode),
            plan_type: auth.as_ref().and_then(CodexAuth::account_plan_type),
        }
    }

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
            analytics_events_client,
            arg0_paths,
            config,
            cli_overrides,
            runtime_feature_enablement,
            cloud_requirements,
            feedback,
            log_db,
        } = args;
        Self {
            auth_manager,
            thread_manager,
            outgoing: outgoing.clone(),
            analytics_events_client,
            arg0_paths,
            config,
            cli_overrides,
            runtime_feature_enablement,
            cloud_requirements,
            active_login: Arc::new(Mutex::new(None)),
            pending_thread_unloads: Arc::new(Mutex::new(HashSet::new())),
            thread_state_manager: ThreadStateManager::new(),
            thread_watch_manager: ThreadWatchManager::new_with_outgoing(outgoing),
            command_exec_manager: CommandExecManager::default(),
            pending_fuzzy_searches: Arc::new(Mutex::new(HashMap::new())),
            fuzzy_search_sessions: Arc::new(Mutex::new(HashMap::new())),
            background_tasks: TaskTracker::new(),
            feedback,
            log_db,
        }
    }

    async fn load_latest_config(
        &self,
        fallback_cwd: Option<PathBuf>,
    ) -> Result<Config, JSONRPCErrorError> {
        let fallback_cwd = fallback_cwd
            .map(AbsolutePathBuf::relative_to_current_dir)
            .transpose()
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("invalid fallback cwd: {err}"),
                data: None,
            })?;
        let cloud_requirements = self.current_cloud_requirements();
        let mut config = codex_core::config::ConfigBuilder::default()
            .cli_overrides(self.current_cli_overrides())
            .fallback_cwd(fallback_cwd)
            .cloud_requirements(cloud_requirements)
            .build()
            .await
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to reload config: {err}"),
                data: None,
            })?;
        apply_runtime_feature_enablement(&mut config, &self.current_runtime_feature_enablement());
        config.codex_self_exe = self.arg0_paths.codex_self_exe.clone();
        config.codex_linux_sandbox_exe = self.arg0_paths.codex_linux_sandbox_exe.clone();
        config.main_execve_wrapper_exe = self.arg0_paths.main_execve_wrapper_exe.clone();
        Ok(config)
    }

    fn current_cloud_requirements(&self) -> CloudRequirementsLoader {
        self.cloud_requirements
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    fn current_cli_overrides(&self) -> Vec<(String, TomlValue)> {
        self.cli_overrides
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    fn current_runtime_feature_enablement(&self) -> BTreeMap<String, bool> {
        self.runtime_feature_enablement
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// If a client sends `developer_instructions: null` during a mode switch,
    /// use the built-in instructions for that mode.
    fn normalize_turn_start_collaboration_mode(
        &self,
        mut collaboration_mode: CollaborationMode,
        collaboration_modes_config: CollaborationModesConfig,
    ) -> CollaborationMode {
        if collaboration_mode.settings.developer_instructions.is_none()
            && let Some(instructions) = self
                .thread_manager
                .get_models_manager()
                .list_collaboration_modes_for_config(collaboration_modes_config)
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

    pub async fn process_request(
        &mut self,
        connection_id: ConnectionId,
        request: ClientRequest,
        app_server_client_name: Option<String>,
        app_server_client_version: Option<String>,
        request_context: RequestContext,
    ) {
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
                self.thread_start(
                    to_connection_request_id(request_id),
                    params,
                    app_server_client_name.clone(),
                    app_server_client_version.clone(),
                    request_context,
                )
                .await;
            }
            ClientRequest::ThreadUnsubscribe { request_id, params } => {
                self.thread_unsubscribe(to_connection_request_id(request_id), params)
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
            ClientRequest::ThreadIncrementElicitation { request_id, params } => {
                self.thread_increment_elicitation(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ThreadDecrementElicitation { request_id, params } => {
                self.thread_decrement_elicitation(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ThreadSetName { request_id, params } => {
                self.thread_set_name(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ThreadMetadataUpdate { request_id, params } => {
                self.thread_metadata_update(to_connection_request_id(request_id), params)
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
            ClientRequest::ThreadShellCommand { request_id, params } => {
                self.thread_shell_command(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::SkillsList { request_id, params } => {
                self.skills_list(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::PluginList { request_id, params } => {
                self.plugin_list(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::PluginRead { request_id, params } => {
                self.plugin_read(to_connection_request_id(request_id), params)
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
            ClientRequest::PluginInstall { request_id, params } => {
                self.plugin_install(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::PluginUninstall { request_id, params } => {
                self.plugin_uninstall(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::TurnStart { request_id, params } => {
                self.turn_start(
                    to_connection_request_id(request_id),
                    params,
                    app_server_client_name.clone(),
                    app_server_client_version.clone(),
                )
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
            ClientRequest::ThreadRealtimeStart { request_id, params } => {
                self.thread_realtime_start(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ThreadRealtimeAppendAudio { request_id, params } => {
                self.thread_realtime_append_audio(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ThreadRealtimeAppendText { request_id, params } => {
                self.thread_realtime_append_text(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ThreadRealtimeStop { request_id, params } => {
                self.thread_realtime_stop(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ReviewStart { request_id, params } => {
                self.review_start(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::GetConversationSummary { request_id, params } => {
                self.get_thread_summary(to_connection_request_id(request_id), params)
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
            ClientRequest::McpResourceRead { request_id, params } => {
                self.read_mcp_resource(to_connection_request_id(request_id), params)
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
            ClientRequest::GitDiffToRemote { request_id, params } => {
                self.git_diff_to_origin(to_connection_request_id(request_id), params.cwd)
                    .await;
            }
            ClientRequest::GetAuthStatus { request_id, params } => {
                self.get_auth_status(to_connection_request_id(request_id), params)
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
            ClientRequest::CommandExecWrite { request_id, params } => {
                self.command_exec_write(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::CommandExecResize { request_id, params } => {
                self.command_exec_resize(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::CommandExecTerminate { request_id, params } => {
                self.command_exec_terminate(to_connection_request_id(request_id), params)
                    .await;
            }
            ClientRequest::ConfigRead { .. }
            | ClientRequest::ConfigValueWrite { .. }
            | ClientRequest::ConfigBatchWrite { .. }
            | ClientRequest::ExperimentalFeatureEnablementSet { .. } => {
                warn!("Config request reached CodexMessageProcessor unexpectedly");
            }
            ClientRequest::FsReadFile { .. }
            | ClientRequest::FsWriteFile { .. }
            | ClientRequest::FsCreateDirectory { .. }
            | ClientRequest::FsGetMetadata { .. }
            | ClientRequest::FsReadDirectory { .. }
            | ClientRequest::FsRemove { .. }
            | ClientRequest::FsCopy { .. }
            | ClientRequest::FsWatch { .. }
            | ClientRequest::FsUnwatch { .. } => {
                warn!("Filesystem request reached CodexMessageProcessor unexpectedly");
            }
            ClientRequest::ConfigRequirementsRead { .. } => {
                warn!("ConfigRequirementsRead request reached CodexMessageProcessor unexpectedly");
            }
            ClientRequest::ExternalAgentConfigDetect { .. }
            | ClientRequest::ExternalAgentConfigImport { .. } => {
                warn!("ExternalAgentConfig request reached CodexMessageProcessor unexpectedly");
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
            LoginAccountParams::ChatgptDeviceCode => {
                self.login_chatgpt_device_code_v2(request_id).await;
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
        if self.auth_manager.is_external_chatgpt_auth_active() {
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

                self.outgoing
                    .send_server_notification(ServerNotification::AccountUpdated(
                        self.current_account_updated_notification(),
                    ))
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

        if self.auth_manager.is_external_chatgpt_auth_active() {
            return Err(self.external_auth_active_error());
        }

        if matches!(config.forced_login_method, Some(ForcedLoginMethod::Api)) {
            return Err(JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: "ChatGPT login is disabled. Use API key login instead.".to_string(),
                data: None,
            });
        }

        let mut opts = LoginServerOptions {
            open_browser: false,
            ..LoginServerOptions::new(
                config.codex_home.clone(),
                CLIENT_ID.to_string(),
                config.forced_chatgpt_workspace_id.clone(),
                config.cli_auth_credentials_store_mode,
            )
        };
        #[cfg(debug_assertions)]
        if let Ok(issuer) = std::env::var(LOGIN_ISSUER_OVERRIDE_ENV_VAR)
            && !issuer.trim().is_empty()
        {
            opts.issuer = issuer;
        }

        Ok(opts)
    }

    fn login_chatgpt_device_code_start_error(err: IoError) -> JSONRPCErrorError {
        let is_not_found = err.kind() == std::io::ErrorKind::NotFound;
        JSONRPCErrorError {
            code: if is_not_found {
                INVALID_REQUEST_ERROR_CODE
            } else {
                INTERNAL_ERROR_CODE
            },
            message: if is_not_found {
                err.to_string()
            } else {
                format!("failed to request device code: {err}")
            },
            data: None,
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
                        *guard = Some(ActiveLogin::Browser {
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
                    let cli_overrides = self.current_cli_overrides();
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
                            let auth = auth_manager.auth_cached();
                            let payload_v2 = AccountUpdatedNotification {
                                auth_mode: auth.as_ref().map(CodexAuth::api_auth_mode),
                                plan_type: auth.as_ref().and_then(CodexAuth::account_plan_type),
                            };
                            outgoing_clone
                                .send_server_notification(ServerNotification::AccountUpdated(
                                    payload_v2,
                                ))
                                .await;
                        }

                        // Clear the active login if it matches this attempt. It may have been replaced or cancelled.
                        let mut guard = active_login.lock().await;
                        if guard.as_ref().map(ActiveLogin::login_id) == Some(login_id) {
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

    async fn login_chatgpt_device_code_v2(&mut self, request_id: ConnectionRequestId) {
        match self.login_chatgpt_common().await {
            Ok(opts) => match request_device_code(&opts).await {
                Ok(device_code) => {
                    let login_id = Uuid::new_v4();
                    let cancel = CancellationToken::new();

                    {
                        let mut guard = self.active_login.lock().await;
                        if let Some(existing) = guard.take() {
                            drop(existing);
                        }
                        *guard = Some(ActiveLogin::DeviceCode {
                            cancel: cancel.clone(),
                            login_id,
                        });
                    }

                    let verification_url = device_code.verification_url.clone();
                    let user_code = device_code.user_code.clone();
                    let response =
                        codex_app_server_protocol::LoginAccountResponse::ChatgptDeviceCode {
                            login_id: login_id.to_string(),
                            verification_url,
                            user_code,
                        };
                    self.outgoing.send_response(request_id, response).await;

                    let outgoing_clone = self.outgoing.clone();
                    let active_login = self.active_login.clone();
                    let auth_manager = self.auth_manager.clone();
                    let cloud_requirements = self.cloud_requirements.clone();
                    let chatgpt_base_url = self.config.chatgpt_base_url.clone();
                    let codex_home = self.config.codex_home.clone();
                    let cli_overrides = self.current_cli_overrides();
                    tokio::spawn(async move {
                        let (success, error_msg) = tokio::select! {
                            _ = cancel.cancelled() => {
                                (false, Some("Login was not completed".to_string()))
                            }
                            r = complete_device_code_login(opts, device_code) => {
                                match r {
                                    Ok(()) => (true, None),
                                    Err(err) => (false, Some(err.to_string())),
                                }
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

                            let auth = auth_manager.auth_cached();
                            let payload_v2 = AccountUpdatedNotification {
                                auth_mode: auth.as_ref().map(CodexAuth::api_auth_mode),
                                plan_type: auth.as_ref().and_then(CodexAuth::account_plan_type),
                            };
                            outgoing_clone
                                .send_server_notification(ServerNotification::AccountUpdated(
                                    payload_v2,
                                ))
                                .await;
                        }

                        let mut guard = active_login.lock().await;
                        if guard.as_ref().map(ActiveLogin::login_id) == Some(login_id) {
                            *guard = None;
                        }
                    });
                }
                Err(err) => {
                    let error = Self::login_chatgpt_device_code_start_error(err);
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
        if guard.as_ref().map(ActiveLogin::login_id) == Some(login_id) {
            if let Some(active) = guard.take() {
                drop(active);
            }
            Ok(())
        } else {
            Err(CancelLoginError::NotFound)
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
                    Err(CancelLoginError::NotFound) => CancelLoginAccountStatus::NotFound,
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
        let cli_overrides = self.current_cli_overrides();
        sync_default_client_residency_requirement(&cli_overrides, self.cloud_requirements.as_ref())
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

        self.outgoing
            .send_server_notification(ServerNotification::AccountUpdated(
                self.current_account_updated_notification(),
            ))
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

    async fn logout_v2(&mut self, request_id: ConnectionRequestId) {
        match self.logout_common().await {
            Ok(current_auth_method) => {
                self.outgoing
                    .send_response(request_id, LogoutAccountResponse {})
                    .await;

                let payload_v2 = AccountUpdatedNotification {
                    auth_mode: current_auth_method,
                    plan_type: None,
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

    async fn refresh_token_if_requested(&self, do_refresh: bool) -> RefreshTokenRequestOutcome {
        if self.auth_manager.is_external_chatgpt_auth_active() {
            return RefreshTokenRequestOutcome::NotAttemptedOrSucceeded;
        }
        if do_refresh && let Err(err) = self.auth_manager.refresh_token().await {
            let failed_reason = err.failed_reason();
            if failed_reason.is_none() {
                tracing::warn!("failed to refresh token while getting account: {err}");
                return RefreshTokenRequestOutcome::FailedTransiently;
            }
            return RefreshTokenRequestOutcome::FailedPermanently;
        }
        RefreshTokenRequestOutcome::NotAttemptedOrSucceeded
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
            let auth = if do_refresh {
                self.auth_manager.auth_cached()
            } else {
                self.auth_manager.auth().await
            };
            match auth {
                Some(auth) => {
                    let permanent_refresh_failure =
                        self.auth_manager.refresh_failure_for_auth(&auth).is_some();
                    let auth_mode = auth.api_auth_mode();
                    let (reported_auth_method, token_opt) =
                        if include_token && permanent_refresh_failure {
                            (Some(auth_mode), None)
                        } else {
                            match auth.get_token() {
                                Ok(token) if !token.is_empty() => {
                                    let tok = if include_token { Some(token) } else { None };
                                    (Some(auth_mode), tok)
                                }
                                Ok(_) => (None, None),
                                Err(err) => {
                                    tracing::warn!("failed to get token for auth status: {err}");
                                    (None, None)
                                }
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
                CoreAuthMode::Chatgpt | CoreAuthMode::ChatgptAuthTokens => {
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

        let CommandExecParams {
            command,
            process_id,
            tty,
            stream_stdin,
            stream_stdout_stderr,
            output_bytes_cap,
            disable_output_cap,
            disable_timeout,
            timeout_ms,
            cwd,
            env: env_overrides,
            size,
            sandbox_policy,
        } = params;

        if size.is_some() && !tty {
            let error = JSONRPCErrorError {
                code: INVALID_PARAMS_ERROR_CODE,
                message: "command/exec size requires tty: true".to_string(),
                data: None,
            };
            self.outgoing.send_error(request, error).await;
            return;
        }

        if disable_output_cap && output_bytes_cap.is_some() {
            let error = JSONRPCErrorError {
                code: INVALID_PARAMS_ERROR_CODE,
                message: "command/exec cannot set both outputBytesCap and disableOutputCap"
                    .to_string(),
                data: None,
            };
            self.outgoing.send_error(request, error).await;
            return;
        }

        if disable_timeout && timeout_ms.is_some() {
            let error = JSONRPCErrorError {
                code: INVALID_PARAMS_ERROR_CODE,
                message: "command/exec cannot set both timeoutMs and disableTimeout".to_string(),
                data: None,
            };
            self.outgoing.send_error(request, error).await;
            return;
        }

        let cwd = match cwd {
            Some(cwd) => match AbsolutePathBuf::relative_to_current_dir(cwd) {
                Ok(cwd) => cwd,
                Err(err) => {
                    let error = JSONRPCErrorError {
                        code: INVALID_PARAMS_ERROR_CODE,
                        message: format!("invalid command/exec cwd: {err}"),
                        data: None,
                    };
                    self.outgoing.send_error(request, error).await;
                    return;
                }
            },
            None => self.config.cwd.clone(),
        };
        let mut env = create_env(
            &self.config.permissions.shell_environment_policy,
            /*thread_id*/ None,
        );
        if let Some(env_overrides) = env_overrides {
            for (key, value) in env_overrides {
                match value {
                    Some(value) => {
                        env.insert(key, value);
                    }
                    None => {
                        env.remove(&key);
                    }
                }
            }
        }
        let timeout_ms = match timeout_ms {
            Some(timeout_ms) => match u64::try_from(timeout_ms) {
                Ok(timeout_ms) => Some(timeout_ms),
                Err(_) => {
                    let error = JSONRPCErrorError {
                        code: INVALID_PARAMS_ERROR_CODE,
                        message: format!(
                            "command/exec timeoutMs must be non-negative, got {timeout_ms}"
                        ),
                        data: None,
                    };
                    self.outgoing.send_error(request, error).await;
                    return;
                }
            },
            None => None,
        };
        let managed_network_requirements_enabled =
            self.config.managed_network_requirements_enabled();
        let started_network_proxy = match self.config.permissions.network.as_ref() {
            Some(spec) => match spec
                .start_proxy(
                    self.config.permissions.sandbox_policy.get(),
                    /*policy_decider*/ None,
                    /*blocked_request_observer*/ None,
                    managed_network_requirements_enabled,
                    NetworkProxyAuditMetadata::default(),
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
        let output_bytes_cap = if disable_output_cap {
            None
        } else {
            Some(output_bytes_cap.unwrap_or(DEFAULT_OUTPUT_BYTES_CAP))
        };
        let expiration = if disable_timeout {
            ExecExpiration::Cancellation(CancellationToken::new())
        } else {
            match timeout_ms {
                Some(timeout_ms) => timeout_ms.into(),
                None => ExecExpiration::DefaultTimeout,
            }
        };
        let capture_policy = if disable_output_cap {
            ExecCapturePolicy::FullBuffer
        } else {
            ExecCapturePolicy::ShellTool
        };
        let sandbox_cwd = self.config.cwd.clone();
        let exec_params = ExecParams {
            command,
            cwd: cwd.clone(),
            expiration,
            capture_policy,
            env,
            network: started_network_proxy
                .as_ref()
                .map(codex_core::config::StartedNetworkProxy::proxy),
            sandbox_permissions: SandboxPermissions::UseDefault,
            windows_sandbox_level,
            windows_sandbox_private_desktop: self
                .config
                .permissions
                .windows_sandbox_private_desktop,
            justification: None,
            arg0: None,
        };

        let requested_policy = sandbox_policy.map(|policy| policy.to_core());
        let (
            effective_policy,
            effective_file_system_sandbox_policy,
            effective_network_sandbox_policy,
        ) = match requested_policy {
            Some(policy) => match self.config.permissions.sandbox_policy.can_set(&policy) {
                Ok(()) => {
                    let file_system_sandbox_policy =
                        codex_protocol::permissions::FileSystemSandboxPolicy::from_legacy_sandbox_policy(&policy, &sandbox_cwd);
                    let network_sandbox_policy =
                        codex_protocol::permissions::NetworkSandboxPolicy::from(&policy);
                    (policy, file_system_sandbox_policy, network_sandbox_policy)
                }
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
            None => (
                self.config.permissions.sandbox_policy.get().clone(),
                self.config.permissions.file_system_sandbox_policy.clone(),
                self.config.permissions.network_sandbox_policy,
            ),
        };

        let codex_linux_sandbox_exe = self.arg0_paths.codex_linux_sandbox_exe.clone();
        let outgoing = self.outgoing.clone();
        let request_for_task = request.clone();
        let started_network_proxy_for_task = started_network_proxy;
        let use_legacy_landlock = self.config.features.use_legacy_landlock();
        let size = match size.map(crate::command_exec::terminal_size_from_protocol) {
            Some(Ok(size)) => Some(size),
            Some(Err(error)) => {
                self.outgoing.send_error(request, error).await;
                return;
            }
            None => None,
        };

        match codex_core::exec::build_exec_request(
            exec_params,
            &effective_policy,
            &effective_file_system_sandbox_policy,
            effective_network_sandbox_policy,
            sandbox_cwd.as_path(),
            &codex_linux_sandbox_exe,
            use_legacy_landlock,
        ) {
            Ok(exec_request) => {
                if let Err(error) = self
                    .command_exec_manager
                    .start(StartCommandExecParams {
                        outgoing,
                        request_id: request_for_task,
                        process_id,
                        exec_request,
                        started_network_proxy: started_network_proxy_for_task,
                        tty,
                        stream_stdin,
                        stream_stdout_stderr,
                        output_bytes_cap,
                        size,
                    })
                    .await
                {
                    self.outgoing.send_error(request, error).await;
                }
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("exec failed: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request, error).await;
            }
        }
    }

    async fn command_exec_write(
        &self,
        request_id: ConnectionRequestId,
        params: CommandExecWriteParams,
    ) {
        match self
            .command_exec_manager
            .write(request_id.clone(), params)
            .await
        {
            Ok(response) => self.outgoing.send_response(request_id, response).await,
            Err(error) => self.outgoing.send_error(request_id, error).await,
        }
    }

    async fn command_exec_resize(
        &self,
        request_id: ConnectionRequestId,
        params: CommandExecResizeParams,
    ) {
        match self
            .command_exec_manager
            .resize(request_id.clone(), params)
            .await
        {
            Ok(response) => self.outgoing.send_response(request_id, response).await,
            Err(error) => self.outgoing.send_error(request_id, error).await,
        }
    }

    async fn command_exec_terminate(
        &self,
        request_id: ConnectionRequestId,
        params: CommandExecTerminateParams,
    ) {
        match self
            .command_exec_manager
            .terminate(request_id.clone(), params)
            .await
        {
            Ok(response) => self.outgoing.send_response(request_id, response).await,
            Err(error) => self.outgoing.send_error(request_id, error).await,
        }
    }

    async fn thread_start(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadStartParams,
        app_server_client_name: Option<String>,
        app_server_client_version: Option<String>,
        request_context: RequestContext,
    ) {
        let ThreadStartParams {
            model,
            model_provider,
            service_tier,
            cwd,
            approval_policy,
            approvals_reviewer,
            sandbox,
            config,
            service_name,
            base_instructions,
            developer_instructions,
            dynamic_tools,
            mock_experimental_field: _mock_experimental_field,
            experimental_raw_events,
            personality,
            ephemeral,
            persist_extended_history,
        } = params;
        let mut typesafe_overrides = match self.build_thread_config_overrides(
            model,
            model_provider,
            service_tier,
            cwd,
            approval_policy,
            approvals_reviewer,
            sandbox,
            base_instructions,
            developer_instructions,
            personality,
        ) {
            Ok(overrides) => overrides,
            Err(error) => {
                self.outgoing.send_error(request_id.clone(), error).await;
                return;
            }
        };
        typesafe_overrides.ephemeral = ephemeral;
        let cloud_requirements = self.current_cloud_requirements();
        let cli_overrides = self.current_cli_overrides();
        let listener_task_context = ListenerTaskContext {
            thread_manager: Arc::clone(&self.thread_manager),
            thread_state_manager: self.thread_state_manager.clone(),
            outgoing: Arc::clone(&self.outgoing),
            analytics_events_client: self.analytics_events_client.clone(),
            general_analytics_enabled: self.config.features.enabled(Feature::GeneralAnalytics),
            thread_watch_manager: self.thread_watch_manager.clone(),
            fallback_model_provider: self.config.model_provider_id.clone(),
            codex_home: self.config.codex_home.clone(),
        };
        let request_trace = request_context.request_trace();
        let runtime_feature_enablement = self.current_runtime_feature_enablement();
        let thread_start_task = async move {
            Self::thread_start_task(
                listener_task_context,
                cli_overrides,
                runtime_feature_enablement,
                cloud_requirements,
                request_id,
                app_server_client_name,
                app_server_client_version,
                config,
                typesafe_overrides,
                dynamic_tools,
                persist_extended_history,
                service_name,
                experimental_raw_events,
                request_trace,
            )
            .await;
        };
        self.background_tasks
            .spawn(thread_start_task.instrument(request_context.span()));
    }

    pub(crate) async fn drain_background_tasks(&self) {
        self.background_tasks.close();
        if tokio::time::timeout(Duration::from_secs(10), self.background_tasks.wait())
            .await
            .is_err()
        {
            warn!("timed out waiting for background tasks to shut down; proceeding");
        }
    }

    pub(crate) async fn cancel_active_login(&self) {
        let mut guard = self.active_login.lock().await;
        if let Some(active_login) = guard.take() {
            drop(active_login);
        }
    }

    pub(crate) async fn clear_all_thread_listeners(&self) {
        self.thread_state_manager.clear_all_listeners().await;
    }

    pub(crate) async fn shutdown_threads(&self) {
        let report = self
            .thread_manager
            .shutdown_all_threads_bounded(Duration::from_secs(10))
            .await;
        for thread_id in report.submit_failed {
            warn!("failed to submit Shutdown to thread {thread_id}");
        }
        for thread_id in report.timed_out {
            warn!("timed out waiting for thread {thread_id} to shut down");
        }
    }

    async fn request_trace_context(
        &self,
        request_id: &ConnectionRequestId,
    ) -> Option<codex_protocol::protocol::W3cTraceContext> {
        self.outgoing.request_trace_context(request_id).await
    }

    async fn submit_core_op(
        &self,
        request_id: &ConnectionRequestId,
        thread: &CodexThread,
        op: Op,
    ) -> CodexResult<String> {
        thread
            .submit_with_trace(op, self.request_trace_context(request_id).await)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn thread_start_task(
        listener_task_context: ListenerTaskContext,
        cli_overrides: Vec<(String, TomlValue)>,
        runtime_feature_enablement: BTreeMap<String, bool>,
        cloud_requirements: CloudRequirementsLoader,
        request_id: ConnectionRequestId,
        app_server_client_name: Option<String>,
        app_server_client_version: Option<String>,
        config_overrides: Option<HashMap<String, serde_json::Value>>,
        typesafe_overrides: ConfigOverrides,
        dynamic_tools: Option<Vec<ApiDynamicToolSpec>>,
        persist_extended_history: bool,
        service_name: Option<String>,
        experimental_raw_events: bool,
        request_trace: Option<W3cTraceContext>,
    ) {
        let requested_cwd = typesafe_overrides.cwd.clone();
        let mut config = match derive_config_from_params(
            &cli_overrides,
            config_overrides.clone(),
            typesafe_overrides.clone(),
            &cloud_requirements,
            &listener_task_context.codex_home,
            &runtime_feature_enablement,
        )
        .await
        {
            Ok(config) => config,
            Err(err) => {
                let error = config_load_error(&err);
                listener_task_context
                    .outgoing
                    .send_error(request_id, error)
                    .await;
                return;
            }
        };

        // The user may have requested WorkspaceWrite or DangerFullAccess via
        // the command line, though in the process of deriving the Config, it
        // could be downgraded to ReadOnly (perhaps there is no sandbox
        // available on Windows or the enterprise config disallows it). The cwd
        // should still be considered "trusted" in this case.
        let requested_sandbox_trusts_project = matches!(
            typesafe_overrides.sandbox_mode,
            Some(
                codex_protocol::config_types::SandboxMode::WorkspaceWrite
                    | codex_protocol::config_types::SandboxMode::DangerFullAccess
            )
        );

        if requested_cwd.is_some()
            && !config.active_project.is_trusted()
            && (requested_sandbox_trusts_project
                || matches!(
                    config.permissions.sandbox_policy.get(),
                    codex_protocol::protocol::SandboxPolicy::WorkspaceWrite { .. }
                        | codex_protocol::protocol::SandboxPolicy::DangerFullAccess
                        | codex_protocol::protocol::SandboxPolicy::ExternalSandbox { .. }
                ))
        {
            let trust_target = resolve_root_git_project_for_trust(config.cwd.as_path())
                .unwrap_or_else(|| config.cwd.to_path_buf());
            if let Err(err) = codex_core::config::set_project_trust_level(
                &listener_task_context.codex_home,
                trust_target.as_path(),
                TrustLevel::Trusted,
            ) {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to persist trusted project state: {err}"),
                    data: None,
                };
                listener_task_context
                    .outgoing
                    .send_error(request_id, error)
                    .await;
                return;
            }

            config = match derive_config_from_params(
                &cli_overrides,
                config_overrides,
                typesafe_overrides,
                &cloud_requirements,
                &listener_task_context.codex_home,
                &runtime_feature_enablement,
            )
            .await
            {
                Ok(config) => config,
                Err(err) => {
                    let error = config_load_error(&err);
                    listener_task_context
                        .outgoing
                        .send_error(request_id, error)
                        .await;
                    return;
                }
            };
        }

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
                listener_task_context
                    .outgoing
                    .send_error(request_id, error)
                    .await;
                return;
            }
            dynamic_tools
                .into_iter()
                .map(|tool| CoreDynamicToolSpec {
                    name: tool.name,
                    description: tool.description,
                    input_schema: tool.input_schema,
                    defer_loading: tool.defer_loading,
                })
                .collect()
        };
        let core_dynamic_tool_count = core_dynamic_tools.len();

        match listener_task_context
            .thread_manager
            .start_thread_with_tools_and_service_name(
                config,
                core_dynamic_tools,
                persist_extended_history,
                service_name,
                request_trace,
            )
            .instrument(tracing::info_span!(
                "app_server.thread_start.create_thread",
                otel.name = "app_server.thread_start.create_thread",
                thread_start.dynamic_tool_count = core_dynamic_tool_count,
                thread_start.persist_extended_history = persist_extended_history,
            ))
            .await
        {
            Ok(new_conv) => {
                let NewThread {
                    thread_id,
                    thread,
                    session_configured,
                    ..
                } = new_conv;
                if let Err(error) = Self::set_app_server_client_info(
                    thread.as_ref(),
                    app_server_client_name,
                    app_server_client_version,
                )
                .await
                {
                    listener_task_context
                        .outgoing
                        .send_error(request_id, error)
                        .await;
                    return;
                }
                let config_snapshot = thread
                    .config_snapshot()
                    .instrument(tracing::info_span!(
                        "app_server.thread_start.config_snapshot",
                        otel.name = "app_server.thread_start.config_snapshot",
                    ))
                    .await;
                let mut thread = build_thread_from_snapshot(
                    thread_id,
                    &config_snapshot,
                    session_configured.rollout_path.clone(),
                );

                // Auto-attach a thread listener when starting a thread.
                Self::log_listener_attach_result(
                    Self::ensure_conversation_listener_task(
                        listener_task_context.clone(),
                        thread_id,
                        request_id.connection_id,
                        experimental_raw_events,
                        ApiVersion::V2,
                    )
                    .instrument(tracing::info_span!(
                        "app_server.thread_start.attach_listener",
                        otel.name = "app_server.thread_start.attach_listener",
                        thread_start.experimental_raw_events = experimental_raw_events,
                    ))
                    .await,
                    thread_id,
                    request_id.connection_id,
                    "thread",
                );

                listener_task_context
                    .thread_watch_manager
                    .upsert_thread_silently(thread.clone())
                    .instrument(tracing::info_span!(
                        "app_server.thread_start.upsert_thread",
                        otel.name = "app_server.thread_start.upsert_thread",
                    ))
                    .await;

                thread.status = resolve_thread_status(
                    listener_task_context
                        .thread_watch_manager
                        .loaded_status_for_thread(&thread.id)
                        .instrument(tracing::info_span!(
                            "app_server.thread_start.resolve_status",
                            otel.name = "app_server.thread_start.resolve_status",
                        ))
                        .await,
                    /*has_in_progress_turn*/ false,
                );

                let response = ThreadStartResponse {
                    thread: thread.clone(),
                    model: config_snapshot.model,
                    model_provider: config_snapshot.model_provider_id,
                    service_tier: config_snapshot.service_tier,
                    cwd: config_snapshot.cwd.to_path_buf(),
                    approval_policy: config_snapshot.approval_policy.into(),
                    approvals_reviewer: config_snapshot.approvals_reviewer.into(),
                    sandbox: config_snapshot.sandbox_policy.into(),
                    reasoning_effort: config_snapshot.reasoning_effort,
                };
                if listener_task_context.general_analytics_enabled {
                    listener_task_context
                        .analytics_events_client
                        .track_response(
                            request_id.connection_id.0,
                            ClientResponse::ThreadStart {
                                request_id: request_id.request_id.clone(),
                                response: response.clone(),
                            },
                        );
                }

                listener_task_context
                    .outgoing
                    .send_response(request_id, response)
                    .instrument(tracing::info_span!(
                        "app_server.thread_start.send_response",
                        otel.name = "app_server.thread_start.send_response",
                    ))
                    .await;

                let notif = ThreadStartedNotification { thread };
                listener_task_context
                    .outgoing
                    .send_server_notification(ServerNotification::ThreadStarted(notif))
                    .instrument(tracing::info_span!(
                        "app_server.thread_start.notify_started",
                        otel.name = "app_server.thread_start.notify_started",
                    ))
                    .await;
            }
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("error creating thread: {err}"),
                    data: None,
                };
                listener_task_context
                    .outgoing
                    .send_error(request_id, error)
                    .await;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_thread_config_overrides(
        &self,
        model: Option<String>,
        model_provider: Option<String>,
        service_tier: Option<Option<codex_protocol::config_types::ServiceTier>>,
        cwd: Option<String>,
        approval_policy: Option<codex_app_server_protocol::AskForApproval>,
        approvals_reviewer: Option<codex_app_server_protocol::ApprovalsReviewer>,
        sandbox: Option<SandboxMode>,
        base_instructions: Option<Option<String>>,
        developer_instructions: Option<Option<String>>,
        personality: Option<Personality>,
    ) -> Result<ConfigOverrides, JSONRPCErrorError> {
        let cwd = cwd
            .map(|cwd| AbsolutePathBuf::relative_to_current_dir(&cwd))
            .transpose()
            .map_err(|err| JSONRPCErrorError {
                code: INVALID_PARAMS_ERROR_CODE,
                message: format!("invalid thread cwd: {err}"),
                data: None,
            })?;
        Ok(ConfigOverrides {
            model,
            model_provider,
            service_tier,
            cwd,
            approval_policy: approval_policy
                .map(codex_app_server_protocol::AskForApproval::to_core),
            approvals_reviewer: approvals_reviewer
                .map(codex_app_server_protocol::ApprovalsReviewer::to_core),
            sandbox_mode: sandbox.map(SandboxMode::to_core),
            codex_linux_sandbox_exe: self.arg0_paths.codex_linux_sandbox_exe.clone(),
            main_execve_wrapper_exe: self.arg0_paths.main_execve_wrapper_exe.clone(),
            base_instructions,
            developer_instructions,
            personality,
            ..Default::default()
        })
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

    async fn thread_increment_elicitation(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadIncrementElicitationParams,
    ) {
        let (_, thread) = match self.load_thread(&params.thread_id).await {
            Ok(value) => value,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        match thread.increment_out_of_band_elicitation_count().await {
            Ok(count) => {
                self.outgoing
                    .send_response(
                        request_id,
                        ThreadIncrementElicitationResponse {
                            count,
                            paused: count > 0,
                        },
                    )
                    .await;
            }
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!("failed to increment out-of-band elicitation counter: {err}"),
                )
                .await;
            }
        }
    }

    async fn thread_decrement_elicitation(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadDecrementElicitationParams,
    ) {
        let (_, thread) = match self.load_thread(&params.thread_id).await {
            Ok(value) => value,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        match thread.decrement_out_of_band_elicitation_count().await {
            Ok(count) => {
                self.outgoing
                    .send_response(
                        request_id,
                        ThreadDecrementElicitationResponse {
                            count,
                            paused: count > 0,
                        },
                    )
                    .await;
            }
            Err(CodexErr::InvalidRequest(message)) => {
                self.send_invalid_request_error(request_id, message).await;
            }
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!("failed to decrement out-of-band elicitation counter: {err}"),
                )
                .await;
            }
        }
    }

    async fn thread_set_name(&self, request_id: ConnectionRequestId, params: ThreadSetNameParams) {
        let ThreadSetNameParams { thread_id, name } = params;
        let thread_id = match ThreadId::from_string(&thread_id) {
            Ok(id) => id,
            Err(err) => {
                self.send_invalid_request_error(request_id, format!("invalid thread id: {err}"))
                    .await;
                return;
            }
        };
        let Some(name) = codex_core::util::normalize_thread_name(&name) else {
            self.send_invalid_request_error(
                request_id,
                "thread name must not be empty".to_string(),
            )
            .await;
            return;
        };

        if let Ok(thread) = self.thread_manager.get_thread(thread_id).await {
            if let Err(err) = self
                .submit_core_op(&request_id, thread.as_ref(), Op::SetThreadName { name })
                .await
            {
                self.send_internal_error(request_id, format!("failed to set thread name: {err}"))
                    .await;
                return;
            }

            self.outgoing
                .send_response(request_id, ThreadSetNameResponse {})
                .await;
            return;
        }

        let thread_exists =
            match find_thread_path_by_id_str(&self.config.codex_home, &thread_id.to_string()).await
            {
                Ok(Some(_)) => true,
                Ok(None) => false,
                Err(err) => {
                    self.send_invalid_request_error(
                        request_id,
                        format!("failed to locate thread id {thread_id}: {err}"),
                    )
                    .await;
                    return;
                }
            };

        if !thread_exists {
            self.send_invalid_request_error(request_id, format!("thread not found: {thread_id}"))
                .await;
            return;
        }

        if let Err(err) =
            codex_core::append_thread_name(&self.config.codex_home, thread_id, &name).await
        {
            self.send_internal_error(request_id, format!("failed to set thread name: {err}"))
                .await;
            return;
        }

        self.outgoing
            .send_response(request_id, ThreadSetNameResponse {})
            .await;
        let notification = ThreadNameUpdatedNotification {
            thread_id: thread_id.to_string(),
            thread_name: Some(name),
        };
        self.outgoing
            .send_server_notification(ServerNotification::ThreadNameUpdated(notification))
            .await;
    }

    async fn thread_metadata_update(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadMetadataUpdateParams,
    ) {
        let ThreadMetadataUpdateParams {
            thread_id,
            git_info,
        } = params;

        let thread_uuid = match ThreadId::from_string(&thread_id) {
            Ok(id) => id,
            Err(err) => {
                self.send_invalid_request_error(request_id, format!("invalid thread id: {err}"))
                    .await;
                return;
            }
        };

        let Some(ThreadMetadataGitInfoUpdateParams {
            sha,
            branch,
            origin_url,
        }) = git_info
        else {
            self.send_invalid_request_error(
                request_id,
                "gitInfo must include at least one field".to_string(),
            )
            .await;
            return;
        };

        if sha.is_none() && branch.is_none() && origin_url.is_none() {
            self.send_invalid_request_error(
                request_id,
                "gitInfo must include at least one field".to_string(),
            )
            .await;
            return;
        }

        let loaded_thread = self.thread_manager.get_thread(thread_uuid).await.ok();
        let mut state_db_ctx = loaded_thread.as_ref().and_then(|thread| thread.state_db());
        if state_db_ctx.is_none() {
            state_db_ctx = get_state_db(&self.config).await;
        }
        let Some(state_db_ctx) = state_db_ctx else {
            self.send_internal_error(
                request_id,
                format!("sqlite state db unavailable for thread {thread_uuid}"),
            )
            .await;
            return;
        };

        if let Err(error) = self
            .ensure_thread_metadata_row_exists(thread_uuid, &state_db_ctx, loaded_thread.as_ref())
            .await
        {
            self.outgoing.send_error(request_id, error).await;
            return;
        }

        let git_sha = match sha {
            Some(Some(sha)) => {
                let sha = sha.trim().to_string();
                if sha.is_empty() {
                    self.send_invalid_request_error(
                        request_id,
                        "gitInfo.sha must not be empty".to_string(),
                    )
                    .await;
                    return;
                }
                Some(Some(sha))
            }
            Some(None) => Some(None),
            None => None,
        };
        let git_branch = match branch {
            Some(Some(branch)) => {
                let branch = branch.trim().to_string();
                if branch.is_empty() {
                    self.send_invalid_request_error(
                        request_id,
                        "gitInfo.branch must not be empty".to_string(),
                    )
                    .await;
                    return;
                }
                Some(Some(branch))
            }
            Some(None) => Some(None),
            None => None,
        };
        let git_origin_url = match origin_url {
            Some(Some(origin_url)) => {
                let origin_url = origin_url.trim().to_string();
                if origin_url.is_empty() {
                    self.send_invalid_request_error(
                        request_id,
                        "gitInfo.originUrl must not be empty".to_string(),
                    )
                    .await;
                    return;
                }
                Some(Some(origin_url))
            }
            Some(None) => Some(None),
            None => None,
        };

        let updated = match state_db_ctx
            .update_thread_git_info(
                thread_uuid,
                git_sha.as_ref().map(|value| value.as_deref()),
                git_branch.as_ref().map(|value| value.as_deref()),
                git_origin_url.as_ref().map(|value| value.as_deref()),
            )
            .await
        {
            Ok(updated) => updated,
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!("failed to update thread metadata for {thread_uuid}: {err}"),
                )
                .await;
                return;
            }
        };
        if !updated {
            self.send_internal_error(
                request_id,
                format!("thread metadata disappeared before update completed: {thread_uuid}"),
            )
            .await;
            return;
        }

        let Some(summary) =
            read_summary_from_state_db_context_by_thread_id(Some(&state_db_ctx), thread_uuid).await
        else {
            self.send_internal_error(
                request_id,
                format!("failed to reload updated thread metadata for {thread_uuid}"),
            )
            .await;
            return;
        };

        let mut thread = summary_to_thread(summary);
        self.attach_thread_name(thread_uuid, &mut thread).await;
        thread.status = resolve_thread_status(
            self.thread_watch_manager
                .loaded_status_for_thread(&thread.id)
                .await,
            /*has_in_progress_turn*/ false,
        );

        self.outgoing
            .send_response(request_id, ThreadMetadataUpdateResponse { thread })
            .await;
    }

    async fn ensure_thread_metadata_row_exists(
        &self,
        thread_uuid: ThreadId,
        state_db_ctx: &Arc<StateRuntime>,
        loaded_thread: Option<&Arc<CodexThread>>,
    ) -> Result<(), JSONRPCErrorError> {
        fn invalid_request(message: String) -> JSONRPCErrorError {
            JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message,
                data: None,
            }
        }

        fn internal_error(message: String) -> JSONRPCErrorError {
            JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message,
                data: None,
            }
        }

        match state_db_ctx.get_thread(thread_uuid).await {
            Ok(Some(_)) => return Ok(()),
            Ok(None) => {}
            Err(err) => {
                return Err(internal_error(format!(
                    "failed to load thread metadata for {thread_uuid}: {err}"
                )));
            }
        }

        if let Some(thread) = loaded_thread {
            let Some(rollout_path) = thread.rollout_path() else {
                return Err(invalid_request(format!(
                    "ephemeral thread does not support metadata updates: {thread_uuid}"
                )));
            };

            reconcile_rollout(
                Some(state_db_ctx),
                rollout_path.as_path(),
                self.config.model_provider_id.as_str(),
                /*builder*/ None,
                &[],
                /*archived_only*/ None,
                /*new_thread_memory_mode*/ None,
            )
            .await;

            match state_db_ctx.get_thread(thread_uuid).await {
                Ok(Some(_)) => return Ok(()),
                Ok(None) => {}
                Err(err) => {
                    return Err(internal_error(format!(
                        "failed to load reconciled thread metadata for {thread_uuid}: {err}"
                    )));
                }
            }

            let config_snapshot = thread.config_snapshot().await;
            let model_provider = config_snapshot.model_provider_id.clone();
            let mut builder = ThreadMetadataBuilder::new(
                thread_uuid,
                rollout_path,
                Utc::now(),
                config_snapshot.session_source.clone(),
            );
            builder.model_provider = Some(model_provider.clone());
            builder.cwd = config_snapshot.cwd.to_path_buf();
            builder.cli_version = Some(env!("CARGO_PKG_VERSION").to_string());
            builder.sandbox_policy = config_snapshot.sandbox_policy.clone();
            builder.approval_mode = config_snapshot.approval_policy;
            let metadata = builder.build(model_provider.as_str());
            if let Err(err) = state_db_ctx.insert_thread_if_absent(&metadata).await {
                return Err(internal_error(format!(
                    "failed to create thread metadata for {thread_uuid}: {err}"
                )));
            }
            return Ok(());
        }

        let rollout_path =
            match find_thread_path_by_id_str(&self.config.codex_home, &thread_uuid.to_string())
                .await
            {
                Ok(Some(path)) => path,
                Ok(None) => match find_archived_thread_path_by_id_str(
                    &self.config.codex_home,
                    &thread_uuid.to_string(),
                )
                .await
                {
                    Ok(Some(path)) => path,
                    Ok(None) => {
                        return Err(invalid_request(format!("thread not found: {thread_uuid}")));
                    }
                    Err(err) => {
                        return Err(internal_error(format!(
                            "failed to locate archived thread id {thread_uuid}: {err}"
                        )));
                    }
                },
                Err(err) => {
                    return Err(internal_error(format!(
                        "failed to locate thread id {thread_uuid}: {err}"
                    )));
                }
            };

        reconcile_rollout(
            Some(state_db_ctx),
            rollout_path.as_path(),
            self.config.model_provider_id.as_str(),
            /*builder*/ None,
            &[],
            /*archived_only*/ None,
            /*new_thread_memory_mode*/ None,
        )
        .await;

        match state_db_ctx.get_thread(thread_uuid).await {
            Ok(Some(_)) => Ok(()),
            Ok(None) => Err(internal_error(format!(
                "failed to create thread metadata from rollout for {thread_uuid}"
            ))),
            Err(err) => Err(internal_error(format!(
                "failed to load reconciled thread metadata for {thread_uuid}: {err}"
            ))),
        }
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
        let state_db_ctx = get_state_db(&self.config).await;
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
                thread.status = resolve_thread_status(
                    self.thread_watch_manager
                        .loaded_status_for_thread(&thread.id)
                        .await,
                    /*has_in_progress_turn*/ false,
                );
                self.attach_thread_name(thread_id, &mut thread).await;
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
            let thread_state = self.thread_state_manager.thread_state(thread_id).await;
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

        if let Err(err) = self
            .submit_core_op(
                &request_id,
                thread.as_ref(),
                Op::ThreadRollback { num_turns },
            )
            .await
        {
            // No ThreadRollback event will arrive if an error occurs.
            // Clean up and reply immediately.
            let thread_state = self.thread_state_manager.thread_state(thread_id).await;
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

        match self
            .submit_core_op(&request_id, thread.as_ref(), Op::Compact)
            .await
        {
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

        match self
            .submit_core_op(&request_id, thread.as_ref(), Op::CleanBackgroundTerminals)
            .await
        {
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

    async fn thread_shell_command(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadShellCommandParams,
    ) {
        let ThreadShellCommandParams { thread_id, command } = params;
        let command = command.trim().to_string();
        if command.is_empty() {
            self.outgoing
                .send_error(
                    request_id,
                    JSONRPCErrorError {
                        code: INVALID_REQUEST_ERROR_CODE,
                        message: "command must not be empty".to_string(),
                        data: None,
                    },
                )
                .await;
            return;
        }

        let (_, thread) = match self.load_thread(&thread_id).await {
            Ok(v) => v,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        match self
            .submit_core_op(
                &request_id,
                thread.as_ref(),
                Op::RunUserShellCommand { command },
            )
            .await
        {
            Ok(_) => {
                self.outgoing
                    .send_response(request_id, ThreadShellCommandResponse {})
                    .await;
            }
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!("failed to start shell command: {err}"),
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
            search_term,
        } = params;
        let cwd = match normalize_thread_list_cwd_filter(cwd) {
            Ok(cwd) => cwd,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

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
                    cwd,
                    search_term,
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
        let mut threads = Vec::with_capacity(summaries.len());
        let mut thread_ids = HashSet::with_capacity(summaries.len());
        let mut status_ids = Vec::with_capacity(summaries.len());

        for summary in summaries {
            let conversation_id = summary.conversation_id;
            thread_ids.insert(conversation_id);

            let thread = summary_to_thread(summary);
            status_ids.push(thread.id.clone());
            threads.push((conversation_id, thread));
        }

        let names = match find_thread_names_by_ids(&self.config.codex_home, &thread_ids).await {
            Ok(names) => names,
            Err(err) => {
                warn!("Failed to read thread names: {err}");
                HashMap::new()
            }
        };

        let statuses = self
            .thread_watch_manager
            .loaded_statuses_for_threads(status_ids)
            .await;

        let data = threads
            .into_iter()
            .map(|(conversation_id, mut thread)| {
                thread.name = names.get(&conversation_id).cloned();
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

        let loaded_thread = self.thread_manager.get_thread(thread_uuid).await.ok();
        let loaded_thread_state_db = loaded_thread.as_ref().and_then(|thread| thread.state_db());
        let db_summary = if let Some(state_db_ctx) = loaded_thread_state_db.as_ref() {
            read_summary_from_state_db_context_by_thread_id(Some(state_db_ctx), thread_uuid).await
        } else {
            read_summary_from_state_db_by_thread_id(&self.config, thread_uuid).await
        };
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
            let Some(thread) = loaded_thread.as_ref() else {
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
        if thread.forked_from_id.is_none()
            && let Some(rollout_path) = rollout_path.as_ref()
        {
            thread.forked_from_id = forked_from_id_from_rollout(rollout_path).await;
        }
        self.attach_thread_name(thread_uuid, &mut thread).await;

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

        let has_live_in_progress_turn = if let Some(loaded_thread) = loaded_thread.as_ref() {
            matches!(loaded_thread.agent_status().await, AgentStatus::Running)
        } else {
            false
        };

        let thread_status = self
            .thread_watch_manager
            .loaded_status_for_thread(&thread.id)
            .await;

        set_thread_status_and_interrupt_stale_turns(
            &mut thread,
            thread_status,
            has_live_in_progress_turn,
        );
        let response = ThreadReadResponse { thread };
        self.outgoing.send_response(request_id, response).await;
    }

    pub(crate) fn thread_created_receiver(&self) -> broadcast::Receiver<ThreadId> {
        self.thread_manager.subscribe_thread_created()
    }

    pub(crate) async fn connection_initialized(&self, connection_id: ConnectionId) {
        self.thread_state_manager
            .connection_initialized(connection_id)
            .await;
    }

    pub(crate) async fn connection_closed(&mut self, connection_id: ConnectionId) {
        self.command_exec_manager
            .connection_closed(connection_id)
            .await;
        self.thread_state_manager
            .remove_connection(connection_id)
            .await;
    }

    pub(crate) fn subscribe_running_assistant_turn_count(&self) -> watch::Receiver<usize> {
        self.thread_watch_manager.subscribe_running_turn_count()
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
            Self::log_listener_attach_result(
                self.ensure_conversation_listener(
                    thread_id,
                    connection_id,
                    /*raw_events_enabled*/ false,
                    ApiVersion::V2,
                )
                .await,
                thread_id,
                connection_id,
                "thread",
            );
        }
    }

    async fn thread_resume(&mut self, request_id: ConnectionRequestId, params: ThreadResumeParams) {
        if let Ok(thread_id) = ThreadId::from_string(&params.thread_id)
            && self
                .pending_thread_unloads
                .lock()
                .await
                .contains(&thread_id)
        {
            self.send_invalid_request_error(
                request_id,
                format!(
                    "thread {thread_id} is closing; retry thread/resume after the thread is closed"
                ),
            )
            .await;
            return;
        }

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
            service_tier,
            cwd,
            approval_policy,
            approvals_reviewer,
            sandbox,
            config: mut request_overrides,
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
        let mut typesafe_overrides = match self.build_thread_config_overrides(
            model,
            model_provider,
            service_tier,
            cwd,
            approval_policy,
            approvals_reviewer,
            sandbox,
            base_instructions,
            developer_instructions,
            personality,
        ) {
            Ok(overrides) => overrides,
            Err(error) => {
                self.outgoing.send_error(request_id.clone(), error).await;
                return;
            }
        };
        let persisted_resume_metadata = self
            .load_and_apply_persisted_resume_metadata(
                &thread_history,
                &mut request_overrides,
                &mut typesafe_overrides,
            )
            .await;

        // Derive a Config using the same logic as new conversation, honoring overrides if provided.
        let cloud_requirements = self.current_cloud_requirements();
        let cli_overrides = self.current_cli_overrides();
        let runtime_feature_enablement = self.current_runtime_feature_enablement();
        let config = match derive_config_for_cwd(
            &cli_overrides,
            request_overrides,
            typesafe_overrides,
            history_cwd,
            &cloud_requirements,
            &self.config.codex_home,
            &runtime_feature_enablement,
        )
        .await
        {
            Ok(config) => config,
            Err(err) => {
                let error = config_load_error(&err);
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let fallback_model_provider = config.model_provider_id.clone();
        let response_history = thread_history.clone();

        match self
            .thread_manager
            .resume_thread_with_history(
                config,
                thread_history,
                self.auth_manager.clone(),
                persist_extended_history,
                self.request_trace_context(&request_id).await,
            )
            .await
        {
            Ok(NewThread {
                thread_id,
                thread,
                session_configured,
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
                Self::log_listener_attach_result(
                    self.ensure_conversation_listener(
                        thread_id,
                        request_id.connection_id,
                        /*raw_events_enabled*/ false,
                        ApiVersion::V2,
                    )
                    .await,
                    thread_id,
                    request_id.connection_id,
                    "thread",
                );

                let mut thread = match self
                    .load_thread_from_resume_source_or_send_internal(
                        thread_id,
                        thread.as_ref(),
                        &response_history,
                        rollout_path.as_path(),
                        fallback_model_provider.as_str(),
                        persisted_resume_metadata.as_ref(),
                    )
                    .await
                {
                    Ok(thread) => thread,
                    Err(message) => {
                        self.send_internal_error(request_id, message).await;
                        return;
                    }
                };

                self.thread_watch_manager
                    .upsert_thread(thread.clone())
                    .await;

                let thread_status = self
                    .thread_watch_manager
                    .loaded_status_for_thread(&thread.id)
                    .await;

                set_thread_status_and_interrupt_stale_turns(
                    &mut thread,
                    thread_status,
                    /*has_live_in_progress_turn*/ false,
                );

                let response = ThreadResumeResponse {
                    thread,
                    model: session_configured.model,
                    model_provider: session_configured.model_provider_id,
                    service_tier: session_configured.service_tier,
                    cwd: session_configured.cwd,
                    approval_policy: session_configured.approval_policy.into(),
                    approvals_reviewer: session_configured.approvals_reviewer.into(),
                    sandbox: session_configured.sandbox_policy.into(),
                    reasoning_effort: session_configured.reasoning_effort,
                };
                if self.config.features.enabled(Feature::GeneralAnalytics) {
                    self.analytics_events_client.track_response(
                        request_id.connection_id.0,
                        ClientResponse::ThreadResume {
                            request_id: request_id.request_id.clone(),
                            response: response.clone(),
                        },
                    );
                }

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

    async fn load_and_apply_persisted_resume_metadata(
        &self,
        thread_history: &InitialHistory,
        request_overrides: &mut Option<HashMap<String, serde_json::Value>>,
        typesafe_overrides: &mut ConfigOverrides,
    ) -> Option<ThreadMetadata> {
        let InitialHistory::Resumed(resumed_history) = thread_history else {
            return None;
        };
        let state_db_ctx = get_state_db(&self.config).await?;
        let persisted_metadata = state_db_ctx
            .get_thread(resumed_history.conversation_id)
            .await
            .ok()
            .flatten()?;
        merge_persisted_resume_metadata(request_overrides, typesafe_overrides, &persisted_metadata);
        Some(persisted_metadata)
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

            let thread_state = self
                .thread_state_manager
                .thread_state(existing_thread_id)
                .await;
            self.ensure_listener_task_running(
                existing_thread_id,
                existing_thread.clone(),
                thread_state.clone(),
                ApiVersion::V2,
            )
            .await;

            let config_snapshot = existing_thread.config_snapshot().await;
            let mismatch_details = collect_resume_override_mismatches(params, &config_snapshot);
            if !mismatch_details.is_empty() {
                tracing::warn!(
                    "thread/resume overrides ignored for running thread {}: {}",
                    existing_thread_id,
                    mismatch_details.join("; ")
                );
            }
            let thread_summary = match load_thread_summary_for_rollout(
                &self.config,
                existing_thread_id,
                rollout_path.as_path(),
                config_snapshot.model_provider_id.as_str(),
                /*persisted_metadata*/ None,
            )
            .await
            {
                Ok(thread) => thread,
                Err(message) => {
                    self.send_internal_error(request_id, message).await;
                    return true;
                }
            };

            let listener_command_tx = {
                let thread_state = thread_state.lock().await;
                thread_state.listener_command_tx()
            };
            let Some(listener_command_tx) = listener_command_tx else {
                let err = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!(
                        "failed to enqueue running thread resume for thread {existing_thread_id}: thread listener is not running"
                    ),
                    data: None,
                };
                self.outgoing.send_error(request_id, err).await;
                return true;
            };

            let command = crate::thread_state::ThreadListenerCommand::SendThreadResumeResponse(
                Box::new(crate::thread_state::PendingThreadResumeRequest {
                    request_id: request_id.clone(),
                    rollout_path: rollout_path.clone(),
                    config_snapshot,
                    thread_summary,
                }),
            );
            if listener_command_tx.send(command).is_err() {
                let err = JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!(
                        "failed to enqueue running thread resume for thread {existing_thread_id}: thread listener command channel is closed"
                    ),
                    data: None,
                };
                self.outgoing.send_error(request_id, err).await;
            }
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

    async fn load_thread_from_resume_source_or_send_internal(
        &self,
        thread_id: ThreadId,
        thread: &CodexThread,
        thread_history: &InitialHistory,
        rollout_path: &Path,
        fallback_provider: &str,
        persisted_resume_metadata: Option<&ThreadMetadata>,
    ) -> std::result::Result<Thread, String> {
        let thread = match thread_history {
            InitialHistory::Resumed(resumed) => {
                load_thread_summary_for_rollout(
                    &self.config,
                    resumed.conversation_id,
                    resumed.rollout_path.as_path(),
                    fallback_provider,
                    persisted_resume_metadata,
                )
                .await
            }
            InitialHistory::Forked(items) => {
                let config_snapshot = thread.config_snapshot().await;
                let mut thread = build_thread_from_snapshot(
                    thread_id,
                    &config_snapshot,
                    Some(rollout_path.into()),
                );
                thread.preview = preview_from_rollout_items(items);
                Ok(thread)
            }
            InitialHistory::New => Err(format!(
                "failed to build resume response for thread {thread_id}: initial history missing"
            )),
        };
        let mut thread = thread?;
        thread.id = thread_id.to_string();
        thread.path = Some(rollout_path.to_path_buf());
        let history_items = thread_history.get_rollout_items();
        populate_thread_turns(
            &mut thread,
            ThreadTurnSource::HistoryItems(&history_items),
            /*active_turn*/ None,
        )
        .await?;
        self.attach_thread_name(thread_id, &mut thread).await;
        Ok(thread)
    }

    async fn attach_thread_name(&self, thread_id: ThreadId, thread: &mut Thread) {
        match find_thread_name_by_id(&self.config.codex_home, &thread_id).await {
            Ok(name) => {
                thread.name = name;
            }
            Err(err) => {
                warn!("Failed to read thread name for {thread_id}: {err}");
            }
        }
    }

    async fn thread_fork(&mut self, request_id: ConnectionRequestId, params: ThreadForkParams) {
        let ThreadForkParams {
            thread_id,
            path,
            model,
            model_provider,
            service_tier,
            cwd,
            approval_policy,
            approvals_reviewer,
            sandbox,
            config: cli_overrides,
            base_instructions,
            developer_instructions,
            ephemeral,
            persist_extended_history,
        } = params;

        let (rollout_path, source_thread_id) = if let Some(path) = path {
            (path, None)
        } else {
            let existing_thread_id = match ThreadId::from_string(&thread_id) {
                Ok(id) => id,
                Err(err) => {
                    self.send_invalid_request_error(
                        request_id,
                        format!("invalid thread id: {err}"),
                    )
                    .await;
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
        let mut typesafe_overrides = match self.build_thread_config_overrides(
            model,
            model_provider,
            service_tier,
            cwd,
            approval_policy,
            approvals_reviewer,
            sandbox,
            base_instructions,
            developer_instructions,
            /*personality*/ None,
        ) {
            Ok(overrides) => overrides,
            Err(error) => {
                self.outgoing.send_error(request_id.clone(), error).await;
                return;
            }
        };
        if typesafe_overrides.base_instructions.is_none()
            && let Ok(history) = RolloutRecorder::get_rollout_history(&rollout_path).await
            && let Some(base_instructions) = history.get_base_instructions()
        {
            typesafe_overrides.base_instructions =
                Some(base_instructions.map(|base_instructions| base_instructions.text));
        }
        typesafe_overrides.ephemeral = ephemeral.then_some(true);
        // Derive a Config using the same logic as new conversation, honoring overrides if provided.
        let cloud_requirements = self.current_cloud_requirements();
        let cli_overrides = self.current_cli_overrides();
        let runtime_feature_enablement = self.current_runtime_feature_enablement();
        let config = match derive_config_for_cwd(
            &cli_overrides,
            request_overrides,
            typesafe_overrides,
            history_cwd,
            &cloud_requirements,
            &self.config.codex_home,
            &runtime_feature_enablement,
        )
        .await
        {
            Ok(config) => config,
            Err(err) => {
                self.outgoing
                    .send_error(request_id, config_load_error(&err))
                    .await;
                return;
            }
        };

        let fallback_model_provider = config.model_provider_id.clone();

        let NewThread {
            thread_id,
            thread: forked_thread,
            session_configured,
            ..
        } = match self
            .thread_manager
            .fork_thread(
                ForkSnapshot::Interrupted,
                config,
                rollout_path.clone(),
                persist_extended_history,
                self.request_trace_context(&request_id).await,
            )
            .await
        {
            Ok(thread) => thread,
            Err(err) => {
                match err {
                    CodexErr::Io(_) | CodexErr::Json(_) => {
                        self.send_invalid_request_error(
                            request_id,
                            format!("failed to load rollout `{}`: {err}", rollout_path.display()),
                        )
                        .await;
                    }
                    CodexErr::InvalidRequest(message) => {
                        self.send_invalid_request_error(request_id, message).await;
                    }
                    _ => {
                        self.send_internal_error(
                            request_id,
                            format!("error forking thread: {err}"),
                        )
                        .await;
                    }
                }
                return;
            }
        };

        // Auto-attach a conversation listener when forking a thread.
        Self::log_listener_attach_result(
            self.ensure_conversation_listener(
                thread_id,
                request_id.connection_id,
                /*raw_events_enabled*/ false,
                ApiVersion::V2,
            )
            .await,
            thread_id,
            request_id.connection_id,
            "thread",
        );

        // Persistent forks materialize their own rollout immediately. Ephemeral forks stay
        // pathless, so they rebuild their visible history from the copied source rollout instead.
        let mut thread = if let Some(fork_rollout_path) = session_configured.rollout_path.as_ref() {
            match read_summary_from_rollout(
                fork_rollout_path.as_path(),
                fallback_model_provider.as_str(),
            )
            .await
            {
                Ok(summary) => {
                    let mut thread = summary_to_thread(summary);
                    thread.forked_from_id =
                        forked_from_id_from_rollout(fork_rollout_path.as_path()).await;
                    thread
                }
                Err(err) => {
                    self.send_internal_error(
                        request_id,
                        format!(
                            "failed to load rollout `{}` for thread {thread_id}: {err}",
                            fork_rollout_path.display()
                        ),
                    )
                    .await;
                    return;
                }
            }
        } else {
            let config_snapshot = forked_thread.config_snapshot().await;
            // forked thread names do not inherit the source thread name
            let mut thread =
                build_thread_from_snapshot(thread_id, &config_snapshot, /*path*/ None);
            let history_items = match read_rollout_items_from_rollout(rollout_path.as_path()).await
            {
                Ok(items) => items,
                Err(err) => {
                    self.send_internal_error(
                        request_id,
                        format!(
                            "failed to load source rollout `{}` for thread {thread_id}: {err}",
                            rollout_path.display()
                        ),
                    )
                    .await;
                    return;
                }
            };
            thread.preview = preview_from_rollout_items(&history_items);
            thread.forked_from_id = source_thread_id
                .or_else(|| {
                    history_items.iter().find_map(|item| match item {
                        RolloutItem::SessionMeta(meta_line) => Some(meta_line.meta.id),
                        _ => None,
                    })
                })
                .map(|id| id.to_string());
            if let Err(message) = populate_thread_turns(
                &mut thread,
                ThreadTurnSource::HistoryItems(&history_items),
                /*active_turn*/ None,
            )
            .await
            {
                self.send_internal_error(request_id, message).await;
                return;
            }
            thread
        };

        if let Some(fork_rollout_path) = session_configured.rollout_path.as_ref()
            && let Err(message) = populate_thread_turns(
                &mut thread,
                ThreadTurnSource::RolloutPath(fork_rollout_path.as_path()),
                /*active_turn*/ None,
            )
            .await
        {
            self.send_internal_error(request_id, message).await;
            return;
        }

        self.thread_watch_manager
            .upsert_thread_silently(thread.clone())
            .await;

        thread.status = resolve_thread_status(
            self.thread_watch_manager
                .loaded_status_for_thread(&thread.id)
                .await,
            /*has_in_progress_turn*/ false,
        );

        let response = ThreadForkResponse {
            thread: thread.clone(),
            model: session_configured.model,
            model_provider: session_configured.model_provider_id,
            service_tier: session_configured.service_tier,
            cwd: session_configured.cwd,
            approval_policy: session_configured.approval_policy.into(),
            approvals_reviewer: session_configured.approvals_reviewer.into(),
            sandbox: session_configured.sandbox_policy.into(),
            reasoning_effort: session_configured.reasoning_effort,
        };
        if self.config.features.enabled(Feature::GeneralAnalytics) {
            self.analytics_events_client.track_response(
                request_id.connection_id.0,
                ClientResponse::ThreadFork {
                    request_id: request_id.request_id.clone(),
                    response: response.clone(),
                },
            );
        }

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
            search_term,
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
        let state_db_ctx = get_state_db(&self.config).await;

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
                    search_term.as_deref(),
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
                    search_term.as_deref(),
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
        let items = thread_manager
            .list_collaboration_modes()
            .into_iter()
            .map(Into::into)
            .collect();
        let response = CollaborationModeListResponse { data: items };
        outgoing.send_response(request_id, response).await;
    }

    async fn experimental_feature_list(
        &self,
        request_id: ConnectionRequestId,
        params: ExperimentalFeatureListParams,
    ) {
        let ExperimentalFeatureListParams { cursor, limit } = params;
        let config = match self.load_latest_config(/*fallback_cwd*/ None).await {
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
        let config = match self.load_latest_config(/*fallback_cwd*/ None).await {
            Ok(config) => config,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        if let Err(error) = self.queue_mcp_server_refresh_for_config(&config).await {
            self.outgoing.send_error(request_id, error).await;
            return;
        }

        let response = McpServerRefreshResponse {};
        self.outgoing.send_response(request_id, response).await;
    }

    async fn queue_mcp_server_refresh_for_config(
        &self,
        config: &Config,
    ) -> Result<(), JSONRPCErrorError> {
        let configured_servers = self.thread_manager.mcp_manager().configured_servers(config);
        let mcp_servers = match serde_json::to_value(configured_servers) {
            Ok(value) => value,
            Err(err) => {
                return Err(JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message: format!("failed to serialize MCP servers: {err}"),
                    data: None,
                });
            }
        };

        let mcp_oauth_credentials_store_mode =
            match serde_json::to_value(config.mcp_oauth_credentials_store_mode) {
                Ok(value) => value,
                Err(err) => {
                    return Err(JSONRPCErrorError {
                        code: INTERNAL_ERROR_CODE,
                        message: format!(
                            "failed to serialize MCP OAuth credentials store mode: {err}"
                        ),
                        data: None,
                    });
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
        Ok(())
    }

    async fn mcp_server_oauth_login(
        &self,
        request_id: ConnectionRequestId,
        params: McpServerOauthLoginParams,
    ) {
        let config = match self.load_latest_config(/*fallback_cwd*/ None).await {
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

        let configured_servers = self
            .thread_manager
            .mcp_manager()
            .configured_servers(&config);
        let Some(server) = configured_servers.get(&name) else {
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

        let discovered_scopes = if scopes.is_none() && server.scopes.is_none() {
            discover_supported_scopes(&server.transport).await
        } else {
            None
        };
        let resolved_scopes =
            resolve_oauth_scopes(scopes, server.scopes.clone(), discovered_scopes);

        match perform_oauth_login_return_url(
            &name,
            &url,
            config.mcp_oauth_credentials_store_mode,
            http_headers,
            env_http_headers,
            &resolved_scopes.scopes,
            server.oauth_resource.as_deref(),
            timeout_secs,
            config.mcp_oauth_callback_port,
            config.mcp_oauth_callback_url.as_deref(),
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
        let config = match self.load_latest_config(/*fallback_cwd*/ None).await {
            Ok(config) => config,
            Err(error) => {
                self.outgoing.send_error(request, error).await;
                return;
            }
        };
        let mcp_config = config.to_mcp_config(self.thread_manager.plugins_manager().as_ref());
        let auth = self.auth_manager.auth().await;

        tokio::spawn(async move {
            Self::list_mcp_server_status_task(outgoing, request, params, config, mcp_config, auth)
                .await;
        });
    }

    async fn list_mcp_server_status_task(
        outgoing: Arc<OutgoingMessageSender>,
        request_id: ConnectionRequestId,
        params: ListMcpServerStatusParams,
        config: Config,
        mcp_config: codex_mcp::mcp::McpConfig,
        auth: Option<CodexAuth>,
    ) {
        let detail = match params.detail.unwrap_or(McpServerStatusDetail::Full) {
            McpServerStatusDetail::Full => McpSnapshotDetail::Full,
            McpServerStatusDetail::ToolsAndAuthOnly => McpSnapshotDetail::ToolsAndAuthOnly,
        };

        let snapshot = collect_mcp_snapshot_with_detail(
            &mcp_config,
            auth.as_ref(),
            request_id.request_id.to_string(),
            detail,
        )
        .await;

        // Rebuild the tool list per original server name instead of using
        // `group_tools_by_server()`: qualified tool names are sanitized for the
        // Responses API, so a config key like `some-server` is encoded as the
        // `mcp__some_server__` prefix. Matching with the original server name's
        // sanitized prefix preserves `/mcp` output for hyphenated names.
        let effective_servers = effective_mcp_servers(&mcp_config, auth.as_ref());
        let mut sanitized_prefix_counts = HashMap::<String, usize>::new();
        for name in effective_servers.keys() {
            let prefix = qualified_mcp_tool_name_prefix(name);
            *sanitized_prefix_counts.entry(prefix).or_default() += 1;
        }
        let tools_by_server = effective_servers
            .keys()
            .map(|name| {
                let prefix = qualified_mcp_tool_name_prefix(name);
                // If multiple server names normalize to the same prefix, the
                // qualified tool namespace is ambiguous (for example
                // `some-server` and `some_server` both become
                // `mcp__some_server__`). In that case, avoid attributing the
                // same tools to multiple servers.
                let tools = if sanitized_prefix_counts
                    .get(&prefix)
                    .copied()
                    .unwrap_or_default()
                    == 1
                {
                    snapshot
                        .tools
                        .iter()
                        .filter_map(|(qualified_name, tool)| {
                            qualified_name
                                .strip_prefix(&prefix)
                                .map(|tool_name| (tool_name.to_string(), tool.clone()))
                        })
                        .collect::<HashMap<_, _>>()
                } else {
                    HashMap::new()
                };
                (name.clone(), tools)
            })
            .collect::<HashMap<_, _>>();

        let mut server_names: Vec<String> = config
            .mcp_servers
            .keys()
            .cloned()
            // Include built-in/plugin MCP servers that are present in the
            // effective runtime config even when they are not user-declared in
            // `config.mcp_servers`.
            .chain(effective_servers.keys().cloned())
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

    async fn read_mcp_resource(
        &self,
        request_id: ConnectionRequestId,
        params: McpResourceReadParams,
    ) {
        let outgoing = Arc::clone(&self.outgoing);
        let (_, thread) = match self.load_thread(&params.thread_id).await {
            Ok(thread) => thread,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        tokio::spawn(async move {
            let result = thread.read_mcp_resource(&params.server, &params.uri).await;
            match result {
                Ok(result) => match serde_json::from_value::<McpResourceReadResponse>(result) {
                    Ok(response) => {
                        outgoing.send_response(request_id, response).await;
                    }
                    Err(error) => {
                        outgoing
                            .send_error(
                                request_id,
                                JSONRPCErrorError {
                                    code: INTERNAL_ERROR_CODE,
                                    message: format!(
                                        "failed to deserialize MCP resource read response: {error}"
                                    ),
                                    data: None,
                                },
                            )
                            .await;
                    }
                },
                Err(error) => {
                    outgoing
                        .send_error(
                            request_id,
                            JSONRPCErrorError {
                                code: INTERNAL_ERROR_CODE,
                                message: format!("{error:#}"),
                                data: None,
                            },
                        )
                        .await;
                }
            }
        });
    }

    async fn send_invalid_request_error(&self, request_id: ConnectionRequestId, message: String) {
        let error = JSONRPCErrorError {
            code: INVALID_REQUEST_ERROR_CODE,
            message,
            data: None,
        };
        self.outgoing.send_error(request_id, error).await;
    }

    fn input_too_large_error(actual_chars: usize) -> JSONRPCErrorError {
        JSONRPCErrorError {
            code: INVALID_PARAMS_ERROR_CODE,
            message: format!(
                "Input exceeds the maximum length of {MAX_USER_INPUT_TEXT_CHARS} characters."
            ),
            data: Some(serde_json::json!({
                "input_error_code": INPUT_TOO_LARGE_ERROR_CODE,
                "max_chars": MAX_USER_INPUT_TEXT_CHARS,
                "actual_chars": actual_chars,
            })),
        }
    }

    fn validate_v2_input_limit(items: &[V2UserInput]) -> Result<(), JSONRPCErrorError> {
        let actual_chars: usize = items.iter().map(V2UserInput::text_char_count).sum();
        if actual_chars > MAX_USER_INPUT_TEXT_CHARS {
            return Err(Self::input_too_large_error(actual_chars));
        }
        Ok(())
    }

    async fn send_internal_error(&self, request_id: ConnectionRequestId, message: String) {
        let error = JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
            message,
            data: None,
        };
        self.outgoing.send_error(request_id, error).await;
    }

    async fn send_marketplace_error(
        &self,
        request_id: ConnectionRequestId,
        err: MarketplaceError,
        action: &str,
    ) {
        match err {
            MarketplaceError::MarketplaceNotFound { .. } => {
                self.send_invalid_request_error(request_id, err.to_string())
                    .await;
            }
            MarketplaceError::Io { .. } => {
                self.send_internal_error(request_id, format!("failed to {action}: {err}"))
                    .await;
            }
            MarketplaceError::InvalidMarketplaceFile { .. }
            | MarketplaceError::PluginNotFound { .. }
            | MarketplaceError::PluginNotAvailable { .. }
            | MarketplaceError::PluginsDisabled
            | MarketplaceError::InvalidPlugin(_) => {
                self.send_invalid_request_error(request_id, err.to_string())
                    .await;
            }
        }
    }

    async fn wait_for_thread_shutdown(thread: &Arc<CodexThread>) -> ThreadShutdownResult {
        match tokio::time::timeout(Duration::from_secs(10), thread.shutdown_and_wait()).await {
            Ok(Ok(())) => ThreadShutdownResult::Complete,
            Ok(Err(_)) => ThreadShutdownResult::SubmitFailed,
            Err(_) => ThreadShutdownResult::TimedOut,
        }
    }

    async fn finalize_thread_teardown(&mut self, thread_id: ThreadId) {
        self.pending_thread_unloads.lock().await.remove(&thread_id);
        self.outgoing
            .cancel_requests_for_thread(thread_id, /*error*/ None)
            .await;
        self.thread_state_manager
            .remove_thread_state(thread_id)
            .await;
        self.thread_watch_manager
            .remove_thread(&thread_id.to_string())
            .await;
    }

    async fn thread_unsubscribe(
        &mut self,
        request_id: ConnectionRequestId,
        params: ThreadUnsubscribeParams,
    ) {
        let thread_id = match ThreadId::from_string(&params.thread_id) {
            Ok(id) => id,
            Err(err) => {
                self.send_invalid_request_error(request_id, format!("invalid thread id: {err}"))
                    .await;
                return;
            }
        };

        let Ok(thread) = self.thread_manager.get_thread(thread_id).await else {
            // Reconcile stale app-server bookkeeping when the thread has already been
            // removed from the core manager. This keeps loaded-status/subscription state
            // consistent with the source of truth before reporting NotLoaded.
            self.finalize_thread_teardown(thread_id).await;
            self.outgoing
                .send_response(
                    request_id,
                    ThreadUnsubscribeResponse {
                        status: ThreadUnsubscribeStatus::NotLoaded,
                    },
                )
                .await;
            return;
        };

        let was_subscribed = self
            .thread_state_manager
            .unsubscribe_connection_from_thread(thread_id, request_id.connection_id)
            .await;
        if !was_subscribed {
            self.outgoing
                .send_response(
                    request_id,
                    ThreadUnsubscribeResponse {
                        status: ThreadUnsubscribeStatus::NotSubscribed,
                    },
                )
                .await;
            return;
        }

        if !self.thread_state_manager.has_subscribers(thread_id).await {
            // This connection was the last subscriber. Only now do we unload the thread.
            info!("thread {thread_id} has no subscribers; shutting down");
            self.pending_thread_unloads.lock().await.insert(thread_id);
            // Any pending app-server -> client requests for this thread can no longer be
            // answered; cancel their callbacks before shutdown/unload.
            self.outgoing
                .cancel_requests_for_thread(thread_id, /*error*/ None)
                .await;
            self.thread_state_manager
                .remove_thread_state(thread_id)
                .await;

            let outgoing = self.outgoing.clone();
            let pending_thread_unloads = self.pending_thread_unloads.clone();
            let thread_manager = self.thread_manager.clone();
            let thread_watch_manager = self.thread_watch_manager.clone();
            tokio::spawn(async move {
                match Self::wait_for_thread_shutdown(&thread).await {
                    ThreadShutdownResult::Complete => {
                        if thread_manager.remove_thread(&thread_id).await.is_none() {
                            info!(
                                "thread {thread_id} was already removed before unsubscribe finalized"
                            );
                            thread_watch_manager
                                .remove_thread(&thread_id.to_string())
                                .await;
                            pending_thread_unloads.lock().await.remove(&thread_id);
                            return;
                        }
                        thread_watch_manager
                            .remove_thread(&thread_id.to_string())
                            .await;
                        let notification = ThreadClosedNotification {
                            thread_id: thread_id.to_string(),
                        };
                        outgoing
                            .send_server_notification(ServerNotification::ThreadClosed(
                                notification,
                            ))
                            .await;
                        pending_thread_unloads.lock().await.remove(&thread_id);
                    }
                    ThreadShutdownResult::SubmitFailed => {
                        pending_thread_unloads.lock().await.remove(&thread_id);
                        warn!("failed to submit Shutdown to thread {thread_id}");
                    }
                    ThreadShutdownResult::TimedOut => {
                        pending_thread_unloads.lock().await.remove(&thread_id);
                        warn!("thread {thread_id} shutdown timed out; leaving thread loaded");
                    }
                }
            });
        }

        self.outgoing
            .send_response(
                request_id,
                ThreadUnsubscribeResponse {
                    status: ThreadUnsubscribeStatus::Unsubscribed,
                },
            )
            .await;
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
            match Self::wait_for_thread_shutdown(&conversation).await {
                ThreadShutdownResult::Complete => {}
                ThreadShutdownResult::SubmitFailed => {
                    error!(
                        "failed to submit Shutdown to thread {thread_id}; proceeding with archive"
                    );
                }
                ThreadShutdownResult::TimedOut => {
                    warn!("thread {thread_id} shutdown timed out; proceeding with archive");
                }
            }
        }
        self.finalize_thread_teardown(thread_id).await;

        if state_db_ctx.is_none() {
            state_db_ctx = get_state_db(&self.config).await;
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

    async fn apps_list(&self, request_id: ConnectionRequestId, params: AppsListParams) {
        let mut config = match self.load_latest_config(/*fallback_cwd*/ None).await {
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

            let _ = config
                .features
                .set_enabled(Feature::Apps, thread.enabled(Feature::Apps));
        }

        let auth = self.auth_manager.auth().await;
        if !config
            .features
            .apps_enabled_for_auth(auth.as_ref().is_some_and(CodexAuth::is_chatgpt_auth))
        {
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
        let cached_all_connectors = all_connectors.clone();

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
        let mut last_notified_apps = None;

        if accessible_connectors.is_some() || all_connectors.is_some() {
            let merged = connectors::with_app_enabled_state(
                apps_list_helpers::merge_loaded_apps(
                    all_connectors.as_deref(),
                    accessible_connectors.as_deref(),
                ),
                &config,
            );
            if apps_list_helpers::should_send_app_list_updated_notification(
                merged.as_slice(),
                accessible_loaded,
                all_loaded,
            ) {
                apps_list_helpers::send_app_list_updated_notification(&outgoing, merged.clone())
                    .await;
                last_notified_apps = Some(merged);
            }
        }

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

            let showing_interim_force_refetch = force_refetch && !(accessible_loaded && all_loaded);
            let all_connectors_for_update =
                if showing_interim_force_refetch && cached_all_connectors.is_some() {
                    cached_all_connectors.as_deref()
                } else {
                    all_connectors.as_deref()
                };
            let accessible_connectors_for_update =
                if showing_interim_force_refetch && !accessible_loaded {
                    None
                } else {
                    accessible_connectors.as_deref()
                };
            let merged = connectors::with_app_enabled_state(
                apps_list_helpers::merge_loaded_apps(
                    all_connectors_for_update,
                    accessible_connectors_for_update,
                ),
                &config,
            );
            if apps_list_helpers::should_send_app_list_updated_notification(
                merged.as_slice(),
                accessible_loaded,
                all_loaded,
            ) && last_notified_apps.as_ref() != Some(&merged)
            {
                apps_list_helpers::send_app_list_updated_notification(&outgoing, merged.clone())
                    .await;
                last_notified_apps = Some(merged.clone());
            }

            if accessible_loaded && all_loaded {
                match apps_list_helpers::paginate_apps(merged.as_slice(), start, limit) {
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

    async fn skills_list(&self, request_id: ConnectionRequestId, params: SkillsListParams) {
        let SkillsListParams {
            cwds,
            force_reload,
            per_cwd_extra_user_roots,
        } = params;
        let cwds = if cwds.is_empty() {
            vec![self.config.cwd.to_path_buf()]
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

        let config = match self.load_latest_config(/*fallback_cwd*/ None).await {
            Ok(config) => config,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };
        let skills_manager = self.thread_manager.skills_manager();
        let plugins_manager = self.thread_manager.plugins_manager();
        let cli_overrides = self.current_cli_overrides();
        let mut data = Vec::new();
        for cwd in cwds {
            let extra_roots = extra_roots_by_cwd
                .get(&cwd)
                .map_or(&[][..], std::vec::Vec::as_slice);
            let cwd_abs = match AbsolutePathBuf::relative_to_current_dir(cwd.as_path()) {
                Ok(path) => path,
                Err(err) => {
                    let error_path = cwd.clone();
                    data.push(codex_app_server_protocol::SkillsListEntry {
                        cwd,
                        skills: Vec::new(),
                        errors: errors_to_info(&[codex_core::skills::SkillError {
                            path: error_path,
                            message: err.to_string(),
                        }]),
                    });
                    continue;
                }
            };
            let config_layer_stack = match load_config_layers_state(
                &self.config.codex_home,
                Some(cwd_abs),
                &cli_overrides,
                LoaderOverrides::default(),
                CloudRequirementsLoader::default(),
            )
            .await
            {
                Ok(config_layer_stack) => config_layer_stack,
                Err(err) => {
                    let error_path = cwd.clone();
                    data.push(codex_app_server_protocol::SkillsListEntry {
                        cwd,
                        skills: Vec::new(),
                        errors: errors_to_info(&[codex_core::skills::SkillError {
                            path: error_path,
                            message: err.to_string(),
                        }]),
                    });
                    continue;
                }
            };
            let effective_skill_roots = plugins_manager.effective_skill_roots_for_layer_stack(
                &config_layer_stack,
                config.features.enabled(Feature::Plugins),
            );
            let skills_input = codex_core::skills::SkillsLoadInput::new(
                cwd.clone(),
                effective_skill_roots,
                config_layer_stack,
                config.bundled_skills_enabled(),
            );
            let outcome = skills_manager
                .skills_for_cwd_with_extra_user_roots(&skills_input, force_reload, extra_roots)
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

    async fn plugin_list(&self, request_id: ConnectionRequestId, params: PluginListParams) {
        let plugins_manager = self.thread_manager.plugins_manager();
        let PluginListParams {
            cwds,
            force_remote_sync,
        } = params;
        let roots = cwds.unwrap_or_default();
        plugins_manager.maybe_start_non_curated_plugin_cache_refresh_for_roots(&roots);

        let mut config = match self.load_latest_config(/*fallback_cwd*/ None).await {
            Ok(config) => config,
            Err(err) => {
                self.outgoing.send_error(request_id, err).await;
                return;
            }
        };
        let mut remote_sync_error = None;
        let auth = self.auth_manager.auth().await;

        if force_remote_sync {
            match plugins_manager
                .sync_plugins_from_remote(&config, auth.as_ref(), /*additive_only*/ false)
                .await
            {
                Ok(sync_result) => {
                    info!(
                        installed_plugin_ids = ?sync_result.installed_plugin_ids,
                        enabled_plugin_ids = ?sync_result.enabled_plugin_ids,
                        disabled_plugin_ids = ?sync_result.disabled_plugin_ids,
                        uninstalled_plugin_ids = ?sync_result.uninstalled_plugin_ids,
                        "completed plugin/list remote sync"
                    );
                }
                Err(err) => {
                    warn!(
                        error = %err,
                        "plugin/list remote sync failed; returning local marketplace state"
                    );
                    remote_sync_error = Some(err.to_string());
                }
            }

            config = match self.load_latest_config(/*fallback_cwd*/ None).await {
                Ok(config) => config,
                Err(err) => {
                    self.outgoing.send_error(request_id, err).await;
                    return;
                }
            };
        }

        let config_for_marketplace_listing = config.clone();
        let plugins_manager_for_marketplace_listing = plugins_manager.clone();
        let (data, marketplace_load_errors) = match tokio::task::spawn_blocking(move || {
            let outcome = plugins_manager_for_marketplace_listing
                .list_marketplaces_for_config(&config_for_marketplace_listing, &roots)?;
            Ok::<
                (
                    Vec<PluginMarketplaceEntry>,
                    Vec<codex_app_server_protocol::MarketplaceLoadErrorInfo>,
                ),
                MarketplaceError,
            >((
                outcome
                    .marketplaces
                    .into_iter()
                    .map(|marketplace| PluginMarketplaceEntry {
                        name: marketplace.name,
                        path: marketplace.path,
                        interface: marketplace.interface.map(|interface| MarketplaceInterface {
                            display_name: interface.display_name,
                        }),
                        plugins: marketplace
                            .plugins
                            .into_iter()
                            .map(|plugin| PluginSummary {
                                id: plugin.id,
                                installed: plugin.installed,
                                enabled: plugin.enabled,
                                name: plugin.name,
                                source: marketplace_plugin_source_to_info(plugin.source),
                                install_policy: plugin.policy.installation.into(),
                                auth_policy: plugin.policy.authentication.into(),
                                interface: plugin.interface.map(plugin_interface_to_info),
                            })
                            .collect(),
                    })
                    .collect(),
                outcome
                    .errors
                    .into_iter()
                    .map(|err| codex_app_server_protocol::MarketplaceLoadErrorInfo {
                        marketplace_path: err.path,
                        message: err.message,
                    })
                    .collect(),
            ))
        })
        .await
        {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(err)) => {
                self.send_marketplace_error(request_id, err, "list marketplace plugins")
                    .await;
                return;
            }
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!("failed to list marketplace plugins: {err}"),
                )
                .await;
                return;
            }
        };

        let featured_plugin_ids = if data
            .iter()
            .any(|marketplace| marketplace.name == OPENAI_CURATED_MARKETPLACE_NAME)
        {
            match plugins_manager
                .featured_plugin_ids_for_config(&config, auth.as_ref())
                .await
            {
                Ok(featured_plugin_ids) => featured_plugin_ids,
                Err(err) => {
                    warn!(
                        error = %err,
                        "plugin/list featured plugin fetch failed; returning empty featured ids"
                    );
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        self.outgoing
            .send_response(
                request_id,
                PluginListResponse {
                    marketplaces: data,
                    marketplace_load_errors,
                    remote_sync_error,
                    featured_plugin_ids,
                },
            )
            .await;
    }

    async fn plugin_read(&self, request_id: ConnectionRequestId, params: PluginReadParams) {
        let plugins_manager = self.thread_manager.plugins_manager();
        let PluginReadParams {
            marketplace_path,
            plugin_name,
        } = params;
        let config_cwd = marketplace_path.as_path().parent().map(Path::to_path_buf);

        let config = match self.load_latest_config(config_cwd).await {
            Ok(config) => config,
            Err(err) => {
                self.outgoing.send_error(request_id, err).await;
                return;
            }
        };

        let request = PluginReadRequest {
            plugin_name,
            marketplace_path,
        };
        let config_for_read = config.clone();
        let outcome = match tokio::task::spawn_blocking(move || {
            plugins_manager.read_plugin_for_config(&config_for_read, &request)
        })
        .await
        {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(err)) => {
                self.send_marketplace_error(request_id, err, "read plugin details")
                    .await;
                return;
            }
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!("failed to read plugin details: {err}"),
                )
                .await;
                return;
            }
        };
        let app_summaries =
            plugin_app_helpers::load_plugin_app_summaries(&config, &outcome.plugin.apps).await;
        let visible_skills = outcome
            .plugin
            .skills
            .iter()
            .filter(|skill| {
                skill.matches_product_restriction_for_product(
                    self.thread_manager.session_source().restriction_product(),
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        let plugin = PluginDetail {
            marketplace_name: outcome.marketplace_name,
            marketplace_path: outcome.marketplace_path,
            summary: PluginSummary {
                id: outcome.plugin.id,
                name: outcome.plugin.name,
                source: marketplace_plugin_source_to_info(outcome.plugin.source),
                installed: outcome.plugin.installed,
                enabled: outcome.plugin.enabled,
                install_policy: outcome.plugin.policy.installation.into(),
                auth_policy: outcome.plugin.policy.authentication.into(),
                interface: outcome.plugin.interface.map(plugin_interface_to_info),
            },
            description: outcome.plugin.description,
            skills: plugin_skills_to_info(&visible_skills, &outcome.plugin.disabled_skill_paths),
            apps: app_summaries,
            mcp_servers: outcome.plugin.mcp_server_names,
        };

        self.outgoing
            .send_response(request_id, PluginReadResponse { plugin })
            .await;
    }

    async fn skills_config_write(
        &self,
        request_id: ConnectionRequestId,
        params: SkillsConfigWriteParams,
    ) {
        let SkillsConfigWriteParams {
            path,
            name,
            enabled,
        } = params;
        let edit = match (path, name) {
            (Some(path), None) => ConfigEdit::SetSkillConfig {
                path: path.into_path_buf(),
                enabled,
            },
            (None, Some(name)) if !name.trim().is_empty() => {
                ConfigEdit::SetSkillConfigByName { name, enabled }
            }
            _ => {
                let error = JSONRPCErrorError {
                    code: INVALID_PARAMS_ERROR_CODE,
                    message: "skills/config/write requires exactly one of path or name".to_string(),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };
        let edits = vec![edit];
        let result = ConfigEditsBuilder::new(&self.config.codex_home)
            .with_edits(edits)
            .apply()
            .await;

        match result {
            Ok(()) => {
                self.thread_manager.plugins_manager().clear_cache();
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

    async fn plugin_install(&self, request_id: ConnectionRequestId, params: PluginInstallParams) {
        let PluginInstallParams {
            marketplace_path,
            plugin_name,
            force_remote_sync,
        } = params;
        let config_cwd = marketplace_path.as_path().parent().map(Path::to_path_buf);

        let plugins_manager = self.thread_manager.plugins_manager();
        let request = PluginInstallRequest {
            plugin_name,
            marketplace_path,
        };

        let install_result = if force_remote_sync {
            let config = match self.load_latest_config(config_cwd.clone()).await {
                Ok(config) => config,
                Err(err) => {
                    self.outgoing.send_error(request_id, err).await;
                    return;
                }
            };
            let auth = self.auth_manager.auth().await;
            plugins_manager
                .install_plugin_with_remote_sync(&config, auth.as_ref(), request)
                .await
        } else {
            plugins_manager.install_plugin(request).await
        };

        match install_result {
            Ok(result) => {
                let config = match self.load_latest_config(config_cwd).await {
                    Ok(config) => config,
                    Err(err) => {
                        warn!(
                            "failed to reload config after plugin install, using current config: {err:?}"
                        );
                        self.config.as_ref().clone()
                    }
                };

                self.clear_plugin_related_caches();

                let plugin_mcp_servers = load_plugin_mcp_servers(result.installed_path.as_path());

                if !plugin_mcp_servers.is_empty() {
                    if let Err(err) = self.queue_mcp_server_refresh_for_config(&config).await {
                        warn!(
                            plugin = result.plugin_id.as_key(),
                            "failed to queue MCP refresh after plugin install: {err:?}"
                        );
                    }
                    self.start_plugin_mcp_oauth_logins(&config, plugin_mcp_servers)
                        .await;
                }

                let plugin_apps = load_plugin_apps(result.installed_path.as_path());
                let auth = self.auth_manager.auth().await;
                let apps_needing_auth = if plugin_apps.is_empty()
                    || !config.features.apps_enabled_for_auth(
                        auth.as_ref().is_some_and(CodexAuth::is_chatgpt_auth),
                    ) {
                    Vec::new()
                } else {
                    let (all_connectors_result, accessible_connectors_result) = tokio::join!(
                        connectors::list_all_connectors_with_options(&config, /*force_refetch*/ true),
                        connectors::list_accessible_connectors_from_mcp_tools_with_options_and_status(
                            &config, /*force_refetch*/ true
                        ),
                    );

                    let all_connectors = match all_connectors_result {
                        Ok(connectors) => connectors,
                        Err(err) => {
                            warn!(
                                plugin = result.plugin_id.as_key(),
                                "failed to load app metadata after plugin install: {err:#}"
                            );
                            connectors::list_cached_all_connectors(&config)
                                .await
                                .unwrap_or_default()
                        }
                    };
                    let all_connectors =
                        connectors::connectors_for_plugin_apps(all_connectors, &plugin_apps);
                    let (accessible_connectors, codex_apps_ready) =
                        match accessible_connectors_result {
                            Ok(status) => (status.connectors, status.codex_apps_ready),
                            Err(err) => {
                                warn!(
                                    plugin = result.plugin_id.as_key(),
                                    "failed to load accessible apps after plugin install: {err:#}"
                                );
                                (
                                    connectors::list_cached_accessible_connectors_from_mcp_tools(
                                        &config,
                                    )
                                    .await
                                    .unwrap_or_default(),
                                    false,
                                )
                            }
                        };
                    if !codex_apps_ready {
                        warn!(
                            plugin = result.plugin_id.as_key(),
                            "codex_apps MCP not ready after plugin install; skipping appsNeedingAuth check"
                        );
                    }

                    plugin_app_helpers::plugin_apps_needing_auth(
                        &all_connectors,
                        &accessible_connectors,
                        &plugin_apps,
                        codex_apps_ready,
                    )
                };

                self.outgoing
                    .send_response(
                        request_id,
                        PluginInstallResponse {
                            auth_policy: result.auth_policy.into(),
                            apps_needing_auth,
                        },
                    )
                    .await;
            }
            Err(err) => {
                if err.is_invalid_request() {
                    self.send_invalid_request_error(request_id, err.to_string())
                        .await;
                    return;
                }

                match err {
                    CorePluginInstallError::Marketplace(err) => {
                        self.send_marketplace_error(request_id, err, "install plugin")
                            .await;
                    }
                    CorePluginInstallError::Config(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to persist installed plugin config: {err}"),
                        )
                        .await;
                    }
                    CorePluginInstallError::Remote(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to enable remote plugin: {err}"),
                        )
                        .await;
                    }
                    CorePluginInstallError::Join(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to install plugin: {err}"),
                        )
                        .await;
                    }
                    CorePluginInstallError::Store(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to install plugin: {err}"),
                        )
                        .await;
                    }
                }
            }
        }
    }

    async fn plugin_uninstall(
        &self,
        request_id: ConnectionRequestId,
        params: PluginUninstallParams,
    ) {
        let PluginUninstallParams {
            plugin_id,
            force_remote_sync,
        } = params;
        let plugins_manager = self.thread_manager.plugins_manager();

        let uninstall_result = if force_remote_sync {
            let config = match self.load_latest_config(/*fallback_cwd*/ None).await {
                Ok(config) => config,
                Err(err) => {
                    self.outgoing.send_error(request_id, err).await;
                    return;
                }
            };
            let auth = self.auth_manager.auth().await;
            plugins_manager
                .uninstall_plugin_with_remote_sync(&config, auth.as_ref(), plugin_id)
                .await
        } else {
            plugins_manager.uninstall_plugin(plugin_id).await
        };

        match uninstall_result {
            Ok(()) => {
                self.clear_plugin_related_caches();
                self.outgoing
                    .send_response(request_id, PluginUninstallResponse {})
                    .await;
            }
            Err(err) => {
                if err.is_invalid_request() {
                    self.send_invalid_request_error(request_id, err.to_string())
                        .await;
                    return;
                }

                match err {
                    CorePluginUninstallError::Config(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to clear plugin config: {err}"),
                        )
                        .await;
                    }
                    CorePluginUninstallError::Remote(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to uninstall remote plugin: {err}"),
                        )
                        .await;
                    }
                    CorePluginUninstallError::Join(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to uninstall plugin: {err}"),
                        )
                        .await;
                    }
                    CorePluginUninstallError::Store(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to uninstall plugin: {err}"),
                        )
                        .await;
                    }
                    CorePluginUninstallError::InvalidPluginId(_) => {
                        unreachable!("invalid plugin ids are handled above");
                    }
                }
            }
        }
    }

    async fn turn_start(
        &self,
        request_id: ConnectionRequestId,
        params: TurnStartParams,
        app_server_client_name: Option<String>,
        app_server_client_version: Option<String>,
    ) {
        if let Err(error) = Self::validate_v2_input_limit(&params.input) {
            self.outgoing.send_error(request_id, error).await;
            return;
        }
        let (_, thread) = match self.load_thread(&params.thread_id).await {
            Ok(v) => v,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };
        if let Err(error) = Self::set_app_server_client_info(
            thread.as_ref(),
            app_server_client_name,
            app_server_client_version,
        )
        .await
        {
            self.outgoing.send_error(request_id, error).await;
            return;
        }

        let collaboration_modes_config = CollaborationModesConfig {
            default_mode_request_user_input: thread.enabled(Feature::DefaultModeRequestUserInput),
        };
        let collaboration_mode = params.collaboration_mode.map(|mode| {
            self.normalize_turn_start_collaboration_mode(mode, collaboration_modes_config)
        });

        // Map v2 input items to core input items.
        let mapped_items: Vec<CoreInputItem> = params
            .input
            .into_iter()
            .map(V2UserInput::into_core)
            .collect();

        let has_any_overrides = params.cwd.is_some()
            || params.approval_policy.is_some()
            || params.approvals_reviewer.is_some()
            || params.sandbox_policy.is_some()
            || params.model.is_some()
            || params.service_tier.is_some()
            || params.effort.is_some()
            || params.summary.is_some()
            || collaboration_mode.is_some()
            || params.personality.is_some();

        // If any overrides are provided, update the session turn context first.
        if has_any_overrides {
            let _ = self
                .submit_core_op(
                    &request_id,
                    thread.as_ref(),
                    Op::OverrideTurnContext {
                        cwd: params.cwd,
                        approval_policy: params.approval_policy.map(AskForApproval::to_core),
                        approvals_reviewer: params
                            .approvals_reviewer
                            .map(codex_app_server_protocol::ApprovalsReviewer::to_core),
                        sandbox_policy: params.sandbox_policy.map(|p| p.to_core()),
                        windows_sandbox_level: None,
                        model: params.model,
                        effort: params.effort.map(Some),
                        summary: params.summary,
                        service_tier: params.service_tier,
                        collaboration_mode,
                        personality: params.personality,
                    },
                )
                .await;
        }

        // Start the turn by submitting the user input. Return its submission id as turn_id.
        let turn_id = self
            .submit_core_op(
                &request_id,
                thread.as_ref(),
                Op::UserInput {
                    items: mapped_items,
                    final_output_json_schema: params.output_schema,
                },
            )
            .await;

        match turn_id {
            Ok(turn_id) => {
                self.outgoing
                    .record_request_turn_id(&request_id, &turn_id)
                    .await;
                let turn = Turn {
                    id: turn_id.clone(),
                    items: vec![],
                    error: None,
                    status: TurnStatus::InProgress,
                    started_at: None,
                    completed_at: None,
                    duration_ms: None,
                };

                let response = TurnStartResponse { turn };
                self.outgoing.send_response(request_id, response).await;
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

    async fn set_app_server_client_info(
        thread: &CodexThread,
        app_server_client_name: Option<String>,
        app_server_client_version: Option<String>,
    ) -> Result<(), JSONRPCErrorError> {
        thread
            .set_app_server_client_info(app_server_client_name, app_server_client_version)
            .await
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to set app server client info: {err}"),
                data: None,
            })
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
        self.outgoing
            .record_request_turn_id(&request_id, &params.expected_turn_id)
            .await;
        if let Err(error) = Self::validate_v2_input_limit(&params.input) {
            self.outgoing.send_error(request_id, error).await;
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
                let (code, message, data) = match err {
                    SteerInputError::NoActiveTurn(_) => (
                        INVALID_REQUEST_ERROR_CODE,
                        "no active turn to steer".to_string(),
                        None,
                    ),
                    SteerInputError::ExpectedTurnMismatch { expected, actual } => (
                        INVALID_REQUEST_ERROR_CODE,
                        format!("expected active turn id `{expected}` but found `{actual}`"),
                        None,
                    ),
                    SteerInputError::ActiveTurnNotSteerable { turn_kind } => {
                        let message = match turn_kind {
                            codex_protocol::protocol::NonSteerableTurnKind::Review => {
                                "cannot steer a review turn".to_string()
                            }
                            codex_protocol::protocol::NonSteerableTurnKind::Compact => {
                                "cannot steer a compact turn".to_string()
                            }
                        };
                        let error = TurnError {
                            message: message.clone(),
                            codex_error_info: Some(
                                AppServerCodexErrorInfo::ActiveTurnNotSteerable {
                                    turn_kind: turn_kind.into(),
                                },
                            ),
                            additional_details: None,
                        };
                        let data = match serde_json::to_value(error) {
                            Ok(data) => Some(data),
                            Err(error) => {
                                tracing::error!(
                                    ?error,
                                    "failed to serialize active-turn-not-steerable turn error"
                                );
                                None
                            }
                        };
                        (INVALID_REQUEST_ERROR_CODE, message, data)
                    }
                    SteerInputError::EmptyInput => (
                        INVALID_REQUEST_ERROR_CODE,
                        "input must not be empty".to_string(),
                        None,
                    ),
                };
                let error = JSONRPCErrorError {
                    code,
                    message,
                    data,
                };
                self.outgoing.send_error(request_id, error).await;
            }
        }
    }

    async fn prepare_realtime_conversation_thread(
        &mut self,
        request_id: ConnectionRequestId,
        thread_id: &str,
    ) -> Option<(ThreadId, Arc<CodexThread>)> {
        let (thread_id, thread) = match self.load_thread(thread_id).await {
            Ok(v) => v,
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return None;
            }
        };

        match self
            .ensure_conversation_listener(
                thread_id,
                request_id.connection_id,
                /*raw_events_enabled*/ false,
                ApiVersion::V2,
            )
            .await
        {
            Ok(EnsureConversationListenerResult::Attached) => {}
            Ok(EnsureConversationListenerResult::ConnectionClosed) => {
                return None;
            }
            Err(error) => {
                self.outgoing.send_error(request_id, error).await;
                return None;
            }
        }

        if !thread.enabled(Feature::RealtimeConversation) {
            self.send_invalid_request_error(
                request_id,
                format!("thread {thread_id} does not support realtime conversation"),
            )
            .await;
            return None;
        }

        Some((thread_id, thread))
    }

    async fn thread_realtime_start(
        &mut self,
        request_id: ConnectionRequestId,
        params: ThreadRealtimeStartParams,
    ) {
        let Some((_, thread)) = self
            .prepare_realtime_conversation_thread(request_id.clone(), &params.thread_id)
            .await
        else {
            return;
        };

        let submit = self
            .submit_core_op(
                &request_id,
                thread.as_ref(),
                Op::RealtimeConversationStart(ConversationStartParams {
                    prompt: params.prompt,
                    session_id: params.session_id,
                }),
            )
            .await;

        match submit {
            Ok(_) => {
                self.outgoing
                    .send_response(request_id, ThreadRealtimeStartResponse::default())
                    .await;
            }
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!("failed to start realtime conversation: {err}"),
                )
                .await;
            }
        }
    }

    async fn thread_realtime_append_audio(
        &mut self,
        request_id: ConnectionRequestId,
        params: ThreadRealtimeAppendAudioParams,
    ) {
        let Some((_, thread)) = self
            .prepare_realtime_conversation_thread(request_id.clone(), &params.thread_id)
            .await
        else {
            return;
        };

        let submit = self
            .submit_core_op(
                &request_id,
                thread.as_ref(),
                Op::RealtimeConversationAudio(ConversationAudioParams {
                    frame: params.audio.into(),
                }),
            )
            .await;

        match submit {
            Ok(_) => {
                self.outgoing
                    .send_response(request_id, ThreadRealtimeAppendAudioResponse::default())
                    .await;
            }
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!("failed to append realtime conversation audio: {err}"),
                )
                .await;
            }
        }
    }

    async fn thread_realtime_append_text(
        &mut self,
        request_id: ConnectionRequestId,
        params: ThreadRealtimeAppendTextParams,
    ) {
        let Some((_, thread)) = self
            .prepare_realtime_conversation_thread(request_id.clone(), &params.thread_id)
            .await
        else {
            return;
        };

        let submit = self
            .submit_core_op(
                &request_id,
                thread.as_ref(),
                Op::RealtimeConversationText(ConversationTextParams { text: params.text }),
            )
            .await;

        match submit {
            Ok(_) => {
                self.outgoing
                    .send_response(request_id, ThreadRealtimeAppendTextResponse::default())
                    .await;
            }
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!("failed to append realtime conversation text: {err}"),
                )
                .await;
            }
        }
    }

    async fn thread_realtime_stop(
        &mut self,
        request_id: ConnectionRequestId,
        params: ThreadRealtimeStopParams,
    ) {
        let Some((_, thread)) = self
            .prepare_realtime_conversation_thread(request_id.clone(), &params.thread_id)
            .await
        else {
            return;
        };

        let submit = self
            .submit_core_op(&request_id, thread.as_ref(), Op::RealtimeConversationClose)
            .await;

        match submit {
            Ok(_) => {
                self.outgoing
                    .send_response(request_id, ThreadRealtimeStopResponse::default())
                    .await;
            }
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!("failed to stop realtime conversation: {err}"),
                )
                .await;
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
            started_at: None,
            completed_at: None,
            duration_ms: None,
        }
    }

    async fn emit_review_started(
        &self,
        request_id: &ConnectionRequestId,
        turn: Turn,
        review_thread_id: String,
    ) {
        let response = ReviewStartResponse {
            turn,
            review_thread_id,
        };
        self.outgoing
            .send_response(request_id.clone(), response)
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
        let turn_id = self
            .submit_core_op(
                request_id,
                parent_thread.as_ref(),
                Op::Review { review_request },
            )
            .await;

        match turn_id {
            Ok(turn_id) => {
                let turn = Self::build_review_turn(turn_id, display_text);
                self.emit_review_started(request_id, turn, parent_thread_id)
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
            .fork_thread(
                ForkSnapshot::Interrupted,
                config,
                rollout_path,
                /*persist_extended_history*/ false,
                self.request_trace_context(request_id).await,
            )
            .await
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("error creating detached review thread: {err}"),
                data: None,
            })?;

        Self::log_listener_attach_result(
            self.ensure_conversation_listener(
                thread_id,
                request_id.connection_id,
                /*raw_events_enabled*/ false,
                ApiVersion::V2,
            )
            .await,
            thread_id,
            request_id.connection_id,
            "review thread",
        );

        let fallback_provider = self.config.model_provider_id.as_str();
        if let Some(rollout_path) = review_thread.rollout_path() {
            match read_summary_from_rollout(rollout_path.as_path(), fallback_provider).await {
                Ok(summary) => {
                    let mut thread = summary_to_thread(summary);
                    self.thread_watch_manager
                        .upsert_thread_silently(thread.clone())
                        .await;
                    thread.status = resolve_thread_status(
                        self.thread_watch_manager
                            .loaded_status_for_thread(&thread.id)
                            .await,
                        /*has_in_progress_turn*/ false,
                    );
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

        let turn_id = self
            .submit_core_op(
                request_id,
                review_thread.as_ref(),
                Op::Review { review_request },
            )
            .await
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to start detached review turn: {err}"),
                data: None,
            })?;

        let turn = Self::build_review_turn(turn_id, display_text);
        let review_thread_id = thread_id.to_string();
        self.emit_review_started(request_id, turn, review_thread_id)
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
        let TurnInterruptParams { thread_id, turn_id } = params;
        self.outgoing
            .record_request_turn_id(&request_id, &turn_id)
            .await;

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
            let thread_state = self.thread_state_manager.thread_state(thread_uuid).await;
            let mut thread_state = thread_state.lock().await;
            thread_state
                .pending_interrupts
                .push((request, ApiVersion::V2));
        }

        // Submit the interrupt; we'll respond upon TurnAborted.
        let _ = self
            .submit_core_op(&request_id, thread.as_ref(), Op::Interrupt)
            .await;
    }

    async fn ensure_conversation_listener(
        &self,
        conversation_id: ThreadId,
        connection_id: ConnectionId,
        raw_events_enabled: bool,
        api_version: ApiVersion,
    ) -> Result<EnsureConversationListenerResult, JSONRPCErrorError> {
        Self::ensure_conversation_listener_task(
            ListenerTaskContext {
                thread_manager: Arc::clone(&self.thread_manager),
                thread_state_manager: self.thread_state_manager.clone(),
                outgoing: Arc::clone(&self.outgoing),
                analytics_events_client: self.analytics_events_client.clone(),
                general_analytics_enabled: self.config.features.enabled(Feature::GeneralAnalytics),
                thread_watch_manager: self.thread_watch_manager.clone(),
                fallback_model_provider: self.config.model_provider_id.clone(),
                codex_home: self.config.codex_home.clone(),
            },
            conversation_id,
            connection_id,
            raw_events_enabled,
            api_version,
        )
        .await
    }

    async fn ensure_conversation_listener_task(
        listener_task_context: ListenerTaskContext,
        conversation_id: ThreadId,
        connection_id: ConnectionId,
        raw_events_enabled: bool,
        api_version: ApiVersion,
    ) -> Result<EnsureConversationListenerResult, JSONRPCErrorError> {
        let conversation = match listener_task_context
            .thread_manager
            .get_thread(conversation_id)
            .await
        {
            Ok(conv) => conv,
            Err(_) => {
                return Err(JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("thread not found: {conversation_id}"),
                    data: None,
                });
            }
        };
        let Some(thread_state) = listener_task_context
            .thread_state_manager
            .try_ensure_connection_subscribed(conversation_id, connection_id, raw_events_enabled)
            .await
        else {
            return Ok(EnsureConversationListenerResult::ConnectionClosed);
        };
        Self::ensure_listener_task_running_task(
            listener_task_context,
            conversation_id,
            conversation,
            thread_state,
            api_version,
        )
        .await;
        Ok(EnsureConversationListenerResult::Attached)
    }

    fn log_listener_attach_result(
        result: Result<EnsureConversationListenerResult, JSONRPCErrorError>,
        thread_id: ThreadId,
        connection_id: ConnectionId,
        thread_kind: &'static str,
    ) {
        match result {
            Ok(EnsureConversationListenerResult::Attached) => {}
            Ok(EnsureConversationListenerResult::ConnectionClosed) => {
                tracing::debug!(
                    thread_id = %thread_id,
                    connection_id = ?connection_id,
                    "skipping auto-attach for closed connection"
                );
            }
            Err(err) => {
                tracing::warn!(
                    "failed to attach listener for {thread_kind} {thread_id}: {message}",
                    message = err.message
                );
            }
        }
    }

    async fn ensure_listener_task_running(
        &self,
        conversation_id: ThreadId,
        conversation: Arc<CodexThread>,
        thread_state: Arc<Mutex<ThreadState>>,
        api_version: ApiVersion,
    ) {
        Self::ensure_listener_task_running_task(
            ListenerTaskContext {
                thread_manager: Arc::clone(&self.thread_manager),
                thread_state_manager: self.thread_state_manager.clone(),
                outgoing: Arc::clone(&self.outgoing),
                analytics_events_client: self.analytics_events_client.clone(),
                general_analytics_enabled: self.config.features.enabled(Feature::GeneralAnalytics),
                thread_watch_manager: self.thread_watch_manager.clone(),
                fallback_model_provider: self.config.model_provider_id.clone(),
                codex_home: self.config.codex_home.clone(),
            },
            conversation_id,
            conversation,
            thread_state,
            api_version,
        )
        .await;
    }

    async fn ensure_listener_task_running_task(
        listener_task_context: ListenerTaskContext,
        conversation_id: ThreadId,
        conversation: Arc<CodexThread>,
        thread_state: Arc<Mutex<ThreadState>>,
        api_version: ApiVersion,
    ) {
        let (cancel_tx, mut cancel_rx) = oneshot::channel();
        let (mut listener_command_rx, listener_generation) = {
            let mut thread_state = thread_state.lock().await;
            if thread_state.listener_matches(&conversation) {
                return;
            }
            thread_state.set_listener(cancel_tx, &conversation)
        };
        let ListenerTaskContext {
            outgoing,
            thread_manager,
            thread_state_manager,
            analytics_events_client: _,
            general_analytics_enabled: _,
            thread_watch_manager,
            fallback_model_provider,
            codex_home,
        } = listener_task_context;
        let outgoing_for_task = Arc::clone(&outgoing);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut cancel_rx => {
                        // Listener was superseded or the thread is being torn down.
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

                        // Track the event before emitting any typed
                        // translations so thread-local state such as raw event
                        // opt-in stays synchronized with the conversation.
                        let raw_events_enabled = {
                            let mut thread_state = thread_state.lock().await;
                            thread_state.track_current_turn_event(&event.msg);
                            thread_state.experimental_raw_events
                        };
                        let subscribed_connection_ids = thread_state_manager
                            .subscribed_connection_ids(conversation_id)
                            .await;
                        if let EventMsg::RawResponseItem(_) = &event.msg && !raw_events_enabled {
                            continue;
                        }

                        let thread_outgoing = ThreadScopedOutgoingMessageSender::new(
                            outgoing_for_task.clone(),
                            subscribed_connection_ids,
                            conversation_id,
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
                            codex_home.as_path(),
                        )
                        .await;
                    }
                    listener_command = listener_command_rx.recv() => {
                        let Some(listener_command) = listener_command else {
                            break;
                        };
                        handle_thread_listener_command(
                            conversation_id,
                            &conversation,
                            codex_home.as_path(),
                            &thread_state_manager,
                            &thread_state,
                            &thread_watch_manager,
                            &outgoing_for_task,
                            listener_command,
                        )
                        .await;
                    }
                }
            }

            let mut thread_state = thread_state.lock().await;
            if thread_state.listener_generation == listener_generation {
                thread_state.clear_listener();
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
            extra_log_files,
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

        if let Some(chatgpt_user_id) = self
            .auth_manager
            .auth_cached()
            .and_then(|auth| auth.get_chatgpt_user_id())
        {
            tracing::info!(target: "feedback_tags", chatgpt_user_id);
        }
        let snapshot = self.feedback.snapshot(conversation_id);
        let thread_id = snapshot.thread_id.clone();
        let sqlite_feedback_logs = if include_logs {
            if let Some(log_db) = self.log_db.as_ref() {
                log_db.flush().await;
            }
            let state_db_ctx = get_state_db(&self.config).await;
            match (state_db_ctx.as_ref(), conversation_id) {
                (Some(state_db_ctx), Some(conversation_id)) => {
                    let thread_id_text = conversation_id.to_string();
                    match state_db_ctx.query_feedback_logs(&thread_id_text).await {
                        Ok(logs) if logs.is_empty() => None,
                        Ok(logs) => Some(logs),
                        Err(err) => {
                            warn!(
                                "failed to query feedback logs from sqlite for thread_id={thread_id_text}: {err}"
                            );
                            None
                        }
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        let validated_rollout_path = if include_logs {
            match conversation_id {
                Some(conv_id) => self.resolve_rollout_path(conv_id).await,
                None => None,
            }
        } else {
            None
        };
        let mut attachment_paths = validated_rollout_path.into_iter().collect::<Vec<_>>();
        if let Some(extra_log_files) = extra_log_files {
            attachment_paths.extend(extra_log_files);
        }

        let session_source = self.thread_manager.session_source();

        let upload_result = tokio::task::spawn_blocking(move || {
            snapshot.upload_feedback(
                &classification,
                reason.as_deref(),
                include_logs,
                &attachment_paths,
                Some(session_source),
                sqlite_feedback_logs,
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
        let cloud_requirements = self.current_cloud_requirements();
        let command_cwd = params.cwd.unwrap_or_else(|| config.cwd.clone());
        let cli_overrides = self.current_cli_overrides();
        let runtime_feature_enablement = self.current_runtime_feature_enablement();
        let outgoing = Arc::clone(&self.outgoing);
        let connection_id = request_id.connection_id;

        tokio::spawn(async move {
            let derived_config = derive_config_for_cwd(
                &cli_overrides,
                /*request_overrides*/ None,
                ConfigOverrides {
                    cwd: Some(command_cwd.clone()),
                    ..Default::default()
                },
                Some(command_cwd.to_path_buf()),
                &cloud_requirements,
                &config.codex_home,
                &runtime_feature_enablement,
            )
            .await;
            let setup_result = match derived_config {
                Ok(config) => {
                    let setup_request = WindowsSandboxSetupRequest {
                        mode,
                        policy: config.permissions.sandbox_policy.get().clone(),
                        policy_cwd: config.cwd.clone(),
                        command_cwd,
                        env_map: std::env::vars().collect(),
                        codex_home: config.codex_home.clone(),
                        active_profile: config.active_profile.clone(),
                    };
                    codex_core::windows_sandbox::run_windows_sandbox_setup(setup_request).await
                }
                Err(err) => Err(err.into()),
            };
            let notification = WindowsSandboxSetupCompletedNotification {
                mode: match mode {
                    CoreWindowsSandboxSetupMode::Elevated => WindowsSandboxSetupMode::Elevated,
                    CoreWindowsSandboxSetupMode::Unelevated => WindowsSandboxSetupMode::Unelevated,
                },
                success: setup_result.is_ok(),
                error: setup_result.err().map(|err| err.to_string()),
            };
            outgoing
                .send_server_notification_to_connections(
                    &[connection_id],
                    ServerNotification::WindowsSandboxSetupCompleted(notification),
                )
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

fn normalize_thread_list_cwd_filter(
    cwd: Option<String>,
) -> Result<Option<PathBuf>, JSONRPCErrorError> {
    let Some(cwd) = cwd else {
        return Ok(None);
    };
    AbsolutePathBuf::relative_to_current_dir(cwd.as_str())
        .map(AbsolutePathBuf::into_path_buf)
        .map(Some)
        .map_err(|err| JSONRPCErrorError {
            code: INVALID_PARAMS_ERROR_CODE,
            message: format!("invalid thread/list cwd filter `{cwd}`: {err}"),
            data: None,
        })
}

#[cfg(test)]
mod thread_list_cwd_filter_tests {
    use super::normalize_thread_list_cwd_filter;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn normalize_thread_list_cwd_filter_preserves_absolute_paths() {
        let cwd = if cfg!(windows) {
            String::from(r"C:\srv\repo-b")
        } else {
            String::from("/srv/repo-b")
        };

        assert_eq!(
            normalize_thread_list_cwd_filter(Some(cwd.clone())).expect("cwd filter should parse"),
            Some(PathBuf::from(cwd))
        );
    }

    #[test]
    fn normalize_thread_list_cwd_filter_resolves_relative_paths_against_server_cwd()
    -> std::io::Result<()> {
        let expected = AbsolutePathBuf::relative_to_current_dir("repo-b")?.to_path_buf();

        assert_eq!(
            normalize_thread_list_cwd_filter(Some(String::from("repo-b")))
                .expect("cwd filter should parse"),
            Some(expected)
        );
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_thread_listener_command(
    conversation_id: ThreadId,
    conversation: &Arc<CodexThread>,
    codex_home: &Path,
    thread_state_manager: &ThreadStateManager,
    thread_state: &Arc<Mutex<ThreadState>>,
    thread_watch_manager: &ThreadWatchManager,
    outgoing: &Arc<OutgoingMessageSender>,
    listener_command: ThreadListenerCommand,
) {
    match listener_command {
        ThreadListenerCommand::SendThreadResumeResponse(resume_request) => {
            handle_pending_thread_resume_request(
                conversation_id,
                conversation,
                codex_home,
                thread_state_manager,
                thread_state,
                thread_watch_manager,
                outgoing,
                *resume_request,
            )
            .await;
        }
        ThreadListenerCommand::ResolveServerRequest {
            request_id,
            completion_tx,
        } => {
            resolve_pending_server_request(
                conversation_id,
                thread_state_manager,
                outgoing,
                request_id,
            )
            .await;
            let _ = completion_tx.send(());
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_pending_thread_resume_request(
    conversation_id: ThreadId,
    conversation: &Arc<CodexThread>,
    codex_home: &Path,
    thread_state_manager: &ThreadStateManager,
    thread_state: &Arc<Mutex<ThreadState>>,
    thread_watch_manager: &ThreadWatchManager,
    outgoing: &Arc<OutgoingMessageSender>,
    pending: crate::thread_state::PendingThreadResumeRequest,
) {
    let active_turn = {
        let state = thread_state.lock().await;
        state.active_turn_snapshot()
    };
    tracing::debug!(
        thread_id = %conversation_id,
        request_id = ?pending.request_id,
        active_turn_present = active_turn.is_some(),
        active_turn_id = ?active_turn.as_ref().map(|turn| turn.id.as_str()),
        active_turn_status = ?active_turn.as_ref().map(|turn| &turn.status),
        "composing running thread resume response"
    );
    let has_live_in_progress_turn =
        matches!(conversation.agent_status().await, AgentStatus::Running)
            || active_turn
                .as_ref()
                .is_some_and(|turn| matches!(turn.status, TurnStatus::InProgress));

    let request_id = pending.request_id;
    let connection_id = request_id.connection_id;
    let mut thread = pending.thread_summary;
    if let Err(message) = populate_thread_turns(
        &mut thread,
        ThreadTurnSource::RolloutPath(pending.rollout_path.as_path()),
        active_turn.as_ref(),
    )
    .await
    {
        outgoing
            .send_error(
                request_id,
                JSONRPCErrorError {
                    code: INTERNAL_ERROR_CODE,
                    message,
                    data: None,
                },
            )
            .await;
        return;
    }

    let thread_status = thread_watch_manager
        .loaded_status_for_thread(&thread.id)
        .await;

    set_thread_status_and_interrupt_stale_turns(
        &mut thread,
        thread_status,
        has_live_in_progress_turn,
    );

    match find_thread_name_by_id(codex_home, &conversation_id).await {
        Ok(thread_name) => thread.name = thread_name,
        Err(err) => warn!("Failed to read thread name for {conversation_id}: {err}"),
    }

    let ThreadConfigSnapshot {
        model,
        model_provider_id,
        service_tier,
        approval_policy,
        approvals_reviewer,
        sandbox_policy,
        cwd,
        reasoning_effort,
        ..
    } = pending.config_snapshot;
    let response = ThreadResumeResponse {
        thread,
        model,
        model_provider: model_provider_id,
        service_tier,
        cwd: cwd.to_path_buf(),
        approval_policy: approval_policy.into(),
        approvals_reviewer: approvals_reviewer.into(),
        sandbox: sandbox_policy.into(),
        reasoning_effort,
    };
    outgoing.send_response(request_id, response).await;
    outgoing
        .replay_requests_to_connection_for_thread(connection_id, conversation_id)
        .await;
    let _attached = thread_state_manager
        .try_add_connection_to_thread(conversation_id, connection_id)
        .await;
}

enum ThreadTurnSource<'a> {
    RolloutPath(&'a Path),
    HistoryItems(&'a [RolloutItem]),
}

async fn populate_thread_turns(
    thread: &mut Thread,
    turn_source: ThreadTurnSource<'_>,
    active_turn: Option<&Turn>,
) -> std::result::Result<(), String> {
    let mut turns = match turn_source {
        ThreadTurnSource::RolloutPath(rollout_path) => {
            read_rollout_items_from_rollout(rollout_path)
                .await
                .map(|items| build_turns_from_rollout_items(&items))
                .map_err(|err| {
                    format!(
                        "failed to load rollout `{}` for thread {}: {err}",
                        rollout_path.display(),
                        thread.id
                    )
                })?
        }
        ThreadTurnSource::HistoryItems(items) => build_turns_from_rollout_items(items),
    };
    if let Some(active_turn) = active_turn {
        merge_turn_history_with_active_turn(&mut turns, active_turn.clone());
    }
    thread.turns = turns;
    Ok(())
}

async fn resolve_pending_server_request(
    conversation_id: ThreadId,
    thread_state_manager: &ThreadStateManager,
    outgoing: &Arc<OutgoingMessageSender>,
    request_id: RequestId,
) {
    let thread_id = conversation_id.to_string();
    let subscribed_connection_ids = thread_state_manager
        .subscribed_connection_ids(conversation_id)
        .await;
    let outgoing = ThreadScopedOutgoingMessageSender::new(
        outgoing.clone(),
        subscribed_connection_ids,
        conversation_id,
    );
    outgoing
        .send_server_notification(ServerNotification::ServerRequestResolved(
            ServerRequestResolvedNotification {
                thread_id,
                request_id,
            },
        ))
        .await;
}

fn merge_turn_history_with_active_turn(turns: &mut Vec<Turn>, active_turn: Turn) {
    turns.retain(|turn| turn.id != active_turn.id);
    turns.push(active_turn);
}

fn set_thread_status_and_interrupt_stale_turns(
    thread: &mut Thread,
    loaded_status: ThreadStatus,
    has_live_in_progress_turn: bool,
) {
    let status = resolve_thread_status(loaded_status, has_live_in_progress_turn);
    if !matches!(status, ThreadStatus::Active { .. }) {
        for turn in &mut thread.turns {
            if matches!(turn.status, TurnStatus::InProgress) {
                turn.status = TurnStatus::Interrupted;
            }
        }
    }
    thread.status = status;
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
    if let Some(requested_service_tier) = request.service_tier.as_ref()
        && requested_service_tier != &config_snapshot.service_tier
    {
        mismatch_details.push(format!(
            "service_tier requested={requested_service_tier:?} active={:?}",
            config_snapshot.service_tier
        ));
    }
    if let Some(requested_cwd) = request.cwd.as_deref() {
        let requested_cwd_path = std::path::PathBuf::from(requested_cwd);
        if requested_cwd_path != config_snapshot.cwd.as_path() {
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
    if let Some(requested_review_policy) = request.approvals_reviewer.as_ref() {
        let active_review_policy: codex_app_server_protocol::ApprovalsReviewer =
            config_snapshot.approvals_reviewer.into();
        if requested_review_policy != &active_review_policy {
            mismatch_details.push(format!(
                "approvals_reviewer requested={requested_review_policy:?} active={active_review_policy:?}"
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

fn merge_persisted_resume_metadata(
    request_overrides: &mut Option<HashMap<String, serde_json::Value>>,
    typesafe_overrides: &mut ConfigOverrides,
    persisted_metadata: &ThreadMetadata,
) {
    if has_model_resume_override(request_overrides.as_ref(), typesafe_overrides) {
        return;
    }

    typesafe_overrides.model = persisted_metadata.model.clone();

    if let Some(reasoning_effort) = persisted_metadata.reasoning_effort {
        request_overrides.get_or_insert_with(HashMap::new).insert(
            "model_reasoning_effort".to_string(),
            serde_json::Value::String(reasoning_effort.to_string()),
        );
    }
}

fn has_model_resume_override(
    request_overrides: Option<&HashMap<String, serde_json::Value>>,
    typesafe_overrides: &ConfigOverrides,
) -> bool {
    typesafe_overrides.model.is_some()
        || typesafe_overrides.model_provider.is_some()
        || request_overrides.is_some_and(|overrides| overrides.contains_key("model"))
        || request_overrides
            .is_some_and(|overrides| overrides.contains_key("model_reasoning_effort"))
}

fn skills_to_info(
    skills: &[codex_core::skills::SkillMetadata],
    disabled_paths: &std::collections::HashSet<PathBuf>,
) -> Vec<codex_app_server_protocol::SkillMetadata> {
    skills
        .iter()
        .map(|skill| {
            let enabled = !disabled_paths.contains(&skill.path_to_skills_md);
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
                path: skill.path_to_skills_md.clone(),
                scope: skill.scope.into(),
                enabled,
            }
        })
        .collect()
}

fn plugin_skills_to_info(
    skills: &[codex_core::skills::SkillMetadata],
    disabled_skill_paths: &std::collections::HashSet<PathBuf>,
) -> Vec<SkillSummary> {
    skills
        .iter()
        .map(|skill| SkillSummary {
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
            path: skill.path_to_skills_md.clone(),
            enabled: !disabled_skill_paths.contains(&skill.path_to_skills_md),
        })
        .collect()
}

fn plugin_interface_to_info(
    interface: codex_core::plugins::PluginManifestInterface,
) -> PluginInterface {
    PluginInterface {
        display_name: interface.display_name,
        short_description: interface.short_description,
        long_description: interface.long_description,
        developer_name: interface.developer_name,
        category: interface.category,
        capabilities: interface.capabilities,
        website_url: interface.website_url,
        privacy_policy_url: interface.privacy_policy_url,
        terms_of_service_url: interface.terms_of_service_url,
        default_prompt: interface.default_prompt,
        brand_color: interface.brand_color,
        composer_icon: interface.composer_icon,
        logo: interface.logo,
        screenshots: interface.screenshots,
    }
}

fn marketplace_plugin_source_to_info(source: MarketplacePluginSource) -> PluginSource {
    match source {
        MarketplacePluginSource::Local { path } => PluginSource::Local { path },
    }
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

fn cloud_requirements_load_error(err: &std::io::Error) -> Option<&CloudRequirementsLoadError> {
    let mut current: Option<&(dyn std::error::Error + 'static)> = err
        .get_ref()
        .map(|source| source as &(dyn std::error::Error + 'static));
    while let Some(source) = current {
        if let Some(cloud_error) = source.downcast_ref::<CloudRequirementsLoadError>() {
            return Some(cloud_error);
        }
        current = source.source();
    }
    None
}

fn config_load_error(err: &std::io::Error) -> JSONRPCErrorError {
    let data = cloud_requirements_load_error(err).map(|cloud_error| {
        let mut data = serde_json::json!({
            "reason": "cloudRequirements",
            "errorCode": format!("{:?}", cloud_error.code()),
            "detail": cloud_error.to_string(),
        });
        if let Some(status_code) = cloud_error.status_code() {
            data["statusCode"] = serde_json::json!(status_code);
        }
        if cloud_error.code() == CloudRequirementsLoadErrorCode::Auth {
            data["action"] = serde_json::json!("relogin");
        }
        data
    });

    JSONRPCErrorError {
        code: INVALID_REQUEST_ERROR_CODE,
        message: format!("failed to load configuration: {err}"),
        data,
    }
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

        if let Err(err) = codex_tools::parse_tool_input_schema(&tool.input_schema) {
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
    codex_home: &Path,
    runtime_feature_enablement: &BTreeMap<String, bool>,
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

    let mut config = codex_core::config::ConfigBuilder::default()
        .codex_home(codex_home.to_path_buf())
        .cli_overrides(merged_cli_overrides)
        .harness_overrides(typesafe_overrides)
        .cloud_requirements(cloud_requirements.clone())
        .build()
        .await?;
    apply_runtime_feature_enablement(&mut config, runtime_feature_enablement);
    Ok(config)
}

async fn derive_config_for_cwd(
    cli_overrides: &[(String, TomlValue)],
    request_overrides: Option<HashMap<String, serde_json::Value>>,
    typesafe_overrides: ConfigOverrides,
    cwd: Option<PathBuf>,
    cloud_requirements: &CloudRequirementsLoader,
    codex_home: &Path,
    runtime_feature_enablement: &BTreeMap<String, bool>,
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

    let mut config = codex_core::config::ConfigBuilder::default()
        .codex_home(codex_home.to_path_buf())
        .cli_overrides(merged_cli_overrides)
        .harness_overrides(typesafe_overrides)
        .fallback_cwd(
            cwd.map(AbsolutePathBuf::relative_to_current_dir)
                .transpose()
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?,
        )
        .cloud_requirements(cloud_requirements.clone())
        .build()
        .await?;
    apply_runtime_feature_enablement(&mut config, runtime_feature_enablement);
    Ok(config)
}

async fn read_history_cwd_from_state_db(
    config: &Config,
    thread_id: Option<ThreadId>,
    rollout_path: &Path,
) -> Option<PathBuf> {
    if let Some(state_db_ctx) = get_state_db(config).await
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
    let state_db_ctx = get_state_db(config).await;
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
    Some(summary_from_thread_metadata(&metadata))
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
        let source = with_thread_spawn_agent_metadata(
            it.source
                .unwrap_or(codex_protocol::protocol::SessionSource::Unknown),
            it.agent_nickname.clone(),
            it.agent_role.clone(),
        );
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
    agent_nickname: Option<String>,
    agent_role: Option<String>,
    git_sha: Option<String>,
    git_branch: Option<String>,
    git_origin_url: Option<String>,
) -> ConversationSummary {
    let preview = first_user_message.unwrap_or_default();
    let source = serde_json::from_str(&source)
        .or_else(|_| serde_json::from_value(serde_json::Value::String(source.clone())))
        .unwrap_or(codex_protocol::protocol::SessionSource::Unknown);
    let source = with_thread_spawn_agent_metadata(source, agent_nickname, agent_role);
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

fn summary_from_thread_metadata(metadata: &ThreadMetadata) -> ConversationSummary {
    summary_from_state_db_metadata(
        metadata.id,
        metadata.rollout_path.clone(),
        metadata.first_user_message.clone(),
        metadata
            .created_at
            .to_rfc3339_opts(SecondsFormat::Secs, true),
        metadata
            .updated_at
            .to_rfc3339_opts(SecondsFormat::Secs, true),
        metadata.model_provider.clone(),
        metadata.cwd.clone(),
        metadata.cli_version.clone(),
        metadata.source.clone(),
        metadata.agent_nickname.clone(),
        metadata.agent_role.clone(),
        metadata.git_sha.clone(),
        metadata.git_branch.clone(),
        metadata.git_origin_url.clone(),
    )
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
    let mut session_meta = session_meta;
    session_meta.source = with_thread_spawn_agent_metadata(
        session_meta.source.clone(),
        session_meta.agent_nickname.clone(),
        session_meta.agent_role.clone(),
    );

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
        sha: git_info.commit_hash.as_ref().map(|sha| sha.0.clone()),
        branch: git_info.branch.clone(),
        origin_url: git_info.repository_url.clone(),
    }
}

async fn load_thread_summary_for_rollout(
    config: &Config,
    thread_id: ThreadId,
    rollout_path: &Path,
    fallback_provider: &str,
    persisted_metadata: Option<&ThreadMetadata>,
) -> std::result::Result<Thread, String> {
    let mut thread = read_summary_from_rollout(rollout_path, fallback_provider)
        .await
        .map(summary_to_thread)
        .map_err(|err| {
            format!(
                "failed to load rollout `{}` for thread {thread_id}: {err}",
                rollout_path.display()
            )
        })?;
    thread.forked_from_id = forked_from_id_from_rollout(rollout_path).await;
    if let Some(persisted_metadata) = persisted_metadata {
        merge_mutable_thread_metadata(
            &mut thread,
            summary_to_thread(summary_from_thread_metadata(persisted_metadata)),
        );
    } else if let Some(summary) = read_summary_from_state_db_by_thread_id(config, thread_id).await {
        merge_mutable_thread_metadata(&mut thread, summary_to_thread(summary));
    }
    Ok(thread)
}

async fn forked_from_id_from_rollout(path: &Path) -> Option<String> {
    read_session_meta_line(path)
        .await
        .ok()
        .and_then(|meta_line| meta_line.meta.forked_from_id)
        .map(|thread_id| thread_id.to_string())
}

fn merge_mutable_thread_metadata(thread: &mut Thread, persisted_thread: Thread) {
    thread.git_info = persisted_thread.git_info;
}

fn preview_from_rollout_items(items: &[RolloutItem]) -> String {
    items
        .iter()
        .find_map(|item| match item {
            RolloutItem::ResponseItem(item) => match codex_core::parse_turn_item(item) {
                Some(codex_protocol::items::TurnItem::UserMessage(user)) => Some(user.message()),
                _ => None,
            },
            _ => None,
        })
        .map(|preview| match preview.find(USER_MESSAGE_BEGIN) {
            Some(idx) => preview[idx + USER_MESSAGE_BEGIN.len()..].trim().to_string(),
            None => preview,
        })
        .unwrap_or_default()
}

fn with_thread_spawn_agent_metadata(
    source: codex_protocol::protocol::SessionSource,
    agent_nickname: Option<String>,
    agent_role: Option<String>,
) -> codex_protocol::protocol::SessionSource {
    if agent_nickname.is_none() && agent_role.is_none() {
        return source;
    }

    match source {
        codex_protocol::protocol::SessionSource::SubAgent(
            codex_protocol::protocol::SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth,
                agent_path,
                agent_nickname: existing_agent_nickname,
                agent_role: existing_agent_role,
            },
        ) => codex_protocol::protocol::SessionSource::SubAgent(
            codex_protocol::protocol::SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth,
                agent_path,
                agent_nickname: agent_nickname.or(existing_agent_nickname),
                agent_role: agent_role.or(existing_agent_role),
            },
        ),
        _ => source,
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
        forked_from_id: None,
        preview: String::new(),
        ephemeral: config_snapshot.ephemeral,
        model_provider: config_snapshot.model_provider_id.clone(),
        created_at: now,
        updated_at: now,
        status: ThreadStatus::NotLoaded,
        path,
        cwd: config_snapshot.cwd.to_path_buf(),
        cli_version: env!("CARGO_PKG_VERSION").to_string(),
        agent_nickname: config_snapshot.session_source.get_nickname(),
        agent_role: config_snapshot.session_source.get_agent_role(),
        source: config_snapshot.session_source.clone().into(),
        git_info: None,
        name: None,
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
        forked_from_id: None,
        preview,
        ephemeral: false,
        model_provider,
        created_at: created_at.map(|dt| dt.timestamp()).unwrap_or(0),
        updated_at: updated_at.map(|dt| dt.timestamp()).unwrap_or(0),
        status: ThreadStatus::NotLoaded,
        path: Some(path),
        cwd,
        cli_version,
        agent_nickname: source.get_nickname(),
        agent_role: source.get_agent_role(),
        source: source.into(),
        git_info,
        name: None,
        turns: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outgoing_message::OutgoingEnvelope;
    use crate::outgoing_message::OutgoingMessage;
    use anyhow::Result;
    use codex_app_server_protocol::ServerRequestPayload;
    use codex_app_server_protocol::ToolRequestUserInputParams;
    use codex_protocol::openai_models::ReasoningEffort;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::SubAgentSource;
    use core_test_support::test_absolute_path;
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
            defer_loading: false,
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
            defer_loading: false,
        }];
        validate_dynamic_tools(&tools).expect("valid schema");
    }

    #[test]
    fn config_load_error_marks_cloud_requirements_failures_for_relogin() {
        let err = std::io::Error::other(CloudRequirementsLoadError::new(
            CloudRequirementsLoadErrorCode::Auth,
            Some(401),
            "Your authentication session could not be refreshed automatically. Please log out and sign in again.",
        ));

        let error = config_load_error(&err);

        assert_eq!(
            error.data,
            Some(json!({
                "reason": "cloudRequirements",
                "errorCode": "Auth",
                "action": "relogin",
                "statusCode": 401,
                "detail": "Your authentication session could not be refreshed automatically. Please log out and sign in again.",
            }))
        );
        assert!(
            error.message.contains("failed to load configuration"),
            "unexpected error message: {}",
            error.message
        );
    }

    #[test]
    fn config_load_error_leaves_non_cloud_requirements_failures_unmarked() {
        let err = std::io::Error::other("required MCP servers failed to initialize");

        let error = config_load_error(&err);

        assert_eq!(error.data, None);
        assert!(
            error.message.contains("failed to load configuration"),
            "unexpected error message: {}",
            error.message
        );
    }

    #[test]
    fn config_load_error_marks_non_auth_cloud_requirements_failures_without_relogin() {
        let err = std::io::Error::other(CloudRequirementsLoadError::new(
            CloudRequirementsLoadErrorCode::RequestFailed,
            /*status_code*/ None,
            "failed to load your workspace-managed config",
        ));

        let error = config_load_error(&err);

        assert_eq!(
            error.data,
            Some(json!({
                "reason": "cloudRequirements",
                "errorCode": "RequestFailed",
                "detail": "failed to load your workspace-managed config",
            }))
        );
    }

    #[test]
    fn collect_resume_override_mismatches_includes_service_tier() {
        let request = ThreadResumeParams {
            thread_id: "thread-1".to_string(),
            history: None,
            path: None,
            model: None,
            model_provider: None,
            service_tier: Some(Some(codex_protocol::config_types::ServiceTier::Fast)),
            cwd: None,
            approval_policy: None,
            approvals_reviewer: None,
            sandbox: None,
            config: None,
            base_instructions: None,
            developer_instructions: None,
            personality: None,
            persist_extended_history: false,
        };
        let config_snapshot = ThreadConfigSnapshot {
            model: "gpt-5".to_string(),
            model_provider_id: "openai".to_string(),
            service_tier: Some(codex_protocol::config_types::ServiceTier::Flex),
            approval_policy: codex_protocol::protocol::AskForApproval::OnRequest,
            approvals_reviewer: codex_protocol::config_types::ApprovalsReviewer::User,
            sandbox_policy: codex_protocol::protocol::SandboxPolicy::DangerFullAccess,
            cwd: test_absolute_path("/tmp"),
            ephemeral: false,
            reasoning_effort: None,
            personality: None,
            session_source: SessionSource::Cli,
        };

        assert_eq!(
            collect_resume_override_mismatches(&request, &config_snapshot),
            vec!["service_tier requested=Some(Fast) active=Some(Flex)".to_string()]
        );
    }

    fn test_thread_metadata(
        model: Option<&str>,
        reasoning_effort: Option<ReasoningEffort>,
    ) -> Result<ThreadMetadata> {
        let thread_id = ThreadId::from_string("3f941c35-29b3-493b-b0a4-e25800d9aeb0")?;
        let mut builder = ThreadMetadataBuilder::new(
            thread_id,
            PathBuf::from("/tmp/rollout.jsonl"),
            Utc::now(),
            codex_protocol::protocol::SessionSource::default(),
        );
        builder.model_provider = Some("mock_provider".to_string());
        let mut metadata = builder.build("mock_provider");
        metadata.model = model.map(ToString::to_string);
        metadata.reasoning_effort = reasoning_effort;
        Ok(metadata)
    }

    #[test]
    fn merge_persisted_resume_metadata_prefers_persisted_model_and_reasoning_effort() -> Result<()>
    {
        let mut request_overrides = None;
        let mut typesafe_overrides = ConfigOverrides::default();
        let persisted_metadata =
            test_thread_metadata(Some("gpt-5.1-codex-max"), Some(ReasoningEffort::High))?;

        merge_persisted_resume_metadata(
            &mut request_overrides,
            &mut typesafe_overrides,
            &persisted_metadata,
        );

        assert_eq!(
            typesafe_overrides.model,
            Some("gpt-5.1-codex-max".to_string())
        );
        assert_eq!(
            request_overrides,
            Some(HashMap::from([(
                "model_reasoning_effort".to_string(),
                serde_json::Value::String("high".to_string()),
            )]))
        );
        Ok(())
    }

    #[test]
    fn merge_persisted_resume_metadata_preserves_explicit_overrides() -> Result<()> {
        let mut request_overrides = Some(HashMap::from([(
            "model_reasoning_effort".to_string(),
            serde_json::Value::String("low".to_string()),
        )]));
        let mut typesafe_overrides = ConfigOverrides {
            model: Some("gpt-5.2-codex".to_string()),
            ..Default::default()
        };
        let persisted_metadata =
            test_thread_metadata(Some("gpt-5.1-codex-max"), Some(ReasoningEffort::High))?;

        merge_persisted_resume_metadata(
            &mut request_overrides,
            &mut typesafe_overrides,
            &persisted_metadata,
        );

        assert_eq!(typesafe_overrides.model, Some("gpt-5.2-codex".to_string()));
        assert_eq!(
            request_overrides,
            Some(HashMap::from([(
                "model_reasoning_effort".to_string(),
                serde_json::Value::String("low".to_string()),
            )]))
        );
        Ok(())
    }

    #[test]
    fn merge_persisted_resume_metadata_skips_persisted_values_when_model_overridden() -> Result<()>
    {
        let mut request_overrides = Some(HashMap::from([(
            "model".to_string(),
            serde_json::Value::String("gpt-5.2-codex".to_string()),
        )]));
        let mut typesafe_overrides = ConfigOverrides::default();
        let persisted_metadata =
            test_thread_metadata(Some("gpt-5.1-codex-max"), Some(ReasoningEffort::High))?;

        merge_persisted_resume_metadata(
            &mut request_overrides,
            &mut typesafe_overrides,
            &persisted_metadata,
        );

        assert_eq!(typesafe_overrides.model, None);
        assert_eq!(
            request_overrides,
            Some(HashMap::from([(
                "model".to_string(),
                serde_json::Value::String("gpt-5.2-codex".to_string()),
            )]))
        );
        Ok(())
    }

    #[test]
    fn merge_persisted_resume_metadata_skips_persisted_values_when_provider_overridden()
    -> Result<()> {
        let mut request_overrides = None;
        let mut typesafe_overrides = ConfigOverrides {
            model_provider: Some("oss".to_string()),
            ..Default::default()
        };
        let persisted_metadata =
            test_thread_metadata(Some("gpt-5.1-codex-max"), Some(ReasoningEffort::High))?;

        merge_persisted_resume_metadata(
            &mut request_overrides,
            &mut typesafe_overrides,
            &persisted_metadata,
        );

        assert_eq!(typesafe_overrides.model, None);
        assert_eq!(typesafe_overrides.model_provider, Some("oss".to_string()));
        assert_eq!(request_overrides, None);
        Ok(())
    }

    #[test]
    fn merge_persisted_resume_metadata_skips_persisted_values_when_reasoning_effort_overridden()
    -> Result<()> {
        let mut request_overrides = Some(HashMap::from([(
            "model_reasoning_effort".to_string(),
            serde_json::Value::String("low".to_string()),
        )]));
        let mut typesafe_overrides = ConfigOverrides::default();
        let persisted_metadata =
            test_thread_metadata(Some("gpt-5.1-codex-max"), Some(ReasoningEffort::High))?;

        merge_persisted_resume_metadata(
            &mut request_overrides,
            &mut typesafe_overrides,
            &persisted_metadata,
        );

        assert_eq!(typesafe_overrides.model, None);
        assert_eq!(
            request_overrides,
            Some(HashMap::from([(
                "model_reasoning_effort".to_string(),
                serde_json::Value::String("low".to_string()),
            )]))
        );
        Ok(())
    }

    #[test]
    fn merge_persisted_resume_metadata_skips_missing_values() -> Result<()> {
        let mut request_overrides = None;
        let mut typesafe_overrides = ConfigOverrides::default();
        let persisted_metadata =
            test_thread_metadata(/*model*/ None, /*reasoning_effort*/ None)?;

        merge_persisted_resume_metadata(
            &mut request_overrides,
            &mut typesafe_overrides,
            &persisted_metadata,
        );

        assert_eq!(typesafe_overrides.model, None);
        assert_eq!(request_overrides, None);
        Ok(())
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
                    "text": "# AGENTS.md instructions for project\n\n<INSTRUCTIONS>\n<AGENTS.md contents>\n</INSTRUCTIONS>".to_string(),
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
            /*git*/ None,
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
    async fn read_summary_from_rollout_preserves_agent_nickname() -> Result<()> {
        use codex_protocol::protocol::RolloutItem;
        use codex_protocol::protocol::RolloutLine;
        use codex_protocol::protocol::SessionMetaLine;
        use std::fs;

        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().join("rollout.jsonl");

        let conversation_id = ThreadId::from_string("bfd12a78-5900-467b-9bc5-d3d35df08191")?;
        let parent_thread_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let timestamp = "2025-09-05T16:53:11.850Z".to_string();

        let session_meta = SessionMeta {
            id: conversation_id,
            timestamp: timestamp.clone(),
            source: SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            }),
            agent_nickname: Some("atlas".to_string()),
            agent_role: Some("explorer".to_string()),
            model_provider: Some("test-provider".to_string()),
            ..SessionMeta::default()
        };

        let line = RolloutLine {
            timestamp,
            item: RolloutItem::SessionMeta(SessionMetaLine {
                meta: session_meta,
                git: None,
            }),
        };
        fs::write(&path, format!("{}\n", serde_json::to_string(&line)?))?;

        let summary = read_summary_from_rollout(path.as_path(), "fallback").await?;
        let thread = summary_to_thread(summary);

        assert_eq!(thread.agent_nickname, Some("atlas".to_string()));
        assert_eq!(thread.agent_role, Some("explorer".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn read_summary_from_rollout_preserves_forked_from_id() -> Result<()> {
        use codex_protocol::protocol::RolloutItem;
        use codex_protocol::protocol::RolloutLine;
        use codex_protocol::protocol::SessionMetaLine;
        use std::fs;

        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().join("rollout.jsonl");

        let conversation_id = ThreadId::from_string("bfd12a78-5900-467b-9bc5-d3d35df08191")?;
        let forked_from_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let timestamp = "2025-09-05T16:53:11.850Z".to_string();

        let session_meta = SessionMeta {
            id: conversation_id,
            forked_from_id: Some(forked_from_id),
            timestamp: timestamp.clone(),
            model_provider: Some("test-provider".to_string()),
            ..SessionMeta::default()
        };

        let line = RolloutLine {
            timestamp,
            item: RolloutItem::SessionMeta(SessionMetaLine {
                meta: session_meta,
                git: None,
            }),
        };
        fs::write(&path, format!("{}\n", serde_json::to_string(&line)?))?;

        assert_eq!(
            forked_from_id_from_rollout(path.as_path()).await,
            Some(forked_from_id.to_string())
        );
        Ok(())
    }

    #[tokio::test]
    async fn aborting_pending_request_clears_pending_state() -> Result<()> {
        let thread_id = ThreadId::from_string("bfd12a78-5900-467b-9bc5-d3d35df08191")?;
        let connection_id = ConnectionId(7);

        let (outgoing_tx, mut outgoing_rx) = tokio::sync::mpsc::channel(8);
        let outgoing = Arc::new(OutgoingMessageSender::new(outgoing_tx));
        let thread_outgoing = ThreadScopedOutgoingMessageSender::new(
            outgoing.clone(),
            vec![connection_id],
            thread_id,
        );

        let (request_id, client_request_rx) = thread_outgoing
            .send_request(ServerRequestPayload::ToolRequestUserInput(
                ToolRequestUserInputParams {
                    thread_id: thread_id.to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "call-1".to_string(),
                    questions: vec![],
                },
            ))
            .await;
        thread_outgoing.abort_pending_server_requests().await;

        let request_message = outgoing_rx.recv().await.expect("request should be sent");
        let OutgoingEnvelope::ToConnection {
            connection_id: request_connection_id,
            message:
                OutgoingMessage::Request(ServerRequest::ToolRequestUserInput {
                    request_id: sent_request_id,
                    ..
                }),
            ..
        } = request_message
        else {
            panic!("expected tool request to be sent to the subscribed connection");
        };
        assert_eq!(request_connection_id, connection_id);
        assert_eq!(sent_request_id, request_id);

        let response = client_request_rx
            .await
            .expect("callback should be resolved");
        let error = response.expect_err("request should be aborted during cleanup");
        assert_eq!(
            error.message,
            "client request resolved because the turn state was changed"
        );
        assert_eq!(error.data, Some(json!({ "reason": "turnTransition" })));
        assert!(
            outgoing
                .pending_requests_for_thread(thread_id)
                .await
                .is_empty()
        );
        assert!(outgoing_rx.try_recv().is_err());
        Ok(())
    }

    #[test]
    fn summary_from_state_db_metadata_preserves_agent_nickname() -> Result<()> {
        let conversation_id = ThreadId::from_string("bfd12a78-5900-467b-9bc5-d3d35df08191")?;
        let source =
            serde_json::to_string(&SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?,
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            }))?;

        let summary = summary_from_state_db_metadata(
            conversation_id,
            PathBuf::from("/tmp/rollout.jsonl"),
            Some("hi".to_string()),
            "2025-09-05T16:53:11Z".to_string(),
            "2025-09-05T16:53:12Z".to_string(),
            "test-provider".to_string(),
            PathBuf::from("/"),
            "0.0.0".to_string(),
            source,
            Some("atlas".to_string()),
            Some("explorer".to_string()),
            /*git_sha*/ None,
            /*git_branch*/ None,
            /*git_origin_url*/ None,
        );

        let thread = summary_to_thread(summary);

        assert_eq!(thread.agent_nickname, Some("atlas".to_string()));
        assert_eq!(thread.agent_role, Some("explorer".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn removing_thread_state_clears_listener_and_active_turn_history() -> Result<()> {
        let manager = ThreadStateManager::new();
        let thread_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let connection = ConnectionId(1);
        let (cancel_tx, cancel_rx) = oneshot::channel();

        manager.connection_initialized(connection).await;
        manager
            .try_ensure_connection_subscribed(
                thread_id, connection, /*experimental_raw_events*/ false,
            )
            .await
            .expect("connection should be live");
        {
            let state = manager.thread_state(thread_id).await;
            let mut state = state.lock().await;
            state.cancel_tx = Some(cancel_tx);
            state.track_current_turn_event(&EventMsg::TurnStarted(
                codex_protocol::protocol::TurnStartedEvent {
                    turn_id: "turn-1".to_string(),
                    started_at: None,
                    model_context_window: None,
                    collaboration_mode_kind: Default::default(),
                },
            ));
        }

        manager.remove_thread_state(thread_id).await;
        assert_eq!(cancel_rx.await, Ok(()));

        let state = manager.thread_state(thread_id).await;
        let state = state.lock().await;
        assert!(
            manager
                .subscribed_connection_ids(thread_id)
                .await
                .is_empty()
        );
        assert!(state.cancel_tx.is_none());
        assert!(state.active_turn_snapshot().is_none());
        Ok(())
    }

    #[tokio::test]
    async fn removing_auto_attached_connection_preserves_listener_for_other_connections()
    -> Result<()> {
        let manager = ThreadStateManager::new();
        let thread_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let connection_a = ConnectionId(1);
        let connection_b = ConnectionId(2);
        let (cancel_tx, mut cancel_rx) = oneshot::channel();

        manager.connection_initialized(connection_a).await;
        manager.connection_initialized(connection_b).await;
        manager
            .try_ensure_connection_subscribed(
                thread_id,
                connection_a,
                /*experimental_raw_events*/ false,
            )
            .await
            .expect("connection_a should be live");
        manager
            .try_ensure_connection_subscribed(
                thread_id,
                connection_b,
                /*experimental_raw_events*/ false,
            )
            .await
            .expect("connection_b should be live");
        {
            let state = manager.thread_state(thread_id).await;
            state.lock().await.cancel_tx = Some(cancel_tx);
        }

        manager.remove_connection(connection_a).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut cancel_rx)
                .await
                .is_err()
        );

        assert_eq!(
            manager.subscribed_connection_ids(thread_id).await,
            vec![connection_b]
        );
        Ok(())
    }

    #[tokio::test]
    async fn closed_connection_cannot_be_reintroduced_by_auto_subscribe() -> Result<()> {
        let manager = ThreadStateManager::new();
        let thread_id = ThreadId::from_string("ad7f0408-99b8-4f6e-a46f-bd0eec433370")?;
        let connection = ConnectionId(1);

        manager.connection_initialized(connection).await;
        manager.remove_connection(connection).await;

        assert!(
            manager
                .try_ensure_connection_subscribed(
                    thread_id, connection, /*experimental_raw_events*/ false
                )
                .await
                .is_none()
        );
        assert!(!manager.has_subscribers(thread_id).await);
        Ok(())
    }
}
