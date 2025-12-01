use std::collections::HashMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use crate::AuthManager;
use crate::SandboxState;
use crate::client_common::REVIEW_PROMPT;
use crate::compact;
use crate::compact::run_inline_auto_compact_task;
use crate::compact::should_use_remote_compact_task;
use crate::compact_remote::run_inline_remote_auto_compact_task;
use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::parse_command::parse_command;
use crate::parse_turn_item;
use crate::response_processing::process_items;
use crate::terminal;
use crate::truncate::TruncationPolicy;
use crate::user_notification::UserNotifier;
use crate::util::error_or_panic;
use async_channel::Receiver;
use async_channel::Sender;
use codex_protocol::ConversationId;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::HasLegacyEvent;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ItemStartedEvent;
use codex_protocol::protocol::RawResponseItemEvent;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TaskStartedEvent;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnContextItem;
use codex_rmcp_client::ElicitationResponse;
use futures::future::BoxFuture;
use futures::prelude::*;
use futures::stream::FuturesOrdered;
use mcp_types::CallToolResult;
use mcp_types::ListResourceTemplatesRequestParams;
use mcp_types::ListResourceTemplatesResult;
use mcp_types::ListResourcesRequestParams;
use mcp_types::ListResourcesResult;
use mcp_types::ReadResourceRequestParams;
use mcp_types::ReadResourceResult;
use mcp_types::RequestId;
use serde_json;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::ModelProviderInfo;
use crate::client::ModelClient;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::compact::collect_user_messages;
use crate::config::Config;
use crate::config::types::ShellEnvironmentPolicy;
use crate::context_manager::ContextManager;
use crate::environment_context::EnvironmentContext;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
#[cfg(test)]
use crate::exec::StreamOutput;
use crate::mcp::auth::compute_auth_statuses;
use crate::mcp_connection_manager::McpConnectionManager;
use crate::model_family::find_family_for_model;
use crate::openai_model_info::get_model_info;
use crate::project_doc::get_user_instructions;
use crate::protocol::AgentMessageContentDeltaEvent;
use crate::protocol::AgentReasoningSectionBreakEvent;
use crate::protocol::ApplyPatchApprovalRequestEvent;
use crate::protocol::AskForApproval;
use crate::protocol::BackgroundEventEvent;
use crate::protocol::DeprecationNoticeEvent;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::ExecApprovalRequestEvent;
use crate::protocol::Op;
use crate::protocol::RateLimitSnapshot;
use crate::protocol::ReasoningContentDeltaEvent;
use crate::protocol::ReasoningRawContentDeltaEvent;
use crate::protocol::ReviewDecision;
use crate::protocol::SandboxCommandAssessment;
use crate::protocol::SandboxPolicy;
use crate::protocol::SessionConfiguredEvent;
use crate::protocol::StreamErrorEvent;
use crate::protocol::Submission;
use crate::protocol::TokenCountEvent;
use crate::protocol::TokenUsage;
use crate::protocol::TokenUsageInfo;
use crate::protocol::TurnDiffEvent;
use crate::protocol::WarningEvent;
use crate::rollout::RolloutRecorder;
use crate::rollout::RolloutRecorderParams;
use crate::rollout::map_session_init_error;
use crate::shell;
use crate::state::ActiveTurn;
use crate::state::SessionServices;
use crate::state::SessionState;
use crate::tasks::GhostSnapshotTask;
use crate::tasks::ReviewTask;
use crate::tasks::SessionTask;
use crate::tasks::SessionTaskContext;
use crate::tools::ToolRouter;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::parallel::ToolCallRuntime;
use crate::tools::sandboxing::ApprovalStore;
use crate::tools::spec::ToolsConfig;
use crate::tools::spec::ToolsConfigParams;
use crate::turn_diff_tracker::TurnDiffTracker;
use crate::unified_exec::UnifiedExecSessionManager;
use crate::user_instructions::DeveloperInstructions;
use crate::user_instructions::UserInstructions;
use crate::user_notification::UserNotification;
use crate::util::backoff;
use codex_async_utils::OrCancelExt;
use codex_execpolicy::Policy as ExecPolicy;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::user_input::UserInput;
use codex_utils_readiness::Readiness;
use codex_utils_readiness::ReadinessFlag;

/// The high-level interface to the Codex system.
/// It operates as a queue pair where you send submissions and receive events.
pub struct Codex {
    pub(crate) next_id: AtomicU64,
    pub(crate) tx_sub: Sender<Submission>,
    pub(crate) rx_event: Receiver<Event>,
}

/// Wrapper returned by [`Codex::spawn`] containing the spawned [`Codex`],
/// the submission id for the initial `ConfigureSession` request and the
/// unique session id.
pub struct CodexSpawnOk {
    pub codex: Codex,
    pub conversation_id: ConversationId,
}

pub(crate) const INITIAL_SUBMIT_ID: &str = "";
pub(crate) const SUBMISSION_CHANNEL_CAPACITY: usize = 64;

impl Codex {
    /// Spawn a new [`Codex`] and initialize the session.
    pub async fn spawn(
        config: Config,
        auth_manager: Arc<AuthManager>,
        conversation_history: InitialHistory,
        session_source: SessionSource,
    ) -> CodexResult<CodexSpawnOk> {
        let (tx_sub, rx_sub) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);
        let (tx_event, rx_event) = async_channel::unbounded();

        let user_instructions = get_user_instructions(&config).await;

        let exec_policy = crate::exec_policy::exec_policy_for(&config.features, &config.codex_home)
            .await
            .map_err(|err| CodexErr::Fatal(format!("failed to load execpolicy: {err}")))?;

        let config = Arc::new(config);

        let session_configuration = SessionConfiguration {
            provider: config.model_provider.clone(),
            model: config.model.clone(),
            model_reasoning_effort: config.model_reasoning_effort,
            model_reasoning_summary: config.model_reasoning_summary,
            developer_instructions: config.developer_instructions.clone(),
            user_instructions,
            base_instructions: config.base_instructions.clone(),
            compact_prompt: config.compact_prompt.clone(),
            approval_policy: config.approval_policy,
            sandbox_policy: config.sandbox_policy.clone(),
            cwd: config.cwd.clone(),
            original_config_do_not_use: Arc::clone(&config),
            features: config.features.clone(),
            exec_policy,
            session_source,
        };

        // Generate a unique ID for the lifetime of this Codex session.
        let session_source_clone = session_configuration.session_source.clone();
        let session = Session::new(
            session_configuration,
            config.clone(),
            auth_manager.clone(),
            tx_event.clone(),
            conversation_history,
            session_source_clone,
        )
        .await
        .map_err(|e| {
            error!("Failed to create session: {e:#}");
            map_session_init_error(&e, &config.codex_home)
        })?;
        let conversation_id = session.conversation_id;

        // This task will run until Op::Shutdown is received.
        tokio::spawn(submission_loop(session, config, rx_sub));
        let codex = Codex {
            next_id: AtomicU64::new(0),
            tx_sub,
            rx_event,
        };

        Ok(CodexSpawnOk {
            codex,
            conversation_id,
        })
    }

    /// Submit the `op` wrapped in a `Submission` with a unique ID.
    pub async fn submit(&self, op: Op) -> CodexResult<String> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            .to_string();
        let sub = Submission { id: id.clone(), op };
        self.submit_with_id(sub).await?;
        Ok(id)
    }

    /// Use sparingly: prefer `submit()` so Codex is responsible for generating
    /// unique IDs for each submission.
    pub async fn submit_with_id(&self, sub: Submission) -> CodexResult<()> {
        self.tx_sub
            .send(sub)
            .await
            .map_err(|_| CodexErr::InternalAgentDied)?;
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
}

/// Context for an initialized model agent
///
/// A session has at most 1 running task at a time, and can be interrupted by user input.
pub(crate) struct Session {
    conversation_id: ConversationId,
    tx_event: Sender<Event>,
    state: Mutex<SessionState>,
    pub(crate) active_turn: Mutex<Option<ActiveTurn>>,
    pub(crate) services: SessionServices,
    next_internal_sub_id: AtomicU64,
}

/// The context needed for a single turn of the conversation.
#[derive(Debug)]
pub(crate) struct TurnContext {
    pub(crate) sub_id: String,
    pub(crate) client: ModelClient,
    /// The session's current working directory. All relative paths provided by
    /// the model as well as sandbox policies are resolved against this path
    /// instead of `std::env::current_dir()`.
    pub(crate) cwd: PathBuf,
    pub(crate) developer_instructions: Option<String>,
    pub(crate) base_instructions: Option<String>,
    pub(crate) compact_prompt: Option<String>,
    pub(crate) user_instructions: Option<String>,
    pub(crate) approval_policy: AskForApproval,
    pub(crate) sandbox_policy: SandboxPolicy,
    pub(crate) shell_environment_policy: ShellEnvironmentPolicy,
    pub(crate) tools_config: ToolsConfig,
    pub(crate) final_output_json_schema: Option<Value>,
    pub(crate) codex_linux_sandbox_exe: Option<PathBuf>,
    pub(crate) tool_call_gate: Arc<ReadinessFlag>,
    pub(crate) exec_policy: Arc<ExecPolicy>,
    pub(crate) truncation_policy: TruncationPolicy,
}

impl TurnContext {
    pub(crate) fn resolve_path(&self, path: Option<String>) -> PathBuf {
        path.as_ref()
            .map(PathBuf::from)
            .map_or_else(|| self.cwd.clone(), |p| self.cwd.join(p))
    }

    pub(crate) fn compact_prompt(&self) -> &str {
        self.compact_prompt
            .as_deref()
            .unwrap_or(compact::SUMMARIZATION_PROMPT)
    }
}

#[derive(Clone)]
pub(crate) struct SessionConfiguration {
    /// Provider identifier ("openai", "openrouter", ...).
    provider: ModelProviderInfo,

    /// If not specified, server will use its default model.
    model: String,

    model_reasoning_effort: Option<ReasoningEffortConfig>,
    model_reasoning_summary: ReasoningSummaryConfig,

    /// Developer instructions that supplement the base instructions.
    developer_instructions: Option<String>,

    /// Model instructions that are appended to the base instructions.
    user_instructions: Option<String>,

    /// Base instructions override.
    base_instructions: Option<String>,

    /// Compact prompt override.
    compact_prompt: Option<String>,

    /// When to escalate for approval for execution
    approval_policy: AskForApproval,
    /// How to sandbox commands executed in the system
    sandbox_policy: SandboxPolicy,

    /// Working directory that should be treated as the *root* of the
    /// session. All relative paths supplied by the model as well as the
    /// execution sandbox are resolved against this directory **instead**
    /// of the process-wide current working directory. CLI front-ends are
    /// expected to expand this to an absolute path before sending the
    /// `ConfigureSession` operation so that the business-logic layer can
    /// operate deterministically.
    cwd: PathBuf,

    /// Set of feature flags for this session
    features: Features,
    /// Execpolicy policy, applied only when enabled by feature flag.
    exec_policy: Arc<ExecPolicy>,

    // TODO(pakrym): Remove config from here
    original_config_do_not_use: Arc<Config>,
    /// Source of the session (cli, vscode, exec, mcp, ...)
    session_source: SessionSource,
}

impl SessionConfiguration {
    pub(crate) fn apply(&self, updates: &SessionSettingsUpdate) -> Self {
        let mut next_configuration = self.clone();
        if let Some(model) = updates.model.clone() {
            next_configuration.model = model;
        }
        if let Some(effort) = updates.reasoning_effort {
            next_configuration.model_reasoning_effort = effort;
        }
        if let Some(summary) = updates.reasoning_summary {
            next_configuration.model_reasoning_summary = summary;
        }
        if let Some(approval_policy) = updates.approval_policy {
            next_configuration.approval_policy = approval_policy;
        }
        if let Some(sandbox_policy) = updates.sandbox_policy.clone() {
            next_configuration.sandbox_policy = sandbox_policy;
        }
        if let Some(cwd) = updates.cwd.clone() {
            next_configuration.cwd = cwd;
        }
        next_configuration
    }
}

#[derive(Default, Clone)]
pub(crate) struct SessionSettingsUpdate {
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) approval_policy: Option<AskForApproval>,
    pub(crate) sandbox_policy: Option<SandboxPolicy>,
    pub(crate) model: Option<String>,
    pub(crate) reasoning_effort: Option<Option<ReasoningEffortConfig>>,
    pub(crate) reasoning_summary: Option<ReasoningSummaryConfig>,
    pub(crate) final_output_json_schema: Option<Option<Value>>,
}

impl Session {
    fn make_turn_context(
        auth_manager: Option<Arc<AuthManager>>,
        otel_event_manager: &OtelEventManager,
        provider: ModelProviderInfo,
        session_configuration: &SessionConfiguration,
        conversation_id: ConversationId,
        sub_id: String,
    ) -> TurnContext {
        let config = session_configuration.original_config_do_not_use.clone();
        let model_family = find_family_for_model(&session_configuration.model)
            .unwrap_or_else(|| config.model_family.clone());
        let mut per_turn_config = (*config).clone();
        per_turn_config.model = session_configuration.model.clone();
        per_turn_config.model_family = model_family.clone();
        per_turn_config.model_reasoning_effort = session_configuration.model_reasoning_effort;
        per_turn_config.model_reasoning_summary = session_configuration.model_reasoning_summary;
        if let Some(model_info) = get_model_info(&model_family) {
            per_turn_config.model_context_window = Some(model_info.context_window);
        }

        let otel_event_manager = otel_event_manager.clone().with_model(
            session_configuration.model.as_str(),
            session_configuration.model.as_str(),
        );

        let client = ModelClient::new(
            Arc::new(per_turn_config.clone()),
            auth_manager,
            otel_event_manager,
            provider,
            session_configuration.model_reasoning_effort,
            session_configuration.model_reasoning_summary,
            conversation_id,
            session_configuration.session_source.clone(),
        );

        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_family: &model_family,
            features: &config.features,
        });

        TurnContext {
            sub_id,
            client,
            cwd: session_configuration.cwd.clone(),
            developer_instructions: session_configuration.developer_instructions.clone(),
            base_instructions: session_configuration.base_instructions.clone(),
            compact_prompt: session_configuration.compact_prompt.clone(),
            user_instructions: session_configuration.user_instructions.clone(),
            approval_policy: session_configuration.approval_policy,
            sandbox_policy: session_configuration.sandbox_policy.clone(),
            shell_environment_policy: config.shell_environment_policy.clone(),
            tools_config,
            final_output_json_schema: None,
            codex_linux_sandbox_exe: config.codex_linux_sandbox_exe.clone(),
            tool_call_gate: Arc::new(ReadinessFlag::new()),
            exec_policy: session_configuration.exec_policy.clone(),
            truncation_policy: TruncationPolicy::new(&per_turn_config),
        }
    }

    async fn new(
        session_configuration: SessionConfiguration,
        config: Arc<Config>,
        auth_manager: Arc<AuthManager>,
        tx_event: Sender<Event>,
        initial_history: InitialHistory,
        session_source: SessionSource,
    ) -> anyhow::Result<Arc<Self>> {
        debug!(
            "Configuring session: model={}; provider={:?}",
            session_configuration.model, session_configuration.provider
        );
        if !session_configuration.cwd.is_absolute() {
            return Err(anyhow::anyhow!(
                "cwd is not absolute: {:?}",
                session_configuration.cwd
            ));
        }

        let (conversation_id, rollout_params) = match &initial_history {
            InitialHistory::New | InitialHistory::Forked(_) => {
                let conversation_id = ConversationId::default();
                (
                    conversation_id,
                    RolloutRecorderParams::new(
                        conversation_id,
                        session_configuration.user_instructions.clone(),
                        session_source,
                    ),
                )
            }
            InitialHistory::Resumed(resumed_history) => (
                resumed_history.conversation_id,
                RolloutRecorderParams::resume(resumed_history.rollout_path.clone()),
            ),
        };

        // Kick off independent async setup tasks in parallel to reduce startup latency.
        //
        // - initialize RolloutRecorder with new or resumed session info
        // - perform default shell discovery
        // - load history metadata
        let rollout_fut = RolloutRecorder::new(&config, rollout_params);

        let default_shell = shell::default_user_shell();
        let history_meta_fut = crate::message_history::history_metadata(&config);
        let auth_statuses_fut = compute_auth_statuses(
            config.mcp_servers.iter(),
            config.mcp_oauth_credentials_store_mode,
        );

        // Join all independent futures.
        let (rollout_recorder, (history_log_id, history_entry_count), auth_statuses) =
            tokio::join!(rollout_fut, history_meta_fut, auth_statuses_fut);

        let rollout_recorder = rollout_recorder.map_err(|e| {
            error!("failed to initialize rollout recorder: {e:#}");
            anyhow::Error::from(e)
        })?;
        let rollout_path = rollout_recorder.rollout_path.clone();

        let mut post_session_configured_events = Vec::<Event>::new();

        for (alias, feature) in session_configuration.features.legacy_feature_usages() {
            let canonical = feature.key();
            let summary = format!("`{alias}` is deprecated. Use `{canonical}` instead.");
            let details = if alias == canonical {
                None
            } else {
                Some(format!(
                    "Enable it with `--enable {canonical}` or `[features].{canonical}` in config.toml. See https://github.com/openai/codex/blob/main/docs/config.md#feature-flags for details."
                ))
            };
            post_session_configured_events.push(Event {
                id: INITIAL_SUBMIT_ID.to_owned(),
                msg: EventMsg::DeprecationNotice(DeprecationNoticeEvent { summary, details }),
            });
        }

        let otel_event_manager = OtelEventManager::new(
            conversation_id,
            config.model.as_str(),
            config.model_family.slug.as_str(),
            auth_manager.auth().and_then(|a| a.get_account_id()),
            auth_manager.auth().and_then(|a| a.get_account_email()),
            auth_manager.auth().map(|a| a.mode),
            config.otel.log_user_prompt,
            terminal::user_agent(),
        );

        otel_event_manager.conversation_starts(
            config.model_provider.name.as_str(),
            config.model_reasoning_effort,
            config.model_reasoning_summary,
            config.model_context_window,
            config.model_auto_compact_token_limit,
            config.approval_policy,
            config.sandbox_policy.clone(),
            config.mcp_servers.keys().map(String::as_str).collect(),
            config.active_profile.clone(),
        );

        // Create the mutable state for the Session.
        let state = SessionState::new(session_configuration.clone());

        let services = SessionServices {
            mcp_connection_manager: Arc::new(RwLock::new(McpConnectionManager::default())),
            mcp_startup_cancellation_token: CancellationToken::new(),
            unified_exec_manager: UnifiedExecSessionManager::default(),
            notifier: UserNotifier::new(config.notify.clone()),
            rollout: Mutex::new(Some(rollout_recorder)),
            user_shell: default_shell,
            show_raw_agent_reasoning: config.show_raw_agent_reasoning,
            auth_manager: Arc::clone(&auth_manager),
            otel_event_manager,
            tool_approvals: Mutex::new(ApprovalStore::default()),
        };

        let sess = Arc::new(Session {
            conversation_id,
            tx_event: tx_event.clone(),
            state: Mutex::new(state),
            active_turn: Mutex::new(None),
            services,
            next_internal_sub_id: AtomicU64::new(0),
        });

        // Dispatch the SessionConfiguredEvent first and then report any errors.
        // If resuming, include converted initial messages in the payload so UIs can render them immediately.
        let initial_messages = initial_history.get_event_msgs();

        let events = std::iter::once(Event {
            id: INITIAL_SUBMIT_ID.to_owned(),
            msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
                session_id: conversation_id,
                model: session_configuration.model.clone(),
                model_provider_id: config.model_provider_id.clone(),
                approval_policy: session_configuration.approval_policy,
                sandbox_policy: session_configuration.sandbox_policy.clone(),
                cwd: session_configuration.cwd.clone(),
                reasoning_effort: session_configuration.model_reasoning_effort,
                history_log_id,
                history_entry_count,
                initial_messages,
                rollout_path,
            }),
        })
        .chain(post_session_configured_events.into_iter());
        for event in events {
            sess.send_event_raw(event).await;
        }
        sess.services
            .mcp_connection_manager
            .write()
            .await
            .initialize(
                config.mcp_servers.clone(),
                config.mcp_oauth_credentials_store_mode,
                auth_statuses.clone(),
                tx_event.clone(),
                sess.services.mcp_startup_cancellation_token.clone(),
            )
            .await;

        let sandbox_state = SandboxState {
            sandbox_policy: session_configuration.sandbox_policy.clone(),
            codex_linux_sandbox_exe: config.codex_linux_sandbox_exe.clone(),
            sandbox_cwd: session_configuration.cwd.clone(),
        };
        if let Err(e) = sess
            .services
            .mcp_connection_manager
            .read()
            .await
            .notify_sandbox_state_change(&sandbox_state)
            .await
        {
            tracing::error!("Failed to notify sandbox state change: {e}");
        }

        // record_initial_history can emit events. We record only after the SessionConfiguredEvent is emitted.
        sess.record_initial_history(initial_history).await;

        Ok(sess)
    }

    pub(crate) fn get_tx_event(&self) -> Sender<Event> {
        self.tx_event.clone()
    }

    /// Ensure all rollout writes are durably flushed.
    pub(crate) async fn flush_rollout(&self) {
        let recorder = {
            let guard = self.services.rollout.lock().await;
            guard.clone()
        };
        if let Some(rec) = recorder
            && let Err(e) = rec.flush().await
        {
            warn!("failed to flush rollout recorder: {e}");
        }
    }

    fn next_internal_sub_id(&self) -> String {
        let id = self
            .next_internal_sub_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("auto-compact-{id}")
    }

    async fn get_total_token_usage(&self) -> i64 {
        let state = self.state.lock().await;
        state.get_total_token_usage()
    }

    async fn record_initial_history(&self, conversation_history: InitialHistory) {
        let turn_context = self.new_turn(SessionSettingsUpdate::default()).await;
        match conversation_history {
            InitialHistory::New => {
                // Build and record initial items (user instructions + environment context)
                let items = self.build_initial_context(&turn_context);
                self.record_conversation_items(&turn_context, &items).await;
                // Ensure initial items are visible to immediate readers (e.g., tests, forks).
                self.flush_rollout().await;
            }
            InitialHistory::Resumed(_) | InitialHistory::Forked(_) => {
                let rollout_items = conversation_history.get_rollout_items();
                let persist = matches!(conversation_history, InitialHistory::Forked(_));

                // If resuming, warn when the last recorded model differs from the current one.
                if let InitialHistory::Resumed(_) = conversation_history
                    && let Some(prev) = rollout_items.iter().rev().find_map(|it| {
                        if let RolloutItem::TurnContext(ctx) = it {
                            Some(ctx.model.as_str())
                        } else {
                            None
                        }
                    })
                {
                    let curr = turn_context.client.get_model();
                    if prev != curr {
                        warn!(
                            "resuming session with different model: previous={prev}, current={curr}"
                        );
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
                }

                // Always add response items to conversation history
                let reconstructed_history =
                    self.reconstruct_history_from_rollout(&turn_context, &rollout_items);
                if !reconstructed_history.is_empty() {
                    self.record_into_history(&reconstructed_history, &turn_context)
                        .await;
                }

                // If persisting, persist all rollout items as-is (recorder filters)
                if persist && !rollout_items.is_empty() {
                    self.persist_rollout_items(&rollout_items).await;
                }
                // Flush after seeding history and any persisted rollout copy.
                self.flush_rollout().await;
            }
        }
    }

    pub(crate) async fn update_settings(&self, updates: SessionSettingsUpdate) {
        let mut state = self.state.lock().await;

        state.session_configuration = state.session_configuration.apply(&updates);
    }

    pub(crate) async fn new_turn(&self, updates: SessionSettingsUpdate) -> Arc<TurnContext> {
        let sub_id = self.next_internal_sub_id();
        self.new_turn_with_sub_id(sub_id, updates).await
    }

    pub(crate) async fn new_turn_with_sub_id(
        &self,
        sub_id: String,
        updates: SessionSettingsUpdate,
    ) -> Arc<TurnContext> {
        let session_configuration = {
            let mut state = self.state.lock().await;
            let session_configuration = state.session_configuration.clone().apply(&updates);
            state.session_configuration = session_configuration.clone();
            session_configuration
        };

        let mut turn_context: TurnContext = Self::make_turn_context(
            Some(Arc::clone(&self.services.auth_manager)),
            &self.services.otel_event_manager,
            session_configuration.provider.clone(),
            &session_configuration,
            self.conversation_id,
            sub_id,
        );
        if let Some(final_schema) = updates.final_output_json_schema {
            turn_context.final_output_json_schema = final_schema;
        }
        Arc::new(turn_context)
    }

    fn build_environment_update_item(
        &self,
        previous: Option<&Arc<TurnContext>>,
        next: &TurnContext,
    ) -> Option<ResponseItem> {
        let prev = previous?;

        let prev_context = EnvironmentContext::from(prev.as_ref());
        let next_context = EnvironmentContext::from(next);
        if prev_context.equals_except_shell(&next_context) {
            return None;
        }
        Some(ResponseItem::from(EnvironmentContext::diff(
            prev.as_ref(),
            next,
        )))
    }

    /// Persist the event to rollout and send it to clients.
    pub(crate) async fn send_event(&self, turn_context: &TurnContext, msg: EventMsg) {
        let legacy_source = msg.clone();
        let event = Event {
            id: turn_context.sub_id.clone(),
            msg,
        };
        self.send_event_raw(event).await;

        let show_raw_agent_reasoning = self.show_raw_agent_reasoning();
        for legacy in legacy_source.as_legacy_events(show_raw_agent_reasoning) {
            let legacy_event = Event {
                id: turn_context.sub_id.clone(),
                msg: legacy,
            };
            self.send_event_raw(legacy_event).await;
        }
    }

    pub(crate) async fn send_event_raw(&self, event: Event) {
        // Persist the event into rollout (recorder filters as needed)
        let rollout_items = vec![RolloutItem::EventMsg(event.msg.clone())];
        self.persist_rollout_items(&rollout_items).await;
        if let Err(e) = self.tx_event.send(event).await {
            error!("failed to send tool call event: {e}");
        }
    }

    async fn emit_turn_item_started(&self, turn_context: &TurnContext, item: &TurnItem) {
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

    async fn emit_turn_item_completed(&self, turn_context: &TurnContext, item: TurnItem) {
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

    pub(crate) async fn assess_sandbox_command(
        &self,
        turn_context: &TurnContext,
        call_id: &str,
        command: &[String],
        failure_message: Option<&str>,
    ) -> Option<SandboxCommandAssessment> {
        let config = turn_context.client.config();
        let provider = turn_context.client.provider().clone();
        let auth_manager = Arc::clone(&self.services.auth_manager);
        let otel = self.services.otel_event_manager.clone();
        crate::sandboxing::assessment::assess_command(
            config,
            provider,
            auth_manager,
            &otel,
            self.conversation_id,
            turn_context.client.get_session_source(),
            call_id,
            command,
            &turn_context.sandbox_policy,
            &turn_context.cwd,
            failure_message,
        )
        .await
    }

    /// Emit an exec approval request event and await the user's decision.
    ///
    /// The request is keyed by `sub_id`/`call_id` so matching responses are delivered
    /// to the correct in-flight turn. If the task is aborted, this returns the
    /// default `ReviewDecision` (`Denied`).
    pub async fn request_command_approval(
        &self,
        turn_context: &TurnContext,
        call_id: String,
        command: Vec<String>,
        cwd: PathBuf,
        reason: Option<String>,
        risk: Option<SandboxCommandAssessment>,
    ) -> ReviewDecision {
        let sub_id = turn_context.sub_id.clone();
        // Add the tx_approve callback to the map before sending the request.
        let (tx_approve, rx_approve) = oneshot::channel();
        let event_id = sub_id.clone();
        let prev_entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    ts.insert_pending_approval(sub_id, tx_approve)
                }
                None => None,
            }
        };
        if prev_entry.is_some() {
            warn!("Overwriting existing pending approval for sub_id: {event_id}");
        }

        let parsed_cmd = parse_command(&command);
        let event = EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
            call_id,
            turn_id: turn_context.sub_id.clone(),
            command,
            cwd,
            reason,
            risk,
            parsed_cmd,
        });
        self.send_event(turn_context, event).await;
        rx_approve.await.unwrap_or_default()
    }

    pub async fn request_patch_approval(
        &self,
        turn_context: &TurnContext,
        call_id: String,
        changes: HashMap<PathBuf, FileChange>,
        reason: Option<String>,
        grant_root: Option<PathBuf>,
    ) -> oneshot::Receiver<ReviewDecision> {
        let sub_id = turn_context.sub_id.clone();
        // Add the tx_approve callback to the map before sending the request.
        let (tx_approve, rx_approve) = oneshot::channel();
        let event_id = sub_id.clone();
        let prev_entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    ts.insert_pending_approval(sub_id, tx_approve)
                }
                None => None,
            }
        };
        if prev_entry.is_some() {
            warn!("Overwriting existing pending approval for sub_id: {event_id}");
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

    pub async fn notify_approval(&self, sub_id: &str, decision: ReviewDecision) {
        let entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    ts.remove_pending_approval(sub_id)
                }
                None => None,
            }
        };
        match entry {
            Some(tx_approve) => {
                tx_approve.send(decision).ok();
            }
            None => {
                warn!("No pending approval found for sub_id: {sub_id}");
            }
        }
    }

    pub async fn resolve_elicitation(
        &self,
        server_name: String,
        id: RequestId,
        response: ElicitationResponse,
    ) -> anyhow::Result<()> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .resolve_elicitation(server_name, id, response)
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

    fn reconstruct_history_from_rollout(
        &self,
        turn_context: &TurnContext,
        rollout_items: &[RolloutItem],
    ) -> Vec<ResponseItem> {
        let mut history = ContextManager::new();
        for item in rollout_items {
            match item {
                RolloutItem::ResponseItem(response_item) => {
                    history.record_items(
                        std::iter::once(response_item),
                        turn_context.truncation_policy,
                    );
                }
                RolloutItem::Compacted(compacted) => {
                    let snapshot = history.get_history();
                    // TODO(jif) clean
                    if let Some(replacement) = &compacted.replacement_history {
                        history.replace(replacement.clone());
                    } else {
                        let user_messages = collect_user_messages(&snapshot);
                        let rebuilt = compact::build_compacted_history(
                            self.build_initial_context(turn_context),
                            &user_messages,
                            &compacted.message,
                        );
                        history.replace(rebuilt);
                    }
                }
                _ => {}
            }
        }
        history.get_history()
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

    pub(crate) async fn replace_history(&self, items: Vec<ResponseItem>) {
        let mut state = self.state.lock().await;
        state.replace_history(items);
    }

    async fn persist_rollout_response_items(&self, items: &[ResponseItem]) {
        let rollout_items: Vec<RolloutItem> = items
            .iter()
            .cloned()
            .map(RolloutItem::ResponseItem)
            .collect();
        self.persist_rollout_items(&rollout_items).await;
    }

    pub async fn enabled(&self, feature: Feature) -> bool {
        self.state
            .lock()
            .await
            .session_configuration
            .features
            .enabled(feature)
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

    pub(crate) fn build_initial_context(&self, turn_context: &TurnContext) -> Vec<ResponseItem> {
        let mut items = Vec::<ResponseItem>::with_capacity(3);
        if let Some(developer_instructions) = turn_context.developer_instructions.as_deref() {
            items.push(DeveloperInstructions::new(developer_instructions.to_string()).into());
        }
        if let Some(user_instructions) = turn_context.user_instructions.as_deref() {
            items.push(
                UserInstructions {
                    text: user_instructions.to_string(),
                    directory: turn_context.cwd.to_string_lossy().into_owned(),
                }
                .into(),
            );
        }
        items.push(ResponseItem::from(EnvironmentContext::new(
            Some(turn_context.cwd.clone()),
            Some(turn_context.approval_policy),
            Some(turn_context.sandbox_policy.clone()),
            self.user_shell().clone(),
        )));
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

    pub(crate) async fn update_token_usage_info(
        &self,
        turn_context: &TurnContext,
        token_usage: Option<&TokenUsage>,
    ) {
        {
            let mut state = self.state.lock().await;
            if let Some(token_usage) = token_usage {
                state.update_token_info_from_usage(
                    token_usage,
                    turn_context.client.get_model_context_window(),
                );
            }
        }
        self.send_token_count_event(turn_context).await;
    }

    pub(crate) async fn recompute_token_usage(&self, turn_context: &TurnContext) {
        let Some(estimated_total_tokens) = self
            .clone_history()
            .await
            .estimate_token_count(turn_context)
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

            if info.model_context_window.is_none() {
                info.model_context_window = turn_context.client.get_model_context_window();
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

    async fn send_token_count_event(&self, turn_context: &TurnContext) {
        let (info, rate_limits) = {
            let state = self.state.lock().await;
            state.token_info_and_rate_limits()
        };
        let event = EventMsg::TokenCount(TokenCountEvent { info, rate_limits });
        self.send_event(turn_context, event).await;
    }

    pub(crate) async fn set_total_tokens_full(&self, turn_context: &TurnContext) {
        let context_window = turn_context.client.get_model_context_window();
        if let Some(context_window) = context_window {
            {
                let mut state = self.state.lock().await;
                state.set_token_usage_full(context_window);
            }
            self.send_token_count_event(turn_context).await;
        }
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
        let codex_error_info = CodexErrorInfo::ResponseStreamDisconnected {
            http_status_code: codex_error.http_status_code_value(),
        };
        let event = EventMsg::StreamError(StreamErrorEvent {
            message: message.into(),
            codex_error_info: Some(codex_error_info),
        });
        self.send_event(turn_context, event).await;
    }

    async fn maybe_start_ghost_snapshot(
        self: &Arc<Self>,
        turn_context: Arc<TurnContext>,
        cancellation_token: CancellationToken,
    ) {
        if !self.enabled(Feature::GhostCommit).await {
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

    /// Returns the input if there was no task running to inject into
    pub async fn inject_input(&self, input: Vec<UserInput>) -> Result<(), Vec<UserInput>> {
        let mut active = self.active_turn.lock().await;
        match active.as_mut() {
            Some(at) => {
                let mut ts = at.turn_state.lock().await;
                ts.push_pending_input(input.into());
                Ok(())
            }
            None => Err(input),
        }
    }

    pub async fn get_pending_input(&self) -> Vec<ResponseInputItem> {
        let mut active = self.active_turn.lock().await;
        match active.as_mut() {
            Some(at) => {
                let mut ts = at.turn_state.lock().await;
                ts.take_pending_input()
            }
            None => Vec::with_capacity(0),
        }
    }

    pub async fn list_resources(
        &self,
        server: &str,
        params: Option<ListResourcesRequestParams>,
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
        params: Option<ListResourceTemplatesRequestParams>,
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
    ) -> anyhow::Result<CallToolResult> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .call_tool(server, tool, arguments)
            .await
    }

    pub(crate) async fn parse_mcp_tool_name(&self, tool_name: &str) -> Option<(String, String)> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .parse_tool_name(tool_name)
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

    pub(crate) fn notifier(&self) -> &UserNotifier {
        &self.services.notifier
    }

    pub(crate) fn user_shell(&self) -> &shell::Shell {
        &self.services.user_shell
    }

    fn show_raw_agent_reasoning(&self) -> bool {
        self.services.show_raw_agent_reasoning
    }

    async fn cancel_mcp_startup(&self) {
        self.services.mcp_startup_cancellation_token.cancel();
    }
}

async fn submission_loop(sess: Arc<Session>, config: Arc<Config>, rx_sub: Receiver<Submission>) {
    // Seed with context in case there is an OverrideTurnContext first.
    let mut previous_context: Option<Arc<TurnContext>> =
        Some(sess.new_turn(SessionSettingsUpdate::default()).await);

    // To break out of this loop, send Op::Shutdown.
    while let Ok(sub) = rx_sub.recv().await {
        debug!(?sub, "Submission");
        match sub.op.clone() {
            Op::Interrupt => {
                handlers::interrupt(&sess).await;
            }
            Op::OverrideTurnContext {
                cwd,
                approval_policy,
                sandbox_policy,
                model,
                effort,
                summary,
            } => {
                handlers::override_turn_context(
                    &sess,
                    SessionSettingsUpdate {
                        cwd,
                        approval_policy,
                        sandbox_policy,
                        model,
                        reasoning_effort: effort,
                        reasoning_summary: summary,
                        ..Default::default()
                    },
                )
                .await;
            }
            Op::UserInput { .. } | Op::UserTurn { .. } => {
                handlers::user_input_or_turn(&sess, sub.id.clone(), sub.op, &mut previous_context)
                    .await;
            }
            Op::ExecApproval { id, decision } => {
                handlers::exec_approval(&sess, id, decision).await;
            }
            Op::PatchApproval { id, decision } => {
                handlers::patch_approval(&sess, id, decision).await;
            }
            Op::AddToHistory { text } => {
                handlers::add_to_history(&sess, &config, text).await;
            }
            Op::GetHistoryEntryRequest { offset, log_id } => {
                handlers::get_history_entry_request(&sess, &config, sub.id.clone(), offset, log_id)
                    .await;
            }
            Op::ListMcpTools => {
                handlers::list_mcp_tools(&sess, &config, sub.id.clone()).await;
            }
            Op::ListCustomPrompts => {
                handlers::list_custom_prompts(&sess, sub.id.clone()).await;
            }
            Op::Undo => {
                handlers::undo(&sess, sub.id.clone()).await;
            }
            Op::Compact => {
                handlers::compact(&sess, sub.id.clone()).await;
            }
            Op::RunUserShellCommand { command } => {
                handlers::run_user_shell_command(
                    &sess,
                    sub.id.clone(),
                    command,
                    &mut previous_context,
                )
                .await;
            }
            Op::ResolveElicitation {
                server_name,
                request_id,
                decision,
            } => {
                handlers::resolve_elicitation(&sess, server_name, request_id, decision).await;
            }
            Op::Shutdown => {
                if handlers::shutdown(&sess, sub.id.clone()).await {
                    break;
                }
            }
            Op::Review { review_request } => {
                handlers::review(&sess, &config, sub.id.clone(), review_request).await;
            }
            _ => {} // Ignore unknown ops; enum is non_exhaustive to allow extensions.
        }
    }
    debug!("Agent loop exited");
}

/// Operation handlers
mod handlers {
    use crate::codex::Session;
    use crate::codex::SessionSettingsUpdate;
    use crate::codex::TurnContext;

    use crate::codex::spawn_review_thread;
    use crate::config::Config;
    use crate::mcp::auth::compute_auth_statuses;
    use crate::tasks::CompactTask;
    use crate::tasks::RegularTask;
    use crate::tasks::UndoTask;
    use crate::tasks::UserShellCommandTask;
    use codex_protocol::custom_prompts::CustomPrompt;
    use codex_protocol::protocol::CodexErrorInfo;
    use codex_protocol::protocol::ErrorEvent;
    use codex_protocol::protocol::Event;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::ListCustomPromptsResponseEvent;
    use codex_protocol::protocol::Op;
    use codex_protocol::protocol::ReviewDecision;
    use codex_protocol::protocol::ReviewRequest;
    use codex_protocol::protocol::TurnAbortReason;

    use codex_protocol::user_input::UserInput;
    use codex_rmcp_client::ElicitationAction;
    use codex_rmcp_client::ElicitationResponse;
    use mcp_types::RequestId;
    use std::sync::Arc;
    use tracing::info;
    use tracing::warn;

    pub async fn interrupt(sess: &Arc<Session>) {
        sess.interrupt_task().await;
    }

    pub async fn override_turn_context(sess: &Session, updates: SessionSettingsUpdate) {
        sess.update_settings(updates).await;
    }

    pub async fn user_input_or_turn(
        sess: &Arc<Session>,
        sub_id: String,
        op: Op,
        previous_context: &mut Option<Arc<TurnContext>>,
    ) {
        let (items, updates) = match op {
            Op::UserTurn {
                cwd,
                approval_policy,
                sandbox_policy,
                model,
                effort,
                summary,
                final_output_json_schema,
                items,
            } => (
                items,
                SessionSettingsUpdate {
                    cwd: Some(cwd),
                    approval_policy: Some(approval_policy),
                    sandbox_policy: Some(sandbox_policy),
                    model: Some(model),
                    reasoning_effort: Some(effort),
                    reasoning_summary: Some(summary),
                    final_output_json_schema: Some(final_output_json_schema),
                },
            ),
            Op::UserInput { items } => (items, SessionSettingsUpdate::default()),
            _ => unreachable!(),
        };

        let current_context = sess.new_turn_with_sub_id(sub_id, updates).await;
        current_context
            .client
            .get_otel_event_manager()
            .user_prompt(&items);

        // Attempt to inject input into current task
        if let Err(items) = sess.inject_input(items).await {
            if let Some(env_item) =
                sess.build_environment_update_item(previous_context.as_ref(), &current_context)
            {
                sess.record_conversation_items(&current_context, std::slice::from_ref(&env_item))
                    .await;
            }

            sess.spawn_task(Arc::clone(&current_context), items, RegularTask)
                .await;
            *previous_context = Some(current_context);
        }
    }

    pub async fn run_user_shell_command(
        sess: &Arc<Session>,
        sub_id: String,
        command: String,
        previous_context: &mut Option<Arc<TurnContext>>,
    ) {
        let turn_context = sess
            .new_turn_with_sub_id(sub_id, SessionSettingsUpdate::default())
            .await;
        sess.spawn_task(
            Arc::clone(&turn_context),
            Vec::new(),
            UserShellCommandTask::new(command),
        )
        .await;
        *previous_context = Some(turn_context);
    }

    pub async fn resolve_elicitation(
        sess: &Arc<Session>,
        server_name: String,
        request_id: RequestId,
        decision: codex_protocol::approvals::ElicitationAction,
    ) {
        let action = match decision {
            codex_protocol::approvals::ElicitationAction::Accept => ElicitationAction::Accept,
            codex_protocol::approvals::ElicitationAction::Decline => ElicitationAction::Decline,
            codex_protocol::approvals::ElicitationAction::Cancel => ElicitationAction::Cancel,
        };
        let response = ElicitationResponse {
            action,
            content: None,
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

    pub async fn exec_approval(sess: &Arc<Session>, id: String, decision: ReviewDecision) {
        match decision {
            ReviewDecision::Abort => {
                sess.interrupt_task().await;
            }
            other => sess.notify_approval(&id, other).await,
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
                    crate::protocol::GetHistoryEntryResponseEvent {
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

    pub async fn list_mcp_tools(sess: &Session, config: &Arc<Config>, sub_id: String) {
        let mcp_connection_manager = sess.services.mcp_connection_manager.read().await;
        let (tools, auth_status_entries, resources, resource_templates) = tokio::join!(
            mcp_connection_manager.list_all_tools(),
            compute_auth_statuses(
                config.mcp_servers.iter(),
                config.mcp_oauth_credentials_store_mode,
            ),
            mcp_connection_manager.list_all_resources(),
            mcp_connection_manager.list_all_resource_templates(),
        );
        let auth_statuses = auth_status_entries
            .iter()
            .map(|(name, entry)| (name.clone(), entry.auth_status))
            .collect();
        let event = Event {
            id: sub_id,
            msg: EventMsg::McpListToolsResponse(crate::protocol::McpListToolsResponseEvent {
                tools: tools
                    .into_iter()
                    .map(|(name, tool)| (name, tool.tool))
                    .collect(),
                resources,
                resource_templates,
                auth_statuses,
            }),
        };
        sess.send_event_raw(event).await;
    }

    pub async fn list_custom_prompts(sess: &Session, sub_id: String) {
        let custom_prompts: Vec<CustomPrompt> =
            if let Some(dir) = crate::custom_prompts::default_prompts_dir() {
                crate::custom_prompts::discover_prompts_in(&dir).await
            } else {
                Vec::new()
            };

        let event = Event {
            id: sub_id,
            msg: EventMsg::ListCustomPromptsResponse(ListCustomPromptsResponseEvent {
                custom_prompts,
            }),
        };
        sess.send_event_raw(event).await;
    }

    pub async fn undo(sess: &Arc<Session>, sub_id: String) {
        let turn_context = sess
            .new_turn_with_sub_id(sub_id, SessionSettingsUpdate::default())
            .await;
        sess.spawn_task(turn_context, Vec::new(), UndoTask::new())
            .await;
    }

    pub async fn compact(sess: &Arc<Session>, sub_id: String) {
        let turn_context = sess
            .new_turn_with_sub_id(sub_id, SessionSettingsUpdate::default())
            .await;

        sess.spawn_task(
            Arc::clone(&turn_context),
            vec![UserInput::Text {
                text: turn_context.compact_prompt().to_string(),
            }],
            CompactTask,
        )
        .await;
    }

    pub async fn shutdown(sess: &Arc<Session>, sub_id: String) -> bool {
        sess.abort_all_tasks(TurnAbortReason::Interrupted).await;
        sess.services
            .unified_exec_manager
            .terminate_all_sessions()
            .await;
        info!("Shutting down Codex instance");

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
        let turn_context = sess
            .new_turn_with_sub_id(sub_id.clone(), SessionSettingsUpdate::default())
            .await;
        spawn_review_thread(
            Arc::clone(sess),
            Arc::clone(config),
            turn_context.clone(),
            sub_id,
            review_request,
        )
        .await;
    }
}

/// Spawn a review thread using the given prompt.
async fn spawn_review_thread(
    sess: Arc<Session>,
    config: Arc<Config>,
    parent_turn_context: Arc<TurnContext>,
    sub_id: String,
    review_request: ReviewRequest,
) {
    let model = config.review_model.clone();
    let review_model_family = find_family_for_model(&model)
        .unwrap_or_else(|| parent_turn_context.client.get_model_family());
    // For reviews, disable web_search and view_image regardless of global settings.
    let mut review_features = config.features.clone();
    review_features
        .disable(crate::features::Feature::WebSearchRequest)
        .disable(crate::features::Feature::ViewImageTool);
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_family: &review_model_family,
        features: &review_features,
    });

    let base_instructions = REVIEW_PROMPT.to_string();
    let review_prompt = review_request.prompt.clone();
    let provider = parent_turn_context.client.get_provider();
    let auth_manager = parent_turn_context.client.get_auth_manager();
    let model_family = review_model_family.clone();

    // Build per‑turn client with the requested model/family.
    let mut per_turn_config = (*config).clone();
    per_turn_config.model = model.clone();
    per_turn_config.model_family = model_family.clone();
    per_turn_config.model_reasoning_effort = Some(ReasoningEffortConfig::Low);
    per_turn_config.model_reasoning_summary = ReasoningSummaryConfig::Detailed;
    if let Some(model_info) = get_model_info(&model_family) {
        per_turn_config.model_context_window = Some(model_info.context_window);
    }

    let otel_event_manager = parent_turn_context
        .client
        .get_otel_event_manager()
        .with_model(
            per_turn_config.model.as_str(),
            per_turn_config.model_family.slug.as_str(),
        );

    let per_turn_config = Arc::new(per_turn_config);
    let client = ModelClient::new(
        per_turn_config.clone(),
        auth_manager,
        otel_event_manager,
        provider,
        per_turn_config.model_reasoning_effort,
        per_turn_config.model_reasoning_summary,
        sess.conversation_id,
        parent_turn_context.client.get_session_source(),
    );

    let review_turn_context = TurnContext {
        sub_id: sub_id.to_string(),
        client,
        tools_config,
        developer_instructions: None,
        user_instructions: None,
        base_instructions: Some(base_instructions.clone()),
        compact_prompt: parent_turn_context.compact_prompt.clone(),
        approval_policy: parent_turn_context.approval_policy,
        sandbox_policy: parent_turn_context.sandbox_policy.clone(),
        shell_environment_policy: parent_turn_context.shell_environment_policy.clone(),
        cwd: parent_turn_context.cwd.clone(),
        final_output_json_schema: None,
        codex_linux_sandbox_exe: parent_turn_context.codex_linux_sandbox_exe.clone(),
        tool_call_gate: Arc::new(ReadinessFlag::new()),
        exec_policy: parent_turn_context.exec_policy.clone(),
        truncation_policy: TruncationPolicy::new(&per_turn_config),
    };

    // Seed the child task with the review prompt as the initial user message.
    let input: Vec<UserInput> = vec![UserInput::Text {
        text: review_prompt,
    }];
    let tc = Arc::new(review_turn_context);
    sess.spawn_task(tc.clone(), input, ReviewTask::new()).await;

    // Announce entering review mode so UIs can switch modes.
    sess.send_event(&tc, EventMsg::EnteredReviewMode(review_request))
        .await;
}

/// Takes a user message as input and runs a loop where, at each turn, the model
/// replies with either:
///
/// - requested function calls
/// - an assistant message
///
/// While it is possible for the model to return multiple of these items in a
/// single turn, in practice, we generally one item per turn:
///
/// - If the model requests a function call, we execute it and send the output
///   back to the model in the next turn.
/// - If the model sends only an assistant message, we record it in the
///   conversation history and consider the task complete.
///
pub(crate) async fn run_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    input: Vec<UserInput>,
    cancellation_token: CancellationToken,
) -> Option<String> {
    if input.is_empty() {
        return None;
    }
    let event = EventMsg::TaskStarted(TaskStartedEvent {
        model_context_window: turn_context.client.get_model_context_window(),
    });
    sess.send_event(&turn_context, event).await;

    let initial_input_for_turn: ResponseInputItem = ResponseInputItem::from(input);
    let response_item: ResponseItem = initial_input_for_turn.clone().into();
    sess.record_response_item_and_emit_turn_item(turn_context.as_ref(), response_item)
        .await;

    sess.maybe_start_ghost_snapshot(Arc::clone(&turn_context), cancellation_token.child_token())
        .await;
    let mut last_agent_message: Option<String> = None;
    // Although from the perspective of codex.rs, TurnDiffTracker has the lifecycle of a Task which contains
    // many turns, from the perspective of the user, it is a single turn.
    let turn_diff_tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));

    loop {
        // Note that pending_input would be something like a message the user
        // submitted through the UI while the model was running. Though the UI
        // may support this, the model might not.
        let pending_input = sess
            .get_pending_input()
            .await
            .into_iter()
            .map(ResponseItem::from)
            .collect::<Vec<ResponseItem>>();

        // Construct the input that we will send to the model.
        let turn_input: Vec<ResponseItem> = {
            sess.record_conversation_items(&turn_context, &pending_input)
                .await;
            sess.clone_history().await.get_history_for_prompt()
        };

        let turn_input_messages = turn_input
            .iter()
            .filter_map(|item| match parse_turn_item(item) {
                Some(TurnItem::UserMessage(user_message)) => Some(user_message),
                _ => None,
            })
            .map(|user_message| user_message.message())
            .collect::<Vec<String>>();
        match run_turn(
            Arc::clone(&sess),
            Arc::clone(&turn_context),
            Arc::clone(&turn_diff_tracker),
            turn_input,
            cancellation_token.child_token(),
        )
        .await
        {
            Ok(turn_output) => {
                let processed_items = turn_output;
                let limit = turn_context
                    .client
                    .get_auto_compact_token_limit()
                    .unwrap_or(i64::MAX);
                let total_usage_tokens = sess.get_total_token_usage().await;
                let token_limit_reached = total_usage_tokens >= limit;
                let (responses, items_to_record_in_conversation_history) =
                    process_items(processed_items, &sess, &turn_context).await;

                // as long as compaction works well in getting us way below the token limit, we shouldn't worry about being in an infinite loop.
                if token_limit_reached {
                    if should_use_remote_compact_task(&sess).await {
                        run_inline_remote_auto_compact_task(sess.clone(), turn_context.clone())
                            .await;
                    } else {
                        run_inline_auto_compact_task(sess.clone(), turn_context.clone()).await;
                    }
                    continue;
                }

                if responses.is_empty() {
                    last_agent_message = get_last_assistant_message_from_turn(
                        &items_to_record_in_conversation_history,
                    );
                    sess.notifier()
                        .notify(&UserNotification::AgentTurnComplete {
                            thread_id: sess.conversation_id.to_string(),
                            turn_id: turn_context.sub_id.clone(),
                            cwd: turn_context.cwd.display().to_string(),
                            input_messages: turn_input_messages,
                            last_assistant_message: last_agent_message.clone(),
                        });
                    break;
                }
                continue;
            }
            Err(CodexErr::TurnAborted {
                dangling_artifacts: processed_items,
            }) => {
                let _ = process_items(processed_items, &sess, &turn_context).await;
                // Aborted turn is reported via a different event.
                break;
            }
            Err(e) => {
                info!("Turn error: {e:#}");
                let event = EventMsg::Error(e.to_error_event(None));
                sess.send_event(&turn_context, event).await;
                // let the user continue the conversation
                break;
            }
        }
    }

    last_agent_message
}

async fn run_turn(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    turn_diff_tracker: SharedTurnDiffTracker,
    input: Vec<ResponseItem>,
    cancellation_token: CancellationToken,
) -> CodexResult<Vec<ProcessedResponseItem>> {
    let mcp_tools = sess
        .services
        .mcp_connection_manager
        .read()
        .await
        .list_all_tools()
        .or_cancel(&cancellation_token)
        .await?;
    let router = Arc::new(ToolRouter::from_config(
        &turn_context.tools_config,
        Some(
            mcp_tools
                .into_iter()
                .map(|(name, tool)| (name, tool.tool))
                .collect(),
        ),
    ));

    let model_supports_parallel = turn_context
        .client
        .get_model_family()
        .supports_parallel_tool_calls;

    // TODO(jif) revert once testing phase is done.
    let parallel_tool_calls = model_supports_parallel
        && sess
            .state
            .lock()
            .await
            .session_configuration
            .features
            .enabled(Feature::ParallelToolCalls);
    let mut base_instructions = turn_context.base_instructions.clone();
    if parallel_tool_calls {
        static INSTRUCTIONS: &str = include_str!("../templates/parallel/instructions.md");
        if let Some(family) =
            find_family_for_model(&sess.state.lock().await.session_configuration.model)
        {
            let mut new_instructions = base_instructions.unwrap_or(family.base_instructions);
            new_instructions.push_str(INSTRUCTIONS);
            base_instructions = Some(new_instructions);
        }
    }
    let prompt = Prompt {
        input,
        tools: router.specs(),
        parallel_tool_calls,
        base_instructions_override: base_instructions,
        output_schema: turn_context.final_output_json_schema.clone(),
    };

    let mut retries = 0;
    loop {
        match try_run_turn(
            Arc::clone(&router),
            Arc::clone(&sess),
            Arc::clone(&turn_context),
            Arc::clone(&turn_diff_tracker),
            &prompt,
            cancellation_token.child_token(),
        )
        .await
        {
            Ok(output) => return Ok(output),
            Err(CodexErr::TurnAborted {
                dangling_artifacts: processed_items,
            }) => {
                return Err(CodexErr::TurnAborted {
                    dangling_artifacts: processed_items,
                });
            }
            Err(CodexErr::Interrupted) => return Err(CodexErr::Interrupted),
            Err(CodexErr::EnvVar(var)) => return Err(CodexErr::EnvVar(var)),
            Err(e @ CodexErr::Fatal(_)) => return Err(e),
            Err(e @ CodexErr::ContextWindowExceeded) => {
                sess.set_total_tokens_full(&turn_context).await;
                return Err(e);
            }
            Err(CodexErr::UsageLimitReached(e)) => {
                let rate_limits = e.rate_limits.clone();
                if let Some(rate_limits) = rate_limits {
                    sess.update_rate_limits(&turn_context, rate_limits).await;
                }
                return Err(CodexErr::UsageLimitReached(e));
            }
            Err(CodexErr::UsageNotIncluded) => return Err(CodexErr::UsageNotIncluded),
            Err(e @ CodexErr::QuotaExceeded) => return Err(e),
            Err(e @ CodexErr::RefreshTokenFailed(_)) => return Err(e),
            Err(e) => {
                // Use the configured provider-specific stream retry budget.
                let max_retries = turn_context.client.get_provider().stream_max_retries();
                if retries < max_retries {
                    retries += 1;
                    let delay = match e {
                        CodexErr::Stream(_, Some(delay)) => delay,
                        _ => backoff(retries),
                    };
                    warn!(
                        "stream disconnected - retrying turn ({retries}/{max_retries} in {delay:?})...",
                    );

                    // Surface retry information to any UI/front‑end so the
                    // user understands what is happening instead of staring
                    // at a seemingly frozen screen.
                    sess.notify_stream_error(
                        &turn_context,
                        format!("Reconnecting... {retries}/{max_retries}"),
                        e,
                    )
                    .await;

                    tokio::time::sleep(delay).await;
                } else {
                    return Err(e);
                }
            }
        }
    }
}

/// When the model is prompted, it returns a stream of events. Some of these
/// events map to a `ResponseItem`. A `ResponseItem` may need to be
/// "handled" such that it produces a `ResponseInputItem` that needs to be
/// sent back to the model on the next turn.
#[derive(Debug)]
pub struct ProcessedResponseItem {
    pub item: ResponseItem,
    pub response: Option<ResponseInputItem>,
}

#[allow(clippy::too_many_arguments)]
async fn try_run_turn(
    router: Arc<ToolRouter>,
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    turn_diff_tracker: SharedTurnDiffTracker,
    prompt: &Prompt,
    cancellation_token: CancellationToken,
) -> CodexResult<Vec<ProcessedResponseItem>> {
    let rollout_item = RolloutItem::TurnContext(TurnContextItem {
        cwd: turn_context.cwd.clone(),
        approval_policy: turn_context.approval_policy,
        sandbox_policy: turn_context.sandbox_policy.clone(),
        model: turn_context.client.get_model(),
        effort: turn_context.client.get_reasoning_effort(),
        summary: turn_context.client.get_reasoning_summary(),
    });

    sess.persist_rollout_items(&[rollout_item]).await;
    let mut stream = turn_context
        .client
        .clone()
        .stream(prompt)
        .or_cancel(&cancellation_token)
        .await??;

    let tool_runtime = ToolCallRuntime::new(
        Arc::clone(&router),
        Arc::clone(&sess),
        Arc::clone(&turn_context),
        Arc::clone(&turn_diff_tracker),
    );
    let mut output: FuturesOrdered<BoxFuture<CodexResult<ProcessedResponseItem>>> =
        FuturesOrdered::new();

    let mut active_item: Option<TurnItem> = None;

    loop {
        // Poll the next item from the model stream. We must inspect *both* Ok and Err
        // cases so that transient stream failures (e.g., dropped SSE connection before
        // `response.completed`) bubble up and trigger the caller's retry logic.
        let event = match stream.next().or_cancel(&cancellation_token).await {
            Ok(event) => event,
            Err(codex_async_utils::CancelErr::Cancelled) => {
                let processed_items = output.try_collect().await?;
                return Err(CodexErr::TurnAborted {
                    dangling_artifacts: processed_items,
                });
            }
        };

        let event = match event {
            Some(res) => res?,
            None => {
                return Err(CodexErr::Stream(
                    "stream closed before response.completed".into(),
                    None,
                ));
            }
        };

        let add_completed = &mut |response_item: ProcessedResponseItem| {
            output.push_back(future::ready(Ok(response_item)).boxed());
        };

        match event {
            ResponseEvent::Created => {}
            ResponseEvent::OutputItemDone(item) => {
                let previously_active_item = active_item.take();
                match ToolRouter::build_tool_call(sess.as_ref(), item.clone()).await {
                    Ok(Some(call)) => {
                        let payload_preview = call.payload.log_payload().into_owned();
                        tracing::info!("ToolCall: {} {}", call.tool_name, payload_preview);

                        let response =
                            tool_runtime.handle_tool_call(call, cancellation_token.child_token());

                        output.push_back(
                            async move {
                                Ok(ProcessedResponseItem {
                                    item,
                                    response: Some(response.await?),
                                })
                            }
                            .boxed(),
                        );
                    }
                    Ok(None) => {
                        if let Some(turn_item) = handle_non_tool_response_item(&item).await {
                            if previously_active_item.is_none() {
                                sess.emit_turn_item_started(&turn_context, &turn_item).await;
                            }

                            sess.emit_turn_item_completed(&turn_context, turn_item)
                                .await;
                        }

                        add_completed(ProcessedResponseItem {
                            item,
                            response: None,
                        });
                    }
                    Err(FunctionCallError::MissingLocalShellCallId) => {
                        let msg = "LocalShellCall without call_id or id";
                        turn_context
                            .client
                            .get_otel_event_manager()
                            .log_tool_failed("local_shell", msg);
                        error!(msg);

                        let response = ResponseInputItem::FunctionCallOutput {
                            call_id: String::new(),
                            output: FunctionCallOutputPayload {
                                content: msg.to_string(),
                                ..Default::default()
                            },
                        };
                        add_completed(ProcessedResponseItem {
                            item,
                            response: Some(response),
                        });
                    }
                    Err(FunctionCallError::RespondToModel(message))
                    | Err(FunctionCallError::Denied(message)) => {
                        let response = ResponseInputItem::FunctionCallOutput {
                            call_id: String::new(),
                            output: FunctionCallOutputPayload {
                                content: message,
                                ..Default::default()
                            },
                        };
                        add_completed(ProcessedResponseItem {
                            item,
                            response: Some(response),
                        });
                    }
                    Err(FunctionCallError::Fatal(message)) => {
                        return Err(CodexErr::Fatal(message));
                    }
                }
            }
            ResponseEvent::OutputItemAdded(item) => {
                if let Some(turn_item) = handle_non_tool_response_item(&item).await {
                    let tracked_item = turn_item.clone();
                    sess.emit_turn_item_started(&turn_context, &turn_item).await;

                    active_item = Some(tracked_item);
                }
            }
            ResponseEvent::RateLimits(snapshot) => {
                // Update internal state with latest rate limits, but defer sending until
                // token usage is available to avoid duplicate TokenCount events.
                sess.update_rate_limits(&turn_context, snapshot).await;
            }
            ResponseEvent::Completed {
                response_id: _,
                token_usage,
            } => {
                sess.update_token_usage_info(&turn_context, token_usage.as_ref())
                    .await;
                let processed_items = output.try_collect().await?;
                let unified_diff = {
                    let mut tracker = turn_diff_tracker.lock().await;
                    tracker.get_unified_diff()
                };
                if let Ok(Some(unified_diff)) = unified_diff {
                    let msg = EventMsg::TurnDiff(TurnDiffEvent { unified_diff });
                    sess.send_event(&turn_context, msg).await;
                }

                return Ok(processed_items);
            }
            ResponseEvent::OutputTextDelta(delta) => {
                // In review child threads, suppress assistant text deltas; the
                // UI will show a selection popup from the final ReviewOutput.
                if let Some(active) = active_item.as_ref() {
                    let event = AgentMessageContentDeltaEvent {
                        thread_id: sess.conversation_id.to_string(),
                        turn_id: turn_context.sub_id.clone(),
                        item_id: active.id(),
                        delta: delta.clone(),
                    };
                    sess.send_event(&turn_context, EventMsg::AgentMessageContentDelta(event))
                        .await;
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
    }
}

async fn handle_non_tool_response_item(item: &ResponseItem) -> Option<TurnItem> {
    debug!(?item, "Output item");

    match item {
        ResponseItem::Message { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::WebSearchCall { .. } => parse_turn_item(item),
        ResponseItem::FunctionCallOutput { .. } | ResponseItem::CustomToolCallOutput { .. } => {
            debug!("unexpected tool output from stream");
            None
        }
        _ => None,
    }
}

pub(super) fn get_last_assistant_message_from_turn(responses: &[ResponseItem]) -> Option<String> {
    responses.iter().rev().find_map(|item| {
        if let ResponseItem::Message { role, content, .. } = item {
            if role == "assistant" {
                content.iter().rev().find_map(|ci| {
                    if let ContentItem::OutputText { text } = ci {
                        Some(text.clone())
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        } else {
            None
        }
    })
}

use crate::features::Features;
#[cfg(test)]
pub(crate) use tests::make_session_and_context;

#[cfg(test)]
pub(crate) use tests::make_session_and_context_with_rx;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigOverrides;
    use crate::config::ConfigToml;
    use crate::exec::ExecToolCallOutput;
    use crate::shell::default_user_shell;
    use crate::tools::format_exec_output_str;

    use crate::protocol::CompactedItem;
    use crate::protocol::InitialHistory;
    use crate::protocol::ResumedHistory;
    use crate::state::TaskKind;
    use crate::tasks::SessionTask;
    use crate::tasks::SessionTaskContext;
    use crate::tools::ToolRouter;
    use crate::tools::context::ToolInvocation;
    use crate::tools::context::ToolOutput;
    use crate::tools::context::ToolPayload;
    use crate::tools::handlers::ShellHandler;
    use crate::tools::handlers::UnifiedExecHandler;
    use crate::tools::registry::ToolHandler;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use codex_app_server_protocol::AuthMode;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use std::time::Duration;
    use tokio::time::sleep;

    use mcp_types::ContentBlock;
    use mcp_types::TextContent;
    use pretty_assertions::assert_eq;
    use serde::Deserialize;
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration as StdDuration;

    #[test]
    fn reconstruct_history_matches_live_compactions() {
        let (session, turn_context) = make_session_and_context();
        let (rollout_items, expected) = sample_rollout(&session, &turn_context);

        let reconstructed = session.reconstruct_history_from_rollout(&turn_context, &rollout_items);

        assert_eq!(expected, reconstructed);
    }

    #[test]
    fn record_initial_history_reconstructs_resumed_transcript() {
        let (session, turn_context) = make_session_and_context();
        let (rollout_items, expected) = sample_rollout(&session, &turn_context);

        tokio_test::block_on(session.record_initial_history(InitialHistory::Resumed(
            ResumedHistory {
                conversation_id: ConversationId::default(),
                history: rollout_items,
                rollout_path: PathBuf::from("/tmp/resume.jsonl"),
            },
        )));

        let actual = tokio_test::block_on(async {
            session.state.lock().await.clone_history().get_history()
        });
        assert_eq!(expected, actual);
    }

    #[test]
    fn record_initial_history_reconstructs_forked_transcript() {
        let (session, turn_context) = make_session_and_context();
        let (rollout_items, expected) = sample_rollout(&session, &turn_context);

        tokio_test::block_on(session.record_initial_history(InitialHistory::Forked(rollout_items)));

        let actual = tokio_test::block_on(async {
            session.state.lock().await.clone_history().get_history()
        });
        assert_eq!(expected, actual);
    }

    #[test]
    fn prefers_structured_content_when_present() {
        let ctr = CallToolResult {
            // Content present but should be ignored because structured_content is set.
            content: vec![text_block("ignored")],
            is_error: None,
            structured_content: Some(json!({
                "ok": true,
                "value": 42
            })),
        };

        let got = FunctionCallOutputPayload::from(&ctr);
        let expected = FunctionCallOutputPayload {
            content: serde_json::to_string(&json!({
                "ok": true,
                "value": 42
            }))
            .unwrap(),
            success: Some(true),
            ..Default::default()
        };

        assert_eq!(expected, got);
    }

    #[test]
    fn includes_timed_out_message() {
        let exec = ExecToolCallOutput {
            exit_code: 0,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new("Command output".to_string()),
            duration: StdDuration::from_secs(1),
            timed_out: true,
        };
        let (_, turn_context) = make_session_and_context();

        let out = format_exec_output_str(&exec, turn_context.truncation_policy);

        assert_eq!(
            out,
            "command timed out after 1000 milliseconds\nCommand output"
        );
    }

    #[test]
    fn falls_back_to_content_when_structured_is_null() {
        let ctr = CallToolResult {
            content: vec![text_block("hello"), text_block("world")],
            is_error: None,
            structured_content: Some(serde_json::Value::Null),
        };

        let got = FunctionCallOutputPayload::from(&ctr);
        let expected = FunctionCallOutputPayload {
            content: serde_json::to_string(&vec![text_block("hello"), text_block("world")])
                .unwrap(),
            success: Some(true),
            ..Default::default()
        };

        assert_eq!(expected, got);
    }

    #[test]
    fn success_flag_reflects_is_error_true() {
        let ctr = CallToolResult {
            content: vec![text_block("unused")],
            is_error: Some(true),
            structured_content: Some(json!({ "message": "bad" })),
        };

        let got = FunctionCallOutputPayload::from(&ctr);
        let expected = FunctionCallOutputPayload {
            content: serde_json::to_string(&json!({ "message": "bad" })).unwrap(),
            success: Some(false),
            ..Default::default()
        };

        assert_eq!(expected, got);
    }

    #[test]
    fn success_flag_true_with_no_error_and_content_used() {
        let ctr = CallToolResult {
            content: vec![text_block("alpha")],
            is_error: Some(false),
            structured_content: None,
        };

        let got = FunctionCallOutputPayload::from(&ctr);
        let expected = FunctionCallOutputPayload {
            content: serde_json::to_string(&vec![text_block("alpha")]).unwrap(),
            success: Some(true),
            ..Default::default()
        };

        assert_eq!(expected, got);
    }

    fn text_block(s: &str) -> ContentBlock {
        ContentBlock::TextContent(TextContent {
            annotations: None,
            text: s.to_string(),
            r#type: "text".to_string(),
        })
    }

    fn otel_event_manager(conversation_id: ConversationId, config: &Config) -> OtelEventManager {
        OtelEventManager::new(
            conversation_id,
            config.model.as_str(),
            config.model_family.slug.as_str(),
            None,
            Some("test@test.com".to_string()),
            Some(AuthMode::ChatGPT),
            false,
            "test".to_string(),
        )
    }

    pub(crate) fn make_session_and_context() -> (Session, TurnContext) {
        let (tx_event, _rx_event) = async_channel::unbounded();
        let codex_home = tempfile::tempdir().expect("create temp dir");
        let config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            codex_home.path().to_path_buf(),
        )
        .expect("load default test config");
        let config = Arc::new(config);
        let conversation_id = ConversationId::default();
        let otel_event_manager = otel_event_manager(conversation_id, config.as_ref());
        let auth_manager = AuthManager::shared(
            config.cwd.clone(),
            false,
            config.cli_auth_credentials_store_mode,
        );

        let session_configuration = SessionConfiguration {
            provider: config.model_provider.clone(),
            model: config.model.clone(),
            model_reasoning_effort: config.model_reasoning_effort,
            model_reasoning_summary: config.model_reasoning_summary,
            developer_instructions: config.developer_instructions.clone(),
            user_instructions: config.user_instructions.clone(),
            base_instructions: config.base_instructions.clone(),
            compact_prompt: config.compact_prompt.clone(),
            approval_policy: config.approval_policy,
            sandbox_policy: config.sandbox_policy.clone(),
            cwd: config.cwd.clone(),
            original_config_do_not_use: Arc::clone(&config),
            features: Features::default(),
            exec_policy: Arc::new(ExecPolicy::empty()),
            session_source: SessionSource::Exec,
        };

        let state = SessionState::new(session_configuration.clone());

        let services = SessionServices {
            mcp_connection_manager: Arc::new(RwLock::new(McpConnectionManager::default())),
            mcp_startup_cancellation_token: CancellationToken::new(),
            unified_exec_manager: UnifiedExecSessionManager::default(),
            notifier: UserNotifier::new(None),
            rollout: Mutex::new(None),
            user_shell: default_user_shell(),
            show_raw_agent_reasoning: config.show_raw_agent_reasoning,
            auth_manager: Arc::clone(&auth_manager),
            otel_event_manager: otel_event_manager.clone(),
            tool_approvals: Mutex::new(ApprovalStore::default()),
        };

        let turn_context = Session::make_turn_context(
            Some(Arc::clone(&auth_manager)),
            &otel_event_manager,
            session_configuration.provider.clone(),
            &session_configuration,
            conversation_id,
            "turn_id".to_string(),
        );

        let session = Session {
            conversation_id,
            tx_event,
            state: Mutex::new(state),
            active_turn: Mutex::new(None),
            services,
            next_internal_sub_id: AtomicU64::new(0),
        };

        (session, turn_context)
    }

    // Like make_session_and_context, but returns Arc<Session> and the event receiver
    // so tests can assert on emitted events.
    pub(crate) fn make_session_and_context_with_rx() -> (
        Arc<Session>,
        Arc<TurnContext>,
        async_channel::Receiver<Event>,
    ) {
        let (tx_event, rx_event) = async_channel::unbounded();
        let codex_home = tempfile::tempdir().expect("create temp dir");
        let config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            codex_home.path().to_path_buf(),
        )
        .expect("load default test config");
        let config = Arc::new(config);
        let conversation_id = ConversationId::default();
        let otel_event_manager = otel_event_manager(conversation_id, config.as_ref());
        let auth_manager = AuthManager::shared(
            config.cwd.clone(),
            false,
            config.cli_auth_credentials_store_mode,
        );

        let session_configuration = SessionConfiguration {
            provider: config.model_provider.clone(),
            model: config.model.clone(),
            model_reasoning_effort: config.model_reasoning_effort,
            model_reasoning_summary: config.model_reasoning_summary,
            developer_instructions: config.developer_instructions.clone(),
            user_instructions: config.user_instructions.clone(),
            base_instructions: config.base_instructions.clone(),
            compact_prompt: config.compact_prompt.clone(),
            approval_policy: config.approval_policy,
            sandbox_policy: config.sandbox_policy.clone(),
            cwd: config.cwd.clone(),
            original_config_do_not_use: Arc::clone(&config),
            features: Features::default(),
            exec_policy: Arc::new(ExecPolicy::empty()),
            session_source: SessionSource::Exec,
        };

        let state = SessionState::new(session_configuration.clone());

        let services = SessionServices {
            mcp_connection_manager: Arc::new(RwLock::new(McpConnectionManager::default())),
            mcp_startup_cancellation_token: CancellationToken::new(),
            unified_exec_manager: UnifiedExecSessionManager::default(),
            notifier: UserNotifier::new(None),
            rollout: Mutex::new(None),
            user_shell: default_user_shell(),
            show_raw_agent_reasoning: config.show_raw_agent_reasoning,
            auth_manager: Arc::clone(&auth_manager),
            otel_event_manager: otel_event_manager.clone(),
            tool_approvals: Mutex::new(ApprovalStore::default()),
        };

        let turn_context = Arc::new(Session::make_turn_context(
            Some(Arc::clone(&auth_manager)),
            &otel_event_manager,
            session_configuration.provider.clone(),
            &session_configuration,
            conversation_id,
            "turn_id".to_string(),
        ));

        let session = Arc::new(Session {
            conversation_id,
            tx_event,
            state: Mutex::new(state),
            active_turn: Mutex::new(None),
            services,
            next_internal_sub_id: AtomicU64::new(0),
        });

        (session, turn_context, rx_event)
    }

    #[derive(Clone, Copy)]
    struct NeverEndingTask {
        kind: TaskKind,
        listen_to_cancellation_token: bool,
    }

    #[async_trait::async_trait]
    impl SessionTask for NeverEndingTask {
        fn kind(&self) -> TaskKind {
            self.kind
        }

        async fn run(
            self: Arc<Self>,
            _session: Arc<SessionTaskContext>,
            _ctx: Arc<TurnContext>,
            _input: Vec<UserInput>,
            cancellation_token: CancellationToken,
        ) -> Option<String> {
            if self.listen_to_cancellation_token {
                cancellation_token.cancelled().await;
                return None;
            }
            loop {
                sleep(Duration::from_secs(60)).await;
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[test_log::test]
    async fn abort_regular_task_emits_turn_aborted_only() {
        let (sess, tc, rx) = make_session_and_context_with_rx();
        let input = vec![UserInput::Text {
            text: "hello".to_string(),
        }];
        sess.spawn_task(
            Arc::clone(&tc),
            input,
            NeverEndingTask {
                kind: TaskKind::Regular,
                listen_to_cancellation_token: false,
            },
        )
        .await;

        sess.abort_all_tasks(TurnAbortReason::Interrupted).await;

        let evt = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout waiting for event")
            .expect("event");
        match evt.msg {
            EventMsg::TurnAborted(e) => assert_eq!(TurnAbortReason::Interrupted, e.reason),
            other => panic!("unexpected event: {other:?}"),
        }
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn abort_gracefuly_emits_turn_aborted_only() {
        let (sess, tc, rx) = make_session_and_context_with_rx();
        let input = vec![UserInput::Text {
            text: "hello".to_string(),
        }];
        sess.spawn_task(
            Arc::clone(&tc),
            input,
            NeverEndingTask {
                kind: TaskKind::Regular,
                listen_to_cancellation_token: true,
            },
        )
        .await;

        sess.abort_all_tasks(TurnAbortReason::Interrupted).await;

        let evt = rx.recv().await.expect("event");
        match evt.msg {
            EventMsg::TurnAborted(e) => assert_eq!(TurnAbortReason::Interrupted, e.reason),
            other => panic!("unexpected event: {other:?}"),
        }
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn abort_review_task_emits_exited_then_aborted_and_records_history() {
        let (sess, tc, rx) = make_session_and_context_with_rx();
        let input = vec![UserInput::Text {
            text: "start review".to_string(),
        }];
        sess.spawn_task(Arc::clone(&tc), input, ReviewTask::new())
            .await;

        sess.abort_all_tasks(TurnAbortReason::Interrupted).await;

        // Drain events until we observe ExitedReviewMode; earlier
        // RawResponseItem entries (e.g., environment context) may arrive first.
        loop {
            let evt = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
                .await
                .expect("timeout waiting for first event")
                .expect("first event");
            match evt.msg {
                EventMsg::ExitedReviewMode(ev) => {
                    assert!(ev.review_output.is_none());
                    break;
                }
                // Ignore any non-critical events before exit.
                _ => continue,
            }
        }
        loop {
            let evt = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
                .await
                .expect("timeout waiting for next event")
                .expect("event");
            match evt.msg {
                EventMsg::RawResponseItem(_) => continue,
                EventMsg::ItemStarted(_) | EventMsg::ItemCompleted(_) => continue,
                EventMsg::AgentMessage(_) => continue,
                EventMsg::TurnAborted(e) => {
                    assert_eq!(TurnAbortReason::Interrupted, e.reason);
                    break;
                }
                other => panic!("unexpected second event: {other:?}"),
            }
        }

        let history = sess.clone_history().await.get_history();
        let _ = history;
    }

    #[tokio::test]
    async fn fatal_tool_error_stops_turn_and_reports_error() {
        let (session, turn_context, _rx) = make_session_and_context_with_rx();
        let tools = {
            session
                .services
                .mcp_connection_manager
                .read()
                .await
                .list_all_tools()
                .await
        };
        let router = ToolRouter::from_config(
            &turn_context.tools_config,
            Some(
                tools
                    .into_iter()
                    .map(|(name, tool)| (name, tool.tool))
                    .collect(),
            ),
        );
        let item = ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "call-1".to_string(),
            name: "shell".to_string(),
            input: "{}".to_string(),
        };

        let call = ToolRouter::build_tool_call(session.as_ref(), item.clone())
            .await
            .expect("build tool call")
            .expect("tool call present");
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));
        let err = router
            .dispatch_tool_call(
                Arc::clone(&session),
                Arc::clone(&turn_context),
                tracker,
                call,
            )
            .await
            .expect_err("expected fatal error");

        match err {
            FunctionCallError::Fatal(message) => {
                assert_eq!(message, "tool shell invoked with incompatible payload");
            }
            other => panic!("expected FunctionCallError::Fatal, got {other:?}"),
        }
    }

    fn sample_rollout(
        session: &Session,
        turn_context: &TurnContext,
    ) -> (Vec<RolloutItem>, Vec<ResponseItem>) {
        let mut rollout_items = Vec::new();
        let mut live_history = ContextManager::new();

        let initial_context = session.build_initial_context(turn_context);
        for item in &initial_context {
            rollout_items.push(RolloutItem::ResponseItem(item.clone()));
        }
        live_history.record_items(initial_context.iter(), turn_context.truncation_policy);

        let user1 = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "first user".to_string(),
            }],
        };
        live_history.record_items(std::iter::once(&user1), turn_context.truncation_policy);
        rollout_items.push(RolloutItem::ResponseItem(user1.clone()));

        let assistant1 = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "assistant reply one".to_string(),
            }],
        };
        live_history.record_items(std::iter::once(&assistant1), turn_context.truncation_policy);
        rollout_items.push(RolloutItem::ResponseItem(assistant1.clone()));

        let summary1 = "summary one";
        let snapshot1 = live_history.get_history();
        let user_messages1 = collect_user_messages(&snapshot1);
        let rebuilt1 = compact::build_compacted_history(
            session.build_initial_context(turn_context),
            &user_messages1,
            summary1,
        );
        live_history.replace(rebuilt1);
        rollout_items.push(RolloutItem::Compacted(CompactedItem {
            message: summary1.to_string(),
            replacement_history: None,
        }));

        let user2 = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "second user".to_string(),
            }],
        };
        live_history.record_items(std::iter::once(&user2), turn_context.truncation_policy);
        rollout_items.push(RolloutItem::ResponseItem(user2.clone()));

        let assistant2 = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "assistant reply two".to_string(),
            }],
        };
        live_history.record_items(std::iter::once(&assistant2), turn_context.truncation_policy);
        rollout_items.push(RolloutItem::ResponseItem(assistant2.clone()));

        let summary2 = "summary two";
        let snapshot2 = live_history.get_history();
        let user_messages2 = collect_user_messages(&snapshot2);
        let rebuilt2 = compact::build_compacted_history(
            session.build_initial_context(turn_context),
            &user_messages2,
            summary2,
        );
        live_history.replace(rebuilt2);
        rollout_items.push(RolloutItem::Compacted(CompactedItem {
            message: summary2.to_string(),
            replacement_history: None,
        }));

        let user3 = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "third user".to_string(),
            }],
        };
        live_history.record_items(std::iter::once(&user3), turn_context.truncation_policy);
        rollout_items.push(RolloutItem::ResponseItem(user3.clone()));

        let assistant3 = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "assistant reply three".to_string(),
            }],
        };
        live_history.record_items(std::iter::once(&assistant3), turn_context.truncation_policy);
        rollout_items.push(RolloutItem::ResponseItem(assistant3.clone()));

        (rollout_items, live_history.get_history())
    }

    #[tokio::test]
    async fn rejects_escalated_permissions_when_policy_not_on_request() {
        use crate::exec::ExecParams;
        use crate::protocol::AskForApproval;
        use crate::protocol::SandboxPolicy;
        use crate::turn_diff_tracker::TurnDiffTracker;
        use std::collections::HashMap;

        let (session, mut turn_context_raw) = make_session_and_context();
        // Ensure policy is NOT OnRequest so the early rejection path triggers
        turn_context_raw.approval_policy = AskForApproval::OnFailure;
        let session = Arc::new(session);
        let mut turn_context = Arc::new(turn_context_raw);

        let timeout_ms = 1000;
        let params = ExecParams {
            command: if cfg!(windows) {
                vec![
                    "cmd.exe".to_string(),
                    "/C".to_string(),
                    "echo hi".to_string(),
                ]
            } else {
                vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    "echo hi".to_string(),
                ]
            },
            cwd: turn_context.cwd.clone(),
            expiration: timeout_ms.into(),
            env: HashMap::new(),
            with_escalated_permissions: Some(true),
            justification: Some("test".to_string()),
            arg0: None,
        };

        let params2 = ExecParams {
            with_escalated_permissions: Some(false),
            command: params.command.clone(),
            cwd: params.cwd.clone(),
            expiration: timeout_ms.into(),
            env: HashMap::new(),
            justification: params.justification.clone(),
            arg0: None,
        };

        let turn_diff_tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));

        let tool_name = "shell";
        let call_id = "test-call".to_string();

        let handler = ShellHandler;
        let resp = handler
            .handle(ToolInvocation {
                session: Arc::clone(&session),
                turn: Arc::clone(&turn_context),
                tracker: Arc::clone(&turn_diff_tracker),
                call_id,
                tool_name: tool_name.to_string(),
                payload: ToolPayload::Function {
                    arguments: serde_json::json!({
                        "command": params.command.clone(),
                        "workdir": Some(turn_context.cwd.to_string_lossy().to_string()),
                        "timeout_ms": params.expiration.timeout_ms(),
                        "with_escalated_permissions": params.with_escalated_permissions,
                        "justification": params.justification.clone(),
                    })
                    .to_string(),
                },
            })
            .await;

        let Err(FunctionCallError::RespondToModel(output)) = resp else {
            panic!("expected error result");
        };

        let expected = format!(
            "approval policy is {policy:?}; reject command — you should not ask for escalated permissions if the approval policy is {policy:?}",
            policy = turn_context.approval_policy
        );

        pretty_assertions::assert_eq!(output, expected);

        // Now retry the same command WITHOUT escalated permissions; should succeed.
        // Force DangerFullAccess to avoid platform sandbox dependencies in tests.
        Arc::get_mut(&mut turn_context)
            .expect("unique turn context Arc")
            .sandbox_policy = SandboxPolicy::DangerFullAccess;

        let resp2 = handler
            .handle(ToolInvocation {
                session: Arc::clone(&session),
                turn: Arc::clone(&turn_context),
                tracker: Arc::clone(&turn_diff_tracker),
                call_id: "test-call-2".to_string(),
                tool_name: tool_name.to_string(),
                payload: ToolPayload::Function {
                    arguments: serde_json::json!({
                        "command": params2.command.clone(),
                        "workdir": Some(turn_context.cwd.to_string_lossy().to_string()),
                        "timeout_ms": params2.expiration.timeout_ms(),
                        "with_escalated_permissions": params2.with_escalated_permissions,
                        "justification": params2.justification.clone(),
                    })
                    .to_string(),
                },
            })
            .await;

        let output = match resp2.expect("expected Ok result") {
            ToolOutput::Function { content, .. } => content,
            _ => panic!("unexpected tool output"),
        };

        #[derive(Deserialize, PartialEq, Eq, Debug)]
        struct ResponseExecMetadata {
            exit_code: i32,
        }

        #[derive(Deserialize)]
        struct ResponseExecOutput {
            output: String,
            metadata: ResponseExecMetadata,
        }

        let exec_output: ResponseExecOutput =
            serde_json::from_str(&output).expect("valid exec output json");

        pretty_assertions::assert_eq!(exec_output.metadata, ResponseExecMetadata { exit_code: 0 });
        assert!(exec_output.output.contains("hi"));
    }
    #[tokio::test]
    async fn unified_exec_rejects_escalated_permissions_when_policy_not_on_request() {
        use crate::protocol::AskForApproval;
        use crate::turn_diff_tracker::TurnDiffTracker;

        let (session, mut turn_context_raw) = make_session_and_context();
        turn_context_raw.approval_policy = AskForApproval::OnFailure;
        let session = Arc::new(session);
        let turn_context = Arc::new(turn_context_raw);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));

        let handler = UnifiedExecHandler;
        let resp = handler
            .handle(ToolInvocation {
                session: Arc::clone(&session),
                turn: Arc::clone(&turn_context),
                tracker: Arc::clone(&tracker),
                call_id: "exec-call".to_string(),
                tool_name: "exec_command".to_string(),
                payload: ToolPayload::Function {
                    arguments: serde_json::json!({
                        "cmd": "echo hi",
                        "with_escalated_permissions": true,
                        "justification": "need unsandboxed execution",
                    })
                    .to_string(),
                },
            })
            .await;

        let Err(FunctionCallError::RespondToModel(output)) = resp else {
            panic!("expected error result");
        };

        let expected = format!(
            "approval policy is {policy:?}; reject command — you cannot ask for escalated permissions if the approval policy is {policy:?}",
            policy = turn_context.approval_policy
        );

        pretty_assertions::assert_eq!(output, expected);
    }
}
