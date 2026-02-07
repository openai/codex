//! Agent loop driver - the core 18-step conversation loop.

use std::sync::Arc;
use std::time::Instant;

use cocode_api::ApiClient;
use cocode_api::CollectedResponse;
use cocode_api::ModelHub;
use cocode_api::QueryResultType;
use cocode_api::RequestBuilder;
use cocode_api::StreamOptions;
use cocode_context::ConversationContext;
use cocode_hooks::AsyncHookTracker;
use cocode_hooks::HookRegistry;
use cocode_message::MessageHistory;
use cocode_message::TrackedMessage;
use cocode_message::Turn;
use cocode_prompt::SystemPromptBuilder;
use cocode_protocol::AgentStatus;
use cocode_protocol::AutoCompactTracking;
use cocode_protocol::CompactConfig;
use cocode_protocol::ContextModifier;
use cocode_protocol::HookEventType;
use cocode_protocol::LoopConfig;
use cocode_protocol::LoopEvent;
use cocode_protocol::QueryTracking;
use cocode_protocol::RoleSelections;
use cocode_protocol::TokenUsage;
use cocode_protocol::ToolResultContent;
use cocode_skill::SkillManager;
use cocode_system_reminder::FileTracker;
use cocode_system_reminder::GeneratorContext;
use cocode_system_reminder::InjectedBlock;
use cocode_system_reminder::InjectedMessage;
use cocode_system_reminder::QueuedCommandInfo;
use cocode_system_reminder::SystemReminderConfig;
use cocode_system_reminder::SystemReminderOrchestrator;
use cocode_system_reminder::create_injected_messages;
use cocode_system_reminder::generators::ASYNC_HOOK_RESPONSES_KEY;
use cocode_system_reminder::generators::AVAILABLE_SKILLS_KEY;
use cocode_system_reminder::generators::AsyncHookResponseInfo;
use cocode_system_reminder::generators::HOOK_BLOCKING_KEY;
use cocode_system_reminder::generators::HOOK_CONTEXT_KEY;
use cocode_system_reminder::generators::HookBlockingInfo;
use cocode_system_reminder::generators::HookContextInfo;
use cocode_system_reminder::generators::SkillInfo;
use cocode_tools::ApprovalStore;
use cocode_tools::ExecutorConfig;
use cocode_tools::FileReadState;
use cocode_tools::FileTracker as ToolsFileTracker;
use cocode_tools::SpawnAgentFn;
use cocode_tools::StreamingToolExecutor;
use cocode_tools::ToolExecutionResult;
use cocode_tools::ToolRegistry;
use hyper_sdk::ContentBlock;
use hyper_sdk::FinishReason;
use hyper_sdk::Message;
use hyper_sdk::ToolCall;
use hyper_sdk::ToolDefinition;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::compaction::CompactionConfig;
use crate::compaction::FileRestoration;
use crate::compaction::InvokedSkillRestoration;
use crate::compaction::SessionMemorySummary;
use crate::compaction::TaskStatusRestoration;
use crate::compaction::ThresholdStatus;
use crate::compaction::build_compact_instructions;
use crate::compaction::build_context_restoration_with_config;
use crate::compaction::calculate_keep_start_index;
use crate::compaction::format_restoration_message;
use crate::compaction::map_message_index_to_keep_turns;
use crate::compaction::try_session_memory_compact;
use crate::compaction::write_session_memory;
use crate::fallback::FallbackConfig;
use crate::fallback::FallbackState;
use crate::result::LoopResult;
use crate::session_memory_agent::SessionMemoryExtractionAgent;
use cocode_plan_mode::PlanModeState;

/// Maximum number of retry attempts for output-token exhaustion recovery.
const MAX_OUTPUT_TOKEN_RECOVERY: i32 = 3;

/// The main agent loop that drives multi-turn conversations with LLM providers.
///
/// `AgentLoop` manages streaming API calls, concurrent tool execution,
/// context compaction, model fallback, and event emission.
pub struct AgentLoop {
    // Provider / model
    api_client: ApiClient,
    /// Model hub for unified model resolution.
    ///
    /// Provides model acquisition and caching. Note: ModelHub is role-agnostic;
    /// role resolution uses `selections` which are passed to ModelHub methods.
    model_hub: Arc<ModelHub>,
    /// Role selections for this agent loop.
    ///
    /// Owned by the loop (cloned from Session at creation time). This enables
    /// proper isolation: subagents get their own copy and are unaffected by
    /// changes to the parent's model settings.
    selections: RoleSelections,

    // Tool system
    tool_registry: Arc<ToolRegistry>,

    // Conversation state
    message_history: MessageHistory,
    context: ConversationContext,

    // Config
    config: LoopConfig,
    fallback_config: FallbackConfig,
    compaction_config: CompactionConfig,
    /// Protocol-level compact configuration with all threshold constants.
    compact_config: CompactConfig,

    // System reminders
    reminder_orchestrator: SystemReminderOrchestrator,
    /// FileTracker for system reminders (change detection).
    file_tracker: FileTracker,
    /// Shared FileTracker for tool execution (persists across turns).
    /// This is synced to `file_tracker` before generating reminders.
    shared_tools_file_tracker: Arc<tokio::sync::Mutex<ToolsFileTracker>>,
    /// Shared ApprovalStore for tool execution (persists across turns).
    shared_approval_store: Arc<tokio::sync::Mutex<ApprovalStore>>,

    // Hooks
    hooks: Arc<HookRegistry>,
    /// Shared async hook tracker (persists across turns for background hooks).
    async_hook_tracker: Arc<AsyncHookTracker>,

    // Event channel
    event_tx: mpsc::Sender<LoopEvent>,

    // State tracking
    turn_number: i32,
    cancel_token: CancellationToken,
    fallback_state: FallbackState,
    total_input_tokens: i32,
    total_output_tokens: i32,

    // Background extraction agent (optional)
    extraction_agent: Option<Arc<SessionMemoryExtractionAgent>>,

    // Agent type tracking (for tier filtering in system reminders)
    /// Whether this is a subagent (spawned by Task tool).
    /// When true, MainAgentOnly tier reminders are skipped.
    is_subagent: bool,
    /// Whether the current turn has user input.
    /// When false, UserPrompt tier reminders are skipped.
    current_turn_has_user_input: bool,

    // Plan mode tracking
    /// Plan mode state for the session.
    plan_mode_state: PlanModeState,

    // Subagent spawning
    /// Optional callback for spawning subagents (used by Task tool).
    spawn_agent_fn: Option<SpawnAgentFn>,

    // Skill system
    /// Optional skill manager for loading and executing skills.
    skill_manager: Option<Arc<SkillManager>>,

    // Real-time steering
    /// Queued commands from user (Enter during streaming).
    /// These are injected as steering reminders and executed after idle.
    queued_commands: Vec<QueuedCommandInfo>,

    // Status broadcast
    /// Watch channel sender for broadcasting agent status.
    /// This allows efficient status polling without processing all events.
    status_tx: watch::Sender<AgentStatus>,
}

/// Builder for constructing an [`AgentLoop`].
pub struct AgentLoopBuilder {
    api_client: Option<ApiClient>,
    model_hub: Option<Arc<ModelHub>>,
    selections: Option<RoleSelections>,
    tool_registry: Option<Arc<ToolRegistry>>,
    message_history: Option<MessageHistory>,
    context: Option<ConversationContext>,
    config: LoopConfig,
    fallback_config: FallbackConfig,
    compaction_config: CompactionConfig,
    compact_config: CompactConfig,
    system_reminder_config: SystemReminderConfig,
    hooks: Option<Arc<HookRegistry>>,
    event_tx: Option<mpsc::Sender<LoopEvent>>,
    cancel_token: CancellationToken,
    extraction_agent: Option<Arc<SessionMemoryExtractionAgent>>,
    is_subagent: bool,
    plan_mode_state: Option<PlanModeState>,
    spawn_agent_fn: Option<SpawnAgentFn>,
    skill_manager: Option<Arc<SkillManager>>,
    queued_commands: Vec<QueuedCommandInfo>,
    status_tx: Option<watch::Sender<AgentStatus>>,
}

impl AgentLoopBuilder {
    /// Create a new builder with default configuration.
    pub fn new() -> Self {
        Self {
            api_client: None,
            model_hub: None,
            selections: None,
            tool_registry: None,
            message_history: None,
            context: None,
            config: LoopConfig::default(),
            fallback_config: FallbackConfig::default(),
            compaction_config: CompactionConfig::default(),
            compact_config: CompactConfig::default(),
            system_reminder_config: SystemReminderConfig::default(),
            hooks: None,
            event_tx: None,
            cancel_token: CancellationToken::new(),
            extraction_agent: None,
            is_subagent: false,
            plan_mode_state: None,
            spawn_agent_fn: None,
            skill_manager: None,
            queued_commands: Vec::new(),
            status_tx: None,
        }
    }

    pub fn api_client(mut self, client: ApiClient) -> Self {
        self.api_client = Some(client);
        self
    }

    /// Set the model hub for model acquisition and caching.
    ///
    /// The hub provides:
    /// - Provider and model caching
    /// - `InferenceContext` for request building
    ///
    /// Note: ModelHub is role-agnostic. Use `selections()` to set role mappings.
    pub fn model_hub(mut self, hub: Arc<ModelHub>) -> Self {
        self.model_hub = Some(hub);
        self
    }

    /// Set the role selections for this agent loop.
    ///
    /// Selections map roles (Main, Fast, Plan, etc.) to model specs and thinking levels.
    /// This should be cloned from the Session at creation time. Subagents receive
    /// their own copy, isolating them from future changes to the parent's settings.
    pub fn selections(mut self, selections: RoleSelections) -> Self {
        self.selections = Some(selections);
        self
    }

    pub fn tool_registry(mut self, registry: Arc<ToolRegistry>) -> Self {
        self.tool_registry = Some(registry);
        self
    }

    pub fn message_history(mut self, history: MessageHistory) -> Self {
        self.message_history = Some(history);
        self
    }

    pub fn context(mut self, ctx: ConversationContext) -> Self {
        self.context = Some(ctx);
        self
    }

    pub fn config(mut self, config: LoopConfig) -> Self {
        self.config = config;
        self
    }

    pub fn fallback_config(mut self, config: FallbackConfig) -> Self {
        self.fallback_config = config;
        self
    }

    pub fn compaction_config(mut self, config: CompactionConfig) -> Self {
        self.compaction_config = config;
        self
    }

    /// Set the protocol-level compact configuration.
    pub fn compact_config(mut self, config: CompactConfig) -> Self {
        self.compact_config = config;
        self
    }

    /// Set the system reminder configuration.
    pub fn system_reminder_config(mut self, config: SystemReminderConfig) -> Self {
        self.system_reminder_config = config;
        self
    }

    pub fn hooks(mut self, hooks: Arc<HookRegistry>) -> Self {
        self.hooks = Some(hooks);
        self
    }

    pub fn event_tx(mut self, tx: mpsc::Sender<LoopEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    pub fn cancel_token(mut self, token: CancellationToken) -> Self {
        self.cancel_token = token;
        self
    }

    /// Set the background session memory extraction agent.
    pub fn extraction_agent(mut self, agent: Arc<SessionMemoryExtractionAgent>) -> Self {
        self.extraction_agent = Some(agent);
        self
    }

    /// Mark this loop as a subagent (spawned via Task tool).
    ///
    /// Subagents skip MainAgentOnly tier system reminders.
    pub fn is_subagent(mut self, is_subagent: bool) -> Self {
        self.is_subagent = is_subagent;
        self
    }

    /// Set initial plan mode state (for session resumption).
    pub fn plan_mode_state(mut self, state: PlanModeState) -> Self {
        self.plan_mode_state = Some(state);
        self
    }

    /// Set the spawn agent callback for the Task tool.
    pub fn spawn_agent_fn(mut self, f: SpawnAgentFn) -> Self {
        self.spawn_agent_fn = Some(f);
        self
    }

    /// Set the skill manager for loading and executing skills.
    pub fn skill_manager(mut self, manager: Arc<SkillManager>) -> Self {
        self.skill_manager = Some(manager);
        self
    }

    /// Set initial queued commands (for real-time steering).
    ///
    /// These commands are injected as `<system-reminder>User sent: {message}</system-reminder>`
    /// to steer the model in real-time, and also executed as new turns after idle.
    pub fn queued_commands(mut self, commands: Vec<QueuedCommandInfo>) -> Self {
        self.queued_commands = commands;
        self
    }

    /// Set the status watch channel sender.
    ///
    /// This enables efficient status polling without processing all events.
    /// If not set, a new channel will be created internally (the receiver
    /// will be accessible via `AgentLoop::status_receiver()`).
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tokio::sync::watch;
    /// use cocode_protocol::AgentStatus;
    ///
    /// let (status_tx, status_rx) = watch::channel(AgentStatus::default());
    /// let loop_builder = AgentLoop::builder()
    ///     .status_tx(status_tx)
    ///     // ... other config
    ///     .build();
    /// // status_rx can be used to poll status efficiently
    /// ```
    pub fn status_tx(mut self, tx: watch::Sender<AgentStatus>) -> Self {
        self.status_tx = Some(tx);
        self
    }

    /// Build the [`AgentLoop`].
    ///
    /// # Panics
    /// Panics if required fields (`api_client`, `tool_registry`,
    /// `context`, `event_tx`, `model_hub`, `selections`) have not been set.
    pub fn build(self) -> AgentLoop {
        let model_name = self
            .config
            .fallback_model
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        // Create system reminder components
        let reminder_orchestrator = SystemReminderOrchestrator::new(self.system_reminder_config);
        let file_tracker = FileTracker::new();
        // Create shared file tracker for tool execution (persists across turns)
        let shared_tools_file_tracker = Arc::new(tokio::sync::Mutex::new(ToolsFileTracker::new()));
        // Create shared approval store for tool execution (persists across turns)
        let shared_approval_store = Arc::new(tokio::sync::Mutex::new(ApprovalStore::new()));

        // Create status channel if not provided
        let status_tx = self
            .status_tx
            .unwrap_or_else(|| watch::channel(AgentStatus::default()).0);

        AgentLoop {
            api_client: self.api_client.expect("api_client is required"),
            model_hub: self.model_hub.expect("model_hub is required"),
            selections: self.selections.expect("selections is required"),
            tool_registry: self.tool_registry.expect("tool_registry is required"),
            message_history: self.message_history.unwrap_or_default(),
            context: self.context.expect("context is required"),
            config: self.config,
            fallback_config: self.fallback_config,
            compaction_config: self.compaction_config,
            compact_config: self.compact_config,
            reminder_orchestrator,
            file_tracker,
            shared_tools_file_tracker,
            shared_approval_store,
            hooks: self.hooks.unwrap_or_else(|| Arc::new(HookRegistry::new())),
            async_hook_tracker: Arc::new(AsyncHookTracker::new()),
            event_tx: self.event_tx.expect("event_tx is required"),
            turn_number: 0,
            cancel_token: self.cancel_token,
            fallback_state: FallbackState::new(model_name),
            total_input_tokens: 0,
            total_output_tokens: 0,
            extraction_agent: self.extraction_agent,
            is_subagent: self.is_subagent,
            // Initially true - the first turn always has user input
            current_turn_has_user_input: true,
            plan_mode_state: self.plan_mode_state.unwrap_or_default(),
            spawn_agent_fn: self.spawn_agent_fn,
            skill_manager: self.skill_manager,
            queued_commands: self.queued_commands,
            status_tx,
        }
    }
}

impl Default for AgentLoopBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentLoop {
    /// Create a builder for constructing an agent loop.
    pub fn builder() -> AgentLoopBuilder {
        AgentLoopBuilder::new()
    }

    /// Queue a command for real-time steering.
    ///
    /// Queued commands serve dual purpose:
    /// 1. Injected as `<system-reminder>User sent: {message}</system-reminder>` immediately
    /// 2. Executed as new user turns after the current turn completes
    ///
    /// This matches Claude Code's behavior where Enter during streaming both
    /// steers the current turn and queues for later execution.
    pub fn queue_command(&mut self, prompt: impl Into<String>) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let cmd = QueuedCommandInfo {
            id: uuid::Uuid::new_v4().to_string(),
            prompt: prompt.into(),
            queued_at: now,
        };
        self.queued_commands.push(cmd);
    }

    /// Get the current queued commands (for processing after idle).
    pub fn take_queued_commands(&mut self) -> Vec<QueuedCommandInfo> {
        std::mem::take(&mut self.queued_commands)
    }

    /// Get the number of queued commands.
    pub fn queued_count(&self) -> usize {
        self.queued_commands.len()
    }

    /// Subscribe to status updates.
    ///
    /// Returns a watch receiver that can be used to efficiently poll
    /// the current agent status without processing all events.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut status_rx = agent_loop.subscribe_status();
    /// loop {
    ///     let status = status_rx.borrow().clone();
    ///     if status.is_busy() {
    ///         println!("Agent is busy: {status}");
    ///     }
    ///     status_rx.changed().await.ok();
    /// }
    /// ```
    pub fn subscribe_status(&self) -> watch::Receiver<AgentStatus> {
        self.status_tx.subscribe()
    }

    /// Get the current agent status.
    pub fn current_status(&self) -> AgentStatus {
        self.status_tx.borrow().clone()
    }

    /// Update the agent status.
    ///
    /// This is called internally at key state transitions.
    fn set_status(&self, status: AgentStatus) {
        // Ignore send errors - if all receivers are dropped, that's fine
        let _ = self.status_tx.send(status);
    }

    /// Run the agent loop to completion, starting with an initial user message.
    ///
    /// Returns a `LoopResult` describing how the loop terminated along with
    /// aggregate token usage and the final response text.
    pub async fn run(&mut self, initial_message: &str) -> anyhow::Result<LoopResult> {
        info!(
            max_turns = ?self.config.max_turns,
            "Starting agent loop"
        );

        // Add user message to history
        let turn_id = uuid::Uuid::new_v4().to_string();
        let user_msg = TrackedMessage::user(initial_message, &turn_id);
        let turn = Turn::new(1, user_msg);
        self.message_history.add_turn(turn);

        // Mark that this turn has user input (new conversation start)
        self.current_turn_has_user_input = true;

        // Initialize tracking
        let mut query_tracking = QueryTracking::new_root(uuid::Uuid::new_v4().to_string());
        let mut auto_compact_tracking = AutoCompactTracking::new();

        self.core_message_loop(&mut query_tracking, &mut auto_compact_tracking)
            .await
    }

    /// Continue the conversation with a new user message.
    ///
    /// This is used to process queued commands after the agent becomes idle.
    /// Unlike `run`, this continues an existing conversation rather than starting
    /// a new one.
    pub async fn continue_with_message(&mut self, message: &str) -> anyhow::Result<LoopResult> {
        info!("Continuing conversation with new message");

        // Add user message to history
        let turn_id = uuid::Uuid::new_v4().to_string();
        let user_msg = TrackedMessage::user(message, &turn_id);
        let next_turn_number = self.message_history.turn_count() as i32 + 1;
        let turn = Turn::new(next_turn_number, user_msg);
        self.message_history.add_turn(turn);

        // Mark that this turn has user input
        self.current_turn_has_user_input = true;

        // Initialize tracking for this continuation
        let mut query_tracking = QueryTracking::new_root(uuid::Uuid::new_v4().to_string());
        let mut auto_compact_tracking = AutoCompactTracking::new();

        self.core_message_loop(&mut query_tracking, &mut auto_compact_tracking)
            .await
    }

    /// Run the agent loop and then process any queued commands.
    ///
    /// This implements Claude Code's dual-purpose queue mechanism:
    /// 1. Queued commands are injected as steering during the initial run
    /// 2. After idle, remaining queued commands are executed as new user turns
    ///
    /// Returns the last result from processing (or the initial result if no queued commands).
    pub async fn run_and_process_queue(
        &mut self,
        initial_message: &str,
    ) -> anyhow::Result<LoopResult> {
        // Run the initial message
        let mut result = self.run(initial_message).await?;

        // After idle, process any queued commands as new user turns
        // This matches Claude Code's useQueuedCommandsProcessor behavior
        while !self.queued_commands.is_empty() {
            // Take the first queued command
            let cmd = self.queued_commands.remove(0);

            info!(
                prompt = %cmd.prompt,
                remaining = self.queued_commands.len(),
                "Processing queued command after idle"
            );

            // Clear queued commands before processing to avoid re-injection
            // The command is being processed as a new turn now
            // Note: New commands queued during this turn will be processed next iteration

            // Process as a new user turn
            result = self.continue_with_message(&cmd.prompt).await?;

            // Check if the loop was interrupted or errored
            match &result.stop_reason {
                crate::result::StopReason::UserInterrupted
                | crate::result::StopReason::Error { .. }
                | crate::result::StopReason::HookStopped => break,
                _ => {}
            }
        }

        Ok(result)
    }

    /// The 18-step core message loop.
    ///
    /// This implements the algorithm from `docs/arch/core-loop.md`:
    ///
    /// SETUP (1-6): emit events, query tracking, normalize, micro-compact,
    ///   auto-compact, init state.
    /// EXECUTION (7-10): resolve model, check token limit, stream with tools
    ///   + retry, record telemetry.
    /// POST-PROCESSING (11-18): check tool calls, execute queue, abort handling,
    ///   hooks, tracking, queued commands, max turns, recurse.
    async fn core_message_loop(
        &mut self,
        query_tracking: &mut QueryTracking,
        auto_compact_tracking: &mut AutoCompactTracking,
    ) -> anyhow::Result<LoopResult> {
        // ── STEP 1: Signal stream_request_start ──
        self.emit(LoopEvent::StreamRequestStart).await;

        // ── STEP 2: Setup query tracking ──
        query_tracking.depth += 1;
        let turn_id = uuid::Uuid::new_v4().to_string();

        // ── STEP 3: Normalize messages ──
        // Messages are already normalized through MessageHistory::messages_for_api().

        // ── STEP 4: Micro-compaction (PRE-API) ──
        if self.config.enable_micro_compaction {
            let (removed, tokens_saved) = self.micro_compact();
            if removed > 0 {
                self.emit(LoopEvent::MicroCompactionApplied {
                    removed_results: removed,
                    tokens_saved,
                })
                .await;
            }
        }

        // ── STEP 5: Auto-compaction check ──
        // Use ThresholdStatus for accurate threshold calculations
        let estimated_tokens = self.message_history.estimate_tokens();
        let context_window = self.context.environment.context_window;

        // Apply safety margin to token estimate
        let estimated_with_margin = self
            .compact_config
            .estimate_tokens_with_margin(estimated_tokens);

        let status =
            ThresholdStatus::calculate(estimated_with_margin, context_window, &self.compact_config);

        debug!(
            estimated_tokens,
            estimated_with_margin,
            context_window,
            percent_left = %format!("{:.1}%", status.percent_left * 100.0),
            status = status.status_description(),
            "Context usage check"
        );

        // Emit warning event if above warning but below auto-compact
        if status.is_above_warning_threshold && !status.is_above_auto_compact_threshold {
            let target = self.compact_config.auto_compact_target(context_window);
            let warning_threshold = self.compact_config.warning_threshold(target);
            self.emit(LoopEvent::ContextUsageWarning {
                estimated_tokens: estimated_with_margin,
                warning_threshold,
                percent_left: status.percent_left,
            })
            .await;
        }

        // Trigger auto-compact if above threshold (and auto-compact is enabled)
        if status.is_above_auto_compact_threshold && self.compact_config.is_auto_compact_enabled() {
            // Tier 1: Try session memory first (zero API cost)
            // Only if session memory compact is enabled
            if self.compaction_config.session_memory.enable_sm_compact {
                if let Some(summary) =
                    try_session_memory_compact(&self.compaction_config.session_memory)
                {
                    self.apply_session_memory_summary(summary, &turn_id, auto_compact_tracking)
                        .await?;
                } else {
                    // Tier 2: Fall back to LLM-based compaction
                    self.compact(auto_compact_tracking, &turn_id, query_tracking)
                        .await?;
                }
            } else {
                // Session memory compact disabled, go directly to Tier 2
                debug!("Session memory compact disabled, using LLM-based compaction");
                self.compact(auto_compact_tracking, &turn_id, query_tracking)
                    .await?;
            }
        }

        // ── STEP 6: Initialize state ──
        self.turn_number += 1;
        // Update status to streaming
        self.set_status(AgentStatus::streaming(turn_id.clone()));
        self.emit(LoopEvent::TurnStarted {
            turn_id: turn_id.clone(),
            turn_number: self.turn_number,
        })
        .await;

        // ── STEP 6.5: Generate system reminders ──
        // System reminders provide dynamic context (file changes, plan mode, etc.)
        // that is visible to the model but hidden from the user.
        //
        // First, sync file read state from tools' FileTracker to system-reminder's FileTracker.
        // This ensures the ChangedFilesGenerator can detect files that have been modified
        // since they were last read by tools (Read, Glob, etc.).
        self.sync_file_trackers().await;

        // Collect completed async hooks from previous turns
        let completed_hooks = self.async_hook_tracker.take_completed();
        let async_responses: Vec<AsyncHookResponseInfo> = completed_hooks
            .iter()
            .map(|h| AsyncHookResponseInfo {
                hook_name: h.hook_name.clone(),
                additional_context: h.additional_context.clone(),
                was_blocking: h.was_blocking,
                blocking_reason: h.blocking_reason.clone(),
                duration_ms: h.duration_ms,
            })
            .collect();

        // Separate blocking and context hooks for their dedicated generators
        let blocking_hooks: Vec<HookBlockingInfo> = completed_hooks
            .iter()
            .filter(|h| h.was_blocking)
            .map(|h| HookBlockingInfo {
                hook_name: h.hook_name.clone(),
                event_type: "async".to_string(),
                tool_name: None,
                reason: h
                    .blocking_reason
                    .clone()
                    .unwrap_or_else(|| "Hook blocked execution".to_string()),
            })
            .collect();

        let context_hooks: Vec<HookContextInfo> = completed_hooks
            .into_iter()
            .filter(|h| h.additional_context.is_some() && !h.was_blocking)
            .map(|h| HookContextInfo {
                hook_name: h.hook_name,
                event_type: "async".to_string(),
                tool_name: None,
                additional_context: h.additional_context.unwrap_or_default(),
            })
            .collect();

        let reminder_config = self.reminder_orchestrator.config();
        let mut gen_ctx_builder = GeneratorContext::builder()
            .config(reminder_config)
            .turn_number(self.turn_number)
            .is_main_agent(!self.is_subagent)
            .has_user_input(self.current_turn_has_user_input)
            .context_window(self.context.environment.context_window)
            .cwd(self.context.environment.cwd.clone())
            .file_tracker(&self.file_tracker)
            .is_plan_mode(self.plan_mode_state.is_active)
            .is_plan_reentry(self.plan_mode_state.is_reentry());

        // Add plan file path if in plan mode
        if let Some(path) = self.plan_mode_state.plan_file_path.clone() {
            gen_ctx_builder = gen_ctx_builder.plan_file_path(path);
        }

        // Add async hook responses to generator context
        if !async_responses.is_empty() {
            gen_ctx_builder = gen_ctx_builder.extension(ASYNC_HOOK_RESPONSES_KEY, async_responses);
        }
        if !blocking_hooks.is_empty() {
            gen_ctx_builder = gen_ctx_builder.extension(HOOK_BLOCKING_KEY, blocking_hooks);
        }
        if !context_hooks.is_empty() {
            gen_ctx_builder = gen_ctx_builder.extension(HOOK_CONTEXT_KEY, context_hooks);
        }

        // Add available skills to generator context for system reminders
        if let Some(ref sm) = self.skill_manager {
            let skill_infos: Vec<SkillInfo> = sm
                .all()
                .map(|skill| SkillInfo {
                    name: skill.name.clone(),
                    description: skill.description.clone(),
                })
                .collect();

            if !skill_infos.is_empty() {
                gen_ctx_builder = gen_ctx_builder.extension(AVAILABLE_SKILLS_KEY, skill_infos);
            }
        }

        // Add queued commands for real-time steering
        // These are injected as `<system-reminder>User sent: {message}</system-reminder>`
        if !self.queued_commands.is_empty() {
            gen_ctx_builder = gen_ctx_builder.queued_commands(self.queued_commands.clone());
        }

        let gen_ctx = gen_ctx_builder.build();

        let reminders = self.reminder_orchestrator.generate_all(&gen_ctx).await;
        let injected_messages = create_injected_messages(reminders);

        // ── STEP 7: Resolve model (permissions checked externally) ──
        // In this implementation, model selection is handled by ApiClient.

        // ── STEP 8: Check blocking token limit ──
        // Use CompactConfig for blocking limit calculation
        let blocking_limit = self.compact_config.blocking_limit(context_window);
        if status.is_at_blocking_limit {
            warn!(
                estimated_tokens = estimated_with_margin,
                blocking_limit, "Context window exceeded blocking limit"
            );
            self.set_status(AgentStatus::error("Context window exceeded"));
            return Ok(LoopResult::error(
                self.turn_number,
                self.total_input_tokens,
                self.total_output_tokens,
                format!(
                    "Context window exceeded: {estimated_with_margin} tokens >= {blocking_limit} limit"
                ),
            ));
        }

        // Create executor for this turn BEFORE streaming starts.
        // This enables tool execution to begin DURING streaming.
        let executor_config = ExecutorConfig {
            session_id: query_tracking.chain_id.clone(),
            permission_mode: self.config.permission_mode,
            cwd: self.context.environment.cwd.clone().into(),
            is_plan_mode: self.plan_mode_state.is_active,
            plan_file_path: self.plan_mode_state.plan_file_path.clone(),
            ..ExecutorConfig::default()
        };
        let mut executor = StreamingToolExecutor::new(
            self.tool_registry.clone(),
            executor_config,
            Some(self.event_tx.clone()),
        )
        .with_cancel_token(self.cancel_token.clone())
        .with_hooks(self.hooks.clone())
        // Share the file tracker across turns for change detection
        .with_file_tracker(self.shared_tools_file_tracker.clone())
        // Share the approval store across turns for permission persistence
        .with_approval_store(self.shared_approval_store.clone())
        // Share async hook tracker for background hook completion tracking
        .with_async_hook_tracker(self.async_hook_tracker.clone());

        // Add spawn_agent_fn if available for Task tool
        if let Some(ref spawn_fn) = self.spawn_agent_fn {
            executor = executor.with_spawn_agent_fn(spawn_fn.clone());
        }

        // Add skill_manager if available for Skill tool
        if let Some(ref sm) = self.skill_manager {
            executor = executor.with_skill_manager(sm.clone());
        }

        // Pass parent selections for subagent isolation
        // Subagents spawned via Task tool will inherit these selections,
        // ensuring they're unaffected by changes to this agent's model settings.
        executor = executor.with_parent_selections(self.selections.clone());

        // ── STEP 9: Main API streaming loop with retry ──
        let mut output_recovery_attempts = 0;
        let collected = loop {
            if self.cancel_token.is_cancelled() {
                self.set_status(AgentStatus::Idle);
                return Ok(LoopResult::interrupted(
                    self.turn_number,
                    self.total_input_tokens,
                    self.total_output_tokens,
                ));
            }

            match self
                .stream_with_tools(&turn_id, &executor, &injected_messages, query_tracking)
                .await
            {
                Ok(collected) => break collected,
                Err(e) => {
                    // Check if retriable (output token exhaustion)
                    output_recovery_attempts += 1;
                    if output_recovery_attempts >= MAX_OUTPUT_TOKEN_RECOVERY {
                        return Err(e);
                    }
                    self.emit(LoopEvent::Retry {
                        attempt: output_recovery_attempts,
                        max_attempts: MAX_OUTPUT_TOKEN_RECOVERY,
                        delay_ms: 0,
                    })
                    .await;
                    continue;
                }
            }
        };

        // ── STEP 10: Record API call info ──
        if let Some(usage) = &collected.usage {
            self.total_input_tokens += usage.input_tokens as i32;
            self.total_output_tokens += usage.output_tokens as i32;
        }

        let usage = collected.usage.clone().unwrap_or_default();
        self.emit(LoopEvent::StreamRequestEnd {
            usage: usage.clone(),
        })
        .await;

        // Extract text from response
        let response_text: String = collected
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();

        // Check for tool calls
        let has_tool_calls = collected
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolUse { .. }));

        // Add assistant message to history
        if let Some(turn) = self.message_history.current_turn_mut() {
            let assistant_msg = TrackedMessage::assistant(&response_text, &turn_id, None);
            turn.set_assistant_message(assistant_msg);
            turn.update_usage(usage.clone());
        }

        // ── STEP 11: Check for tool calls ──
        // ── STEP 12: Execute tool queue ──
        // Tool execution already started DURING streaming for safe tools.
        // Now we execute pending unsafe tools and collect all results.
        if has_tool_calls {
            let tool_calls: Vec<_> = collected
                .content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolUse {
                        id, name, input, ..
                    } => Some(hyper_sdk::ToolCall::new(id, name, input.clone())),
                    _ => None,
                })
                .collect();

            // Execute pending unsafe tools (safe tools already started during streaming)
            executor.execute_pending_unsafe().await;

            // Drain all results (both from streaming and unsafe execution)
            let results = executor.drain().await;

            // ── STEP 13: Handle abort after tool execution ──
            // Check if cancelled during tool execution
            if self.cancel_token.is_cancelled() {
                self.set_status(AgentStatus::Idle);
                return Ok(LoopResult::interrupted(
                    self.turn_number,
                    self.total_input_tokens,
                    self.total_output_tokens,
                ));
            }

            // Add tool results to history and apply context modifiers
            self.add_tool_results_to_history(&results, &tool_calls)
                .await;

            // ── Handle plan mode transitions ──
            // Check if EnterPlanMode or ExitPlanMode was called
            for tc in &tool_calls {
                match tc.name.as_str() {
                    "EnterPlanMode" => {
                        // Find the result for this tool call to extract plan file path
                        if let Some(result) = results.iter().find(|r| r.call_id == tc.id) {
                            if let Ok(output) = &result.result {
                                // Extract plan file path from output
                                // The output is text containing "Plan file: /path/to/file"
                                if let ToolResultContent::Text(text) = &output.content {
                                    if let Some(path_line) =
                                        text.lines().find(|l| l.starts_with("Plan file:"))
                                    {
                                        let path_str =
                                            path_line.trim_start_matches("Plan file:").trim();
                                        let path = std::path::PathBuf::from(path_str);
                                        let slug = path
                                            .file_stem()
                                            .and_then(|s| s.to_str())
                                            .unwrap_or("plan")
                                            .to_string();
                                        self.plan_mode_state.enter(path, slug, self.turn_number);
                                        info!(turn = self.turn_number, "Entered plan mode");
                                    }
                                }
                            }
                        }
                    }
                    "ExitPlanMode" => {
                        // Update plan mode state
                        self.plan_mode_state.exit(self.turn_number);
                        info!(turn = self.turn_number, "Exited plan mode");

                        // Return with plan mode exit stop reason
                        // Note: approval is false until user confirms
                        return Ok(LoopResult::plan_mode_exit(
                            self.turn_number,
                            self.total_input_tokens,
                            self.total_output_tokens,
                            false, // approved = false, awaiting user approval
                            collected.content,
                        ));
                    }
                    _ => {}
                }
            }

            // Track tool calls for extraction triggering
            for _ in &tool_calls {
                auto_compact_tracking.record_tool_call();
            }
        }

        // ── STEP 14: Check for hook stop ──
        // Hook execution is deferred to a future session.

        // ── STEP 15: Update auto-compact tracking ──
        auto_compact_tracking.turn_counter += 1;

        // ── STEP 15.5: Check session memory extraction trigger ──
        // This runs a background agent to proactively update summary.md
        if let Some(ref extraction_agent) = self.extraction_agent {
            let estimated_tokens = self.message_history.estimate_tokens();
            let is_compacting = false; // We're not currently in a compaction

            if extraction_agent.should_trigger(
                auto_compact_tracking,
                estimated_tokens,
                is_compacting,
            ) {
                // Build conversation text for extraction
                let messages = self.message_history.messages_for_api();
                let conversation_text: String = messages
                    .iter()
                    .map(|m| format!("{:?}", m))
                    .collect::<Vec<_>>()
                    .join("\n");

                let current_tokens = estimated_tokens;
                let tool_calls_since = auto_compact_tracking.tool_calls_since_extraction();
                let last_message_id = turn_id.clone();
                let message_count = messages.len() as i32;

                // Mark extraction as started
                auto_compact_tracking.mark_extraction_started();

                // Clone what we need for the background task
                let agent = Arc::clone(extraction_agent);
                let tracking_current_tokens = current_tokens;

                // Spawn extraction in background (non-blocking)
                tokio::spawn(async move {
                    match agent
                        .run_extraction(
                            &conversation_text,
                            tracking_current_tokens,
                            tool_calls_since,
                            &last_message_id,
                            message_count,
                        )
                        .await
                    {
                        Ok(result) => {
                            debug!(
                                summary_tokens = result.summary_tokens,
                                last_id = %result.last_summarized_id,
                                "Background extraction completed"
                            );
                            // Note: We can't update auto_compact_tracking here since
                            // it's owned by the main loop. The next turn will see
                            // the updated summary.md file.
                        }
                        Err(e) => {
                            warn!(error = %e, "Background extraction failed");
                        }
                    }
                });
            }
        }

        // ── STEP 16: Process queued commands and attachments ──
        // Deferred to future sessions.

        // ── STEP 17: Check max turns limit ──
        if let Some(max) = self.config.max_turns {
            if self.turn_number >= max {
                self.emit(LoopEvent::MaxTurnsReached).await;
                return Ok(LoopResult::max_turns_reached(
                    self.turn_number,
                    self.total_input_tokens,
                    self.total_output_tokens,
                ));
            }
        }

        // Emit turn completed
        self.emit(LoopEvent::TurnCompleted {
            turn_id: turn_id.clone(),
            usage,
        })
        .await;

        // ── STEP 18: Recurse or return ──
        match collected.finish_reason {
            FinishReason::Stop => {
                // Turn completed with stop - set status to Idle
                self.set_status(AgentStatus::Idle);
                Ok(LoopResult::completed(
                    self.turn_number,
                    self.total_input_tokens,
                    self.total_output_tokens,
                    response_text,
                    collected.content,
                ))
            }
            FinishReason::ToolCalls => {
                // Tool call turns don't have fresh user input - only tool results
                self.current_turn_has_user_input = false;
                // Recursive call for next turn (boxed to avoid infinite future size)
                Box::pin(self.core_message_loop(query_tracking, auto_compact_tracking)).await
            }
            FinishReason::MaxTokens => {
                // Output token recovery already handled in step 9
                self.set_status(AgentStatus::Idle);
                Ok(LoopResult::completed(
                    self.turn_number,
                    self.total_input_tokens,
                    self.total_output_tokens,
                    response_text,
                    collected.content,
                ))
            }
            other => {
                warn!(?other, "Unexpected finish reason");
                self.set_status(AgentStatus::Idle);
                Ok(LoopResult::completed(
                    self.turn_number,
                    self.total_input_tokens,
                    self.total_output_tokens,
                    response_text,
                    collected.content,
                ))
            }
        }
    }

    /// Stream an API request and collect the response.
    ///
    /// Uses `ApiClient::stream_request()` with tool definitions from the
    /// registry. Includes stall detection based on `stall_detection` config.
    ///
    /// **Key feature**: Tool execution starts DURING streaming. When a ToolUse
    /// block is received, safe tools begin execution immediately via the
    /// executor. This enables concurrent tool execution while the LLM continues
    /// generating output.
    ///
    /// # Arguments
    ///
    /// * `turn_id` - Unique identifier for this turn
    /// * `executor` - Tool executor for handling tool calls
    /// * `injected_messages` - Injected messages from system reminders
    /// * `query_tracking` - Query tracking info containing the real session_id (chain_id)
    async fn stream_with_tools(
        &mut self,
        turn_id: &str,
        executor: &StreamingToolExecutor,
        injected_messages: &[InjectedMessage],
        query_tracking: &QueryTracking,
    ) -> anyhow::Result<CollectedResponse> {
        debug!(turn_id, "Sending API request");

        // Get model and build request using ModelHub
        // Use the real session_id from query_tracking instead of extracting from turn_id
        let session_id = &query_tracking.chain_id;
        let (ctx, model) = self
            .model_hub
            .prepare_main_with_selections(&self.selections, session_id, self.turn_number)
            .map_err(|e| anyhow::anyhow!("Failed to prepare main model: {e}"))?;

        // Build messages and tools using existing logic (model-aware filtering)
        let (messages, tools) = self.build_messages_and_tools(injected_messages, &ctx.model_info);

        // Use RequestBuilder to assemble the final request with context parameters
        let mut builder = RequestBuilder::new(ctx).messages(messages);
        if !tools.is_empty() {
            builder = builder.tools(tools);
        }
        if let Some(max_tokens) = self.config.max_tokens {
            builder = builder.max_tokens(max_tokens);
        }

        let request = builder.build();

        let mut stream = self
            .api_client
            .stream_request(&*model, request, StreamOptions::streaming())
            .await
            .map_err(|e| anyhow::anyhow!("API stream error: {e}"))?;

        let mut all_content: Vec<ContentBlock> = Vec::new();
        let mut final_usage: Option<TokenUsage> = None;
        let mut final_finish_reason = FinishReason::Stop;

        // Stall detection configuration
        let stall_timeout = self.config.stall_detection.stall_timeout;
        let stall_enabled = self.config.stall_detection.enabled;
        let mut last_event_time = Instant::now();

        // Process streaming results with stall detection
        loop {
            let next_event = stream.next();

            // Use tokio::select! for stall detection if enabled
            let result = if stall_enabled {
                let timeout_at = last_event_time + stall_timeout;
                let remaining = timeout_at.saturating_duration_since(Instant::now());

                tokio::select! {
                    biased;
                    result = next_event => result,
                    _ = tokio::time::sleep(remaining) => {
                        // Stream stall detected
                        self.emit(LoopEvent::StreamStallDetected {
                            turn_id: turn_id.to_string(),
                            timeout: stall_timeout,
                        }).await;

                        // Handle based on recovery strategy
                        match self.config.stall_detection.recovery {
                            cocode_protocol::StallRecovery::Abort => {
                                return Err(anyhow::anyhow!(
                                    "Stream stalled for {:?}, aborting", stall_timeout
                                ));
                            }
                            cocode_protocol::StallRecovery::Retry => {
                                warn!(turn_id, timeout = ?stall_timeout, "Stream stalled, retrying");
                                return Err(anyhow::anyhow!(
                                    "Stream stalled for {:?}, retry requested", stall_timeout
                                ));
                            }
                            cocode_protocol::StallRecovery::Fallback => {
                                // Attempt model fallback
                                if self.fallback_state.should_fallback(&self.fallback_config) {
                                    if let Some(fallback_model) = self.fallback_state.next_model(&self.fallback_config) {
                                        self.emit(LoopEvent::ModelFallbackStarted {
                                            from: self.fallback_state.current_model.clone(),
                                            to: fallback_model.clone(),
                                            reason: format!("Stream stalled for {:?}", stall_timeout),
                                        }).await;
                                        self.fallback_state.record_fallback(
                                            fallback_model,
                                            format!("Stream stalled for {:?}", stall_timeout),
                                        );
                                    }
                                }
                                return Err(anyhow::anyhow!(
                                    "Stream stalled for {:?}, fallback triggered", stall_timeout
                                ));
                            }
                        }
                    }
                }
            } else {
                next_event.await
            };

            // Process the result
            let Some(result) = result else {
                break; // Stream ended
            };

            let result = result.map_err(|e| {
                // Check if this is an overload error for fallback handling
                let err_str = e.to_string();
                if err_str.contains("overload") || err_str.contains("rate_limit") {
                    if self.fallback_state.should_fallback(&self.fallback_config) {
                        if let Some(fallback_model) =
                            self.fallback_state.next_model(&self.fallback_config)
                        {
                            // Note: We can't emit async events here, but we record the fallback
                            self.fallback_state
                                .record_fallback(fallback_model, format!("API error: {}", err_str));
                        }
                    }
                }
                anyhow::anyhow!("Stream error: {e}")
            })?;

            // Update stall timer on any event
            last_event_time = Instant::now();

            match result.result_type {
                QueryResultType::Assistant => {
                    // Emit text deltas for UI and process tool uses DURING streaming
                    for block in &result.content {
                        match block {
                            ContentBlock::Text { text } if !text.is_empty() => {
                                self.emit(LoopEvent::TextDelta {
                                    turn_id: turn_id.to_string(),
                                    delta: text.clone(),
                                })
                                .await;
                            }
                            ContentBlock::Thinking { content, .. } if !content.is_empty() => {
                                self.emit(LoopEvent::ThinkingDelta {
                                    turn_id: turn_id.to_string(),
                                    delta: content.clone(),
                                })
                                .await;
                            }
                            ContentBlock::ToolUse {
                                id, name, input, ..
                            } => {
                                // Start tool execution DURING streaming!
                                // Safe tools begin immediately; unsafe tools are queued.
                                let tool_call = ToolCall::new(id, name, input.clone());
                                executor.on_tool_complete(tool_call).await;
                            }
                            _ => {}
                        }
                    }
                    all_content.extend(result.content);

                    // Capture usage from non-streaming responses
                    if result.usage.is_some() {
                        final_usage = result.usage;
                    }
                    if let Some(fr) = result.finish_reason {
                        final_finish_reason = fr;
                    }
                }
                QueryResultType::Done => {
                    final_usage = result.usage;
                    if let Some(fr) = result.finish_reason {
                        final_finish_reason = fr;
                    }
                    break;
                }
                QueryResultType::Error => {
                    let msg = result.error.unwrap_or_else(|| "Unknown error".to_string());

                    // Check for overload errors and handle fallback
                    if msg.contains("overload") || msg.contains("rate_limit") {
                        if self.fallback_state.should_fallback(&self.fallback_config) {
                            if let Some(fallback_model) =
                                self.fallback_state.next_model(&self.fallback_config)
                            {
                                self.emit(LoopEvent::ModelFallbackStarted {
                                    from: self.fallback_state.current_model.clone(),
                                    to: fallback_model.clone(),
                                    reason: msg.clone(),
                                })
                                .await;
                                self.fallback_state
                                    .record_fallback(fallback_model, msg.clone());
                            }
                        }
                    }

                    return Err(anyhow::anyhow!("Stream error: {msg}"));
                }
                QueryResultType::Retry | QueryResultType::Event => {
                    // Continue
                }
            }
        }

        Ok(CollectedResponse {
            content: all_content,
            usage: final_usage,
            finish_reason: final_finish_reason,
        })
    }

    /// Build messages and tool definitions for the API request.
    ///
    /// This extracts the message/tool building logic for use with `RequestBuilder`.
    /// Tool definitions are filtered per-model based on `ModelInfo` capabilities.
    ///
    /// # Arguments
    ///
    /// * `injected_messages` - Injected messages from system reminders
    /// * `model_info` - Model information for tool filtering
    fn build_messages_and_tools(
        &self,
        injected_messages: &[InjectedMessage],
        model_info: &cocode_protocol::ModelInfo,
    ) -> (Vec<Message>, Vec<ToolDefinition>) {
        // Build system prompt
        let system_prompt = SystemPromptBuilder::build(&self.context);

        // Get conversation messages
        let messages = self.message_history.messages_for_api();

        // Build messages with system, reminders, and conversation
        let mut all_messages = vec![Message::system(&system_prompt)];

        // Inject system reminders as individual messages before the conversation
        // This supports both text reminders and multi-message tool_use/tool_result pairs
        for msg in injected_messages {
            all_messages.push(self.convert_injected_message(msg));
        }

        all_messages.extend(messages);

        // Get tool definitions with model-aware filtering
        let tools = self.select_tools_for_model(model_info);

        (all_messages, tools)
    }

    fn select_tools_for_model(
        &self,
        model_info: &cocode_protocol::ModelInfo,
    ) -> Vec<ToolDefinition> {
        select_tools_for_model(self.tool_registry.all_definitions(), model_info)
    }

    /// Convert an injected message to an API Message.
    fn convert_injected_message(&self, msg: &InjectedMessage) -> Message {
        match msg {
            InjectedMessage::UserText { content, .. } => {
                // Text reminders become simple user messages
                Message::user(content.as_str())
            }
            InjectedMessage::AssistantBlocks { blocks, .. } => {
                // Assistant blocks (typically tool_use) become assistant messages
                let content_blocks: Vec<ContentBlock> =
                    blocks.iter().map(Self::convert_injected_block).collect();
                Message::new(hyper_sdk::Role::Assistant, content_blocks)
            }
            InjectedMessage::UserBlocks { blocks, .. } => {
                // User blocks (typically tool_result) become user messages
                let content_blocks: Vec<ContentBlock> =
                    blocks.iter().map(Self::convert_injected_block).collect();
                Message::new(hyper_sdk::Role::User, content_blocks)
            }
        }
    }

    /// Convert an injected block to a hyper_sdk ContentBlock.
    fn convert_injected_block(block: &InjectedBlock) -> ContentBlock {
        match block {
            InjectedBlock::Text(text) => ContentBlock::text(text.as_str()),
            InjectedBlock::ToolUse { id, name, input } => {
                ContentBlock::tool_use(id.as_str(), name.as_str(), input.clone())
            }
            InjectedBlock::ToolResult {
                tool_use_id,
                content,
            } => ContentBlock::tool_result(
                tool_use_id.as_str(),
                hyper_sdk::ToolResultContent::text(content.as_str()),
                false,
            ),
        }
    }

    /// Micro-compaction: remove old tool results to save tokens (no LLM call).
    ///
    /// Uses `ThresholdStatus` to determine if micro-compaction is needed based on
    /// current context usage relative to the warning threshold.
    ///
    /// Returns a tuple of (removed_count, tokens_saved).
    fn micro_compact(&mut self) -> (i32, i32) {
        // Check if micro-compact is enabled
        if !self.compact_config.is_micro_compact_enabled() {
            return (0, 0);
        }

        let tokens_before = self.message_history.estimate_tokens();
        let context_window = self.context.environment.context_window;

        // Use ThresholdStatus to check if we're above warning threshold
        let status =
            ThresholdStatus::calculate(tokens_before, context_window, &self.compact_config);

        if !status.is_above_warning_threshold {
            debug!(
                tokens_before,
                status = status.status_description(),
                "Below warning threshold, skipping micro-compact"
            );
            return (0, 0);
        }

        // Apply micro-compaction using configured recent_tool_results_to_keep
        let keep_count = self.compact_config.recent_tool_results_to_keep;
        let removed = self.message_history.micro_compact(keep_count);

        // Calculate tokens saved
        let tokens_after = self.message_history.estimate_tokens();
        let tokens_saved = tokens_before - tokens_after;

        debug!(
            removed,
            tokens_before, tokens_after, tokens_saved, "Micro-compaction complete"
        );

        (removed, tokens_saved)
    }

    /// Run auto-compaction (LLM-based summarization).
    ///
    /// Uses the 9-section compact instructions from `build_compact_instructions()`
    /// to generate a comprehensive conversation summary.
    ///
    /// Before compaction begins, PreCompact hooks are executed. If any hook
    /// returns `Reject`, compaction is skipped and the rejection is logged.
    async fn compact(
        &mut self,
        tracking: &mut AutoCompactTracking,
        turn_id: &str,
        query_tracking: &QueryTracking,
    ) -> anyhow::Result<()> {
        // Execute PreCompact hooks before starting compaction
        let hook_ctx = cocode_hooks::HookContext::new(
            cocode_hooks::HookEventType::PreCompact,
            turn_id.to_string(),
            self.context.environment.cwd.clone(),
        );

        let outcomes = self.hooks.execute(&hook_ctx).await;

        // Check if any hook rejected compaction
        for outcome in &outcomes {
            // Emit HookExecuted event for each hook
            self.emit(LoopEvent::HookExecuted {
                hook_type: HookEventType::PreCompact,
                hook_name: outcome.hook_name.clone(),
            })
            .await;

            if let cocode_hooks::HookResult::Reject { reason } = &outcome.result {
                info!(
                    hook_name = %outcome.hook_name,
                    reason = %reason,
                    "Compaction skipped by hook"
                );
                self.emit(LoopEvent::CompactionSkippedByHook {
                    hook_name: outcome.hook_name.clone(),
                    reason: reason.clone(),
                })
                .await;
                return Ok(());
            }
        }

        // Update status to compacting
        self.set_status(AgentStatus::Compacting);
        self.emit(LoopEvent::CompactionStarted).await;

        // Estimate tokens before compaction
        let tokens_before = self.message_history.estimate_tokens();

        // Build summarization prompt from conversation text
        let messages = self.message_history.messages_for_api();
        let conversation_text: String = messages
            .iter()
            .map(|m| format!("{:?}", m))
            .collect::<Vec<_>>()
            .join("\n");

        // Use the 9-section compact instructions
        let max_output_tokens = self.compact_config.max_compact_output_tokens;
        let system_prompt = build_compact_instructions(max_output_tokens);

        // Fallback to legacy prompt builder if available
        let (_, user_prompt) = SystemPromptBuilder::build_summarization(&conversation_text, None);

        // Use the API client to get a summary with retry mechanism
        let max_retries = self.compact_config.max_summary_retries;
        let mut attempt = 0;

        let summary_text = loop {
            attempt += 1;
            let last_error: String;

            // Build request for each attempt
            let summary_messages =
                vec![Message::system(&system_prompt), Message::user(&user_prompt)];

            // Get compact model and build request using ModelHub
            // Use the real session_id from query_tracking
            let session_id = &query_tracking.chain_id;
            let (ctx, compact_model) = self
                .model_hub
                .prepare_compact_with_selections(&self.selections, session_id, self.turn_number)
                .map_err(|e| anyhow::anyhow!("Failed to prepare compact model: {e}"))?;

            // Use RequestBuilder for the summary request
            let summary_request = RequestBuilder::new(ctx)
                .messages(summary_messages.clone())
                .max_tokens(max_output_tokens)
                .build();

            match self
                .api_client
                .generate(&*compact_model, summary_request)
                .await
            {
                Ok(response) => {
                    // Extract summary text
                    let text: String = response
                        .content
                        .iter()
                        .filter_map(|b| match b {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect();

                    if text.is_empty() {
                        last_error = "Empty summary produced".to_string();
                        if attempt <= max_retries {
                            // Exponential backoff: 1s, 2s, 4s, ...
                            let delay_ms = 1000 * (1 << (attempt - 1));
                            self.emit(LoopEvent::CompactionRetry {
                                attempt,
                                max_attempts: max_retries + 1,
                                delay_ms,
                                reason: last_error.clone(),
                            })
                            .await;
                            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms as u64))
                                .await;
                            continue;
                        }
                    } else {
                        break text;
                    }
                }
                Err(e) => {
                    last_error = e.to_string();
                    if attempt <= max_retries {
                        // Exponential backoff: 1s, 2s, 4s, ...
                        let delay_ms = 1000 * (1 << (attempt - 1));
                        warn!(
                            attempt,
                            max_retries,
                            error = %last_error,
                            delay_ms,
                            "Compaction API call failed, retrying"
                        );
                        self.emit(LoopEvent::CompactionRetry {
                            attempt,
                            max_attempts: max_retries + 1,
                            delay_ms,
                            reason: last_error.clone(),
                        })
                        .await;
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms as u64))
                            .await;
                        continue;
                    }
                }
            }

            // All retries exhausted
            warn!(
                attempts = attempt,
                error = %last_error,
                "Compaction failed after all retries"
            );
            self.emit(LoopEvent::CompactionFailed {
                attempts: attempt,
                error: last_error,
            })
            .await;
            return Ok(());
        };

        // Extract task status from tool calls before compaction
        let tool_calls: Vec<(String, serde_json::Value)> = self
            .message_history
            .turns()
            .iter()
            .flat_map(|turn| {
                turn.tool_calls
                    .iter()
                    .map(|tc| (tc.name.clone(), tc.input.clone()))
            })
            .collect();

        let task_status = TaskStatusRestoration::from_tool_calls(&tool_calls);

        // Extract invoked skills from tool calls with turn numbers
        let tool_calls_with_turns: Vec<(String, serde_json::Value, i32)> = self
            .message_history
            .turns()
            .iter()
            .flat_map(|turn| {
                let turn_num = turn.number;
                turn.tool_calls
                    .iter()
                    .map(move |tc| (tc.name.clone(), tc.input.clone(), turn_num))
            })
            .collect();

        let invoked_skills = InvokedSkillRestoration::from_tool_calls(&tool_calls_with_turns);

        // Build final summary with task status
        let final_summary = if task_status.tasks.is_empty() {
            summary_text
        } else {
            let tasks_section = task_status
                .tasks
                .iter()
                .map(|t| {
                    let owner = t.owner.as_deref().unwrap_or("unassigned");
                    format!("- [{}] {}: {} ({})", t.status, t.id, t.subject, owner)
                })
                .collect::<Vec<_>>()
                .join("\n");

            format!("{summary_text}\n\n<task_status>\n{tasks_section}\n</task_status>")
        };

        // Calculate keep window using token-based algorithm
        let messages_json = self.message_history.messages_for_api_json();
        let keep_result =
            calculate_keep_start_index(&messages_json, &self.compact_config.keep_window);
        let keep_turns = map_message_index_to_keep_turns(
            self.message_history.turn_count(),
            &messages_json,
            keep_result.keep_start_index,
        );
        let tokens_saved = (tokens_before - self.message_history.estimate_tokens()).max(0);

        debug!(
            keep_turns,
            keep_start_index = keep_result.keep_start_index,
            messages_to_keep = keep_result.messages_to_keep,
            keep_tokens = keep_result.keep_tokens,
            text_messages_kept = keep_result.text_messages_kept,
            "Calculated keep window for compaction"
        );

        // Get transcript path from context if available
        let transcript_path = self.context.transcript_path.clone();

        self.message_history.apply_compaction_with_metadata(
            final_summary.clone(),
            keep_turns,
            turn_id,
            tokens_saved,
            cocode_protocol::CompactTrigger::Auto,
            tokens_before,
            transcript_path.clone(),
            true, // Recent messages are preserved
        );

        // Update tracking
        tracking.mark_compacted(turn_id, self.turn_number);

        // Calculate post-compaction tokens and update boundary
        let post_tokens = self.message_history.estimate_tokens();
        self.message_history
            .update_boundary_post_tokens(post_tokens);

        // Compaction complete - restore status to Idle
        self.set_status(AgentStatus::Idle);
        self.emit(LoopEvent::CompactionCompleted {
            removed_messages: 0, // Tracked by MessageHistory
            summary_tokens: post_tokens,
        })
        .await;

        // Emit compact boundary inserted event
        self.emit(LoopEvent::CompactBoundaryInserted {
            trigger: cocode_protocol::CompactTrigger::Auto,
            pre_tokens: tokens_before,
            post_tokens,
        })
        .await;

        // Emit invoked skills restored event if any skills were found
        if !invoked_skills.is_empty() {
            let skill_names: Vec<String> = invoked_skills.iter().map(|s| s.name.clone()).collect();
            self.emit(LoopEvent::InvokedSkillsRestored {
                skills: skill_names,
            })
            .await;
        }

        // Context restoration: restore important files that were read before compaction
        self.restore_context_after_compaction(&invoked_skills, &task_status)
            .await;

        // Save to session memory for future Tier 1 compaction
        if self.compaction_config.session_memory.enabled {
            if let Some(ref path) = self.compaction_config.session_memory.summary_path {
                let summary_content = final_summary;
                let turn_id_owned = turn_id.to_string();
                let path_owned = path.clone();

                // Spawn background task to write session memory
                tokio::spawn(async move {
                    if let Err(e) =
                        write_session_memory(&path_owned, &summary_content, &turn_id_owned).await
                    {
                        tracing::warn!(
                            error = %e,
                            path = ?path_owned,
                            "Failed to write session memory"
                        );
                    } else {
                        tracing::debug!(
                            path = ?path_owned,
                            "Session memory saved for future Tier 1 compaction"
                        );
                    }
                });
            }
        }

        // Execute SessionStart hooks after compaction (with source: 'compact')
        // This allows hooks to provide additional context after compaction
        self.execute_post_compact_hooks(turn_id).await;

        Ok(())
    }

    /// Execute SessionStart hooks after compaction.
    ///
    /// Runs SessionStart hooks with source='compact' to allow them to provide
    /// additional context for the resumed conversation.
    async fn execute_post_compact_hooks(&mut self, turn_id: &str) {
        let hook_ctx = cocode_hooks::HookContext::new(
            cocode_hooks::HookEventType::SessionStart,
            turn_id.to_string(),
            self.context.environment.cwd.clone(),
        )
        .with_metadata("source", "compact");

        let outcomes = self.hooks.execute(&hook_ctx).await;

        let mut hooks_executed = 0;
        let mut additional_context_count = 0;

        for outcome in &outcomes {
            // Emit HookExecuted event for each hook
            self.emit(LoopEvent::HookExecuted {
                hook_type: HookEventType::SessionStart,
                hook_name: outcome.hook_name.clone(),
            })
            .await;

            hooks_executed += 1;

            // Check for additional context from hooks
            if let cocode_hooks::HookResult::ContinueWithContext { additional_context } =
                &outcome.result
            {
                if let Some(ctx) = additional_context {
                    if !ctx.is_empty() {
                        additional_context_count += 1;
                        debug!(
                            hook_name = %outcome.hook_name,
                            context_len = ctx.len(),
                            "Hook provided additional context"
                        );
                    }
                }
            }
        }

        if hooks_executed > 0 {
            self.emit(LoopEvent::PostCompactHooksExecuted {
                hooks_executed,
                additional_context_count,
            })
            .await;
        }
    }

    /// Restore context after compaction.
    ///
    /// This method restores important files, skills, and task status that were
    /// tracked before compaction. Files are prioritized by recency and importance.
    ///
    /// # Arguments
    /// * `invoked_skills` - Skills that were invoked before compaction
    /// * `task_status` - Task status restoration data
    async fn restore_context_after_compaction(
        &mut self,
        invoked_skills: &[InvokedSkillRestoration],
        task_status: &TaskStatusRestoration,
    ) {
        // Get file restoration config
        let file_config = &self.compact_config.file_restoration;

        // Collect files from the file tracker (Layer 2)
        let tracked_files = self.file_tracker.tracked_files();
        let mut files_for_restoration: Vec<FileRestoration> = Vec::new();

        for path in tracked_files {
            // Skip excluded patterns
            let path_str = path.to_string_lossy();
            if file_config.should_exclude(&path_str) {
                continue;
            }

            // Try to read the file content
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                let tokens = (content.len() / 4) as i32; // Rough estimate

                // Get last access time from file tracker
                let last_accessed = if let Some(state) = self.file_tracker.get_state(&path) {
                    // Use read_turn as a proxy for access time
                    state.read_turn as i64
                } else {
                    0
                };

                files_for_restoration.push(FileRestoration {
                    path,
                    content,
                    priority: 1, // Default priority
                    tokens,
                    last_accessed,
                });
            }
        }

        // Limit to configured max files
        if files_for_restoration.len() > file_config.max_files as usize {
            // Sort by last_accessed descending (most recent first)
            files_for_restoration.sort_by(|a, b| b.last_accessed.cmp(&a.last_accessed));
            files_for_restoration.truncate(file_config.max_files as usize);
        }

        // Build todo list from task status
        let todos = if !task_status.tasks.is_empty() {
            let todo_text = task_status
                .tasks
                .iter()
                .map(|t| format!("- [{}] {}: {}", t.status, t.id, t.subject))
                .collect::<Vec<_>>()
                .join("\n");
            Some(todo_text)
        } else {
            None
        };

        // Build skills list from invoked skills
        let skills: Vec<String> = invoked_skills.iter().map(|s| s.name.clone()).collect();

        // Get plan content if in plan mode
        let plan = if self.plan_mode_state.is_active {
            if let Some(plan_path) = &self.plan_mode_state.plan_file_path {
                tokio::fs::read_to_string(plan_path).await.ok()
            } else {
                None
            }
        } else {
            None
        };

        // Build context restoration
        let restoration = build_context_restoration_with_config(
            files_for_restoration,
            todos,
            plan,
            skills,
            file_config,
        );

        // Format and inject restoration message if there's content to restore
        let restoration_message = format_restoration_message(&restoration);
        if !restoration_message.is_empty() {
            let files_count = restoration.files.len();
            debug!(
                files_restored = files_count,
                has_todos = restoration.todos.is_some(),
                has_plan = restoration.plan.is_some(),
                skills_count = restoration.skills.len(),
                "Context restoration completed"
            );

            // Emit context restoration event
            self.emit(LoopEvent::ContextRestored {
                files_count: files_count as i32,
                has_todos: restoration.todos.is_some(),
                has_plan: restoration.plan.is_some(),
            })
            .await;
        }
    }

    /// Apply a cached session memory summary (Tier 1 compaction).
    ///
    /// This is the zero-cost compaction path that uses a previously saved summary
    /// instead of making an LLM API call. The summary is stored in the session memory
    /// file and can be reused across conversation continuations.
    ///
    /// # Arguments
    /// * `summary` - The cached session memory summary
    /// * `turn_id` - ID of the current turn
    /// * `tracking` - Auto-compact tracking state
    async fn apply_session_memory_summary(
        &mut self,
        summary: SessionMemorySummary,
        turn_id: &str,
        tracking: &mut AutoCompactTracking,
    ) -> anyhow::Result<()> {
        let tokens_before = self.message_history.estimate_tokens();

        info!(
            summary_tokens = summary.token_estimate,
            last_id = ?summary.last_summarized_id,
            "Applying session memory summary (Tier 1)"
        );

        // Get transcript path from context if available
        let transcript_path = self.context.transcript_path.clone();

        // Calculate keep window using token-based algorithm
        let messages_json = self.message_history.messages_for_api_json();
        let keep_result =
            calculate_keep_start_index(&messages_json, &self.compact_config.keep_window);
        let keep_turns = map_message_index_to_keep_turns(
            self.message_history.turn_count(),
            &messages_json,
            keep_result.keep_start_index,
        );
        let tokens_saved = (tokens_before - summary.token_estimate).max(0);

        debug!(
            keep_turns,
            keep_start_index = keep_result.keep_start_index,
            messages_to_keep = keep_result.messages_to_keep,
            keep_tokens = keep_result.keep_tokens,
            "Calculated keep window for session memory compact"
        );

        self.message_history.apply_compaction_with_metadata(
            summary.summary.clone(),
            keep_turns,
            turn_id,
            tokens_saved,
            cocode_protocol::CompactTrigger::Auto,
            tokens_before,
            transcript_path,
            true, // Recent messages preserved
        );

        // Update tracking
        tracking.mark_compacted(turn_id, self.turn_number);

        // Calculate post-compaction tokens and update boundary
        let post_tokens = self.message_history.estimate_tokens();
        self.message_history
            .update_boundary_post_tokens(post_tokens);

        // Emit events
        self.emit(LoopEvent::SessionMemoryCompactApplied {
            saved_tokens: tokens_saved,
            summary_tokens: summary.token_estimate,
        })
        .await;

        // Emit compact boundary inserted event
        self.emit(LoopEvent::CompactBoundaryInserted {
            trigger: cocode_protocol::CompactTrigger::Auto,
            pre_tokens: tokens_before,
            post_tokens,
        })
        .await;

        Ok(())
    }

    /// Add tool results to the message history and apply context modifiers.
    ///
    /// This creates proper tool_result messages that link back to the tool_use
    /// blocks via their call_id. The results are added to the current turn
    /// for tracking, and a new turn with tool result messages is created
    /// for the next API call.
    ///
    /// Context modifiers from tool outputs are applied to update:
    /// - `FileTracker`: Records file reads with content and timestamps
    /// - `ApprovalStore`: Records permission grants for future operations
    /// - Queued commands (logged but not yet executed)
    async fn add_tool_results_to_history(
        &mut self,
        results: &[ToolExecutionResult],
        _tool_calls: &[ToolCall],
    ) {
        if results.is_empty() {
            return;
        }

        // Collect all modifiers from successful tool executions
        let mut all_modifiers: Vec<ContextModifier> = Vec::new();

        // Add tool results to current turn for tracking
        for result in results {
            let (output, is_error) = match &result.result {
                Ok(output) => {
                    // Collect modifiers from successful executions
                    all_modifiers.extend(output.modifiers.clone());
                    (output.content.clone(), output.is_error)
                }
                Err(e) => (ToolResultContent::Text(e.to_string()), true),
            };
            self.message_history
                .add_tool_result(&result.call_id, &result.name, output, is_error);
        }

        // Apply context modifiers
        if !all_modifiers.is_empty() {
            self.apply_modifiers(&all_modifiers).await;
        }

        // Create a new turn with tool result messages for the next API call
        // Using TrackedMessage::tool_result for proper role assignment
        let next_turn_id = uuid::Uuid::new_v4().to_string();

        // Build tool result content blocks for the user message
        // (Some providers expect tool results as user messages with special content)
        let tool_results_text: String = results
            .iter()
            .map(|r| {
                let output_text = match &r.result {
                    Ok(output) => match &output.content {
                        ToolResultContent::Text(t) => t.clone(),
                        ToolResultContent::Structured(v) => v.to_string(),
                    },
                    Err(e) => format!("Tool error: {e}"),
                };
                format!(
                    "<tool_result tool_use_id=\"{}\" name=\"{}\">\n{}\n</tool_result>",
                    r.call_id, r.name, output_text
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        // Create a user message containing the tool results
        // This will be normalized by MessageHistory::messages_for_api() to the correct format
        let user_msg = TrackedMessage::user(&tool_results_text, &next_turn_id);
        let turn = Turn::new(self.turn_number + 1, user_msg);
        self.message_history.add_turn(turn);
    }

    /// Apply context modifiers from tool execution results.
    ///
    /// This processes modifiers collected from tool outputs and updates the
    /// appropriate stores:
    /// - `FileRead`: Updates the FileTracker with file content and timestamps
    /// - `PermissionGranted`: Updates the ApprovalStore with granted permissions
    async fn apply_modifiers(&mut self, modifiers: &[ContextModifier]) {
        for modifier in modifiers {
            match modifier {
                ContextModifier::FileRead { path, content } => {
                    // Update the shared file tracker with the file read state
                    let mut tracker = self.shared_tools_file_tracker.lock().await;
                    // Get file mtime for change detection
                    let file_mtime = tokio::fs::metadata(path)
                        .await
                        .ok()
                        .and_then(|m| m.modified().ok());
                    let state = FileReadState::complete(content.clone(), file_mtime);
                    tracker.record_read_with_state(path.clone(), state);
                    debug!(
                        path = %path.display(),
                        content_len = content.len(),
                        "Applied FileRead modifier"
                    );
                }
                ContextModifier::PermissionGranted { tool, pattern } => {
                    // Update the shared approval store with the granted permission
                    let mut store = self.shared_approval_store.lock().await;
                    store.approve_pattern(tool, pattern);
                    debug!(
                        tool = %tool,
                        pattern = %pattern,
                        "Applied PermissionGranted modifier"
                    );
                }
            }
        }
    }

    /// Emit a loop event to the event channel.
    async fn emit(&self, event: LoopEvent) {
        if let Err(e) = self.event_tx.send(event).await {
            debug!("Failed to send loop event: {e}");
        }
    }

    /// Returns the current turn number.
    pub fn turn_number(&self) -> i32 {
        self.turn_number
    }

    /// Returns the total input tokens consumed.
    pub fn total_input_tokens(&self) -> i32 {
        self.total_input_tokens
    }

    /// Returns the total output tokens generated.
    pub fn total_output_tokens(&self) -> i32 {
        self.total_output_tokens
    }

    /// Returns a reference to the message history.
    pub fn message_history(&self) -> &MessageHistory {
        &self.message_history
    }

    /// Returns a reference to the loop configuration.
    pub fn config(&self) -> &LoopConfig {
        &self.config
    }

    /// Returns the cancellation token.
    pub fn cancel_token(&self) -> &CancellationToken {
        &self.cancel_token
    }

    /// Sync file read state from tools' FileTracker to system-reminder's FileTracker.
    ///
    /// This bridges the gap between the two FileTracker implementations:
    /// - `cocode_tools::FileTracker`: Tracks file reads during tool execution
    /// - `cocode_system_reminder::FileTracker`: Detects file changes for reminders
    ///
    /// By syncing data from tools to system-reminder, the `ChangedFilesGenerator`
    /// can accurately detect files that have been modified since they were read.
    async fn sync_file_trackers(&mut self) {
        let tools_tracker = self.shared_tools_file_tracker.lock().await;

        for (path, state) in tools_tracker.read_files_with_state() {
            // Convert tools::FileReadState to system_reminder::ReadFileState
            // Only sync if we have content (complete reads)
            if let Some(content) = &state.content {
                if state.is_complete_read {
                    self.file_tracker.sync_read(
                        &path,
                        content.clone(),
                        state.file_mtime,
                        self.turn_number,
                    );
                } else if let (Some(offset), Some(limit)) = (state.offset, state.limit) {
                    self.file_tracker.sync_partial_read(
                        &path,
                        content.clone(),
                        state.file_mtime,
                        self.turn_number,
                        offset as i64,
                        limit as i64,
                    );
                }
            } else {
                // For reads without content (simple record_read calls), sync with minimal info
                self.file_tracker.sync_read(
                    &path,
                    String::new(),
                    state.file_mtime,
                    self.turn_number,
                );
            }
        }
    }
}

/// Filter tool definitions based on model capabilities.
///
/// This ensures each model only sees tools it supports:
/// - `apply_patch`: controlled by `ModelInfo.apply_patch_tool_type`
/// - experimental tools: controlled by `ModelInfo.experimental_supported_tools`
fn select_tools_for_model(
    mut defs: Vec<ToolDefinition>,
    model_info: &cocode_protocol::ModelInfo,
) -> Vec<ToolDefinition> {
    use cocode_protocol::ApplyPatchToolType;
    use cocode_tools::builtin::ApplyPatchTool;

    // 1. Handle apply_patch: remove registry default, add model-specific variant
    defs.retain(|d| d.name != "apply_patch");
    match model_info.apply_patch_tool_type {
        Some(ApplyPatchToolType::Function) => {
            defs.push(ApplyPatchTool::function_definition());
        }
        Some(ApplyPatchToolType::Freeform) => {
            defs.push(ApplyPatchTool::freeform_definition());
        }
        Some(ApplyPatchToolType::Shell) | None => {
            // Shell: prompt handles it; None: no apply_patch at all
        }
    }

    // 2. Handle experimental_supported_tools (whitelist filter)
    if let Some(ref supported) = model_info.experimental_supported_tools {
        if !supported.is_empty() {
            defs.retain(|d| supported.contains(&d.name));
        }
    }

    defs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result::StopReason;

    #[test]
    fn test_default_config() {
        let config = LoopConfig::default();
        assert_eq!(config.max_turns, None);
        assert!((config.auto_compact_threshold - 0.8).abs() < f32::EPSILON);
        assert!(!config.enable_streaming_tools);
        assert!(!config.enable_micro_compaction);
    }

    #[test]
    fn test_builder_defaults() {
        let builder = AgentLoopBuilder::new();
        assert!(builder.api_client.is_none());
        assert!(builder.tool_registry.is_none());
        assert!(builder.context.is_none());
        assert!(builder.event_tx.is_none());
    }

    #[test]
    fn test_loop_result_constructors() {
        let completed = LoopResult::completed(5, 1000, 500, "text".to_string(), vec![]);
        assert_eq!(completed.turns_completed, 5);
        assert!(matches!(completed.stop_reason, StopReason::ModelStopSignal));

        let max = LoopResult::max_turns_reached(10, 2000, 1000);
        assert!(matches!(max.stop_reason, StopReason::MaxTurnsReached));

        let interrupted = LoopResult::interrupted(3, 500, 200);
        assert!(matches!(
            interrupted.stop_reason,
            StopReason::UserInterrupted
        ));

        let err = LoopResult::error(1, 100, 50, "boom".to_string());
        assert!(matches!(err.stop_reason, StopReason::Error { .. }));
    }

    #[test]
    fn test_constants() {
        assert_eq!(cocode_protocol::DEFAULT_MIN_BLOCKING_OFFSET, 13_000);
        assert_eq!(MAX_OUTPUT_TOKEN_RECOVERY, 3);
    }

    #[test]
    fn test_micro_compact_empty_history() {
        // Cannot construct a full AgentLoop without a model, but we can test
        // the candidate finder directly.
        let messages: Vec<serde_json::Value> = vec![];
        let candidates = crate::compaction::micro_compact_candidates(&messages);
        assert!(candidates.is_empty());
    }

    mod select_tools_for_model_tests {
        use super::*;
        use cocode_protocol::ApplyPatchToolType;
        use cocode_protocol::ModelInfo;
        use cocode_tools::builtin::ApplyPatchTool;

        fn sample_defs() -> Vec<ToolDefinition> {
            vec![
                ToolDefinition::new("Read", serde_json::json!({})),
                ToolDefinition::new("Edit", serde_json::json!({})),
                ToolDefinition::new("apply_patch", serde_json::json!({})),
            ]
        }

        #[test]
        fn function_variant_replaces_registry_default() {
            let model_info = ModelInfo {
                apply_patch_tool_type: Some(ApplyPatchToolType::Function),
                ..Default::default()
            };
            let result = select_tools_for_model(sample_defs(), &model_info);
            let ap = result.iter().find(|d| d.name == "apply_patch").unwrap();
            assert_eq!(ap.parameters["type"], "object");
            assert!(ap.parameters["properties"]["input"].is_object());
        }

        #[test]
        fn freeform_variant_uses_custom_tool() {
            let model_info = ModelInfo {
                apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
                ..Default::default()
            };
            let result = select_tools_for_model(sample_defs(), &model_info);
            let ap = result.iter().find(|d| d.name == "apply_patch").unwrap();
            assert!(ap.custom_format.is_some());
            assert_eq!(ap.custom_format.as_ref().unwrap()["type"], "grammar");
        }

        #[test]
        fn shell_variant_excludes_apply_patch() {
            let model_info = ModelInfo {
                apply_patch_tool_type: Some(ApplyPatchToolType::Shell),
                ..Default::default()
            };
            let result = select_tools_for_model(sample_defs(), &model_info);
            assert!(result.iter().all(|d| d.name != "apply_patch"));
            assert_eq!(result.len(), 2); // Read, Edit
        }

        #[test]
        fn none_excludes_apply_patch() {
            let model_info = ModelInfo {
                apply_patch_tool_type: None,
                ..Default::default()
            };
            let result = select_tools_for_model(sample_defs(), &model_info);
            assert!(result.iter().all(|d| d.name != "apply_patch"));
            assert_eq!(result.len(), 2);
        }

        #[test]
        fn experimental_supported_tools_whitelist() {
            let model_info = ModelInfo {
                apply_patch_tool_type: Some(ApplyPatchToolType::Function),
                experimental_supported_tools: Some(vec![
                    "Read".to_string(),
                    "apply_patch".to_string(),
                ]),
                ..Default::default()
            };
            let result = select_tools_for_model(sample_defs(), &model_info);
            assert_eq!(result.len(), 2);
            assert!(result.iter().any(|d| d.name == "Read"));
            assert!(result.iter().any(|d| d.name == "apply_patch"));
            assert!(result.iter().all(|d| d.name != "Edit"));
        }

        #[test]
        fn empty_supported_tools_does_not_filter() {
            let model_info = ModelInfo {
                apply_patch_tool_type: Some(ApplyPatchToolType::Function),
                experimental_supported_tools: Some(vec![]),
                ..Default::default()
            };
            let result = select_tools_for_model(sample_defs(), &model_info);
            // Empty whitelist = no filtering
            assert_eq!(result.len(), 3);
        }

        #[test]
        fn static_definitions_match_expected() {
            let func_def = ApplyPatchTool::function_definition();
            assert_eq!(func_def.name, "apply_patch");
            assert_eq!(func_def.parameters["type"], "object");

            let free_def = ApplyPatchTool::freeform_definition();
            assert_eq!(free_def.name, "apply_patch");
            assert!(free_def.custom_format.is_some());
            assert_eq!(free_def.custom_format.as_ref().unwrap()["type"], "grammar");
        }
    }
}
