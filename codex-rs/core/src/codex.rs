use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::agent::AgentControl;
use crate::agent::AgentStatus;
use crate::agent::Mailbox;
use crate::agent::MailboxReceiver;
use crate::agent::agent_status_from_event;
use crate::agent::status::is_final;
use crate::agent_identity::AgentIdentityManager;
use crate::apps::render_apps_section;
use crate::commit_attribution::commit_message_trailer_instruction;
use crate::compact;
use crate::compact::InitialContextInjection;
use crate::compact::run_inline_auto_compact_task;
use crate::compact::should_use_remote_compact_task;
use crate::compact_remote::run_inline_remote_auto_compact_task;
use crate::config::ManagedFeatures;
use crate::connectors;
use crate::exec_policy::ExecPolicyManager;
use crate::installation_id::resolve_installation_id;
use crate::mcp_tool_exposure::build_mcp_tool_exposure;
use crate::parse_turn_item;
use crate::path_utils::normalize_for_native_workdir;
use crate::realtime_conversation::RealtimeConversationManager;
use crate::realtime_conversation::handle_audio as handle_realtime_conversation_audio;
use crate::realtime_conversation::handle_close as handle_realtime_conversation_close;
use crate::realtime_conversation::handle_start as handle_realtime_conversation_start;
use crate::realtime_conversation::handle_text as handle_realtime_conversation_text;
use crate::render_skills_section;
use crate::rollout::find_thread_name_by_id;
use crate::session_prefix::format_subagent_notification_message;
use crate::skills_load_input_from_config;
use crate::stream_events_utils::HandleOutputCtx;
use crate::stream_events_utils::handle_non_tool_response_item;
use crate::stream_events_utils::handle_output_item_done;
use crate::stream_events_utils::last_assistant_message_from_item;
use crate::stream_events_utils::raw_assistant_output_text_from_item;
use crate::stream_events_utils::record_completed_response_item;
use crate::turn_metadata::TurnMetadataState;
use crate::unavailable_tool::collect_unavailable_called_tools;
use crate::util::error_or_panic;
use async_channel::Receiver;
use async_channel::Sender;
use chrono::Local;
use chrono::Utc;
use codex_analytics::AnalyticsEventsClient;
use codex_analytics::AppInvocation;
use codex_analytics::CompactionPhase;
use codex_analytics::CompactionReason;
use codex_analytics::InvocationType;
use codex_analytics::SubAgentThreadStartedInput;
use codex_analytics::TurnResolvedConfigFact;
use codex_analytics::build_track_events_context;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::FileSystemSandboxContext;
use codex_features::FEATURES;
use codex_features::Feature;
use codex_features::unstable_features_warning_event;
use codex_hooks::HookEvent;
use codex_hooks::HookEventAfterAgent;
use codex_hooks::HookPayload;
use codex_hooks::HookResult;
use codex_hooks::Hooks;
use codex_hooks::HooksConfig;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::auth_env_telemetry::collect_auth_env_telemetry;
use codex_login::default_client::originator;
use codex_mcp::McpConnectionManager;
use codex_mcp::ToolInfo;
use codex_mcp::codex_apps_tools_cache_key;
#[cfg(test)]
use codex_models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_models_manager::manager::ModelsManager;
use codex_models_manager::manager::RefreshStrategy;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::NetworkProxyAuditMetadata;
use codex_network_proxy::normalize_host;
use codex_otel::current_span_trace_id;
use codex_otel::current_span_w3c_trace_context;
use codex_otel::set_parent_from_w3c_trace_context;
use codex_protocol::ThreadId;
use codex_protocol::ToolName;
use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::approvals::ExecPolicyAmendment;
use codex_protocol::approvals::NetworkPolicyAmendment;
use codex_protocol::approvals::NetworkPolicyRuleAction;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::items::PlanItem;
use codex_protocol::items::TurnItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::items::build_hook_prompt_message;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::format_allow_prefixes;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::HasLegacyEvent;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ItemStartedEvent;
use codex_protocol::protocol::RawResponseItemEvent;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::TurnContextNetworkItem;
use codex_protocol::protocol::W3cTraceContext;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsArgs;
use codex_protocol::request_permissions::RequestPermissionsEvent;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_rmcp_client::ElicitationResponse;
use codex_rollout::RolloutConfig;
use codex_rollout::state_db;
use codex_shell_command::parse_command::parse_command;
use codex_terminal_detection::user_agent;
use codex_thread_store::LocalThreadStore;
use codex_tools::filter_tool_suggest_discoverable_tools_for_client;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_stream_parser::AssistantTextChunk;
use codex_utils_stream_parser::AssistantTextStreamParser;
use codex_utils_stream_parser::ProposedPlanSegment;
use codex_utils_stream_parser::extract_proposed_plan_text;
use codex_utils_stream_parser::strip_citations;
use futures::future::BoxFuture;
use futures::future::Shared;
use futures::prelude::*;
use futures::stream::FuturesOrdered;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::RequestId;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use toml::Value as TomlValue;
use tracing::Instrument;
use tracing::debug;
use tracing::debug_span;
use tracing::error;
use tracing::field;
use tracing::info;
use tracing::info_span;
use tracing::instrument;
use tracing::trace;
use tracing::trace_span;
use tracing::warn;
use uuid::Uuid;

use crate::client::ModelClient;
use crate::client::ModelClientSession;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::codex_thread::ThreadConfigSnapshot;
use crate::compact::collect_user_messages;
use crate::config::Config;
use crate::config::Constrained;
use crate::config::ConstraintResult;
use crate::config::GhostSnapshotConfig;
use crate::config::StartedNetworkProxy;
use crate::config::resolve_web_search_mode_for_turn;
use crate::context_manager::ContextManager;
use crate::context_manager::TotalTokenUsageBreakdown;
use crate::environment_context::EnvironmentContext;
use crate::thread_rollout_truncation::initial_history_has_prior_user_turns;
use codex_config::CONFIG_TOML_FILE;
use codex_config::types::McpServerConfig;
use codex_config::types::ShellEnvironmentPolicy;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
#[cfg(test)]
use codex_protocol::exec_output::StreamOutput;

mod rollout_reconstruction;
#[cfg(test)]
mod rollout_reconstruction_tests;

#[derive(Debug, PartialEq)]
pub enum SteerInputError {
    NoActiveTurn(Vec<UserInput>),
    ExpectedTurnMismatch { expected: String, actual: String },
    ActiveTurnNotSteerable { turn_kind: NonSteerableTurnKind },
    EmptyInput,
}

impl SteerInputError {
    fn to_error_event(&self) -> ErrorEvent {
        match self {
            Self::NoActiveTurn(_) => ErrorEvent {
                message: "no active turn to steer".to_string(),
                codex_error_info: Some(CodexErrorInfo::BadRequest),
            },
            Self::ExpectedTurnMismatch { expected, actual } => ErrorEvent {
                message: format!("expected active turn id `{expected}` but found `{actual}`"),
                codex_error_info: Some(CodexErrorInfo::BadRequest),
            },
            Self::ActiveTurnNotSteerable { turn_kind } => {
                let turn_kind_label = match turn_kind {
                    NonSteerableTurnKind::Review => "review",
                    NonSteerableTurnKind::Compact => "compact",
                };
                ErrorEvent {
                    message: format!("cannot steer a {turn_kind_label} turn"),
                    codex_error_info: Some(CodexErrorInfo::ActiveTurnNotSteerable {
                        turn_kind: *turn_kind,
                    }),
                }
            }
            Self::EmptyInput => ErrorEvent {
                message: "input must not be empty".to_string(),
                codex_error_info: Some(CodexErrorInfo::BadRequest),
            },
        }
    }
}

/// Notes from the previous real user turn.
///
/// Conceptually this is the same role that `previous_model` used to fill, but
/// it can carry other prior-turn settings that matter when constructing
/// sensible state-change diffs or full-context reinjection, such as model
/// switches or detecting a prior `realtime_active -> false` transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreviousTurnSettings {
    pub(crate) model: String,
    pub(crate) realtime_active: Option<bool>,
}

use crate::SkillError;
use crate::SkillInjections;
use crate::SkillLoadOutcome;
use crate::SkillMetadata;
use crate::SkillsManager;
use crate::build_skill_injections;
use crate::collect_env_var_dependencies;
use crate::collect_explicit_skill_mentions;
use crate::exec_policy::ExecPolicyUpdateError;
use crate::feedback_tags;
use crate::guardian::GuardianReviewSessionManager;
use crate::hook_runtime::PendingInputHookDisposition;
use crate::hook_runtime::inspect_pending_input;
use crate::hook_runtime::record_additional_contexts;
use crate::hook_runtime::record_pending_input;
use crate::hook_runtime::run_pending_session_start_hooks;
use crate::hook_runtime::run_user_prompt_submit_hooks;
use crate::injection::ToolMentionKind;
use crate::injection::app_id_from_path;
use crate::injection::tool_kind_for_path;
use crate::instructions::UserInstructions;
use crate::mcp::McpManager;
use crate::mcp_skill_dependencies::maybe_prompt_and_install_mcp_dependencies;
use crate::memories;
use crate::mentions::build_connector_slug_counts;
use crate::mentions::build_skill_name_counts;
use crate::mentions::collect_explicit_app_ids;
use crate::mentions::collect_explicit_plugin_mentions;
use crate::mentions::collect_tool_mentions_from_messages;
use crate::network_policy_decision::execpolicy_network_rule_amendment;
use crate::plugins::PluginsManager;
use crate::plugins::build_plugin_injections;
use crate::plugins::render_plugins_section;
use crate::project_doc::get_user_instructions;
use crate::resolve_skill_dependencies_for_turn;
use crate::rollout::RolloutRecorder;
use crate::rollout::RolloutRecorderParams;
use crate::rollout::map_session_init_error;
use crate::rollout::metadata;
use crate::rollout::policy::EventPersistenceMode;
use crate::session_startup_prewarm::SessionStartupPrewarmHandle;
use crate::shell;
use crate::shell_snapshot::ShellSnapshot;
use crate::skills_watcher::SkillsWatcher;
use crate::skills_watcher::SkillsWatcherEvent;
use crate::state::ActiveTurn;
use crate::state::MailboxDeliveryPhase;
use crate::state::SessionServices;
use crate::state::SessionState;
use crate::tasks::GhostSnapshotTask;
use crate::tasks::ReviewTask;
use crate::tasks::SessionTask;
use crate::tasks::SessionTaskContext;
use crate::tools::ToolRouter;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::js_repl::JsReplHandle;
use crate::tools::js_repl::resolve_compatible_node;
use crate::tools::network_approval::NetworkApprovalService;
use crate::tools::network_approval::build_blocked_request_observer;
use crate::tools::network_approval::build_network_policy_decider;
use crate::tools::parallel::ToolCallRuntime;
use crate::tools::router::ToolRouterParams;
use crate::tools::sandboxing::ApprovalStore;
use crate::turn_diff_tracker::TurnDiffTracker;
use crate::turn_timing::TurnTimingState;
use crate::turn_timing::record_turn_ttfm_metric;
use crate::turn_timing::record_turn_ttft_metric;
use crate::unified_exec::UnifiedExecProcessManager;
use crate::util::backoff;
use crate::windows_sandbox::WindowsSandboxLevelExt;
use codex_async_utils::OrCancelExt;
use codex_git_utils::get_git_repo_root;
use codex_mcp::compute_auth_statuses;
use codex_mcp::with_codex_apps_mcp;
use codex_otel::SessionTelemetry;
use codex_otel::THREAD_STARTED_METRIC;
use codex_otel::TelemetryAuthMode;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::ContentItem;
use codex_protocol::models::DeveloperInstructions;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::AgentMessageContentDeltaEvent;
use codex_protocol::protocol::AgentReasoningSectionBreakEvent;
use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::BackgroundEventEvent;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::CompactedItem;
use codex_protocol::protocol::DeprecationNoticeEvent;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::McpServerRefreshConfig;
use codex_protocol::protocol::ModelRerouteEvent;
use codex_protocol::protocol::ModelRerouteReason;
use codex_protocol::protocol::NetworkApprovalContext;
use codex_protocol::protocol::NonSteerableTurnKind;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::PlanDeltaEvent;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::ReasoningContentDeltaEvent;
use codex_protocol::protocol::ReasoningRawContentDeltaEvent;
use codex_protocol::protocol::RequestUserInputEvent;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_protocol::protocol::SessionNetworkProxyRuntime;
use codex_protocol::protocol::SkillDependencies as ProtocolSkillDependencies;
use codex_protocol::protocol::SkillErrorInfo;
use codex_protocol::protocol::SkillInterface as ProtocolSkillInterface;
use codex_protocol::protocol::SkillMetadata as ProtocolSkillMetadata;
use codex_protocol::protocol::SkillToolDependency as ProtocolSkillToolDependency;
use codex_protocol::protocol::StreamErrorEvent;
use codex_protocol::protocol::Submission;
use codex_protocol::protocol::TokenCountEvent;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TokenUsageInfo;
use codex_protocol::protocol::TurnDiffEvent;
use codex_protocol::protocol::WarningEvent;
use codex_protocol::user_input::UserInput;
use codex_tools::ToolsConfig;
use codex_tools::ToolsConfigParams;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_readiness::Readiness;
use codex_utils_readiness::ReadinessFlag;

fn image_generation_tool_auth_allowed(auth_manager: Option<&AuthManager>) -> bool {
    matches!(
        auth_manager.and_then(AuthManager::auth_mode),
        Some(AuthMode::Chatgpt)
    )
}

/// The high-level interface to the Codex system.
/// It operates as a queue pair where you send submissions and receive events.
pub struct Codex {
    pub(crate) tx_sub: Sender<Submission>,
    pub(crate) rx_event: Receiver<Event>,
    // Last known status of the agent.
    pub(crate) agent_status: watch::Receiver<AgentStatus>,
    pub(crate) session: Arc<Session>,
    // Shared future for the background submission loop completion so multiple
    // callers can wait for shutdown.
    pub(crate) session_loop_termination: SessionLoopTermination,
}

pub(crate) type SessionLoopTermination = Shared<BoxFuture<'static, ()>>;

/// Wrapper returned by [`Codex::spawn`] containing the spawned [`Codex`],
/// the submission id for the initial `ConfigureSession` request and the
/// unique session id.
pub struct CodexSpawnOk {
    pub codex: Codex,
    pub thread_id: ThreadId,
}

pub(crate) struct CodexSpawnArgs {
    pub(crate) config: Config,
    pub(crate) auth_manager: Arc<AuthManager>,
    pub(crate) models_manager: Arc<ModelsManager>,
    pub(crate) environment_manager: Arc<EnvironmentManager>,
    pub(crate) skills_manager: Arc<SkillsManager>,
    pub(crate) plugins_manager: Arc<PluginsManager>,
    pub(crate) mcp_manager: Arc<McpManager>,
    pub(crate) skills_watcher: Arc<SkillsWatcher>,
    pub(crate) conversation_history: InitialHistory,
    pub(crate) session_source: SessionSource,
    pub(crate) agent_control: AgentControl,
    pub(crate) dynamic_tools: Vec<DynamicToolSpec>,
    pub(crate) persist_extended_history: bool,
    pub(crate) metrics_service_name: Option<String>,
    pub(crate) inherited_shell_snapshot: Option<Arc<ShellSnapshot>>,
    pub(crate) inherited_exec_policy: Option<Arc<ExecPolicyManager>>,
    pub(crate) user_shell_override: Option<shell::Shell>,
    pub(crate) parent_trace: Option<W3cTraceContext>,
    pub(crate) analytics_events_client: Option<AnalyticsEventsClient>,
}

pub(crate) const INITIAL_SUBMIT_ID: &str = "";
pub(crate) const SUBMISSION_CHANNEL_CAPACITY: usize = 512;
const CYBER_VERIFY_URL: &str = "https://chatgpt.com/cyber";
const CYBER_SAFETY_URL: &str = "https://developers.openai.com/codex/concepts/cyber-safety";
impl Codex {
    /// Spawn a new [`Codex`] and initialize the session.
    pub(crate) async fn spawn(args: CodexSpawnArgs) -> CodexResult<CodexSpawnOk> {
        let parent_trace = match args.parent_trace {
            Some(trace) => {
                if codex_otel::context_from_w3c_trace_context(&trace).is_some() {
                    Some(trace)
                } else {
                    warn!("ignoring invalid thread spawn trace carrier");
                    None
                }
            }
            None => None,
        };
        let thread_spawn_span = info_span!("thread_spawn", otel.name = "thread_spawn");
        if let Some(trace) = parent_trace.as_ref() {
            let _ = set_parent_from_w3c_trace_context(&thread_spawn_span, trace);
        }
        Self::spawn_internal(CodexSpawnArgs {
            parent_trace,
            ..args
        })
        .instrument(thread_spawn_span)
        .await
    }

    async fn spawn_internal(args: CodexSpawnArgs) -> CodexResult<CodexSpawnOk> {
        let CodexSpawnArgs {
            mut config,
            auth_manager,
            models_manager,
            environment_manager,
            skills_manager,
            plugins_manager,
            mcp_manager,
            skills_watcher,
            conversation_history,
            session_source,
            agent_control,
            dynamic_tools,
            persist_extended_history,
            metrics_service_name,
            inherited_shell_snapshot,
            user_shell_override,
            inherited_exec_policy,
            parent_trace: _,
            analytics_events_client,
        } = args;
        let (tx_sub, rx_sub) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);
        let (tx_event, rx_event) = async_channel::unbounded();

        let environment = environment_manager
            .current()
            .await
            .map_err(|err| CodexErr::Fatal(format!("failed to create environment: {err}")))?;
        let fs = environment
            .as_ref()
            .map(|environment| environment.get_filesystem());
        let plugin_outcome = plugins_manager.plugins_for_config(&config).await;
        let effective_skill_roots = plugin_outcome.effective_skill_roots();
        let skills_input = skills_load_input_from_config(&config, effective_skill_roots);
        let loaded_skills = skills_manager.skills_for_config(&skills_input, fs).await;

        for err in &loaded_skills.errors {
            error!(
                "failed to load skill {}: {}",
                err.path.display(),
                err.message
            );
        }

        if let SessionSource::SubAgent(SubAgentSource::ThreadSpawn { depth, .. }) = session_source
            && depth >= config.agent_max_depth
        {
            let _ = config.features.disable(Feature::SpawnCsv);
            let _ = config.features.disable(Feature::Collab);
        }

        if config.features.enabled(Feature::JsRepl)
            && let Err(err) = resolve_compatible_node(config.js_repl_node_path.as_deref()).await
        {
            let _ = config.features.disable(Feature::JsRepl);
            let _ = config.features.disable(Feature::JsReplToolsOnly);
            let message = if config.features.enabled(Feature::JsRepl) {
                format!(
                    "`js_repl` remains enabled because enterprise requirements pin it on, but the configured Node runtime is unavailable or incompatible. {err}"
                )
            } else {
                format!(
                    "Disabled `js_repl` for this session because the configured Node runtime is unavailable or incompatible. {err}"
                )
            };
            warn!("{message}");
            config.startup_warnings.push(message);
        }
        if config.features.enabled(Feature::CodeMode)
            && let Err(err) = resolve_compatible_node(config.js_repl_node_path.as_deref()).await
        {
            let message = format!(
                "Disabled `exec` for this session because the configured Node runtime is unavailable or incompatible. {err}"
            );
            warn!("{message}");
            let _ = config.features.disable(Feature::CodeMode);
            config.startup_warnings.push(message);
        }

        let user_instructions = get_user_instructions(&config, environment.as_deref()).await;

        let exec_policy = if crate::guardian::is_guardian_reviewer_source(&session_source) {
            // Guardian review should rely on the built-in shell safety checks,
            // not on caller-provided exec-policy rules that could shape the
            // reviewer or silently auto-approve commands.
            Arc::new(ExecPolicyManager::default())
        } else if let Some(exec_policy) = &inherited_exec_policy {
            Arc::clone(exec_policy)
        } else {
            Arc::new(
                ExecPolicyManager::load(&config.config_layer_stack)
                    .await
                    .map_err(|err| CodexErr::Fatal(format!("failed to load rules: {err}")))?,
            )
        };

        let config = Arc::new(config);
        let refresh_strategy = match session_source {
            SessionSource::SubAgent(_) => codex_models_manager::manager::RefreshStrategy::Offline,
            _ => codex_models_manager::manager::RefreshStrategy::OnlineIfUncached,
        };
        if config.model.is_none()
            || !matches!(
                refresh_strategy,
                codex_models_manager::manager::RefreshStrategy::Offline
            )
        {
            let _ = models_manager.list_models(refresh_strategy).await;
        }
        let model = models_manager
            .get_default_model(&config.model, refresh_strategy)
            .await;

        // Resolve base instructions for the session. Priority order:
        // 1. config.base_instructions override
        // 2. conversation history => session_meta.base_instructions
        // 3. base_instructions for current model
        let model_info = models_manager
            .get_model_info(model.as_str(), &config.to_models_manager_config())
            .await;
        let base_instructions = config
            .base_instructions
            .clone()
            .or_else(|| conversation_history.get_base_instructions().map(|s| s.text))
            .unwrap_or_else(|| model_info.get_model_instructions(config.personality));

        // Respect thread-start tools. When missing (resumed/forked threads), read from the db
        // first, then fall back to rollout-file tools.
        let persisted_tools = if dynamic_tools.is_empty() {
            let thread_id = match &conversation_history {
                InitialHistory::Resumed(resumed) => Some(resumed.conversation_id),
                InitialHistory::Forked(_) => conversation_history.forked_from_id(),
                InitialHistory::New | InitialHistory::Cleared => None,
            };
            match thread_id {
                Some(thread_id) => {
                    let state_db_ctx = state_db::get_state_db(&config).await;
                    state_db::get_dynamic_tools(state_db_ctx.as_deref(), thread_id, "codex_spawn")
                        .await
                }
                None => None,
            }
        } else {
            None
        };
        let dynamic_tools = if dynamic_tools.is_empty() {
            persisted_tools
                .or_else(|| conversation_history.get_dynamic_tools())
                .unwrap_or_default()
        } else {
            dynamic_tools
        };

        // TODO (aibrahim): Consolidate config.model and config.model_reasoning_effort into config.collaboration_mode
        // to avoid extracting these fields separately and constructing CollaborationMode here.
        let collaboration_mode = CollaborationMode {
            mode: ModeKind::Default,
            settings: Settings {
                model: model.clone(),
                reasoning_effort: config.model_reasoning_effort,
                developer_instructions: None,
            },
        };
        let session_configuration = SessionConfiguration {
            provider: config.model_provider.clone(),
            collaboration_mode,
            model_reasoning_summary: config.model_reasoning_summary,
            service_tier: config.service_tier,
            developer_instructions: config.developer_instructions.clone(),
            user_instructions,
            personality: config.personality,
            base_instructions,
            compact_prompt: config.compact_prompt.clone(),
            approval_policy: config.permissions.approval_policy.clone(),
            approvals_reviewer: config.approvals_reviewer,
            sandbox_policy: config.permissions.sandbox_policy.clone(),
            file_system_sandbox_policy: config.permissions.file_system_sandbox_policy.clone(),
            network_sandbox_policy: config.permissions.network_sandbox_policy,
            windows_sandbox_level: WindowsSandboxLevel::from_config(&config),
            cwd: config.cwd.clone(),
            codex_home: config.codex_home.clone(),
            thread_name: None,
            original_config_do_not_use: Arc::clone(&config),
            metrics_service_name,
            app_server_client_name: None,
            app_server_client_version: None,
            session_source,
            dynamic_tools,
            persist_extended_history,
            inherited_shell_snapshot,
            user_shell_override,
        };

        // Generate a unique ID for the lifetime of this Codex session.
        let session_source_clone = session_configuration.session_source.clone();
        let (agent_status_tx, agent_status_rx) = watch::channel(AgentStatus::PendingInit);

        let session = Session::new(
            session_configuration,
            config.clone(),
            auth_manager.clone(),
            models_manager.clone(),
            exec_policy,
            tx_event.clone(),
            agent_status_tx.clone(),
            conversation_history,
            session_source_clone,
            skills_manager,
            plugins_manager,
            mcp_manager.clone(),
            skills_watcher,
            agent_control,
            environment,
            analytics_events_client,
        )
        .await
        .map_err(|e| {
            error!("Failed to create session: {e:#}");
            map_session_init_error(&e, &config.codex_home)
        })?;
        let thread_id = session.conversation_id;

        // This task will run until Op::Shutdown is received.
        let session_for_loop = Arc::clone(&session);
        let session_loop_handle = tokio::spawn(async move {
            submission_loop(session_for_loop, config, rx_sub)
                .instrument(info_span!("session_loop", thread_id = %thread_id))
                .await;
        });
        let codex = Codex {
            tx_sub,
            rx_event,
            agent_status: agent_status_rx,
            session,
            session_loop_termination: session_loop_termination_from_handle(session_loop_handle),
        };

        Ok(CodexSpawnOk { codex, thread_id })
    }

    /// Submit the `op` wrapped in a `Submission` with a unique ID.
    pub async fn submit(&self, op: Op) -> CodexResult<String> {
        self.submit_with_trace(op, /*trace*/ None).await
    }

    pub async fn submit_with_trace(
        &self,
        op: Op,
        trace: Option<W3cTraceContext>,
    ) -> CodexResult<String> {
        let id = Uuid::now_v7().to_string();
        let sub = Submission {
            id: id.clone(),
            op,
            trace,
        };
        self.submit_with_id(sub).await?;
        Ok(id)
    }

    /// Use sparingly: prefer `submit()` so Codex is responsible for generating
    /// unique IDs for each submission.
    pub async fn submit_with_id(&self, mut sub: Submission) -> CodexResult<()> {
        if sub.trace.is_none() {
            sub.trace = current_span_w3c_trace_context();
        }
        self.tx_sub
            .send(sub)
            .await
            .map_err(|_| CodexErr::InternalAgentDied)?;
        Ok(())
    }

    /// Persist a thread-level memory mode update for the active session.
    ///
    /// This is a local-only operation that updates rollout metadata directly
    /// and does not involve the model.
    pub async fn set_thread_memory_mode(
        &self,
        mode: codex_protocol::protocol::ThreadMemoryMode,
    ) -> anyhow::Result<()> {
        handlers::persist_thread_memory_mode_update(&self.session, mode).await
    }

    pub async fn shutdown_and_wait(&self) -> CodexResult<()> {
        let session_loop_termination = self.session_loop_termination.clone();
        match self.submit(Op::Shutdown).await {
            Ok(_) => {}
            Err(CodexErr::InternalAgentDied) => {}
            Err(err) => return Err(err),
        }
        session_loop_termination.await;
        Ok(())
    }

    pub async fn next_event(&self) -> CodexResult<Event> {
        let event = self
            .rx_event
            .recv()
            .await
            .map_err(|_| CodexErr::InternalAgentDied)?;
        Ok(event)
    }

    pub async fn steer_input(
        &self,
        input: Vec<UserInput>,
        expected_turn_id: Option<&str>,
        responsesapi_client_metadata: Option<HashMap<String, String>>,
    ) -> Result<String, SteerInputError> {
        self.session
            .steer_input(input, expected_turn_id, responsesapi_client_metadata)
            .await
    }

    pub(crate) async fn set_app_server_client_info(
        &self,
        app_server_client_name: Option<String>,
        app_server_client_version: Option<String>,
    ) -> ConstraintResult<()> {
        self.session
            .update_settings(SessionSettingsUpdate {
                app_server_client_name,
                app_server_client_version,
                ..Default::default()
            })
            .await
    }

    pub(crate) async fn agent_status(&self) -> AgentStatus {
        self.agent_status.borrow().clone()
    }

    pub(crate) async fn thread_config_snapshot(&self) -> ThreadConfigSnapshot {
        let state = self.session.state.lock().await;
        state.session_configuration.thread_config_snapshot()
    }

    pub(crate) fn state_db(&self) -> Option<state_db::StateDbHandle> {
        self.session.state_db()
    }

    pub(crate) fn enabled(&self, feature: Feature) -> bool {
        self.session.enabled(feature)
    }
}

#[cfg(test)]
pub(crate) fn completed_session_loop_termination() -> SessionLoopTermination {
    futures::future::ready(()).boxed().shared()
}

pub(crate) fn session_loop_termination_from_handle(
    handle: JoinHandle<()>,
) -> SessionLoopTermination {
    async move {
        let _ = handle.await;
    }
    .boxed()
    .shared()
}

/// Context for an initialized model agent
///
/// A session has at most 1 running task at a time, and can be interrupted by user input.
pub(crate) struct Session {
    pub(crate) conversation_id: ThreadId,
    tx_event: Sender<Event>,
    agent_status: watch::Sender<AgentStatus>,
    out_of_band_elicitation_paused: watch::Sender<bool>,
    state: Mutex<SessionState>,
    /// Serializes rebuild/apply cycles for the running proxy; each cycle
    /// rebuilds from the current SessionState while holding this lock.
    managed_network_proxy_refresh_lock: Mutex<()>,
    /// The set of enabled features should be invariant for the lifetime of the
    /// session.
    features: ManagedFeatures,
    pending_mcp_server_refresh_config: Mutex<Option<McpServerRefreshConfig>>,
    pub(crate) conversation: Arc<RealtimeConversationManager>,
    pub(crate) active_turn: Mutex<Option<ActiveTurn>>,
    mailbox: Mailbox,
    mailbox_rx: Mutex<MailboxReceiver>,
    idle_pending_input: Mutex<Vec<ResponseInputItem>>, // TODO (jif) merge with mailbox!
    pub(crate) guardian_review_session: GuardianReviewSessionManager,
    pub(crate) services: SessionServices,
    js_repl: Arc<JsReplHandle>,
    next_internal_sub_id: AtomicU64,
}

#[derive(Clone, Debug)]
pub(crate) struct TurnSkillsContext {
    pub(crate) outcome: Arc<SkillLoadOutcome>,
    pub(crate) implicit_invocation_seen_skills: Arc<Mutex<HashSet<String>>>,
}

impl TurnSkillsContext {
    pub(crate) fn new(outcome: Arc<SkillLoadOutcome>) -> Self {
        Self {
            outcome,
            implicit_invocation_seen_skills: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

/// The context needed for a single turn of the thread.
#[derive(Debug)]
pub(crate) struct TurnContext {
    pub(crate) sub_id: String,
    pub(crate) trace_id: Option<String>,
    pub(crate) realtime_active: bool,
    pub(crate) config: Arc<Config>,
    pub(crate) auth_manager: Option<Arc<AuthManager>>,
    pub(crate) model_info: ModelInfo,
    pub(crate) session_telemetry: SessionTelemetry,
    pub(crate) provider: ModelProviderInfo,
    pub(crate) reasoning_effort: Option<ReasoningEffortConfig>,
    pub(crate) reasoning_summary: ReasoningSummaryConfig,
    pub(crate) session_source: SessionSource,
    pub(crate) environment: Option<Arc<Environment>>,
    /// The session's absolute working directory. All relative paths provided
    /// by the model as well as sandbox policies are resolved against this path
    /// instead of `std::env::current_dir()`.
    pub(crate) cwd: AbsolutePathBuf,
    pub(crate) current_date: Option<String>,
    pub(crate) timezone: Option<String>,
    pub(crate) app_server_client_name: Option<String>,
    pub(crate) developer_instructions: Option<String>,
    pub(crate) compact_prompt: Option<String>,
    pub(crate) user_instructions: Option<String>,
    pub(crate) collaboration_mode: CollaborationMode,
    pub(crate) personality: Option<Personality>,
    pub(crate) approval_policy: Constrained<AskForApproval>,
    pub(crate) sandbox_policy: Constrained<SandboxPolicy>,
    pub(crate) file_system_sandbox_policy: FileSystemSandboxPolicy,
    pub(crate) network_sandbox_policy: NetworkSandboxPolicy,
    pub(crate) network: Option<NetworkProxy>,
    pub(crate) windows_sandbox_level: WindowsSandboxLevel,
    pub(crate) shell_environment_policy: ShellEnvironmentPolicy,
    pub(crate) tools_config: ToolsConfig,
    pub(crate) features: ManagedFeatures,
    pub(crate) ghost_snapshot: GhostSnapshotConfig,
    pub(crate) final_output_json_schema: Option<Value>,
    pub(crate) codex_self_exe: Option<PathBuf>,
    pub(crate) codex_linux_sandbox_exe: Option<PathBuf>,
    pub(crate) tool_call_gate: Arc<ReadinessFlag>,
    pub(crate) truncation_policy: TruncationPolicy,
    pub(crate) js_repl: Arc<JsReplHandle>,
    pub(crate) dynamic_tools: Vec<DynamicToolSpec>,
    pub(crate) turn_metadata_state: Arc<TurnMetadataState>,
    pub(crate) turn_skills: TurnSkillsContext,
    pub(crate) turn_timing_state: Arc<TurnTimingState>,
}
impl TurnContext {
    pub(crate) fn model_context_window(&self) -> Option<i64> {
        let effective_context_window_percent = self.model_info.effective_context_window_percent;
        self.model_info.context_window.map(|context_window| {
            context_window.saturating_mul(effective_context_window_percent) / 100
        })
    }

    pub(crate) fn apps_enabled(&self) -> bool {
        let is_chatgpt_auth = self
            .auth_manager
            .as_deref()
            .and_then(AuthManager::auth_cached)
            .as_ref()
            .is_some_and(CodexAuth::is_chatgpt_auth);
        self.features.apps_enabled_for_auth(is_chatgpt_auth)
    }

    pub(crate) async fn with_model(&self, model: String, models_manager: &ModelsManager) -> Self {
        let mut config = (*self.config).clone();
        config.model = Some(model.clone());
        let model_info = models_manager
            .get_model_info(model.as_str(), &config.to_models_manager_config())
            .await;
        let truncation_policy = model_info.truncation_policy.into();
        let supported_reasoning_levels = model_info
            .supported_reasoning_levels
            .iter()
            .map(|preset| preset.effort)
            .collect::<Vec<_>>();
        let reasoning_effort = if let Some(current_reasoning_effort) = self.reasoning_effort {
            if supported_reasoning_levels.contains(&current_reasoning_effort) {
                Some(current_reasoning_effort)
            } else {
                supported_reasoning_levels
                    .get(supported_reasoning_levels.len().saturating_sub(1) / 2)
                    .copied()
                    .or(model_info.default_reasoning_level)
            }
        } else {
            supported_reasoning_levels
                .get(supported_reasoning_levels.len().saturating_sub(1) / 2)
                .copied()
                .or(model_info.default_reasoning_level)
        };
        config.model_reasoning_effort = reasoning_effort;

        let collaboration_mode = self.collaboration_mode.with_updates(
            Some(model.clone()),
            Some(reasoning_effort),
            /*developer_instructions*/ None,
        );
        let features = self.features.clone();
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &models_manager
                .list_models(RefreshStrategy::OnlineIfUncached)
                .await,
            features: &features,
            image_generation_tool_auth_allowed: image_generation_tool_auth_allowed(
                self.auth_manager.as_deref(),
            ),
            web_search_mode: self.tools_config.web_search_mode,
            session_source: self.session_source.clone(),
            sandbox_policy: self.sandbox_policy.get(),
            windows_sandbox_level: self.windows_sandbox_level,
        })
        .with_unified_exec_shell_mode(self.tools_config.unified_exec_shell_mode.clone())
        .with_web_search_config(self.tools_config.web_search_config.clone())
        .with_allow_login_shell(self.tools_config.allow_login_shell)
        .with_has_environment(self.tools_config.has_environment)
        .with_spawn_agent_usage_hint(config.multi_agent_v2.usage_hint_enabled)
        .with_spawn_agent_usage_hint_text(config.multi_agent_v2.usage_hint_text.clone())
        .with_hide_spawn_agent_metadata(config.multi_agent_v2.hide_spawn_agent_metadata)
        .with_agent_type_description(crate::agent::role::spawn_tool_spec::build(
            &config.agent_roles,
        ));

        Self {
            sub_id: self.sub_id.clone(),
            trace_id: self.trace_id.clone(),
            realtime_active: self.realtime_active,
            config: Arc::new(config),
            auth_manager: self.auth_manager.clone(),
            model_info: model_info.clone(),
            session_telemetry: self
                .session_telemetry
                .clone()
                .with_model(model.as_str(), model_info.slug.as_str()),
            provider: self.provider.clone(),
            reasoning_effort,
            reasoning_summary: self.reasoning_summary,
            session_source: self.session_source.clone(),
            environment: self.environment.clone(),
            cwd: self.cwd.clone(),
            current_date: self.current_date.clone(),
            timezone: self.timezone.clone(),
            app_server_client_name: self.app_server_client_name.clone(),
            developer_instructions: self.developer_instructions.clone(),
            compact_prompt: self.compact_prompt.clone(),
            user_instructions: self.user_instructions.clone(),
            collaboration_mode,
            personality: self.personality,
            approval_policy: self.approval_policy.clone(),
            sandbox_policy: self.sandbox_policy.clone(),
            file_system_sandbox_policy: self.file_system_sandbox_policy.clone(),
            network_sandbox_policy: self.network_sandbox_policy,
            network: self.network.clone(),
            windows_sandbox_level: self.windows_sandbox_level,
            shell_environment_policy: self.shell_environment_policy.clone(),
            tools_config,
            features,
            ghost_snapshot: self.ghost_snapshot.clone(),
            final_output_json_schema: self.final_output_json_schema.clone(),
            codex_self_exe: self.codex_self_exe.clone(),
            codex_linux_sandbox_exe: self.codex_linux_sandbox_exe.clone(),
            tool_call_gate: Arc::new(ReadinessFlag::new()),
            truncation_policy,
            js_repl: Arc::clone(&self.js_repl),
            dynamic_tools: self.dynamic_tools.clone(),
            turn_metadata_state: self.turn_metadata_state.clone(),
            turn_skills: self.turn_skills.clone(),
            turn_timing_state: Arc::clone(&self.turn_timing_state),
        }
    }

    pub(crate) fn resolve_path(&self, path: Option<String>) -> AbsolutePathBuf {
        path.as_ref()
            .map_or_else(|| self.cwd.clone(), |path| self.cwd.join(path))
    }

    pub(crate) fn file_system_sandbox_context(
        &self,
        additional_permissions: Option<PermissionProfile>,
    ) -> FileSystemSandboxContext {
        FileSystemSandboxContext {
            sandbox_policy: self.sandbox_policy.get().clone(),
            windows_sandbox_level: self.windows_sandbox_level,
            windows_sandbox_private_desktop: self
                .config
                .permissions
                .windows_sandbox_private_desktop,
            use_legacy_landlock: self.features.use_legacy_landlock(),
            additional_permissions,
        }
    }

    pub(crate) fn compact_prompt(&self) -> &str {
        self.compact_prompt
            .as_deref()
            .unwrap_or(compact::SUMMARIZATION_PROMPT)
    }

    pub(crate) fn to_turn_context_item(&self) -> TurnContextItem {
        TurnContextItem {
            turn_id: Some(self.sub_id.clone()),
            trace_id: self.trace_id.clone(),
            cwd: self.cwd.to_path_buf(),
            current_date: self.current_date.clone(),
            timezone: self.timezone.clone(),
            approval_policy: self.approval_policy.value(),
            sandbox_policy: self.sandbox_policy.get().clone(),
            network: self.turn_context_network_item(),
            model: self.model_info.slug.clone(),
            personality: self.personality,
            collaboration_mode: Some(self.collaboration_mode.clone()),
            realtime_active: Some(self.realtime_active),
            effort: self.reasoning_effort,
            summary: self.reasoning_summary,
            user_instructions: self.user_instructions.clone(),
            developer_instructions: self.developer_instructions.clone(),
            final_output_json_schema: self.final_output_json_schema.clone(),
            truncation_policy: Some(self.truncation_policy),
        }
    }

    fn turn_context_network_item(&self) -> Option<TurnContextNetworkItem> {
        let network = self
            .config
            .config_layer_stack
            .requirements()
            .network
            .as_ref()?;
        Some(TurnContextNetworkItem {
            allowed_domains: network
                .domains
                .as_ref()
                .and_then(codex_config::NetworkDomainPermissionsToml::allowed_domains)
                .unwrap_or_default(),
            denied_domains: network
                .domains
                .as_ref()
                .and_then(codex_config::NetworkDomainPermissionsToml::denied_domains)
                .unwrap_or_default(),
        })
    }
}

fn local_time_context() -> (String, String) {
    match iana_time_zone::get_timezone() {
        Ok(timezone) => (Local::now().format("%Y-%m-%d").to_string(), timezone),
        Err(_) => (
            Utc::now().format("%Y-%m-%d").to_string(),
            "Etc/UTC".to_string(),
        ),
    }
}

async fn thread_title_from_state_db(
    state_db: Option<&state_db::StateDbHandle>,
    codex_home: &AbsolutePathBuf,
    conversation_id: ThreadId,
) -> Option<String> {
    if let Some(metadata) = state_db
        && let Some(metadata) = metadata.get_thread(conversation_id).await.ok().flatten()
    {
        let title = metadata.title.trim();
        if !title.is_empty() && metadata.first_user_message.as_deref().map(str::trim) != Some(title)
        {
            return Some(title.to_string());
        }
    }
    find_thread_name_by_id(codex_home, &conversation_id)
        .await
        .ok()
        .flatten()
}

#[derive(Clone)]
pub(crate) struct SessionConfiguration {
    /// Provider identifier ("openai", "openrouter", ...).
    provider: ModelProviderInfo,

    collaboration_mode: CollaborationMode,
    model_reasoning_summary: Option<ReasoningSummaryConfig>,
    service_tier: Option<ServiceTier>,

    /// Developer instructions that supplement the base instructions.
    developer_instructions: Option<String>,

    /// Model instructions that are appended to the base instructions.
    user_instructions: Option<String>,

    /// Personality preference for the model.
    personality: Option<Personality>,

    /// Base instructions for the session.
    base_instructions: String,

    /// Compact prompt override.
    compact_prompt: Option<String>,

    /// When to escalate for approval for execution
    approval_policy: Constrained<AskForApproval>,
    approvals_reviewer: ApprovalsReviewer,
    /// How to sandbox commands executed in the system
    sandbox_policy: Constrained<SandboxPolicy>,
    file_system_sandbox_policy: FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    windows_sandbox_level: WindowsSandboxLevel,

    /// Absolute working directory that should be treated as the *root* of the
    /// session. All relative paths supplied by the model as well as the
    /// execution sandbox are resolved against this directory **instead** of
    /// the process-wide current working directory.
    cwd: AbsolutePathBuf,
    /// Directory containing all Codex state for this session.
    codex_home: AbsolutePathBuf,
    /// Optional user-facing name for the thread, updated during the session.
    thread_name: Option<String>,

    // TODO(pakrym): Remove config from here
    original_config_do_not_use: Arc<Config>,
    /// Optional service name tag for session metrics.
    metrics_service_name: Option<String>,
    app_server_client_name: Option<String>,
    app_server_client_version: Option<String>,
    /// Source of the session (cli, vscode, exec, mcp, ...)
    session_source: SessionSource,
    dynamic_tools: Vec<DynamicToolSpec>,
    persist_extended_history: bool,
    inherited_shell_snapshot: Option<Arc<ShellSnapshot>>,
    user_shell_override: Option<shell::Shell>,
}

impl SessionConfiguration {
    pub(crate) fn codex_home(&self) -> &AbsolutePathBuf {
        &self.codex_home
    }

    fn thread_config_snapshot(&self) -> ThreadConfigSnapshot {
        ThreadConfigSnapshot {
            model: self.collaboration_mode.model().to_string(),
            model_provider_id: self.original_config_do_not_use.model_provider_id.clone(),
            service_tier: self.service_tier,
            approval_policy: self.approval_policy.value(),
            approvals_reviewer: self.approvals_reviewer,
            sandbox_policy: self.sandbox_policy.get().clone(),
            cwd: self.cwd.clone(),
            ephemeral: self.original_config_do_not_use.ephemeral,
            reasoning_effort: self.collaboration_mode.reasoning_effort(),
            personality: self.personality,
            session_source: self.session_source.clone(),
        }
    }

    pub(crate) fn apply(&self, updates: &SessionSettingsUpdate) -> ConstraintResult<Self> {
        let mut next_configuration = self.clone();
        let file_system_policy_matches_legacy = self.file_system_sandbox_policy
            == FileSystemSandboxPolicy::from_legacy_sandbox_policy(
                self.sandbox_policy.get(),
                &self.cwd,
            );
        if let Some(collaboration_mode) = updates.collaboration_mode.clone() {
            next_configuration.collaboration_mode = collaboration_mode;
        }
        if let Some(summary) = updates.reasoning_summary {
            next_configuration.model_reasoning_summary = Some(summary);
        }
        if let Some(service_tier) = updates.service_tier {
            next_configuration.service_tier = service_tier;
        }
        if let Some(personality) = updates.personality {
            next_configuration.personality = Some(personality);
        }
        if let Some(approval_policy) = updates.approval_policy {
            next_configuration.approval_policy.set(approval_policy)?;
        }
        if let Some(approvals_reviewer) = updates.approvals_reviewer {
            next_configuration.approvals_reviewer = approvals_reviewer;
        }
        let mut sandbox_policy_changed = false;
        if let Some(sandbox_policy) = updates.sandbox_policy.clone() {
            next_configuration.sandbox_policy.set(sandbox_policy)?;
            next_configuration.network_sandbox_policy =
                NetworkSandboxPolicy::from(next_configuration.sandbox_policy.get());
            sandbox_policy_changed = true;
        }
        if let Some(windows_sandbox_level) = updates.windows_sandbox_level {
            next_configuration.windows_sandbox_level = windows_sandbox_level;
        }

        let absolute_cwd = updates
            .cwd
            .as_ref()
            .map(|cwd| {
                AbsolutePathBuf::relative_to_current_dir(normalize_for_native_workdir(
                    cwd.as_path(),
                ))
                .unwrap_or_else(|e| {
                    warn!("failed to normalize update cwd: {cwd:?}: {e}");
                    self.cwd.clone()
                })
            })
            .unwrap_or_else(|| self.cwd.clone());

        let cwd_changed = absolute_cwd.as_path() != self.cwd.as_path();
        next_configuration.cwd = absolute_cwd;
        if sandbox_policy_changed || (cwd_changed && file_system_policy_matches_legacy) {
            // Preserve richer split policies across cwd-only updates; only
            // rederive when the session is already using the legacy bridge.
            next_configuration.file_system_sandbox_policy =
                FileSystemSandboxPolicy::from_legacy_sandbox_policy(
                    next_configuration.sandbox_policy.get(),
                    &next_configuration.cwd,
                );
        }
        if let Some(app_server_client_name) = updates.app_server_client_name.clone() {
            next_configuration.app_server_client_name = Some(app_server_client_name);
        }
        if let Some(app_server_client_version) = updates.app_server_client_version.clone() {
            next_configuration.app_server_client_version = Some(app_server_client_version);
        }
        Ok(next_configuration)
    }
}

#[derive(Default, Clone)]
pub(crate) struct SessionSettingsUpdate {
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) approval_policy: Option<AskForApproval>,
    pub(crate) approvals_reviewer: Option<ApprovalsReviewer>,
    pub(crate) sandbox_policy: Option<SandboxPolicy>,
    pub(crate) windows_sandbox_level: Option<WindowsSandboxLevel>,
    pub(crate) collaboration_mode: Option<CollaborationMode>,
    pub(crate) reasoning_summary: Option<ReasoningSummaryConfig>,
    pub(crate) service_tier: Option<Option<ServiceTier>>,
    pub(crate) final_output_json_schema: Option<Option<Value>>,
    pub(crate) personality: Option<Personality>,
    pub(crate) app_server_client_name: Option<String>,
    pub(crate) app_server_client_version: Option<String>,
}

pub(crate) struct AppServerClientMetadata {
    pub(crate) client_name: Option<String>,
    pub(crate) client_version: Option<String>,
}

impl Session {
    pub(crate) async fn app_server_client_metadata(&self) -> AppServerClientMetadata {
        let state = self.state.lock().await;
        AppServerClientMetadata {
            client_name: state.session_configuration.app_server_client_name.clone(),
            client_version: state
                .session_configuration
                .app_server_client_version
                .clone(),
        }
    }

    /// Builds the `x-codex-beta-features` header value for this session.
    ///
    /// `ModelClient` is session-scoped and intentionally does not depend on the full `Config`, so
    /// we precompute the comma-separated list of enabled experimental feature keys at session
    /// creation time and thread it into the client.
    fn build_model_client_beta_features_header(config: &Config) -> Option<String> {
        let beta_features_header = FEATURES
            .iter()
            .filter_map(|spec| {
                if spec.stage.experimental_menu_description().is_some()
                    && config.features.enabled(spec.id)
                {
                    Some(spec.key)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(",");

        if beta_features_header.is_empty() {
            None
        } else {
            Some(beta_features_header)
        }
    }

    async fn start_managed_network_proxy(
        spec: &crate::config::NetworkProxySpec,
        exec_policy: &codex_execpolicy::Policy,
        sandbox_policy: &SandboxPolicy,
        network_policy_decider: Option<Arc<dyn codex_network_proxy::NetworkPolicyDecider>>,
        blocked_request_observer: Option<Arc<dyn codex_network_proxy::BlockedRequestObserver>>,
        managed_network_requirements_enabled: bool,
        audit_metadata: NetworkProxyAuditMetadata,
    ) -> anyhow::Result<(StartedNetworkProxy, SessionNetworkProxyRuntime)> {
        let spec = spec
            .with_exec_policy_network_rules(exec_policy)
            .map_err(|err| {
                tracing::warn!(
                    "failed to apply execpolicy network rules to managed proxy; continuing with configured network policy: {err}"
                );
                err
            })
            .unwrap_or_else(|_| spec.clone());
        let network_proxy = spec
            .start_proxy(
                sandbox_policy,
                network_policy_decider,
                blocked_request_observer,
                managed_network_requirements_enabled,
                audit_metadata,
            )
            .await
            .map_err(|err| anyhow::anyhow!("failed to start managed network proxy: {err}"))?;
        let session_network_proxy = {
            let proxy = network_proxy.proxy();
            SessionNetworkProxyRuntime {
                http_addr: proxy.http_addr().to_string(),
                socks_addr: proxy.socks_addr().to_string(),
            }
        };
        Ok((network_proxy, session_network_proxy))
    }

    async fn refresh_managed_network_proxy_for_current_sandbox_policy(&self) {
        let Some(started_proxy) = self.services.network_proxy.as_ref() else {
            return;
        };
        let _refresh_guard = self.managed_network_proxy_refresh_lock.lock().await;
        let session_configuration = {
            let state = self.state.lock().await;
            state.session_configuration.clone()
        };
        let Some(spec) = session_configuration
            .original_config_do_not_use
            .permissions
            .network
            .as_ref()
        else {
            return;
        };

        let spec = match spec
            .recompute_for_sandbox_policy(session_configuration.sandbox_policy.get())
        {
            Ok(spec) => spec,
            Err(err) => {
                warn!("failed to rebuild managed network proxy policy for sandbox change: {err}");
                return;
            }
        };
        let current_exec_policy = self.services.exec_policy.current();
        let spec = match spec.with_exec_policy_network_rules(current_exec_policy.as_ref()) {
            Ok(spec) => spec,
            Err(err) => {
                warn!(
                    "failed to apply execpolicy network rules while refreshing managed network proxy: {err}"
                );
                spec
            }
        };
        if let Err(err) = spec.apply_to_started_proxy(started_proxy).await {
            warn!("failed to refresh managed network proxy for sandbox change: {err}");
        }
    }

    /// Don't expand the number of mutated arguments on config. We are in the process of getting rid of it.
    pub(crate) fn build_per_turn_config(session_configuration: &SessionConfiguration) -> Config {
        // todo(aibrahim): store this state somewhere else so we don't need to mut config
        let config = session_configuration.original_config_do_not_use.clone();
        let mut per_turn_config = (*config).clone();
        per_turn_config.cwd = session_configuration.cwd.clone();
        per_turn_config.model_reasoning_effort =
            session_configuration.collaboration_mode.reasoning_effort();
        per_turn_config.model_reasoning_summary = session_configuration.model_reasoning_summary;
        per_turn_config.service_tier = session_configuration.service_tier;
        per_turn_config.personality = session_configuration.personality;
        per_turn_config.approvals_reviewer = session_configuration.approvals_reviewer;
        let resolved_web_search_mode = resolve_web_search_mode_for_turn(
            &per_turn_config.web_search_mode,
            session_configuration.sandbox_policy.get(),
        );
        if let Err(err) = per_turn_config
            .web_search_mode
            .set(resolved_web_search_mode)
        {
            let fallback_value = per_turn_config.web_search_mode.value();
            tracing::warn!(
                error = %err,
                ?resolved_web_search_mode,
                ?fallback_value,
                "resolved web_search_mode is disallowed by requirements; keeping constrained value"
            );
        }
        per_turn_config.features = config.features.clone();
        per_turn_config
    }

    pub(crate) async fn codex_home(&self) -> AbsolutePathBuf {
        let state = self.state.lock().await;
        state.session_configuration.codex_home().clone()
    }

    pub(crate) fn subscribe_out_of_band_elicitation_pause_state(&self) -> watch::Receiver<bool> {
        self.out_of_band_elicitation_paused.subscribe()
    }

    pub(crate) fn set_out_of_band_elicitation_pause_state(&self, paused: bool) {
        self.out_of_band_elicitation_paused.send_replace(paused);
    }

    fn start_skills_watcher_listener(self: &Arc<Self>) {
        let mut rx = self.services.skills_watcher.subscribe();
        let weak_sess = Arc::downgrade(self);
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(SkillsWatcherEvent::SkillsChanged { .. }) => {
                        let Some(sess) = weak_sess.upgrade() else {
                            break;
                        };
                        let event = Event {
                            id: sess.next_internal_sub_id(),
                            msg: EventMsg::SkillsUpdateAvailable,
                        };
                        sess.send_event_raw(event).await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    }

    fn start_agent_identity_registration(self: &Arc<Self>) {
        if !self.services.agent_identity_manager.is_enabled() {
            return;
        }

        let weak_sess = Arc::downgrade(self);
        let mut auth_state_rx = self.services.auth_manager.subscribe_auth_state();
        tokio::spawn(async move {
            loop {
                let Some(sess) = weak_sess.upgrade() else {
                    return;
                };
                match sess
                    .services
                    .agent_identity_manager
                    .ensure_registered_identity()
                    .await
                {
                    Ok(Some(_)) => return,
                    Ok(None) => {
                        drop(sess);
                        if auth_state_rx.changed().await.is_err() {
                            return;
                        }
                    }
                    Err(error) => {
                        sess.fail_agent_identity_registration(error).await;
                        return;
                    }
                }
            }
        });
    }

    async fn fail_agent_identity_registration(self: &Arc<Self>, error: anyhow::Error) {
        warn!(error = %error, "agent identity registration failed");
        let message = format!(
            "Agent identity registration failed. Codex cannot continue while `features.use_agent_identity` is enabled: {error}"
        );
        self.send_event_raw(Event {
            id: self.next_internal_sub_id(),
            msg: EventMsg::Error(ErrorEvent {
                message,
                codex_error_info: Some(CodexErrorInfo::Other),
            }),
        })
        .await;
        handlers::shutdown(self, self.next_internal_sub_id()).await;
    }

    #[allow(clippy::too_many_arguments)]
    fn make_turn_context(
        conversation_id: ThreadId,
        auth_manager: Option<Arc<AuthManager>>,
        session_telemetry: &SessionTelemetry,
        provider: ModelProviderInfo,
        session_configuration: &SessionConfiguration,
        user_shell: &shell::Shell,
        shell_zsh_path: Option<&PathBuf>,
        main_execve_wrapper_exe: Option<&PathBuf>,
        per_turn_config: Config,
        model_info: ModelInfo,
        models_manager: &ModelsManager,
        network: Option<NetworkProxy>,
        environment: Option<Arc<Environment>>,
        sub_id: String,
        js_repl: Arc<JsReplHandle>,
        skills_outcome: Arc<SkillLoadOutcome>,
    ) -> TurnContext {
        let reasoning_effort = session_configuration.collaboration_mode.reasoning_effort();
        let reasoning_summary = session_configuration
            .model_reasoning_summary
            .unwrap_or(model_info.default_reasoning_summary);
        let session_telemetry = session_telemetry.clone().with_model(
            session_configuration.collaboration_mode.model(),
            model_info.slug.as_str(),
        );
        let session_source = session_configuration.session_source.clone();
        let image_generation_tool_auth_allowed =
            image_generation_tool_auth_allowed(auth_manager.as_deref());
        let auth_manager_for_context = auth_manager;
        let provider_for_context = provider;
        let session_telemetry_for_context = session_telemetry;
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_info: &model_info,
            available_models: &models_manager.try_list_models().unwrap_or_default(),
            features: &per_turn_config.features,
            image_generation_tool_auth_allowed,
            web_search_mode: Some(per_turn_config.web_search_mode.value()),
            session_source: session_source.clone(),
            sandbox_policy: session_configuration.sandbox_policy.get(),
            windows_sandbox_level: session_configuration.windows_sandbox_level,
        })
        .with_unified_exec_shell_mode_for_session(
            crate::tools::spec::tool_user_shell_type(user_shell),
            shell_zsh_path,
            main_execve_wrapper_exe,
        )
        .with_web_search_config(per_turn_config.web_search_config.clone())
        .with_allow_login_shell(per_turn_config.permissions.allow_login_shell)
        .with_has_environment(environment.is_some())
        .with_spawn_agent_usage_hint(per_turn_config.multi_agent_v2.usage_hint_enabled)
        .with_spawn_agent_usage_hint_text(per_turn_config.multi_agent_v2.usage_hint_text.clone())
        .with_hide_spawn_agent_metadata(per_turn_config.multi_agent_v2.hide_spawn_agent_metadata)
        .with_agent_type_description(crate::agent::role::spawn_tool_spec::build(
            &per_turn_config.agent_roles,
        ));

        let cwd = session_configuration.cwd.clone();

        let per_turn_config = Arc::new(per_turn_config);
        let turn_metadata_state = Arc::new(TurnMetadataState::new(
            conversation_id.to_string(),
            &session_source,
            sub_id.clone(),
            cwd.clone(),
            session_configuration.sandbox_policy.get(),
            session_configuration.windows_sandbox_level,
        ));
        let (current_date, timezone) = local_time_context();
        TurnContext {
            sub_id,
            trace_id: current_span_trace_id(),
            realtime_active: false,
            config: per_turn_config.clone(),
            auth_manager: auth_manager_for_context,
            model_info: model_info.clone(),
            session_telemetry: session_telemetry_for_context,
            provider: provider_for_context,
            reasoning_effort,
            reasoning_summary,
            session_source,
            environment,
            cwd,
            current_date: Some(current_date),
            timezone: Some(timezone),
            app_server_client_name: session_configuration.app_server_client_name.clone(),
            developer_instructions: session_configuration.developer_instructions.clone(),
            compact_prompt: session_configuration.compact_prompt.clone(),
            user_instructions: session_configuration.user_instructions.clone(),
            collaboration_mode: session_configuration.collaboration_mode.clone(),
            personality: session_configuration.personality,
            approval_policy: session_configuration.approval_policy.clone(),
            sandbox_policy: session_configuration.sandbox_policy.clone(),
            file_system_sandbox_policy: session_configuration.file_system_sandbox_policy.clone(),
            network_sandbox_policy: session_configuration.network_sandbox_policy,
            network,
            windows_sandbox_level: session_configuration.windows_sandbox_level,
            shell_environment_policy: per_turn_config.permissions.shell_environment_policy.clone(),
            tools_config,
            features: per_turn_config.features.clone(),
            ghost_snapshot: per_turn_config.ghost_snapshot.clone(),
            final_output_json_schema: None,
            codex_self_exe: per_turn_config.codex_self_exe.clone(),
            codex_linux_sandbox_exe: per_turn_config.codex_linux_sandbox_exe.clone(),
            tool_call_gate: Arc::new(ReadinessFlag::new()),
            truncation_policy: model_info.truncation_policy.into(),
            js_repl,
            dynamic_tools: session_configuration.dynamic_tools.clone(),
            turn_metadata_state,
            turn_skills: TurnSkillsContext::new(skills_outcome),
            turn_timing_state: Arc::new(TurnTimingState::default()),
        }
    }

    #[instrument(name = "session_init", level = "info", skip_all)]
    #[allow(clippy::too_many_arguments)]
    async fn new(
        mut session_configuration: SessionConfiguration,
        config: Arc<Config>,
        auth_manager: Arc<AuthManager>,
        models_manager: Arc<ModelsManager>,
        exec_policy: Arc<ExecPolicyManager>,
        tx_event: Sender<Event>,
        agent_status: watch::Sender<AgentStatus>,
        initial_history: InitialHistory,
        session_source: SessionSource,
        skills_manager: Arc<SkillsManager>,
        plugins_manager: Arc<PluginsManager>,
        mcp_manager: Arc<McpManager>,
        skills_watcher: Arc<SkillsWatcher>,
        agent_control: AgentControl,
        environment: Option<Arc<Environment>>,
        analytics_events_client: Option<AnalyticsEventsClient>,
    ) -> anyhow::Result<Arc<Self>> {
        debug!(
            "Configuring session: model={}; provider={:?}",
            session_configuration.collaboration_mode.model(),
            session_configuration.provider
        );
        let forked_from_id = initial_history.forked_from_id();

        let (conversation_id, rollout_params) = match &initial_history {
            InitialHistory::New | InitialHistory::Cleared | InitialHistory::Forked(_) => {
                let conversation_id = ThreadId::default();
                (
                    conversation_id,
                    RolloutRecorderParams::new(
                        conversation_id,
                        forked_from_id,
                        session_source,
                        BaseInstructions {
                            text: session_configuration.base_instructions.clone(),
                        },
                        session_configuration.dynamic_tools.clone(),
                        if session_configuration.persist_extended_history {
                            EventPersistenceMode::Extended
                        } else {
                            EventPersistenceMode::Limited
                        },
                    ),
                )
            }
            InitialHistory::Resumed(resumed_history) => (
                resumed_history.conversation_id,
                RolloutRecorderParams::resume(
                    resumed_history.rollout_path.clone(),
                    if session_configuration.persist_extended_history {
                        EventPersistenceMode::Extended
                    } else {
                        EventPersistenceMode::Limited
                    },
                ),
            ),
        };
        let window_generation = match &initial_history {
            InitialHistory::Resumed(resumed_history) => u64::try_from(
                resumed_history
                    .history
                    .iter()
                    .filter(|item| matches!(item, RolloutItem::Compacted(_)))
                    .count(),
            )
            .unwrap_or(u64::MAX),
            InitialHistory::New | InitialHistory::Cleared | InitialHistory::Forked(_) => 0,
        };
        let state_builder = match &initial_history {
            InitialHistory::Resumed(resumed) => metadata::builder_from_items(
                resumed.history.as_slice(),
                resumed.rollout_path.as_path(),
            ),
            InitialHistory::New | InitialHistory::Cleared | InitialHistory::Forked(_) => None,
        };

        // Kick off independent async setup tasks in parallel to reduce startup latency.
        //
        // - initialize RolloutRecorder with new or resumed session info
        // - perform default shell discovery
        // - load history metadata (skipped for subagents)
        let rollout_fut = async {
            if config.ephemeral {
                Ok::<_, anyhow::Error>((None, None))
            } else {
                let state_db_ctx = state_db::init(&config).await;
                let rollout_recorder = RolloutRecorder::new(
                    &config,
                    rollout_params,
                    state_db_ctx.clone(),
                    state_builder.clone(),
                )
                .await?;
                Ok((Some(rollout_recorder), state_db_ctx))
            }
        }
        .instrument(info_span!(
            "session_init.rollout",
            otel.name = "session_init.rollout",
            session_init.ephemeral = config.ephemeral,
        ));

        let is_subagent = matches!(
            session_configuration.session_source,
            SessionSource::SubAgent(_)
        );
        let history_meta_fut = async {
            if is_subagent {
                (0, 0)
            } else {
                crate::message_history::history_metadata(&config).await
            }
        }
        .instrument(info_span!(
            "session_init.history_metadata",
            otel.name = "session_init.history_metadata",
            session_init.is_subagent = is_subagent,
        ));
        let auth_manager_clone = Arc::clone(&auth_manager);
        let config_for_mcp = Arc::clone(&config);
        let mcp_manager_for_mcp = Arc::clone(&mcp_manager);
        let auth_and_mcp_fut = async move {
            let auth = auth_manager_clone.auth().await;
            let mcp_servers = mcp_manager_for_mcp
                .effective_servers(&config_for_mcp, auth.as_ref())
                .await;
            let auth_statuses = compute_auth_statuses(
                mcp_servers.iter(),
                config_for_mcp.mcp_oauth_credentials_store_mode,
            )
            .await;
            (auth, mcp_servers, auth_statuses)
        }
        .instrument(info_span!(
            "session_init.auth_mcp",
            otel.name = "session_init.auth_mcp",
        ));

        // Join all independent futures.
        let (
            rollout_recorder_and_state_db,
            (history_log_id, history_entry_count),
            (auth, mcp_servers, auth_statuses),
        ) = tokio::join!(rollout_fut, history_meta_fut, auth_and_mcp_fut);

        let (rollout_recorder, state_db_ctx) = rollout_recorder_and_state_db.map_err(|e| {
            error!("failed to initialize rollout recorder: {e:#}");
            e
        })?;
        let rollout_path = rollout_recorder
            .as_ref()
            .map(|rec| rec.rollout_path().to_path_buf());

        let mut post_session_configured_events = Vec::<Event>::new();

        for usage in config.features.legacy_feature_usages() {
            post_session_configured_events.push(Event {
                id: INITIAL_SUBMIT_ID.to_owned(),
                msg: EventMsg::DeprecationNotice(DeprecationNoticeEvent {
                    summary: usage.summary.clone(),
                    details: usage.details.clone(),
                }),
            });
        }
        if crate::config::uses_deprecated_instructions_file(&config.config_layer_stack) {
            post_session_configured_events.push(Event {
                id: INITIAL_SUBMIT_ID.to_owned(),
                msg: EventMsg::DeprecationNotice(DeprecationNoticeEvent {
                    summary: "`experimental_instructions_file` is deprecated and ignored. Use `model_instructions_file` instead."
                        .to_string(),
                    details: Some(
                        "Move the setting to `model_instructions_file` in config.toml (or under a profile) to load instructions from a file."
                            .to_string(),
                    ),
                }),
            });
        }
        for message in &config.startup_warnings {
            post_session_configured_events.push(Event {
                id: "".to_owned(),
                msg: EventMsg::Warning(WarningEvent {
                    message: message.clone(),
                }),
            });
        }
        let config_path = config.codex_home.join(CONFIG_TOML_FILE);
        if let Some(event) = unstable_features_warning_event(
            config
                .config_layer_stack
                .effective_config()
                .get("features")
                .and_then(TomlValue::as_table),
            config.suppress_unstable_features_warning,
            &config.features,
            &config_path.display().to_string(),
        ) {
            post_session_configured_events.push(event);
        }
        if config.permissions.approval_policy.value() == AskForApproval::OnFailure {
            post_session_configured_events.push(Event {
                id: "".to_owned(),
                msg: EventMsg::Warning(WarningEvent {
                    message: "`on-failure` approval policy is deprecated and will be removed in a future release. Use `on-request` for interactive approvals or `never` for non-interactive runs.".to_string(),
                }),
            });
        }

        let auth = auth.as_ref();
        let auth_mode = auth.map(CodexAuth::auth_mode).map(TelemetryAuthMode::from);
        let account_id = auth.and_then(CodexAuth::get_account_id);
        let account_email = auth.and_then(CodexAuth::get_account_email);
        let originator = originator().value;
        let terminal_type = user_agent();
        let session_model = session_configuration.collaboration_mode.model().to_string();
        let auth_env_telemetry = collect_auth_env_telemetry(
            &session_configuration.provider,
            auth_manager.codex_api_key_env_enabled(),
        );
        let mut session_telemetry = SessionTelemetry::new(
            conversation_id,
            session_model.as_str(),
            session_model.as_str(),
            account_id.clone(),
            account_email.clone(),
            auth_mode,
            originator.clone(),
            config.otel.log_user_prompt,
            terminal_type.clone(),
            session_configuration.session_source.clone(),
        )
        .with_auth_env(auth_env_telemetry.to_otel_metadata());
        if let Some(service_name) = session_configuration.metrics_service_name.as_deref() {
            session_telemetry = session_telemetry.with_metrics_service_name(service_name);
        }
        let network_proxy_audit_metadata = NetworkProxyAuditMetadata {
            conversation_id: Some(conversation_id.to_string()),
            app_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            user_account_id: account_id,
            auth_mode: auth_mode.map(|mode| mode.to_string()),
            originator: Some(originator),
            user_email: account_email,
            terminal_type: Some(terminal_type),
            model: Some(session_model.clone()),
            slug: Some(session_model),
        };
        config.features.emit_metrics(&session_telemetry);
        session_telemetry.counter(
            THREAD_STARTED_METRIC,
            /*inc*/ 1,
            &[(
                "is_git",
                if get_git_repo_root(&session_configuration.cwd).is_some() {
                    "true"
                } else {
                    "false"
                },
            )],
        );

        session_telemetry.conversation_starts(
            config.model_provider.name.as_str(),
            session_configuration.collaboration_mode.reasoning_effort(),
            config
                .model_reasoning_summary
                .unwrap_or(ReasoningSummaryConfig::Auto),
            config.model_context_window,
            config.model_auto_compact_token_limit,
            config.permissions.approval_policy.value(),
            config.permissions.sandbox_policy.get().clone(),
            mcp_servers.keys().map(String::as_str).collect(),
            config.active_profile.clone(),
        );

        let use_zsh_fork_shell = config.features.enabled(Feature::ShellZshFork);
        let mut default_shell = if let Some(user_shell_override) =
            session_configuration.user_shell_override.clone()
        {
            user_shell_override
        } else if use_zsh_fork_shell {
            let zsh_path = config.zsh_path.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "zsh fork feature enabled, but `zsh_path` is not configured; set `zsh_path` in config.toml"
                )
            })?;
            let zsh_path = zsh_path.to_path_buf();
            shell::get_shell(shell::ShellType::Zsh, Some(&zsh_path)).ok_or_else(|| {
                anyhow::anyhow!(
                    "zsh fork feature enabled, but zsh_path `{}` is not usable; set `zsh_path` to a valid zsh executable",
                    zsh_path.display()
                )
            })?
        } else {
            shell::default_user_shell()
        };
        // Create the mutable state for the Session.
        let shell_snapshot_tx = if config.features.enabled(Feature::ShellSnapshot) {
            if let Some(snapshot) = session_configuration.inherited_shell_snapshot.clone() {
                let (tx, rx) = watch::channel(Some(snapshot));
                default_shell.shell_snapshot = rx;
                tx
            } else {
                ShellSnapshot::start_snapshotting(
                    config.codex_home.clone(),
                    conversation_id,
                    session_configuration.cwd.clone(),
                    &mut default_shell,
                    session_telemetry.clone(),
                )
            }
        } else {
            let (tx, rx) = watch::channel(None);
            default_shell.shell_snapshot = rx;
            tx
        };
        let thread_name =
            thread_title_from_state_db(state_db_ctx.as_ref(), &config.codex_home, conversation_id)
                .instrument(info_span!(
                    "session_init.thread_name_lookup",
                    otel.name = "session_init.thread_name_lookup",
                ))
                .await;
        session_configuration.thread_name = thread_name.clone();
        let state = SessionState::new(session_configuration.clone());
        let managed_network_requirements_enabled = config.managed_network_requirements_enabled();
        let network_approval = Arc::new(NetworkApprovalService::default());
        // The managed proxy can call back into core for allowlist-miss decisions.
        let network_policy_decider_session = if managed_network_requirements_enabled {
            config
                .permissions
                .network
                .as_ref()
                .map(|_| Arc::new(RwLock::new(std::sync::Weak::<Session>::new())))
        } else {
            None
        };
        let blocked_request_observer = if managed_network_requirements_enabled {
            config
                .permissions
                .network
                .as_ref()
                .map(|_| build_blocked_request_observer(Arc::clone(&network_approval)))
        } else {
            None
        };
        let network_policy_decider =
            network_policy_decider_session
                .as_ref()
                .map(|network_policy_decider_session| {
                    build_network_policy_decider(
                        Arc::clone(&network_approval),
                        Arc::clone(network_policy_decider_session),
                    )
                });
        let (network_proxy, session_network_proxy) =
            if let Some(spec) = config.permissions.network.as_ref() {
                let current_exec_policy = exec_policy.current();
                let (network_proxy, session_network_proxy) = Self::start_managed_network_proxy(
                    spec,
                    current_exec_policy.as_ref(),
                    config.permissions.sandbox_policy.get(),
                    network_policy_decider.as_ref().map(Arc::clone),
                    blocked_request_observer.as_ref().map(Arc::clone),
                    managed_network_requirements_enabled,
                    network_proxy_audit_metadata,
                )
                .instrument(info_span!(
                    "session_init.network_proxy",
                    otel.name = "session_init.network_proxy",
                    session_init.managed_network_requirements_enabled =
                        managed_network_requirements_enabled,
                ))
                .await?;
                (Some(network_proxy), Some(session_network_proxy))
            } else {
                (None, None)
            };

        let mut hook_shell_argv =
            default_shell.derive_exec_args("", /*use_login_shell*/ false);
        let hook_shell_program = hook_shell_argv.remove(0);
        let _ = hook_shell_argv.pop();
        let hooks = Hooks::new(HooksConfig {
            legacy_notify_argv: config.notify.clone(),
            feature_enabled: config.features.enabled(Feature::CodexHooks),
            config_layer_stack: Some(config.config_layer_stack.clone()),
            shell_program: Some(hook_shell_program),
            shell_args: hook_shell_argv,
        });
        for warning in hooks.startup_warnings() {
            post_session_configured_events.push(Event {
                id: INITIAL_SUBMIT_ID.to_owned(),
                msg: EventMsg::Warning(WarningEvent {
                    message: warning.clone(),
                }),
            });
        }

        let installation_id = resolve_installation_id(&config.codex_home).await?;
        let analytics_events_client = analytics_events_client.unwrap_or_else(|| {
            AnalyticsEventsClient::new(
                Arc::clone(&auth_manager),
                config.chatgpt_base_url.trim_end_matches('/').to_string(),
                config.analytics_enabled,
            )
        });
        let services = SessionServices {
            // Initialize the MCP connection manager with an uninitialized
            // instance. It will be replaced with one created via
            // McpConnectionManager::new() once all its constructor args are
            // available. This also ensures `SessionConfigured` is emitted
            // before any MCP-related events. It is reasonable to consider
            // changing this to use Option or OnceCell, though the current
            // setup is straightforward enough and performs well.
            mcp_connection_manager: Arc::new(RwLock::new(McpConnectionManager::new_uninitialized(
                &config.permissions.approval_policy,
                &config.permissions.sandbox_policy,
            ))),
            mcp_startup_cancellation_token: Mutex::new(CancellationToken::new()),
            unified_exec_manager: UnifiedExecProcessManager::new(
                config.background_terminal_max_timeout,
            ),
            shell_zsh_path: config.zsh_path.clone(),
            main_execve_wrapper_exe: config.main_execve_wrapper_exe.clone(),
            analytics_events_client,
            hooks,
            rollout: Mutex::new(rollout_recorder),
            user_shell: Arc::new(default_shell),
            agent_identity_manager: Arc::new(AgentIdentityManager::new(
                config.as_ref(),
                Arc::clone(&auth_manager),
                session_configuration.session_source.clone(),
            )),
            shell_snapshot_tx,
            show_raw_agent_reasoning: config.show_raw_agent_reasoning,
            exec_policy,
            auth_manager: Arc::clone(&auth_manager),
            session_telemetry,
            models_manager: Arc::clone(&models_manager),
            tool_approvals: Mutex::new(ApprovalStore::default()),
            guardian_rejections: Mutex::new(HashMap::new()),
            skills_manager,
            plugins_manager: Arc::clone(&plugins_manager),
            mcp_manager: Arc::clone(&mcp_manager),
            skills_watcher,
            agent_control,
            network_proxy,
            network_approval: Arc::clone(&network_approval),
            state_db: state_db_ctx.clone(),
            thread_store: LocalThreadStore::new(RolloutConfig::from_view(config.as_ref())),
            model_client: ModelClient::new(
                Some(Arc::clone(&auth_manager)),
                conversation_id,
                installation_id,
                session_configuration.provider.clone(),
                session_configuration.session_source.clone(),
                config.model_verbosity,
                config.features.enabled(Feature::EnableRequestCompression),
                config.features.enabled(Feature::RuntimeMetrics),
                Self::build_model_client_beta_features_header(config.as_ref()),
            ),
            code_mode_service: crate::tools::code_mode::CodeModeService::new(
                config.js_repl_node_path.clone(),
            ),
            environment,
        };
        services
            .model_client
            .set_window_generation(window_generation);
        let js_repl = Arc::new(JsReplHandle::with_node_path(
            config.js_repl_node_path.clone(),
            config.js_repl_node_module_dirs.clone(),
        ));
        let (out_of_band_elicitation_paused, _out_of_band_elicitation_paused_rx) =
            watch::channel(false);

        let (mailbox, mailbox_rx) = Mailbox::new();
        let sess = Arc::new(Session {
            conversation_id,
            tx_event: tx_event.clone(),
            agent_status,
            out_of_band_elicitation_paused,
            state: Mutex::new(state),
            managed_network_proxy_refresh_lock: Mutex::new(()),
            features: config.features.clone(),
            pending_mcp_server_refresh_config: Mutex::new(None),
            conversation: Arc::new(RealtimeConversationManager::new()),
            active_turn: Mutex::new(None),
            mailbox,
            mailbox_rx: Mutex::new(mailbox_rx),
            idle_pending_input: Mutex::new(Vec::new()),
            guardian_review_session: GuardianReviewSessionManager::default(),
            services,
            js_repl,
            next_internal_sub_id: AtomicU64::new(0),
        });
        if let Some(network_policy_decider_session) = network_policy_decider_session {
            let mut guard = network_policy_decider_session.write().await;
            *guard = Arc::downgrade(&sess);
        }
        // Dispatch the SessionConfiguredEvent first and then report any errors.
        // If resuming, include converted initial messages in the payload so UIs can render them immediately.
        let initial_messages = initial_history.get_event_msgs();
        let events = std::iter::once(Event {
            id: INITIAL_SUBMIT_ID.to_owned(),
            msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
                session_id: conversation_id,
                forked_from_id,
                thread_name: session_configuration.thread_name.clone(),
                model: session_configuration.collaboration_mode.model().to_string(),
                model_provider_id: config.model_provider_id.clone(),
                service_tier: session_configuration.service_tier,
                approval_policy: session_configuration.approval_policy.value(),
                approvals_reviewer: session_configuration.approvals_reviewer,
                sandbox_policy: session_configuration.sandbox_policy.get().clone(),
                cwd: session_configuration.cwd.clone(),
                reasoning_effort: session_configuration.collaboration_mode.reasoning_effort(),
                history_log_id,
                history_entry_count,
                initial_messages,
                network_proxy: session_network_proxy,
                rollout_path,
            }),
        })
        .chain(post_session_configured_events.into_iter());
        for event in events {
            sess.send_event_raw(event).await;
        }

        // Start the watcher after SessionConfigured so it cannot emit earlier events.
        sess.start_skills_watcher_listener();
        sess.start_agent_identity_registration();
        let mut required_mcp_servers: Vec<String> = mcp_servers
            .iter()
            .filter(|(_, server)| server.enabled && server.required)
            .map(|(name, _)| name.clone())
            .collect();
        required_mcp_servers.sort();
        let enabled_mcp_server_count = mcp_servers.values().filter(|server| server.enabled).count();
        let required_mcp_server_count = required_mcp_servers.len();
        let tool_plugin_provenance = mcp_manager.tool_plugin_provenance(config.as_ref()).await;
        {
            let mut cancel_guard = sess.services.mcp_startup_cancellation_token.lock().await;
            cancel_guard.cancel();
            *cancel_guard = CancellationToken::new();
        }
        let (mcp_connection_manager, cancel_token) = McpConnectionManager::new(
            &mcp_servers,
            config.mcp_oauth_credentials_store_mode,
            auth_statuses.clone(),
            &session_configuration.approval_policy,
            INITIAL_SUBMIT_ID.to_owned(),
            tx_event.clone(),
            session_configuration.sandbox_policy.get().clone(),
            config.codex_home.to_path_buf(),
            codex_apps_tools_cache_key(auth),
            tool_plugin_provenance,
        )
        .instrument(info_span!(
            "session_init.mcp_manager_init",
            otel.name = "session_init.mcp_manager_init",
            session_init.enabled_mcp_server_count = enabled_mcp_server_count,
            session_init.required_mcp_server_count = required_mcp_server_count,
        ))
        .await;
        {
            let mut manager_guard = sess.services.mcp_connection_manager.write().await;
            *manager_guard = mcp_connection_manager;
        }
        {
            let mut cancel_guard = sess.services.mcp_startup_cancellation_token.lock().await;
            if cancel_guard.is_cancelled() {
                cancel_token.cancel();
            }
            *cancel_guard = cancel_token;
        }
        if !required_mcp_servers.is_empty() {
            let failures = sess
                .services
                .mcp_connection_manager
                .read()
                .await
                .required_startup_failures(&required_mcp_servers)
                .instrument(info_span!(
                    "session_init.required_mcp_wait",
                    otel.name = "session_init.required_mcp_wait",
                    session_init.required_mcp_server_count = required_mcp_server_count,
                ))
                .await;
            if !failures.is_empty() {
                let details = failures
                    .iter()
                    .map(|failure| format!("{}: {}", failure.server, failure.error))
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(anyhow::anyhow!(
                    "required MCP servers failed to initialize: {details}"
                ));
            }
        }
        sess.schedule_startup_prewarm(session_configuration.base_instructions.clone())
            .await;
        let session_start_source = match &initial_history {
            InitialHistory::Resumed(_) => codex_hooks::SessionStartSource::Resume,
            InitialHistory::New | InitialHistory::Forked(_) => {
                codex_hooks::SessionStartSource::Startup
            }
            InitialHistory::Cleared => codex_hooks::SessionStartSource::Clear,
        };

        // record_initial_history can emit events. We record only after the SessionConfiguredEvent is emitted.
        sess.record_initial_history(initial_history).await;
        {
            let mut state = sess.state.lock().await;
            state.set_pending_session_start_source(Some(session_start_source));
        }

        memories::start_memories_startup_task(
            &sess,
            Arc::clone(&config),
            &session_configuration.session_source,
        );

        Ok(sess)
    }

    pub(crate) fn get_tx_event(&self) -> Sender<Event> {
        self.tx_event.clone()
    }

    pub(crate) fn state_db(&self) -> Option<state_db::StateDbHandle> {
        self.services.state_db.clone()
    }

    /// Flush rollout writes and return the final durability-barrier result.
    pub(crate) async fn flush_rollout(&self) -> std::io::Result<()> {
        let recorder = {
            let guard = self.services.rollout.lock().await;
            guard.clone()
        };
        if let Some(recorder) = recorder {
            recorder.flush().await
        } else {
            Ok(())
        }
    }

    pub(crate) async fn try_ensure_rollout_materialized(&self) -> std::io::Result<()> {
        let recorder = {
            let guard = self.services.rollout.lock().await;
            guard.clone()
        };
        if let Some(rec) = recorder {
            rec.persist().await?;
        }
        Ok(())
    }

    pub(crate) async fn ensure_rollout_materialized(&self) {
        if let Err(e) = self.try_ensure_rollout_materialized().await {
            warn!("failed to materialize rollout recorder: {e}");
        }
    }

    fn next_internal_sub_id(&self) -> String {
        let id = self
            .next_internal_sub_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("auto-compact-{id}")
    }

    pub(crate) async fn route_realtime_text_input(self: &Arc<Self>, text: String) {
        handlers::user_input_or_turn_inner(
            self,
            self.next_internal_sub_id(),
            Op::UserInput {
                items: vec![UserInput::Text {
                    text,
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
                responsesapi_client_metadata: None,
            },
            /*mirror_user_text_to_realtime*/ None,
        )
        .await;
    }

    pub(crate) async fn get_total_token_usage(&self) -> i64 {
        let state = self.state.lock().await;
        state.get_total_token_usage(state.server_reasoning_included())
    }

    pub(crate) async fn get_total_token_usage_breakdown(&self) -> TotalTokenUsageBreakdown {
        let state = self.state.lock().await;
        state.history.get_total_token_usage_breakdown()
    }

    pub(crate) async fn total_token_usage(&self) -> Option<TokenUsage> {
        let state = self.state.lock().await;
        state.token_info().map(|info| info.total_token_usage)
    }

    pub(crate) async fn get_estimated_token_count(
        &self,
        turn_context: &TurnContext,
    ) -> Option<i64> {
        let state = self.state.lock().await;
        state.history.estimate_token_count(turn_context)
    }

    pub(crate) async fn get_base_instructions(&self) -> BaseInstructions {
        let state = self.state.lock().await;
        BaseInstructions {
            text: state.session_configuration.base_instructions.clone(),
        }
    }

    // Merges connector IDs into the session-level explicit connector selection.
    pub(crate) async fn merge_connector_selection(
        &self,
        connector_ids: HashSet<String>,
    ) -> HashSet<String> {
        let mut state = self.state.lock().await;
        state.merge_connector_selection(connector_ids)
    }

    // Returns the connector IDs currently selected for this session.
    pub(crate) async fn get_connector_selection(&self) -> HashSet<String> {
        let state = self.state.lock().await;
        state.get_connector_selection()
    }

    // Clears connector IDs that were accumulated for explicit selection.
    pub(crate) async fn clear_connector_selection(&self) {
        let mut state = self.state.lock().await;
        state.clear_connector_selection();
    }

    async fn record_initial_history(&self, conversation_history: InitialHistory) {
        let turn_context = self.new_default_turn().await;
        let is_subagent = {
            let state = self.state.lock().await;
            matches!(
                state.session_configuration.session_source,
                SessionSource::SubAgent(_)
            )
        };
        let has_prior_user_turns = initial_history_has_prior_user_turns(&conversation_history);
        {
            let mut state = self.state.lock().await;
            state.set_next_turn_is_first(!has_prior_user_turns);
        }
        match conversation_history {
            InitialHistory::New | InitialHistory::Cleared => {
                // Defer initial context insertion until the first real turn starts so
                // turn/start overrides can be merged before we write model-visible context.
                self.set_previous_turn_settings(/*previous_turn_settings*/ None)
                    .await;
            }
            InitialHistory::Resumed(resumed_history) => {
                let rollout_items = resumed_history.history;
                let previous_turn_settings = self
                    .apply_rollout_reconstruction(&turn_context, &rollout_items)
                    .await;

                // If resuming, warn when the last recorded model differs from the current one.
                let curr: &str = turn_context.model_info.slug.as_str();
                if let Some(prev) = previous_turn_settings
                    .as_ref()
                    .map(|settings| settings.model.as_str())
                    .filter(|model| *model != curr)
                {
                    warn!("resuming session with different model: previous={prev}, current={curr}");
                    self.send_event(
                        &turn_context,
                        EventMsg::Warning(WarningEvent {
                            message: format!(
                                "This session was recorded with model `{prev}` but is resuming with `{curr}`. \
                         Consider switching back to `{prev}` as it may affect Codex performance."
                            ),
                        }),
                    )
                    .await;
                }

                // Seed usage info from the recorded rollout so UIs can show token counts
                // immediately on resume/fork.
                if let Some(info) = Self::last_token_info_from_rollout(&rollout_items) {
                    let mut state = self.state.lock().await;
                    state.set_token_info(Some(info));
                }

                // Defer seeding the session's initial context until the first turn starts so
                // turn/start overrides can be merged before we write to the rollout.
                if !is_subagent {
                    let _ = self.flush_rollout().await;
                }
            }
            InitialHistory::Forked(rollout_items) => {
                self.apply_rollout_reconstruction(&turn_context, &rollout_items)
                    .await;

                // Seed usage info from the recorded rollout so UIs can show token counts
                // immediately on resume/fork.
                if let Some(info) = Self::last_token_info_from_rollout(&rollout_items) {
                    let mut state = self.state.lock().await;
                    state.set_token_info(Some(info));
                }

                // If persisting, persist all rollout items as-is (recorder filters)
                if !rollout_items.is_empty() {
                    self.persist_rollout_items(&rollout_items).await;
                }

                // Forked threads should remain file-backed immediately after startup.
                self.ensure_rollout_materialized().await;

                // Flush after seeding history and any persisted rollout copy.
                if !is_subagent {
                    let _ = self.flush_rollout().await;
                }
            }
        }
    }

    async fn apply_rollout_reconstruction(
        &self,
        turn_context: &TurnContext,
        rollout_items: &[RolloutItem],
    ) -> Option<PreviousTurnSettings> {
        let reconstructed_rollout = self
            .reconstruct_history_from_rollout(turn_context, rollout_items)
            .await;
        let previous_turn_settings = reconstructed_rollout.previous_turn_settings.clone();
        self.replace_history(
            reconstructed_rollout.history,
            reconstructed_rollout.reference_context_item,
        )
        .await;
        self.set_previous_turn_settings(previous_turn_settings.clone())
            .await;
        previous_turn_settings
    }

    fn last_token_info_from_rollout(rollout_items: &[RolloutItem]) -> Option<TokenUsageInfo> {
        rollout_items.iter().rev().find_map(|item| match item {
            RolloutItem::EventMsg(EventMsg::TokenCount(ev)) => ev.info.clone(),
            _ => None,
        })
    }

    async fn previous_turn_settings(&self) -> Option<PreviousTurnSettings> {
        let state = self.state.lock().await;
        state.previous_turn_settings()
    }

    pub(crate) async fn set_previous_turn_settings(
        &self,
        previous_turn_settings: Option<PreviousTurnSettings>,
    ) {
        let mut state = self.state.lock().await;
        state.set_previous_turn_settings(previous_turn_settings);
    }

    fn maybe_refresh_shell_snapshot_for_cwd(
        &self,
        previous_cwd: &AbsolutePathBuf,
        next_cwd: &AbsolutePathBuf,
        codex_home: &AbsolutePathBuf,
        session_source: &SessionSource,
    ) {
        if previous_cwd == next_cwd {
            return;
        }

        if !self.features.enabled(Feature::ShellSnapshot) {
            return;
        }

        if matches!(
            session_source,
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. })
        ) {
            return;
        }

        ShellSnapshot::refresh_snapshot(
            codex_home.clone(),
            self.conversation_id,
            next_cwd.clone(),
            self.services.user_shell.as_ref().clone(),
            self.services.shell_snapshot_tx.clone(),
            self.services.session_telemetry.clone(),
        );
    }

    pub(crate) async fn update_settings(
        &self,
        updates: SessionSettingsUpdate,
    ) -> ConstraintResult<()> {
        let mut state = self.state.lock().await;

        match state.session_configuration.apply(&updates) {
            Ok(updated) => {
                let previous_cwd = state.session_configuration.cwd.clone();
                let sandbox_policy_changed =
                    state.session_configuration.sandbox_policy != updated.sandbox_policy;
                let next_cwd = updated.cwd.clone();
                let codex_home = updated.codex_home.clone();
                let session_source = updated.session_source.clone();
                state.session_configuration = updated;
                drop(state);

                self.maybe_refresh_shell_snapshot_for_cwd(
                    &previous_cwd,
                    &next_cwd,
                    &codex_home,
                    &session_source,
                );
                if sandbox_policy_changed {
                    self.refresh_managed_network_proxy_for_current_sandbox_policy()
                        .await;
                }

                Ok(())
            }
            Err(err) => {
                warn!("rejected session settings update: {err}");
                Err(err)
            }
        }
    }

    pub(crate) async fn new_turn_with_sub_id(
        &self,
        sub_id: String,
        updates: SessionSettingsUpdate,
    ) -> ConstraintResult<Arc<TurnContext>> {
        let (
            session_configuration,
            sandbox_policy_changed,
            previous_cwd,
            codex_home,
            session_source,
        ) = {
            let mut state = self.state.lock().await;
            match state.session_configuration.clone().apply(&updates) {
                Ok(next) => {
                    let previous_cwd = state.session_configuration.cwd.clone();
                    let sandbox_policy_changed =
                        state.session_configuration.sandbox_policy != next.sandbox_policy;
                    let codex_home = next.codex_home.clone();
                    let session_source = next.session_source.clone();
                    state.session_configuration = next.clone();
                    (
                        next,
                        sandbox_policy_changed,
                        previous_cwd,
                        codex_home,
                        session_source,
                    )
                }
                Err(err) => {
                    drop(state);
                    self.send_event_raw(Event {
                        id: sub_id.clone(),
                        msg: EventMsg::Error(ErrorEvent {
                            message: err.to_string(),
                            codex_error_info: Some(CodexErrorInfo::BadRequest),
                        }),
                    })
                    .await;
                    return Err(err);
                }
            }
        };

        self.maybe_refresh_shell_snapshot_for_cwd(
            &previous_cwd,
            &session_configuration.cwd,
            &codex_home,
            &session_source,
        );

        if sandbox_policy_changed {
            self.refresh_managed_network_proxy_for_current_sandbox_policy()
                .await;
        }

        Ok(self
            .new_turn_from_configuration(
                sub_id,
                session_configuration,
                updates.final_output_json_schema,
            )
            .await)
    }

    async fn new_turn_from_configuration(
        &self,
        sub_id: String,
        session_configuration: SessionConfiguration,
        final_output_json_schema: Option<Option<Value>>,
    ) -> Arc<TurnContext> {
        let per_turn_config = Self::build_per_turn_config(&session_configuration);
        {
            let mcp_connection_manager = self.services.mcp_connection_manager.read().await;
            mcp_connection_manager.set_approval_policy(&session_configuration.approval_policy);
            mcp_connection_manager
                .set_sandbox_policy(per_turn_config.permissions.sandbox_policy.get());
        }

        let model_info = self
            .services
            .models_manager
            .get_model_info(
                session_configuration.collaboration_mode.model(),
                &per_turn_config.to_models_manager_config(),
            )
            .await;
        let plugin_outcome = self
            .services
            .plugins_manager
            .plugins_for_config(&per_turn_config)
            .await;
        let effective_skill_roots = plugin_outcome.effective_skill_roots();
        let skills_input = skills_load_input_from_config(&per_turn_config, effective_skill_roots);
        let fs = self
            .services
            .environment
            .as_ref()
            .map(|environment| environment.get_filesystem());
        let skills_outcome = Arc::new(
            self.services
                .skills_manager
                .skills_for_config(&skills_input, fs)
                .await,
        );
        let mut turn_context: TurnContext = Self::make_turn_context(
            self.conversation_id,
            Some(Arc::clone(&self.services.auth_manager)),
            &self.services.session_telemetry,
            session_configuration.provider.clone(),
            &session_configuration,
            self.services.user_shell.as_ref(),
            self.services.shell_zsh_path.as_ref(),
            self.services.main_execve_wrapper_exe.as_ref(),
            per_turn_config,
            model_info,
            &self.services.models_manager,
            self.services
                .network_proxy
                .as_ref()
                .map(StartedNetworkProxy::proxy),
            self.services.environment.clone(),
            sub_id,
            Arc::clone(&self.js_repl),
            skills_outcome,
        );
        turn_context.realtime_active = self.conversation.running_state().await.is_some();

        if let Some(final_schema) = final_output_json_schema {
            turn_context.final_output_json_schema = final_schema;
        }
        let turn_context = Arc::new(turn_context);
        turn_context.turn_metadata_state.spawn_git_enrichment_task();
        turn_context
    }

    pub(crate) async fn maybe_emit_unknown_model_warning_for_turn(&self, tc: &TurnContext) {
        if tc.model_info.used_fallback_model_metadata {
            self.send_event(
                tc,
                EventMsg::Warning(WarningEvent {
                    message: format!(
                        "Model metadata for `{}` not found. Defaulting to fallback metadata; this can degrade performance and cause issues.",
                        tc.model_info.slug
                    ),
                }),
            )
            .await;
        }
    }

    pub(crate) async fn new_default_turn(&self) -> Arc<TurnContext> {
        self.new_default_turn_with_sub_id(self.next_internal_sub_id())
            .await
    }

    pub(crate) async fn set_session_startup_prewarm(
        &self,
        startup_prewarm: SessionStartupPrewarmHandle,
    ) {
        let mut state = self.state.lock().await;
        state.set_session_startup_prewarm(startup_prewarm);
    }

    pub(crate) async fn take_session_startup_prewarm(&self) -> Option<SessionStartupPrewarmHandle> {
        let mut state = self.state.lock().await;
        state.take_session_startup_prewarm()
    }

    pub(crate) async fn get_config(&self) -> std::sync::Arc<Config> {
        let state = self.state.lock().await;
        state
            .session_configuration
            .original_config_do_not_use
            .clone()
    }

    pub(crate) async fn provider(&self) -> ModelProviderInfo {
        let state = self.state.lock().await;
        state.session_configuration.provider.clone()
    }

    pub(crate) async fn reload_user_config_layer(&self) {
        let config_toml_path = {
            let state = self.state.lock().await;
            state
                .session_configuration
                .codex_home
                .join(CONFIG_TOML_FILE)
        };

        let user_config = match std::fs::read_to_string(&config_toml_path) {
            Ok(contents) => match toml::from_str::<toml::Value>(&contents) {
                Ok(config) => config,
                Err(err) => {
                    warn!("failed to parse user config while reloading layer: {err}");
                    return;
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                toml::Value::Table(Default::default())
            }
            Err(err) => {
                warn!("failed to read user config while reloading layer: {err}");
                return;
            }
        };

        let mut state = self.state.lock().await;
        let mut config = (*state.session_configuration.original_config_do_not_use).clone();
        config.config_layer_stack = config
            .config_layer_stack
            .with_user_config(&config_toml_path, user_config);
        state.session_configuration.original_config_do_not_use = Arc::new(config);
        self.services.skills_manager.clear_cache();
        self.services.plugins_manager.clear_cache();
    }

    pub(crate) async fn new_default_turn_with_sub_id(&self, sub_id: String) -> Arc<TurnContext> {
        let session_configuration = {
            let state = self.state.lock().await;
            state.session_configuration.clone()
        };
        self.new_turn_from_configuration(
            sub_id,
            session_configuration,
            /*final_output_json_schema*/ None,
        )
        .await
    }

    async fn build_settings_update_items(
        &self,
        reference_context_item: Option<&TurnContextItem>,
        current_context: &TurnContext,
    ) -> Vec<ResponseItem> {
        // TODO: Make context updates a pure diff of persisted previous/current TurnContextItem
        // state so replay/backtracking is deterministic. Runtime inputs that affect model-visible
        // context (shell, exec policy, feature gates, previous-turn bridge) should be persisted
        // state or explicit non-state replay events.
        let previous_turn_settings = {
            let state = self.state.lock().await;
            state.previous_turn_settings()
        };
        let shell = self.user_shell();
        let exec_policy = self.services.exec_policy.current();
        crate::context_manager::updates::build_settings_update_items(
            reference_context_item,
            previous_turn_settings.as_ref(),
            current_context,
            shell.as_ref(),
            exec_policy.as_ref(),
            self.features.enabled(Feature::Personality),
        )
    }

    /// Persist the event to rollout and send it to clients.
    pub(crate) async fn send_event(&self, turn_context: &TurnContext, msg: EventMsg) {
        let legacy_source = msg.clone();
        let event = Event {
            id: turn_context.sub_id.clone(),
            msg,
        };
        self.send_event_raw(event).await;
        self.maybe_notify_parent_of_terminal_turn(turn_context, &legacy_source)
            .await;
        self.maybe_mirror_event_text_to_realtime(&legacy_source)
            .await;
        self.maybe_clear_realtime_handoff_for_event(&legacy_source)
            .await;

        let show_raw_agent_reasoning = self.show_raw_agent_reasoning();
        for legacy in legacy_source.as_legacy_events(show_raw_agent_reasoning) {
            let legacy_event = Event {
                id: turn_context.sub_id.clone(),
                msg: legacy,
            };
            self.send_event_raw(legacy_event).await;
        }
    }

    /// Forwards terminal turn events from spawned MultiAgentV2 children to their direct parent.
    async fn maybe_notify_parent_of_terminal_turn(
        &self,
        turn_context: &TurnContext,
        msg: &EventMsg,
    ) {
        if !self.enabled(Feature::MultiAgentV2) {
            return;
        }

        if !matches!(msg, EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_)) {
            return;
        }

        let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            agent_path: Some(child_agent_path),
            ..
        }) = &turn_context.session_source
        else {
            return;
        };

        let Some(status) = agent_status_from_event(msg) else {
            return;
        };
        if !is_final(&status) {
            return;
        }

        self.forward_child_completion_to_parent(*parent_thread_id, child_agent_path, status)
            .await;
    }

    /// Sends the standard completion envelope from a spawned MultiAgentV2 child to its parent.
    async fn forward_child_completion_to_parent(
        &self,
        parent_thread_id: ThreadId,
        child_agent_path: &codex_protocol::AgentPath,
        status: AgentStatus,
    ) {
        let Some(parent_agent_path) = child_agent_path
            .as_str()
            .rsplit_once('/')
            .and_then(|(parent, _)| codex_protocol::AgentPath::try_from(parent).ok())
        else {
            return;
        };

        let message = format_subagent_notification_message(child_agent_path.as_str(), &status);
        let communication = InterAgentCommunication::new(
            child_agent_path.clone(),
            parent_agent_path,
            Vec::new(),
            message,
            /*trigger_turn*/ false,
        );
        if let Err(err) = self
            .services
            .agent_control
            .send_inter_agent_communication(parent_thread_id, communication)
            .await
        {
            debug!("failed to notify parent thread {parent_thread_id}: {err}");
        }
    }

    async fn maybe_mirror_event_text_to_realtime(&self, msg: &EventMsg) {
        let Some(text) = realtime_text_for_event(msg) else {
            return;
        };
        if self.conversation.running_state().await.is_none()
            || self.conversation.active_handoff_id().await.is_none()
        {
            return;
        }
        if let Err(err) = self.conversation.handoff_out(text).await {
            debug!("failed to mirror event text to realtime conversation: {err}");
        }
    }

    async fn maybe_clear_realtime_handoff_for_event(&self, msg: &EventMsg) {
        if !matches!(msg, EventMsg::TurnComplete(_)) {
            return;
        }
        if let Err(err) = self.conversation.handoff_complete().await {
            debug!("failed to finalize realtime handoff output: {err}");
        }
        self.conversation.clear_active_handoff().await;
    }

    pub(crate) async fn send_event_raw(&self, event: Event) {
        // Persist the event into rollout (recorder filters as needed)
        let rollout_items = vec![RolloutItem::EventMsg(event.msg.clone())];
        self.persist_rollout_items(&rollout_items).await;
        self.deliver_event_raw(event).await;
    }

    async fn deliver_event_raw(&self, event: Event) {
        // Record the last known agent status.
        if let Some(status) = agent_status_from_event(&event.msg) {
            self.agent_status.send_replace(status);
        }
        if let Err(e) = self.tx_event.send(event).await {
            debug!("dropping event because channel is closed: {e}");
        }
    }

    pub(crate) async fn emit_turn_item_started(&self, turn_context: &TurnContext, item: &TurnItem) {
        self.send_event(
            turn_context,
            EventMsg::ItemStarted(ItemStartedEvent {
                thread_id: self.conversation_id,
                turn_id: turn_context.sub_id.clone(),
                item: item.clone(),
            }),
        )
        .await;
    }

    pub(crate) async fn emit_turn_item_completed(
        &self,
        turn_context: &TurnContext,
        item: TurnItem,
    ) {
        record_turn_ttfm_metric(turn_context, &item).await;
        self.send_event(
            turn_context,
            EventMsg::ItemCompleted(ItemCompletedEvent {
                thread_id: self.conversation_id,
                turn_id: turn_context.sub_id.clone(),
                item,
            }),
        )
        .await;
    }

    /// Adds an execpolicy amendment to both the in-memory and on-disk policies so future
    /// commands can use the newly approved prefix.
    pub(crate) async fn persist_execpolicy_amendment(
        &self,
        amendment: &ExecPolicyAmendment,
    ) -> Result<(), ExecPolicyUpdateError> {
        let codex_home = self
            .state
            .lock()
            .await
            .session_configuration
            .codex_home()
            .clone();

        self.services
            .exec_policy
            .append_amendment_and_update(&codex_home, amendment)
            .await?;

        Ok(())
    }

    pub(crate) async fn turn_context_for_sub_id(&self, sub_id: &str) -> Option<Arc<TurnContext>> {
        let active = self.active_turn.lock().await;
        active
            .as_ref()
            .and_then(|turn| turn.tasks.get(sub_id))
            .map(|task| Arc::clone(&task.turn_context))
    }

    async fn active_turn_context_and_cancellation_token(
        &self,
    ) -> Option<(Arc<TurnContext>, CancellationToken)> {
        let active = self.active_turn.lock().await;
        let (_, task) = active.as_ref()?.tasks.first()?;
        Some((
            Arc::clone(&task.turn_context),
            task.cancellation_token.child_token(),
        ))
    }

    pub(crate) async fn record_execpolicy_amendment_message(
        &self,
        sub_id: &str,
        amendment: &ExecPolicyAmendment,
    ) {
        let Some(prefixes) = format_allow_prefixes(vec![amendment.command.clone()]) else {
            warn!("execpolicy amendment for {sub_id} had no command prefix");
            return;
        };
        let text = format!("Approved command prefix saved:\n{prefixes}");
        let message: ResponseItem = DeveloperInstructions::new(text.clone()).into();

        if let Some(turn_context) = self.turn_context_for_sub_id(sub_id).await {
            self.record_conversation_items(&turn_context, std::slice::from_ref(&message))
                .await;
            return;
        }

        if self
            .inject_response_items(vec![ResponseInputItem::Message {
                role: "developer".to_string(),
                content: vec![ContentItem::InputText { text }],
            }])
            .await
            .is_err()
        {
            warn!("no active turn found to record execpolicy amendment message for {sub_id}");
        }
    }

    pub(crate) async fn persist_network_policy_amendment(
        &self,
        amendment: &NetworkPolicyAmendment,
        network_approval_context: &NetworkApprovalContext,
    ) -> anyhow::Result<()> {
        let _refresh_guard = self.managed_network_proxy_refresh_lock.lock().await;
        let host =
            Self::validated_network_policy_amendment_host(amendment, network_approval_context)?;
        let codex_home = self
            .state
            .lock()
            .await
            .session_configuration
            .codex_home()
            .clone();
        let execpolicy_amendment =
            execpolicy_network_rule_amendment(amendment, network_approval_context, &host);

        if let Some(started_network_proxy) = self.services.network_proxy.as_ref() {
            let proxy = started_network_proxy.proxy();
            match amendment.action {
                NetworkPolicyRuleAction::Allow => proxy
                    .add_allowed_domain(&host)
                    .await
                    .map_err(|err| anyhow::anyhow!("failed to update runtime allowlist: {err}"))?,
                NetworkPolicyRuleAction::Deny => proxy
                    .add_denied_domain(&host)
                    .await
                    .map_err(|err| anyhow::anyhow!("failed to update runtime denylist: {err}"))?,
            }
        }

        self.services
            .exec_policy
            .append_network_rule_and_update(
                &codex_home,
                &host,
                execpolicy_amendment.protocol,
                execpolicy_amendment.decision,
                Some(execpolicy_amendment.justification),
            )
            .await
            .map_err(|err| {
                anyhow::anyhow!("failed to persist network policy amendment to execpolicy: {err}")
            })?;

        Ok(())
    }

    fn validated_network_policy_amendment_host(
        amendment: &NetworkPolicyAmendment,
        network_approval_context: &NetworkApprovalContext,
    ) -> anyhow::Result<String> {
        let approved_host = normalize_host(&network_approval_context.host);
        let amendment_host = normalize_host(&amendment.host);
        if amendment_host != approved_host {
            return Err(anyhow::anyhow!(
                "network policy amendment host '{}' does not match approved host '{}'",
                amendment.host,
                network_approval_context.host
            ));
        }
        Ok(approved_host)
    }

    pub(crate) async fn record_network_policy_amendment_message(
        &self,
        sub_id: &str,
        amendment: &NetworkPolicyAmendment,
    ) {
        let (action, list_name) = match amendment.action {
            NetworkPolicyRuleAction::Allow => ("Allowed", "allowlist"),
            NetworkPolicyRuleAction::Deny => ("Denied", "denylist"),
        };
        let text = format!(
            "{action} network rule saved in execpolicy ({list_name}): {}",
            amendment.host
        );
        let message: ResponseItem = DeveloperInstructions::new(text.clone()).into();

        if let Some(turn_context) = self.turn_context_for_sub_id(sub_id).await {
            self.record_conversation_items(&turn_context, std::slice::from_ref(&message))
                .await;
            return;
        }

        if self
            .inject_response_items(vec![ResponseInputItem::Message {
                role: "developer".to_string(),
                content: vec![ContentItem::InputText { text }],
            }])
            .await
            .is_err()
        {
            warn!("no active turn found to record network policy amendment message for {sub_id}");
        }
    }

    /// Emit an exec approval request event and await the user's decision.
    ///
    /// The request is keyed by `call_id` + `approval_id` so matching responses
    /// are delivered to the correct in-flight turn. If the pending approval is
    /// cleared before a response arrives, treat it as an abort so interrupted
    /// turns do not continue on a synthetic denial.
    ///
    /// Note that if `available_decisions` is `None`, then the other fields will
    /// be used to derive the available decisions via
    /// [ExecApprovalRequestEvent::default_available_decisions].
    #[allow(clippy::too_many_arguments)]
    pub async fn request_command_approval(
        &self,
        turn_context: &TurnContext,
        call_id: String,
        approval_id: Option<String>,
        command: Vec<String>,
        cwd: AbsolutePathBuf,
        reason: Option<String>,
        network_approval_context: Option<NetworkApprovalContext>,
        proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
        additional_permissions: Option<PermissionProfile>,
        available_decisions: Option<Vec<ReviewDecision>>,
    ) -> ReviewDecision {
        //  command-level approvals use `call_id`.
        // `approval_id` is only present for subcommand callbacks (execve intercept)
        let effective_approval_id = approval_id.clone().unwrap_or_else(|| call_id.clone());
        // Add the tx_approve callback to the map before sending the request.
        let (tx_approve, rx_approve) = oneshot::channel();
        let prev_entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    ts.insert_pending_approval(effective_approval_id.clone(), tx_approve)
                }
                None => None,
            }
        };
        if prev_entry.is_some() {
            warn!("Overwriting existing pending approval for call_id: {effective_approval_id}");
        }

        let parsed_cmd = parse_command(&command);
        let proposed_network_policy_amendments = network_approval_context.as_ref().map(|context| {
            vec![
                NetworkPolicyAmendment {
                    host: context.host.clone(),
                    action: NetworkPolicyRuleAction::Allow,
                },
                NetworkPolicyAmendment {
                    host: context.host.clone(),
                    action: NetworkPolicyRuleAction::Deny,
                },
            ]
        });
        let available_decisions = available_decisions.unwrap_or_else(|| {
            ExecApprovalRequestEvent::default_available_decisions(
                network_approval_context.as_ref(),
                proposed_execpolicy_amendment.as_ref(),
                proposed_network_policy_amendments.as_deref(),
                additional_permissions.as_ref(),
            )
        });
        let event = EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
            call_id,
            approval_id,
            turn_id: turn_context.sub_id.clone(),
            command,
            cwd,
            reason,
            network_approval_context,
            proposed_execpolicy_amendment,
            proposed_network_policy_amendments,
            additional_permissions,
            available_decisions: Some(available_decisions),
            parsed_cmd,
        });
        self.send_event(turn_context, event).await;
        rx_approve.await.unwrap_or(ReviewDecision::Abort)
    }

    pub async fn request_patch_approval(
        &self,
        turn_context: &TurnContext,
        call_id: String,
        changes: HashMap<PathBuf, FileChange>,
        reason: Option<String>,
        grant_root: Option<PathBuf>,
    ) -> oneshot::Receiver<ReviewDecision> {
        // Add the tx_approve callback to the map before sending the request.
        let (tx_approve, rx_approve) = oneshot::channel();
        let approval_id = call_id.clone();
        let prev_entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    ts.insert_pending_approval(approval_id.clone(), tx_approve)
                }
                None => None,
            }
        };
        if prev_entry.is_some() {
            warn!("Overwriting existing pending approval for call_id: {approval_id}");
        }

        let event = EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
            call_id,
            turn_id: turn_context.sub_id.clone(),
            changes,
            reason,
            grant_root,
        });
        self.send_event(turn_context, event).await;
        rx_approve
    }

    pub async fn request_permissions(
        &self,
        turn_context: &TurnContext,
        call_id: String,
        args: RequestPermissionsArgs,
    ) -> Option<RequestPermissionsResponse> {
        match turn_context.approval_policy.value() {
            AskForApproval::Never => {
                return Some(RequestPermissionsResponse {
                    permissions: RequestPermissionProfile::default(),
                    scope: PermissionGrantScope::Turn,
                });
            }
            AskForApproval::Granular(granular_config)
                if !granular_config.allows_request_permissions() =>
            {
                return Some(RequestPermissionsResponse {
                    permissions: RequestPermissionProfile::default(),
                    scope: PermissionGrantScope::Turn,
                });
            }
            AskForApproval::OnFailure
            | AskForApproval::OnRequest
            | AskForApproval::UnlessTrusted
            | AskForApproval::Granular(_) => {}
        }

        let (tx_response, rx_response) = oneshot::channel();
        let prev_entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    ts.insert_pending_request_permissions(call_id.clone(), tx_response)
                }
                None => None,
            }
        };
        if prev_entry.is_some() {
            warn!("Overwriting existing pending request_permissions for call_id: {call_id}");
        }

        // TODO(ccunningham): Support auto-review for request_permissions /
        // with_additional_permissions. V0 still routes this surface through
        // the existing manual RequestPermissions event flow.
        let event = EventMsg::RequestPermissions(RequestPermissionsEvent {
            call_id,
            turn_id: turn_context.sub_id.clone(),
            reason: args.reason,
            permissions: args.permissions,
        });
        self.send_event(turn_context, event).await;
        rx_response.await.ok()
    }

    pub async fn request_user_input(
        &self,
        turn_context: &TurnContext,
        call_id: String,
        args: RequestUserInputArgs,
    ) -> Option<RequestUserInputResponse> {
        let sub_id = turn_context.sub_id.clone();
        let (tx_response, rx_response) = oneshot::channel();
        let event_id = sub_id.clone();
        let prev_entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    ts.insert_pending_user_input(sub_id, tx_response)
                }
                None => None,
            }
        };
        if prev_entry.is_some() {
            warn!("Overwriting existing pending user input for sub_id: {event_id}");
        }

        let event = EventMsg::RequestUserInput(RequestUserInputEvent {
            call_id,
            turn_id: turn_context.sub_id.clone(),
            questions: args.questions,
        });
        self.send_event(turn_context, event).await;
        rx_response.await.ok()
    }

    pub async fn request_mcp_server_elicitation(
        &self,
        turn_context: &TurnContext,
        request_id: RequestId,
        params: McpServerElicitationRequestParams,
    ) -> Option<ElicitationResponse> {
        let server_name = params.server_name.clone();
        let request = match params.request {
            McpServerElicitationRequest::Form {
                meta,
                message,
                requested_schema,
            } => {
                let requested_schema = match serde_json::to_value(requested_schema) {
                    Ok(requested_schema) => requested_schema,
                    Err(err) => {
                        warn!(
                            "failed to serialize MCP elicitation schema for server_name: {server_name}, request_id: {request_id}: {err:#}"
                        );
                        return None;
                    }
                };
                codex_protocol::approvals::ElicitationRequest::Form {
                    meta,
                    message,
                    requested_schema,
                }
            }
            McpServerElicitationRequest::Url {
                meta,
                message,
                url,
                elicitation_id,
            } => codex_protocol::approvals::ElicitationRequest::Url {
                meta,
                message,
                url,
                elicitation_id,
            },
        };

        let (tx_response, rx_response) = oneshot::channel();
        let prev_entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    ts.insert_pending_elicitation(
                        server_name.clone(),
                        request_id.clone(),
                        tx_response,
                    )
                }
                None => None,
            }
        };
        if prev_entry.is_some() {
            warn!(
                "Overwriting existing pending elicitation for server_name: {server_name}, request_id: {request_id}"
            );
        }
        let id = match request_id {
            rmcp::model::NumberOrString::String(value) => {
                codex_protocol::mcp::RequestId::String(value.to_string())
            }
            rmcp::model::NumberOrString::Number(value) => {
                codex_protocol::mcp::RequestId::Integer(value)
            }
        };
        let event = EventMsg::ElicitationRequest(ElicitationRequestEvent {
            turn_id: params.turn_id,
            server_name,
            id,
            request,
        });
        self.send_event(turn_context, event).await;
        rx_response.await.ok()
    }

    pub async fn notify_user_input_response(
        &self,
        sub_id: &str,
        response: RequestUserInputResponse,
    ) {
        let entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    ts.remove_pending_user_input(sub_id)
                }
                None => None,
            }
        };
        match entry {
            Some(tx_response) => {
                tx_response.send(response).ok();
            }
            None => {
                warn!("No pending user input found for sub_id: {sub_id}");
            }
        }
    }

    pub async fn notify_request_permissions_response(
        &self,
        call_id: &str,
        response: RequestPermissionsResponse,
    ) {
        let mut granted_for_session = None;
        let entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    let entry = ts.remove_pending_request_permissions(call_id);
                    if entry.is_some() && !response.permissions.is_empty() {
                        match response.scope {
                            PermissionGrantScope::Turn => {
                                ts.record_granted_permissions(response.permissions.clone().into());
                            }
                            PermissionGrantScope::Session => {
                                granted_for_session = Some(response.permissions.clone());
                            }
                        }
                    }
                    entry
                }
                None => None,
            }
        };
        if let Some(permissions) = granted_for_session {
            let mut state = self.state.lock().await;
            state.record_granted_permissions(permissions.into());
        }
        match entry {
            Some(tx_response) => {
                tx_response.send(response).ok();
            }
            None => {
                warn!("No pending request_permissions found for call_id: {call_id}");
            }
        }
    }

    pub(crate) async fn granted_turn_permissions(&self) -> Option<PermissionProfile> {
        let active = self.active_turn.lock().await;
        let active = active.as_ref()?;
        let ts = active.turn_state.lock().await;
        ts.granted_permissions()
    }

    pub(crate) async fn granted_session_permissions(&self) -> Option<PermissionProfile> {
        let state = self.state.lock().await;
        state.granted_permissions()
    }

    pub async fn notify_dynamic_tool_response(&self, call_id: &str, response: DynamicToolResponse) {
        let entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    ts.remove_pending_dynamic_tool(call_id)
                }
                None => None,
            }
        };
        match entry {
            Some(tx_response) => {
                tx_response.send(response).ok();
            }
            None => {
                warn!("No pending dynamic tool call found for call_id: {call_id}");
            }
        }
    }

    pub async fn notify_approval(&self, approval_id: &str, decision: ReviewDecision) {
        let entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    ts.remove_pending_approval(approval_id)
                }
                None => None,
            }
        };
        match entry {
            Some(tx_approve) => {
                tx_approve.send(decision).ok();
            }
            None => {
                warn!("No pending approval found for call_id: {approval_id}");
            }
        }
    }

    pub async fn resolve_elicitation(
        &self,
        server_name: String,
        id: RequestId,
        response: ElicitationResponse,
    ) -> anyhow::Result<()> {
        let entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    ts.remove_pending_elicitation(&server_name, &id)
                }
                None => None,
            }
        };
        if let Some(tx_response) = entry {
            tx_response
                .send(response)
                .map_err(|e| anyhow::anyhow!("failed to send elicitation response: {e:?}"))?;
            return Ok(());
        }

        self.services
            .mcp_connection_manager
            .read()
            .await
            .resolve_elicitation(server_name, id, response)
            .await
    }

    /// Records input items: always append to conversation history and
    /// persist these response items to rollout.
    pub(crate) async fn record_conversation_items(
        &self,
        turn_context: &TurnContext,
        items: &[ResponseItem],
    ) {
        self.record_into_history(items, turn_context).await;
        self.persist_rollout_response_items(items).await;
        self.send_raw_response_items(turn_context, items).await;
    }

    /// Append ResponseItems to the in-memory conversation history only.
    pub(crate) async fn record_into_history(
        &self,
        items: &[ResponseItem],
        turn_context: &TurnContext,
    ) {
        let mut state = self.state.lock().await;
        state.record_items(items.iter(), turn_context.truncation_policy);
    }

    pub(crate) async fn record_model_warning(&self, message: impl Into<String>, ctx: &TurnContext) {
        self.services
            .session_telemetry
            .counter("codex.model_warning", /*inc*/ 1, &[]);
        let item = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: format!("Warning: {}", message.into()),
            }],
            end_turn: None,
            phase: None,
        };

        self.record_conversation_items(ctx, &[item]).await;
    }

    async fn maybe_warn_on_server_model_mismatch(
        self: &Arc<Self>,
        turn_context: &Arc<TurnContext>,
        server_model: String,
    ) -> bool {
        let requested_model = turn_context.model_info.slug.clone();
        let server_model_normalized = server_model.to_ascii_lowercase();
        let requested_model_normalized = requested_model.to_ascii_lowercase();
        if server_model_normalized == requested_model_normalized {
            info!("server reported model {server_model} (matches requested model)");
            return false;
        }

        warn!("server reported model {server_model} while requested model was {requested_model}");

        let warning_message = format!(
            "Your account was flagged for potentially high-risk cyber activity and this request was routed to gpt-5.2 as a fallback. To regain access to gpt-5.3-codex, apply for trusted access: {CYBER_VERIFY_URL} or learn more: {CYBER_SAFETY_URL}"
        );

        self.send_event(
            turn_context,
            EventMsg::ModelReroute(ModelRerouteEvent {
                from_model: requested_model.clone(),
                to_model: server_model.clone(),
                reason: ModelRerouteReason::HighRiskCyberActivity,
            }),
        )
        .await;

        self.send_event(
            turn_context,
            EventMsg::Warning(WarningEvent {
                message: warning_message.clone(),
            }),
        )
        .await;
        self.record_model_warning(warning_message, turn_context)
            .await;
        true
    }

    pub(crate) async fn replace_history(
        &self,
        items: Vec<ResponseItem>,
        reference_context_item: Option<TurnContextItem>,
    ) {
        let mut state = self.state.lock().await;
        state.replace_history(items, reference_context_item);
    }

    pub(crate) async fn replace_compacted_history(
        &self,
        items: Vec<ResponseItem>,
        reference_context_item: Option<TurnContextItem>,
        compacted_item: CompactedItem,
    ) {
        self.replace_history(items, reference_context_item.clone())
            .await;

        self.persist_rollout_items(&[RolloutItem::Compacted(compacted_item)])
            .await;
        if let Some(turn_context_item) = reference_context_item {
            self.persist_rollout_items(&[RolloutItem::TurnContext(turn_context_item)])
                .await;
        }
        self.services.model_client.advance_window_generation();
    }

    async fn persist_rollout_response_items(&self, items: &[ResponseItem]) {
        let rollout_items: Vec<RolloutItem> = items
            .iter()
            .cloned()
            .map(RolloutItem::ResponseItem)
            .collect();
        self.persist_rollout_items(&rollout_items).await;
    }

    pub fn enabled(&self, feature: Feature) -> bool {
        self.features.enabled(feature)
    }

    pub(crate) fn features(&self) -> ManagedFeatures {
        self.features.clone()
    }

    pub(crate) async fn collaboration_mode(&self) -> CollaborationMode {
        let state = self.state.lock().await;
        state.session_configuration.collaboration_mode.clone()
    }

    async fn send_raw_response_items(&self, turn_context: &TurnContext, items: &[ResponseItem]) {
        for item in items {
            self.send_event(
                turn_context,
                EventMsg::RawResponseItem(RawResponseItemEvent { item: item.clone() }),
            )
            .await;
        }
    }

    pub(crate) async fn build_initial_context(
        &self,
        turn_context: &TurnContext,
    ) -> Vec<ResponseItem> {
        let mut developer_sections = Vec::<String>::with_capacity(8);
        let mut contextual_user_sections = Vec::<String>::with_capacity(2);
        let shell = self.user_shell();
        let (
            reference_context_item,
            previous_turn_settings,
            collaboration_mode,
            base_instructions,
            session_source,
        ) = {
            let state = self.state.lock().await;
            (
                state.reference_context_item(),
                state.previous_turn_settings(),
                state.session_configuration.collaboration_mode.clone(),
                state.session_configuration.base_instructions.clone(),
                state.session_configuration.session_source.clone(),
            )
        };
        if let Some(model_switch_message) =
            crate::context_manager::updates::build_model_instructions_update_item(
                previous_turn_settings.as_ref(),
                turn_context,
            )
        {
            developer_sections.push(model_switch_message.into_text());
        }
        if turn_context.config.include_permissions_instructions {
            developer_sections.push(
                DeveloperInstructions::from_policy(
                    turn_context.sandbox_policy.get(),
                    turn_context.approval_policy.value(),
                    turn_context.config.approvals_reviewer,
                    self.services.exec_policy.current().as_ref(),
                    &turn_context.cwd,
                    turn_context
                        .features
                        .enabled(Feature::ExecPermissionApprovals),
                    turn_context
                        .features
                        .enabled(Feature::RequestPermissionsTool),
                )
                .into_text(),
            );
        }
        let separate_guardian_developer_message =
            crate::guardian::is_guardian_reviewer_source(&session_source);
        // Keep the guardian policy prompt out of the aggregated developer bundle so it
        // stays isolated as its own top-level developer message for guardian subagents.
        if !separate_guardian_developer_message
            && let Some(developer_instructions) = turn_context.developer_instructions.as_deref()
            && !developer_instructions.is_empty()
        {
            developer_sections.push(developer_instructions.to_string());
        }
        // Add developer instructions for memories.
        if turn_context.features.enabled(Feature::MemoryTool)
            && turn_context.config.memories.use_memories
            && let Some(memory_prompt) =
                build_memory_tool_developer_instructions(&turn_context.config.codex_home).await
        {
            developer_sections.push(memory_prompt);
        }
        // Add developer instructions from collaboration_mode if they exist and are non-empty
        if let Some(collab_instructions) =
            DeveloperInstructions::from_collaboration_mode(&collaboration_mode)
        {
            developer_sections.push(collab_instructions.into_text());
        }
        if let Some(realtime_update) = crate::context_manager::updates::build_initial_realtime_item(
            reference_context_item.as_ref(),
            previous_turn_settings.as_ref(),
            turn_context,
        ) {
            developer_sections.push(realtime_update.into_text());
        }
        if self.features.enabled(Feature::Personality)
            && let Some(personality) = turn_context.personality
        {
            let model_info = turn_context.model_info.clone();
            let has_baked_personality = model_info.supports_personality()
                && base_instructions == model_info.get_model_instructions(Some(personality));
            if !has_baked_personality
                && let Some(personality_message) =
                    crate::context_manager::updates::personality_message_for(
                        &model_info,
                        personality,
                    )
            {
                developer_sections.push(
                    DeveloperInstructions::personality_spec_message(personality_message)
                        .into_text(),
                );
            }
        }
        if turn_context.config.include_apps_instructions && turn_context.apps_enabled() {
            let mcp_connection_manager = self.services.mcp_connection_manager.read().await;
            let accessible_and_enabled_connectors =
                connectors::list_accessible_and_enabled_connectors_from_manager(
                    &mcp_connection_manager,
                    &turn_context.config,
                )
                .await;
            if let Some(apps_section) = render_apps_section(&accessible_and_enabled_connectors) {
                developer_sections.push(apps_section);
            }
        }
        let implicit_skills = turn_context
            .turn_skills
            .outcome
            .allowed_skills_for_implicit_invocation();
        if let Some(skills_section) = render_skills_section(&implicit_skills) {
            developer_sections.push(skills_section);
        }
        let loaded_plugins = self
            .services
            .plugins_manager
            .plugins_for_config(&turn_context.config)
            .await;
        if let Some(plugin_section) = render_plugins_section(loaded_plugins.capability_summaries())
        {
            developer_sections.push(plugin_section);
        }
        if turn_context.features.enabled(Feature::CodexGitCommit)
            && let Some(commit_message_instruction) = commit_message_trailer_instruction(
                turn_context.config.commit_attribution.as_deref(),
            )
        {
            developer_sections.push(commit_message_instruction);
        }
        if let Some(user_instructions) = turn_context.user_instructions.as_deref() {
            contextual_user_sections.push(
                UserInstructions {
                    text: user_instructions.to_string(),
                    directory: turn_context.cwd.to_string_lossy().into_owned(),
                }
                .serialize_to_text(),
            );
        }
        if turn_context.config.include_environment_context {
            let subagents = self
                .services
                .agent_control
                .format_environment_context_subagents(self.conversation_id)
                .await;
            contextual_user_sections.push(
                EnvironmentContext::from_turn_context(turn_context, shell.as_ref())
                    .with_subagents(subagents)
                    .serialize_to_xml(),
            );
        }

        let mut items = Vec::with_capacity(3);
        if let Some(developer_message) =
            crate::context_manager::updates::build_developer_update_item(developer_sections)
        {
            items.push(developer_message);
        }
        if let Some(contextual_user_message) =
            crate::context_manager::updates::build_contextual_user_message(contextual_user_sections)
        {
            items.push(contextual_user_message);
        }
        // Emit the guardian policy prompt as a separate developer item so the guardian
        // subagent sees a distinct, easy-to-audit instruction block.
        if separate_guardian_developer_message
            && let Some(developer_instructions) = turn_context.developer_instructions.as_deref()
            && !developer_instructions.is_empty()
            && let Some(guardian_developer_message) =
                crate::context_manager::updates::build_developer_update_item(vec![
                    developer_instructions.to_string(),
                ])
        {
            items.push(guardian_developer_message);
        }
        items
    }

    pub(crate) async fn persist_rollout_items(&self, items: &[RolloutItem]) {
        let recorder = {
            let guard = self.services.rollout.lock().await;
            guard.clone()
        };
        if let Some(rec) = recorder
            && let Err(e) = rec.record_items(items).await
        {
            error!("failed to record rollout items: {e:#}");
        }
    }

    pub(crate) async fn clone_history(&self) -> ContextManager {
        let state = self.state.lock().await;
        state.clone_history()
    }

    pub(crate) async fn reference_context_item(&self) -> Option<TurnContextItem> {
        let state = self.state.lock().await;
        state.reference_context_item()
    }

    /// Persist the latest turn context snapshot for the first real user turn and for
    /// steady-state turns that emit model-visible context updates.
    ///
    /// When the reference snapshot is missing, this injects full initial context. Otherwise, it
    /// emits only settings diff items.
    ///
    /// If full context is injected and a model switch occurred, this prepends the
    /// `<model_switch>` developer message so model-specific instructions are not lost.
    ///
    /// This is the normal runtime path that establishes a new `reference_context_item`.
    /// Mid-turn compaction is the other path that can re-establish that baseline when it
    /// reinjects full initial context into replacement history. Other non-regular tasks
    /// intentionally do not update the baseline.
    pub(crate) async fn record_context_updates_and_set_reference_context_item(
        &self,
        turn_context: &TurnContext,
    ) {
        let reference_context_item = {
            let state = self.state.lock().await;
            state.reference_context_item()
        };
        let should_inject_full_context = reference_context_item.is_none();
        let context_items = if should_inject_full_context {
            self.build_initial_context(turn_context).await
        } else {
            // Steady-state path: append only context diffs to minimize token overhead.
            self.build_settings_update_items(reference_context_item.as_ref(), turn_context)
                .await
        };
        let turn_context_item = turn_context.to_turn_context_item();
        if !context_items.is_empty() {
            self.record_conversation_items(turn_context, &context_items)
                .await;
        }
        // Persist one `TurnContextItem` per real user turn so resume/lazy replay can recover the
        // latest durable baseline even when this turn emitted no model-visible context diffs.
        self.persist_rollout_items(&[RolloutItem::TurnContext(turn_context_item.clone())])
            .await;

        // Advance the in-memory diff baseline even when this turn emitted no model-visible
        // context items. This keeps later runtime diffing aligned with the current turn state.
        let mut state = self.state.lock().await;
        state.set_reference_context_item(Some(turn_context_item));
    }

    pub(crate) async fn update_token_usage_info(
        &self,
        turn_context: &TurnContext,
        token_usage: Option<&TokenUsage>,
    ) {
        if let Some(token_usage) = token_usage {
            let mut state = self.state.lock().await;
            state.update_token_info_from_usage(token_usage, turn_context.model_context_window());
        }
        self.send_token_count_event(turn_context).await;
    }

    pub(crate) async fn recompute_token_usage(&self, turn_context: &TurnContext) {
        let history = self.clone_history().await;
        let base_instructions = self.get_base_instructions().await;
        let Some(estimated_total_tokens) =
            history.estimate_token_count_with_base_instructions(&base_instructions)
        else {
            return;
        };
        {
            let mut state = self.state.lock().await;
            let mut info = state.token_info().unwrap_or(TokenUsageInfo {
                total_token_usage: TokenUsage::default(),
                last_token_usage: TokenUsage::default(),
                model_context_window: None,
            });

            info.last_token_usage = TokenUsage {
                input_tokens: 0,
                cached_input_tokens: 0,
                output_tokens: 0,
                reasoning_output_tokens: 0,
                total_tokens: estimated_total_tokens.max(0),
            };

            if let Some(model_context_window) = turn_context.model_context_window() {
                info.model_context_window = Some(model_context_window);
            }

            state.set_token_info(Some(info));
        }
        self.send_token_count_event(turn_context).await;
    }

    pub(crate) async fn update_rate_limits(
        &self,
        turn_context: &TurnContext,
        new_rate_limits: RateLimitSnapshot,
    ) {
        {
            let mut state = self.state.lock().await;
            state.set_rate_limits(new_rate_limits);
        }
        self.send_token_count_event(turn_context).await;
    }

    pub(crate) async fn mcp_dependency_prompted(&self) -> HashSet<String> {
        let state = self.state.lock().await;
        state.mcp_dependency_prompted()
    }

    pub(crate) async fn record_mcp_dependency_prompted<I>(&self, names: I)
    where
        I: IntoIterator<Item = String>,
    {
        let mut state = self.state.lock().await;
        state.record_mcp_dependency_prompted(names);
    }

    pub async fn dependency_env(&self) -> HashMap<String, String> {
        let state = self.state.lock().await;
        state.dependency_env()
    }

    pub async fn set_dependency_env(&self, values: HashMap<String, String>) {
        let mut state = self.state.lock().await;
        state.set_dependency_env(values);
    }

    pub(crate) async fn set_server_reasoning_included(&self, included: bool) {
        let mut state = self.state.lock().await;
        state.set_server_reasoning_included(included);
    }

    async fn send_token_count_event(&self, turn_context: &TurnContext) {
        let (info, rate_limits) = {
            let state = self.state.lock().await;
            state.token_info_and_rate_limits()
        };
        let event = EventMsg::TokenCount(TokenCountEvent { info, rate_limits });
        self.send_event(turn_context, event).await;
    }

    pub(crate) async fn set_total_tokens_full(&self, turn_context: &TurnContext) {
        if let Some(context_window) = turn_context.model_context_window() {
            let mut state = self.state.lock().await;
            state.set_token_usage_full(context_window);
        }
        self.send_token_count_event(turn_context).await;
    }

    pub(crate) async fn record_response_item_and_emit_turn_item(
        &self,
        turn_context: &TurnContext,
        response_item: ResponseItem,
    ) {
        // Add to conversation history and persist response item to rollout.
        self.record_conversation_items(turn_context, std::slice::from_ref(&response_item))
            .await;

        // Derive a turn item and emit lifecycle events if applicable.
        if let Some(item) = parse_turn_item(&response_item) {
            self.emit_turn_item_started(turn_context, &item).await;
            self.emit_turn_item_completed(turn_context, item).await;
        }
    }

    pub(crate) async fn record_user_prompt_and_emit_turn_item(
        &self,
        turn_context: &TurnContext,
        input: &[UserInput],
        response_item: ResponseItem,
    ) {
        // Persist the user message to history, but emit the turn item from `UserInput` so
        // UI-only `text_elements` are preserved. `ResponseItem::Message` does not carry
        // those spans, and `record_response_item_and_emit_turn_item` would drop them.
        self.record_conversation_items(turn_context, std::slice::from_ref(&response_item))
            .await;
        let turn_item = TurnItem::UserMessage(UserMessageItem::new(input));
        self.emit_turn_item_started(turn_context, &turn_item).await;
        self.emit_turn_item_completed(turn_context, turn_item).await;
        self.ensure_rollout_materialized().await;
    }

    pub(crate) async fn notify_background_event(
        &self,
        turn_context: &TurnContext,
        message: impl Into<String>,
    ) {
        let event = EventMsg::BackgroundEvent(BackgroundEventEvent {
            message: message.into(),
        });
        self.send_event(turn_context, event).await;
    }

    pub(crate) async fn notify_stream_error(
        &self,
        turn_context: &TurnContext,
        message: impl Into<String>,
        codex_error: CodexErr,
    ) {
        let additional_details = codex_error.to_string();
        let codex_error_info = CodexErrorInfo::ResponseStreamDisconnected {
            http_status_code: codex_error.http_status_code_value(),
        };
        let event = EventMsg::StreamError(StreamErrorEvent {
            message: message.into(),
            codex_error_info: Some(codex_error_info),
            additional_details: Some(additional_details),
        });
        self.send_event(turn_context, event).await;
    }

    async fn maybe_start_ghost_snapshot(
        self: &Arc<Self>,
        turn_context: Arc<TurnContext>,
        cancellation_token: CancellationToken,
    ) {
        if !self.enabled(Feature::GhostCommit) {
            return;
        }
        let token = match turn_context.tool_call_gate.subscribe().await {
            Ok(token) => token,
            Err(err) => {
                warn!("failed to subscribe to ghost snapshot readiness: {err}");
                return;
            }
        };

        info!("spawning ghost snapshot task");
        let task = GhostSnapshotTask::new(token);
        Arc::new(task)
            .run(
                Arc::new(SessionTaskContext::new(self.clone())),
                turn_context.clone(),
                Vec::new(),
                cancellation_token,
            )
            .await;
    }

    /// Inject additional user input into the currently active turn.
    ///
    /// Returns the active turn id when accepted.
    pub async fn steer_input(
        &self,
        input: Vec<UserInput>,
        expected_turn_id: Option<&str>,
        responsesapi_client_metadata: Option<HashMap<String, String>>,
    ) -> Result<String, SteerInputError> {
        if input.is_empty() {
            return Err(SteerInputError::EmptyInput);
        }

        let mut active = self.active_turn.lock().await;
        let Some(active_turn) = active.as_mut() else {
            return Err(SteerInputError::NoActiveTurn(input));
        };

        let Some((active_turn_id, _)) = active_turn.tasks.first() else {
            return Err(SteerInputError::NoActiveTurn(input));
        };

        if let Some(expected_turn_id) = expected_turn_id
            && expected_turn_id != active_turn_id
        {
            return Err(SteerInputError::ExpectedTurnMismatch {
                expected: expected_turn_id.to_string(),
                actual: active_turn_id.clone(),
            });
        }

        match active_turn.tasks.first().map(|(_, task)| task.kind) {
            Some(crate::state::TaskKind::Regular) => {}
            Some(crate::state::TaskKind::Review) => {
                return Err(SteerInputError::ActiveTurnNotSteerable {
                    turn_kind: NonSteerableTurnKind::Review,
                });
            }
            Some(crate::state::TaskKind::Compact) => {
                return Err(SteerInputError::ActiveTurnNotSteerable {
                    turn_kind: NonSteerableTurnKind::Compact,
                });
            }
            None => return Err(SteerInputError::NoActiveTurn(input)),
        }

        if let Some(responsesapi_client_metadata) = responsesapi_client_metadata
            && let Some((_, active_task)) = active_turn.tasks.first()
        {
            active_task
                .turn_context
                .turn_metadata_state
                .set_responsesapi_client_metadata(responsesapi_client_metadata);
        }

        let mut turn_state = active_turn.turn_state.lock().await;
        turn_state.push_pending_input(input.into());
        turn_state.accept_mailbox_delivery_for_current_turn();
        Ok(active_turn_id.clone())
    }

    /// Returns the input if there was no task running to inject into.
    pub async fn inject_response_items(
        &self,
        input: Vec<ResponseInputItem>,
    ) -> Result<(), Vec<ResponseInputItem>> {
        let mut active = self.active_turn.lock().await;
        match active.as_mut() {
            Some(at) => {
                let mut ts = at.turn_state.lock().await;
                for item in input {
                    ts.push_pending_input(item);
                }
                Ok(())
            }
            None => Err(input),
        }
    }

    pub(crate) async fn defer_mailbox_delivery_to_next_turn(&self, sub_id: &str) {
        let turn_state = self.turn_state_for_sub_id(sub_id).await;
        let Some(turn_state) = turn_state else {
            return;
        };
        let mut turn_state = turn_state.lock().await;
        if turn_state.has_pending_input() {
            return;
        }
        turn_state.set_mailbox_delivery_phase(MailboxDeliveryPhase::NextTurn);
    }

    pub(crate) async fn accept_mailbox_delivery_for_current_turn(&self, sub_id: &str) {
        let turn_state = self.turn_state_for_sub_id(sub_id).await;
        let Some(turn_state) = turn_state else {
            return;
        };
        turn_state
            .lock()
            .await
            .set_mailbox_delivery_phase(MailboxDeliveryPhase::CurrentTurn);
    }

    async fn turn_state_for_sub_id(
        &self,
        sub_id: &str,
    ) -> Option<Arc<tokio::sync::Mutex<crate::state::TurnState>>> {
        let active = self.active_turn.lock().await;
        active.as_ref().and_then(|active_turn| {
            active_turn
                .tasks
                .contains_key(sub_id)
                .then(|| Arc::clone(&active_turn.turn_state))
        })
    }

    pub(crate) fn subscribe_mailbox_seq(&self) -> watch::Receiver<u64> {
        self.mailbox.subscribe()
    }

    pub(crate) fn enqueue_mailbox_communication(&self, communication: InterAgentCommunication) {
        self.mailbox.send(communication);
    }

    pub(crate) async fn has_trigger_turn_mailbox_items(&self) -> bool {
        self.mailbox_rx.lock().await.has_pending_trigger_turn()
    }

    pub async fn prepend_pending_input(&self, input: Vec<ResponseInputItem>) -> Result<(), ()> {
        let mut active = self.active_turn.lock().await;
        match active.as_mut() {
            Some(at) => {
                let mut ts = at.turn_state.lock().await;
                ts.prepend_pending_input(input);
                Ok(())
            }
            None => Err(()),
        }
    }

    pub async fn get_pending_input(&self) -> Vec<ResponseInputItem> {
        let (pending_input, accepts_mailbox_delivery) = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    (
                        ts.take_pending_input(),
                        ts.accepts_mailbox_delivery_for_current_turn(),
                    )
                }
                None => (Vec::new(), true),
            }
        };
        if !accepts_mailbox_delivery {
            return pending_input;
        }
        let mailbox_items = {
            let mut mailbox_rx = self.mailbox_rx.lock().await;
            mailbox_rx
                .drain()
                .into_iter()
                .map(|mail| mail.to_response_input_item())
                .collect::<Vec<_>>()
        };
        if pending_input.is_empty() {
            mailbox_items
        } else if mailbox_items.is_empty() {
            pending_input
        } else {
            let mut pending_input = pending_input;
            pending_input.extend(mailbox_items);
            pending_input
        }
    }

    /// Queue response items to be injected into the next active turn created for this session.
    #[cfg(test)]
    pub(crate) async fn queue_response_items_for_next_turn(&self, items: Vec<ResponseInputItem>) {
        if items.is_empty() {
            return;
        }

        let mut idle_pending_input = self.idle_pending_input.lock().await;
        idle_pending_input.extend(items);
    }

    pub(crate) async fn take_queued_response_items_for_next_turn(&self) -> Vec<ResponseInputItem> {
        std::mem::take(&mut *self.idle_pending_input.lock().await)
    }

    pub(crate) async fn has_queued_response_items_for_next_turn(&self) -> bool {
        !self.idle_pending_input.lock().await.is_empty()
    }

    pub async fn has_pending_input(&self) -> bool {
        let (has_turn_pending_input, accepts_mailbox_delivery) = {
            let active = self.active_turn.lock().await;
            match active.as_ref() {
                Some(at) => {
                    let ts = at.turn_state.lock().await;
                    (
                        ts.has_pending_input(),
                        ts.accepts_mailbox_delivery_for_current_turn(),
                    )
                }
                None => (false, true),
            }
        };
        if has_turn_pending_input {
            return true;
        }
        if !accepts_mailbox_delivery {
            return false;
        }
        self.mailbox_rx.lock().await.has_pending()
    }

    pub async fn list_resources(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> anyhow::Result<ListResourcesResult> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .list_resources(server, params)
            .await
    }

    pub async fn list_resource_templates(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> anyhow::Result<ListResourceTemplatesResult> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .list_resource_templates(server, params)
            .await
    }

    pub async fn read_resource(
        &self,
        server: &str,
        params: ReadResourceRequestParams,
    ) -> anyhow::Result<ReadResourceResult> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .read_resource(server, params)
            .await
    }

    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) -> anyhow::Result<CallToolResult> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .call_tool(server, tool, arguments, meta)
            .await
    }

    pub(crate) async fn resolve_mcp_tool_info(&self, tool_name: &ToolName) -> Option<ToolInfo> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .resolve_tool_info(tool_name)
            .await
    }

    pub async fn interrupt_task(self: &Arc<Self>) {
        info!("interrupt received: abort current task, if any");
        let has_active_turn = { self.active_turn.lock().await.is_some() };
        if has_active_turn {
            self.abort_all_tasks(TurnAbortReason::Interrupted).await;
        } else {
            self.cancel_mcp_startup().await;
        }
    }

    pub(crate) fn hooks(&self) -> &Hooks {
        &self.services.hooks
    }

    pub(crate) fn user_shell(&self) -> Arc<shell::Shell> {
        Arc::clone(&self.services.user_shell)
    }

    pub(crate) async fn current_rollout_path(&self) -> Option<PathBuf> {
        let recorder = {
            let guard = self.services.rollout.lock().await;
            guard.clone()
        };
        recorder.map(|recorder| recorder.rollout_path().to_path_buf())
    }

    pub(crate) async fn hook_transcript_path(&self) -> Option<PathBuf> {
        self.ensure_rollout_materialized().await;
        self.current_rollout_path().await
    }

    pub(crate) async fn take_pending_session_start_source(
        &self,
    ) -> Option<codex_hooks::SessionStartSource> {
        let mut state = self.state.lock().await;
        state.take_pending_session_start_source()
    }

    async fn refresh_mcp_servers_inner(
        &self,
        turn_context: &TurnContext,
        mcp_servers: HashMap<String, McpServerConfig>,
        store_mode: OAuthCredentialsStoreMode,
    ) {
        let auth = self.services.auth_manager.auth().await;
        let config = self.get_config().await;
        let mcp_config = config
            .to_mcp_config(self.services.plugins_manager.as_ref())
            .await;
        let tool_plugin_provenance = self
            .services
            .mcp_manager
            .tool_plugin_provenance(config.as_ref())
            .await;
        let mcp_servers = with_codex_apps_mcp(mcp_servers, auth.as_ref(), &mcp_config);
        let auth_statuses = compute_auth_statuses(mcp_servers.iter(), store_mode).await;
        {
            let mut guard = self.services.mcp_startup_cancellation_token.lock().await;
            guard.cancel();
            *guard = CancellationToken::new();
        }
        let (refreshed_manager, cancel_token) = McpConnectionManager::new(
            &mcp_servers,
            store_mode,
            auth_statuses,
            &turn_context.config.permissions.approval_policy,
            turn_context.sub_id.clone(),
            self.get_tx_event(),
            turn_context.sandbox_policy.get().clone(),
            config.codex_home.to_path_buf(),
            codex_apps_tools_cache_key(auth.as_ref()),
            tool_plugin_provenance,
        )
        .await;
        {
            let mut guard = self.services.mcp_startup_cancellation_token.lock().await;
            if guard.is_cancelled() {
                cancel_token.cancel();
            }
            *guard = cancel_token;
        }

        let mut manager = self.services.mcp_connection_manager.write().await;
        *manager = refreshed_manager;
    }

    async fn refresh_mcp_servers_if_requested(&self, turn_context: &TurnContext) {
        let refresh_config = { self.pending_mcp_server_refresh_config.lock().await.take() };
        let Some(refresh_config) = refresh_config else {
            return;
        };

        let McpServerRefreshConfig {
            mcp_servers,
            mcp_oauth_credentials_store_mode,
        } = refresh_config;

        let mcp_servers =
            match serde_json::from_value::<HashMap<String, McpServerConfig>>(mcp_servers) {
                Ok(servers) => servers,
                Err(err) => {
                    warn!("failed to parse MCP server refresh config: {err}");
                    return;
                }
            };
        let store_mode = match serde_json::from_value::<OAuthCredentialsStoreMode>(
            mcp_oauth_credentials_store_mode,
        ) {
            Ok(mode) => mode,
            Err(err) => {
                warn!("failed to parse MCP OAuth refresh config: {err}");
                return;
            }
        };

        self.refresh_mcp_servers_inner(turn_context, mcp_servers, store_mode)
            .await;
    }

    pub(crate) async fn refresh_mcp_servers_now(
        &self,
        turn_context: &TurnContext,
        mcp_servers: HashMap<String, McpServerConfig>,
        store_mode: OAuthCredentialsStoreMode,
    ) {
        self.refresh_mcp_servers_inner(turn_context, mcp_servers, store_mode)
            .await;
    }

    #[cfg(test)]
    async fn mcp_startup_cancellation_token(&self) -> CancellationToken {
        self.services
            .mcp_startup_cancellation_token
            .lock()
            .await
            .clone()
    }

    fn show_raw_agent_reasoning(&self) -> bool {
        self.services.show_raw_agent_reasoning
    }

    async fn cancel_mcp_startup(&self) {
        self.services
            .mcp_startup_cancellation_token
            .lock()
            .await
            .cancel();
    }
}

pub(crate) fn emit_subagent_session_started(
    analytics_events_client: &AnalyticsEventsClient,
    client_metadata: AppServerClientMetadata,
    thread_id: ThreadId,
    parent_thread_id: Option<ThreadId>,
    thread_config: ThreadConfigSnapshot,
    subagent_source: SubAgentSource,
) {
    let AppServerClientMetadata {
        client_name,
        client_version,
    } = client_metadata;
    let (Some(client_name), Some(client_version)) = (client_name, client_version) else {
        tracing::warn!("skipping subagent thread analytics: missing inherited client metadata");
        return;
    };
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    analytics_events_client.track_subagent_thread_started(SubAgentThreadStartedInput {
        thread_id: thread_id.to_string(),
        parent_thread_id: parent_thread_id.map(|thread_id| thread_id.to_string()),
        product_client_id: client_name.clone(),
        client_name,
        client_version,
        model: thread_config.model,
        ephemeral: thread_config.ephemeral,
        subagent_source,
        created_at,
    });
}

async fn submission_loop(sess: Arc<Session>, config: Arc<Config>, rx_sub: Receiver<Submission>) {
    // To break out of this loop, send Op::Shutdown.
    while let Ok(sub) = rx_sub.recv().await {
        debug!(?sub, "Submission");
        let dispatch_span = submission_dispatch_span(&sub);
        let should_exit = async {
            match sub.op.clone() {
                Op::Interrupt => {
                    handlers::interrupt(&sess).await;
                    false
                }
                Op::CleanBackgroundTerminals => {
                    handlers::clean_background_terminals(&sess).await;
                    false
                }
                Op::RealtimeConversationStart(params) => {
                    if let Err(err) =
                        handle_realtime_conversation_start(&sess, sub.id.clone(), params).await
                    {
                        sess.send_event_raw(Event {
                            id: sub.id.clone(),
                            msg: EventMsg::Error(ErrorEvent {
                                message: err.to_string(),
                                codex_error_info: Some(CodexErrorInfo::Other),
                            }),
                        })
                        .await;
                    }
                    false
                }
                Op::RealtimeConversationAudio(params) => {
                    handle_realtime_conversation_audio(&sess, sub.id.clone(), params).await;
                    false
                }
                Op::RealtimeConversationText(params) => {
                    handle_realtime_conversation_text(&sess, sub.id.clone(), params).await;
                    false
                }
                Op::RealtimeConversationClose => {
                    handle_realtime_conversation_close(&sess, sub.id.clone()).await;
                    false
                }
                Op::RealtimeConversationListVoices => {
                    handlers::realtime_conversation_list_voices(&sess, sub.id.clone()).await;
                    false
                }
                Op::OverrideTurnContext {
                    cwd,
                    approval_policy,
                    approvals_reviewer,
                    sandbox_policy,
                    windows_sandbox_level,
                    model,
                    effort,
                    summary,
                    service_tier,
                    collaboration_mode,
                    personality,
                } => {
                    let collaboration_mode = if let Some(collab_mode) = collaboration_mode {
                        collab_mode
                    } else {
                        let state = sess.state.lock().await;
                        state.session_configuration.collaboration_mode.with_updates(
                            model.clone(),
                            effort,
                            /*developer_instructions*/ None,
                        )
                    };
                    handlers::override_turn_context(
                        &sess,
                        sub.id.clone(),
                        SessionSettingsUpdate {
                            cwd,
                            approval_policy,
                            approvals_reviewer,
                            sandbox_policy,
                            windows_sandbox_level,
                            collaboration_mode: Some(collaboration_mode),
                            reasoning_summary: summary,
                            service_tier,
                            personality,
                            ..Default::default()
                        },
                    )
                    .await;
                    false
                }
                Op::UserInput { .. } | Op::UserTurn { .. } => {
                    handlers::user_input_or_turn(&sess, sub.id.clone(), sub.op).await;
                    false
                }
                Op::InterAgentCommunication { communication } => {
                    handlers::inter_agent_communication(&sess, sub.id.clone(), communication).await;
                    false
                }
                Op::ExecApproval {
                    id: approval_id,
                    turn_id,
                    decision,
                } => {
                    handlers::exec_approval(&sess, approval_id, turn_id, decision).await;
                    false
                }
                Op::PatchApproval { id, decision } => {
                    handlers::patch_approval(&sess, id, decision).await;
                    false
                }
                Op::UserInputAnswer { id, response } => {
                    handlers::request_user_input_response(&sess, id, response).await;
                    false
                }
                Op::RequestPermissionsResponse { id, response } => {
                    handlers::request_permissions_response(&sess, id, response).await;
                    false
                }
                Op::DynamicToolResponse { id, response } => {
                    handlers::dynamic_tool_response(&sess, id, response).await;
                    false
                }
                Op::AddToHistory { text } => {
                    handlers::add_to_history(&sess, &config, text).await;
                    false
                }
                Op::GetHistoryEntryRequest { offset, log_id } => {
                    handlers::get_history_entry_request(
                        &sess,
                        &config,
                        sub.id.clone(),
                        offset,
                        log_id,
                    )
                    .await;
                    false
                }
                Op::ListMcpTools => {
                    handlers::list_mcp_tools(&sess, &config, sub.id.clone()).await;
                    false
                }
                Op::RefreshMcpServers { config } => {
                    handlers::refresh_mcp_servers(&sess, config).await;
                    false
                }
                Op::ReloadUserConfig => {
                    handlers::reload_user_config(&sess).await;
                    false
                }
                Op::ListSkills { cwds, force_reload } => {
                    handlers::list_skills(&sess, sub.id.clone(), cwds, force_reload).await;
                    false
                }
                Op::Undo => {
                    handlers::undo(&sess, sub.id.clone()).await;
                    false
                }
                Op::Compact => {
                    handlers::compact(&sess, sub.id.clone()).await;
                    false
                }
                Op::DropMemories => {
                    handlers::drop_memories(&sess, &config, sub.id.clone()).await;
                    false
                }
                Op::UpdateMemories => {
                    handlers::update_memories(&sess, &config, sub.id.clone()).await;
                    false
                }
                Op::ThreadRollback { num_turns } => {
                    handlers::thread_rollback(&sess, sub.id.clone(), num_turns).await;
                    false
                }
                Op::SetThreadName { name } => {
                    handlers::set_thread_name(&sess, sub.id.clone(), name).await;
                    false
                }
                Op::SetThreadMemoryMode { mode } => {
                    handlers::set_thread_memory_mode(&sess, sub.id.clone(), mode).await;
                    false
                }
                Op::RunUserShellCommand { command } => {
                    handlers::run_user_shell_command(&sess, sub.id.clone(), command).await;
                    false
                }
                Op::ResolveElicitation {
                    server_name,
                    request_id,
                    decision,
                    content,
                    meta,
                } => {
                    handlers::resolve_elicitation(
                        &sess,
                        server_name,
                        request_id,
                        decision,
                        content,
                        meta,
                    )
                    .await;
                    false
                }
                Op::Shutdown => handlers::shutdown(&sess, sub.id.clone()).await,
                Op::Review { review_request } => {
                    handlers::review(&sess, &config, sub.id.clone(), review_request).await;
                    false
                }
                _ => false, // Ignore unknown ops; enum is non_exhaustive to allow extensions.
            }
        }
        .instrument(dispatch_span)
        .await;
        if should_exit {
            break;
        }
    }
    // Also drain cached guardian state if the submission loop exits because
    // the channel closed without receiving an explicit shutdown op.
    sess.guardian_review_session.shutdown().await;
    debug!("Agent loop exited");
}

fn submission_dispatch_span(sub: &Submission) -> tracing::Span {
    let op_name = sub.op.kind();
    let span_name = format!("op.dispatch.{op_name}");
    let dispatch_span = match &sub.op {
        Op::RealtimeConversationAudio(_) => {
            debug_span!(
                "submission_dispatch",
                otel.name = span_name.as_str(),
                submission.id = sub.id.as_str(),
                codex.op = op_name
            )
        }
        _ => info_span!(
            "submission_dispatch",
            otel.name = span_name.as_str(),
            submission.id = sub.id.as_str(),
            codex.op = op_name
        ),
    };
    if let Some(trace) = sub.trace.as_ref()
        && !set_parent_from_w3c_trace_context(&dispatch_span, trace)
    {
        warn!(
            submission.id = sub.id.as_str(),
            "ignoring invalid submission trace carrier"
        );
    }
    dispatch_span
}

/// Operation handlers
mod handlers {
    use crate::codex::Session;
    use crate::codex::SessionSettingsUpdate;
    use crate::codex::SteerInputError;

    use crate::codex::spawn_review_thread;
    use crate::config::Config;
    use crate::config_loader::CloudRequirementsLoader;
    use crate::config_loader::LoaderOverrides;
    use crate::config_loader::load_config_layers_state;
    use crate::realtime_context::REALTIME_TURN_TOKEN_BUDGET;
    use crate::realtime_context::truncate_realtime_text_to_token_budget;
    use codex_features::Feature;
    use codex_utils_absolute_path::AbsolutePathBuf;

    use crate::review_prompts::resolve_review_request;
    use crate::rollout::RolloutRecorder;
    use crate::rollout::read_session_meta_line;
    use crate::tasks::CompactTask;
    use crate::tasks::UndoTask;
    use crate::tasks::UserShellCommandMode;
    use crate::tasks::UserShellCommandTask;
    use crate::tasks::execute_user_shell_command;
    use codex_mcp::collect_mcp_snapshot_from_manager;
    use codex_mcp::compute_auth_statuses;
    use codex_protocol::protocol::CodexErrorInfo;
    use codex_protocol::protocol::ErrorEvent;
    use codex_protocol::protocol::Event;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::InterAgentCommunication;
    use codex_protocol::protocol::ListSkillsResponseEvent;
    use codex_protocol::protocol::McpServerRefreshConfig;
    use codex_protocol::protocol::Op;
    use codex_protocol::protocol::RealtimeConversationListVoicesResponseEvent;
    use codex_protocol::protocol::RealtimeVoicesList;
    use codex_protocol::protocol::ReviewDecision;
    use codex_protocol::protocol::ReviewRequest;
    use codex_protocol::protocol::RolloutItem;
    use codex_protocol::protocol::SkillErrorInfo;
    use codex_protocol::protocol::SkillsListEntry;
    use codex_protocol::protocol::ThreadMemoryMode;
    use codex_protocol::protocol::ThreadNameUpdatedEvent;
    use codex_protocol::protocol::ThreadRolledBackEvent;
    use codex_protocol::protocol::TurnAbortReason;
    use codex_protocol::protocol::WarningEvent;
    use codex_protocol::request_permissions::RequestPermissionsResponse;
    use codex_protocol::request_user_input::RequestUserInputResponse;

    use crate::context_manager::is_user_turn_boundary;
    use codex_protocol::config_types::CollaborationMode;
    use codex_protocol::config_types::ModeKind;
    use codex_protocol::config_types::Settings;
    use codex_protocol::dynamic_tools::DynamicToolResponse;
    use codex_protocol::items::UserMessageItem;
    use codex_protocol::mcp::RequestId as ProtocolRequestId;
    use codex_protocol::user_input::UserInput;
    use codex_rmcp_client::ElicitationAction;
    use codex_rmcp_client::ElicitationResponse;
    use serde_json::Value;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tracing::debug;
    use tracing::info;
    use tracing::warn;

    pub async fn interrupt(sess: &Arc<Session>) {
        sess.interrupt_task().await;
    }

    pub async fn clean_background_terminals(sess: &Arc<Session>) {
        sess.close_unified_exec_processes().await;
    }

    pub async fn realtime_conversation_list_voices(sess: &Session, sub_id: String) {
        sess.send_event_raw(Event {
            id: sub_id,
            msg: EventMsg::RealtimeConversationListVoicesResponse(
                RealtimeConversationListVoicesResponseEvent {
                    voices: RealtimeVoicesList::builtin(),
                },
            ),
        })
        .await;
    }

    pub async fn override_turn_context(
        sess: &Session,
        sub_id: String,
        updates: SessionSettingsUpdate,
    ) {
        if let Err(err) = sess.update_settings(updates).await {
            sess.send_event_raw(Event {
                id: sub_id,
                msg: EventMsg::Error(ErrorEvent {
                    message: err.to_string(),
                    codex_error_info: Some(CodexErrorInfo::BadRequest),
                }),
            })
            .await;
        }
    }

    pub async fn user_input_or_turn(sess: &Arc<Session>, sub_id: String, op: Op) {
        user_input_or_turn_inner(
            sess,
            sub_id,
            op,
            /*mirror_user_text_to_realtime*/ Some(()),
        )
        .await;
    }

    pub(super) async fn user_input_or_turn_inner(
        sess: &Arc<Session>,
        sub_id: String,
        op: Op,
        mirror_user_text_to_realtime: Option<()>,
    ) {
        let (items, updates, responsesapi_client_metadata) = match op {
            Op::UserTurn {
                cwd,
                approval_policy,
                approvals_reviewer,
                sandbox_policy,
                model,
                effort,
                summary,
                service_tier,
                final_output_json_schema,
                items,
                collaboration_mode,
                personality,
            } => {
                let collaboration_mode = collaboration_mode.or_else(|| {
                    Some(CollaborationMode {
                        mode: ModeKind::Default,
                        settings: Settings {
                            model: model.clone(),
                            reasoning_effort: effort,
                            developer_instructions: None,
                        },
                    })
                });
                (
                    items,
                    SessionSettingsUpdate {
                        cwd: Some(cwd),
                        approval_policy: Some(approval_policy),
                        approvals_reviewer,
                        sandbox_policy: Some(sandbox_policy),
                        windows_sandbox_level: None,
                        collaboration_mode,
                        reasoning_summary: summary,
                        service_tier,
                        final_output_json_schema: Some(final_output_json_schema),
                        personality,
                        app_server_client_name: None,
                        app_server_client_version: None,
                    },
                    None,
                )
            }
            Op::UserInput {
                items,
                final_output_json_schema,
                responsesapi_client_metadata,
            } => (
                items,
                SessionSettingsUpdate {
                    final_output_json_schema: Some(final_output_json_schema),
                    ..Default::default()
                },
                responsesapi_client_metadata,
            ),
            _ => unreachable!(),
        };

        let Ok(current_context) = sess.new_turn_with_sub_id(sub_id.clone(), updates).await else {
            // new_turn_with_sub_id already emits the error event.
            return;
        };
        sess.maybe_emit_unknown_model_warning_for_turn(current_context.as_ref())
            .await;
        let accepted_items = match sess
            .steer_input(
                items.clone(),
                /*expected_turn_id*/ None,
                responsesapi_client_metadata.clone(),
            )
            .await
        {
            Ok(_) => {
                current_context.session_telemetry.user_prompt(&items);
                Some(items)
            }
            Err(SteerInputError::NoActiveTurn(items)) => {
                if let Some(responsesapi_client_metadata) = responsesapi_client_metadata {
                    current_context
                        .turn_metadata_state
                        .set_responsesapi_client_metadata(responsesapi_client_metadata);
                }
                current_context.session_telemetry.user_prompt(&items);
                sess.refresh_mcp_servers_if_requested(&current_context)
                    .await;
                let accepted_items = items.clone();
                sess.spawn_task(
                    Arc::clone(&current_context),
                    items,
                    crate::tasks::RegularTask::new(),
                )
                .await;
                Some(accepted_items)
            }
            Err(err) => {
                sess.send_event_raw(Event {
                    id: sub_id,
                    msg: EventMsg::Error(err.to_error_event()),
                })
                .await;
                None
            }
        };
        if let (Some(items), Some(())) = (accepted_items, mirror_user_text_to_realtime) {
            self::mirror_user_text_to_realtime(sess, &items).await;
        }
    }

    async fn mirror_user_text_to_realtime(sess: &Arc<Session>, items: &[UserInput]) {
        let text = UserMessageItem::new(items).message();
        if text.is_empty() {
            return;
        }
        let text = truncate_realtime_text_to_token_budget(&text, REALTIME_TURN_TOKEN_BUDGET);
        if text.is_empty() {
            return;
        }
        if sess.conversation.running_state().await.is_none() {
            return;
        }
        if let Err(err) = sess.conversation.text_in(text).await {
            debug!("failed to mirror user text to realtime conversation: {err}");
        }
    }

    /// Records an inter-agent assistant envelope, then lets the shared pending-work scheduler
    /// decide whether an idle session should start a regular turn.
    pub async fn inter_agent_communication(
        sess: &Arc<Session>,
        sub_id: String,
        communication: InterAgentCommunication,
    ) {
        let trigger_turn = communication.trigger_turn;
        sess.enqueue_mailbox_communication(communication);
        if trigger_turn {
            sess.maybe_start_turn_for_pending_work_with_sub_id(sub_id)
                .await;
        }
    }

    pub async fn run_user_shell_command(sess: &Arc<Session>, sub_id: String, command: String) {
        if let Some((turn_context, cancellation_token)) =
            sess.active_turn_context_and_cancellation_token().await
        {
            let session = Arc::clone(sess);
            tokio::spawn(async move {
                execute_user_shell_command(
                    session,
                    turn_context,
                    command,
                    cancellation_token,
                    UserShellCommandMode::ActiveTurnAuxiliary,
                )
                .await;
            });
            return;
        }

        let turn_context = sess.new_default_turn_with_sub_id(sub_id).await;
        sess.spawn_task(
            Arc::clone(&turn_context),
            Vec::new(),
            UserShellCommandTask::new(command),
        )
        .await;
    }

    pub async fn resolve_elicitation(
        sess: &Arc<Session>,
        server_name: String,
        request_id: ProtocolRequestId,
        decision: codex_protocol::approvals::ElicitationAction,
        content: Option<Value>,
        meta: Option<Value>,
    ) {
        let action = match decision {
            codex_protocol::approvals::ElicitationAction::Accept => ElicitationAction::Accept,
            codex_protocol::approvals::ElicitationAction::Decline => ElicitationAction::Decline,
            codex_protocol::approvals::ElicitationAction::Cancel => ElicitationAction::Cancel,
        };
        let content = match action {
            // Preserve the legacy fallback for clients that only send an action.
            ElicitationAction::Accept => Some(content.unwrap_or_else(|| serde_json::json!({}))),
            ElicitationAction::Decline | ElicitationAction::Cancel => None,
        };
        let response = ElicitationResponse {
            action,
            content,
            meta,
        };
        let request_id = match request_id {
            ProtocolRequestId::String(value) => {
                rmcp::model::NumberOrString::String(std::sync::Arc::from(value))
            }
            ProtocolRequestId::Integer(value) => rmcp::model::NumberOrString::Number(value),
        };
        if let Err(err) = sess
            .resolve_elicitation(server_name, request_id, response)
            .await
        {
            warn!(
                error = %err,
                "failed to resolve elicitation request in session"
            );
        }
    }

    /// Propagate a user's exec approval decision to the session.
    /// Also optionally applies an execpolicy amendment.
    pub async fn exec_approval(
        sess: &Arc<Session>,
        approval_id: String,
        turn_id: Option<String>,
        decision: ReviewDecision,
    ) {
        let event_turn_id = turn_id.unwrap_or_else(|| approval_id.clone());
        if let ReviewDecision::ApprovedExecpolicyAmendment {
            proposed_execpolicy_amendment,
        } = &decision
        {
            match sess
                .persist_execpolicy_amendment(proposed_execpolicy_amendment)
                .await
            {
                Ok(()) => {
                    sess.record_execpolicy_amendment_message(
                        &event_turn_id,
                        proposed_execpolicy_amendment,
                    )
                    .await;
                }
                Err(err) => {
                    let message = format!("Failed to apply execpolicy amendment: {err}");
                    tracing::warn!("{message}");
                    let warning = EventMsg::Warning(WarningEvent { message });
                    sess.send_event_raw(Event {
                        id: event_turn_id.clone(),
                        msg: warning,
                    })
                    .await;
                }
            }
        }
        match decision {
            ReviewDecision::Abort => {
                sess.interrupt_task().await;
            }
            other => sess.notify_approval(&approval_id, other).await,
        }
    }

    pub async fn patch_approval(sess: &Arc<Session>, id: String, decision: ReviewDecision) {
        match decision {
            ReviewDecision::Abort => {
                sess.interrupt_task().await;
            }
            other => sess.notify_approval(&id, other).await,
        }
    }

    pub async fn request_user_input_response(
        sess: &Arc<Session>,
        id: String,
        response: RequestUserInputResponse,
    ) {
        sess.notify_user_input_response(&id, response).await;
    }

    pub async fn request_permissions_response(
        sess: &Arc<Session>,
        id: String,
        response: RequestPermissionsResponse,
    ) {
        sess.notify_request_permissions_response(&id, response)
            .await;
    }

    pub async fn dynamic_tool_response(
        sess: &Arc<Session>,
        id: String,
        response: DynamicToolResponse,
    ) {
        sess.notify_dynamic_tool_response(&id, response).await;
    }

    pub async fn add_to_history(sess: &Arc<Session>, config: &Arc<Config>, text: String) {
        let id = sess.conversation_id;
        let config = Arc::clone(config);
        tokio::spawn(async move {
            if let Err(e) = crate::message_history::append_entry(&text, &id, &config).await {
                warn!("failed to append to message history: {e}");
            }
        });
    }

    pub async fn get_history_entry_request(
        sess: &Arc<Session>,
        config: &Arc<Config>,
        sub_id: String,
        offset: usize,
        log_id: u64,
    ) {
        let config = Arc::clone(config);
        let sess_clone = Arc::clone(sess);

        tokio::spawn(async move {
            // Run lookup in blocking thread because it does file IO + locking.
            let entry_opt = tokio::task::spawn_blocking(move || {
                crate::message_history::lookup(log_id, offset, &config)
            })
            .await
            .unwrap_or(None);

            let event = Event {
                id: sub_id,
                msg: EventMsg::GetHistoryEntryResponse(
                    codex_protocol::protocol::GetHistoryEntryResponseEvent {
                        offset,
                        log_id,
                        entry: entry_opt.map(|e| codex_protocol::message_history::HistoryEntry {
                            conversation_id: e.session_id,
                            ts: e.ts,
                            text: e.text,
                        }),
                    },
                ),
            };

            sess_clone.send_event_raw(event).await;
        });
    }

    pub async fn refresh_mcp_servers(sess: &Arc<Session>, refresh_config: McpServerRefreshConfig) {
        let mut guard = sess.pending_mcp_server_refresh_config.lock().await;
        *guard = Some(refresh_config);
    }

    pub async fn reload_user_config(sess: &Arc<Session>) {
        sess.reload_user_config_layer().await;
    }

    pub async fn list_mcp_tools(sess: &Session, config: &Arc<Config>, sub_id: String) {
        let mcp_connection_manager = sess.services.mcp_connection_manager.read().await;
        let auth = sess.services.auth_manager.auth().await;
        let mcp_servers = sess
            .services
            .mcp_manager
            .effective_servers(config, auth.as_ref())
            .await;
        let snapshot = collect_mcp_snapshot_from_manager(
            &mcp_connection_manager,
            compute_auth_statuses(mcp_servers.iter(), config.mcp_oauth_credentials_store_mode)
                .await,
        )
        .await;
        let event = Event {
            id: sub_id,
            msg: EventMsg::McpListToolsResponse(snapshot),
        };
        sess.send_event_raw(event).await;
    }

    pub async fn list_skills(
        sess: &Session,
        sub_id: String,
        cwds: Vec<PathBuf>,
        force_reload: bool,
    ) {
        let default_cwd = {
            let state = sess.state.lock().await;
            state.session_configuration.cwd.to_path_buf()
        };
        let cwds = if cwds.is_empty() {
            vec![default_cwd]
        } else {
            cwds
        };

        let skills_manager = &sess.services.skills_manager;
        let plugins_manager = &sess.services.plugins_manager;
        let fs = sess
            .services
            .environment
            .as_ref()
            .map(|environment| environment.get_filesystem());
        let config = sess.get_config().await;
        let codex_home = sess.codex_home().await;
        let mut skills = Vec::new();
        let empty_cli_overrides: &[(String, toml::Value)] = &[];
        for cwd in cwds {
            let cwd_abs = match AbsolutePathBuf::relative_to_current_dir(cwd.as_path()) {
                Ok(path) => path,
                Err(err) => {
                    let error_path = cwd.clone();
                    skills.push(SkillsListEntry {
                        cwd,
                        skills: Vec::new(),
                        errors: vec![SkillErrorInfo {
                            path: error_path,
                            message: err.to_string(),
                        }],
                    });
                    continue;
                }
            };
            let config_layer_stack = match load_config_layers_state(
                &codex_home,
                Some(cwd_abs.clone()),
                empty_cli_overrides,
                LoaderOverrides::default(),
                CloudRequirementsLoader::default(),
            )
            .await
            {
                Ok(config_layer_stack) => config_layer_stack,
                Err(err) => {
                    let error_path = cwd.clone();
                    skills.push(SkillsListEntry {
                        cwd,
                        skills: Vec::new(),
                        errors: vec![SkillErrorInfo {
                            path: error_path,
                            message: err.to_string(),
                        }],
                    });
                    continue;
                }
            };
            let effective_skill_roots = plugins_manager
                .effective_skill_roots_for_layer_stack(
                    &config_layer_stack,
                    config.features.enabled(Feature::Plugins),
                )
                .await;
            let skills_input = crate::SkillsLoadInput::new(
                cwd_abs.clone(),
                effective_skill_roots,
                config_layer_stack,
                config.bundled_skills_enabled(),
            );
            let outcome = skills_manager
                .skills_for_cwd(&skills_input, force_reload, fs.clone())
                .await;
            let errors = super::errors_to_info(&outcome.errors);
            let skills_metadata = super::skills_to_info(&outcome.skills, &outcome.disabled_paths);
            skills.push(SkillsListEntry {
                cwd,
                skills: skills_metadata,
                errors,
            });
        }

        let event = Event {
            id: sub_id,
            msg: EventMsg::ListSkillsResponse(ListSkillsResponseEvent { skills }),
        };
        sess.send_event_raw(event).await;
    }

    pub async fn undo(sess: &Arc<Session>, sub_id: String) {
        let turn_context = sess.new_default_turn_with_sub_id(sub_id).await;
        sess.spawn_task(turn_context, Vec::new(), UndoTask::new())
            .await;
    }

    pub async fn compact(sess: &Arc<Session>, sub_id: String) {
        let turn_context = sess.new_default_turn_with_sub_id(sub_id).await;

        sess.spawn_task(
            Arc::clone(&turn_context),
            vec![UserInput::Text {
                text: turn_context.compact_prompt().to_string(),
                // Compaction prompt is synthesized; no UI element ranges to preserve.
                text_elements: Vec::new(),
            }],
            CompactTask,
        )
        .await;
    }

    pub async fn drop_memories(sess: &Arc<Session>, config: &Arc<Config>, sub_id: String) {
        let mut errors = Vec::new();

        if let Some(state_db) = sess.services.state_db.as_deref() {
            if let Err(err) = state_db.clear_memory_data().await {
                errors.push(format!("failed clearing memory rows from state db: {err}"));
            }
        } else {
            errors.push("state db unavailable; memory rows were not cleared".to_string());
        }

        let memory_root = crate::memories::memory_root(&config.codex_home);
        if let Err(err) = crate::memories::clear_memory_root_contents(&memory_root).await {
            errors.push(format!(
                "failed clearing memory directory {}: {err}",
                memory_root.display()
            ));
        }

        if errors.is_empty() {
            sess.send_event_raw(Event {
                id: sub_id,
                msg: EventMsg::Warning(WarningEvent {
                    message: format!(
                        "Dropped memories at {} and cleared memory rows from state db.",
                        memory_root.display()
                    ),
                }),
            })
            .await;
            return;
        }

        sess.send_event_raw(Event {
            id: sub_id,
            msg: EventMsg::Error(ErrorEvent {
                message: format!("Memory drop completed with errors: {}", errors.join("; ")),
                codex_error_info: Some(CodexErrorInfo::Other),
            }),
        })
        .await;
    }

    pub async fn update_memories(sess: &Arc<Session>, config: &Arc<Config>, sub_id: String) {
        let session_source = {
            let state = sess.state.lock().await;
            state.session_configuration.session_source.clone()
        };

        crate::memories::start_memories_startup_task(sess, Arc::clone(config), &session_source);

        sess.send_event_raw(Event {
            id: sub_id.clone(),
            msg: EventMsg::Warning(WarningEvent {
                message: "Memory update triggered.".to_string(),
            }),
        })
        .await;
    }

    pub async fn thread_rollback(sess: &Arc<Session>, sub_id: String, num_turns: u32) {
        if num_turns == 0 {
            sess.send_event_raw(Event {
                id: sub_id,
                msg: EventMsg::Error(ErrorEvent {
                    message: "num_turns must be >= 1".to_string(),
                    codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
                }),
            })
            .await;
            return;
        }

        let has_active_turn = { sess.active_turn.lock().await.is_some() };
        if has_active_turn {
            sess.send_event_raw(Event {
                id: sub_id,
                msg: EventMsg::Error(ErrorEvent {
                    message: "Cannot rollback while a turn is in progress.".to_string(),
                    codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
                }),
            })
            .await;
            return;
        }

        let turn_context = sess.new_default_turn_with_sub_id(sub_id).await;
        let rollout_path = {
            let recorder = {
                let guard = sess.services.rollout.lock().await;
                guard.clone()
            };
            let Some(recorder) = recorder else {
                sess.send_event_raw(Event {
                    id: turn_context.sub_id.clone(),
                    msg: EventMsg::Error(ErrorEvent {
                        message: "thread rollback requires a persisted rollout path".to_string(),
                        codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
                    }),
                })
                .await;
                return;
            };
            recorder.rollout_path().to_path_buf()
        };
        if let Some(recorder) = {
            let guard = sess.services.rollout.lock().await;
            guard.clone()
        } && let Err(err) = recorder.flush().await
        {
            sess.send_event_raw(Event {
                id: turn_context.sub_id.clone(),
                msg: EventMsg::Error(ErrorEvent {
                    message: format!(
                        "failed to flush rollout `{}` for rollback replay: {err}",
                        rollout_path.display()
                    ),
                    codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
                }),
            })
            .await;
            return;
        }

        let initial_history =
            match RolloutRecorder::get_rollout_history(rollout_path.as_path()).await {
                Ok(history) => history,
                Err(err) => {
                    sess.send_event_raw(Event {
                        id: turn_context.sub_id.clone(),
                        msg: EventMsg::Error(ErrorEvent {
                            message: format!(
                                "failed to load rollout `{}` for rollback replay: {err}",
                                rollout_path.display()
                            ),
                            codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
                        }),
                    })
                    .await;
                    return;
                }
            };

        let rollback_event = ThreadRolledBackEvent { num_turns };
        let rollback_msg = EventMsg::ThreadRolledBack(rollback_event.clone());
        let replay_items = initial_history
            .get_rollout_items()
            .into_iter()
            .chain(std::iter::once(RolloutItem::EventMsg(rollback_msg.clone())))
            .collect::<Vec<_>>();
        sess.apply_rollout_reconstruction(turn_context.as_ref(), replay_items.as_slice())
            .await;
        sess.recompute_token_usage(turn_context.as_ref()).await;

        sess.persist_rollout_items(&[RolloutItem::EventMsg(rollback_msg.clone())])
            .await;
        if let Err(err) = sess.flush_rollout().await {
            sess.send_event(
                turn_context.as_ref(),
                EventMsg::Warning(WarningEvent {
                    message: format!(
                        "Rolled the thread back, but failed to save the rollback marker. Codex will continue retrying. Error: {err}"
                    ),
                }),
            )
            .await;
        }

        sess.deliver_event_raw(Event {
            id: turn_context.sub_id.clone(),
            msg: rollback_msg,
        })
        .await;
    }

    async fn persist_thread_name_update(
        sess: &Arc<Session>,
        event: ThreadNameUpdatedEvent,
    ) -> anyhow::Result<EventMsg> {
        let msg = EventMsg::ThreadNameUpdated(event);
        let item = RolloutItem::EventMsg(msg.clone());
        let recorder = {
            let guard = sess.services.rollout.lock().await;
            guard.clone()
        }
        .ok_or_else(|| anyhow::anyhow!("Session persistence is disabled; cannot rename thread."))?;
        recorder.persist().await?;
        recorder.record_items(std::slice::from_ref(&item)).await?;
        recorder.flush().await?;
        Ok(msg)
    }

    pub(super) async fn persist_thread_memory_mode_update(
        sess: &Arc<Session>,
        mode: ThreadMemoryMode,
    ) -> anyhow::Result<()> {
        let recorder = {
            let guard = sess.services.rollout.lock().await;
            guard.clone()
        }
        .ok_or_else(|| {
            anyhow::anyhow!("Session persistence is disabled; cannot update thread memory mode.")
        })?;
        recorder.persist().await?;
        recorder.flush().await?;

        let rollout_path = recorder.rollout_path().to_path_buf();
        let mut session_meta = read_session_meta_line(rollout_path.as_path()).await?;
        if session_meta.meta.id != sess.conversation_id {
            anyhow::bail!(
                "rollout session metadata id mismatch: expected {}, found {}",
                sess.conversation_id,
                session_meta.meta.id
            );
        }
        session_meta.meta.memory_mode = Some(
            match mode {
                ThreadMemoryMode::Enabled => "enabled",
                ThreadMemoryMode::Disabled => "disabled",
            }
            .to_string(),
        );

        let item = RolloutItem::SessionMeta(session_meta);
        recorder.record_items(std::slice::from_ref(&item)).await?;
        recorder.flush().await?;
        Ok(())
    }

    /// Persists the thread name in the rollout and state database, updates in-memory state, and
    /// emits a `ThreadNameUpdated` event on success.
    pub async fn set_thread_name(sess: &Arc<Session>, sub_id: String, name: String) {
        let Some(name) = crate::util::normalize_thread_name(&name) else {
            let event = Event {
                id: sub_id,
                msg: EventMsg::Error(ErrorEvent {
                    message: "Thread name cannot be empty.".to_string(),
                    codex_error_info: Some(CodexErrorInfo::BadRequest),
                }),
            };
            sess.send_event_raw(event).await;
            return;
        };

        let updated = ThreadNameUpdatedEvent {
            thread_id: sess.conversation_id,
            thread_name: Some(name.clone()),
        };

        let msg = match persist_thread_name_update(sess, updated).await {
            Ok(msg) => msg,
            Err(err) => {
                warn!("Failed to persist thread name update to rollout: {err}");
                let event = Event {
                    id: sub_id,
                    msg: EventMsg::Error(ErrorEvent {
                        message: err.to_string(),
                        codex_error_info: Some(CodexErrorInfo::Other),
                    }),
                };
                sess.send_event_raw(event).await;
                return;
            }
        };

        if let Some(state_db) = sess.services.state_db.as_deref()
            && let Err(err) = state_db
                .update_thread_title(sess.conversation_id, &name)
                .await
        {
            warn!("Failed to update thread title in state db: {err}");
        }

        {
            let mut state = sess.state.lock().await;
            state.session_configuration.thread_name = Some(name.clone());
        }

        let codex_home = sess.codex_home().await;
        if let Err(err) =
            crate::rollout::append_thread_name(&codex_home, sess.conversation_id, &name).await
        {
            warn!("Failed to update legacy thread name index: {err}");
        }

        sess.deliver_event_raw(Event { id: sub_id, msg }).await;
    }

    /// Persists thread-level memory mode metadata for the active session.
    ///
    /// This does not involve the model and only affects whether the thread is
    /// eligible for future memory generation.
    pub async fn set_thread_memory_mode(
        sess: &Arc<Session>,
        sub_id: String,
        mode: ThreadMemoryMode,
    ) {
        if let Err(err) = persist_thread_memory_mode_update(sess, mode).await {
            warn!("Failed to persist thread memory mode update to rollout: {err}");
            let event = Event {
                id: sub_id,
                msg: EventMsg::Error(ErrorEvent {
                    message: err.to_string(),
                    codex_error_info: Some(CodexErrorInfo::Other),
                }),
            };
            sess.send_event_raw(event).await;
        }
    }

    pub async fn shutdown(sess: &Arc<Session>, sub_id: String) -> bool {
        sess.abort_all_tasks(TurnAbortReason::Interrupted).await;
        let _ = sess.conversation.shutdown().await;
        sess.services
            .unified_exec_manager
            .terminate_all_processes()
            .await;
        sess.guardian_review_session.shutdown().await;
        info!("Shutting down Codex instance");
        let history = sess.clone_history().await;
        let turn_count = history
            .raw_items()
            .iter()
            .filter(|item| is_user_turn_boundary(item))
            .count();
        sess.services.session_telemetry.counter(
            "codex.conversation.turn.count",
            i64::try_from(turn_count).unwrap_or(0),
            &[],
        );

        // Gracefully flush and shutdown rollout recorder on session end so tests
        // that inspect the rollout file do not race with the background writer.
        let recorder_opt = {
            let mut guard = sess.services.rollout.lock().await;
            guard.take()
        };
        if let Some(rec) = recorder_opt
            && let Err(e) = rec.shutdown().await
        {
            warn!("failed to shutdown rollout recorder: {e}");
            let event = Event {
                id: sub_id.clone(),
                msg: EventMsg::Error(ErrorEvent {
                    message: "Failed to shutdown rollout recorder".to_string(),
                    codex_error_info: Some(CodexErrorInfo::Other),
                }),
            };
            sess.send_event_raw(event).await;
        }

        let event = Event {
            id: sub_id,
            msg: EventMsg::ShutdownComplete,
        };
        sess.send_event_raw(event).await;
        true
    }

    pub async fn review(
        sess: &Arc<Session>,
        config: &Arc<Config>,
        sub_id: String,
        review_request: ReviewRequest,
    ) {
        let turn_context = sess.new_default_turn_with_sub_id(sub_id.clone()).await;
        sess.maybe_emit_unknown_model_warning_for_turn(turn_context.as_ref())
            .await;
        sess.refresh_mcp_servers_if_requested(&turn_context).await;
        match resolve_review_request(review_request, &turn_context.cwd) {
            Ok(resolved) => {
                spawn_review_thread(
                    Arc::clone(sess),
                    Arc::clone(config),
                    turn_context.clone(),
                    sub_id,
                    resolved,
                )
                .await;
            }
            Err(err) => {
                let event = Event {
                    id: sub_id,
                    msg: EventMsg::Error(ErrorEvent {
                        message: err.to_string(),
                        codex_error_info: Some(CodexErrorInfo::Other),
                    }),
                };
                sess.send_event(&turn_context, event.msg).await;
            }
        }
    }
}

/// Spawn a review thread using the given prompt.
async fn spawn_review_thread(
    sess: Arc<Session>,
    config: Arc<Config>,
    parent_turn_context: Arc<TurnContext>,
    sub_id: String,
    resolved: crate::review_prompts::ResolvedReviewRequest,
) {
    let model = config
        .review_model
        .clone()
        .unwrap_or_else(|| parent_turn_context.model_info.slug.clone());
    let review_model_info = sess
        .services
        .models_manager
        .get_model_info(&model, &config.to_models_manager_config())
        .await;
    // For reviews, disable web_search and view_image regardless of global settings.
    let mut review_features = sess.features.clone();
    let _ = review_features.disable(Feature::WebSearchRequest);
    let _ = review_features.disable(Feature::WebSearchCached);
    let review_web_search_mode = WebSearchMode::Disabled;
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_info: &review_model_info,
        available_models: &sess
            .services
            .models_manager
            .list_models(RefreshStrategy::OnlineIfUncached)
            .await,
        features: &review_features,
        image_generation_tool_auth_allowed: image_generation_tool_auth_allowed(Some(
            sess.services.auth_manager.as_ref(),
        )),
        web_search_mode: Some(review_web_search_mode),
        session_source: parent_turn_context.session_source.clone(),
        sandbox_policy: parent_turn_context.sandbox_policy.get(),
        windows_sandbox_level: parent_turn_context.windows_sandbox_level,
    })
    .with_unified_exec_shell_mode_for_session(
        crate::tools::spec::tool_user_shell_type(sess.services.user_shell.as_ref()),
        sess.services.shell_zsh_path.as_ref(),
        sess.services.main_execve_wrapper_exe.as_ref(),
    )
    .with_web_search_config(/*web_search_config*/ None)
    .with_allow_login_shell(config.permissions.allow_login_shell)
    .with_has_environment(parent_turn_context.environment.is_some())
    .with_spawn_agent_usage_hint(config.multi_agent_v2.usage_hint_enabled)
    .with_spawn_agent_usage_hint_text(config.multi_agent_v2.usage_hint_text.clone())
    .with_hide_spawn_agent_metadata(config.multi_agent_v2.hide_spawn_agent_metadata)
    .with_agent_type_description(crate::agent::role::spawn_tool_spec::build(
        &config.agent_roles,
    ));

    let review_prompt = resolved.prompt.clone();
    let provider = parent_turn_context.provider.clone();
    let auth_manager = parent_turn_context.auth_manager.clone();
    let model_info = review_model_info.clone();

    // Build per‑turn client with the requested model/family.
    let mut per_turn_config = (*config).clone();
    per_turn_config.model = Some(model.clone());
    per_turn_config.features = review_features.clone();
    if let Err(err) = per_turn_config.web_search_mode.set(review_web_search_mode) {
        let fallback_value = per_turn_config.web_search_mode.value();
        tracing::warn!(
            error = %err,
            ?review_web_search_mode,
            ?fallback_value,
            "review web_search_mode is disallowed by requirements; keeping constrained value"
        );
    }

    let session_telemetry = parent_turn_context
        .session_telemetry
        .clone()
        .with_model(model.as_str(), review_model_info.slug.as_str());
    let auth_manager_for_context = auth_manager.clone();
    let provider_for_context = provider.clone();
    let session_telemetry_for_context = session_telemetry.clone();
    let reasoning_effort = per_turn_config.model_reasoning_effort;
    let reasoning_summary = per_turn_config
        .model_reasoning_summary
        .unwrap_or(model_info.default_reasoning_summary);
    let session_source = parent_turn_context.session_source.clone();

    let per_turn_config = Arc::new(per_turn_config);
    let review_turn_id = sub_id.to_string();
    let turn_metadata_state = Arc::new(TurnMetadataState::new(
        sess.conversation_id.to_string(),
        &session_source,
        review_turn_id.clone(),
        parent_turn_context.cwd.clone(),
        parent_turn_context.sandbox_policy.get(),
        parent_turn_context.windows_sandbox_level,
    ));

    let review_turn_context = TurnContext {
        sub_id: review_turn_id,
        trace_id: current_span_trace_id(),
        realtime_active: parent_turn_context.realtime_active,
        config: per_turn_config,
        auth_manager: auth_manager_for_context,
        model_info: model_info.clone(),
        session_telemetry: session_telemetry_for_context,
        provider: provider_for_context,
        reasoning_effort,
        reasoning_summary,
        session_source,
        environment: parent_turn_context.environment.clone(),
        tools_config,
        features: parent_turn_context.features.clone(),
        ghost_snapshot: parent_turn_context.ghost_snapshot.clone(),
        current_date: parent_turn_context.current_date.clone(),
        timezone: parent_turn_context.timezone.clone(),
        app_server_client_name: parent_turn_context.app_server_client_name.clone(),
        developer_instructions: None,
        user_instructions: None,
        compact_prompt: parent_turn_context.compact_prompt.clone(),
        collaboration_mode: parent_turn_context.collaboration_mode.clone(),
        personality: parent_turn_context.personality,
        approval_policy: parent_turn_context.approval_policy.clone(),
        sandbox_policy: parent_turn_context.sandbox_policy.clone(),
        file_system_sandbox_policy: parent_turn_context.file_system_sandbox_policy.clone(),
        network_sandbox_policy: parent_turn_context.network_sandbox_policy,
        network: parent_turn_context.network.clone(),
        windows_sandbox_level: parent_turn_context.windows_sandbox_level,
        shell_environment_policy: parent_turn_context.shell_environment_policy.clone(),
        cwd: parent_turn_context.cwd.clone(),
        final_output_json_schema: None,
        codex_self_exe: parent_turn_context.codex_self_exe.clone(),
        codex_linux_sandbox_exe: parent_turn_context.codex_linux_sandbox_exe.clone(),
        tool_call_gate: Arc::new(ReadinessFlag::new()),
        js_repl: Arc::clone(&sess.js_repl),
        dynamic_tools: parent_turn_context.dynamic_tools.clone(),
        truncation_policy: model_info.truncation_policy.into(),
        turn_metadata_state,
        turn_skills: TurnSkillsContext::new(parent_turn_context.turn_skills.outcome.clone()),
        turn_timing_state: Arc::new(TurnTimingState::default()),
    };

    // Seed the child task with the review prompt as the initial user message.
    let input: Vec<UserInput> = vec![UserInput::Text {
        text: review_prompt,
        // Review prompt is synthesized; no UI element ranges to preserve.
        text_elements: Vec::new(),
    }];
    let tc = Arc::new(review_turn_context);
    tc.turn_metadata_state.spawn_git_enrichment_task();
    // TODO(ccunningham): Review turns currently rely on `spawn_task` for TurnComplete but do not
    // emit a parent TurnStarted. Consider giving review a full parent turn lifecycle
    // (TurnStarted + TurnComplete) for consistency with other standalone tasks.
    sess.spawn_task(tc.clone(), input, ReviewTask::new()).await;

    // Announce entering review mode so UIs can switch modes.
    let review_request = ReviewRequest {
        target: resolved.target,
        user_facing_hint: Some(resolved.user_facing_hint),
    };
    sess.send_event(&tc, EventMsg::EnteredReviewMode(review_request))
        .await;
}

fn skills_to_info(
    skills: &[SkillMetadata],
    disabled_paths: &HashSet<AbsolutePathBuf>,
) -> Vec<ProtocolSkillMetadata> {
    skills
        .iter()
        .map(|skill| ProtocolSkillMetadata {
            name: skill.name.clone(),
            description: skill.description.clone(),
            short_description: skill.short_description.clone(),
            interface: skill
                .interface
                .clone()
                .map(|interface| ProtocolSkillInterface {
                    display_name: interface.display_name,
                    short_description: interface.short_description,
                    icon_small: interface.icon_small,
                    icon_large: interface.icon_large,
                    brand_color: interface.brand_color,
                    default_prompt: interface.default_prompt,
                }),
            dependencies: skill.dependencies.clone().map(|dependencies| {
                ProtocolSkillDependencies {
                    tools: dependencies
                        .tools
                        .into_iter()
                        .map(|tool| ProtocolSkillToolDependency {
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
            scope: skill.scope,
            enabled: !disabled_paths.contains(&skill.path_to_skills_md),
        })
        .collect()
}

fn errors_to_info(errors: &[SkillError]) -> Vec<SkillErrorInfo> {
    errors
        .iter()
        .map(|err| SkillErrorInfo {
            path: err.path.to_path_buf(),
            message: err.message.clone(),
        })
        .collect()
}

/// Takes a user message as input and runs a loop where, at each sampling request, the model
/// replies with either:
///
/// - requested function calls
/// - an assistant message
///
/// While it is possible for the model to return multiple of these items in a
/// single sampling request, in practice, we generally one item per sampling request:
///
/// - If the model requests a function call, we execute it and send the output
///   back to the model in the next sampling request.
/// - If the model sends only an assistant message, we record it in the
///   conversation history and consider the turn complete.
///
pub(crate) async fn run_turn(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    input: Vec<UserInput>,
    prewarmed_client_session: Option<ModelClientSession>,
    cancellation_token: CancellationToken,
) -> Option<String> {
    if input.is_empty() && !sess.has_pending_input().await {
        return None;
    }

    let model_info = turn_context.model_info.clone();
    let auto_compact_limit = model_info.auto_compact_token_limit().unwrap_or(i64::MAX);
    let mut prewarmed_client_session = prewarmed_client_session;
    // TODO(ccunningham): Pre-turn compaction runs before context updates and the
    // new user message are recorded. Estimate pending incoming items (context
    // diffs/full reinjection + user input) and trigger compaction preemptively
    // when they would push the thread over the compaction threshold.
    let pre_sampling_compacted = match run_pre_sampling_compact(&sess, &turn_context).await {
        Ok(pre_sampling_compacted) => pre_sampling_compacted,
        Err(_) => {
            error!("Failed to run pre-sampling compact");
            return None;
        }
    };
    if pre_sampling_compacted && let Some(mut client_session) = prewarmed_client_session.take() {
        client_session.reset_websocket_session();
    }

    let skills_outcome = Some(turn_context.turn_skills.outcome.as_ref());

    sess.record_context_updates_and_set_reference_context_item(turn_context.as_ref())
        .await;

    let loaded_plugins = sess
        .services
        .plugins_manager
        .plugins_for_config(&turn_context.config)
        .await;
    // Structured plugin:// mentions are resolved from the current session's
    // enabled plugins, then converted into turn-scoped guidance below.
    let mentioned_plugins =
        collect_explicit_plugin_mentions(&input, loaded_plugins.capability_summaries());
    let mcp_tools = if turn_context.apps_enabled() || !mentioned_plugins.is_empty() {
        // Plugin mentions need raw MCP/app inventory even when app tools
        // are normally hidden so we can describe the plugin's currently
        // usable capabilities for this turn.
        match sess
            .services
            .mcp_connection_manager
            .read()
            .await
            .list_all_tools()
            .or_cancel(&cancellation_token)
            .await
        {
            Ok(mcp_tools) => mcp_tools,
            Err(_) if turn_context.apps_enabled() => return None,
            Err(_) => HashMap::new(),
        }
    } else {
        HashMap::new()
    };
    let available_connectors = if turn_context.apps_enabled() {
        let connectors = connectors::merge_plugin_apps_with_accessible(
            loaded_plugins.effective_apps(),
            connectors::accessible_connectors_from_mcp_tools(&mcp_tools),
        );
        connectors::with_app_enabled_state(connectors, &turn_context.config)
    } else {
        Vec::new()
    };
    let connector_slug_counts = build_connector_slug_counts(&available_connectors);
    let skill_name_counts_lower = skills_outcome
        .as_ref()
        .map_or_else(HashMap::new, |outcome| {
            build_skill_name_counts(&outcome.skills, &outcome.disabled_paths).1
        });
    let mentioned_skills = skills_outcome.as_ref().map_or_else(Vec::new, |outcome| {
        collect_explicit_skill_mentions(
            &input,
            &outcome.skills,
            &outcome.disabled_paths,
            &connector_slug_counts,
        )
    });
    let config = turn_context.config.clone();
    if config
        .features
        .enabled(Feature::SkillEnvVarDependencyPrompt)
    {
        let env_var_dependencies = collect_env_var_dependencies(&mentioned_skills);
        resolve_skill_dependencies_for_turn(&sess, &turn_context, &env_var_dependencies).await;
    }

    maybe_prompt_and_install_mcp_dependencies(
        sess.as_ref(),
        turn_context.as_ref(),
        &cancellation_token,
        &mentioned_skills,
    )
    .await;

    let session_telemetry = turn_context.session_telemetry.clone();
    let thread_id = sess.conversation_id.to_string();
    let tracking = build_track_events_context(
        turn_context.model_info.slug.clone(),
        thread_id,
        turn_context.sub_id.clone(),
    );
    let SkillInjections {
        items: skill_items,
        warnings: skill_warnings,
    } = build_skill_injections(
        &mentioned_skills,
        skills_outcome,
        Some(&session_telemetry),
        &sess.services.analytics_events_client,
        tracking.clone(),
    )
    .await;

    for message in skill_warnings {
        sess.send_event(&turn_context, EventMsg::Warning(WarningEvent { message }))
            .await;
    }

    let plugin_items =
        build_plugin_injections(&mentioned_plugins, &mcp_tools, &available_connectors);
    let mentioned_plugin_metadata = mentioned_plugins
        .iter()
        .filter_map(crate::plugins::PluginCapabilitySummary::telemetry_metadata)
        .collect::<Vec<_>>();

    let mut explicitly_enabled_connectors = collect_explicit_app_ids(&input);
    explicitly_enabled_connectors.extend(collect_explicit_app_ids_from_skill_items(
        &skill_items,
        &available_connectors,
        &skill_name_counts_lower,
    ));
    let connector_names_by_id = available_connectors
        .iter()
        .map(|connector| (connector.id.as_str(), connector.name.as_str()))
        .collect::<HashMap<&str, &str>>();
    let mentioned_app_invocations = explicitly_enabled_connectors
        .iter()
        .map(|connector_id| AppInvocation {
            connector_id: Some(connector_id.clone()),
            app_name: connector_names_by_id
                .get(connector_id.as_str())
                .map(|name| (*name).to_string()),
            invocation_type: Some(InvocationType::Explicit),
        })
        .collect::<Vec<_>>();

    if run_pending_session_start_hooks(&sess, &turn_context).await {
        return None;
    }
    let additional_contexts = if input.is_empty() {
        Vec::new()
    } else {
        let initial_input_for_turn: ResponseInputItem = ResponseInputItem::from(input.clone());
        let response_item: ResponseItem = initial_input_for_turn.clone().into();
        let user_prompt_submit_outcome = run_user_prompt_submit_hooks(
            &sess,
            &turn_context,
            UserMessageItem::new(&input).message(),
        )
        .await;
        if user_prompt_submit_outcome.should_stop {
            record_additional_contexts(
                &sess,
                &turn_context,
                user_prompt_submit_outcome.additional_contexts,
            )
            .await;
            return None;
        }
        sess.record_user_prompt_and_emit_turn_item(turn_context.as_ref(), &input, response_item)
            .await;
        user_prompt_submit_outcome.additional_contexts
    };
    sess.services
        .analytics_events_client
        .track_app_mentioned(tracking.clone(), mentioned_app_invocations);
    for plugin in mentioned_plugin_metadata {
        sess.services
            .analytics_events_client
            .track_plugin_used(tracking.clone(), plugin);
    }
    sess.merge_connector_selection(explicitly_enabled_connectors.clone())
        .await;
    record_additional_contexts(&sess, &turn_context, additional_contexts).await;
    if !input.is_empty() {
        // Track the previous-turn baseline from the regular user-turn path only so
        // standalone tasks (compact/shell/review/undo) cannot suppress future
        // model/realtime injections.
        sess.set_previous_turn_settings(Some(PreviousTurnSettings {
            model: turn_context.model_info.slug.clone(),
            realtime_active: Some(turn_context.realtime_active),
        }))
        .await;
    }

    if !skill_items.is_empty() {
        sess.record_conversation_items(&turn_context, &skill_items)
            .await;
    }
    if !plugin_items.is_empty() {
        sess.record_conversation_items(&turn_context, &plugin_items)
            .await;
    }

    track_turn_resolved_config_analytics(&sess, &turn_context, &input).await;

    let skills_outcome = Some(turn_context.turn_skills.outcome.as_ref());
    sess.maybe_start_ghost_snapshot(Arc::clone(&turn_context), cancellation_token.child_token())
        .await;
    let mut last_agent_message: Option<String> = None;
    let mut stop_hook_active = false;
    // Although from the perspective of codex.rs, TurnDiffTracker has the lifecycle of a Task which contains
    // many turns, from the perspective of the user, it is a single turn.
    let turn_diff_tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));
    let mut server_model_warning_emitted_for_turn = false;

    // `ModelClientSession` is turn-scoped and caches WebSocket + sticky routing state, so we reuse
    // one instance across retries within this turn.
    let mut client_session =
        prewarmed_client_session.unwrap_or_else(|| sess.services.model_client.new_session());
    // Pending input is drained into history before building the next model request.
    // However, we defer that drain until after sampling in two cases:
    // 1. At the start of a turn, so the fresh user prompt in `input` gets sampled first.
    // 2. After auto-compact, when model/tool continuation needs to resume before any steer.
    let mut can_drain_pending_input = input.is_empty();

    loop {
        if run_pending_session_start_hooks(&sess, &turn_context).await {
            break;
        }

        // Note that pending_input would be something like a message the user
        // submitted through the UI while the model was running. Though the UI
        // may support this, the model might not.
        let pending_input = if can_drain_pending_input {
            sess.get_pending_input().await
        } else {
            Vec::new()
        };

        let mut blocked_pending_input = false;
        let mut blocked_pending_input_contexts = Vec::new();
        let mut requeued_pending_input = false;
        let mut accepted_pending_input = Vec::new();
        if !pending_input.is_empty() {
            let mut pending_input_iter = pending_input.into_iter();
            while let Some(pending_input_item) = pending_input_iter.next() {
                match inspect_pending_input(&sess, &turn_context, pending_input_item).await {
                    PendingInputHookDisposition::Accepted(pending_input) => {
                        accepted_pending_input.push(*pending_input);
                    }
                    PendingInputHookDisposition::Blocked {
                        additional_contexts,
                    } => {
                        let remaining_pending_input = pending_input_iter.collect::<Vec<_>>();
                        if !remaining_pending_input.is_empty() {
                            let _ = sess.prepend_pending_input(remaining_pending_input).await;
                            requeued_pending_input = true;
                        }
                        blocked_pending_input_contexts = additional_contexts;
                        blocked_pending_input = true;
                        break;
                    }
                }
            }
        }

        let has_accepted_pending_input = !accepted_pending_input.is_empty();
        for pending_input in accepted_pending_input {
            record_pending_input(&sess, &turn_context, pending_input).await;
        }
        record_additional_contexts(&sess, &turn_context, blocked_pending_input_contexts).await;

        if blocked_pending_input && !has_accepted_pending_input {
            if requeued_pending_input {
                continue;
            }
            break;
        }

        // Construct the input that we will send to the model.
        let sampling_request_input: Vec<ResponseItem> = {
            sess.clone_history()
                .await
                .for_prompt(&turn_context.model_info.input_modalities)
        };

        let sampling_request_input_messages = sampling_request_input
            .iter()
            .filter_map(|item| match parse_turn_item(item) {
                Some(TurnItem::UserMessage(user_message)) => Some(user_message),
                _ => None,
            })
            .map(|user_message| user_message.message())
            .collect::<Vec<String>>();
        let turn_metadata_header = turn_context.turn_metadata_state.current_header_value();
        match run_sampling_request(
            Arc::clone(&sess),
            Arc::clone(&turn_context),
            Arc::clone(&turn_diff_tracker),
            &mut client_session,
            turn_metadata_header.as_deref(),
            sampling_request_input,
            &explicitly_enabled_connectors,
            skills_outcome,
            &mut server_model_warning_emitted_for_turn,
            cancellation_token.child_token(),
        )
        .await
        {
            Ok(sampling_request_output) => {
                let SamplingRequestResult {
                    needs_follow_up: model_needs_follow_up,
                    last_agent_message: sampling_request_last_agent_message,
                } = sampling_request_output;
                can_drain_pending_input = true;
                let has_pending_input = sess.has_pending_input().await;
                let needs_follow_up = model_needs_follow_up || has_pending_input;
                let total_usage_tokens = sess.get_total_token_usage().await;
                let token_limit_reached = total_usage_tokens >= auto_compact_limit;

                let estimated_token_count =
                    sess.get_estimated_token_count(turn_context.as_ref()).await;

                trace!(
                    turn_id = %turn_context.sub_id,
                    total_usage_tokens,
                    estimated_token_count = ?estimated_token_count,
                    auto_compact_limit,
                    token_limit_reached,
                    model_needs_follow_up,
                    has_pending_input,
                    needs_follow_up,
                    "post sampling token usage"
                );

                // as long as compaction works well in getting us way below the token limit, we shouldn't worry about being in an infinite loop.
                if token_limit_reached && needs_follow_up {
                    if run_auto_compact(
                        &sess,
                        &turn_context,
                        InitialContextInjection::BeforeLastUserMessage,
                        CompactionReason::ContextLimit,
                        CompactionPhase::MidTurn,
                    )
                    .await
                    .is_err()
                    {
                        return None;
                    }
                    client_session.reset_websocket_session();
                    can_drain_pending_input = !model_needs_follow_up;
                    continue;
                }

                if !needs_follow_up {
                    last_agent_message = sampling_request_last_agent_message;
                    let stop_hook_permission_mode = match turn_context.approval_policy.value() {
                        AskForApproval::Never => "bypassPermissions",
                        AskForApproval::UnlessTrusted
                        | AskForApproval::OnFailure
                        | AskForApproval::OnRequest
                        | AskForApproval::Granular(_) => "default",
                    }
                    .to_string();
                    let stop_request = codex_hooks::StopRequest {
                        session_id: sess.conversation_id,
                        turn_id: turn_context.sub_id.clone(),
                        cwd: turn_context.cwd.clone(),
                        transcript_path: sess.hook_transcript_path().await,
                        model: turn_context.model_info.slug.clone(),
                        permission_mode: stop_hook_permission_mode,
                        stop_hook_active,
                        last_assistant_message: last_agent_message.clone(),
                    };
                    for run in sess.hooks().preview_stop(&stop_request) {
                        sess.send_event(
                            &turn_context,
                            EventMsg::HookStarted(codex_protocol::protocol::HookStartedEvent {
                                turn_id: Some(turn_context.sub_id.clone()),
                                run,
                            }),
                        )
                        .await;
                    }
                    let stop_outcome = sess.hooks().run_stop(stop_request).await;
                    for completed in stop_outcome.hook_events {
                        sess.send_event(&turn_context, EventMsg::HookCompleted(completed))
                            .await;
                    }
                    if stop_outcome.should_block {
                        if let Some(hook_prompt_message) =
                            build_hook_prompt_message(&stop_outcome.continuation_fragments)
                        {
                            sess.record_conversation_items(
                                &turn_context,
                                std::slice::from_ref(&hook_prompt_message),
                            )
                            .await;
                            stop_hook_active = true;
                            continue;
                        } else {
                            sess.send_event(
                                &turn_context,
                                EventMsg::Warning(WarningEvent {
                                    message: "Stop hook requested continuation without a prompt; ignoring the block.".to_string(),
                                }),
                            )
                            .await;
                        }
                    }
                    if stop_outcome.should_stop {
                        break;
                    }
                    let hook_outcomes = sess
                        .hooks()
                        .dispatch(HookPayload {
                            session_id: sess.conversation_id,
                            cwd: turn_context.cwd.clone(),
                            client: turn_context.app_server_client_name.clone(),
                            triggered_at: chrono::Utc::now(),
                            hook_event: HookEvent::AfterAgent {
                                event: HookEventAfterAgent {
                                    thread_id: sess.conversation_id,
                                    turn_id: turn_context.sub_id.clone(),
                                    input_messages: sampling_request_input_messages,
                                    last_assistant_message: last_agent_message.clone(),
                                },
                            },
                        })
                        .await;

                    let mut abort_message = None;
                    for hook_outcome in hook_outcomes {
                        let hook_name = hook_outcome.hook_name;
                        match hook_outcome.result {
                            HookResult::Success => {}
                            HookResult::FailedContinue(error) => {
                                warn!(
                                    turn_id = %turn_context.sub_id,
                                    hook_name = %hook_name,
                                    error = %error,
                                    "after_agent hook failed; continuing"
                                );
                            }
                            HookResult::FailedAbort(error) => {
                                let message = format!(
                                    "after_agent hook '{hook_name}' failed and aborted turn completion: {error}"
                                );
                                warn!(
                                    turn_id = %turn_context.sub_id,
                                    hook_name = %hook_name,
                                    error = %error,
                                    "after_agent hook failed; aborting operation"
                                );
                                if abort_message.is_none() {
                                    abort_message = Some(message);
                                }
                            }
                        }
                    }
                    if let Some(message) = abort_message {
                        sess.send_event(
                            &turn_context,
                            EventMsg::Error(ErrorEvent {
                                message,
                                codex_error_info: None,
                            }),
                        )
                        .await;
                        return None;
                    }
                    break;
                }
                continue;
            }
            Err(CodexErr::TurnAborted) => {
                // Aborted turn is reported via a different event.
                break;
            }
            Err(CodexErr::InvalidImageRequest()) => {
                let mut state = sess.state.lock().await;
                error_or_panic(
                    "Invalid image detected; sanitizing tool output to prevent poisoning",
                );
                if state.history.replace_last_turn_images("Invalid image") {
                    continue;
                }
                let event = EventMsg::Error(ErrorEvent {
                    message: "Invalid image in your last message. Please remove it and try again."
                        .to_string(),
                    codex_error_info: Some(CodexErrorInfo::BadRequest),
                });
                sess.send_event(&turn_context, event).await;
                break;
            }
            Err(e) => {
                info!("Turn error: {e:#}");
                let event = EventMsg::Error(e.to_error_event(/*message_prefix*/ None));
                sess.send_event(&turn_context, event).await;
                // let the user continue the conversation
                break;
            }
        }
    }

    last_agent_message
}

async fn track_turn_resolved_config_analytics(
    sess: &Session,
    turn_context: &TurnContext,
    input: &[UserInput],
) {
    if !sess.enabled(Feature::GeneralAnalytics) {
        return;
    }

    let thread_config = {
        let state = sess.state.lock().await;
        state.session_configuration.thread_config_snapshot()
    };
    let is_first_turn = {
        let mut state = sess.state.lock().await;
        state.take_next_turn_is_first()
    };
    sess.services
        .analytics_events_client
        .track_turn_resolved_config(TurnResolvedConfigFact {
            turn_id: turn_context.sub_id.clone(),
            thread_id: sess.conversation_id.to_string(),
            num_input_images: input
                .iter()
                .filter(|item| {
                    matches!(item, UserInput::Image { .. } | UserInput::LocalImage { .. })
                })
                .count(),
            submission_type: None,
            ephemeral: thread_config.ephemeral,
            session_source: thread_config.session_source,
            model: turn_context.model_info.slug.clone(),
            model_provider: turn_context.config.model_provider_id.clone(),
            sandbox_policy: turn_context.sandbox_policy.get().clone(),
            reasoning_effort: turn_context.reasoning_effort,
            reasoning_summary: Some(turn_context.reasoning_summary),
            service_tier: turn_context.config.service_tier,
            approval_policy: turn_context.approval_policy.value(),
            approvals_reviewer: turn_context.config.approvals_reviewer,
            sandbox_network_access: turn_context.network_sandbox_policy.is_enabled(),
            collaboration_mode: turn_context.collaboration_mode.mode,
            personality: turn_context.personality,
            is_first_turn,
        });
}

async fn run_pre_sampling_compact(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
) -> CodexResult<bool> {
    let total_usage_tokens_before_compaction = sess.get_total_token_usage().await;
    let mut pre_sampling_compacted = maybe_run_previous_model_inline_compact(
        sess,
        turn_context,
        total_usage_tokens_before_compaction,
    )
    .await?;
    let total_usage_tokens = sess.get_total_token_usage().await;
    let auto_compact_limit = turn_context
        .model_info
        .auto_compact_token_limit()
        .unwrap_or(i64::MAX);
    // Compact if the total usage tokens are greater than the auto compact limit
    if total_usage_tokens >= auto_compact_limit {
        run_auto_compact(
            sess,
            turn_context,
            InitialContextInjection::DoNotInject,
            CompactionReason::ContextLimit,
            CompactionPhase::PreTurn,
        )
        .await?;
        pre_sampling_compacted = true;
    }
    Ok(pre_sampling_compacted)
}

/// Runs pre-sampling compaction against the previous model when switching to a smaller
/// context-window model.
///
/// Returns `Ok(true)` when compaction ran successfully, `Ok(false)` when compaction was skipped
/// because the model/context-window preconditions were not met, and `Err(_)` only when compaction
/// was attempted and failed.
async fn maybe_run_previous_model_inline_compact(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    total_usage_tokens: i64,
) -> CodexResult<bool> {
    let Some(previous_turn_settings) = sess.previous_turn_settings().await else {
        return Ok(false);
    };
    let previous_model_turn_context = Arc::new(
        turn_context
            .with_model(previous_turn_settings.model, &sess.services.models_manager)
            .await,
    );

    let Some(old_context_window) = previous_model_turn_context.model_context_window() else {
        return Ok(false);
    };
    let Some(new_context_window) = turn_context.model_context_window() else {
        return Ok(false);
    };
    let new_auto_compact_limit = turn_context
        .model_info
        .auto_compact_token_limit()
        .unwrap_or(i64::MAX);
    let should_run = total_usage_tokens > new_auto_compact_limit
        && previous_model_turn_context.model_info.slug != turn_context.model_info.slug
        && old_context_window > new_context_window;
    if should_run {
        run_auto_compact(
            sess,
            &previous_model_turn_context,
            InitialContextInjection::DoNotInject,
            CompactionReason::ModelDownshift,
            CompactionPhase::PreTurn,
        )
        .await?;
        return Ok(true);
    }
    Ok(false)
}

async fn run_auto_compact(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    initial_context_injection: InitialContextInjection,
    reason: CompactionReason,
    phase: CompactionPhase,
) -> CodexResult<()> {
    if should_use_remote_compact_task(&turn_context.provider) {
        run_inline_remote_auto_compact_task(
            Arc::clone(sess),
            Arc::clone(turn_context),
            initial_context_injection,
            reason,
            phase,
        )
        .await?;
    } else {
        run_inline_auto_compact_task(
            Arc::clone(sess),
            Arc::clone(turn_context),
            initial_context_injection,
            reason,
            phase,
        )
        .await?;
    }
    Ok(())
}

fn collect_explicit_app_ids_from_skill_items(
    skill_items: &[ResponseItem],
    connectors: &[connectors::AppInfo],
    skill_name_counts_lower: &HashMap<String, usize>,
) -> HashSet<String> {
    if skill_items.is_empty() || connectors.is_empty() {
        return HashSet::new();
    }

    let skill_messages = skill_items
        .iter()
        .filter_map(|item| match item {
            ResponseItem::Message { content, .. } => {
                content.iter().find_map(|content_item| match content_item {
                    ContentItem::InputText { text } => Some(text.clone()),
                    _ => None,
                })
            }
            _ => None,
        })
        .collect::<Vec<String>>();
    if skill_messages.is_empty() {
        return HashSet::new();
    }

    let mentions = collect_tool_mentions_from_messages(&skill_messages);
    let mention_names_lower = mentions
        .plain_names
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect::<HashSet<String>>();
    let mut connector_ids = mentions
        .paths
        .iter()
        .filter(|path| tool_kind_for_path(path) == ToolMentionKind::App)
        .filter_map(|path| app_id_from_path(path).map(str::to_string))
        .collect::<HashSet<String>>();

    let connector_slug_counts = build_connector_slug_counts(connectors);
    for connector in connectors {
        let slug = connectors::connector_mention_slug(connector);
        let connector_count = connector_slug_counts.get(&slug).copied().unwrap_or(0);
        let skill_count = skill_name_counts_lower.get(&slug).copied().unwrap_or(0);
        if connector_count == 1 && skill_count == 0 && mention_names_lower.contains(&slug) {
            connector_ids.insert(connector.id.clone());
        }
    }

    connector_ids
}

fn filter_connectors_for_input(
    connectors: &[connectors::AppInfo],
    input: &[ResponseItem],
    explicitly_enabled_connectors: &HashSet<String>,
    skill_name_counts_lower: &HashMap<String, usize>,
) -> Vec<connectors::AppInfo> {
    let connectors: Vec<connectors::AppInfo> = connectors
        .iter()
        .filter(|connector| connector.is_enabled)
        .cloned()
        .collect::<Vec<_>>();
    if connectors.is_empty() {
        return Vec::new();
    }

    let user_messages = collect_user_messages(input);
    if user_messages.is_empty() && explicitly_enabled_connectors.is_empty() {
        return Vec::new();
    }

    let mentions = collect_tool_mentions_from_messages(&user_messages);
    let mention_names_lower = mentions
        .plain_names
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect::<HashSet<String>>();

    let connector_slug_counts = build_connector_slug_counts(&connectors);
    let mut allowed_connector_ids = explicitly_enabled_connectors.clone();
    for path in mentions
        .paths
        .iter()
        .filter(|path| tool_kind_for_path(path) == ToolMentionKind::App)
    {
        if let Some(connector_id) = app_id_from_path(path) {
            allowed_connector_ids.insert(connector_id.to_string());
        }
    }

    connectors
        .into_iter()
        .filter(|connector| {
            connector_inserted_in_messages(
                connector,
                &mention_names_lower,
                &allowed_connector_ids,
                &connector_slug_counts,
                skill_name_counts_lower,
            )
        })
        .collect()
}

fn connector_inserted_in_messages(
    connector: &connectors::AppInfo,
    mention_names_lower: &HashSet<String>,
    allowed_connector_ids: &HashSet<String>,
    connector_slug_counts: &HashMap<String, usize>,
    skill_name_counts_lower: &HashMap<String, usize>,
) -> bool {
    if allowed_connector_ids.contains(&connector.id) {
        return true;
    }

    let mention_slug = connectors::connector_mention_slug(connector);
    let connector_count = connector_slug_counts
        .get(&mention_slug)
        .copied()
        .unwrap_or(0);
    let skill_count = skill_name_counts_lower
        .get(&mention_slug)
        .copied()
        .unwrap_or(0);
    connector_count == 1 && skill_count == 0 && mention_names_lower.contains(&mention_slug)
}

pub(crate) fn build_prompt(
    input: Vec<ResponseItem>,
    router: &ToolRouter,
    turn_context: &TurnContext,
    base_instructions: BaseInstructions,
) -> Prompt {
    let deferred_dynamic_tools = turn_context
        .dynamic_tools
        .iter()
        .filter(|tool| tool.defer_loading)
        .map(|tool| tool.name.as_str())
        .collect::<HashSet<_>>();
    let tools = if deferred_dynamic_tools.is_empty() {
        router.model_visible_specs()
    } else {
        router
            .model_visible_specs()
            .into_iter()
            .filter(|spec| !deferred_dynamic_tools.contains(spec.name()))
            .collect()
    };

    Prompt {
        input,
        tools,
        parallel_tool_calls: turn_context.model_info.supports_parallel_tool_calls,
        base_instructions,
        personality: turn_context.personality,
        output_schema: turn_context.final_output_json_schema.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
#[instrument(level = "trace",
    skip_all,
    fields(
        turn_id = %turn_context.sub_id,
        model = %turn_context.model_info.slug,
        cwd = %turn_context.cwd.display()
    )
)]
async fn run_sampling_request(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    turn_diff_tracker: SharedTurnDiffTracker,
    client_session: &mut ModelClientSession,
    turn_metadata_header: Option<&str>,
    input: Vec<ResponseItem>,
    explicitly_enabled_connectors: &HashSet<String>,
    skills_outcome: Option<&SkillLoadOutcome>,
    server_model_warning_emitted_for_turn: &mut bool,
    cancellation_token: CancellationToken,
) -> CodexResult<SamplingRequestResult> {
    let router = built_tools(
        sess.as_ref(),
        turn_context.as_ref(),
        &input,
        explicitly_enabled_connectors,
        skills_outcome,
        &cancellation_token,
    )
    .await?;

    let base_instructions = sess.get_base_instructions().await;

    let tool_runtime = ToolCallRuntime::new(
        Arc::clone(&router),
        Arc::clone(&sess),
        Arc::clone(&turn_context),
        Arc::clone(&turn_diff_tracker),
    );
    let _code_mode_worker = sess
        .services
        .code_mode_service
        .start_turn_worker(
            &sess,
            &turn_context,
            Arc::clone(&router),
            Arc::clone(&turn_diff_tracker),
        )
        .await;
    let mut retries = 0;
    let mut initial_input = Some(input);
    loop {
        let prompt_input = if let Some(input) = initial_input.take() {
            input
        } else {
            sess.clone_history()
                .await
                .for_prompt(&turn_context.model_info.input_modalities)
        };
        let prompt = build_prompt(
            prompt_input,
            router.as_ref(),
            turn_context.as_ref(),
            base_instructions.clone(),
        );
        let err = match try_run_sampling_request(
            tool_runtime.clone(),
            Arc::clone(&sess),
            Arc::clone(&turn_context),
            client_session,
            turn_metadata_header,
            Arc::clone(&turn_diff_tracker),
            server_model_warning_emitted_for_turn,
            &prompt,
            cancellation_token.child_token(),
        )
        .await
        {
            Ok(output) => {
                return Ok(output);
            }
            Err(CodexErr::ContextWindowExceeded) => {
                sess.set_total_tokens_full(&turn_context).await;
                return Err(CodexErr::ContextWindowExceeded);
            }
            Err(CodexErr::UsageLimitReached(e)) => {
                let rate_limits = e.rate_limits.clone();
                if let Some(rate_limits) = rate_limits {
                    sess.update_rate_limits(&turn_context, *rate_limits).await;
                }
                return Err(CodexErr::UsageLimitReached(e));
            }
            Err(err) => err,
        };

        if !err.is_retryable() {
            return Err(err);
        }

        // Use the configured provider-specific stream retry budget.
        let max_retries = turn_context.provider.stream_max_retries();
        if retries >= max_retries
            && client_session.try_switch_fallback_transport(
                &turn_context.session_telemetry,
                &turn_context.model_info,
            )
        {
            sess.send_event(
                &turn_context,
                EventMsg::Warning(WarningEvent {
                    message: format!("Falling back from WebSockets to HTTPS transport. {err:#}"),
                }),
            )
            .await;
            retries = 0;
            continue;
        }
        if retries < max_retries {
            retries += 1;
            let delay = match &err {
                CodexErr::Stream(_, requested_delay) => {
                    requested_delay.unwrap_or_else(|| backoff(retries))
                }
                _ => backoff(retries),
            };
            warn!(
                "stream disconnected - retrying sampling request ({retries}/{max_retries} in {delay:?})...",
            );

            // In release builds, hide the first websocket retry notification to reduce noisy
            // transient reconnect messages. In debug builds, keep full visibility for diagnosis.
            let report_error = retries > 1
                || cfg!(debug_assertions)
                || !sess.services.model_client.responses_websocket_enabled();
            if report_error {
                // Surface retry information to any UI/front‑end so the
                // user understands what is happening instead of staring
                // at a seemingly frozen screen.
                sess.notify_stream_error(
                    &turn_context,
                    format!("Reconnecting... {retries}/{max_retries}"),
                    err,
                )
                .await;
            }
            tokio::time::sleep(delay).await;
        } else {
            return Err(err);
        }
    }
}

pub(crate) async fn built_tools(
    sess: &Session,
    turn_context: &TurnContext,
    input: &[ResponseItem],
    explicitly_enabled_connectors: &HashSet<String>,
    skills_outcome: Option<&SkillLoadOutcome>,
    cancellation_token: &CancellationToken,
) -> CodexResult<Arc<ToolRouter>> {
    let mcp_connection_manager = sess.services.mcp_connection_manager.read().await;
    let has_mcp_servers = mcp_connection_manager.has_servers();
    let all_mcp_tools = mcp_connection_manager
        .list_all_tools()
        .or_cancel(cancellation_token)
        .await?;
    drop(mcp_connection_manager);
    let loaded_plugins = sess
        .services
        .plugins_manager
        .plugins_for_config(&turn_context.config)
        .await;

    let mut effective_explicitly_enabled_connectors = explicitly_enabled_connectors.clone();
    effective_explicitly_enabled_connectors.extend(sess.get_connector_selection().await);

    let apps_enabled = turn_context.apps_enabled();
    let accessible_connectors =
        apps_enabled.then(|| connectors::accessible_connectors_from_mcp_tools(&all_mcp_tools));
    let accessible_connectors_with_enabled_state =
        accessible_connectors.as_ref().map(|connectors| {
            connectors::with_app_enabled_state(connectors.clone(), &turn_context.config)
        });
    let connectors = if apps_enabled {
        let connectors = connectors::merge_plugin_apps_with_accessible(
            loaded_plugins.effective_apps(),
            accessible_connectors.clone().unwrap_or_default(),
        );
        Some(connectors::with_app_enabled_state(
            connectors,
            &turn_context.config,
        ))
    } else {
        None
    };
    let auth = sess.services.auth_manager.auth().await;
    let discoverable_tools = if apps_enabled && turn_context.tools_config.tool_suggest {
        if let Some(accessible_connectors) = accessible_connectors_with_enabled_state.as_ref() {
            match connectors::list_tool_suggest_discoverable_tools_with_auth(
                &turn_context.config,
                auth.as_ref(),
                accessible_connectors.as_slice(),
            )
            .await
            .map(|discoverable_tools| {
                filter_tool_suggest_discoverable_tools_for_client(
                    discoverable_tools,
                    turn_context.app_server_client_name.as_deref(),
                )
            }) {
                Ok(discoverable_tools) if discoverable_tools.is_empty() => None,
                Ok(discoverable_tools) => Some(discoverable_tools),
                Err(err) => {
                    warn!("failed to load discoverable tool suggestions: {err:#}");
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    let explicitly_enabled = if let Some(connectors) = connectors.as_ref() {
        let skill_name_counts_lower = skills_outcome.map_or_else(HashMap::new, |outcome| {
            build_skill_name_counts(&outcome.skills, &outcome.disabled_paths).1
        });

        filter_connectors_for_input(
            connectors,
            input,
            &effective_explicitly_enabled_connectors,
            &skill_name_counts_lower,
        )
    } else {
        Vec::new()
    };
    let mcp_tool_exposure = build_mcp_tool_exposure(
        &all_mcp_tools,
        connectors.as_deref(),
        explicitly_enabled.as_slice(),
        &turn_context.config,
        &turn_context.tools_config,
    );
    let mcp_tools = has_mcp_servers.then_some(mcp_tool_exposure.direct_tools);
    let deferred_mcp_tools = mcp_tool_exposure.deferred_tools;
    let unavailable_called_tools = {
        let exposed_tool_names = mcp_tools
            .iter()
            .chain(deferred_mcp_tools.iter())
            .flat_map(|tools| tools.keys().map(String::as_str))
            .collect::<HashSet<_>>();
        collect_unavailable_called_tools(input, &exposed_tool_names)
    };

    let parallel_mcp_server_names = turn_context
        .config
        .mcp_servers
        .get()
        .iter()
        .filter_map(|(server_name, server_config)| {
            server_config
                .supports_parallel_tool_calls
                .then_some(server_name.clone())
        })
        .collect::<HashSet<_>>();

    Ok(Arc::new(ToolRouter::from_config(
        &turn_context.tools_config,
        ToolRouterParams {
            mcp_tools,
            deferred_mcp_tools,
            unavailable_called_tools,
            parallel_mcp_server_names,
            discoverable_tools,
            dynamic_tools: turn_context.dynamic_tools.as_slice(),
        },
    )))
}

#[derive(Debug)]
struct SamplingRequestResult {
    needs_follow_up: bool,
    last_agent_message: Option<String>,
}

/// Ephemeral per-response state for streaming a single proposed plan.
/// This is intentionally not persisted or stored in session/state since it
/// only exists while a response is actively streaming. The final plan text
/// is extracted from the completed assistant message.
/// Tracks a single proposed plan item across a streaming response.
struct ProposedPlanItemState {
    item_id: String,
    started: bool,
    completed: bool,
}

/// Aggregated state used only while streaming a plan-mode response.
/// Includes per-item parsers, deferred agent message bookkeeping, and the plan item lifecycle.
struct PlanModeStreamState {
    /// Agent message items started by the model but deferred until we see non-plan text.
    pending_agent_message_items: HashMap<String, TurnItem>,
    /// Agent message items whose start notification has been emitted.
    started_agent_message_items: HashSet<String>,
    /// Leading whitespace buffered until we see non-whitespace text for an item.
    leading_whitespace_by_item: HashMap<String, String>,
    /// Tracks plan item lifecycle while streaming plan output.
    plan_item_state: ProposedPlanItemState,
}

impl PlanModeStreamState {
    fn new(turn_id: &str) -> Self {
        Self {
            pending_agent_message_items: HashMap::new(),
            started_agent_message_items: HashSet::new(),
            leading_whitespace_by_item: HashMap::new(),
            plan_item_state: ProposedPlanItemState::new(turn_id),
        }
    }
}

#[derive(Debug, Default)]
struct AssistantMessageStreamParsers {
    plan_mode: bool,
    parsers_by_item: HashMap<String, AssistantTextStreamParser>,
}

type ParsedAssistantTextDelta = AssistantTextChunk;

impl AssistantMessageStreamParsers {
    fn new(plan_mode: bool) -> Self {
        Self {
            plan_mode,
            parsers_by_item: HashMap::new(),
        }
    }

    fn parser_mut(&mut self, item_id: &str) -> &mut AssistantTextStreamParser {
        let plan_mode = self.plan_mode;
        self.parsers_by_item
            .entry(item_id.to_string())
            .or_insert_with(|| AssistantTextStreamParser::new(plan_mode))
    }

    fn seed_item_text(&mut self, item_id: &str, text: &str) -> ParsedAssistantTextDelta {
        if text.is_empty() {
            return ParsedAssistantTextDelta::default();
        }
        self.parser_mut(item_id).push_str(text)
    }

    fn parse_delta(&mut self, item_id: &str, delta: &str) -> ParsedAssistantTextDelta {
        self.parser_mut(item_id).push_str(delta)
    }

    fn finish_item(&mut self, item_id: &str) -> ParsedAssistantTextDelta {
        let Some(mut parser) = self.parsers_by_item.remove(item_id) else {
            return ParsedAssistantTextDelta::default();
        };
        parser.finish()
    }

    fn drain_finished(&mut self) -> Vec<(String, ParsedAssistantTextDelta)> {
        let parsers_by_item = std::mem::take(&mut self.parsers_by_item);
        parsers_by_item
            .into_iter()
            .map(|(item_id, mut parser)| (item_id, parser.finish()))
            .collect()
    }
}

impl ProposedPlanItemState {
    fn new(turn_id: &str) -> Self {
        Self {
            item_id: format!("{turn_id}-plan"),
            started: false,
            completed: false,
        }
    }

    async fn start(&mut self, sess: &Session, turn_context: &TurnContext) {
        if self.started || self.completed {
            return;
        }
        self.started = true;
        let item = TurnItem::Plan(PlanItem {
            id: self.item_id.clone(),
            text: String::new(),
        });
        sess.emit_turn_item_started(turn_context, &item).await;
    }

    async fn push_delta(&mut self, sess: &Session, turn_context: &TurnContext, delta: &str) {
        if self.completed {
            return;
        }
        if delta.is_empty() {
            return;
        }
        let event = PlanDeltaEvent {
            thread_id: sess.conversation_id.to_string(),
            turn_id: turn_context.sub_id.clone(),
            item_id: self.item_id.clone(),
            delta: delta.to_string(),
        };
        sess.send_event(turn_context, EventMsg::PlanDelta(event))
            .await;
    }

    async fn complete_with_text(
        &mut self,
        sess: &Session,
        turn_context: &TurnContext,
        text: String,
    ) {
        if self.completed || !self.started {
            return;
        }
        self.completed = true;
        let item = TurnItem::Plan(PlanItem {
            id: self.item_id.clone(),
            text,
        });
        sess.emit_turn_item_completed(turn_context, item).await;
    }
}

/// In plan mode we defer agent message starts until the parser emits non-plan
/// text. The parser buffers each line until it can rule out a tag prefix, so
/// plan-only outputs never show up as empty assistant messages.
async fn maybe_emit_pending_agent_message_start(
    sess: &Session,
    turn_context: &TurnContext,
    state: &mut PlanModeStreamState,
    item_id: &str,
) {
    if state.started_agent_message_items.contains(item_id) {
        return;
    }
    if let Some(item) = state.pending_agent_message_items.remove(item_id) {
        sess.emit_turn_item_started(turn_context, &item).await;
        state
            .started_agent_message_items
            .insert(item_id.to_string());
    }
}

/// Agent messages are text-only today; concatenate all text entries.
fn agent_message_text(item: &codex_protocol::items::AgentMessageItem) -> String {
    item.content
        .iter()
        .map(|entry| match entry {
            codex_protocol::items::AgentMessageContent::Text { text } => text.as_str(),
        })
        .collect()
}

fn realtime_text_for_event(msg: &EventMsg) -> Option<String> {
    match msg {
        EventMsg::AgentMessage(event) => Some(event.message.clone()),
        EventMsg::ItemCompleted(event) => match &event.item {
            TurnItem::AgentMessage(item) => Some(agent_message_text(item)),
            _ => None,
        },
        EventMsg::Error(_)
        | EventMsg::Warning(_)
        | EventMsg::RealtimeConversationStarted(_)
        | EventMsg::RealtimeConversationSdp(_)
        | EventMsg::RealtimeConversationRealtime(_)
        | EventMsg::RealtimeConversationClosed(_)
        | EventMsg::ModelReroute(_)
        | EventMsg::ContextCompacted(_)
        | EventMsg::ThreadRolledBack(_)
        | EventMsg::TurnStarted(_)
        | EventMsg::TurnComplete(_)
        | EventMsg::TokenCount(_)
        | EventMsg::UserMessage(_)
        | EventMsg::AgentMessageDelta(_)
        | EventMsg::AgentReasoning(_)
        | EventMsg::AgentReasoningDelta(_)
        | EventMsg::AgentReasoningRawContent(_)
        | EventMsg::AgentReasoningRawContentDelta(_)
        | EventMsg::AgentReasoningSectionBreak(_)
        | EventMsg::SessionConfigured(_)
        | EventMsg::ThreadNameUpdated(_)
        | EventMsg::McpStartupUpdate(_)
        | EventMsg::McpStartupComplete(_)
        | EventMsg::McpToolCallBegin(_)
        | EventMsg::McpToolCallEnd(_)
        | EventMsg::WebSearchBegin(_)
        | EventMsg::WebSearchEnd(_)
        | EventMsg::ExecCommandBegin(_)
        | EventMsg::ExecCommandOutputDelta(_)
        | EventMsg::TerminalInteraction(_)
        | EventMsg::ExecCommandEnd(_)
        | EventMsg::PatchApplyBegin(_)
        | EventMsg::PatchApplyEnd(_)
        | EventMsg::ViewImageToolCall(_)
        | EventMsg::ImageGenerationBegin(_)
        | EventMsg::ImageGenerationEnd(_)
        | EventMsg::ExecApprovalRequest(_)
        | EventMsg::RequestPermissions(_)
        | EventMsg::RequestUserInput(_)
        | EventMsg::DynamicToolCallRequest(_)
        | EventMsg::DynamicToolCallResponse(_)
        | EventMsg::GuardianAssessment(_)
        | EventMsg::ElicitationRequest(_)
        | EventMsg::ApplyPatchApprovalRequest(_)
        | EventMsg::DeprecationNotice(_)
        | EventMsg::BackgroundEvent(_)
        | EventMsg::UndoStarted(_)
        | EventMsg::UndoCompleted(_)
        | EventMsg::StreamError(_)
        | EventMsg::TurnDiff(_)
        | EventMsg::GetHistoryEntryResponse(_)
        | EventMsg::McpListToolsResponse(_)
        | EventMsg::ListSkillsResponse(_)
        | EventMsg::RealtimeConversationListVoicesResponse(_)
        | EventMsg::SkillsUpdateAvailable
        | EventMsg::PlanUpdate(_)
        | EventMsg::TurnAborted(_)
        | EventMsg::ShutdownComplete
        | EventMsg::EnteredReviewMode(_)
        | EventMsg::ExitedReviewMode(_)
        | EventMsg::RawResponseItem(_)
        | EventMsg::ItemStarted(_)
        | EventMsg::HookStarted(_)
        | EventMsg::HookCompleted(_)
        | EventMsg::AgentMessageContentDelta(_)
        | EventMsg::PlanDelta(_)
        | EventMsg::ReasoningContentDelta(_)
        | EventMsg::ReasoningRawContentDelta(_)
        | EventMsg::CollabAgentSpawnBegin(_)
        | EventMsg::CollabAgentSpawnEnd(_)
        | EventMsg::CollabAgentInteractionBegin(_)
        | EventMsg::CollabAgentInteractionEnd(_)
        | EventMsg::CollabWaitingBegin(_)
        | EventMsg::CollabWaitingEnd(_)
        | EventMsg::CollabCloseBegin(_)
        | EventMsg::CollabCloseEnd(_)
        | EventMsg::CollabResumeBegin(_)
        | EventMsg::CollabResumeEnd(_) => None,
    }
}

/// Split the stream into normal assistant text vs. proposed plan content.
/// Normal text becomes AgentMessage deltas; plan content becomes PlanDelta +
/// TurnItem::Plan.
async fn handle_plan_segments(
    sess: &Session,
    turn_context: &TurnContext,
    state: &mut PlanModeStreamState,
    item_id: &str,
    segments: Vec<ProposedPlanSegment>,
) {
    for segment in segments {
        match segment {
            ProposedPlanSegment::Normal(delta) => {
                if delta.is_empty() {
                    continue;
                }
                let has_non_whitespace = delta.chars().any(|ch| !ch.is_whitespace());
                if !has_non_whitespace && !state.started_agent_message_items.contains(item_id) {
                    let entry = state
                        .leading_whitespace_by_item
                        .entry(item_id.to_string())
                        .or_default();
                    entry.push_str(&delta);
                    continue;
                }
                let delta = if !state.started_agent_message_items.contains(item_id) {
                    if let Some(prefix) = state.leading_whitespace_by_item.remove(item_id) {
                        format!("{prefix}{delta}")
                    } else {
                        delta
                    }
                } else {
                    delta
                };
                maybe_emit_pending_agent_message_start(sess, turn_context, state, item_id).await;

                let event = AgentMessageContentDeltaEvent {
                    thread_id: sess.conversation_id.to_string(),
                    turn_id: turn_context.sub_id.clone(),
                    item_id: item_id.to_string(),
                    delta,
                };
                sess.send_event(turn_context, EventMsg::AgentMessageContentDelta(event))
                    .await;
            }
            ProposedPlanSegment::ProposedPlanStart => {
                if !state.plan_item_state.completed {
                    state.plan_item_state.start(sess, turn_context).await;
                }
            }
            ProposedPlanSegment::ProposedPlanDelta(delta) => {
                if !state.plan_item_state.completed {
                    if !state.plan_item_state.started {
                        state.plan_item_state.start(sess, turn_context).await;
                    }
                    state
                        .plan_item_state
                        .push_delta(sess, turn_context, &delta)
                        .await;
                }
            }
            ProposedPlanSegment::ProposedPlanEnd => {}
        }
    }
}

async fn emit_streamed_assistant_text_delta(
    sess: &Session,
    turn_context: &TurnContext,
    plan_mode_state: Option<&mut PlanModeStreamState>,
    item_id: &str,
    parsed: ParsedAssistantTextDelta,
) {
    if parsed.is_empty() {
        return;
    }
    if !parsed.citations.is_empty() {
        // Citation extraction is intentionally local for now; we strip citations from display text
        // but do not yet surface them in protocol events.
        let _citations = parsed.citations;
    }
    if let Some(state) = plan_mode_state {
        if !parsed.plan_segments.is_empty() {
            handle_plan_segments(sess, turn_context, state, item_id, parsed.plan_segments).await;
        }
        return;
    }
    if parsed.visible_text.is_empty() {
        return;
    }
    let event = AgentMessageContentDeltaEvent {
        thread_id: sess.conversation_id.to_string(),
        turn_id: turn_context.sub_id.clone(),
        item_id: item_id.to_string(),
        delta: parsed.visible_text,
    };
    sess.send_event(turn_context, EventMsg::AgentMessageContentDelta(event))
        .await;
}

/// Flush buffered assistant text parser state when an assistant message item ends.
async fn flush_assistant_text_segments_for_item(
    sess: &Session,
    turn_context: &TurnContext,
    plan_mode_state: Option<&mut PlanModeStreamState>,
    parsers: &mut AssistantMessageStreamParsers,
    item_id: &str,
) {
    let parsed = parsers.finish_item(item_id);
    emit_streamed_assistant_text_delta(sess, turn_context, plan_mode_state, item_id, parsed).await;
}

/// Flush any remaining buffered assistant text parser state at response completion.
async fn flush_assistant_text_segments_all(
    sess: &Session,
    turn_context: &TurnContext,
    mut plan_mode_state: Option<&mut PlanModeStreamState>,
    parsers: &mut AssistantMessageStreamParsers,
) {
    for (item_id, parsed) in parsers.drain_finished() {
        emit_streamed_assistant_text_delta(
            sess,
            turn_context,
            plan_mode_state.as_deref_mut(),
            &item_id,
            parsed,
        )
        .await;
    }
}

/// Emit completion for plan items by parsing the finalized assistant message.
async fn maybe_complete_plan_item_from_message(
    sess: &Session,
    turn_context: &TurnContext,
    state: &mut PlanModeStreamState,
    item: &ResponseItem,
) {
    if let ResponseItem::Message { role, content, .. } = item
        && role == "assistant"
    {
        let mut text = String::new();
        for entry in content {
            if let ContentItem::OutputText { text: chunk } = entry {
                text.push_str(chunk);
            }
        }
        if let Some(plan_text) = extract_proposed_plan_text(&text) {
            let (plan_text, _citations) = strip_citations(&plan_text);
            if !state.plan_item_state.started {
                state.plan_item_state.start(sess, turn_context).await;
            }
            state
                .plan_item_state
                .complete_with_text(sess, turn_context, plan_text)
                .await;
        }
    }
}

/// Emit a completed agent message in plan mode, respecting deferred starts.
async fn emit_agent_message_in_plan_mode(
    sess: &Session,
    turn_context: &TurnContext,
    agent_message: codex_protocol::items::AgentMessageItem,
    state: &mut PlanModeStreamState,
) {
    let agent_message_id = agent_message.id.clone();
    let text = agent_message_text(&agent_message);
    if text.trim().is_empty() {
        state.pending_agent_message_items.remove(&agent_message_id);
        state.started_agent_message_items.remove(&agent_message_id);
        return;
    }

    maybe_emit_pending_agent_message_start(sess, turn_context, state, &agent_message_id).await;

    if !state
        .started_agent_message_items
        .contains(&agent_message_id)
    {
        let start_item = state
            .pending_agent_message_items
            .remove(&agent_message_id)
            .unwrap_or_else(|| {
                TurnItem::AgentMessage(codex_protocol::items::AgentMessageItem {
                    id: agent_message_id.clone(),
                    content: Vec::new(),
                    phase: None,
                    memory_citation: None,
                })
            });
        sess.emit_turn_item_started(turn_context, &start_item).await;
        state
            .started_agent_message_items
            .insert(agent_message_id.clone());
    }

    sess.emit_turn_item_completed(turn_context, TurnItem::AgentMessage(agent_message))
        .await;
    state.started_agent_message_items.remove(&agent_message_id);
}

/// Emit completion for a plan-mode turn item, handling agent messages specially.
async fn emit_turn_item_in_plan_mode(
    sess: &Session,
    turn_context: &TurnContext,
    turn_item: TurnItem,
    previously_active_item: Option<&TurnItem>,
    state: &mut PlanModeStreamState,
) {
    match turn_item {
        TurnItem::AgentMessage(agent_message) => {
            emit_agent_message_in_plan_mode(sess, turn_context, agent_message, state).await;
        }
        _ => {
            if previously_active_item.is_none() {
                sess.emit_turn_item_started(turn_context, &turn_item).await;
            }
            sess.emit_turn_item_completed(turn_context, turn_item).await;
        }
    }
}

/// Handle a completed assistant response item in plan mode, returning true if handled.
async fn handle_assistant_item_done_in_plan_mode(
    sess: &Session,
    turn_context: &TurnContext,
    item: &ResponseItem,
    state: &mut PlanModeStreamState,
    previously_active_item: Option<&TurnItem>,
    last_agent_message: &mut Option<String>,
) -> bool {
    if let ResponseItem::Message { role, .. } = item
        && role == "assistant"
    {
        maybe_complete_plan_item_from_message(sess, turn_context, state, item).await;

        if let Some(turn_item) =
            handle_non_tool_response_item(sess, turn_context, item, /*plan_mode*/ true).await
        {
            emit_turn_item_in_plan_mode(
                sess,
                turn_context,
                turn_item,
                previously_active_item,
                state,
            )
            .await;
        }

        record_completed_response_item(sess, turn_context, item).await;
        if let Some(agent_message) = last_assistant_message_from_item(item, /*plan_mode*/ true) {
            *last_agent_message = Some(agent_message);
        }
        return true;
    }
    false
}

async fn drain_in_flight(
    in_flight: &mut FuturesOrdered<BoxFuture<'static, CodexResult<ResponseInputItem>>>,
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
) -> CodexResult<()> {
    while let Some(res) = in_flight.next().await {
        match res {
            Ok(response_input) => {
                sess.record_conversation_items(&turn_context, &[response_input.into()])
                    .await;
            }
            Err(err) => {
                error_or_panic(format!("in-flight tool future failed during drain: {err}"));
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[instrument(level = "trace",
    skip_all,
    fields(
        turn_id = %turn_context.sub_id,
        model = %turn_context.model_info.slug
    )
)]
async fn try_run_sampling_request(
    tool_runtime: ToolCallRuntime,
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    client_session: &mut ModelClientSession,
    turn_metadata_header: Option<&str>,
    turn_diff_tracker: SharedTurnDiffTracker,
    server_model_warning_emitted_for_turn: &mut bool,
    prompt: &Prompt,
    cancellation_token: CancellationToken,
) -> CodexResult<SamplingRequestResult> {
    feedback_tags!(
        model = turn_context.model_info.slug.clone(),
        approval_policy = turn_context.approval_policy.value(),
        sandbox_policy = turn_context.sandbox_policy.get(),
        effort = turn_context.reasoning_effort,
        auth_mode = sess.services.auth_manager.auth_mode(),
        features = sess.features.enabled_features(),
    );
    let mut stream = client_session
        .stream(
            prompt,
            &turn_context.model_info,
            &turn_context.session_telemetry,
            turn_context.reasoning_effort,
            turn_context.reasoning_summary,
            turn_context.config.service_tier,
            turn_metadata_header,
        )
        .instrument(trace_span!("stream_request"))
        .or_cancel(&cancellation_token)
        .await??;
    let mut in_flight: FuturesOrdered<BoxFuture<'static, CodexResult<ResponseInputItem>>> =
        FuturesOrdered::new();
    let mut needs_follow_up = false;
    let mut last_agent_message: Option<String> = None;
    let mut active_item: Option<TurnItem> = None;
    let mut should_emit_turn_diff = false;
    let plan_mode = turn_context.collaboration_mode.mode == ModeKind::Plan;
    let mut assistant_message_stream_parsers = AssistantMessageStreamParsers::new(plan_mode);
    let mut plan_mode_state = plan_mode.then(|| PlanModeStreamState::new(&turn_context.sub_id));
    let receiving_span = trace_span!("receiving_stream");
    let outcome: CodexResult<SamplingRequestResult> = loop {
        let handle_responses = trace_span!(
            parent: &receiving_span,
            "handle_responses",
            otel.name = field::Empty,
            tool_name = field::Empty,
            from = field::Empty,
        );

        let event = match stream
            .next()
            .instrument(trace_span!(parent: &handle_responses, "receiving"))
            .or_cancel(&cancellation_token)
            .await
        {
            Ok(event) => event,
            Err(codex_async_utils::CancelErr::Cancelled) => break Err(CodexErr::TurnAborted),
        };

        let event = match event {
            Some(Ok(event)) => event,
            Some(Err(err)) => break Err(err),
            None => {
                break Err(CodexErr::Stream(
                    "stream closed before response.completed".into(),
                    None,
                ));
            }
        };

        sess.services
            .session_telemetry
            .record_responses(&handle_responses, &event);
        record_turn_ttft_metric(&turn_context, &event).await;

        match event {
            ResponseEvent::Created => {}
            ResponseEvent::OutputItemDone(item) => {
                let previously_active_item = active_item.take();
                if let Some(previous) = previously_active_item.as_ref()
                    && matches!(previous, TurnItem::AgentMessage(_))
                {
                    let item_id = previous.id();
                    flush_assistant_text_segments_for_item(
                        &sess,
                        &turn_context,
                        plan_mode_state.as_mut(),
                        &mut assistant_message_stream_parsers,
                        &item_id,
                    )
                    .await;
                }
                if let Some(state) = plan_mode_state.as_mut()
                    && handle_assistant_item_done_in_plan_mode(
                        &sess,
                        &turn_context,
                        &item,
                        state,
                        previously_active_item.as_ref(),
                        &mut last_agent_message,
                    )
                    .await
                {
                    continue;
                }

                let mut ctx = HandleOutputCtx {
                    sess: sess.clone(),
                    turn_context: turn_context.clone(),
                    tool_runtime: tool_runtime.clone(),
                    cancellation_token: cancellation_token.child_token(),
                };

                let output_result =
                    match handle_output_item_done(&mut ctx, item, previously_active_item)
                        .instrument(handle_responses)
                        .await
                    {
                        Ok(output_result) => output_result,
                        Err(err) => break Err(err),
                    };
                if let Some(tool_future) = output_result.tool_future {
                    in_flight.push_back(tool_future);
                }
                if let Some(agent_message) = output_result.last_agent_message {
                    last_agent_message = Some(agent_message);
                }
                needs_follow_up |= output_result.needs_follow_up;
            }
            ResponseEvent::OutputItemAdded(item) => {
                if let Some(turn_item) = handle_non_tool_response_item(
                    sess.as_ref(),
                    turn_context.as_ref(),
                    &item,
                    plan_mode,
                )
                .await
                {
                    let mut turn_item = turn_item;
                    let mut seeded_parsed: Option<ParsedAssistantTextDelta> = None;
                    let mut seeded_item_id: Option<String> = None;
                    if matches!(turn_item, TurnItem::AgentMessage(_))
                        && let Some(raw_text) = raw_assistant_output_text_from_item(&item)
                    {
                        let item_id = turn_item.id();
                        let mut seeded =
                            assistant_message_stream_parsers.seed_item_text(&item_id, &raw_text);
                        if let TurnItem::AgentMessage(agent_message) = &mut turn_item {
                            agent_message.content =
                                vec![codex_protocol::items::AgentMessageContent::Text {
                                    text: if plan_mode {
                                        String::new()
                                    } else {
                                        std::mem::take(&mut seeded.visible_text)
                                    },
                                }];
                        }
                        seeded_parsed = plan_mode.then_some(seeded);
                        seeded_item_id = Some(item_id);
                    }
                    if let Some(state) = plan_mode_state.as_mut()
                        && matches!(turn_item, TurnItem::AgentMessage(_))
                    {
                        let item_id = turn_item.id();
                        state
                            .pending_agent_message_items
                            .insert(item_id, turn_item.clone());
                    } else {
                        sess.emit_turn_item_started(&turn_context, &turn_item).await;
                    }
                    if let (Some(state), Some(item_id), Some(parsed)) = (
                        plan_mode_state.as_mut(),
                        seeded_item_id.as_deref(),
                        seeded_parsed,
                    ) {
                        emit_streamed_assistant_text_delta(
                            &sess,
                            &turn_context,
                            Some(state),
                            item_id,
                            parsed,
                        )
                        .await;
                    }
                    active_item = Some(turn_item);
                }
            }
            ResponseEvent::ServerModel(server_model) => {
                if !*server_model_warning_emitted_for_turn
                    && sess
                        .maybe_warn_on_server_model_mismatch(&turn_context, server_model)
                        .await
                {
                    *server_model_warning_emitted_for_turn = true;
                }
            }
            ResponseEvent::ServerReasoningIncluded(included) => {
                sess.set_server_reasoning_included(included).await;
            }
            ResponseEvent::RateLimits(snapshot) => {
                // Update internal state with latest rate limits, but defer sending until
                // token usage is available to avoid duplicate TokenCount events.
                sess.update_rate_limits(&turn_context, snapshot).await;
            }
            ResponseEvent::ModelsEtag(etag) => {
                // Update internal state with latest models etag
                sess.services.models_manager.refresh_if_new_etag(etag).await;
            }
            ResponseEvent::Completed {
                response_id: _,
                token_usage,
            } => {
                flush_assistant_text_segments_all(
                    &sess,
                    &turn_context,
                    plan_mode_state.as_mut(),
                    &mut assistant_message_stream_parsers,
                )
                .await;
                sess.update_token_usage_info(&turn_context, token_usage.as_ref())
                    .await;
                should_emit_turn_diff = true;

                break Ok(SamplingRequestResult {
                    needs_follow_up,
                    last_agent_message,
                });
            }
            ResponseEvent::OutputTextDelta(delta) => {
                // In review child threads, suppress assistant text deltas; the
                // UI will show a selection popup from the final ReviewOutput.
                if let Some(active) = active_item.as_ref() {
                    let item_id = active.id();
                    if matches!(active, TurnItem::AgentMessage(_)) {
                        let parsed = assistant_message_stream_parsers.parse_delta(&item_id, &delta);
                        emit_streamed_assistant_text_delta(
                            &sess,
                            &turn_context,
                            plan_mode_state.as_mut(),
                            &item_id,
                            parsed,
                        )
                        .await;
                    } else {
                        let event = AgentMessageContentDeltaEvent {
                            thread_id: sess.conversation_id.to_string(),
                            turn_id: turn_context.sub_id.clone(),
                            item_id,
                            delta,
                        };
                        sess.send_event(&turn_context, EventMsg::AgentMessageContentDelta(event))
                            .await;
                    }
                } else {
                    error_or_panic("OutputTextDelta without active item".to_string());
                }
            }
            ResponseEvent::ReasoningSummaryDelta {
                delta,
                summary_index,
            } => {
                if let Some(active) = active_item.as_ref() {
                    let event = ReasoningContentDeltaEvent {
                        thread_id: sess.conversation_id.to_string(),
                        turn_id: turn_context.sub_id.clone(),
                        item_id: active.id(),
                        delta,
                        summary_index,
                    };
                    sess.send_event(&turn_context, EventMsg::ReasoningContentDelta(event))
                        .await;
                } else {
                    error_or_panic("ReasoningSummaryDelta without active item".to_string());
                }
            }
            ResponseEvent::ReasoningSummaryPartAdded { summary_index } => {
                if let Some(active) = active_item.as_ref() {
                    let event =
                        EventMsg::AgentReasoningSectionBreak(AgentReasoningSectionBreakEvent {
                            item_id: active.id(),
                            summary_index,
                        });
                    sess.send_event(&turn_context, event).await;
                } else {
                    error_or_panic("ReasoningSummaryPartAdded without active item".to_string());
                }
            }
            ResponseEvent::ReasoningContentDelta {
                delta,
                content_index,
            } => {
                if let Some(active) = active_item.as_ref() {
                    let event = ReasoningRawContentDeltaEvent {
                        thread_id: sess.conversation_id.to_string(),
                        turn_id: turn_context.sub_id.clone(),
                        item_id: active.id(),
                        delta,
                        content_index,
                    };
                    sess.send_event(&turn_context, EventMsg::ReasoningRawContentDelta(event))
                        .await;
                } else {
                    error_or_panic("ReasoningRawContentDelta without active item".to_string());
                }
            }
        }
    };

    flush_assistant_text_segments_all(
        &sess,
        &turn_context,
        plan_mode_state.as_mut(),
        &mut assistant_message_stream_parsers,
    )
    .await;

    drain_in_flight(&mut in_flight, sess.clone(), turn_context.clone()).await?;

    if cancellation_token.is_cancelled() {
        return Err(CodexErr::TurnAborted);
    }

    if should_emit_turn_diff {
        let unified_diff = {
            let mut tracker = turn_diff_tracker.lock().await;
            tracker.get_unified_diff()
        };
        if let Ok(Some(unified_diff)) = unified_diff {
            let msg = EventMsg::TurnDiff(TurnDiffEvent { unified_diff });
            sess.clone().send_event(&turn_context, msg).await;
        }
    }

    outcome
}

pub(super) fn get_last_assistant_message_from_turn(responses: &[ResponseItem]) -> Option<String> {
    for item in responses.iter().rev() {
        if let Some(message) = last_assistant_message_from_item(item, /*plan_mode*/ false) {
            return Some(message);
        }
    }
    None
}

use crate::memories::prompts::build_memory_tool_developer_instructions;
#[cfg(test)]
pub(crate) use tests::make_session_and_context;
#[cfg(test)]
pub(crate) use tests::make_session_and_context_with_dynamic_tools_and_rx;
#[cfg(test)]
pub(crate) use tests::make_session_and_context_with_rx;
#[cfg(test)]
pub(crate) use tests::make_session_configuration_for_tests;

#[cfg(test)]
#[path = "codex_tests.rs"]
mod tests;
