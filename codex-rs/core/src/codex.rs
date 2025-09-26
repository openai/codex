use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use crate::AuthManager;
use crate::client_common::REVIEW_PROMPT;
use crate::event_mapping::map_response_item_to_event_messages;
use crate::function_tool::FunctionCallError;
use crate::review_format::format_review_findings_block;
use crate::user_notification::UserNotifier;
use async_channel::Receiver;
use async_channel::Sender;
use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::MaybeApplyPatchVerified;
use codex_apply_patch::maybe_parse_apply_patch_verified;
use codex_protocol::mcp_protocol::ConversationId;
use codex_protocol::protocol::ConversationPathResponseEvent;
use codex_protocol::protocol::ExitedReviewModeEvent;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::TaskStartedEvent;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnContextItem;
use futures::prelude::*;
use mcp_types::CallToolResult;
use serde::Deserialize;
use serde::Serialize;
use serde_json;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::trace;
use tracing::warn;

use crate::ModelProviderInfo;
use crate::apply_patch;
use crate::apply_patch::ApplyPatchExec;
use crate::apply_patch::CODEX_APPLY_PATCH_ARG1;
use crate::apply_patch::InternalApplyPatchInvocation;
use crate::apply_patch::convert_apply_patch_to_protocol;
use crate::client::ModelClient;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::config::Config;
use crate::config_types::ShellEnvironmentPolicy;
use crate::conversation_history::ConversationHistory;
use crate::environment_context::EnvironmentContext;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::error::SandboxErr;
use crate::error::get_error_message_ui;
use crate::exec::ExecParams;
use crate::exec::ExecToolCallOutput;
use crate::exec::SandboxType;
use crate::exec::StdoutStream;
use crate::exec::StreamOutput;
use crate::exec::process_exec_tool_call;
use crate::exec_command::EXEC_COMMAND_TOOL_NAME;
use crate::exec_command::ExecCommandParams;
use crate::exec_command::ExecSessionManager;
use crate::exec_command::WRITE_STDIN_TOOL_NAME;
use crate::exec_command::WriteStdinParams;
use crate::exec_env::create_env;
use crate::mcp_connection_manager::McpConnectionManager;
use crate::mcp_tool_call::handle_mcp_tool_call;
use crate::model_family::find_family_for_model;
use crate::openai_model_info::get_model_info;
use crate::openai_tools::ApplyPatchToolArgs;
use crate::openai_tools::ToolsConfig;
use crate::openai_tools::ToolsConfigParams;
use crate::openai_tools::get_openai_tools;
use crate::parse_command::parse_command;
use crate::plan_tool::handle_update_plan;
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
use crate::protocol::ExecCommandBeginEvent;
use crate::protocol::ExecCommandEndEvent;
use crate::protocol::FileChange;
use crate::protocol::InputItem;
use crate::protocol::ListCustomPromptsResponseEvent;
use crate::protocol::Op;
use crate::protocol::PatchApplyBeginEvent;
use crate::protocol::PatchApplyEndEvent;
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
use crate::safety::SafetyCheck;
use crate::safety::assess_command_safety;
use crate::safety::assess_safety_for_untrusted_command;
use crate::shell;
use crate::state::ActiveTurn;
use crate::state::SessionServices;
use crate::tasks::CompactTask;
use crate::tasks::RegularTask;
use crate::tasks::ReviewTask;
use crate::turn_diff_tracker::TurnDiffTracker;
use crate::unified_exec::UnifiedExecSessionManager;
use crate::user_instructions::UserInstructions;
use crate::user_notification::UserNotification;
use crate::util::backoff;
use codex_protocol::config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::custom_prompts::CustomPrompt;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::ShellToolCallParams;
use codex_protocol::protocol::InitialHistory;

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

// Model-formatting limits: clients get full streams; oonly content sent to the model is truncated.
pub(crate) const MODEL_FORMAT_MAX_BYTES: usize = 10 * 1024; // 10 KiB
pub(crate) const MODEL_FORMAT_MAX_LINES: usize = 256; // lines
pub(crate) const MODEL_FORMAT_HEAD_LINES: usize = MODEL_FORMAT_MAX_LINES / 2;
pub(crate) const MODEL_FORMAT_TAIL_LINES: usize = MODEL_FORMAT_MAX_LINES - MODEL_FORMAT_HEAD_LINES; // 128
pub(crate) const MODEL_FORMAT_HEAD_BYTES: usize = MODEL_FORMAT_MAX_BYTES / 2;

impl Codex {
    /// Spawn a new [`Codex`] and initialize the session.
    pub async fn spawn(
        config: Config,
        auth_manager: Arc<AuthManager>,
        conversation_history: InitialHistory,
    ) -> CodexResult<CodexSpawnOk> {
        let (tx_sub, rx_sub) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);
        let (tx_event, rx_event) = async_channel::unbounded();

        let user_instructions = get_user_instructions(&config).await;

        let config = Arc::new(config);

        let configure_session = ConfigureSession {
            provider: config.model_provider.clone(),
            model: config.model.clone(),
            model_reasoning_effort: config.model_reasoning_effort,
            model_reasoning_summary: config.model_reasoning_summary,
            user_instructions,
            base_instructions: config.base_instructions.clone(),
            approval_policy: config.approval_policy,
            sandbox_policy: config.sandbox_policy.clone(),
            notify: UserNotifier::new(config.notify.clone()),
            cwd: config.cwd.clone(),
        };

        // Generate a unique ID for the lifetime of this Codex session.
        let (session, turn_context) = Session::new(
            configure_session,
            config.clone(),
            auth_manager.clone(),
            tx_event.clone(),
            conversation_history,
        )
        .await
        .map_err(|e| {
            error!("Failed to create session: {e:#}");
            CodexErr::InternalAgentDied
        })?;
        let conversation_id = session.conversation_id;

        // This task will run until Op::Shutdown is received.
        tokio::spawn(submission_loop(session, turn_context, config, rx_sub));
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

use crate::state::SessionState;

/// Context for an initialized model agent
///
/// A session has at most 1 running task at a time, and can be interrupted by user input.
pub(crate) struct Session {
    conversation_id: ConversationId,
    tx_event: Sender<Event>,
    state: Mutex<SessionState>,
    pub(crate) active_turn: Mutex<Option<ActiveTurn>>,
    services: SessionServices,
    next_internal_sub_id: AtomicU64,
}

/// The context needed for a single turn of the conversation.
#[derive(Debug)]
pub(crate) struct TurnContext {
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
}

impl TurnContext {
    fn resolve_path(&self, path: Option<String>) -> PathBuf {
        path.as_ref()
            .map(PathBuf::from)
            .map_or_else(|| self.cwd.clone(), |p| self.cwd.join(p))
    }
}

/// Configure the model session.
struct ConfigureSession {
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

    notify: UserNotifier,

    /// Working directory that should be treated as the *root* of the
    /// session. All relative paths supplied by the model as well as the
    /// execution sandbox are resolved against this directory **instead**
    /// of the process-wide current working directory. CLI front-ends are
    /// expected to expand this to an absolute path before sending the
    /// `ConfigureSession` operation so that the business-logic layer can
    /// operate deterministically.
    cwd: PathBuf,
}

impl Session {
    async fn new(
        configure_session: ConfigureSession,
        config: Arc<Config>,
        auth_manager: Arc<AuthManager>,
        tx_event: Sender<Event>,
        initial_history: InitialHistory,
    ) -> anyhow::Result<(Arc<Self>, TurnContext)> {
        let ConfigureSession {
            provider,
            model,
            model_reasoning_effort,
            model_reasoning_summary,
            user_instructions,
            base_instructions,
            approval_policy,
            sandbox_policy,
            notify,
            cwd,
        } = configure_session;
        debug!("Configuring session: model={model}; provider={provider:?}");
        if !cwd.is_absolute() {
            return Err(anyhow::anyhow!("cwd is not absolute: {cwd:?}"));
        }

        let (conversation_id, rollout_params) = match &initial_history {
            InitialHistory::New | InitialHistory::Forked(_) => {
                let conversation_id = ConversationId::default();
                (
                    conversation_id,
                    RolloutRecorderParams::new(conversation_id, user_instructions.clone()),
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
            config.use_experimental_use_rmcp_client,
        );
        let default_shell_fut = shell::default_user_shell();
        let history_meta_fut = crate::message_history::history_metadata(&config);

        // Join all independent futures.
        let (rollout_recorder, mcp_res, default_shell, (history_log_id, history_entry_count)) =
            tokio::join!(rollout_fut, mcp_fut, default_shell_fut, history_meta_fut);

        let rollout_recorder = rollout_recorder.map_err(|e| {
            error!("failed to initialize rollout recorder: {e:#}");
            anyhow::anyhow!("failed to initialize rollout recorder: {e:#}")
        })?;
        let rollout_path = rollout_recorder.rollout_path.clone();
        // Create the mutable state for the Session.
        let state = SessionState::new();

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
                let message = format!("MCP client for `{server_name}` failed to start: {err:#}");
                error!("{message}");
                post_session_configured_error_events.push(Event {
                    id: INITIAL_SUBMIT_ID.to_owned(),
                    msg: EventMsg::Error(ErrorEvent { message }),
                });
            }
        }

        // Now that the conversation id is final (may have been updated by resume),
        // construct the model client.
        let client = ModelClient::new(
            config.clone(),
            Some(auth_manager.clone()),
            provider.clone(),
            model_reasoning_effort,
            model_reasoning_summary,
            conversation_id,
        );
        let turn_context = TurnContext {
            client,
            tools_config: ToolsConfig::new(&ToolsConfigParams {
                model_family: &config.model_family,
                include_plan_tool: config.include_plan_tool,
                include_apply_patch_tool: config.include_apply_patch_tool,
                include_web_search_request: config.tools_web_search_request,
                use_streamable_shell_tool: config.use_experimental_streamable_shell_tool,
                include_view_image_tool: config.include_view_image_tool,
                experimental_unified_exec_tool: config.use_experimental_unified_exec_tool,
            }),
            user_instructions,
            base_instructions,
            approval_policy,
            sandbox_policy,
            shell_environment_policy: config.shell_environment_policy.clone(),
            cwd,
            is_review_mode: false,
            final_output_json_schema: None,
        };
        let services = SessionServices {
            mcp_connection_manager,
            session_manager: ExecSessionManager::default(),
            unified_exec_manager: UnifiedExecSessionManager::default(),
            notifier: notify,
            rollout: Mutex::new(Some(rollout_recorder)),
            codex_linux_sandbox_exe: config.codex_linux_sandbox_exe.clone(),
            user_shell: default_shell,
            show_raw_agent_reasoning: config.show_raw_agent_reasoning,
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
        sess.record_initial_history(&turn_context, initial_history)
            .await;

        let events = std::iter::once(Event {
            id: INITIAL_SUBMIT_ID.to_owned(),
            msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
                session_id: conversation_id,
                model,
                reasoning_effort: model_reasoning_effort,
                history_log_id,
                history_entry_count,
                initial_messages,
                rollout_path,
            }),
        })
        .chain(post_session_configured_error_events.into_iter());
        for event in events {
            sess.send_event(event).await;
        }

        Ok((sess, turn_context))
    }

    fn next_internal_sub_id(&self) -> String {
        let id = self
            .next_internal_sub_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("auto-compact-{id}")
    }

    async fn record_initial_history(
        &self,
        turn_context: &TurnContext,
        conversation_history: InitialHistory,
    ) {
        match conversation_history {
            InitialHistory::New => {
                // Build and record initial items (user instructions + environment context)
                let items = self.build_initial_context(turn_context);
                self.record_conversation_items(&items).await;
            }
            InitialHistory::Resumed(_) | InitialHistory::Forked(_) => {
                let rollout_items = conversation_history.get_rollout_items();
                let persist = matches!(conversation_history, InitialHistory::Forked(_));

                // Always add response items to conversation history
                let reconstructed_history =
                    self.reconstruct_history_from_rollout(turn_context, &rollout_items);
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

    /// Persist the event to rollout and send it to clients.
    pub(crate) async fn send_event(&self, event: Event) {
        // Persist the event into rollout (recorder filters as needed)
        let rollout_items = vec![RolloutItem::EventMsg(event.msg.clone())];
        self.persist_rollout_items(&rollout_items).await;
        if let Err(e) = self.tx_event.send(event).await {
            error!("failed to send tool call event: {e}");
        }
    }

    pub async fn request_command_approval(
        &self,
        sub_id: String,
        call_id: String,
        command: Vec<String>,
        cwd: PathBuf,
        reason: Option<String>,
    ) -> ReviewDecision {
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

        let event = Event {
            id: event_id,
            msg: EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                call_id,
                command,
                cwd,
                reason,
            }),
        };
        self.send_event(event).await;
        rx_approve.await.unwrap_or_default()
    }

    pub async fn request_patch_approval(
        &self,
        sub_id: String,
        call_id: String,
        action: &ApplyPatchAction,
        reason: Option<String>,
        grant_root: Option<PathBuf>,
    ) -> oneshot::Receiver<ReviewDecision> {
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

        let event = Event {
            id: event_id,
            msg: EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
                call_id,
                changes: convert_apply_patch_to_protocol(action),
                reason,
                grant_root,
            }),
        };
        self.send_event(event).await;
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

    pub async fn add_approved_command(&self, cmd: Vec<String>) {
        let mut state = self.state.lock().await;
        state.add_approved_command(cmd);
    }

    /// Records input items: always append to conversation history and
    /// persist these response items to rollout.
    async fn record_conversation_items(&self, items: &[ResponseItem]) {
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
                    let snapshot = history.contents();
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
        history.contents()
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

    pub(crate) async fn history_snapshot(&self) -> Vec<ResponseItem> {
        let state = self.state.lock().await;
        state.history_snapshot()
    }

    async fn update_token_usage_info(
        &self,
        sub_id: &str,
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
        self.send_token_count_event(sub_id).await;
    }

    async fn update_rate_limits(&self, sub_id: &str, new_rate_limits: RateLimitSnapshot) {
        {
            let mut state = self.state.lock().await;
            state.set_rate_limits(new_rate_limits);
        }
        self.send_token_count_event(sub_id).await;
    }

    async fn send_token_count_event(&self, sub_id: &str) {
        let (info, rate_limits) = {
            let state = self.state.lock().await;
            state.token_info_and_rate_limits()
        };
        let event = Event {
            id: sub_id.to_string(),
            msg: EventMsg::TokenCount(TokenCountEvent { info, rate_limits }),
        };
        self.send_event(event).await;
    }

    /// Record a user input item to conversation history and also persist a
    /// corresponding UserMessage EventMsg to rollout.
    async fn record_input_and_rollout_usermsg(&self, response_input: &ResponseInputItem) {
        let response_item: ResponseItem = response_input.clone().into();
        // Add to conversation history and persist response item to rollout
        self.record_conversation_items(std::slice::from_ref(&response_item))
            .await;

        // Derive user message events and persist only UserMessage to rollout
        let msgs =
            map_response_item_to_event_messages(&response_item, self.show_raw_agent_reasoning());
        let user_msgs: Vec<RolloutItem> = msgs
            .into_iter()
            .filter_map(|m| match m {
                EventMsg::UserMessage(ev) => Some(RolloutItem::EventMsg(EventMsg::UserMessage(ev))),
                _ => None,
            })
            .collect();
        if !user_msgs.is_empty() {
            self.persist_rollout_items(&user_msgs).await;
        }
    }

    async fn on_exec_command_begin(
        &self,
        turn_diff_tracker: &mut TurnDiffTracker,
        exec_command_context: ExecCommandContext,
    ) {
        let ExecCommandContext {
            sub_id,
            call_id,
            command_for_display,
            cwd,
            apply_patch,
        } = exec_command_context;
        let msg = match apply_patch {
            Some(ApplyPatchCommandContext {
                user_explicitly_approved_this_action,
                changes,
            }) => {
                turn_diff_tracker.on_patch_begin(&changes);

                EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
                    call_id,
                    auto_approved: !user_explicitly_approved_this_action,
                    changes,
                })
            }
            None => EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                call_id,
                command: command_for_display.clone(),
                cwd,
                parsed_cmd: parse_command(&command_for_display)
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            }),
        };
        let event = Event {
            id: sub_id.to_string(),
            msg,
        };
        self.send_event(event).await;
    }

    async fn on_exec_command_end(
        &self,
        turn_diff_tracker: &mut TurnDiffTracker,
        sub_id: &str,
        call_id: &str,
        output: &ExecToolCallOutput,
        is_apply_patch: bool,
    ) {
        let ExecToolCallOutput {
            stdout,
            stderr,
            aggregated_output,
            duration,
            exit_code,
            timed_out: _,
        } = output;
        // Send full stdout/stderr to clients; do not truncate.
        let stdout = stdout.text.clone();
        let stderr = stderr.text.clone();
        let formatted_output = format_exec_output_str(output);
        let aggregated_output: String = aggregated_output.text.clone();

        let msg = if is_apply_patch {
            EventMsg::PatchApplyEnd(PatchApplyEndEvent {
                call_id: call_id.to_string(),
                stdout,
                stderr,
                success: *exit_code == 0,
            })
        } else {
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id: call_id.to_string(),
                stdout,
                stderr,
                aggregated_output,
                exit_code: *exit_code,
                duration: *duration,
                formatted_output,
            })
        };

        let event = Event {
            id: sub_id.to_string(),
            msg,
        };
        self.send_event(event).await;

        // If this is an apply_patch, after we emit the end patch, emit a second event
        // with the full turn diff if there is one.
        if is_apply_patch {
            let unified_diff = turn_diff_tracker.get_unified_diff();
            if let Ok(Some(unified_diff)) = unified_diff {
                let msg = EventMsg::TurnDiff(TurnDiffEvent { unified_diff });
                let event = Event {
                    id: sub_id.into(),
                    msg,
                };
                self.send_event(event).await;
            }
        }
    }
    /// Runs the exec tool call and emits events for the begin and end of the
    /// command even on error.
    ///
    /// Returns the output of the exec tool call.
    async fn run_exec_with_events<'a>(
        &self,
        turn_diff_tracker: &mut TurnDiffTracker,
        begin_ctx: ExecCommandContext,
        exec_args: ExecInvokeArgs<'a>,
    ) -> crate::error::Result<ExecToolCallOutput> {
        let is_apply_patch = begin_ctx.apply_patch.is_some();
        let sub_id = begin_ctx.sub_id.clone();
        let call_id = begin_ctx.call_id.clone();

        self.on_exec_command_begin(turn_diff_tracker, begin_ctx.clone())
            .await;

        let result = process_exec_tool_call(
            exec_args.params,
            exec_args.sandbox_type,
            exec_args.sandbox_policy,
            exec_args.sandbox_cwd,
            exec_args.codex_linux_sandbox_exe,
            exec_args.stdout_stream,
        )
        .await;

        let output_stderr;
        let borrowed: &ExecToolCallOutput = match &result {
            Ok(output) => output,
            Err(CodexErr::Sandbox(SandboxErr::Timeout { output })) => output,
            Err(e) => {
                output_stderr = ExecToolCallOutput {
                    exit_code: -1,
                    stdout: StreamOutput::new(String::new()),
                    stderr: StreamOutput::new(get_error_message_ui(e)),
                    aggregated_output: StreamOutput::new(get_error_message_ui(e)),
                    duration: Duration::default(),
                    timed_out: false,
                };
                &output_stderr
            }
        };
        self.on_exec_command_end(
            turn_diff_tracker,
            &sub_id,
            &call_id,
            borrowed,
            is_apply_patch,
        )
        .await;

        result
    }

    /// Helper that emits a BackgroundEvent with the given message. This keeps
    /// the call‑sites terse so adding more diagnostics does not clutter the
    /// core agent logic.
    async fn notify_background_event(&self, sub_id: &str, message: impl Into<String>) {
        let event = Event {
            id: sub_id.to_string(),
            msg: EventMsg::BackgroundEvent(BackgroundEventEvent {
                message: message.into(),
            }),
        };
        self.send_event(event).await;
    }

    async fn notify_stream_error(&self, sub_id: &str, message: impl Into<String>) {
        let event = Event {
            id: sub_id.to_string(),
            msg: EventMsg::StreamError(StreamErrorEvent {
                message: message.into(),
            }),
        };
        self.send_event(event).await;
    }

    /// Build the full turn input by concatenating the current conversation
    /// history with additional items for this turn.
    pub async fn turn_input_with_history(&self, extra: Vec<ResponseItem>) -> Vec<ResponseItem> {
        let history = {
            let state = self.state.lock().await;
            state.history_snapshot()
        };
        [history, extra].concat()
    }

    /// Returns the input if there was no task running to inject into
    pub async fn inject_input(&self, input: Vec<InputItem>) -> Result<(), Vec<InputItem>> {
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

    pub async fn interrupt_task(self: &Arc<Self>) {
        info!("interrupt received: abort current task, if any");
        self.abort_all_tasks(TurnAbortReason::Interrupted).await;
    }

    fn interrupt_task_sync(&self) {
        if let Ok(mut active) = self.active_turn.try_lock()
            && let Some(at) = active.as_mut()
        {
            at.try_clear_pending_sync();
            let tasks = at.drain_tasks();
            *active = None;
            for (_sub_id, task) in tasks {
                task.handle.abort();
            }
        }
    }

    pub(crate) fn notifier(&self) -> &UserNotifier {
        &self.services.notifier
    }

    fn user_shell(&self) -> &shell::Shell {
        &self.services.user_shell
    }

    fn show_raw_agent_reasoning(&self) -> bool {
        self.services.show_raw_agent_reasoning
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.interrupt_task_sync();
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ExecCommandContext {
    pub(crate) sub_id: String,
    pub(crate) call_id: String,
    pub(crate) command_for_display: Vec<String>,
    pub(crate) cwd: PathBuf,
    pub(crate) apply_patch: Option<ApplyPatchCommandContext>,
}

#[derive(Clone, Debug)]
pub(crate) struct ApplyPatchCommandContext {
    pub(crate) user_explicitly_approved_this_action: bool,
    pub(crate) changes: HashMap<PathBuf, FileChange>,
}

async fn submission_loop(
    sess: Arc<Session>,
    turn_context: TurnContext,
    config: Arc<Config>,
    rx_sub: Receiver<Submission>,
) {
    // Wrap once to avoid cloning TurnContext for each task.
    let mut turn_context = Arc::new(turn_context);
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
                // Recalculate the persistent turn context with provided overrides.
                let prev = Arc::clone(&turn_context);
                let provider = prev.client.get_provider();

                // Effective model + family
                let (effective_model, effective_family) = if let Some(ref m) = model {
                    let fam =
                        find_family_for_model(m).unwrap_or_else(|| config.model_family.clone());
                    (m.clone(), fam)
                } else {
                    (prev.client.get_model(), prev.client.get_model_family())
                };

                // Effective reasoning settings
                let effective_effort = effort.unwrap_or(prev.client.get_reasoning_effort());
                let effective_summary = summary.unwrap_or(prev.client.get_reasoning_summary());

                let auth_manager = prev.client.get_auth_manager();

                // Build updated config for the client
                let mut updated_config = (*config).clone();
                updated_config.model = effective_model.clone();
                updated_config.model_family = effective_family.clone();
                if let Some(model_info) = get_model_info(&effective_family) {
                    updated_config.model_context_window = Some(model_info.context_window);
                }

                let client = ModelClient::new(
                    Arc::new(updated_config),
                    auth_manager,
                    provider,
                    effective_effort,
                    effective_summary,
                    sess.conversation_id,
                );

                let new_approval_policy = approval_policy.unwrap_or(prev.approval_policy);
                let new_sandbox_policy = sandbox_policy
                    .clone()
                    .unwrap_or(prev.sandbox_policy.clone());
                let new_cwd = cwd.clone().unwrap_or_else(|| prev.cwd.clone());

                let tools_config = ToolsConfig::new(&ToolsConfigParams {
                    model_family: &effective_family,
                    include_plan_tool: config.include_plan_tool,
                    include_apply_patch_tool: config.include_apply_patch_tool,
                    include_web_search_request: config.tools_web_search_request,
                    use_streamable_shell_tool: config.use_experimental_streamable_shell_tool,
                    include_view_image_tool: config.include_view_image_tool,
                    experimental_unified_exec_tool: config.use_experimental_unified_exec_tool,
                });

                let new_turn_context = TurnContext {
                    client,
                    tools_config,
                    user_instructions: prev.user_instructions.clone(),
                    base_instructions: prev.base_instructions.clone(),
                    approval_policy: new_approval_policy,
                    sandbox_policy: new_sandbox_policy.clone(),
                    shell_environment_policy: prev.shell_environment_policy.clone(),
                    cwd: new_cwd.clone(),
                    is_review_mode: false,
                    final_output_json_schema: None,
                };

                // Install the new persistent context for subsequent tasks/turns.
                turn_context = Arc::new(new_turn_context);

                // Optionally persist changes to model / effort
                if cwd.is_some() || approval_policy.is_some() || sandbox_policy.is_some() {
                    sess.record_conversation_items(&[ResponseItem::from(EnvironmentContext::new(
                        cwd,
                        approval_policy,
                        sandbox_policy,
                        // Shell is not configurable from turn to turn
                        None,
                    ))])
                    .await;
                }
            }
            Op::UserInput { items } => {
                // attempt to inject input into current task
                if let Err(items) = sess.inject_input(items).await {
                    // no current task, spawn a new one
                    sess.spawn_task(Arc::clone(&turn_context), sub.id, items, RegularTask)
                        .await;
                }
            }
            Op::UserTurn {
                items,
                cwd,
                approval_policy,
                sandbox_policy,
                model,
                effort,
                summary,
                final_output_json_schema,
            } => {
                // attempt to inject input into current task
                if let Err(items) = sess.inject_input(items).await {
                    // Derive a fresh TurnContext for this turn using the provided overrides.
                    let provider = turn_context.client.get_provider();
                    let auth_manager = turn_context.client.get_auth_manager();

                    // Derive a model family for the requested model; fall back to the session's.
                    let model_family = find_family_for_model(&model)
                        .unwrap_or_else(|| config.model_family.clone());

                    // Create a per‑turn Config clone with the requested model/family.
                    let mut per_turn_config = (*config).clone();
                    per_turn_config.model = model.clone();
                    per_turn_config.model_family = model_family.clone();
                    if let Some(model_info) = get_model_info(&model_family) {
                        per_turn_config.model_context_window = Some(model_info.context_window);
                    }

                    // Build a new client with per‑turn reasoning settings.
                    // Reuse the same provider and session id; auth defaults to env/API key.
                    let client = ModelClient::new(
                        Arc::new(per_turn_config),
                        auth_manager,
                        provider,
                        effort,
                        summary,
                        sess.conversation_id,
                    );

                    let fresh_turn_context = TurnContext {
                        client,
                        tools_config: ToolsConfig::new(&ToolsConfigParams {
                            model_family: &model_family,
                            include_plan_tool: config.include_plan_tool,
                            include_apply_patch_tool: config.include_apply_patch_tool,
                            include_web_search_request: config.tools_web_search_request,
                            use_streamable_shell_tool: config
                                .use_experimental_streamable_shell_tool,
                            include_view_image_tool: config.include_view_image_tool,
                            experimental_unified_exec_tool: config
                                .use_experimental_unified_exec_tool,
                        }),
                        user_instructions: turn_context.user_instructions.clone(),
                        base_instructions: turn_context.base_instructions.clone(),
                        approval_policy,
                        sandbox_policy,
                        shell_environment_policy: turn_context.shell_environment_policy.clone(),
                        cwd,
                        is_review_mode: false,
                        final_output_json_schema,
                    };

                    // if the environment context has changed, record it in the conversation history
                    let previous_env_context = EnvironmentContext::from(turn_context.as_ref());
                    let new_env_context = EnvironmentContext::from(&fresh_turn_context);
                    if !new_env_context.equals_except_shell(&previous_env_context) {
                        sess.record_conversation_items(&[ResponseItem::from(new_env_context)])
                            .await;
                    }

                    // Install the new persistent context for subsequent tasks/turns.
                    turn_context = Arc::new(fresh_turn_context);

                    // no current task, spawn a new one with the per-turn context
                    sess.spawn_task(Arc::clone(&turn_context), sub.id, items, RegularTask)
                        .await;
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

                    sess_clone.send_event(event).await;
                });
            }
            Op::ListMcpTools => {
                let sub_id = sub.id.clone();

                // This is a cheap lookup from the connection manager's cache.
                let tools = sess.services.mcp_connection_manager.list_all_tools();
                let event = Event {
                    id: sub_id,
                    msg: EventMsg::McpListToolsResponse(
                        crate::protocol::McpListToolsResponseEvent { tools },
                    ),
                };
                sess.send_event(event).await;
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
                sess.send_event(event).await;
            }
            Op::Compact => {
                // Attempt to inject input into current task
                if let Err(items) = sess
                    .inject_input(vec![InputItem::Text {
                        text: compact::SUMMARIZATION_PROMPT.to_string(),
                    }])
                    .await
                {
                    sess.spawn_task(Arc::clone(&turn_context), sub.id, items, CompactTask)
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
                    sess.send_event(event).await;
                }

                let event = Event {
                    id: sub.id.clone(),
                    msg: EventMsg::ShutdownComplete,
                };
                sess.send_event(event).await;
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
                sess.send_event(event).await;
            }
            Op::Review { review_request } => {
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
    let tools_config = ToolsConfig::new(&ToolsConfigParams {
        model_family: &review_model_family,
        include_plan_tool: false,
        include_apply_patch_tool: config.include_apply_patch_tool,
        include_web_search_request: false,
        use_streamable_shell_tool: false,
        include_view_image_tool: false,
        experimental_unified_exec_tool: config.use_experimental_unified_exec_tool,
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

    let per_turn_config = Arc::new(per_turn_config);
    let client = ModelClient::new(
        per_turn_config.clone(),
        auth_manager,
        provider,
        per_turn_config.model_reasoning_effort,
        per_turn_config.model_reasoning_summary,
        sess.conversation_id,
    );

    let review_turn_context = TurnContext {
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
    };

    // Seed the child task with the review prompt as the initial user message.
    let input: Vec<InputItem> = vec![InputItem::Text {
        text: format!("{base_instructions}\n\n---\n\nNow, here's your task: {review_prompt}"),
    }];
    let tc = Arc::new(review_turn_context);

    // Clone sub_id for the upcoming announcement before moving it into the task.
    let sub_id_for_event = sub_id.clone();
    sess.spawn_task(tc.clone(), sub_id, input, ReviewTask).await;

    // Announce entering review mode so UIs can switch modes.
    sess.send_event(Event {
        id: sub_id_for_event,
        msg: EventMsg::EnteredReviewMode(review_request),
    })
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
    sub_id: String,
    input: Vec<InputItem>,
) -> Option<String> {
    if input.is_empty() {
        return None;
    }
    let event = Event {
        id: sub_id.clone(),
        msg: EventMsg::TaskStarted(TaskStartedEvent {
            model_context_window: turn_context.client.get_model_context_window(),
        }),
    };
    sess.send_event(event).await;

    let initial_input_for_turn: ResponseInputItem = ResponseInputItem::from(input);
    // For review threads, keep an isolated in-memory history so the
    // model sees a fresh conversation without the parent session's history.
    // For normal turns, continue recording to the session history as before.
    let is_review_mode = turn_context.is_review_mode;
    let mut review_thread_history: Vec<ResponseItem> = Vec::new();
    if is_review_mode {
        // Seed review threads with environment context so the model knows the working directory.
        review_thread_history.extend(sess.build_initial_context(turn_context.as_ref()));
        review_thread_history.push(initial_input_for_turn.into());
    } else {
        sess.record_input_and_rollout_usermsg(&initial_input_for_turn)
            .await;
    }

    let mut last_agent_message: Option<String> = None;
    // Although from the perspective of codex.rs, TurnDiffTracker has the lifecycle of a Task which contains
    // many turns, from the perspective of the user, it is a single turn.
    let mut turn_diff_tracker = TurnDiffTracker::new();
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
                review_thread_history.extend(pending_input);
            }
            review_thread_history.clone()
        } else {
            sess.record_conversation_items(&pending_input).await;
            sess.turn_input_with_history(pending_input).await
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
            &sess,
            turn_context.as_ref(),
            &mut turn_diff_tracker,
            sub_id.clone(),
            turn_input,
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
                    .map(|tokens| (tokens as i64) >= limit)
                    .unwrap_or(false);
                let mut items_to_record_in_conversation_history = Vec::<ResponseItem>::new();
                let mut responses = Vec::<ResponseInputItem>::new();
                for processed_response_item in processed_items {
                    let ProcessedResponseItem { item, response } = processed_response_item;
                    match (&item, &response) {
                        (ResponseItem::Message { role, .. }, None) if role == "assistant" => {
                            // If the model returned a message, we need to record it.
                            items_to_record_in_conversation_history.push(item);
                        }
                        (
                            ResponseItem::LocalShellCall { .. },
                            Some(ResponseInputItem::FunctionCallOutput { call_id, output }),
                        ) => {
                            items_to_record_in_conversation_history.push(item);
                            items_to_record_in_conversation_history.push(
                                ResponseItem::FunctionCallOutput {
                                    call_id: call_id.clone(),
                                    output: output.clone(),
                                },
                            );
                        }
                        (
                            ResponseItem::FunctionCall { .. },
                            Some(ResponseInputItem::FunctionCallOutput { call_id, output }),
                        ) => {
                            items_to_record_in_conversation_history.push(item);
                            items_to_record_in_conversation_history.push(
                                ResponseItem::FunctionCallOutput {
                                    call_id: call_id.clone(),
                                    output: output.clone(),
                                },
                            );
                        }
                        (
                            ResponseItem::CustomToolCall { .. },
                            Some(ResponseInputItem::CustomToolCallOutput { call_id, output }),
                        ) => {
                            items_to_record_in_conversation_history.push(item);
                            items_to_record_in_conversation_history.push(
                                ResponseItem::CustomToolCallOutput {
                                    call_id: call_id.clone(),
                                    output: output.clone(),
                                },
                            );
                        }
                        (
                            ResponseItem::FunctionCall { .. },
                            Some(ResponseInputItem::McpToolCallOutput { call_id, result }),
                        ) => {
                            items_to_record_in_conversation_history.push(item);
                            let output = match result {
                                Ok(call_tool_result) => {
                                    convert_call_tool_result_to_function_call_output_payload(
                                        call_tool_result,
                                    )
                                }
                                Err(err) => FunctionCallOutputPayload {
                                    content: err.clone(),
                                    success: Some(false),
                                },
                            };
                            items_to_record_in_conversation_history.push(
                                ResponseItem::FunctionCallOutput {
                                    call_id: call_id.clone(),
                                    output,
                                },
                            );
                        }
                        (
                            ResponseItem::Reasoning {
                                id,
                                summary,
                                content,
                                encrypted_content,
                            },
                            None,
                        ) => {
                            items_to_record_in_conversation_history.push(ResponseItem::Reasoning {
                                id: id.clone(),
                                summary: summary.clone(),
                                content: content.clone(),
                                encrypted_content: encrypted_content.clone(),
                            });
                        }
                        _ => {
                            warn!("Unexpected response item: {item:?} with response: {response:?}");
                        }
                    };
                    if let Some(response) = response {
                        responses.push(response);
                    }
                }

                // Only attempt to take the lock if there is something to record.
                if !items_to_record_in_conversation_history.is_empty() {
                    if is_review_mode {
                        review_thread_history
                            .extend(items_to_record_in_conversation_history.clone());
                    } else {
                        sess.record_conversation_items(&items_to_record_in_conversation_history)
                            .await;
                    }
                }

                if token_limit_reached {
                    if auto_compact_recently_attempted {
                        let limit_str = limit.to_string();
                        let current_tokens = total_usage_tokens
                            .map(|tokens| tokens.to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        let event = Event {
                            id: sub_id.clone(),
                            msg: EventMsg::Error(ErrorEvent {
                                message: format!(
                                    "Conversation is still above the token limit after automatic summarization (limit {limit_str}, current {current_tokens}). Please start a new session or trim your input."
                                ),
                            }),
                        };
                        sess.send_event(event).await;
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
                            turn_id: sub_id.clone(),
                            input_messages: turn_input_messages,
                            last_assistant_message: last_agent_message.clone(),
                        });
                    break;
                }
                continue;
            }
            Err(e) => {
                info!("Turn error: {e:#}");
                let event = Event {
                    id: sub_id.clone(),
                    msg: EventMsg::Error(ErrorEvent {
                        message: e.to_string(),
                    }),
                };
                sess.send_event(event).await;
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
            sub_id.clone(),
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
    sess: &Session,
    turn_context: &TurnContext,
    turn_diff_tracker: &mut TurnDiffTracker,
    sub_id: String,
    input: Vec<ResponseItem>,
) -> CodexResult<TurnRunResult> {
    let tools = get_openai_tools(
        &turn_context.tools_config,
        Some(sess.services.mcp_connection_manager.list_all_tools()),
    );

    let prompt = Prompt {
        input,
        tools,
        base_instructions_override: turn_context.base_instructions.clone(),
        output_schema: turn_context.final_output_json_schema.clone(),
    };

    let mut retries = 0;
    loop {
        match try_run_turn(sess, turn_context, turn_diff_tracker, &sub_id, &prompt).await {
            Ok(output) => return Ok(output),
            Err(CodexErr::Interrupted) => return Err(CodexErr::Interrupted),
            Err(CodexErr::EnvVar(var)) => return Err(CodexErr::EnvVar(var)),
            Err(CodexErr::UsageLimitReached(e)) => {
                let rate_limits = e.rate_limits.clone();
                if let Some(rate_limits) = rate_limits {
                    sess.update_rate_limits(&sub_id, rate_limits).await;
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
                        &sub_id,
                        format!(
                            "stream error: {e}; retrying {retries}/{max_retries} in {delay:?}…"
                        ),
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
struct ProcessedResponseItem {
    item: ResponseItem,
    response: Option<ResponseInputItem>,
}

#[derive(Debug)]
struct TurnRunResult {
    processed_items: Vec<ProcessedResponseItem>,
    total_token_usage: Option<TokenUsage>,
}

async fn try_run_turn(
    sess: &Session,
    turn_context: &TurnContext,
    turn_diff_tracker: &mut TurnDiffTracker,
    sub_id: &str,
    prompt: &Prompt,
) -> CodexResult<TurnRunResult> {
    // call_ids that are part of this response.
    let completed_call_ids = prompt
        .input
        .iter()
        .filter_map(|ri| match ri {
            ResponseItem::FunctionCallOutput { call_id, .. } => Some(call_id),
            ResponseItem::LocalShellCall {
                call_id: Some(call_id),
                ..
            } => Some(call_id),
            ResponseItem::CustomToolCallOutput { call_id, .. } => Some(call_id),
            _ => None,
        })
        .collect::<Vec<_>>();

    // call_ids that were pending but are not part of this response.
    // This usually happens because the user interrupted the model before we responded to one of its tool calls
    // and then the user sent a follow-up message.
    let missing_calls = {
        prompt
            .input
            .iter()
            .filter_map(|ri| match ri {
                ResponseItem::FunctionCall { call_id, .. } => Some(call_id),
                ResponseItem::LocalShellCall {
                    call_id: Some(call_id),
                    ..
                } => Some(call_id),
                ResponseItem::CustomToolCall { call_id, .. } => Some(call_id),
                _ => None,
            })
            .filter_map(|call_id| {
                if completed_call_ids.contains(&call_id) {
                    None
                } else {
                    Some(call_id.clone())
                }
            })
            .map(|call_id| ResponseItem::CustomToolCallOutput {
                call_id,
                output: "aborted".to_string(),
            })
            .collect::<Vec<_>>()
    };
    let prompt: Cow<Prompt> = if missing_calls.is_empty() {
        Cow::Borrowed(prompt)
    } else {
        // Add the synthetic aborted missing calls to the beginning of the input to ensure all call ids have responses.
        let input = [missing_calls, prompt.input.clone()].concat();
        Cow::Owned(Prompt {
            input,
            ..prompt.clone()
        })
    };

    let rollout_item = RolloutItem::TurnContext(TurnContextItem {
        cwd: turn_context.cwd.clone(),
        approval_policy: turn_context.approval_policy,
        sandbox_policy: turn_context.sandbox_policy.clone(),
        model: turn_context.client.get_model(),
        effort: turn_context.client.get_reasoning_effort(),
        summary: turn_context.client.get_reasoning_summary(),
    });
    sess.persist_rollout_items(&[rollout_item]).await;
    let mut stream = turn_context.client.clone().stream(&prompt).await?;

    let mut output = Vec::new();

    loop {
        // Poll the next item from the model stream. We must inspect *both* Ok and Err
        // cases so that transient stream failures (e.g., dropped SSE connection before
        // `response.completed`) bubble up and trigger the caller's retry logic.
        let event = stream.next().await;
        let Some(event) = event else {
            // Channel closed without yielding a final Completed event or explicit error.
            // Treat as a disconnected stream so the caller can retry.
            return Err(CodexErr::Stream(
                "stream closed before response.completed".into(),
                None,
            ));
        };

        let event = match event {
            Ok(ev) => ev,
            Err(e) => {
                // Propagate the underlying stream error to the caller (run_turn), which
                // will apply the configured `stream_max_retries` policy.
                return Err(e);
            }
        };

        match event {
            ResponseEvent::Created => {}
            ResponseEvent::OutputItemDone(item) => {
                let response = handle_response_item(
                    sess,
                    turn_context,
                    turn_diff_tracker,
                    sub_id,
                    item.clone(),
                )
                .await?;
                output.push(ProcessedResponseItem { item, response });
            }
            ResponseEvent::WebSearchCallBegin { call_id } => {
                let _ = sess
                    .tx_event
                    .send(Event {
                        id: sub_id.to_string(),
                        msg: EventMsg::WebSearchBegin(WebSearchBeginEvent { call_id }),
                    })
                    .await;
            }
            ResponseEvent::RateLimits(snapshot) => {
                // Update internal state with latest rate limits, but defer sending until
                // token usage is available to avoid duplicate TokenCount events.
                sess.update_rate_limits(sub_id, snapshot).await;
            }
            ResponseEvent::Completed {
                response_id: _,
                token_usage,
            } => {
                sess.update_token_usage_info(sub_id, turn_context, token_usage.as_ref())
                    .await;

                let unified_diff = turn_diff_tracker.get_unified_diff();
                if let Ok(Some(unified_diff)) = unified_diff {
                    let msg = EventMsg::TurnDiff(TurnDiffEvent { unified_diff });
                    let event = Event {
                        id: sub_id.to_string(),
                        msg,
                    };
                    sess.send_event(event).await;
                }

                let result = TurnRunResult {
                    processed_items: output,
                    total_token_usage: token_usage.clone(),
                };

                return Ok(result);
            }
            ResponseEvent::OutputTextDelta(delta) => {
                // In review child threads, suppress assistant text deltas; the
                // UI will show a selection popup from the final ReviewOutput.
                if !turn_context.is_review_mode {
                    let event = Event {
                        id: sub_id.to_string(),
                        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta }),
                    };
                    sess.send_event(event).await;
                } else {
                    trace!("suppressing OutputTextDelta in review mode");
                }
            }
            ResponseEvent::ReasoningSummaryDelta(delta) => {
                let event = Event {
                    id: sub_id.to_string(),
                    msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent { delta }),
                };
                sess.send_event(event).await;
            }
            ResponseEvent::ReasoningSummaryPartAdded => {
                let event = Event {
                    id: sub_id.to_string(),
                    msg: EventMsg::AgentReasoningSectionBreak(AgentReasoningSectionBreakEvent {}),
                };
                sess.send_event(event).await;
            }
            ResponseEvent::ReasoningContentDelta(delta) => {
                if sess.show_raw_agent_reasoning() {
                    let event = Event {
                        id: sub_id.to_string(),
                        msg: EventMsg::AgentReasoningRawContentDelta(
                            AgentReasoningRawContentDeltaEvent { delta },
                        ),
                    };
                    sess.send_event(event).await;
                }
            }
        }
    }
}

async fn handle_response_item(
    sess: &Session,
    turn_context: &TurnContext,
    turn_diff_tracker: &mut TurnDiffTracker,
    sub_id: &str,
    item: ResponseItem,
) -> CodexResult<Option<ResponseInputItem>> {
    debug!(?item, "Output item");
    let output = match item {
        ResponseItem::FunctionCall {
            name,
            arguments,
            call_id,
            ..
        } => {
            info!("FunctionCall: {name}({arguments})");
            if let Some((server, tool_name)) =
                sess.services.mcp_connection_manager.parse_tool_name(&name)
            {
                let resp = handle_mcp_tool_call(
                    sess,
                    sub_id,
                    call_id.clone(),
                    server,
                    tool_name,
                    arguments,
                )
                .await;
                Some(resp)
            } else {
                let result = handle_function_call(
                    sess,
                    turn_context,
                    turn_diff_tracker,
                    sub_id.to_string(),
                    name,
                    arguments,
                    call_id.clone(),
                )
                .await;

                let output = match result {
                    Ok(content) => FunctionCallOutputPayload {
                        content,
                        success: Some(true),
                    },
                    Err(FunctionCallError::RespondToModel(msg)) => FunctionCallOutputPayload {
                        content: msg,
                        success: Some(false),
                    },
                };
                Some(ResponseInputItem::FunctionCallOutput { call_id, output })
            }
        }
        ResponseItem::LocalShellCall {
            id,
            call_id,
            status: _,
            action,
        } => {
            let LocalShellAction::Exec(action) = action;
            tracing::info!("LocalShellCall: {action:?}");
            let params = ShellToolCallParams {
                command: action.command,
                workdir: action.working_directory,
                timeout_ms: action.timeout_ms,
                with_escalated_permissions: None,
                justification: None,
            };
            let effective_call_id = match (call_id, id) {
                (Some(call_id), _) => call_id,
                (None, Some(id)) => id,
                (None, None) => {
                    error!("LocalShellCall without call_id or id");
                    return Ok(Some(ResponseInputItem::FunctionCallOutput {
                        call_id: "".to_string(),
                        output: FunctionCallOutputPayload {
                            content: "LocalShellCall without call_id or id".to_string(),
                            success: None,
                        },
                    }));
                }
            };

            let exec_params = to_exec_params(params, turn_context);
            {
                let result = handle_container_exec_with_params(
                    exec_params,
                    sess,
                    turn_context,
                    turn_diff_tracker,
                    sub_id.to_string(),
                    effective_call_id.clone(),
                )
                .await;

                let output = match result {
                    Ok(content) => FunctionCallOutputPayload {
                        content,
                        success: Some(true),
                    },
                    Err(FunctionCallError::RespondToModel(msg)) => FunctionCallOutputPayload {
                        content: msg,
                        success: Some(false),
                    },
                };
                Some(ResponseInputItem::FunctionCallOutput {
                    call_id: effective_call_id,
                    output,
                })
            }
        }
        ResponseItem::CustomToolCall {
            id: _,
            call_id,
            name,
            input,
            status: _,
        } => {
            let result = handle_custom_tool_call(
                sess,
                turn_context,
                turn_diff_tracker,
                sub_id.to_string(),
                name,
                input,
                call_id.clone(),
            )
            .await;

            let output = match result {
                Ok(content) => content,
                Err(FunctionCallError::RespondToModel(msg)) => msg,
            };
            Some(ResponseInputItem::CustomToolCallOutput { call_id, output })
        }
        ResponseItem::FunctionCallOutput { .. } => {
            debug!("unexpected FunctionCallOutput from stream");
            None
        }
        ResponseItem::CustomToolCallOutput { .. } => {
            debug!("unexpected CustomToolCallOutput from stream");
            None
        }
        ResponseItem::Message { .. }
        | ResponseItem::Reasoning { .. }
        | ResponseItem::WebSearchCall { .. } => {
            // In review child threads, suppress assistant message events but
            // keep reasoning/web search.
            let msgs = match &item {
                ResponseItem::Message { .. } if turn_context.is_review_mode => {
                    trace!("suppressing assistant Message in review mode");
                    Vec::new()
                }
                _ => map_response_item_to_event_messages(&item, sess.show_raw_agent_reasoning()),
            };
            for msg in msgs {
                let event = Event {
                    id: sub_id.to_string(),
                    msg,
                };
                sess.send_event(event).await;
            }
            None
        }
        ResponseItem::Other => None,
    };
    Ok(output)
}

async fn handle_unified_exec_tool_call(
    sess: &Session,
    session_id: Option<String>,
    arguments: Vec<String>,
    timeout_ms: Option<u64>,
) -> Result<String, FunctionCallError> {
    let parsed_session_id = if let Some(session_id) = session_id {
        match session_id.parse::<i32>() {
            Ok(parsed) => Some(parsed),
            Err(output) => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "invalid session_id: {session_id} due to error {output:?}"
                )));
            }
        }
    } else {
        None
    };

    let request = crate::unified_exec::UnifiedExecRequest {
        session_id: parsed_session_id,
        input_chunks: &arguments,
        timeout_ms,
    };

    let value = sess
        .services
        .unified_exec_manager
        .handle_request(request)
        .await
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!("unified exec failed: {err:?}"))
        })?;

    #[derive(Serialize)]
    struct SerializedUnifiedExecResult {
        session_id: Option<String>,
        output: String,
    }

    serde_json::to_string(&SerializedUnifiedExecResult {
        session_id: value.session_id.map(|id| id.to_string()),
        output: value.output,
    })
    .map_err(|err| {
        FunctionCallError::RespondToModel(format!(
            "failed to serialize unified exec output: {err:?}"
        ))
    })
}

async fn handle_function_call(
    sess: &Session,
    turn_context: &TurnContext,
    turn_diff_tracker: &mut TurnDiffTracker,
    sub_id: String,
    name: String,
    arguments: String,
    call_id: String,
) -> Result<String, FunctionCallError> {
    match name.as_str() {
        "container.exec" | "shell" => {
            let params = parse_container_exec_arguments(arguments, turn_context, &call_id)?;
            handle_container_exec_with_params(
                params,
                sess,
                turn_context,
                turn_diff_tracker,
                sub_id,
                call_id,
            )
            .await
        }
        "unified_exec" => {
            #[derive(Deserialize)]
            struct UnifiedExecArgs {
                input: Vec<String>,
                #[serde(default)]
                session_id: Option<String>,
                #[serde(default)]
                timeout_ms: Option<u64>,
            }

            let args: UnifiedExecArgs = serde_json::from_str(&arguments).map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "failed to parse function arguments: {err:?}"
                ))
            })?;

            handle_unified_exec_tool_call(sess, args.session_id, args.input, args.timeout_ms).await
        }
        "view_image" => {
            #[derive(serde::Deserialize)]
            struct SeeImageArgs {
                path: String,
            }
            let args: SeeImageArgs = serde_json::from_str(&arguments).map_err(|e| {
                FunctionCallError::RespondToModel(format!(
                    "failed to parse function arguments: {e:?}"
                ))
            })?;
            let abs = turn_context.resolve_path(Some(args.path));
            sess.inject_input(vec![InputItem::LocalImage { path: abs }])
                .await
                .map_err(|_| {
                    FunctionCallError::RespondToModel(
                        "unable to attach image (no active task)".to_string(),
                    )
                })?;

            Ok("attached local image path".to_string())
        }
        "apply_patch" => {
            let args: ApplyPatchToolArgs = serde_json::from_str(&arguments).map_err(|e| {
                FunctionCallError::RespondToModel(format!(
                    "failed to parse function arguments: {e:?}"
                ))
            })?;
            let exec_params = ExecParams {
                command: vec!["apply_patch".to_string(), args.input.clone()],
                cwd: turn_context.cwd.clone(),
                timeout_ms: None,
                env: HashMap::new(),
                with_escalated_permissions: None,
                justification: None,
            };
            handle_container_exec_with_params(
                exec_params,
                sess,
                turn_context,
                turn_diff_tracker,
                sub_id,
                call_id,
            )
            .await
        }
        "update_plan" => handle_update_plan(sess, arguments, sub_id, call_id).await,
        EXEC_COMMAND_TOOL_NAME => {
            // TODO(mbolin): Sandbox check.
            let exec_params: ExecCommandParams = serde_json::from_str(&arguments).map_err(|e| {
                FunctionCallError::RespondToModel(format!(
                    "failed to parse function arguments: {e:?}"
                ))
            })?;
            let result = sess
                .services
                .session_manager
                .handle_exec_command_request(exec_params)
                .await;
            match result {
                Ok(output) => Ok(output.to_text_output()),
                Err(err) => Err(FunctionCallError::RespondToModel(err)),
            }
        }
        WRITE_STDIN_TOOL_NAME => {
            let write_stdin_params =
                serde_json::from_str::<WriteStdinParams>(&arguments).map_err(|e| {
                    FunctionCallError::RespondToModel(format!(
                        "failed to parse function arguments: {e:?}"
                    ))
                })?;

            let result = sess
                .services
                .session_manager
                .handle_write_stdin_request(write_stdin_params)
                .await
                .map_err(FunctionCallError::RespondToModel)?;

            Ok(result.to_text_output())
        }
        _ => Err(FunctionCallError::RespondToModel(format!(
            "unsupported call: {name}"
        ))),
    }
}

async fn handle_custom_tool_call(
    sess: &Session,
    turn_context: &TurnContext,
    turn_diff_tracker: &mut TurnDiffTracker,
    sub_id: String,
    name: String,
    input: String,
    call_id: String,
) -> Result<String, FunctionCallError> {
    info!("CustomToolCall: {name} {input}");
    match name.as_str() {
        "apply_patch" => {
            let exec_params = ExecParams {
                command: vec!["apply_patch".to_string(), input.clone()],
                cwd: turn_context.cwd.clone(),
                timeout_ms: None,
                env: HashMap::new(),
                with_escalated_permissions: None,
                justification: None,
            };

            handle_container_exec_with_params(
                exec_params,
                sess,
                turn_context,
                turn_diff_tracker,
                sub_id,
                call_id,
            )
            .await
        }
        _ => {
            debug!("unexpected CustomToolCall from stream");
            Err(FunctionCallError::RespondToModel(format!(
                "unsupported custom tool call: {name}"
            )))
        }
    }
}

fn to_exec_params(params: ShellToolCallParams, turn_context: &TurnContext) -> ExecParams {
    ExecParams {
        command: params.command,
        cwd: turn_context.resolve_path(params.workdir.clone()),
        timeout_ms: params.timeout_ms,
        env: create_env(&turn_context.shell_environment_policy),
        with_escalated_permissions: params.with_escalated_permissions,
        justification: params.justification,
    }
}

fn parse_container_exec_arguments(
    arguments: String,
    turn_context: &TurnContext,
    _call_id: &str,
) -> Result<ExecParams, FunctionCallError> {
    serde_json::from_str::<ShellToolCallParams>(&arguments)
        .map(|p| to_exec_params(p, turn_context))
        .map_err(|e| {
            FunctionCallError::RespondToModel(format!("failed to parse function arguments: {e:?}"))
        })
}

pub struct ExecInvokeArgs<'a> {
    pub params: ExecParams,
    pub sandbox_type: SandboxType,
    pub sandbox_policy: &'a SandboxPolicy,
    pub sandbox_cwd: &'a Path,
    pub codex_linux_sandbox_exe: &'a Option<PathBuf>,
    pub stdout_stream: Option<StdoutStream>,
}

fn maybe_translate_shell_command(
    params: ExecParams,
    sess: &Session,
    turn_context: &TurnContext,
) -> ExecParams {
    let should_translate = matches!(sess.user_shell(), crate::shell::Shell::PowerShell(_))
        || turn_context.shell_environment_policy.use_profile;

    if should_translate
        && let Some(command) = sess
            .user_shell()
            .format_default_shell_invocation(params.command.clone())
    {
        return ExecParams { command, ..params };
    }
    params
}

async fn handle_container_exec_with_params(
    params: ExecParams,
    sess: &Session,
    turn_context: &TurnContext,
    turn_diff_tracker: &mut TurnDiffTracker,
    sub_id: String,
    call_id: String,
) -> Result<String, FunctionCallError> {
    if params.with_escalated_permissions.unwrap_or(false)
        && !matches!(turn_context.approval_policy, AskForApproval::OnRequest)
    {
        return Err(FunctionCallError::RespondToModel(format!(
            "approval policy is {policy:?}; reject command — you should not ask for escalated permissions if the approval policy is {policy:?}",
            policy = turn_context.approval_policy
        )));
    }

    // check if this was a patch, and apply it if so
    let apply_patch_exec = match maybe_parse_apply_patch_verified(&params.command, &params.cwd) {
        MaybeApplyPatchVerified::Body(changes) => {
            match apply_patch::apply_patch(sess, turn_context, &sub_id, &call_id, changes).await {
                InternalApplyPatchInvocation::Output(item) => return item,
                InternalApplyPatchInvocation::DelegateToExec(apply_patch_exec) => {
                    Some(apply_patch_exec)
                }
            }
        }
        MaybeApplyPatchVerified::CorrectnessError(parse_error) => {
            // It looks like an invocation of `apply_patch`, but we
            // could not resolve it into a patch that would apply
            // cleanly. Return to model for resample.
            return Err(FunctionCallError::RespondToModel(format!(
                "error: {parse_error:#?}"
            )));
        }
        MaybeApplyPatchVerified::ShellParseError(error) => {
            trace!("Failed to parse shell command, {error:?}");
            None
        }
        MaybeApplyPatchVerified::NotApplyPatch => None,
    };

    let (params, safety, command_for_display) = match &apply_patch_exec {
        Some(ApplyPatchExec {
            action: ApplyPatchAction { patch, cwd, .. },
            user_explicitly_approved_this_action,
        }) => {
            let path_to_codex = std::env::current_exe()
                .ok()
                .map(|p| p.to_string_lossy().to_string());
            let Some(path_to_codex) = path_to_codex else {
                return Err(FunctionCallError::RespondToModel(
                    "failed to determine path to codex executable".to_string(),
                ));
            };

            let params = ExecParams {
                command: vec![
                    path_to_codex,
                    CODEX_APPLY_PATCH_ARG1.to_string(),
                    patch.clone(),
                ],
                cwd: cwd.clone(),
                timeout_ms: params.timeout_ms,
                env: HashMap::new(),
                with_escalated_permissions: params.with_escalated_permissions,
                justification: params.justification.clone(),
            };
            let safety = if *user_explicitly_approved_this_action {
                SafetyCheck::AutoApprove {
                    sandbox_type: SandboxType::None,
                }
            } else {
                assess_safety_for_untrusted_command(
                    turn_context.approval_policy,
                    &turn_context.sandbox_policy,
                    params.with_escalated_permissions.unwrap_or(false),
                )
            };
            (
                params,
                safety,
                vec!["apply_patch".to_string(), patch.clone()],
            )
        }
        None => {
            let safety = {
                let state = sess.state.lock().await;
                assess_command_safety(
                    &params.command,
                    turn_context.approval_policy,
                    &turn_context.sandbox_policy,
                    state.approved_commands_ref(),
                    params.with_escalated_permissions.unwrap_or(false),
                )
            };
            let command_for_display = params.command.clone();
            (params, safety, command_for_display)
        }
    };

    let sandbox_type = match safety {
        SafetyCheck::AutoApprove { sandbox_type } => sandbox_type,
        SafetyCheck::AskUser => {
            let decision = sess
                .request_command_approval(
                    sub_id.clone(),
                    call_id.clone(),
                    params.command.clone(),
                    params.cwd.clone(),
                    params.justification.clone(),
                )
                .await;
            match decision {
                ReviewDecision::Approved => (),
                ReviewDecision::ApprovedForSession => {
                    sess.add_approved_command(params.command.clone()).await;
                }
                ReviewDecision::Denied | ReviewDecision::Abort => {
                    return Err(FunctionCallError::RespondToModel(
                        "exec command rejected by user".to_string(),
                    ));
                }
            }
            // No sandboxing is applied because the user has given
            // explicit approval. Often, we end up in this case because
            // the command cannot be run in a sandbox, such as
            // installing a new dependency that requires network access.
            SandboxType::None
        }
        SafetyCheck::Reject { reason } => {
            return Err(FunctionCallError::RespondToModel(format!(
                "exec command rejected: {reason:?}"
            )));
        }
    };

    let exec_command_context = ExecCommandContext {
        sub_id: sub_id.clone(),
        call_id: call_id.clone(),
        command_for_display: command_for_display.clone(),
        cwd: params.cwd.clone(),
        apply_patch: apply_patch_exec.map(
            |ApplyPatchExec {
                 action,
                 user_explicitly_approved_this_action,
             }| ApplyPatchCommandContext {
                user_explicitly_approved_this_action,
                changes: convert_apply_patch_to_protocol(&action),
            },
        ),
    };

    let params = maybe_translate_shell_command(params, sess, turn_context);
    let output_result = sess
        .run_exec_with_events(
            turn_diff_tracker,
            exec_command_context.clone(),
            ExecInvokeArgs {
                params: params.clone(),
                sandbox_type,
                sandbox_policy: &turn_context.sandbox_policy,
                sandbox_cwd: &turn_context.cwd,
                codex_linux_sandbox_exe: &sess.services.codex_linux_sandbox_exe,
                stdout_stream: if exec_command_context.apply_patch.is_some() {
                    None
                } else {
                    Some(StdoutStream {
                        sub_id: sub_id.clone(),
                        call_id: call_id.clone(),
                        tx_event: sess.tx_event.clone(),
                    })
                },
            },
        )
        .await;

    match output_result {
        Ok(output) => {
            let ExecToolCallOutput { exit_code, .. } = &output;
            let content = format_exec_output(&output);
            if *exit_code == 0 {
                Ok(content)
            } else {
                Err(FunctionCallError::RespondToModel(content))
            }
        }
        Err(CodexErr::Sandbox(error)) => {
            handle_sandbox_error(
                turn_diff_tracker,
                params,
                exec_command_context,
                error,
                sandbox_type,
                sess,
                turn_context,
            )
            .await
        }
        Err(e) => Err(FunctionCallError::RespondToModel(format!(
            "execution error: {e:?}"
        ))),
    }
}

async fn handle_sandbox_error(
    turn_diff_tracker: &mut TurnDiffTracker,
    params: ExecParams,
    exec_command_context: ExecCommandContext,
    error: SandboxErr,
    sandbox_type: SandboxType,
    sess: &Session,
    turn_context: &TurnContext,
) -> Result<String, FunctionCallError> {
    let call_id = exec_command_context.call_id.clone();
    let sub_id = exec_command_context.sub_id.clone();
    let cwd = exec_command_context.cwd.clone();

    if let SandboxErr::Timeout { output } = &error {
        let content = format_exec_output(output);
        return Err(FunctionCallError::RespondToModel(content));
    }

    // Early out if either the user never wants to be asked for approval, or
    // we're letting the model manage escalation requests. Otherwise, continue
    match turn_context.approval_policy {
        AskForApproval::Never | AskForApproval::OnRequest => {
            return Err(FunctionCallError::RespondToModel(format!(
                "failed in sandbox {sandbox_type:?} with execution error: {error:?}"
            )));
        }
        AskForApproval::UnlessTrusted | AskForApproval::OnFailure => (),
    }

    // Note that when `error` is `SandboxErr::Denied`, it could be a false
    // positive. That is, it may have exited with a non-zero exit code, not
    // because the sandbox denied it, but because that is its expected behavior,
    // i.e., a grep command that did not match anything. Ideally we would
    // include additional metadata on the command to indicate whether non-zero
    // exit codes merit a retry.

    // For now, we categorically ask the user to retry without sandbox and
    // emit the raw error as a background event.
    sess.notify_background_event(&sub_id, format!("Execution failed: {error}"))
        .await;

    let decision = sess
        .request_command_approval(
            sub_id.clone(),
            call_id.clone(),
            params.command.clone(),
            cwd.clone(),
            Some("command failed; retry without sandbox?".to_string()),
        )
        .await;

    match decision {
        ReviewDecision::Approved | ReviewDecision::ApprovedForSession => {
            // Persist this command as pre‑approved for the
            // remainder of the session so future
            // executions skip the sandbox directly.
            // TODO(ragona): Isn't this a bug? It always saves the command in an | fork?
            sess.add_approved_command(params.command.clone()).await;
            // Inform UI we are retrying without sandbox.
            sess.notify_background_event(&sub_id, "retrying command without sandbox")
                .await;

            // This is an escalated retry; the policy will not be
            // examined and the sandbox has been set to `None`.
            let retry_output_result = sess
                .run_exec_with_events(
                    turn_diff_tracker,
                    exec_command_context.clone(),
                    ExecInvokeArgs {
                        params,
                        sandbox_type: SandboxType::None,
                        sandbox_policy: &turn_context.sandbox_policy,
                        sandbox_cwd: &turn_context.cwd,
                        codex_linux_sandbox_exe: &sess.services.codex_linux_sandbox_exe,
                        stdout_stream: if exec_command_context.apply_patch.is_some() {
                            None
                        } else {
                            Some(StdoutStream {
                                sub_id: sub_id.clone(),
                                call_id: call_id.clone(),
                                tx_event: sess.tx_event.clone(),
                            })
                        },
                    },
                )
                .await;

            match retry_output_result {
                Ok(retry_output) => {
                    let ExecToolCallOutput { exit_code, .. } = &retry_output;
                    let content = format_exec_output(&retry_output);
                    if *exit_code == 0 {
                        Ok(content)
                    } else {
                        Err(FunctionCallError::RespondToModel(content))
                    }
                }
                Err(e) => Err(FunctionCallError::RespondToModel(format!(
                    "retry failed: {e}"
                ))),
            }
        }
        ReviewDecision::Denied | ReviewDecision::Abort => {
            // Fall through to original failure handling.
            Err(FunctionCallError::RespondToModel(
                "exec command rejected by user".to_string(),
            ))
        }
    }
}

fn format_exec_output_str(exec_output: &ExecToolCallOutput) -> String {
    let ExecToolCallOutput {
        aggregated_output, ..
    } = exec_output;

    // Head+tail truncation for the model: show the beginning and end with an elision.
    // Clients still receive full streams; only this formatted summary is capped.

    let mut s = &aggregated_output.text;
    let prefixed_str: String;

    if exec_output.timed_out {
        prefixed_str = format!(
            "command timed out after {} milliseconds\n",
            exec_output.duration.as_millis()
        ) + s;
        s = &prefixed_str;
    }

    let total_lines = s.lines().count();
    if s.len() <= MODEL_FORMAT_MAX_BYTES && total_lines <= MODEL_FORMAT_MAX_LINES {
        return s.to_string();
    }

    let lines: Vec<&str> = s.lines().collect();
    let head_take = MODEL_FORMAT_HEAD_LINES.min(lines.len());
    let tail_take = MODEL_FORMAT_TAIL_LINES.min(lines.len().saturating_sub(head_take));
    let omitted = lines.len().saturating_sub(head_take + tail_take);

    // Join head and tail blocks (lines() strips newlines; reinsert them)
    let head_block = lines
        .iter()
        .take(head_take)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let tail_block = if tail_take > 0 {
        lines[lines.len() - tail_take..].join("\n")
    } else {
        String::new()
    };
    let marker = format!("\n[... omitted {omitted} of {total_lines} lines ...]\n\n");

    // Byte budgets for head/tail around the marker
    let mut head_budget = MODEL_FORMAT_HEAD_BYTES.min(MODEL_FORMAT_MAX_BYTES);
    let tail_budget = MODEL_FORMAT_MAX_BYTES.saturating_sub(head_budget + marker.len());
    if tail_budget == 0 && marker.len() >= MODEL_FORMAT_MAX_BYTES {
        // Degenerate case: marker alone exceeds budget; return a clipped marker
        return take_bytes_at_char_boundary(&marker, MODEL_FORMAT_MAX_BYTES).to_string();
    }
    if tail_budget == 0 {
        // Make room for the marker by shrinking head
        head_budget = MODEL_FORMAT_MAX_BYTES.saturating_sub(marker.len());
    }

    // Enforce line-count cap by trimming head/tail lines
    let head_lines_text = head_block;
    let tail_lines_text = tail_block;
    // Build final string respecting byte budgets
    let head_part = take_bytes_at_char_boundary(&head_lines_text, head_budget);
    let mut result = String::with_capacity(MODEL_FORMAT_MAX_BYTES.min(s.len()));

    result.push_str(head_part);
    result.push_str(&marker);

    let remaining = MODEL_FORMAT_MAX_BYTES.saturating_sub(result.len());
    let tail_budget_final = remaining;
    let tail_part = take_last_bytes_at_char_boundary(&tail_lines_text, tail_budget_final);
    result.push_str(tail_part);

    result
}

// Truncate a &str to a byte budget at a char boundary (prefix)
#[inline]
fn take_bytes_at_char_boundary(s: &str, maxb: usize) -> &str {
    if s.len() <= maxb {
        return s;
    }
    let mut last_ok = 0;
    for (i, ch) in s.char_indices() {
        let nb = i + ch.len_utf8();
        if nb > maxb {
            break;
        }
        last_ok = nb;
    }
    &s[..last_ok]
}

// Take a suffix of a &str within a byte budget at a char boundary
#[inline]
fn take_last_bytes_at_char_boundary(s: &str, maxb: usize) -> &str {
    if s.len() <= maxb {
        return s;
    }
    let mut start = s.len();
    let mut used = 0usize;
    for (i, ch) in s.char_indices().rev() {
        let nb = ch.len_utf8();
        if used + nb > maxb {
            break;
        }
        start = i;
        used += nb;
        if start == 0 {
            break;
        }
    }
    &s[start..]
}

/// Exec output is a pre-serialized JSON payload
fn format_exec_output(exec_output: &ExecToolCallOutput) -> String {
    let ExecToolCallOutput {
        exit_code,
        duration,
        ..
    } = exec_output;

    #[derive(Serialize)]
    struct ExecMetadata {
        exit_code: i32,
        duration_seconds: f32,
    }

    #[derive(Serialize)]
    struct ExecOutput<'a> {
        output: &'a str,
        metadata: ExecMetadata,
    }

    // round to 1 decimal place
    let duration_seconds = ((duration.as_secs_f32()) * 10.0).round() / 10.0;

    let formatted_output = format_exec_output_str(exec_output);

    let payload = ExecOutput {
        output: &formatted_output,
        metadata: ExecMetadata {
            exit_code: *exit_code,
            duration_seconds,
        },
    };

    #[expect(clippy::expect_used)]
    serde_json::to_string(&payload).expect("serialize ExecOutput")
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
fn convert_call_tool_result_to_function_call_output_payload(
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
    task_sub_id: String,
    review_output: Option<ReviewOutputEvent>,
) {
    let event = Event {
        id: task_sub_id,
        msg: EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
            review_output: review_output.clone(),
        }),
    };
    session.send_event(event).await;

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

#[cfg(test)]
pub(crate) use tests::make_session_and_context;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigOverrides;
    use crate::config::ConfigToml;
    use crate::protocol::CompactedItem;
    use crate::protocol::InitialHistory;
    use crate::protocol::ResumedHistory;
    use crate::state::TaskKind;
    use crate::tasks::SessionTask;
    use crate::tasks::SessionTaskContext;
    use codex_protocol::models::ContentItem;
    use mcp_types::ContentBlock;
    use mcp_types::TextContent;
    use pretty_assertions::assert_eq;
    use serde::Deserialize;
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration as StdDuration;
    use tokio::time::Duration;
    use tokio::time::sleep;

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

        tokio_test::block_on(session.record_initial_history(
            &turn_context,
            InitialHistory::Resumed(ResumedHistory {
                conversation_id: ConversationId::default(),
                history: rollout_items,
                rollout_path: PathBuf::from("/tmp/resume.jsonl"),
            }),
        ));

        let actual = tokio_test::block_on(async { session.state.lock().await.history_snapshot() });
        assert_eq!(expected, actual);
    }

    #[test]
    fn record_initial_history_reconstructs_forked_transcript() {
        let (session, turn_context) = make_session_and_context();
        let (rollout_items, expected) = sample_rollout(&session, &turn_context);

        tokio_test::block_on(
            session.record_initial_history(&turn_context, InitialHistory::Forked(rollout_items)),
        );

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

        // Expect elision marker with correct counts
        let omitted = 400 - MODEL_FORMAT_MAX_LINES; // 144
        let marker = format!("\n[... omitted {omitted} of 400 lines ...]\n\n");
        assert!(out.contains(&marker), "missing marker: {out}");

        // Validate head and tail
        let parts: Vec<&str> = out.split(&marker).collect();
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
        assert!(out.len() <= MODEL_FORMAT_MAX_BYTES, "exceeds byte budget");
        assert!(out.contains("omitted"), "should contain elision marker");

        // Ensure head and tail are drawn from the original
        assert!(full.starts_with(out.chars().take(8).collect::<String>().as_str()));
        assert!(
            full.ends_with(
                out.chars()
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
        let client = ModelClient::new(
            config.clone(),
            None,
            config.model_provider.clone(),
            config.model_reasoning_effort,
            config.model_reasoning_summary,
            conversation_id,
        );
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_family: &config.model_family,
            include_plan_tool: config.include_plan_tool,
            include_apply_patch_tool: config.include_apply_patch_tool,
            include_web_search_request: config.tools_web_search_request,
            use_streamable_shell_tool: config.use_experimental_streamable_shell_tool,
            include_view_image_tool: config.include_view_image_tool,
            experimental_unified_exec_tool: config.use_experimental_unified_exec_tool,
        });
        let turn_context = TurnContext {
            client,
            cwd: config.cwd.clone(),
            base_instructions: config.base_instructions.clone(),
            user_instructions: config.user_instructions.clone(),
            approval_policy: config.approval_policy,
            sandbox_policy: config.sandbox_policy.clone(),
            shell_environment_policy: config.shell_environment_policy.clone(),
            tools_config,
            is_review_mode: false,
            final_output_json_schema: None,
        };
        let services = SessionServices {
            mcp_connection_manager: McpConnectionManager::default(),
            session_manager: ExecSessionManager::default(),
            unified_exec_manager: UnifiedExecSessionManager::default(),
            notifier: UserNotifier::default(),
            rollout: Mutex::new(None),
            codex_linux_sandbox_exe: None,
            user_shell: shell::Shell::Unknown,
            show_raw_agent_reasoning: config.show_raw_agent_reasoning,
        };
        let session = Session {
            conversation_id,
            tx_event,
            state: Mutex::new(SessionState::new()),
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
        let client = ModelClient::new(
            config.clone(),
            None,
            config.model_provider.clone(),
            config.model_reasoning_effort,
            config.model_reasoning_summary,
            conversation_id,
        );
        let tools_config = ToolsConfig::new(&ToolsConfigParams {
            model_family: &config.model_family,
            include_plan_tool: config.include_plan_tool,
            include_apply_patch_tool: config.include_apply_patch_tool,
            include_web_search_request: config.tools_web_search_request,
            use_streamable_shell_tool: config.use_experimental_streamable_shell_tool,
            include_view_image_tool: config.include_view_image_tool,
            experimental_unified_exec_tool: config.use_experimental_unified_exec_tool,
        });
        let turn_context = Arc::new(TurnContext {
            client,
            cwd: config.cwd.clone(),
            base_instructions: config.base_instructions.clone(),
            user_instructions: config.user_instructions.clone(),
            approval_policy: config.approval_policy,
            sandbox_policy: config.sandbox_policy.clone(),
            shell_environment_policy: config.shell_environment_policy.clone(),
            tools_config,
            is_review_mode: false,
            final_output_json_schema: None,
        });
        let services = SessionServices {
            mcp_connection_manager: McpConnectionManager::default(),
            session_manager: ExecSessionManager::default(),
            unified_exec_manager: UnifiedExecSessionManager::default(),
            notifier: UserNotifier::default(),
            rollout: Mutex::new(None),
            codex_linux_sandbox_exe: None,
            user_shell: shell::Shell::Unknown,
            show_raw_agent_reasoning: config.show_raw_agent_reasoning,
        };
        let session = Arc::new(Session {
            conversation_id,
            tx_event,
            state: Mutex::new(SessionState::new()),
            active_turn: Mutex::new(None),
            services,
            next_internal_sub_id: AtomicU64::new(0),
        });
        (session, turn_context, rx_event)
    }

    #[derive(Clone, Copy)]
    struct NeverEndingTask(TaskKind);

    #[async_trait::async_trait]
    impl SessionTask for NeverEndingTask {
        fn kind(&self) -> TaskKind {
            self.0
        }

        async fn run(
            self: Arc<Self>,
            _session: Arc<SessionTaskContext>,
            _ctx: Arc<TurnContext>,
            _sub_id: String,
            _input: Vec<InputItem>,
        ) -> Option<String> {
            loop {
                sleep(Duration::from_secs(60)).await;
            }
        }

        async fn abort(&self, session: Arc<SessionTaskContext>, sub_id: &str) {
            if let TaskKind::Review = self.0 {
                exit_review_mode(session.clone_session(), sub_id.to_string(), None).await;
            }
        }
    }

    #[tokio::test]
    async fn abort_regular_task_emits_turn_aborted_only() {
        let (sess, tc, rx) = make_session_and_context_with_rx();
        let sub_id = "sub-regular".to_string();
        let input = vec![InputItem::Text {
            text: "hello".to_string(),
        }];
        sess.spawn_task(
            Arc::clone(&tc),
            sub_id.clone(),
            input,
            NeverEndingTask(TaskKind::Regular),
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

    #[tokio::test]
    async fn abort_review_task_emits_exited_then_aborted_and_records_history() {
        let (sess, tc, rx) = make_session_and_context_with_rx();
        let sub_id = "sub-review".to_string();
        let input = vec![InputItem::Text {
            text: "start review".to_string(),
        }];
        sess.spawn_task(
            Arc::clone(&tc),
            sub_id.clone(),
            input,
            NeverEndingTask(TaskKind::Review),
        )
        .await;

        sess.abort_all_tasks(TurnAbortReason::Interrupted).await;

        let first = rx.recv().await.expect("first event");
        match first.msg {
            EventMsg::ExitedReviewMode(ev) => assert!(ev.review_output.is_none()),
            other => panic!("unexpected first event: {other:?}"),
        }
        let second = rx.recv().await.expect("second event");
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
        let snapshot1 = live_history.contents();
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
        let snapshot2 = live_history.contents();
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

        (rollout_items, live_history.contents())
    }

    #[tokio::test]
    async fn rejects_escalated_permissions_when_policy_not_on_request() {
        use crate::exec::ExecParams;
        use crate::protocol::AskForApproval;
        use crate::protocol::SandboxPolicy;
        use crate::turn_diff_tracker::TurnDiffTracker;
        use std::collections::HashMap;

        let (session, mut turn_context) = make_session_and_context();
        // Ensure policy is NOT OnRequest so the early rejection path triggers
        turn_context.approval_policy = AskForApproval::OnFailure;

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
        };

        let params2 = ExecParams {
            with_escalated_permissions: Some(false),
            ..params.clone()
        };

        let mut turn_diff_tracker = TurnDiffTracker::new();

        let sub_id = "test-sub".to_string();
        let call_id = "test-call".to_string();

        let resp = handle_container_exec_with_params(
            params,
            &session,
            &turn_context,
            &mut turn_diff_tracker,
            sub_id,
            call_id,
        )
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
        turn_context.sandbox_policy = SandboxPolicy::DangerFullAccess;

        let resp2 = handle_container_exec_with_params(
            params2,
            &session,
            &turn_context,
            &mut turn_diff_tracker,
            "test-sub".to_string(),
            "test-call-2".to_string(),
        )
        .await;

        let output = resp2.expect("expected Ok result");

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
}
