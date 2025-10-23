use std::collections::HashMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use crate::AuthManager;
use crate::client_common::REVIEW_PROMPT;
use crate::function_tool::FunctionCallError;
use crate::mcp::auth::McpAuthStatusEntry;
use crate::parse_command::parse_command;
use crate::parse_turn_item;
use crate::response_processing::process_items;
use crate::review_format::format_review_findings_block;
use crate::terminal;
use crate::user_notification::UserNotifier;
use async_channel::Receiver;
use async_channel::Sender;
use codex_apply_patch::ApplyPatchAction;
use codex_protocol::ConversationId;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::ConversationPathResponseEvent;
use codex_protocol::protocol::ExitedReviewModeEvent;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ItemStartedEvent;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TaskStartedEvent;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnContextItem;
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
use serde_json;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::trace;
use tracing::warn;

use crate::ModelProviderInfo;
use crate::apply_patch::convert_apply_patch_to_protocol;
use crate::client::ModelClient;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::config::Config;
use crate::config_types::McpServerTransportConfig;
use crate::config_types::ShellEnvironmentPolicy;
use crate::conversation_history::ConversationHistory;
use crate::environment_context::EnvironmentContext;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
#[cfg(test)]
use crate::exec::StreamOutput;
// Removed: legacy executor wiring replaced by ToolOrchestrator flows.
// legacy normalize_exec_result no longer used after orchestrator migration
use crate::mcp::auth::compute_auth_statuses;
use crate::mcp_connection_manager::McpConnectionManager;
use crate::model_family::find_family_for_model;
use crate::openai_model_info::get_model_info;
use crate::project_doc::get_user_instructions;
use crate::protocol::AgentMessageDeltaEvent;
use crate::protocol::AgentReasoningDeltaEvent;
use crate::protocol::AgentReasoningRawContentDeltaEvent;
use crate::protocol::AgentReasoningSectionBreakEvent;
use crate::protocol::ApplyPatchApprovalRequestEvent;
use crate::protocol::AskForApproval;
use crate::protocol::BackgroundEventEvent;
use crate::protocol::ErrorEvent;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::ExecApprovalRequestEvent;
use crate::protocol::ListCustomPromptsResponseEvent;
use crate::protocol::Op;
use crate::protocol::RateLimitSnapshot;
use crate::protocol::ReviewDecision;
use crate::protocol::ReviewOutputEvent;
use crate::protocol::SandboxPolicy;
use crate::protocol::SessionConfiguredEvent;
use crate::protocol::StreamErrorEvent;
use crate::protocol::Submission;
use crate::protocol::TokenCountEvent;
use crate::protocol::TokenUsage;
use crate::protocol::TurnDiffEvent;
use crate::protocol::WebSearchBeginEvent;
use crate::rollout::RolloutRecorder;
use crate::rollout::RolloutRecorderParams;
use crate::shell;
use crate::state::ActiveTurn;
use crate::state::SessionServices;
use crate::state::SessionState;
use crate::state::TaskKind;
use crate::tasks::CompactTask;
use crate::tasks::RegularTask;
use crate::tasks::ReviewTask;
use crate::tools::ToolRouter;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::parallel::ToolCallRuntime;
use crate::tools::sandboxing::ApprovalStore;
use crate::tools::spec::ToolsConfig;
use crate::tools::spec::ToolsConfigParams;
use crate::turn_diff_tracker::TurnDiffTracker;
use crate::unified_exec::UnifiedExecSessionManager;
use crate::user_instructions::UserInstructions;
use crate::user_notification::UserNotification;
use crate::util::backoff;
use codex_async_utils::OrCancelExt;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::custom_prompts::CustomPrompt;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::user_input::UserInput;

pub mod compact;
use self::compact::build_compacted_history;
use self::compact::collect_user_messages;

/// The high-level interface to the Codex system.
/// It operates as a queue pair where you send submissions and receive events.
pub struct Codex {
    next_id: AtomicU64,
    tx_sub: Sender<Submission>,
    rx_event: Receiver<Event>,
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

        let config = Arc::new(config);

        let session_configuration = SessionConfiguration {
            provider: config.model_provider.clone(),
            model: config.model.clone(),
            model_reasoning_effort: config.model_reasoning_effort,
            model_reasoning_summary: config.model_reasoning_summary,
            user_instructions,
            base_instructions: config.base_instructions.clone(),
            approval_policy: config.approval_policy,
            sandbox_policy: config.sandbox_policy.clone(),
            cwd: config.cwd.clone(),
            original_config_do_not_use: Arc::clone(&config),
        };

        // Generate a unique ID for the lifetime of this Codex session.
        let session = Session::new(
            session_configuration,
            config.clone(),
            auth_manager.clone(),
            tx_event.clone(),
            conversation_history,
            session_source,
        )
        .await
        .map_err(|e| {
            error!("Failed to create session: {e:#}");
            CodexErr::InternalAgentDied
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
    pub(crate) base_instructions: Option<String>,
    pub(crate) user_instructions: Option<String>,
    pub(crate) approval_policy: AskForApproval,
    pub(crate) sandbox_policy: SandboxPolicy,
    pub(crate) shell_environment_policy: ShellEnvironmentPolicy,
    pub(crate) tools_config: ToolsConfig,
    pub(crate) is_review_mode: bool,
    pub(crate) final_output_json_schema: Option<Value>,
    pub(crate) codex_linux_sandbox_exe: Option<PathBuf>,
}

impl TurnContext {
    pub(crate) fn resolve_path(&self, path: Option<String>) -> PathBuf {
        path.as_ref()
            .map(PathBuf::from)
            .map_or_else(|| self.cwd.clone(), |p| self.cwd.join(p))
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

    /// Model instructions that are appended to the base instructions.
    user_instructions: Option<String>,

    /// Base instructions override.
    base_instructions: Option<String>,

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

    // TODO(pakrym): Remove config from here
    original_config_do_not_use: Arc<Config>,
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
            Arc::new(per_turn_config),
            auth_manager,
            otel_event_manager,
            provider,
            session_configuration.model_reasoning_effort,
            session_configuration.model_reasoning_summary,
            conversation_id,
        );

        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_family: &model_family,
            features: &config.features,
        });

        TurnContext {
            sub_id,
            client,
            cwd: session_configuration.cwd.clone(),
            base_instructions: session_configuration.base_instructions.clone(),
            user_instructions: session_configuration.user_instructions.clone(),
            approval_policy: session_configuration.approval_policy,
            sandbox_policy: session_configuration.sandbox_policy.clone(),
            shell_environment_policy: config.shell_environment_policy.clone(),
            tools_config,
            is_review_mode: false,
            final_output_json_schema: None,
            codex_linux_sandbox_exe: config.codex_linux_sandbox_exe.clone(),
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

        // Error messages to dispatch after SessionConfigured is sent.
        let mut post_session_configured_error_events = Vec::<Event>::new();

        // Kick off independent async setup tasks in parallel to reduce startup latency.
        //
        // - initialize RolloutRecorder with new or resumed session info
        // - spin up MCP connection manager
        // - perform default shell discovery
        // - load history metadata
        let rollout_fut = RolloutRecorder::new(&config, rollout_params);

        let mcp_fut = McpConnectionManager::new(
            config.mcp_servers.clone(),
            config.mcp_oauth_credentials_store_mode,
        );
        let default_shell_fut = shell::default_user_shell();
        let history_meta_fut = crate::message_history::history_metadata(&config);
        let auth_statuses_fut = compute_auth_statuses(
            config.mcp_servers.iter(),
            config.mcp_oauth_credentials_store_mode,
        );

        // Join all independent futures.
        let (
            rollout_recorder,
            mcp_res,
            default_shell,
            (history_log_id, history_entry_count),
            auth_statuses,
        ) = tokio::join!(
            rollout_fut,
            mcp_fut,
            default_shell_fut,
            history_meta_fut,
            auth_statuses_fut
        );

        let rollout_recorder = rollout_recorder.map_err(|e| {
            error!("failed to initialize rollout recorder: {e:#}");
            anyhow::anyhow!("failed to initialize rollout recorder: {e:#}")
        })?;
        let rollout_path = rollout_recorder.rollout_path.clone();

        // Handle MCP manager result and record any startup failures.
        let (mcp_connection_manager, failed_clients) = match mcp_res {
            Ok((mgr, failures)) => (mgr, failures),
            Err(e) => {
                let message = format!("Failed to create MCP connection manager: {e:#}");
                error!("{message}");
                post_session_configured_error_events.push(Event {
                    id: INITIAL_SUBMIT_ID.to_owned(),
                    msg: EventMsg::Error(ErrorEvent { message }),
                });
                (McpConnectionManager::default(), Default::default())
            }
        };

        // Surface individual client start-up failures to the user.
        if !failed_clients.is_empty() {
            for (server_name, err) in failed_clients {
                let auth_entry = auth_statuses.get(&server_name);
                let display_message = mcp_init_error_display(&server_name, auth_entry, &err);
                warn!("MCP client for `{server_name}` failed to start: {err:#}");
                post_session_configured_error_events.push(Event {
                    id: INITIAL_SUBMIT_ID.to_owned(),
                    msg: EventMsg::Error(ErrorEvent {
                        message: display_message,
                    }),
                });
            }
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
            config.model_max_output_tokens,
            config.model_auto_compact_token_limit,
            config.approval_policy,
            config.sandbox_policy.clone(),
            config.mcp_servers.keys().map(String::as_str).collect(),
            config.active_profile.clone(),
        );

        // Create the mutable state for the Session.
        let state = SessionState::new(session_configuration.clone());

        let services = SessionServices {
            mcp_connection_manager,
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
        sess.record_initial_history(initial_history).await;

        let events = std::iter::once(Event {
            id: INITIAL_SUBMIT_ID.to_owned(),
            msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
                session_id: conversation_id,
                model: session_configuration.model.clone(),
                reasoning_effort: session_configuration.model_reasoning_effort,
                history_log_id,
                history_entry_count,
                initial_messages,
                rollout_path,
            }),
        })
        .chain(post_session_configured_error_events.into_iter());
        for event in events {
            sess.send_event_raw(event).await;
        }

        Ok(sess)
    }

    pub(crate) fn get_tx_event(&self) -> Sender<Event> {
        self.tx_event.clone()
    }

    fn next_internal_sub_id(&self) -> String {
        let id = self
            .next_internal_sub_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("auto-compact-{id}")
    }

    async fn record_initial_history(&self, conversation_history: InitialHistory) {
        let turn_context = self.new_turn(SessionSettingsUpdate::default()).await;
        match conversation_history {
            InitialHistory::New => {
                // Build and record initial items (user instructions + environment context)
                let items = self.build_initial_context(&turn_context);
                self.record_conversation_items(&items).await;
            }
            InitialHistory::Resumed(_) | InitialHistory::Forked(_) => {
                let rollout_items = conversation_history.get_rollout_items();
                let persist = matches!(conversation_history, InitialHistory::Forked(_));

                // Always add response items to conversation history
                let reconstructed_history =
                    self.reconstruct_history_from_rollout(&turn_context, &rollout_items);
                if !reconstructed_history.is_empty() {
                    self.record_into_history(&reconstructed_history).await;
                }

                // If persisting, persist all rollout items as-is (recorder filters)
                if persist && !rollout_items.is_empty() {
                    self.persist_rollout_items(&rollout_items).await;
                }
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
        let event = Event {
            id: turn_context.sub_id.clone(),
            msg,
        };
        self.send_event_raw(event).await;
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

    async fn emit_turn_item_completed(
        &self,
        turn_context: &TurnContext,
        item: TurnItem,
        emit_raw_agent_reasoning: bool,
    ) {
        self.send_event(
            turn_context,
            EventMsg::ItemCompleted(ItemCompletedEvent {
                thread_id: self.conversation_id,
                turn_id: turn_context.sub_id.clone(),
                item: item.clone(),
            }),
        )
        .await;
        self.emit_turn_item_legacy_events(turn_context, &item, emit_raw_agent_reasoning)
            .await;
    }

    async fn emit_turn_item_started_completed(
        &self,
        turn_context: &TurnContext,
        item: TurnItem,
        emit_raw_agent_reasoning: bool,
    ) {
        self.emit_turn_item_started(turn_context, &item).await;
        self.emit_turn_item_completed(turn_context, item, emit_raw_agent_reasoning)
            .await;
    }

    async fn emit_turn_item_legacy_events(
        &self,
        turn_context: &TurnContext,
        item: &TurnItem,
        emit_raw_agent_reasoning: bool,
    ) {
        for event in item.as_legacy_events(emit_raw_agent_reasoning) {
            self.send_event(turn_context, event).await;
        }
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
            command,
            cwd,
            reason,
            parsed_cmd,
        });
        self.send_event(turn_context, event).await;
        rx_approve.await.unwrap_or_default()
    }

    pub async fn request_patch_approval(
        &self,
        turn_context: &TurnContext,
        call_id: String,
        action: &ApplyPatchAction,
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
            changes: convert_apply_patch_to_protocol(action),
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

    /// Records input items: always append to conversation history and
    /// persist these response items to rollout.
    pub(crate) async fn record_conversation_items(&self, items: &[ResponseItem]) {
        self.record_into_history(items).await;
        self.persist_rollout_response_items(items).await;
    }

    fn reconstruct_history_from_rollout(
        &self,
        turn_context: &TurnContext,
        rollout_items: &[RolloutItem],
    ) -> Vec<ResponseItem> {
        let mut history = ConversationHistory::new();
        for item in rollout_items {
            match item {
                RolloutItem::ResponseItem(response_item) => {
                    history.record_items(std::iter::once(response_item));
                }
                RolloutItem::Compacted(compacted) => {
                    let snapshot = history.get_history();
                    let user_messages = collect_user_messages(&snapshot);
                    let rebuilt = build_compacted_history(
                        self.build_initial_context(turn_context),
                        &user_messages,
                        &compacted.message,
                    );
                    history.replace(rebuilt);
                }
                _ => {}
            }
        }
        history.get_history()
    }

    /// Append ResponseItems to the in-memory conversation history only.
    async fn record_into_history(&self, items: &[ResponseItem]) {
        let mut state = self.state.lock().await;
        state.record_items(items.iter());
    }

    async fn replace_history(&self, items: Vec<ResponseItem>) {
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

    pub(crate) fn build_initial_context(&self, turn_context: &TurnContext) -> Vec<ResponseItem> {
        let mut items = Vec::<ResponseItem>::with_capacity(2);
        if let Some(user_instructions) = turn_context.user_instructions.as_deref() {
            items.push(UserInstructions::new(user_instructions.to_string()).into());
        }
        items.push(ResponseItem::from(EnvironmentContext::new(
            Some(turn_context.cwd.clone()),
            Some(turn_context.approval_policy),
            Some(turn_context.sandbox_policy.clone()),
            Some(self.user_shell().clone()),
        )));
        items
    }

    async fn persist_rollout_items(&self, items: &[RolloutItem]) {
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

    // todo (aibrahim): get rid of this method. we shouldn't deal with vec[resposne_item] and rather use ConversationHistory.
    pub(crate) async fn history_snapshot(&self) -> Vec<ResponseItem> {
        let mut state = self.state.lock().await;
        state.history_snapshot()
    }

    pub(crate) async fn clone_history(&self) -> ConversationHistory {
        let state = self.state.lock().await;
        state.clone_history()
    }

    async fn update_token_usage_info(
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

    async fn update_rate_limits(
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

    async fn set_total_tokens_full(&self, turn_context: &TurnContext) {
        let context_window = turn_context.client.get_model_context_window();
        if let Some(context_window) = context_window {
            {
                let mut state = self.state.lock().await;
                state.set_token_usage_full(context_window);
            }
            self.send_token_count_event(turn_context).await;
        }
    }

    /// Record a user input item to conversation history and also persist a
    /// corresponding UserMessage EventMsg to rollout.
    async fn record_input_and_rollout_usermsg(
        &self,
        turn_context: &TurnContext,
        response_input: &ResponseInputItem,
    ) {
        let response_item: ResponseItem = response_input.clone().into();
        // Add to conversation history and persist response item to rollout
        self.record_conversation_items(std::slice::from_ref(&response_item))
            .await;

        // Derive user message events and persist only UserMessage to rollout
        let turn_item = parse_turn_item(&response_item);

        if let Some(item @ TurnItem::UserMessage(_)) = turn_item {
            self.emit_turn_item_started_completed(turn_context, item, false)
                .await;
        }
    }

    /// Helper that emits a BackgroundEvent with the given message. This keeps
    /// the call‑sites terse so adding more diagnostics does not clutter the
    /// core agent logic.
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

    async fn notify_stream_error(&self, turn_context: &TurnContext, message: impl Into<String>) {
        let event = EventMsg::StreamError(StreamErrorEvent {
            message: message.into(),
        });
        self.send_event(turn_context, event).await;
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
            .call_tool(server, tool, arguments)
            .await
    }

    pub(crate) fn parse_mcp_tool_name(&self, tool_name: &str) -> Option<(String, String)> {
        self.services
            .mcp_connection_manager
            .parse_tool_name(tool_name)
    }

    pub async fn interrupt_task(self: &Arc<Self>) {
        info!("interrupt received: abort current task, if any");
        self.abort_all_tasks(TurnAbortReason::Interrupted).await;
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
}

async fn submission_loop(sess: Arc<Session>, config: Arc<Config>, rx_sub: Receiver<Submission>) {
    let mut previous_context: Option<Arc<TurnContext>> = None;
    // To break out of this loop, send Op::Shutdown.
    while let Ok(sub) = rx_sub.recv().await {
        debug!(?sub, "Submission");
        match sub.op {
            Op::Interrupt => {
                sess.interrupt_task().await;
            }
            Op::OverrideTurnContext {
                cwd,
                approval_policy,
                sandbox_policy,
                model,
                effort,
                summary,
            } => {
                let updates = SessionSettingsUpdate {
                    cwd,
                    approval_policy,
                    sandbox_policy,
                    model,
                    reasoning_effort: effort,
                    reasoning_summary: summary,
                    ..Default::default()
                };
                sess.update_settings(updates).await;
            }

            Op::UserInput { .. } | Op::UserTurn { .. } => {
                let (items, updates) = match sub.op {
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
                let current_context = sess.new_turn_with_sub_id(sub.id.clone(), updates).await;
                current_context
                    .client
                    .get_otel_event_manager()
                    .user_prompt(&items);
                // attempt to inject input into current task
                if let Err(items) = sess.inject_input(items).await {
                    if let Some(env_item) = sess
                        .build_environment_update_item(previous_context.as_ref(), &current_context)
                    {
                        sess.record_conversation_items(std::slice::from_ref(&env_item))
                            .await;
                    }

                    sess.spawn_task(Arc::clone(&current_context), items, RegularTask)
                        .await;
                    previous_context = Some(current_context);
                }
            }
            Op::ExecApproval { id, decision } => match decision {
                ReviewDecision::Abort => {
                    sess.interrupt_task().await;
                }
                other => sess.notify_approval(&id, other).await,
            },
            Op::PatchApproval { id, decision } => match decision {
                ReviewDecision::Abort => {
                    sess.interrupt_task().await;
                }
                other => sess.notify_approval(&id, other).await,
            },
            Op::AddToHistory { text } => {
                let id = sess.conversation_id;
                let config = config.clone();
                tokio::spawn(async move {
                    if let Err(e) = crate::message_history::append_entry(&text, &id, &config).await
                    {
                        warn!("failed to append to message history: {e}");
                    }
                });
            }

            Op::GetHistoryEntryRequest { offset, log_id } => {
                let config = config.clone();
                let sess_clone = sess.clone();
                let sub_id = sub.id.clone();

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
                                entry: entry_opt.map(|e| {
                                    codex_protocol::message_history::HistoryEntry {
                                        conversation_id: e.session_id,
                                        ts: e.ts,
                                        text: e.text,
                                    }
                                }),
                            },
                        ),
                    };

                    sess_clone.send_event_raw(event).await;
                });
            }
            Op::ListMcpTools => {
                let sub_id = sub.id.clone();

                // This is a cheap lookup from the connection manager's cache.
                let tools = sess.services.mcp_connection_manager.list_all_tools();
                let (auth_status_entries, resources, resource_templates) = tokio::join!(
                    compute_auth_statuses(
                        config.mcp_servers.iter(),
                        config.mcp_oauth_credentials_store_mode,
                    ),
                    sess.services.mcp_connection_manager.list_all_resources(),
                    sess.services
                        .mcp_connection_manager
                        .list_all_resource_templates()
                );
                let auth_statuses = auth_status_entries
                    .iter()
                    .map(|(name, entry)| (name.clone(), entry.auth_status))
                    .collect();
                let event = Event {
                    id: sub_id,
                    msg: EventMsg::McpListToolsResponse(
                        crate::protocol::McpListToolsResponseEvent {
                            tools,
                            resources,
                            resource_templates,
                            auth_statuses,
                        },
                    ),
                };
                sess.send_event_raw(event).await;
            }
            Op::ListCustomPrompts => {
                let sub_id = sub.id.clone();

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
            Op::Compact => {
                let turn_context = sess
                    .new_turn_with_sub_id(sub.id.clone(), SessionSettingsUpdate::default())
                    .await;
                // Attempt to inject input into current task
                if let Err(items) = sess
                    .inject_input(vec![UserInput::Text {
                        text: compact::SUMMARIZATION_PROMPT.to_string(),
                    }])
                    .await
                {
                    sess.spawn_task(Arc::clone(&turn_context), items, CompactTask)
                        .await;
                }
            }
            Op::Shutdown => {
                sess.abort_all_tasks(TurnAbortReason::Interrupted).await;
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
                        id: sub.id.clone(),
                        msg: EventMsg::Error(ErrorEvent {
                            message: "Failed to shutdown rollout recorder".to_string(),
                        }),
                    };
                    sess.send_event_raw(event).await;
                }

                let event = Event {
                    id: sub.id.clone(),
                    msg: EventMsg::ShutdownComplete,
                };
                sess.send_event_raw(event).await;
                break;
            }
            Op::GetPath => {
                let sub_id = sub.id.clone();
                // Flush rollout writes before returning the path so readers observe a consistent file.
                let (path, rec_opt) = {
                    let guard = sess.services.rollout.lock().await;
                    match guard.as_ref() {
                        Some(rec) => (rec.get_rollout_path(), Some(rec.clone())),
                        None => {
                            error!("rollout recorder not found");
                            continue;
                        }
                    }
                };
                if let Some(rec) = rec_opt
                    && let Err(e) = rec.flush().await
                {
                    warn!("failed to flush rollout recorder before GetHistory: {e}");
                }
                let event = Event {
                    id: sub_id.clone(),
                    msg: EventMsg::ConversationPath(ConversationPathResponseEvent {
                        conversation_id: sess.conversation_id,
                        path,
                    }),
                };
                sess.send_event_raw(event).await;
            }
            Op::Review { review_request } => {
                let turn_context = sess
                    .new_turn_with_sub_id(sub.id.clone(), SessionSettingsUpdate::default())
                    .await;
                spawn_review_thread(
                    sess.clone(),
                    config.clone(),
                    turn_context.clone(),
                    sub.id,
                    review_request,
                )
                .await;
            }
            _ => {
                // Ignore unknown ops; enum is non_exhaustive to allow extensions.
            }
        }
    }
    debug!("Agent loop exited");
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
    review_features.disable(crate::features::Feature::WebSearchRequest);
    review_features.disable(crate::features::Feature::ViewImageTool);
    review_features.disable(crate::features::Feature::StreamableShell);
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
    );

    let review_turn_context = TurnContext {
        sub_id: sub_id.to_string(),
        client,
        tools_config,
        user_instructions: None,
        base_instructions: Some(base_instructions.clone()),
        approval_policy: parent_turn_context.approval_policy,
        sandbox_policy: parent_turn_context.sandbox_policy.clone(),
        shell_environment_policy: parent_turn_context.shell_environment_policy.clone(),
        cwd: parent_turn_context.cwd.clone(),
        is_review_mode: true,
        final_output_json_schema: None,
        codex_linux_sandbox_exe: parent_turn_context.codex_linux_sandbox_exe.clone(),
    };

    // Seed the child task with the review prompt as the initial user message.
    let input: Vec<UserInput> = vec![UserInput::Text {
        text: review_prompt,
    }];
    let tc = Arc::new(review_turn_context);
    sess.spawn_task(tc.clone(), input, ReviewTask).await;

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
/// Review mode: when `turn_context.is_review_mode` is true, the turn runs in an
/// isolated in-memory thread without the parent session's prior history or
/// user_instructions. Emits ExitedReviewMode upon final review message.
pub(crate) async fn run_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    input: Vec<UserInput>,
    task_kind: TaskKind,
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
    // For review threads, keep an isolated in-memory history so the
    // model sees a fresh conversation without the parent session's history.
    // For normal turns, continue recording to the session history as before.
    let is_review_mode = turn_context.is_review_mode;

    let mut review_thread_history: ConversationHistory = ConversationHistory::new();
    if is_review_mode {
        // Seed review threads with environment context so the model knows the working directory.
        review_thread_history
            .record_items(sess.build_initial_context(turn_context.as_ref()).iter());
        review_thread_history.record_items(std::iter::once(&initial_input_for_turn.into()));
    } else {
        sess.record_input_and_rollout_usermsg(turn_context.as_ref(), &initial_input_for_turn)
            .await;
    }

    let mut last_agent_message: Option<String> = None;
    // Although from the perspective of codex.rs, TurnDiffTracker has the lifecycle of a Task which contains
    // many turns, from the perspective of the user, it is a single turn.
    let turn_diff_tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));
    let mut auto_compact_recently_attempted = false;

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
        //
        // - For review threads, use the isolated in-memory history so the
        //   model sees a fresh conversation (no parent history/user_instructions).
        //
        // - For normal turns, use the session's full history. When using the
        //   chat completions API (or ZDR clients), the model needs the full
        //   conversation history on each turn. The rollout file, however, should
        //   only record the new items that originated in this turn so that it
        //   represents an append-only log without duplicates.
        let turn_input: Vec<ResponseItem> = if is_review_mode {
            if !pending_input.is_empty() {
                review_thread_history.record_items(&pending_input);
            }
            review_thread_history.get_history()
        } else {
            sess.record_conversation_items(&pending_input).await;
            sess.history_snapshot().await
        };

        let turn_input_messages: Vec<String> = turn_input
            .iter()
            .filter_map(|item| match item {
                ResponseItem::Message { content, .. } => Some(content),
                _ => None,
            })
            .flat_map(|content| {
                content.iter().filter_map(|item| match item {
                    ContentItem::OutputText { text } => Some(text.clone()),
                    _ => None,
                })
            })
            .collect();
        match run_turn(
            Arc::clone(&sess),
            Arc::clone(&turn_context),
            Arc::clone(&turn_diff_tracker),
            turn_input,
            task_kind,
            cancellation_token.child_token(),
        )
        .await
        {
            Ok(turn_output) => {
                let TurnRunResult {
                    processed_items,
                    total_token_usage,
                } = turn_output;
                let limit = turn_context
                    .client
                    .get_auto_compact_token_limit()
                    .unwrap_or(i64::MAX);
                let total_usage_tokens = total_token_usage
                    .as_ref()
                    .map(TokenUsage::tokens_in_context_window);
                let token_limit_reached = total_usage_tokens
                    .map(|tokens| tokens >= limit)
                    .unwrap_or(false);
                let (responses, items_to_record_in_conversation_history) = process_items(
                    processed_items,
                    is_review_mode,
                    &mut review_thread_history,
                    &sess,
                )
                .await;

                if token_limit_reached {
                    if auto_compact_recently_attempted {
                        let limit_str = limit.to_string();
                        let current_tokens = total_usage_tokens
                            .map(|tokens| tokens.to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        let event = EventMsg::Error(ErrorEvent {
                            message: format!(
                                "Conversation is still above the token limit after automatic summarization (limit {limit_str}, current {current_tokens}). Please start a new session or trim your input."
                            ),
                        });
                        sess.send_event(&turn_context, event).await;
                        break;
                    }
                    auto_compact_recently_attempted = true;
                    compact::run_inline_auto_compact_task(sess.clone(), turn_context.clone()).await;
                    continue;
                }

                auto_compact_recently_attempted = false;

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
                let _ = process_items(
                    processed_items,
                    is_review_mode,
                    &mut review_thread_history,
                    &sess,
                )
                .await;
                // Aborted turn is reported via a different event.
                break;
            }
            Err(e) => {
                info!("Turn error: {e:#}");
                let event = EventMsg::Error(ErrorEvent {
                    message: e.to_string(),
                });
                sess.send_event(&turn_context, event).await;
                // let the user continue the conversation
                break;
            }
        }
    }

    // If this was a review thread and we have a final assistant message,
    // try to parse it as a ReviewOutput.
    //
    // If parsing fails, construct a minimal ReviewOutputEvent using the plain
    // text as the overall explanation. Else, just exit review mode with None.
    //
    // Emits an ExitedReviewMode event with the parsed review output.
    if turn_context.is_review_mode {
        exit_review_mode(
            sess.clone(),
            Arc::clone(&turn_context),
            last_agent_message.as_deref().map(parse_review_output_event),
        )
        .await;
    }

    last_agent_message
}

/// Parse the review output; when not valid JSON, build a structured
/// fallback that carries the plain text as the overall explanation.
///
/// Returns: a ReviewOutputEvent parsed from JSON or a fallback populated from text.
fn parse_review_output_event(text: &str) -> ReviewOutputEvent {
    // Try direct parse first
    if let Ok(ev) = serde_json::from_str::<ReviewOutputEvent>(text) {
        return ev;
    }
    // If wrapped in markdown fences or extra prose, attempt to extract the first JSON object
    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}'))
        && start < end
        && let Some(slice) = text.get(start..=end)
        && let Ok(ev) = serde_json::from_str::<ReviewOutputEvent>(slice)
    {
        return ev;
    }
    // Not JSON – return a structured ReviewOutputEvent that carries
    // the plain text as the overall explanation.
    ReviewOutputEvent {
        overall_explanation: text.to_string(),
        ..Default::default()
    }
}

async fn run_turn(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    turn_diff_tracker: SharedTurnDiffTracker,
    input: Vec<ResponseItem>,
    task_kind: TaskKind,
    cancellation_token: CancellationToken,
) -> CodexResult<TurnRunResult> {
    let mcp_tools = sess.services.mcp_connection_manager.list_all_tools();
    let router = Arc::new(ToolRouter::from_config(
        &turn_context.tools_config,
        Some(mcp_tools),
    ));

    let model_supports_parallel = turn_context
        .client
        .get_model_family()
        .supports_parallel_tool_calls;
    let parallel_tool_calls = model_supports_parallel;
    let prompt = Prompt {
        input,
        tools: router.specs(),
        parallel_tool_calls,
        base_instructions_override: turn_context.base_instructions.clone(),
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
            task_kind,
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
                sess.set_total_tokens_full(turn_context.as_ref()).await;
                return Err(e);
            }
            Err(CodexErr::UsageLimitReached(e)) => {
                let rate_limits = e.rate_limits.clone();
                if let Some(rate_limits) = rate_limits {
                    sess.update_rate_limits(turn_context.as_ref(), rate_limits)
                        .await;
                }
                return Err(CodexErr::UsageLimitReached(e));
            }
            Err(CodexErr::UsageNotIncluded) => return Err(CodexErr::UsageNotIncluded),
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
                        turn_context.as_ref(),
                        format!("Re-connecting... {retries}/{max_retries}"),
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

#[derive(Debug)]
struct TurnRunResult {
    processed_items: Vec<ProcessedResponseItem>,
    total_token_usage: Option<TokenUsage>,
}

#[allow(clippy::too_many_arguments)]
async fn try_run_turn(
    router: Arc<ToolRouter>,
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    turn_diff_tracker: SharedTurnDiffTracker,
    prompt: &Prompt,
    task_kind: TaskKind,
    cancellation_token: CancellationToken,
) -> CodexResult<TurnRunResult> {
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
        .stream_with_task_kind(prompt, task_kind)
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
                match ToolRouter::build_tool_call(sess.as_ref(), item.clone()) {
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
                        let response = handle_non_tool_response_item(
                            sess.as_ref(),
                            Arc::clone(&turn_context),
                            item.clone(),
                            sess.show_raw_agent_reasoning(),
                        )
                        .await?;
                        add_completed(ProcessedResponseItem { item, response });
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
                                success: None,
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
                                success: None,
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
            ResponseEvent::WebSearchCallBegin { call_id } => {
                let _ = sess
                    .tx_event
                    .send(Event {
                        id: turn_context.sub_id.clone(),
                        msg: EventMsg::WebSearchBegin(WebSearchBeginEvent { call_id }),
                    })
                    .await;
            }
            ResponseEvent::RateLimits(snapshot) => {
                // Update internal state with latest rate limits, but defer sending until
                // token usage is available to avoid duplicate TokenCount events.
                sess.update_rate_limits(turn_context.as_ref(), snapshot)
                    .await;
            }
            ResponseEvent::Completed {
                response_id: _,
                token_usage,
            } => {
                sess.update_token_usage_info(turn_context.as_ref(), token_usage.as_ref())
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

                let result = TurnRunResult {
                    processed_items,
                    total_token_usage: token_usage.clone(),
                };

                return Ok(result);
            }
            ResponseEvent::OutputTextDelta(delta) => {
                // In review child threads, suppress assistant text deltas; the
                // UI will show a selection popup from the final ReviewOutput.
                if !turn_context.is_review_mode {
                    let event = EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta });
                    sess.send_event(&turn_context, event).await;
                } else {
                    trace!("suppressing OutputTextDelta in review mode");
                }
            }
            ResponseEvent::ReasoningSummaryDelta(delta) => {
                let event = EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent { delta });
                sess.send_event(&turn_context, event).await;
            }
            ResponseEvent::ReasoningSummaryPartAdded => {
                let event =
                    EventMsg::AgentReasoningSectionBreak(AgentReasoningSectionBreakEvent {});
                sess.send_event(&turn_context, event).await;
            }
            ResponseEvent::ReasoningContentDelta(delta) => {
                if sess.show_raw_agent_reasoning() {
                    let event = EventMsg::AgentReasoningRawContentDelta(
                        AgentReasoningRawContentDeltaEvent { delta },
                    );
                    sess.send_event(&turn_context, event).await;
                }
            }
        }
    }
}

async fn handle_non_tool_response_item(
    sess: &Session,
    turn_context: Arc<TurnContext>,
    item: ResponseItem,
    show_raw_agent_reasoning: bool,
) -> CodexResult<Option<ResponseInputItem>> {
    debug!(?item, "Output item");

    match &item {
        ResponseItem::Message { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::WebSearchCall { .. } => {
            let turn_item = match &item {
                ResponseItem::Message { .. } if turn_context.is_review_mode => {
                    trace!("suppressing assistant Message in review mode");
                    None
                }
                _ => parse_turn_item(&item),
            };
            if let Some(turn_item) = turn_item {
                sess.emit_turn_item_started_completed(
                    turn_context.as_ref(),
                    turn_item,
                    show_raw_agent_reasoning,
                )
                .await;
            }
        }
        ResponseItem::FunctionCallOutput { .. } | ResponseItem::CustomToolCallOutput { .. } => {
            debug!("unexpected tool output from stream");
        }
        _ => {}
    }

    Ok(None)
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
pub(crate) fn convert_call_tool_result_to_function_call_output_payload(
    call_tool_result: &CallToolResult,
) -> FunctionCallOutputPayload {
    let CallToolResult {
        content,
        is_error,
        structured_content,
    } = call_tool_result;

    // In terms of what to send back to the model, we prefer structured_content,
    // if available, and fallback to content, otherwise.
    let mut is_success = is_error != &Some(true);
    let content = if let Some(structured_content) = structured_content
        && structured_content != &serde_json::Value::Null
        && let Ok(serialized_structured_content) = serde_json::to_string(&structured_content)
    {
        serialized_structured_content
    } else {
        match serde_json::to_string(&content) {
            Ok(serialized_content) => serialized_content,
            Err(err) => {
                // If we could not serialize either content or structured_content to
                // JSON, flag this as an error.
                is_success = false;
                err.to_string()
            }
        }
    };

    FunctionCallOutputPayload {
        content,
        success: Some(is_success),
    }
}

/// Emits an ExitedReviewMode Event with optional ReviewOutput,
/// and records a developer message with the review output.
pub(crate) async fn exit_review_mode(
    session: Arc<Session>,
    turn_context: Arc<TurnContext>,
    review_output: Option<ReviewOutputEvent>,
) {
    let event = EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
        review_output: review_output.clone(),
    });
    session.send_event(turn_context.as_ref(), event).await;

    let mut user_message = String::new();
    if let Some(out) = review_output {
        let mut findings_str = String::new();
        let text = out.overall_explanation.trim();
        if !text.is_empty() {
            findings_str.push_str(text);
        }
        if !out.findings.is_empty() {
            let block = format_review_findings_block(&out.findings, None);
            findings_str.push_str(&format!("\n{block}"));
        }
        user_message.push_str(&format!(
            r#"<user_action>
  <context>User initiated a review task. Here's the full review output from reviewer model. User may select one or more comments to resolve.</context>
  <action>review</action>
  <results>
  {findings_str}
  </results>
</user_action>
"#));
    } else {
        user_message.push_str(r#"<user_action>
  <context>User initiated a review task, but was interrupted. If user asks about this, tell them to re-initiate a review with `/review` and wait for it to complete.</context>
  <action>review</action>
  <results>
  None.
  </results>
</user_action>
"#);
    }

    session
        .record_conversation_items(&[ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text: user_message }],
        }])
        .await;
}

fn mcp_init_error_display(
    server_name: &str,
    entry: Option<&McpAuthStatusEntry>,
    err: &anyhow::Error,
) -> String {
    if let Some(McpServerTransportConfig::StreamableHttp {
        url,
        bearer_token_env_var,
        http_headers,
        ..
    }) = &entry.map(|entry| &entry.config.transport)
        && url == "https://api.githubcopilot.com/mcp/"
        && bearer_token_env_var.is_none()
        && http_headers.as_ref().map(HashMap::is_empty).unwrap_or(true)
    {
        // GitHub only supports OAUth for first party MCP clients.
        // That means that the user has to specify a personal access token either via bearer_token_env_var or http_headers.
        // https://github.com/github/github-mcp-server/issues/921#issuecomment-3221026448
        format!(
            "GitHub MCP does not support OAuth. Log in by adding `bearer_token_env_var = CODEX_GITHUB_PAT` in the `mcp_servers.{server_name}` section of your config.toml"
        )
    } else if is_mcp_client_auth_required_error(err) {
        format!(
            "The {server_name} MCP server is not logged in. Run `codex mcp login {server_name}`."
        )
    } else {
        format!("MCP client for `{server_name}` failed to start: {err:#}")
    }
}

fn is_mcp_client_auth_required_error(error: &anyhow::Error) -> bool {
    // StreamableHttpError::AuthRequired from the MCP SDK.
    error.to_string().contains("Auth required")
}

#[cfg(test)]
pub(crate) use tests::make_session_and_context;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigOverrides;
    use crate::config::ConfigToml;
    use crate::config_types::McpServerConfig;
    use crate::config_types::McpServerTransportConfig;
    use crate::exec::ExecToolCallOutput;
    use crate::mcp::auth::McpAuthStatusEntry;
    use crate::tools::format_exec_output_str;

    use crate::protocol::CompactedItem;
    use crate::protocol::InitialHistory;
    use crate::protocol::ResumedHistory;
    use crate::state::TaskKind;
    use crate::tasks::SessionTask;
    use crate::tasks::SessionTaskContext;
    use crate::tools::MODEL_FORMAT_HEAD_LINES;
    use crate::tools::MODEL_FORMAT_MAX_BYTES;
    use crate::tools::MODEL_FORMAT_MAX_LINES;
    use crate::tools::MODEL_FORMAT_TAIL_LINES;
    use crate::tools::ToolRouter;
    use crate::tools::context::ToolInvocation;
    use crate::tools::context::ToolOutput;
    use crate::tools::context::ToolPayload;
    use crate::tools::handlers::ShellHandler;
    use crate::tools::registry::ToolHandler;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use codex_app_server_protocol::AuthMode;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::protocol::McpAuthStatus;
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

        let actual = tokio_test::block_on(async { session.state.lock().await.history_snapshot() });
        assert_eq!(expected, actual);
    }

    #[test]
    fn record_initial_history_reconstructs_forked_transcript() {
        let (session, turn_context) = make_session_and_context();
        let (rollout_items, expected) = sample_rollout(&session, &turn_context);

        tokio_test::block_on(session.record_initial_history(InitialHistory::Forked(rollout_items)));

        let actual = tokio_test::block_on(async { session.state.lock().await.history_snapshot() });
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

        let got = convert_call_tool_result_to_function_call_output_payload(&ctr);
        let expected = FunctionCallOutputPayload {
            content: serde_json::to_string(&json!({
                "ok": true,
                "value": 42
            }))
            .unwrap(),
            success: Some(true),
        };

        assert_eq!(expected, got);
    }

    #[test]
    fn model_truncation_head_tail_by_lines() {
        // Build 400 short lines so line-count limit, not byte budget, triggers truncation
        let lines: Vec<String> = (1..=400).map(|i| format!("line{i}")).collect();
        let full = lines.join("\n");

        let exec = ExecToolCallOutput {
            exit_code: 0,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new(full),
            duration: StdDuration::from_secs(1),
            timed_out: false,
        };

        let out = format_exec_output_str(&exec);

        // Strip truncation header if present for subsequent assertions
        let body = out
            .strip_prefix("Total output lines: ")
            .and_then(|rest| rest.split_once("\n\n").map(|x| x.1))
            .unwrap_or(out.as_str());

        // Expect elision marker with correct counts
        let omitted = 400 - MODEL_FORMAT_MAX_LINES; // 144
        let marker = format!("\n[... omitted {omitted} of 400 lines ...]\n\n");
        assert!(out.contains(&marker), "missing marker: {out}");

        // Validate head and tail
        let parts: Vec<&str> = body.split(&marker).collect();
        assert_eq!(parts.len(), 2, "expected one marker split");
        let head = parts[0];
        let tail = parts[1];

        let expected_head: String = (1..=MODEL_FORMAT_HEAD_LINES)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(head.starts_with(&expected_head), "head mismatch");

        let expected_tail: String = ((400 - MODEL_FORMAT_TAIL_LINES + 1)..=400)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(tail.ends_with(&expected_tail), "tail mismatch");
    }

    #[test]
    fn model_truncation_respects_byte_budget() {
        // Construct a large output (about 100kB) so byte budget dominates
        let big_line = "x".repeat(100);
        let full = std::iter::repeat_n(big_line, 1000)
            .collect::<Vec<_>>()
            .join("\n");

        let exec = ExecToolCallOutput {
            exit_code: 0,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new(full.clone()),
            duration: StdDuration::from_secs(1),
            timed_out: false,
        };

        let out = format_exec_output_str(&exec);
        // Keep strict budget on the truncated body (excluding header)
        let body = out
            .strip_prefix("Total output lines: ")
            .and_then(|rest| rest.split_once("\n\n").map(|x| x.1))
            .unwrap_or(out.as_str());
        assert!(body.len() <= MODEL_FORMAT_MAX_BYTES, "exceeds byte budget");
        assert!(out.contains("omitted"), "should contain elision marker");

        // Ensure head and tail are drawn from the original
        assert!(full.starts_with(body.chars().take(8).collect::<String>().as_str()));
        assert!(
            full.ends_with(
                body.chars()
                    .rev()
                    .take(8)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>()
                    .as_str()
            )
        );
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

        let out = format_exec_output_str(&exec);

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

        let got = convert_call_tool_result_to_function_call_output_payload(&ctr);
        let expected = FunctionCallOutputPayload {
            content: serde_json::to_string(&vec![text_block("hello"), text_block("world")])
                .unwrap(),
            success: Some(true),
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

        let got = convert_call_tool_result_to_function_call_output_payload(&ctr);
        let expected = FunctionCallOutputPayload {
            content: serde_json::to_string(&json!({ "message": "bad" })).unwrap(),
            success: Some(false),
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

        let got = convert_call_tool_result_to_function_call_output_payload(&ctr);
        let expected = FunctionCallOutputPayload {
            content: serde_json::to_string(&vec![text_block("alpha")]).unwrap(),
            success: Some(true),
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
        let auth_manager = AuthManager::shared(config.cwd.clone(), false);

        let session_configuration = SessionConfiguration {
            provider: config.model_provider.clone(),
            model: config.model.clone(),
            model_reasoning_effort: config.model_reasoning_effort,
            model_reasoning_summary: config.model_reasoning_summary,
            user_instructions: config.user_instructions.clone(),
            base_instructions: config.base_instructions.clone(),
            approval_policy: config.approval_policy,
            sandbox_policy: config.sandbox_policy.clone(),
            cwd: config.cwd.clone(),
            original_config_do_not_use: Arc::clone(&config),
        };

        let state = SessionState::new(session_configuration.clone());

        let services = SessionServices {
            mcp_connection_manager: McpConnectionManager::default(),
            unified_exec_manager: UnifiedExecSessionManager::default(),
            notifier: UserNotifier::new(None),
            rollout: Mutex::new(None),
            user_shell: shell::Shell::Unknown,
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
    fn make_session_and_context_with_rx() -> (
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
        let auth_manager = AuthManager::shared(config.cwd.clone(), false);

        let session_configuration = SessionConfiguration {
            provider: config.model_provider.clone(),
            model: config.model.clone(),
            model_reasoning_effort: config.model_reasoning_effort,
            model_reasoning_summary: config.model_reasoning_summary,
            user_instructions: config.user_instructions.clone(),
            base_instructions: config.base_instructions.clone(),
            approval_policy: config.approval_policy,
            sandbox_policy: config.sandbox_policy.clone(),
            cwd: config.cwd.clone(),
            original_config_do_not_use: Arc::clone(&config),
        };

        let state = SessionState::new(session_configuration.clone());

        let services = SessionServices {
            mcp_connection_manager: McpConnectionManager::default(),
            unified_exec_manager: UnifiedExecSessionManager::default(),
            notifier: UserNotifier::new(None),
            rollout: Mutex::new(None),
            user_shell: shell::Shell::Unknown,
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

        async fn abort(&self, session: Arc<SessionTaskContext>, ctx: Arc<TurnContext>) {
            if let TaskKind::Review = self.kind {
                exit_review_mode(session.clone_session(), ctx, None).await;
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
        sess.spawn_task(
            Arc::clone(&tc),
            input,
            NeverEndingTask {
                kind: TaskKind::Review,
                listen_to_cancellation_token: false,
            },
        )
        .await;

        sess.abort_all_tasks(TurnAbortReason::Interrupted).await;

        let first = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout waiting for first event")
            .expect("first event");
        match first.msg {
            EventMsg::ExitedReviewMode(ev) => assert!(ev.review_output.is_none()),
            other => panic!("unexpected first event: {other:?}"),
        }
        let second = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout waiting for second event")
            .expect("second event");
        match second.msg {
            EventMsg::TurnAborted(e) => assert_eq!(TurnAbortReason::Interrupted, e.reason),
            other => panic!("unexpected second event: {other:?}"),
        }

        let history = sess.history_snapshot().await;
        let found = history.iter().any(|item| match item {
            ResponseItem::Message { role, content, .. } if role == "user" => {
                content.iter().any(|ci| match ci {
                    ContentItem::InputText { text } => {
                        text.contains("<user_action>")
                            && text.contains("review")
                            && text.contains("interrupted")
                    }
                    _ => false,
                })
            }
            _ => false,
        });
        assert!(
            found,
            "synthetic review interruption not recorded in history"
        );
    }

    #[tokio::test]
    async fn fatal_tool_error_stops_turn_and_reports_error() {
        let (session, turn_context, _rx) = make_session_and_context_with_rx();
        let router = ToolRouter::from_config(
            &turn_context.tools_config,
            Some(session.services.mcp_connection_manager.list_all_tools()),
        );
        let item = ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "call-1".to_string(),
            name: "shell".to_string(),
            input: "{}".to_string(),
        };

        let call = ToolRouter::build_tool_call(session.as_ref(), item.clone())
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
        let mut live_history = ConversationHistory::new();

        let initial_context = session.build_initial_context(turn_context);
        for item in &initial_context {
            rollout_items.push(RolloutItem::ResponseItem(item.clone()));
        }
        live_history.record_items(initial_context.iter());

        let user1 = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "first user".to_string(),
            }],
        };
        live_history.record_items(std::iter::once(&user1));
        rollout_items.push(RolloutItem::ResponseItem(user1.clone()));

        let assistant1 = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "assistant reply one".to_string(),
            }],
        };
        live_history.record_items(std::iter::once(&assistant1));
        rollout_items.push(RolloutItem::ResponseItem(assistant1.clone()));

        let summary1 = "summary one";
        let snapshot1 = live_history.get_history();
        let user_messages1 = collect_user_messages(&snapshot1);
        let rebuilt1 = build_compacted_history(
            session.build_initial_context(turn_context),
            &user_messages1,
            summary1,
        );
        live_history.replace(rebuilt1);
        rollout_items.push(RolloutItem::Compacted(CompactedItem {
            message: summary1.to_string(),
        }));

        let user2 = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "second user".to_string(),
            }],
        };
        live_history.record_items(std::iter::once(&user2));
        rollout_items.push(RolloutItem::ResponseItem(user2.clone()));

        let assistant2 = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "assistant reply two".to_string(),
            }],
        };
        live_history.record_items(std::iter::once(&assistant2));
        rollout_items.push(RolloutItem::ResponseItem(assistant2.clone()));

        let summary2 = "summary two";
        let snapshot2 = live_history.get_history();
        let user_messages2 = collect_user_messages(&snapshot2);
        let rebuilt2 = build_compacted_history(
            session.build_initial_context(turn_context),
            &user_messages2,
            summary2,
        );
        live_history.replace(rebuilt2);
        rollout_items.push(RolloutItem::Compacted(CompactedItem {
            message: summary2.to_string(),
        }));

        let user3 = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "third user".to_string(),
            }],
        };
        live_history.record_items(std::iter::once(&user3));
        rollout_items.push(RolloutItem::ResponseItem(user3.clone()));

        let assistant3 = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "assistant reply three".to_string(),
            }],
        };
        live_history.record_items(std::iter::once(&assistant3));
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
            timeout_ms: Some(1000),
            env: HashMap::new(),
            with_escalated_permissions: Some(true),
            justification: Some("test".to_string()),
            arg0: None,
        };

        let params2 = ExecParams {
            with_escalated_permissions: Some(false),
            ..params.clone()
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
                        "timeout_ms": params.timeout_ms,
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
                        "timeout_ms": params2.timeout_ms,
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

    #[test]
    fn mcp_init_error_display_prompts_for_github_pat() {
        let server_name = "github";
        let entry = McpAuthStatusEntry {
            config: McpServerConfig {
                transport: McpServerTransportConfig::StreamableHttp {
                    url: "https://api.githubcopilot.com/mcp/".to_string(),
                    bearer_token_env_var: None,
                    http_headers: None,
                    env_http_headers: None,
                },
                enabled: true,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
            },
            auth_status: McpAuthStatus::Unsupported,
        };
        let err = anyhow::anyhow!("OAuth is unsupported");

        let display = mcp_init_error_display(server_name, Some(&entry), &err);

        let expected = format!(
            "GitHub MCP does not support OAuth. Log in by adding `bearer_token_env_var = CODEX_GITHUB_PAT` in the `mcp_servers.{server_name}` section of your config.toml"
        );

        assert_eq!(expected, display);
    }

    #[test]
    fn mcp_init_error_display_prompts_for_login_when_auth_required() {
        let server_name = "example";
        let err = anyhow::anyhow!("Auth required for server");

        let display = mcp_init_error_display(server_name, None, &err);

        let expected = format!(
            "The {server_name} MCP server is not logged in. Run `codex mcp login {server_name}`."
        );

        assert_eq!(expected, display);
    }

    #[test]
    fn mcp_init_error_display_reports_generic_errors() {
        let server_name = "custom";
        let entry = McpAuthStatusEntry {
            config: McpServerConfig {
                transport: McpServerTransportConfig::StreamableHttp {
                    url: "https://example.com".to_string(),
                    bearer_token_env_var: Some("TOKEN".to_string()),
                    http_headers: None,
                    env_http_headers: None,
                },
                enabled: true,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
            },
            auth_status: McpAuthStatus::Unsupported,
        };
        let err = anyhow::anyhow!("boom");

        let display = mcp_init_error_display(server_name, Some(&entry), &err);

        let expected = format!("MCP client for `{server_name}` failed to start: {err:#}");

        assert_eq!(expected, display);
    }
}
