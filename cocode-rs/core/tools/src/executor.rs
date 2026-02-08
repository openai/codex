//! Streaming tool executor for concurrent tool execution.
//!
//! This module provides [`StreamingToolExecutor`] which manages tool execution
//! during streaming, starting safe tools immediately and queuing unsafe tools.
//!
//! ## Hook Integration
//!
//! The executor supports hook execution at key lifecycle points:
//! - **PreToolUse**: Called before tool validation, can reject or modify input
//! - **PostToolUse**: Called after successful tool execution
//! - **PostToolUseFailure**: Called when a tool execution fails

use crate::context::ApprovalStore;
use crate::context::FileTracker;
use crate::context::SpawnAgentFn;
use crate::context::ToolContext;
use crate::context::ToolContextBuilder;
use crate::error::Result;
use crate::registry::ToolRegistry;
use crate::result_persistence;
use cocode_hooks::AsyncHookTracker;
use cocode_hooks::HookContext;
use cocode_hooks::HookEventType;
use cocode_hooks::HookRegistry;
use cocode_hooks::HookResult;
use cocode_protocol::AbortReason;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::LoopEvent;
use cocode_protocol::PermissionMode;
use cocode_protocol::ToolOutput;
use cocode_protocol::ValidationResult;
use cocode_shell::ShellExecutor;
use hyper_sdk::ToolCall;
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

/// Default maximum concurrent tool executions.
pub const DEFAULT_MAX_TOOL_CONCURRENCY: i32 = 10;

/// Configuration for the tool executor.
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Maximum concurrent tool executions.
    ///
    /// Configurable via `COCODE_MAX_TOOL_USE_CONCURRENCY` environment variable.
    /// Default: 10.
    pub max_concurrency: i32,
    /// Working directory for tool execution.
    pub cwd: PathBuf,
    /// Session ID.
    pub session_id: String,
    /// Permission mode.
    pub permission_mode: PermissionMode,
    /// Default timeout for tool execution (seconds).
    pub default_timeout_secs: i64,
    /// Whether plan mode is currently active.
    pub is_plan_mode: bool,
    /// Path to the current plan file (if in plan mode).
    pub plan_file_path: Option<PathBuf>,
    /// Session directory for storing large tool results.
    ///
    /// When set, tool results exceeding the configured size threshold are
    /// persisted to `{session_dir}/tool-results/{call_id}.txt`.
    pub session_dir: Option<PathBuf>,
    /// Tool configuration for result persistence thresholds.
    pub tool_config: cocode_protocol::ToolConfig,
    /// Feature flags for tool enablement.
    pub features: cocode_protocol::Features,
    /// Model-level cap on tool output size (characters).
    /// When set, applied after per-tool truncation but before persistence.
    pub max_tool_output_chars: Option<i32>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        // Check environment variable for max concurrency override
        let max_concurrency = std::env::var("COCODE_MAX_TOOL_USE_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<i32>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_MAX_TOOL_CONCURRENCY);

        Self {
            max_concurrency,
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            session_id: String::new(),
            permission_mode: PermissionMode::Default,
            default_timeout_secs: 120,
            is_plan_mode: false,
            plan_file_path: None,
            session_dir: None,
            tool_config: cocode_protocol::ToolConfig::default(),
            features: cocode_protocol::Features::with_defaults(),
            max_tool_output_chars: None,
        }
    }
}

/// Pending tool call waiting for execution.
#[derive(Debug)]
struct PendingToolCall {
    tool_call: ToolCall,
    #[allow(dead_code)]
    queued_at: std::time::Instant,
}

/// Result from a tool execution.
#[derive(Debug)]
pub struct ToolExecutionResult {
    /// Tool call ID.
    pub call_id: String,
    /// Tool name.
    pub name: String,
    /// Execution result.
    pub result: Result<ToolOutput>,
}

/// Streaming tool executor that manages concurrent tool execution.
///
/// This executor handles tool execution during streaming responses:
/// - Safe tools start immediately when their `ToolUse` block completes
/// - Unsafe tools are queued and executed sequentially after message_stop
///
/// ## Hook Integration
///
/// The executor supports hooks at key lifecycle points:
/// - **PreToolUse**: Before validation, can reject or modify input
/// - **PostToolUse**: After successful execution
/// - **PostToolUseFailure**: After failed execution
///
/// # Example
///
/// ```ignore
/// let executor = StreamingToolExecutor::new(registry, config, event_tx)
///     .with_hooks(hooks);
///
/// // During streaming - when content_block_stop for tool_use is received
/// executor.on_tool_complete(tool_call, ctx.clone());
///
/// // After message_stop - execute queued unsafe tools
/// executor.execute_pending_unsafe(&ctx).await;
///
/// // Wait for all tools to complete
/// let results = executor.drain().await;
/// ```
pub struct StreamingToolExecutor {
    registry: Arc<ToolRegistry>,
    config: ExecutorConfig,
    event_tx: Option<mpsc::Sender<LoopEvent>>,
    cancel_token: CancellationToken,
    approval_store: Arc<Mutex<ApprovalStore>>,
    file_tracker: Arc<Mutex<FileTracker>>,
    /// Hook registry for pre/post tool hooks.
    hooks: Option<Arc<HookRegistry>>,
    /// Tracker for async hooks running in background.
    async_hook_tracker: Arc<AsyncHookTracker>,
    /// Active tool execution tasks.
    active_tasks: Arc<Mutex<HashMap<String, JoinHandle<ToolExecutionResult>>>>,
    /// Pending unsafe tools waiting for sequential execution.
    pending_unsafe: Arc<Mutex<Vec<PendingToolCall>>>,
    /// Completed results waiting to be collected.
    completed_results: Arc<Mutex<Vec<ToolExecutionResult>>>,
    /// Shell executor for command execution and background tasks.
    shell_executor: ShellExecutor,
    /// Optional callback for spawning subagents.
    spawn_agent_fn: Option<SpawnAgentFn>,
    /// Optional skill manager for the Skill tool.
    skill_manager: Option<Arc<cocode_skill::SkillManager>>,
    /// Parent selections for subagent isolation.
    ///
    /// When spawning subagents, these selections are passed to ensure
    /// subagents are unaffected by changes to the parent's model settings.
    parent_selections: Option<cocode_protocol::RoleSelections>,
    /// Optional permission requester for interactive approval flow.
    permission_requester: Option<Arc<dyn crate::context::PermissionRequester>>,
    /// Optional permission rule evaluator.
    permission_evaluator: Option<crate::permission_rules::PermissionRuleEvaluator>,
    /// Allowlist of tool names the model was actually given.
    ///
    /// Set after `select_tools_for_model()` via [`set_allowed_tool_names`].
    /// When `Some`, only these tools can be executed; all others get `NotFound`.
    /// When `None` (default), all registered tools are executable.
    allowed_tool_names: Arc<std::sync::RwLock<Option<HashSet<String>>>>,
}

impl StreamingToolExecutor {
    /// Create a new executor.
    pub fn new(
        registry: Arc<ToolRegistry>,
        config: ExecutorConfig,
        event_tx: Option<mpsc::Sender<LoopEvent>>,
    ) -> Self {
        let shell_executor = ShellExecutor::new(config.cwd.clone());
        Self {
            registry,
            config,
            event_tx,
            cancel_token: CancellationToken::new(),
            approval_store: Arc::new(Mutex::new(ApprovalStore::new())),
            file_tracker: Arc::new(Mutex::new(FileTracker::new())),
            hooks: None,
            async_hook_tracker: Arc::new(AsyncHookTracker::new()),
            active_tasks: Arc::new(Mutex::new(HashMap::new())),
            pending_unsafe: Arc::new(Mutex::new(Vec::new())),
            completed_results: Arc::new(Mutex::new(Vec::new())),
            shell_executor,
            spawn_agent_fn: None,
            skill_manager: None,
            parent_selections: None,
            permission_requester: None,
            permission_evaluator: None,
            allowed_tool_names: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    /// Set the cancellation token.
    pub fn with_cancel_token(mut self, token: CancellationToken) -> Self {
        self.cancel_token = token;
        self
    }

    /// Set the hook registry for pre/post tool hooks.
    pub fn with_hooks(mut self, hooks: Arc<HookRegistry>) -> Self {
        self.hooks = Some(hooks);
        self
    }

    /// Set the approval store.
    pub fn with_approval_store(mut self, store: Arc<Mutex<ApprovalStore>>) -> Self {
        self.approval_store = store;
        self
    }

    /// Set the file tracker.
    pub fn with_file_tracker(mut self, tracker: Arc<Mutex<FileTracker>>) -> Self {
        self.file_tracker = tracker;
        self
    }

    /// Set the shell executor for command execution and background tasks.
    pub fn with_shell_executor(mut self, executor: ShellExecutor) -> Self {
        self.shell_executor = executor;
        self
    }

    /// Set the spawn agent callback for the Task tool.
    pub fn with_spawn_agent_fn(mut self, f: SpawnAgentFn) -> Self {
        self.spawn_agent_fn = Some(f);
        self
    }

    /// Set a custom async hook tracker.
    pub fn with_async_hook_tracker(mut self, tracker: Arc<AsyncHookTracker>) -> Self {
        self.async_hook_tracker = tracker;
        self
    }

    /// Set parent selections for subagent isolation.
    ///
    /// When spawning subagents via the Task tool, these selections will be
    /// cloned and passed to the subagent, ensuring it's unaffected by
    /// subsequent changes to the parent's model settings.
    pub fn with_parent_selections(mut self, selections: cocode_protocol::RoleSelections) -> Self {
        self.parent_selections = Some(selections);
        self
    }

    /// Set the permission requester for interactive approval flow.
    pub fn with_permission_requester(
        mut self,
        requester: Arc<dyn crate::context::PermissionRequester>,
    ) -> Self {
        self.permission_requester = Some(requester);
        self
    }

    /// Set the permission rule evaluator.
    pub fn with_permission_evaluator(
        mut self,
        evaluator: crate::permission_rules::PermissionRuleEvaluator,
    ) -> Self {
        self.permission_evaluator = Some(evaluator);
        self
    }

    /// Set the skill manager for the Skill tool.
    pub fn with_skill_manager(mut self, manager: Arc<cocode_skill::SkillManager>) -> Self {
        // Store in a way that can be passed to tool context
        // Note: The actual wiring happens in create_context
        self.skill_manager = Some(manager);
        self
    }

    /// Set the allowlist of tool names that the model was given.
    ///
    /// Called from the driver after `select_tools_for_model()` resolves the
    /// final set of definitions. Any tool call whose name is not in this set
    /// is rejected with `NotFound`, preventing hallucinated or injected calls
    /// to tools the model was never offered (e.g. `apply_patch` when
    /// `apply_patch_tool_type` is `None`, or tools outside
    /// `experimental_supported_tools`).
    pub fn set_allowed_tool_names(&self, names: HashSet<String>) {
        *self.allowed_tool_names.write().unwrap() = Some(names);
    }

    /// Check if a tool name is allowed by the current allowlist.
    ///
    /// Returns `true` if no allowlist is set (all tools allowed) or the name
    /// is in the allowlist.
    fn is_tool_allowed(&self, name: &str) -> bool {
        match self.allowed_tool_names.read().unwrap().as_ref() {
            None => true,
            Some(set) => set.contains(name),
        }
    }

    /// Get the async hook tracker for collecting completed async hooks.
    ///
    /// Call `tracker.take_completed()` to retrieve and clear completed hooks
    /// for injection into system reminders.
    ///
    /// ## Usage with System Reminders
    ///
    /// After each turn, collect completed async hooks and pass them to the
    /// system reminder generator context:
    ///
    /// ```ignore
    /// use cocode_system_reminder::{
    ///     AsyncHookResponseInfo, ASYNC_HOOK_RESPONSES_KEY,
    ///     GeneratorContextBuilder,
    /// };
    ///
    /// // Collect completed hooks
    /// let completed = executor.async_hook_tracker().take_completed();
    ///
    /// // Convert to system reminder format
    /// let responses: Vec<AsyncHookResponseInfo> = completed
    ///     .into_iter()
    ///     .map(|h| AsyncHookResponseInfo {
    ///         hook_name: h.hook_name,
    ///         additional_context: h.additional_context,
    ///         was_blocking: h.was_blocking,
    ///         blocking_reason: h.blocking_reason,
    ///         duration_ms: h.duration_ms,
    ///     })
    ///     .collect();
    ///
    /// // Pass to generator context
    /// let ctx = GeneratorContextBuilder::new(&config)
    ///     .extension(ASYNC_HOOK_RESPONSES_KEY, responses)
    ///     .build();
    /// ```
    pub fn async_hook_tracker(&self) -> &Arc<AsyncHookTracker> {
        &self.async_hook_tracker
    }

    /// Called when a tool_use block completes during streaming.
    ///
    /// For safe tools, execution starts immediately.
    /// For unsafe tools, they are queued for later execution.
    pub async fn on_tool_complete(&self, tool_call: ToolCall) {
        let call_id = &tool_call.id;
        let name = &tool_call.name;

        debug!(call_id = %call_id, name = %name, "Tool use complete");

        // Reject tools not in the model's allowlist (if set).
        // This prevents hallucinated calls to tools the model was never offered
        // (e.g., apply_patch when apply_patch_tool_type is None, or tools outside
        // experimental_supported_tools).
        if !self.is_tool_allowed(name) {
            debug!(call_id = %call_id, name = %name, "Tool not in allowed set, rejecting");
            let result =
                Err(crate::error::tool_error::NotFoundSnafu { name: name.clone() }.build());
            self.emit_completed(call_id, &result).await;
            self.completed_results
                .lock()
                .await
                .push(ToolExecutionResult {
                    call_id: call_id.clone(),
                    name: name.clone(),
                    result,
                });
            return;
        }

        // Emit queued event
        self.emit_event(LoopEvent::ToolUseQueued {
            call_id: call_id.clone(),
            name: name.clone(),
            input: tool_call.arguments.clone(),
        })
        .await;

        // Check if tool exists and get concurrency safety
        let tool = match self.registry.get(name) {
            Some(t) => t,
            None => {
                // Queue for later - might be MCP tool
                self.pending_unsafe.lock().await.push(PendingToolCall {
                    tool_call,
                    queued_at: std::time::Instant::now(),
                });
                return;
            }
        };

        let concurrency = tool.concurrency_safety();

        match concurrency {
            ConcurrencySafety::Safe => {
                // Check concurrency limit
                let active_count = self.active_tasks.lock().await.len();
                if active_count >= self.config.max_concurrency as usize {
                    // Queue instead of starting immediately
                    self.pending_unsafe.lock().await.push(PendingToolCall {
                        tool_call,
                        queued_at: std::time::Instant::now(),
                    });
                    return;
                }

                // Start immediately
                self.start_tool_execution(tool_call).await;
            }
            ConcurrencySafety::Unsafe => {
                // Queue for sequential execution
                self.pending_unsafe.lock().await.push(PendingToolCall {
                    tool_call,
                    queued_at: std::time::Instant::now(),
                });
            }
        }
    }

    /// Start tool execution in a background task.
    async fn start_tool_execution(&self, tool_call: ToolCall) {
        let call_id = tool_call.id.clone();
        let name = tool_call.name.clone();
        let original_input = tool_call.arguments.clone();

        // Execute pre-hooks before starting the tool
        let modified_input = match self.execute_pre_hooks(&name, original_input.clone()).await {
            Ok(input) => input,
            Err(reason) => {
                // Pre-hook rejected the tool call
                let result = Err(crate::error::tool_error::HookRejectedSnafu { reason }.build());
                self.emit_completed(&call_id, &result).await;
                self.completed_results
                    .lock()
                    .await
                    .push(ToolExecutionResult {
                        call_id,
                        name,
                        result,
                    });
                return;
            }
        };

        // Emit started event
        self.emit_event(LoopEvent::ToolUseStarted {
            call_id: call_id.clone(),
            name: name.clone(),
        })
        .await;

        // Create context for this execution
        let ctx = self.create_context(&call_id);

        // Clone what we need for the task
        let registry = self.registry.clone();
        let timeout_secs = self.config.default_timeout_secs;

        // Create modified tool call with potentially modified input
        let modified_tool_call = ToolCall::new(&call_id, &name, modified_input.clone());

        // Clone hooks for post-hook execution
        let hooks = self.hooks.clone();
        let session_id = self.config.session_id.clone();
        let cwd = self.config.cwd.clone();

        // Clone persistence config for result handling
        let session_dir = self.config.session_dir.clone();
        let tool_config = self.config.tool_config.clone();
        let call_id_for_persistence = call_id.clone();
        let max_tool_output_chars = self.config.max_tool_output_chars;

        // Spawn the execution task
        let handle = tokio::spawn(async move {
            let mut result = execute_tool(
                &registry,
                modified_tool_call,
                ctx,
                timeout_secs,
                max_tool_output_chars,
            )
            .await;

            // Apply large result persistence if configured
            if let (Ok(output), Some(dir)) = (&result, &session_dir) {
                let persisted = result_persistence::persist_if_needed(
                    output.clone(),
                    &call_id_for_persistence,
                    dir,
                    &tool_config,
                )
                .await;
                result = Ok(persisted);
            }

            // Execute post-hooks within the spawned task
            if let Some(hooks) = hooks {
                let is_error = result.is_err();
                let event_type = if is_error {
                    HookEventType::PostToolUseFailure
                } else {
                    HookEventType::PostToolUse
                };

                let hook_ctx =
                    HookContext::new(event_type, session_id, cwd).with_tool(&name, modified_input);

                let outcomes = hooks.execute(&hook_ctx).await;
                for outcome in outcomes {
                    if let HookResult::Reject { reason } = outcome.result {
                        warn!(
                            tool = %name,
                            hook = %outcome.hook_name,
                            reason = %reason,
                            "Post-hook returned rejection (logged but result unchanged)"
                        );
                    }
                }
            }

            ToolExecutionResult {
                call_id,
                name,
                result,
            }
        });

        self.active_tasks.lock().await.insert(tool_call.id, handle);
    }

    /// Execute queued unsafe tools sequentially.
    pub async fn execute_pending_unsafe(&self) {
        let pending = {
            let mut lock = self.pending_unsafe.lock().await;
            std::mem::take(&mut *lock)
        };

        for pending_call in pending {
            if self.cancel_token.is_cancelled() {
                break;
            }

            let tool_call = pending_call.tool_call;
            let call_id = tool_call.id.clone();
            let name = tool_call.name.clone();
            let original_input = tool_call.arguments.clone();

            // Reject tools not in the model's allowlist (if set)
            if !self.is_tool_allowed(&name) {
                debug!(call_id = %call_id, name = %name, "Tool not in allowed set, rejecting");
                let result =
                    Err(crate::error::tool_error::NotFoundSnafu { name: name.clone() }.build());
                self.emit_completed(&call_id, &result).await;
                self.completed_results
                    .lock()
                    .await
                    .push(ToolExecutionResult {
                        call_id,
                        name,
                        result,
                    });
                continue;
            }

            // Execute pre-hooks before starting the tool
            let modified_input = match self.execute_pre_hooks(&name, original_input.clone()).await {
                Ok(input) => input,
                Err(reason) => {
                    // Pre-hook rejected the tool call
                    let result =
                        Err(crate::error::tool_error::HookRejectedSnafu { reason }.build());
                    self.emit_completed(&call_id, &result).await;
                    self.completed_results
                        .lock()
                        .await
                        .push(ToolExecutionResult {
                            call_id,
                            name,
                            result,
                        });
                    continue;
                }
            };

            // Emit started event
            self.emit_event(LoopEvent::ToolUseStarted {
                call_id: call_id.clone(),
                name: name.clone(),
            })
            .await;

            // Create context and execute with potentially modified input
            let ctx = self.create_context(&call_id);
            let modified_tool_call = ToolCall::new(&call_id, &name, modified_input.clone());
            let mut result = execute_tool(
                &self.registry,
                modified_tool_call,
                ctx,
                self.config.default_timeout_secs,
                self.config.max_tool_output_chars,
            )
            .await;

            // Apply large result persistence if configured
            if let (Ok(output), Some(dir)) = (&result, &self.config.session_dir) {
                let persisted = result_persistence::persist_if_needed(
                    output.clone(),
                    &call_id,
                    dir,
                    &self.config.tool_config,
                )
                .await;
                result = Ok(persisted);
            }

            // Execute post-hooks
            let is_error = result.is_err();
            self.execute_post_hooks(&name, &modified_input, is_error)
                .await;

            // Emit completed event
            self.emit_completed(&call_id, &result).await;

            // Store result
            self.completed_results
                .lock()
                .await
                .push(ToolExecutionResult {
                    call_id,
                    name,
                    result,
                });
        }
    }

    /// Wait for all active tasks and return their results.
    pub async fn drain(&self) -> Vec<ToolExecutionResult> {
        // Wait for all active tasks
        let tasks: Vec<_> = {
            let mut lock = self.active_tasks.lock().await;
            lock.drain().collect()
        };

        for (call_id, handle) in tasks {
            match handle.await {
                Ok(result) => {
                    self.emit_completed(&result.call_id, &result.result).await;
                    self.completed_results.lock().await.push(result);
                }
                Err(e) => {
                    error!(call_id = %call_id, error = %e, "Task panicked");
                    let result = Err(crate::error::tool_error::InternalSnafu {
                        message: format!("Tool execution task panicked (call_id: {call_id}): {e}"),
                    }
                    .build());
                    self.emit_completed(&call_id, &result).await;
                    self.completed_results
                        .lock()
                        .await
                        .push(ToolExecutionResult {
                            call_id: call_id.clone(),
                            name: format!("<panicked:{call_id}>"),
                            result,
                        });
                }
            }
        }

        // Return all completed results
        let mut results = self.completed_results.lock().await;
        std::mem::take(&mut *results)
    }

    /// Abort a running tool by call ID.
    pub async fn abort(&self, call_id: &str, reason: AbortReason) {
        // Cancel the token associated with this tool
        // Note: In a full implementation, each tool would have its own cancel token
        info!(call_id = %call_id, reason = ?reason, "Aborting tool");

        // Remove from active tasks
        if let Some(handle) = self.active_tasks.lock().await.remove(call_id) {
            handle.abort();
        }

        // Emit aborted event
        self.emit_event(LoopEvent::ToolExecutionAborted { reason })
            .await;
    }

    /// Abort all running and pending tools.
    pub async fn abort_all(&self, reason: AbortReason) {
        // Cancel all active tasks
        let tasks: Vec<_> = {
            let mut lock = self.active_tasks.lock().await;
            lock.drain().collect()
        };

        for (_, handle) in tasks {
            handle.abort();
        }

        // Clear pending
        self.pending_unsafe.lock().await.clear();

        // Emit aborted event
        self.emit_event(LoopEvent::ToolExecutionAborted { reason })
            .await;
    }

    /// Get the number of active tasks.
    pub async fn active_count(&self) -> usize {
        self.active_tasks.lock().await.len()
    }

    /// Get the number of pending unsafe tasks.
    pub async fn pending_count(&self) -> usize {
        self.pending_unsafe.lock().await.len()
    }

    /// Create a tool context for execution.
    fn create_context(&self, call_id: &str) -> ToolContext {
        let mut builder = ToolContextBuilder::new(call_id, &self.config.session_id)
            .cwd(self.shell_executor.cwd())
            .permission_mode(self.config.permission_mode)
            .cancel_token(self.cancel_token.clone())
            .approval_store(self.approval_store.clone())
            .file_tracker(self.file_tracker.clone())
            .plan_mode(self.config.is_plan_mode, self.config.plan_file_path.clone())
            .features(self.config.features.clone())
            .shell_executor(self.shell_executor.clone());

        // Add spawn_agent_fn if available
        if let Some(ref spawn_fn) = self.spawn_agent_fn {
            builder = builder.spawn_agent_fn(spawn_fn.clone());
        }

        // Add skill_manager if available
        if let Some(ref sm) = self.skill_manager {
            builder = builder.skill_manager(sm.clone());
        }

        // Add session_dir if available
        if let Some(ref dir) = self.config.session_dir {
            builder = builder.session_dir(dir.clone());
        }

        // Add parent_selections for subagent isolation
        if let Some(ref selections) = self.parent_selections {
            builder = builder.parent_selections(selections.clone());
        }

        // Add permission requester for interactive approval flow
        if let Some(ref requester) = self.permission_requester {
            builder = builder.permission_requester(requester.clone());
        }

        // Add permission rule evaluator
        if let Some(ref evaluator) = self.permission_evaluator {
            builder = builder.permission_evaluator(evaluator.clone());
        }

        builder.build()
    }

    /// Emit a loop event.
    async fn emit_event(&self, event: LoopEvent) {
        if let Some(tx) = &self.event_tx {
            if let Err(e) = tx.send(event).await {
                debug!("Failed to send tool event: {e}");
            }
        }
    }

    /// Emit a completed event.
    async fn emit_completed(&self, call_id: &str, result: &Result<ToolOutput>) {
        let (output, is_error) = match result {
            Ok(output) => (output.content.clone(), output.is_error),
            Err(e) => (
                cocode_protocol::ToolResultContent::Text(e.to_string()),
                true,
            ),
        };

        self.emit_event(LoopEvent::ToolUseCompleted {
            call_id: call_id.to_string(),
            output,
            is_error,
        })
        .await;
    }

    /// Execute pre-tool-use hooks and return the (possibly modified) input.
    ///
    /// Returns `None` if the tool call should be rejected.
    async fn execute_pre_hooks(
        &self,
        tool_name: &str,
        input: Value,
    ) -> std::result::Result<Value, String> {
        let hooks = match &self.hooks {
            Some(h) => h,
            None => return Ok(input),
        };

        let ctx = HookContext::new(
            HookEventType::PreToolUse,
            self.config.session_id.clone(),
            self.config.cwd.clone(),
        )
        .with_tool(tool_name, input.clone());

        let outcomes = hooks.execute(&ctx).await;
        let mut current_input = input;

        for outcome in outcomes {
            // Emit hook executed event
            self.emit_event(LoopEvent::HookExecuted {
                hook_type: cocode_protocol::HookEventType::PreToolCall,
                hook_name: outcome.hook_name.clone(),
            })
            .await;

            match outcome.result {
                HookResult::Continue | HookResult::ContinueWithContext { .. } => {
                    // Continue with current input
                }
                HookResult::Reject { reason } => {
                    warn!(
                        tool = %tool_name,
                        hook = %outcome.hook_name,
                        reason = %reason,
                        "Tool call rejected by pre-hook"
                    );
                    return Err(reason);
                }
                HookResult::ModifyInput { new_input } => {
                    debug!(
                        tool = %tool_name,
                        hook = %outcome.hook_name,
                        "Tool input modified by pre-hook"
                    );
                    current_input = new_input;
                }
                HookResult::Async { task_id, hook_name } => {
                    // Register async hook for tracking - result will be delivered via system reminders
                    self.async_hook_tracker
                        .register(task_id.clone(), hook_name.clone());
                    debug!(
                        tool = %tool_name,
                        task_id = %task_id,
                        async_hook = %hook_name,
                        "Async hook registered and running in background"
                    );
                }
            }
        }

        Ok(current_input)
    }

    /// Execute post-tool-use hooks.
    async fn execute_post_hooks(&self, tool_name: &str, input: &Value, is_error: bool) {
        let hooks = match &self.hooks {
            Some(h) => h,
            None => return,
        };

        let event_type = if is_error {
            HookEventType::PostToolUseFailure
        } else {
            HookEventType::PostToolUse
        };

        let ctx = HookContext::new(
            event_type,
            self.config.session_id.clone(),
            self.config.cwd.clone(),
        )
        .with_tool(tool_name, input.clone());

        let outcomes = hooks.execute(&ctx).await;

        for outcome in outcomes {
            let hook_type = if is_error {
                cocode_protocol::HookEventType::PostToolCallFailure
            } else {
                cocode_protocol::HookEventType::PostToolCall
            };

            self.emit_event(LoopEvent::HookExecuted {
                hook_type,
                hook_name: outcome.hook_name.clone(),
            })
            .await;

            // Post hooks can only continue or reject; rejection is logged but doesn't change the result
            if let HookResult::Reject { reason } = outcome.result {
                warn!(
                    tool = %tool_name,
                    hook = %outcome.hook_name,
                    reason = %reason,
                    "Post-hook returned rejection (logged but result unchanged)"
                );
            }
        }
    }
}

/// Execute a single tool with timeout.
async fn execute_tool(
    registry: &ToolRegistry,
    tool_call: ToolCall,
    mut ctx: ToolContext,
    timeout_secs: i64,
    max_tool_output_chars: Option<i32>,
) -> Result<ToolOutput> {
    let timeout_duration = std::time::Duration::from_secs(timeout_secs as u64);

    match tokio::time::timeout(
        timeout_duration,
        execute_tool_inner(registry, tool_call, &mut ctx, max_tool_output_chars),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err(crate::error::tool_error::TimeoutSnafu { timeout_secs }.build()),
    }
}

/// Check if a tool name is an edit/write tool (for AcceptEdits mode).
fn is_edit_tool(name: &str) -> bool {
    matches!(name, "Edit" | "Write" | "NotebookEdit" | "ApplyPatch")
}

/// Check if a tool name is read-only or a plan mode control tool.
fn is_read_only_or_plan_tool(name: &str) -> bool {
    matches!(
        name,
        "Read"
            | "Glob"
            | "Grep"
            | "TaskOutput"
            | "EnterPlanMode"
            | "ExitPlanMode"
            | "AskUserQuestion"
            | "Lsp"
    )
}

/// Extract file_path from tool input if present.
fn extract_file_path(input: &Value) -> Option<std::path::PathBuf> {
    input
        .get("file_path")
        .and_then(|v| v.as_str())
        .map(std::path::PathBuf::from)
}

/// Extract a command prefix pattern for the "allow similar commands" option.
///
/// For Bash commands, extracts the first word as a prefix pattern.
/// E.g. `"git push origin main"` → `Some("git *")`.
fn extract_prefix_pattern(tool_name: &str, input: &Value) -> Option<String> {
    if tool_name != "Bash" {
        return None;
    }
    let command = input.get("command").and_then(|v| v.as_str())?;
    let first_word = command.split_whitespace().next()?;
    if first_word.is_empty() {
        return None;
    }
    Some(format!("{first_word} *"))
}

/// Build a default approval request for a tool that needs user approval.
fn default_approval_request(name: &str, input: &Value) -> cocode_protocol::ApprovalRequest {
    let description = if let Some(path) = extract_file_path(input) {
        format!("{name}: {}", path.display())
    } else if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
        let truncated = if cmd.len() > 80 {
            format!("{}...", &cmd[..80])
        } else {
            cmd.to_string()
        };
        format!("{name}: {truncated}")
    } else {
        format!("Execute tool: {name}")
    };

    let proposed_prefix_pattern = extract_prefix_pattern(name, input);

    cocode_protocol::ApprovalRequest {
        request_id: format!(
            "default-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ),
        tool_name: name.to_string(),
        description,
        risks: vec![],
        allow_remember: true,
        proposed_prefix_pattern,
    }
}

/// Extract command string from shell tool input.
fn extract_command_input(name: &str, input: &Value) -> Option<String> {
    match name {
        "Bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .map(String::from),
        "shell" => input.get("command").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        }),
        _ => None,
    }
}

/// Full permission pipeline (5 stages) aligned with Claude Code v2.1.7.
///
/// 1. Check DENY rules (all sources) → if match → Deny
/// 2. Check ASK rules (all sources) → if match → NeedsApproval
/// 3. Tool-specific check_permission() → returns allow/deny/ask/passthrough
/// 4. Check ALLOW rules (all sources) → if match → Allow
/// 5. Default behavior: reads → Allow, writes → NeedsApproval
async fn check_permission_pipeline(
    tool: &dyn crate::tool::Tool,
    name: &str,
    input: &Value,
    ctx: &ToolContext,
) -> cocode_protocol::PermissionResult {
    let file_path = extract_file_path(input);
    let command_input = extract_command_input(name, input);

    if let Some(ref evaluator) = ctx.permission_evaluator {
        // Stage 1: Check DENY rules
        if let Some(decision) = evaluator.evaluate_behavior(
            name,
            file_path.as_deref(),
            crate::permission_rules::RuleAction::Deny,
            command_input.as_deref(),
        ) {
            return decision.result;
        }

        // Stage 2: Check ASK rules
        if let Some(decision) = evaluator.evaluate_behavior(
            name,
            file_path.as_deref(),
            crate::permission_rules::RuleAction::Ask,
            command_input.as_deref(),
        ) {
            // ASK rule matched — the tool must ask for approval
            return cocode_protocol::PermissionResult::NeedsApproval {
                request: cocode_protocol::ApprovalRequest {
                    request_id: format!("rule-ask-{name}"),
                    tool_name: name.to_string(),
                    description: decision.reason,
                    risks: vec![],
                    allow_remember: true,
                    proposed_prefix_pattern: extract_prefix_pattern(name, input),
                },
            };
        }
    }

    // Stage 3: Tool-specific check
    let tool_result = tool.check_permission(input, ctx).await;
    if !tool_result.is_passthrough() {
        return tool_result;
    }

    if let Some(ref evaluator) = ctx.permission_evaluator {
        // Stage 4: Check ALLOW rules
        if let Some(decision) = evaluator.evaluate_behavior(
            name,
            file_path.as_deref(),
            crate::permission_rules::RuleAction::Allow,
            command_input.as_deref(),
        ) {
            if decision.result.is_allowed() {
                return cocode_protocol::PermissionResult::Allowed;
            }
        }
    }

    // Stage 5: Default behavior
    if tool.is_read_only() {
        cocode_protocol::PermissionResult::Allowed
    } else {
        cocode_protocol::PermissionResult::NeedsApproval {
            request: default_approval_request(name, input),
        }
    }
}

/// Apply permission mode on top of pipeline result.
///
/// Converts results based on the current mode:
/// - Bypass: everything → Allowed
/// - DontAsk: NeedsApproval → Denied
/// - AcceptEdits: edit/write NeedsApproval → Allowed
/// - Plan: non-read-only → Denied
fn apply_permission_mode(
    result: cocode_protocol::PermissionResult,
    mode: PermissionMode,
    tool_name: &str,
) -> cocode_protocol::PermissionResult {
    match mode {
        PermissionMode::Bypass => cocode_protocol::PermissionResult::Allowed,
        PermissionMode::DontAsk => match result {
            cocode_protocol::PermissionResult::NeedsApproval { request } => {
                cocode_protocol::PermissionResult::Denied {
                    reason: format!(
                        "DontAsk mode: permission prompt suppressed for '{}': {}",
                        tool_name, request.description
                    ),
                }
            }
            other => other,
        },
        PermissionMode::AcceptEdits if is_edit_tool(tool_name) => match result {
            cocode_protocol::PermissionResult::NeedsApproval { .. } => {
                cocode_protocol::PermissionResult::Allowed
            }
            other => other,
        },
        PermissionMode::Plan if !is_read_only_or_plan_tool(tool_name) => {
            // In plan mode, deny all non-read-only tools (unless already allowed/denied)
            match result {
                cocode_protocol::PermissionResult::Allowed
                | cocode_protocol::PermissionResult::NeedsApproval { .. } => {
                    cocode_protocol::PermissionResult::Denied {
                        reason: "Plan mode: only read-only tools allowed".to_string(),
                    }
                }
                other => other,
            }
        }
        _ => result,
    }
}

/// Inner tool execution logic (without timeout).
async fn execute_tool_inner(
    registry: &ToolRegistry,
    tool_call: ToolCall,
    ctx: &mut ToolContext,
    max_tool_output_chars: Option<i32>,
) -> Result<ToolOutput> {
    let name = &tool_call.name;
    let input = tool_call.arguments;

    // Get the tool
    let tool = registry
        .get(name)
        .ok_or_else(|| crate::error::tool_error::NotFoundSnafu { name: name.clone() }.build())?;

    // Defense-in-depth: reject calls to feature-gated tools that are disabled.
    // Normally the model never sees these (definitions_filtered excludes them),
    // but a hallucinated or injected tool name could still reach here.
    if let Some(feature) = tool.feature_gate() {
        if !ctx.features.enabled(feature) {
            return Err(crate::error::tool_error::NotFoundSnafu { name: name.clone() }.build());
        }
    }

    // Validate input
    let validation = tool.validate(&input).await;
    if let ValidationResult::Invalid { errors } = validation {
        let error_msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        return Err(crate::error::tool_error::InvalidInputSnafu {
            message: error_msgs.join(", "),
        }
        .build());
    }

    // Run the full permission pipeline
    let pipeline_result = check_permission_pipeline(tool.as_ref(), name, &input, ctx).await;

    // Apply permission mode on top
    let permission = apply_permission_mode(pipeline_result, ctx.permission_mode, name);

    match permission {
        cocode_protocol::PermissionResult::Allowed => {}
        cocode_protocol::PermissionResult::Denied { reason } => {
            return Err(
                crate::error::tool_error::PermissionDeniedSnafu { message: reason }.build(),
            );
        }
        cocode_protocol::PermissionResult::NeedsApproval { request } => {
            // Check ApprovalStore first
            let pattern = &request.description;
            if ctx.is_approved(name, pattern).await {
                // Already approved for this pattern (exact or wildcard)
            } else if let Some(requester) = &ctx.permission_requester {
                // Use the permission requester for interactive approval
                let worker_id = ctx.call_id.clone();
                let decision = requester
                    .request_permission(request.clone(), &worker_id)
                    .await;
                match decision {
                    cocode_protocol::ApprovalDecision::Denied => {
                        return Err(crate::error::tool_error::PermissionDeniedSnafu {
                            message: format!("User denied permission for tool '{name}'"),
                        }
                        .build());
                    }
                    cocode_protocol::ApprovalDecision::Approved => {
                        // Session-only: remember exact description
                        ctx.approve_pattern(name, pattern).await;
                    }
                    cocode_protocol::ApprovalDecision::ApprovedWithPrefix { prefix_pattern } => {
                        // Remember prefix pattern in session + persist to disk
                        ctx.approve_pattern(name, &prefix_pattern).await;
                        ctx.persist_permission_rule(name, &prefix_pattern).await;
                    }
                }
            } else {
                // No permission requester available - deny
                return Err(crate::error::tool_error::PermissionDeniedSnafu {
                    message: format!("Tool '{name}' requires approval: {}", request.description),
                }
                .build());
            }
        }
        cocode_protocol::PermissionResult::Passthrough => {
            // Should not happen after pipeline — treat as allowed
        }
    }

    // Execute
    let result = tool.execute(input, ctx).await;

    // Post-process
    let mut output = match result {
        Ok(output) => tool.post_process(output, ctx).await,
        Err(e) => return Err(e),
    };

    // Apply truncation: use the smaller of per-tool limit and model-level limit.
    // Single pass avoids double truncation markers.
    let per_tool = tool.max_result_size_chars() as usize;
    let max_chars = match max_tool_output_chars {
        Some(model_limit) => per_tool.min(model_limit as usize),
        None => per_tool,
    };
    output.truncate_to(max_chars);

    // Cleanup
    tool.cleanup(ctx).await;

    Ok(output)
}

impl std::fmt::Debug for StreamingToolExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamingToolExecutor")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::Tool;
    use async_trait::async_trait;
    use serde_json::Value;

    struct SafeTool;

    #[async_trait]
    impl Tool for SafeTool {
        fn name(&self) -> &str {
            "safe_tool"
        }
        fn description(&self) -> &str {
            "A safe tool"
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({"type": "object"})
        }
        fn concurrency_safety(&self) -> ConcurrencySafety {
            ConcurrencySafety::Safe
        }
        async fn execute(&self, _input: Value, _ctx: &mut ToolContext) -> Result<ToolOutput> {
            Ok(ToolOutput::text("safe result"))
        }
    }

    struct UnsafeTool;

    #[async_trait]
    impl Tool for UnsafeTool {
        fn name(&self) -> &str {
            "unsafe_tool"
        }
        fn description(&self) -> &str {
            "An unsafe tool"
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({"type": "object"})
        }
        fn concurrency_safety(&self) -> ConcurrencySafety {
            ConcurrencySafety::Unsafe
        }
        async fn execute(&self, _input: Value, _ctx: &mut ToolContext) -> Result<ToolOutput> {
            Ok(ToolOutput::text("unsafe result"))
        }
    }

    #[tokio::test]
    async fn test_executor_safe_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(SafeTool);

        let executor =
            StreamingToolExecutor::new(Arc::new(registry), ExecutorConfig::default(), None);

        let tool_call = ToolCall::new("call-1", "safe_tool", serde_json::json!({}));
        executor.on_tool_complete(tool_call).await;

        // Safe tool should start immediately
        assert_eq!(executor.active_count().await, 1);
        assert_eq!(executor.pending_count().await, 0);

        let results = executor.drain().await;
        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_ok());
    }

    #[tokio::test]
    async fn test_executor_unsafe_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(UnsafeTool);

        let executor =
            StreamingToolExecutor::new(Arc::new(registry), ExecutorConfig::default(), None);

        let tool_call = ToolCall::new("call-1", "unsafe_tool", serde_json::json!({}));
        executor.on_tool_complete(tool_call).await;

        // Unsafe tool should be queued
        assert_eq!(executor.active_count().await, 0);
        assert_eq!(executor.pending_count().await, 1);

        // Execute pending
        executor.execute_pending_unsafe().await;

        let results = executor.drain().await;
        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_ok());
    }

    /// A tool gated on a feature flag.
    struct FeatureGatedTool;

    #[async_trait]
    impl Tool for FeatureGatedTool {
        fn name(&self) -> &str {
            "gated_tool"
        }
        fn description(&self) -> &str {
            "A feature-gated tool"
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({"type": "object"})
        }
        fn feature_gate(&self) -> Option<cocode_protocol::Feature> {
            Some(cocode_protocol::Feature::Ls)
        }
        async fn execute(&self, _input: Value, _ctx: &mut ToolContext) -> Result<ToolOutput> {
            Ok(ToolOutput::text("gated result"))
        }
    }

    #[tokio::test]
    async fn test_feature_gated_tool_rejected_when_disabled() {
        let mut registry = ToolRegistry::new();
        registry.register(FeatureGatedTool);

        // Disable the Ls feature
        let mut features = cocode_protocol::Features::with_defaults();
        features.disable(cocode_protocol::Feature::Ls);

        let config = ExecutorConfig {
            features,
            ..ExecutorConfig::default()
        };
        let executor = StreamingToolExecutor::new(Arc::new(registry), config, None);

        let tool_call = ToolCall::new("call-1", "gated_tool", serde_json::json!({}));
        executor.on_tool_complete(tool_call).await;
        executor.execute_pending_unsafe().await;

        let results = executor.drain().await;
        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_err());
        let err = results[0].result.as_ref().unwrap_err().to_string();
        assert!(
            err.contains("not found") || err.contains("NotFound"),
            "Expected NotFound error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_executor_not_found() {
        let registry = ToolRegistry::new();
        let executor =
            StreamingToolExecutor::new(Arc::new(registry), ExecutorConfig::default(), None);

        let tool_call = ToolCall::new("call-1", "nonexistent", serde_json::json!({}));
        executor.on_tool_complete(tool_call).await;

        // Should be queued since tool not found
        assert_eq!(executor.pending_count().await, 1);

        // Execute pending - should fail
        executor.execute_pending_unsafe().await;

        let results = executor.drain().await;
        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_err());
    }

    #[tokio::test]
    async fn test_allowed_tool_names_rejects_unlisted_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(SafeTool);
        registry.register(UnsafeTool);

        let executor =
            StreamingToolExecutor::new(Arc::new(registry), ExecutorConfig::default(), None);

        // Only allow safe_tool — unsafe_tool is registered but not in the allowlist
        executor.set_allowed_tool_names(vec!["safe_tool".to_string()].into_iter().collect());

        // safe_tool → should succeed
        let tool_call = ToolCall::new("call-1", "safe_tool", serde_json::json!({}));
        executor.on_tool_complete(tool_call).await;

        // unsafe_tool → should be rejected immediately by allowlist
        let tool_call = ToolCall::new("call-2", "unsafe_tool", serde_json::json!({}));
        executor.on_tool_complete(tool_call).await;

        executor.execute_pending_unsafe().await;
        let results = executor.drain().await;

        assert_eq!(results.len(), 2);

        let safe_result = results.iter().find(|r| r.call_id == "call-1").unwrap();
        assert!(safe_result.result.is_ok(), "safe_tool should succeed");

        let unsafe_result = results.iter().find(|r| r.call_id == "call-2").unwrap();
        assert!(
            unsafe_result.result.is_err(),
            "unsafe_tool should be rejected"
        );
        let err = unsafe_result.result.as_ref().unwrap_err().to_string();
        assert!(
            err.contains("not found") || err.contains("NotFound"),
            "Expected NotFound error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_no_allowlist_allows_all_tools() {
        let mut registry = ToolRegistry::new();
        registry.register(SafeTool);

        let executor =
            StreamingToolExecutor::new(Arc::new(registry), ExecutorConfig::default(), None);

        // No allowlist set → all registered tools should work
        let tool_call = ToolCall::new("call-1", "safe_tool", serde_json::json!({}));
        executor.on_tool_complete(tool_call).await;

        let results = executor.drain().await;
        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_ok());
    }

    #[test]
    fn test_extract_prefix_pattern_bash_command() {
        let input = serde_json::json!({"command": "git push origin main"});
        assert_eq!(
            extract_prefix_pattern("Bash", &input),
            Some("git *".to_string())
        );
    }

    #[test]
    fn test_extract_prefix_pattern_bash_single_word() {
        let input = serde_json::json!({"command": "ls"});
        assert_eq!(
            extract_prefix_pattern("Bash", &input),
            Some("ls *".to_string())
        );
    }

    #[test]
    fn test_extract_prefix_pattern_non_bash_tool() {
        let input = serde_json::json!({"command": "git push"});
        assert_eq!(extract_prefix_pattern("Read", &input), None);
        assert_eq!(extract_prefix_pattern("Edit", &input), None);
        assert_eq!(extract_prefix_pattern("Write", &input), None);
    }

    #[test]
    fn test_extract_prefix_pattern_missing_command() {
        let input = serde_json::json!({"file_path": "/tmp/test"});
        assert_eq!(extract_prefix_pattern("Bash", &input), None);
    }

    #[test]
    fn test_extract_prefix_pattern_empty_command() {
        let input = serde_json::json!({"command": ""});
        assert_eq!(extract_prefix_pattern("Bash", &input), None);
    }

    #[test]
    fn test_extract_prefix_pattern_whitespace_only() {
        let input = serde_json::json!({"command": "   "});
        assert_eq!(extract_prefix_pattern("Bash", &input), None);
    }

    #[test]
    fn test_extract_prefix_pattern_complex_command() {
        let input = serde_json::json!({"command": "cargo test --no-fail-fast -- -q"});
        assert_eq!(
            extract_prefix_pattern("Bash", &input),
            Some("cargo *".to_string())
        );
    }
}
