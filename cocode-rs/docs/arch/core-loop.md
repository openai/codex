# Agent Loop Implementation

## Overview

The agent loop is the core execution engine, modeled after Claude Code's `coreMessageLoop`. It features:

- **Streaming tool execution**: Tools execute DURING API streaming, not after
- **Concurrency-safe execution**: Parallel for safe tools, sequential for unsafe
- **Auto-compaction**: Automatic context summarization when approaching limits
- **Model fallback**: Switch to fallback model on overload
- **Event-driven**: All state changes emit `LoopEvent` for UI integration

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         AgentLoop                                │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────────┐   │
│  │   Model     │ │ToolRegistry │ │    Context              │   │
│  │  (hyper)    │ │             │ │                         │   │
│  └─────────────┘ └─────────────┘ └─────────────────────────┘   │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │              StreamingToolExecutor                          │ │
│  │  - Execute tools DURING API streaming                      │ │
│  │  - Parallel for concurrency-safe tools                     │ │
│  │  - Sequential for unsafe tools                             │ │
│  │  - Results returned as they complete                       │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │              Event Channel                                  │ │
│  │  mpsc::Sender<LoopEvent> → UI / app-server / TUI          │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Complete Loop Flow

Claude Code v2.1.7 uses an 18-step algorithm. This diagram shows the high-level flow:

```
User Input
    │
    ▼
┌───────────────────────────────────────────────────────────────┐
│                    coreMessageLoop (18 Steps)                  │
│                                                                │
│  SETUP PHASE (Steps 1-6)                                      │
│  ─────────────────────────────────────────────────────────    │
│  1. Signal stream_request_start                               │
│  2. Setup query tracking (chainId, depth for telemetry)       │
│  3. Normalize messages (slice from compact boundary)          │
│  4. Micro-compaction (PRE-API: clear old tool results)        │
│  5. Auto-compaction threshold check (context_limit - 13K)     │
│     ├─ Tier 1: Session Memory Compact (cached summary)        │
│     └─ Tier 2: Full Compact (LLM-based fallback)              │
│  6. Initialize state (tool executor, turn tracking)           │
│                                                                │
│  EXECUTION PHASE (Steps 7-10)                                 │
│  ─────────────────────────────────────────────────────────    │
│  7. Resolve model with permissions                            │
│  8. Check blocking token limit                                │
│  ┌────────────────────────────────────────────────────────┐   │
│  │  9. API Streaming Loop (with retry, max 3 attempts)    │   │
│  │     Model.stream() ──► SSE Events ──► emit to UI       │   │
│  │           │                                             │   │
│  │           ├─► content_block_delta → TextDelta          │   │
│  │           ├─► content_block_stop → ToolCallComplete    │   │
│  │           │         │                                   │   │
│  │           │         ▼                                   │   │
│  │           │   StreamingToolExecutor.add_tool()         │   │
│  │           │   (Execute DURING streaming)               │   │
│  │           │                                             │   │
│  │           └─► (if overloaded) ModelFallbackError       │   │
│  │               tombstone_orphaned_messages()            │   │
│  │               switch_to_fallback_model()               │   │
│  └────────────────────────────────────────────────────────┘   │
│  10. Record API call info (telemetry)                         │
│                                                                │
│  POST-PROCESSING PHASE (Steps 11-18)                          │
│  ─────────────────────────────────────────────────────────    │
│  11. Check for tool calls                                     │
│  12. Execute tool queue (parallel for safe, sequential)       │
│  13. Handle abort after tool execution                        │
│  14. Check for hook stop                                      │
│  15. Update auto-compact tracking                             │
│  16. Process queued commands and attachments                  │
│  17. Check max turns limit                                    │
│  18. Recursive call for next turn (if tool_use)              │
│                                                                │
└───────────────────────────────────────────────────────────────┘
            │
            ▼ (stop_reason != tool_use)
       Final Response
```

## Core Types

### LoopConfig (Complete)

```rust
pub struct LoopConfig {
    /// Maximum turns before stopping
    pub max_turns: Option<i32>,

    /// Maximum tokens per response
    pub max_tokens: Option<i32>,

    /// Context window usage threshold for auto-compaction (0.0-1.0)
    pub auto_compact_threshold: f32,

    /// Permission mode for tool execution
    pub permission_mode: PermissionMode,

    /// Enable streaming tool execution (execute during API streaming)
    pub enable_streaming_tools: bool,

    /// Enable micro-compaction (remove low-value tool results)
    pub enable_micro_compaction: bool,

    /// Model fallback on overload
    pub fallback_model: Option<String>,

    /// Agent ID (for subagents)
    pub agent_id: Option<String>,

    /// Parent agent ID (for tracking)
    pub parent_agent_id: Option<String>,

    /// Enable sidechain transcript recording
    pub record_sidechain: bool,

    /// Session memory configuration (for file restoration after compaction)
    pub session_memory: SessionMemoryConfig,

    /// Stream stall detection configuration
    pub stall_detection: StallDetectionConfig,

    /// Prompt caching configuration
    pub prompt_caching: PromptCachingConfig,
}

/// Session memory configuration for file restoration after compaction
#[derive(Debug, Clone)]
pub struct SessionMemoryConfig {
    /// Token budget for session memory (default: 50k tokens)
    pub budget_tokens: i32,
    /// Priority for file restoration
    pub restoration_priority: FileRestorationPriority,
    /// Enable session memory
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileRestorationPriority {
    /// Restore most recently read files first
    #[default]
    MostRecent,
    /// Restore most frequently accessed files first
    MostAccessed,
}

impl Default for SessionMemoryConfig {
    fn default() -> Self {
        Self {
            budget_tokens: 50000,
            restoration_priority: FileRestorationPriority::MostRecent,
            enabled: true,
        }
    }
}

/// Stream stall detection configuration
#[derive(Debug, Clone)]
pub struct StallDetectionConfig {
    /// Timeout before considering stream stalled (default: 30s)
    pub stall_timeout: Duration,
    /// Recovery strategy when stall detected
    pub recovery: StallRecovery,
    /// Enable stall detection
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StallRecovery {
    /// Retry the request
    #[default]
    Retry,
    /// Abort the request
    Abort,
    /// Switch to fallback model
    Fallback,
}

impl Default for StallDetectionConfig {
    fn default() -> Self {
        Self {
            stall_timeout: Duration::from_secs(30),
            recovery: StallRecovery::Retry,
            enabled: true,
        }
    }
}

/// Prompt caching configuration for long system prompts
#[derive(Debug, Clone)]
pub struct PromptCachingConfig {
    /// Enable prompt caching
    pub enabled: bool,
    /// Cache breakpoints for long system prompts
    pub cache_breakpoints: Vec<CacheBreakpoint>,
}

#[derive(Debug, Clone)]
pub struct CacheBreakpoint {
    /// Position in system prompt (by content block index)
    pub position: i32,
    /// Cache type (ephemeral for session-scoped)
    pub cache_type: CacheType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CacheType {
    /// Ephemeral cache (session-scoped, auto-expires)
    #[default]
    Ephemeral,
}

impl Default for PromptCachingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cache_breakpoints: vec![],
        }
    }
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self {
            max_turns: None,
            max_tokens: None,
            auto_compact_threshold: 0.8,
            permission_mode: PermissionMode::Default,
            enable_streaming_tools: true,
            enable_micro_compaction: true,
            fallback_model: None,
            agent_id: None,
            parent_agent_id: None,
            record_sidechain: false,
            session_memory: SessionMemoryConfig::default(),
            stall_detection: StallDetectionConfig::default(),
            prompt_caching: PromptCachingConfig::default(),
        }
    }
}
```

### LoopEvent (Complete)

Based on Claude Code v2.1.7 implementation, the event types include additional streaming and error events:

```rust
pub enum LoopEvent {
    // Stream lifecycle
    StreamRequestStart,
    StreamRequestEnd { usage: TokenUsage },

    // Turn lifecycle
    TurnStarted { turn_id: String, turn_number: i32 },
    TurnCompleted { turn_id: String, usage: TokenUsage },

    // Content streaming
    TextDelta { turn_id: String, delta: String },
    ThinkingDelta { turn_id: String, delta: String },
    ToolCallDelta { call_id: String, delta: String },

    // Raw SSE event passthrough (for debugging/logging)
    StreamEvent { event: RawStreamEvent },

    // Tool execution
    ToolUseQueued { call_id: String, name: String, input: Value },
    ToolUseStarted { call_id: String, name: String },
    ToolProgress { call_id: String, progress: ToolProgress },
    ToolUseCompleted { call_id: String, output: ToolResultContent, is_error: bool },
    ToolExecutionAborted { reason: AbortReason },

    // Permission
    ApprovalRequired { request: ApprovalRequest },
    ApprovalResponse { request_id: String, approved: bool },

    // Agent events
    SubagentSpawned { agent_id: String, agent_type: String, description: String },
    SubagentProgress { agent_id: String, progress: AgentProgress },
    SubagentCompleted { agent_id: String, result: String },
    SubagentBackgrounded { agent_id: String, output_file: PathBuf },

    // Background tasks
    BackgroundTaskStarted { task_id: String, task_type: TaskType },
    BackgroundTaskProgress { task_id: String, progress: TaskProgress },
    BackgroundTaskCompleted { task_id: String, result: String },

    // Compaction
    CompactionStarted,
    CompactionCompleted { removed_messages: i32, summary_tokens: i32 },
    MicroCompactionApplied { removed_results: i32 },
    SessionMemoryCompactApplied { saved_tokens: i32, summary_tokens: i32 },

    // Model fallback
    ModelFallbackStarted { from: String, to: String, reason: String },
    ModelFallbackCompleted,
    /// Tombstone event when orphaned messages are invalidated during fallback
    Tombstone { message: ConversationMessage },

    // Retry events
    Retry { attempt: i32, max_attempts: i32, delay_ms: i32 },

    // API error events
    ApiError { error: ApiError, retry_info: Option<RetryInfo> },

    // MCP events
    McpToolCallBegin { server: String, tool: String, call_id: String },
    McpToolCallEnd { server: String, tool: String, call_id: String, is_error: bool },
    McpStartupUpdate { server: String, status: McpStartupStatus },
    McpStartupComplete { servers: Vec<McpServerInfo>, failed: Vec<(String, String)> },

    // Plan mode
    PlanModeEntered { plan_file: PathBuf },
    PlanModeExited { approved: bool },

    // Hook events
    HookExecuted { hook_type: HookEventType, hook_name: String },

    // Stream stall detection
    StreamStallDetected { turn_id: String, timeout: Duration },

    // Prompt caching
    PromptCacheHit { cached_tokens: i32 },
    PromptCacheMiss,

    // Errors & control
    Error { error: LoopError },
    Interrupted,
    MaxTurnsReached,
}

/// Raw stream event from SSE
#[derive(Debug, Clone)]
pub struct RawStreamEvent {
    pub event_type: String,
    pub data: Value,
}

/// Retry information for API errors
#[derive(Debug, Clone)]
pub struct RetryInfo {
    pub attempt: i32,
    pub max_attempts: i32,
    pub delay_ms: i32,
    pub retriable: bool,
}

/// Abort reasons for tool execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbortReason {
    /// Model switched to non-streaming fallback
    StreamingFallback,
    /// A sibling tool call failed
    SiblingError,
    /// User interrupted
    UserInterrupted,
}
```

### AgentLoop

```rust
pub struct AgentLoop {
    /// LLM model for generation
    model: Arc<dyn Model>,

    /// Available tools
    tools: ToolRegistry,

    /// Conversation context
    context: ConversationContext,

    /// Loop configuration
    config: LoopConfig,

    /// Streaming tool executor
    tool_executor: StreamingToolExecutor,

    /// Hook registry
    hooks: Arc<HookRegistry>,

    /// Event sender for streaming updates
    event_tx: mpsc::Sender<LoopEvent>,

    /// Cancellation token
    cancel: CancellationToken,

    /// Current turn number
    turn_number: i32,
}
```

## StreamingToolExecutor

The key innovation: tools execute DURING API streaming, not after.

### Feature Flag and Configuration

In Claude Code v2.1.7, streaming tool execution is controlled by:

| Configuration | Value | Notes |
|---------------|-------|-------|
| Feature flag | `tengu_streaming_tool_execution2` | Must be enabled |
| Max concurrency | 10 | Env: `CLAUDE_CODE_MAX_TOOL_USE_CONCURRENCY` |
| Stall threshold | 30s | `STALL_THRESHOLD_MS = 30_000` |

### Tool States

Tools progress through these states:

```
queued → executing → completed → yielded
```

### Core Structure

```rust
pub struct StreamingToolExecutor {
    /// Tool queue indexed by call_id
    tool_queue: HashMap<String, QueuedTool>,

    /// Pending progress events (for streaming tools)
    pending_progress: Vec<ToolProgress>,

    /// Async queue for collecting results
    result_queue: AsyncQueue<ToolExecutionResult>,

    /// Tool registry reference
    registry: Arc<ToolRegistry>,

    /// Execution context
    context: ToolContext,

    /// Maximum concurrent executions (default: 10)
    max_concurrency: i32,

    /// Abort controller for cancellation
    abort_controller: Option<AbortController>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolStatus {
    Queued,
    Executing,
    Completed,
    Yielded,
}

struct QueuedTool {
    call_id: String,
    name: String,
    input: Value,
    status: ToolStatus,
    is_concurrency_safe: bool,
    queued_at: Instant,
    /// Context modifier to apply (for sequential execution)
    context_modifier: Option<ContextModifier>,
}
```

### Abort Handling

When tools are aborted, synthetic error messages are generated based on the abort reason:

| Abort Reason | Synthetic Error Message |
|--------------|------------------------|
| `streaming_fallback` | `"Tool execution was aborted: model switched to non-streaming fallback"` |
| `sibling_error` | `"Tool execution was aborted: a sibling tool call failed"` |
| `user_interrupted` | `"Tool execution was aborted: user interrupted"` |

```rust
impl StreamingToolExecutor {
    /// Abort all executing tools with given reason
    pub fn abort_all(&mut self, reason: AbortReason) {
        let error_message = match reason {
            AbortReason::StreamingFallback =>
                "Tool execution was aborted: model switched to non-streaming fallback",
            AbortReason::SiblingError =>
                "Tool execution was aborted: a sibling tool call failed",
            AbortReason::UserInterrupted =>
                "Tool execution was aborted: user interrupted",
        };

        // Signal abort to all executing tools
        if let Some(controller) = &self.abort_controller {
            controller.abort();
        }

        // Generate synthetic error results for executing tools
        for (call_id, tool) in &self.tool_queue {
            if tool.status == ToolStatus::Executing {
                self.result_queue.push(ToolExecutionResult {
                    tool_use_id: call_id.clone(),
                    content: ToolResultContent::text(error_message),
                    is_error: true,
                });
            }
        }
    }
}
```

### Context Modifier Application

Context modifiers are applied differently based on execution mode:

```rust
impl StreamingToolExecutor {
    /// Apply context modifier for tool result
    fn apply_context_modifier(&mut self, tool: &QueuedTool, result: &ToolExecutionResult) {
        if let Some(modifier) = &tool.context_modifier {
            if tool.is_concurrency_safe {
                // Parallel execution: batch modifiers, apply after all complete
                self.pending_modifiers.push(modifier.clone());
            } else {
                // Sequential execution: apply immediately
                modifier.apply(&mut self.context);
            }
        }
    }
}
```

### Implementation

```rust
impl StreamingToolExecutor {
    /// Add tool from streaming response (called as tool_use blocks arrive)
    pub fn add_tool(&mut self, tool_use: ToolUseBlock, assistant_msg: &Message) {
        let tool = self.registry.get(&tool_use.name);
        let is_safe = tool
            .map(|t| t.is_concurrency_safe(&tool_use.input))
            .unwrap_or(true); // Unknown tools complete immediately

        self.tool_queue.insert(tool_use.id.clone(), QueuedTool {
            call_id: tool_use.id,
            name: tool_use.name,
            input: tool_use.input,
            status: ToolStatus::Queued,
            is_concurrency_safe: is_safe,
            queued_at: Instant::now(),
            context_modifier: None,
        });

        // Try to execute immediately if possible
        self.try_execute_next();
    }

    /// Check if we can execute the next tool
    pub fn can_execute_tool(&self, is_safe: bool) -> bool {
        // Count currently executing tools
        let executing_count = self.tool_queue.values()
            .filter(|t| t.status == ToolStatus::Executing)
            .count() as i32;

        if executing_count >= self.max_concurrency {
            return false; // At max concurrency
        }

        // Check for unsafe tools blocking
        let has_unsafe_executing = self.tool_queue.values()
            .any(|t| t.status == ToolStatus::Executing && !t.is_concurrency_safe);

        if has_unsafe_executing {
            return false; // Unsafe tool blocks all
        }
        if !is_safe {
            // Unsafe tool can only run when no other tools executing
            return executing_count == 0;
        }
        true // Safe tools can run in parallel
    }

    /// Get completed results (non-blocking generator)
    pub fn get_completed_results(&mut self) -> impl Iterator<Item = ToolExecutionResult> + '_ {
        self.tool_queue.values_mut()
            .filter(|t| t.status == ToolStatus::Completed)
            .map(|t| {
                t.status = ToolStatus::Yielded;
                self.result_queue.pop()
            })
            .flatten()
    }

    /// Get remaining results (blocking async generator)
    pub async fn get_remaining_results(&mut self) -> impl Stream<Item = ToolExecutionResult> + '_ {
        async_stream::stream! {
            // Wait for all queued and executing tools
            while self.has_pending_tools() {
                if let Some(result) = self.result_queue.pop_async().await {
                    if let Some(tool) = self.tool_queue.get_mut(&result.tool_use_id) {
                        tool.status = ToolStatus::Yielded;
                    }
                    yield result;
                }
            }
        }
    }

    fn has_pending_tools(&self) -> bool {
        self.tool_queue.values().any(|t|
            t.status == ToolStatus::Queued || t.status == ToolStatus::Executing
        )
    }

    fn try_execute_next(&mut self) {
        let queued_ids: Vec<_> = self.tool_queue.iter()
            .filter(|(_, t)| t.status == ToolStatus::Queued)
            .map(|(id, t)| (id.clone(), t.is_concurrency_safe))
            .collect();

        for (call_id, is_safe) in queued_ids {
            if !self.can_execute_tool(is_safe) {
                break;
            }

            if let Some(tool) = self.tool_queue.get_mut(&call_id) {
                tool.status = ToolStatus::Executing;
                self.spawn_execution(tool.clone());
            }
        }
    }

    fn spawn_execution(&self, queued: QueuedTool) {
        let registry = self.registry.clone();
        let context = self.context.clone();
        let result_queue = self.result_queue.clone();
        let abort_signal = self.abort_controller.as_ref().map(|c| c.signal());

        tokio::spawn(async move {
            let tool = registry.get(&queued.name);
            let result = match tool {
                Some(t) => {
                    execute_single_tool(
                        t.as_ref(),
                        queued.input,
                        &context,
                        &queued.call_id,
                        abort_signal,
                    ).await
                }
                None => ToolExecutionResult::error(
                    &queued.call_id,
                    format!("Tool not found: {}", queued.name),
                ),
            };
            result_queue.push(result);
        });
    }
}
```

## Main Loop Algorithm

The actual Claude Code v2.1.7 implementation uses an 18-step algorithm with comprehensive error handling and telemetry.

### Full Algorithm (18 Steps)

```
┌────────────────────────────────────────────────────────────────────────────┐
│                         coreMessageLoop (18 Steps)                          │
├────────────────────────────────────────────────────────────────────────────┤
│ STEP 1:  Signal stream_request_start event                                  │
│ STEP 2:  Setup query tracking (chainId, depth) for telemetry               │
│ STEP 3:  Normalize messages (slice from compact_boundary)                   │
│ STEP 4:  Micro-compaction (PRE-API: clear old tool results, no LLM)        │
│ STEP 5:  Auto-compaction (if threshold exceeded)                           │
│ STEP 6:  Initialize state (streaming tool executor, abort controller)      │
│ STEP 7:  Resolve model with permissions (check model access)               │
│ STEP 8:  Check blocking token limit (context_limit - 13K)                  │
│ STEP 9:  Main API streaming loop (with retry support, max 3 attempts)      │
│ STEP 10: Record API call info (telemetry, usage tracking)                  │
│ STEP 11: Check for tool calls (content_block_stop with tool_use)           │
│ STEP 12: Execute tool queue (parallel/sequential via executor)             │
│ STEP 13: Handle abort after tool execution (check sibling errors)          │
│ STEP 14: Check for hook stop (PreToolResult, PostToolExecution hooks)      │
│ STEP 15: Update auto-compact tracking (increment counters)                 │
│ STEP 16: Process queued commands and attachments                           │
│ STEP 17: Check max turns limit                                             │
│ STEP 18: RECURSIVE CALL for next turn (if stop_reason == tool_use)         │
└────────────────────────────────────────────────────────────────────────────┘
```

### Query Tracking Structure

```rust
/// Query tracking for telemetry and debugging
pub struct QueryTracking {
    /// Unique chain identifier for the conversation thread
    pub chain_id: String,
    /// Depth in the conversation (increments with each recursive call)
    pub depth: i32,
    /// Parent query ID (for subagent tracking)
    pub parent_query_id: Option<String>,
}
```

### Auto-Compact Tracking

```rust
/// Tracking structure for auto-compaction decisions
pub struct AutoCompactTracking {
    /// Whether compaction has occurred in this session
    pub compacted: bool,
    /// Turn ID when last compaction occurred
    pub turn_id: Option<String>,
    /// Number of turns since last compaction
    pub turn_counter: i32,
}
```

### Output Token Recovery

When the model hits `max_tokens` stop reason, Claude Code attempts recovery:

```rust
/// Maximum attempts for output token recovery
pub const MAX_OUTPUT_TOKEN_RECOVERY: i32 = 3;

impl AgentLoop {
    /// Handle max_tokens stop reason with recovery attempts
    async fn handle_max_tokens_recovery(
        &mut self,
        turn_id: &str,
        attempt: i32,
    ) -> Result<RecoveryAction, LoopError> {
        if attempt >= MAX_OUTPUT_TOKEN_RECOVERY {
            return Ok(RecoveryAction::Abort);
        }

        // Emit retry event
        self.emit(LoopEvent::Retry {
            attempt,
            max_attempts: MAX_OUTPUT_TOKEN_RECOVERY,
            delay_ms: 0,
        }).await;

        // Continue with next API call (model will resume)
        Ok(RecoveryAction::Continue)
    }
}
```

### Model Fallback Error

```rust
/// Error type for triggering model fallback
pub struct ModelFallbackError {
    /// Original error that triggered fallback
    pub original_error: Box<dyn std::error::Error + Send + Sync>,
    /// Suggested fallback model
    pub fallback_model: String,
    /// Reason for fallback
    pub reason: FallbackReason,
}

#[derive(Debug, Clone)]
pub enum FallbackReason {
    /// Model is overloaded
    Overloaded,
    /// Rate limited
    RateLimited,
    /// Context window exceeded
    ContextExceeded,
    /// Stream stalled
    StreamStalled,
}
```

### Implementation

```rust
impl AgentLoop {
    pub async fn run(
        &mut self,
        initial_message: ConversationMessage,
    ) -> Result<LoopResult, LoopError> {
        // Add initial message to context
        self.context.add_message(initial_message);

        // Initialize tracking
        let mut query_tracking = QueryTracking::new();
        let mut auto_compact_tracking = AutoCompactTracking::default();

        self.core_message_loop(&mut query_tracking, &mut auto_compact_tracking).await
    }

    async fn core_message_loop(
        &mut self,
        query_tracking: &mut QueryTracking,
        auto_compact_tracking: &mut AutoCompactTracking,
    ) -> Result<LoopResult, LoopError> {
        // STEP 1: Signal stream request start
        self.emit(LoopEvent::StreamRequestStart).await;

        // STEP 2: Setup query tracking
        query_tracking.depth += 1;
        let turn_id = uuid::Uuid::new_v4().to_string();

        // STEP 3: Normalize messages (slice from compact boundary)
        let messages = self.context.normalize_messages(auto_compact_tracking.turn_id.as_deref());

        // STEP 4: Micro-compaction (PRE-API)
        if self.config.enable_micro_compaction {
            let removed = self.micro_compact();
            if removed > 0 {
                self.emit(LoopEvent::MicroCompactionApplied { removed_results: removed }).await;
            }
        }

        // STEP 5: Auto-compaction (if threshold exceeded)
        if self.should_compact() {
            self.compact().await?;
            auto_compact_tracking.compacted = true;
            auto_compact_tracking.turn_id = Some(turn_id.clone());
        }

        // STEP 6: Initialize state
        self.turn_number += 1;
        self.tool_executor.reset();
        self.emit(LoopEvent::TurnStarted {
            turn_id: turn_id.clone(),
            turn_number: self.turn_number,
        }).await;

        // STEP 7: Resolve model with permissions
        let model = self.resolve_model_with_permissions().await?;

        // STEP 8: Check blocking token limit
        let usage = self.context.estimate_tokens();
        let blocking_limit = model.context_window() - 13000;
        if usage >= blocking_limit {
            return Err(LoopError::context_window_exceeded(usage, blocking_limit));
        }

        // STEP 9: Main API streaming loop (with retry)
        let mut output_recovery_attempts = 0;
        let response = loop {
            match self.stream_with_tools(&turn_id, &model).await {
                Ok(resp) => break resp,
                Err(e) if e.is_retriable() => {
                    output_recovery_attempts += 1;
                    if output_recovery_attempts >= MAX_OUTPUT_TOKEN_RECOVERY {
                        return Err(e);
                    }
                    self.emit(LoopEvent::Retry {
                        attempt: output_recovery_attempts,
                        max_attempts: MAX_OUTPUT_TOKEN_RECOVERY,
                        delay_ms: 0,
                    }).await;
                    continue;
                }
                Err(e) if e.is_model_fallback() => {
                    return self.handle_model_fallback(e, query_tracking, auto_compact_tracking).await;
                }
                Err(e) => return Err(e),
            }
        };

        // STEP 10: Record API call info
        self.record_api_call_info(&response, query_tracking).await;

        // STEP 11: Check for tool calls
        let has_tool_calls = response.content.iter()
            .any(|block| matches!(block, ContentBlock::ToolUse { .. }));

        // STEP 12: Execute tool queue
        if has_tool_calls {
            let results = self.execute_tool_queue().await?;

            // STEP 13: Handle abort after tool execution
            if self.tool_executor.has_aborted() {
                self.emit(LoopEvent::ToolExecutionAborted {
                    reason: self.tool_executor.abort_reason(),
                }).await;
            }

            // Add tool results to context
            let result_msg = ConversationMessage::tool_results(results);
            self.context.add_message(result_msg);
        }

        // Add assistant message to context
        let assistant_msg = ConversationMessage::from_response(response.clone());
        self.context.add_message(assistant_msg);

        // STEP 14: Check for hook stop
        if self.run_stop_hooks().await? {
            return Ok(LoopResult::hook_stopped());
        }

        // STEP 15: Update auto-compact tracking
        auto_compact_tracking.turn_counter += 1;

        // STEP 16: Process queued commands and attachments
        self.process_queued_commands().await?;

        // STEP 17: Check max turns limit
        if let Some(max) = self.config.max_turns {
            if self.turn_number >= max {
                self.emit(LoopEvent::MaxTurnsReached).await;
                return Ok(LoopResult::max_turns_reached());
            }
        }

        // Emit turn completed
        self.emit(LoopEvent::TurnCompleted {
            turn_id: turn_id.clone(),
            usage: response.usage.clone(),
        }).await;

        // Check stop reason
        match response.finish_reason {
            FinishReason::Stop => {
                self.emit(LoopEvent::StreamRequestEnd { usage: response.usage }).await;
                Ok(LoopResult::completed(response))
            }
            FinishReason::ToolUse => {
                // STEP 18: Recursive call for next turn
                self.core_message_loop(query_tracking, auto_compact_tracking).await
            }
            FinishReason::MaxTokens => {
                // Already handled in STEP 9 retry loop
                Ok(LoopResult::completed(response))
            }
            _ => Err(LoopError::unexpected_finish_reason(response.finish_reason)),
        }
    }

    /// Execute tool queue with parallel/sequential handling
    async fn execute_tool_queue(&mut self) -> Result<Vec<ToolExecutionResult>, LoopError> {
        let mut results = Vec::new();

        // Collect completed results during streaming (non-blocking)
        for result in self.tool_executor.get_completed_results() {
            self.emit(LoopEvent::ToolUseCompleted {
                call_id: result.tool_use_id.clone(),
                output: result.content.clone(),
                is_error: result.is_error,
            }).await;
            results.push(result);
        }

        // Wait for remaining results (blocking)
        let mut remaining = self.tool_executor.get_remaining_results().await;
        while let Some(result) = remaining.next().await {
            self.emit(LoopEvent::ToolUseCompleted {
                call_id: result.tool_use_id.clone(),
                output: result.content.clone(),
                is_error: result.is_error,
            }).await;
            results.push(result);
        }

        Ok(results)
    }
}
```

### Stream Processing with Tools

```rust
impl AgentLoop {
    /// Stream response while executing tools in parallel
    async fn stream_with_tools(
        &mut self,
        turn_id: &str,
        model: &dyn Model,
    ) -> Result<GenerateResponse, LoopError> {
        let request = self.build_request().await?;
        let mut stream = model.stream(request).await
            .map_err(|e| self.handle_llm_error(e))?;

        while let Some(event) = stream.next_event().await {
            match event? {
                StreamEvent::TextDelta { delta, .. } => {
                    self.emit(LoopEvent::TextDelta {
                        turn_id: turn_id.to_string(),
                        delta,
                    }).await;
                }
                StreamEvent::ThinkingDelta { delta, .. } => {
                    self.emit(LoopEvent::ThinkingDelta {
                        turn_id: turn_id.to_string(),
                        delta,
                    }).await;
                }
                StreamEvent::ToolCallStart { id, name, .. } => {
                    self.emit(LoopEvent::ToolUseQueued {
                        call_id: id.clone(),
                        name: name.clone(),
                        input: Value::Null, // Will be filled in
                    }).await;
                }
                StreamEvent::ToolCallComplete { id, name, arguments } => {
                    // Add to streaming executor - starts executing immediately if safe
                    let tool_use = ToolUseBlock { id, name, input: arguments };
                    self.tool_executor.add_tool(tool_use, &stream.current_message());

                    // Emit started event
                    self.emit(LoopEvent::ToolUseStarted {
                        call_id: tool_use.id.clone(),
                        name: tool_use.name.clone(),
                    }).await;
                }
                StreamEvent::Done => {
                    break;
                }
                _ => {}
            }

            // Check for completed tool results and emit events
            for result in self.tool_executor.get_completed_results() {
                self.emit(LoopEvent::ToolUseCompleted {
                    call_id: result.tool_use_id.clone(),
                    output: result.content.clone(),
                    is_error: result.is_error,
                }).await;
            }
        }

        stream.response()
    }
}
```

## Micro-Compaction (PRE-API Phase)

Micro-compaction runs **before every API call** and removes old tool results without LLM involvement. This is separate from the threshold-triggered auto-compact.

### Configuration Constants (v2.1.7)

```rust
pub const RECENT_TOOL_RESULTS_TO_KEEP: i32 = 3;  // Keep last 3 tool results
pub const MIN_SAVINGS_THRESHOLD: i32 = 20000;    // Min 20K tokens to compact
```

### Implementation

```rust
impl AgentLoop {
    /// Micro-compact: Remove old tool results (no LLM call)
    fn micro_compact(&mut self) -> i32 {
        let mut compacted_ids: HashSet<String> = HashSet::new();
        let tool_results = self.collect_tool_results();

        // Keep last RECENT_TOOL_RESULTS_TO_KEEP results
        let results_to_compact: Vec<_> = tool_results
            .iter()
            .rev()
            .skip(RECENT_TOOL_RESULTS_TO_KEEP as usize)
            .collect();

        for result in results_to_compact {
            if self.is_compactable_tool(&result.tool_name) {
                // Persist to disk if large (for potential retrieval)
                if result.token_count > 1000 {
                    self.persist_tool_result(result);
                }

                // Clear the content
                self.clear_tool_result(&result.call_id);
                compacted_ids.insert(result.call_id.clone());
            }
        }

        // Verify savings meet threshold
        let savings = self.calculate_token_savings(&compacted_ids);
        if savings < MIN_SAVINGS_THRESHOLD {
            self.revert_compaction(&compacted_ids);
            return 0;
        }

        compacted_ids.len() as i32
    }

    fn is_compactable_tool(&self, name: &str) -> bool {
        matches!(name, "Read" | "Bash" | "Grep" | "Glob" |
                       "WebSearch" | "WebFetch" | "Edit" | "Write")
    }
}
```

### Integration in Loop

Micro-compact runs at the start of each turn, before the threshold check:

```rust
// In coreMessageLoop, step 3-4:
// 3. Micro-compaction (remove low-value tool results)
if self.config.enable_micro_compaction {
    let removed = self.micro_compact();
    if removed > 0 {
        self.emit(LoopEvent::MicroCompactionApplied { removed_results: removed }).await;
    }
}

// 4. Auto-compaction (only if threshold exceeded AFTER micro-compact)
if self.should_compact() {
    self.compact().await?;
}
```

## Context Compaction

Claude Code v2.1.7 uses a **three-tier compaction system**. For comprehensive documentation, see [features.md - Context Compaction](./features.md#context-compaction).

### Compaction Flow Summary

```
┌─────────────────────────────────────────────────────────────┐
│  1. Micro-Compact (PRE-API, every turn)                     │
│     - No LLM call, clears old tool results                  │
│     - Keep last 3 results, min 20K savings                  │
├─────────────────────────────────────────────────────────────┤
│  2. Auto-Compact (when context_limit - 13K exceeded)        │
│     ├─ Tier 1: Session Memory Compact (zero API cost)       │
│     │  - Uses cached summary.md from background agent       │
│     │  - Keeps messages after lastSummarizedId              │
│     └─ Tier 2: Full Compact (LLM-based fallback)            │
│        - Streaming summarization                            │
│        - Context restoration (5 files, 50K budget)          │
├─────────────────────────────────────────────────────────────┤
│  Background: Session Memory Extraction Agent                 │
│  - Trigger: 5K tokens + 10 tool calls                       │
│  - Forked agent with Edit-only permission                   │
│  - Updates ~/.claude/<session>/session-memory/summary.md    │
└─────────────────────────────────────────────────────────────┘
```

### Implementation

```rust
impl AgentLoop {
    fn should_compact(&self) -> bool {
        let usage = self.context.estimate_tokens();
        let threshold = self.model.context_window() - 13000;  // v2.1.7 offset
        usage >= threshold
    }

    async fn compact(&mut self) -> Result<(), LoopError> {
        // 1. Run PreCompact hook
        self.hooks.execute(HookEventType::PreCompact, &self.context).await?;

        // 2. Emit start event
        self.emit(LoopEvent::CompactionStarted).await;

        // 3. Try Session Memory Compact first (Tier 1 - zero API cost)
        if self.try_session_memory_compact().await? {
            return Ok(());  // Success with cached summary
        }

        // 4. Fall through to Full Compact (Tier 2 - LLM-based)
        let summary = self.summarize_context().await?;
        let removed = self.context.compact(summary);

        // 5. Restore context (files, todos, plans, tasks)
        self.restore_context_after_compact().await?;

        // 6. Emit completion event
        self.emit(LoopEvent::CompactionCompleted {
            removed_messages: removed,
            summary_tokens: self.context.estimate_tokens(),
        }).await;

        Ok(())
    }

    /// Tier 1: Session Memory Compact (uses cached summary, zero API cost)
    async fn try_session_memory_compact(&mut self) -> Result<bool, LoopError> {
        let memory_dir = self.get_session_memory_path();
        let summary_path = memory_dir.join("summary.md");
        let metadata_path = memory_dir.join("metadata.json");

        // Load cached summary and metadata
        let summary = tokio::fs::read_to_string(&summary_path).await.ok();
        let metadata: Option<SessionMemoryMetadata> = tokio::fs::read_to_string(&metadata_path)
            .await
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());

        match (summary, metadata) {
            (Some(summary), Some(metadata)) => {
                // Calculate savings
                let msgs_to_remove = self.context.messages_before_id(&metadata.last_summarized_id);
                let tokens_to_save: i32 = msgs_to_remove.iter().map(|m| m.estimate_tokens()).sum();
                let summary_tokens = estimate_tokens(&summary);

                if tokens_to_save - summary_tokens >= 10000 {  // Min 10K savings
                    // Apply session memory compact
                    self.context.replace_messages_before_id(
                        &metadata.last_summarized_id,
                        ConversationMessage::summary(summary),
                    );
                    self.emit(LoopEvent::SessionMemoryCompactApplied {
                        saved_tokens: tokens_to_save - summary_tokens,
                        summary_tokens,
                    }).await;
                    return Ok(true);
                }
            }
            _ => {}
        }

        Ok(false)  // Fall through to Tier 2
    }
}
```

### Context Restoration (Tier 2 Only)

After Full Compact, restore critical context within budget limits:

```rust
/// Context restoration configuration (v2.1.7 values)
pub struct ContextRestorationConfig {
    pub max_files: i32,              // 5
    pub total_budget_tokens: i32,    // 50,000
    pub per_file_limit_tokens: i32,  // 5,000
}

impl AgentLoop {
    async fn restore_context_after_compact(&mut self) -> Result<(), LoopError> {
        let config = &self.config.context_restoration;

        // Get files from cache (respecting limits)
        let files = self.file_cache.get_files_for_restoration(
            config.max_files,
            config.total_budget_tokens,
        );

        // Also restore: todos, current plan, active skills, task summaries
        let restoration = ContextRestoration {
            files,
            todos: self.get_active_todos().await.unwrap_or_default(),
            plan: self.get_current_plan().await,
            skills: self.get_active_skills(),
            tasks: self.get_task_summaries(),
        };

        self.context.add_restoration_attachment(restoration);
        Ok(())
    }
}
```

### Related Documentation

- **Full details:** [features.md - Context Compaction](./features.md#context-compaction)
- **Background agent:** [features.md - Background Extraction Agent](./features.md#background-extraction-agent-session-memory)
- **Session Memory:** [features.md - Session Memory](./features.md#session-memory)

## Stream Stall Detection

Detect and recover from stream stalls during API streaming.

```rust
impl AgentLoop {
    /// Stream response with stall detection
    async fn stream_with_stall_detection(
        &mut self,
        turn_id: &str,
        request: ChatRequest,
    ) -> Result<GenerateResponse, LoopError> {
        let config = &self.config.stall_detection;
        let timeout = config.stall_timeout;

        let mut stream = self.model.stream(request.clone()).await
            .map_err(|e| self.handle_llm_error(e))?;

        let mut last_event = Instant::now();

        loop {
            let event = tokio::select! {
                event = stream.next_event() => event,
                _ = tokio::time::sleep_until((last_event + timeout).into()) => {
                    // Stall detected
                    return self.handle_stream_stall(turn_id, request).await;
                }
            };

            match event? {
                Some(e) => {
                    last_event = Instant::now();
                    self.process_stream_event(turn_id, e).await?;
                }
                None => break,
            }
        }

        stream.response()
    }

    /// Handle stream stall based on configured recovery strategy
    async fn handle_stream_stall(
        &mut self,
        turn_id: &str,
        request: ChatRequest,
    ) -> Result<GenerateResponse, LoopError> {
        self.emit(LoopEvent::StreamStallDetected {
            turn_id: turn_id.to_string(),
            timeout: self.config.stall_detection.stall_timeout,
        }).await;

        match self.config.stall_detection.recovery {
            StallRecovery::Retry => {
                // Retry the request
                self.stream_with_stall_detection(turn_id, request).await
            }
            StallRecovery::Abort => {
                Err(LoopError::stream_stalled())
            }
            StallRecovery::Fallback => {
                // Switch to fallback model and retry
                if let Some(fallback) = &self.config.fallback_model {
                    self.emit(LoopEvent::ModelFallbackStarted {
                        from: self.model.name().to_string(),
                        to: fallback.clone(),
                        reason: "Stream stalled".to_string(),
                    }).await;
                    // Retry with fallback model
                    Err(LoopError::retry_with_fallback(fallback.clone()))
                } else {
                    Err(LoopError::stream_stalled())
                }
            }
        }
    }
}
```

## Model Fallback

Handle model overload by switching to fallback.

```rust
impl AgentLoop {
    fn handle_llm_error(&mut self, error: LlmError) -> LoopError {
        match &error {
            LlmError::Overloaded { .. } if self.config.fallback_model.is_some() => {
                // Tombstone orphaned messages (tool_use without tool_result)
                self.tombstone_orphaned_messages();

                // Switch to fallback
                let fallback = self.config.fallback_model.clone().unwrap();
                self.emit_sync(LoopEvent::ModelFallbackStarted {
                    from: self.model.name().to_string(),
                    to: fallback.clone(),
                    reason: "Model overloaded".to_string(),
                });

                // Create new model (actual switch handled by caller)
                LoopError::retry_with_fallback(fallback)
            }
            _ => LoopError::llm_error(error),
        }
    }

    fn tombstone_orphaned_messages(&mut self) {
        // Find tool_use blocks without corresponding tool_result
        // Mark them as orphaned to prevent API errors
    }
}
```

## Prompt Caching

Reduce API costs by caching long system prompts.

```rust
impl AgentLoop {
    /// Build request with prompt caching
    async fn build_request(&self) -> Result<ChatRequest, LoopError> {
        let mut messages = self.context.messages().clone();
        let tools = self.tools.definitions();

        // Apply cache breakpoints to system prompt
        if self.config.prompt_caching.enabled {
            self.apply_cache_breakpoints(&mut messages);
        }

        Ok(ChatRequest {
            model: self.model.name().to_string(),
            messages,
            tools: Some(tools),
            max_tokens: self.config.max_tokens,
            ..Default::default()
        })
    }

    /// Apply cache breakpoints to system prompt content
    fn apply_cache_breakpoints(&self, messages: &mut Vec<ConversationMessage>) {
        for msg in messages.iter_mut() {
            if msg.role != Role::System {
                continue;
            }

            for breakpoint in &self.config.prompt_caching.cache_breakpoints {
                if let Some(content) = msg.content.get_mut(breakpoint.position as usize) {
                    content.cache_control = Some(CacheControl {
                        cache_type: breakpoint.cache_type,
                    });
                }
            }
        }
    }
}

/// Cache control for prompt caching
#[derive(Debug, Clone, Serialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub cache_type: CacheType,
}

// Usage: Add cache breakpoint after expensive system prompt sections
// Example: After CLAUDE.md content, after tool definitions
```

### Prompt Caching Strategy

```
System Prompt Structure:
┌────────────────────────────────────────┐
│ Base system prompt                     │
├────────────────────────────────────────┤
│ CLAUDE.md / project context            │ ← Cache breakpoint #1
├────────────────────────────────────────┤
│ Tool definitions                       │ ← Cache breakpoint #2
├────────────────────────────────────────┤
│ MCP tool descriptions                  │ ← Cache breakpoint #3
├────────────────────────────────────────┤
│ Dynamic context (skills, reminders)    │
└────────────────────────────────────────┘
```

---

## System Reminder Injection

System reminders (attachments) are contextual information injected into the conversation at strategic points. They provide the model with context about session state, file changes, and other dynamic information.

### Injection Timing

```
User Input
    │
    ▼
┌───────────────────────────────────────────────────────────────┐
│                    coreMessageLoop                             │
│                                                                │
│  1. Parse @mentions, extract user prompt text                  │
│                                                                │
│  2. generate_all_attachments() ─────────────────────────────┐  │
│     │                                                        │  │
│     ├─→ User Prompt Attachments (if user input exists)      │  │
│     │   ├─→ @mentioned files                                │  │
│     │   ├─→ MCP resources                                   │  │
│     │   └─→ @agent mentions                                 │  │
│     │                                                        │  │
│     ├─→ Core Attachments (always checked)                   │  │
│     │   ├─→ changed_files                                   │  │
│     │   ├─→ nested_memory                                   │  │
│     │   ├─→ plan_mode / plan_mode_exit                      │  │
│     │   ├─→ delegate_mode / delegate_mode_exit              │  │
│     │   ├─→ todo_reminders                                  │  │
│     │   ├─→ collab_notification                             │  │
│     │   └─→ critical_system_reminder                        │  │
│     │                                                        │  │
│     └─→ Main Agent Attachments (if main agent)             │  │
│         ├─→ ide_selection, ide_opened_file                  │  │
│         ├─→ diagnostics, lsp_diagnostics                    │  │
│         ├─→ task_status, task_progress                      │  │
│         ├─→ memory, token_usage, budget_usd                 │  │
│         └─→ verify_plan_reminder                            │  │
│                                                              │  │
│  3. (Timeout: 1 second max for all attachment generation)   │  │
│                                                              │  │
│  4. convert_attachments_to_system_messages()                │  │
│                                                              │  │
│  5. Insert into message array before API call               │  │
│                                                                │
└───────────────────────────────────────────────────────────────┘
        │
        ▼
   Claude API Call
```

### Attachment Generation Pipeline

```rust
impl AgentLoop {
    /// Generate and inject attachments before API call
    async fn prepare_attachments(
        &mut self,
        user_prompt: Option<&str>,
    ) -> Result<Vec<MetaMessage>> {
        // 1 second timeout to prevent blocking
        let timeout = Duration::from_secs(1);

        // Generate all attachments in parallel
        let attachments = tokio::time::timeout(
            timeout,
            generate_all_attachments(
                user_prompt,
                &self.context.tool_context(),
                &self.ide_context,
                &self.queued_commands,
                self.context.messages(),
            ),
        )
        .await
        .unwrap_or_default();

        // Convert attachments to system messages
        let system_messages: Vec<MetaMessage> = attachments
            .into_iter()
            .flat_map(convert_attachment_to_system_message)
            .collect();

        // Send telemetry (5% sampling)
        if rand::random::<f32>() < 0.05 {
            emit_telemetry("attachments_generated", &json!({
                "attachment_types": system_messages.iter().map(|m| m.attachment_type()).collect::<Vec<_>>(),
            }));
        }

        Ok(system_messages)
    }

    /// Inject attachments into the conversation
    fn inject_attachments(&mut self, attachments: Vec<MetaMessage>) {
        for attachment in attachments {
            // Insert as user message with isMeta: true
            self.context.add_meta_message(attachment);
        }
    }
}
```

### Attachment to API Message Conversion

Attachments are converted to API messages with `<system-reminder>` XML tags:

```rust
/// Convert attachment to system messages
pub fn convert_attachment_to_system_message(attachment: Attachment) -> Vec<MetaMessage> {
    match attachment {
        // Tool simulation: wrap in system-reminder with tool use/result
        Attachment::File { filename, content, truncated } => {
            wrap_in_system_reminder(vec![
                create_tool_use_message("Read", json!({ "file_path": filename })),
                create_tool_result_message("Read", &format_file_content(&content)),
            ])
        }

        // Direct meta block with system-reminder
        Attachment::PlanMode { reminder_type, is_sub_agent, plan_file_path, plan_exists } => {
            let content = match (is_sub_agent, reminder_type) {
                (true, _) => build_sub_agent_plan_reminder(&plan_file_path, plan_exists),
                (false, PlanReminderType::Sparse) => build_sparse_plan_reminder(&plan_file_path),
                (false, PlanReminderType::Full) => build_full_plan_reminder(&plan_file_path, plan_exists),
            };
            wrap_in_system_reminder(vec![create_meta_block(content)])
        }

        // Direct wrap for simple types
        Attachment::TaskStatus { task_id, task_type, status, description, delta_summary } => {
            let message = format_task_status(task_id, task_type, status, description, delta_summary);
            vec![create_meta_block(wrap_system_reminder_text(&message))]
        }

        // Silent types (handled elsewhere)
        Attachment::AlreadyReadFile { .. } => vec![],
        Attachment::DelegateMode { .. } => vec![],

        // ... other attachment types
    }
}
```

### XML Tag Format

System reminders use `<system-reminder>` XML tags:

```xml
<system-reminder>
[Reminder content - instructions, warnings, or metadata]
</system-reminder>
```

See [XML Format Specification](./xml-format.md) for complete tag documentation.

### Configuration Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `ATTACHMENT_TIMEOUT` | 1000ms | Max time for all attachment generation |
| `TURNS_BETWEEN_ATTACHMENTS` | 5 | Turns between plan mode attachments |
| `FULL_REMINDER_EVERY_N_ATTACHMENTS` | 5 | Full plan mode reminder interval |
| `PROGRESS_TURN_THRESHOLD` | 3 | Turns between task progress updates |

### Related Documentation

- [Attachments](./attachments.md) - Complete attachment type catalog
- [XML Format](./xml-format.md) - XML tag specifications
- [Plan Mode](./features.md#plan-mode) - Plan mode workflow and reminders

---

## Error Recovery

```rust
impl AgentLoop {
    async fn handle_error(&mut self, error: LoopError) -> Result<(), LoopError> {
        match &error {
            LoopError::LlmOverloaded { .. } => {
                // Try fallback model
                if let Some(fallback) = &self.config.fallback_model {
                    self.switch_model(fallback).await?;
                    return Ok(());
                }
            }
            LoopError::ContextWindowExceeded => {
                // Force compaction
                self.compact().await?;
                return Ok(());
            }
            _ => {}
        }

        Err(error)
    }
}
```

## Usage Example

```rust
use cocode_loop::{AgentLoop, LoopConfig, LoopEvent};
use cocode_tools::ToolRegistry;
use cocode_tools::register_all_tools;
use hyper_sdk::prelude::*;

async fn run_agent() -> Result<()> {
    // Setup provider and model
    let provider = AnthropicProvider::from_env()?;
    let model = provider.model("claude-sonnet-4-20250514")?;

    // Setup tools
    let mut registry = ToolRegistry::new();
    register_all_tools(&mut registry);

    // Create event channel
    let (event_tx, mut event_rx) = mpsc::channel(100);

    // Spawn event handler
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                LoopEvent::TextDelta { delta, .. } => print!("{delta}"),
                LoopEvent::ToolUseStarted { name, .. } => println!("\n[Tool: {name}]"),
                LoopEvent::SubagentSpawned { agent_type, description, .. } => {
                    println!("\n[Spawning {agent_type}: {description}]");
                }
                _ => {}
            }
        }
    });

    // Create and run loop
    let config = LoopConfig {
        enable_streaming_tools: true,
        enable_micro_compaction: true,
        ..Default::default()
    };
    let context = ConversationContext::new();
    let mut loop_driver = AgentLoop::new(model, registry, context, config, event_tx);

    let msg = ConversationMessage::user("Write a hello world in Rust");
    let result = loop_driver.run(msg).await?;

    println!("\nFinal: {}", result.final_text());
    Ok(())
}
```

## Stream Event Processing

Claude Code v2.1.7 processes Anthropic SSE (Server-Sent Events) with a content block aggregation state machine.

### SSE Event Types (Anthropic Protocol)

The actual SSE events from the Anthropic API differ from the simplified events used in documentation:

| Documentation Event | Actual SSE Event Type | Delta Type |
|---------------------|-----------------------|------------|
| `TextDelta` | `content_block_delta` | `text_delta` |
| `ThinkingDelta` | `content_block_delta` | `thinking_delta` |
| `ToolCallStart` | `content_block_start` | type: `tool_use` |
| `ToolCallComplete` | `content_block_stop` | - |
| (JSON accumulation) | `content_block_delta` | `input_json_delta` |
| (Signature) | `content_block_delta` | `signature_delta` |

### Content Block Aggregation State Machine

```
                    ┌─────────────────────────────────────────────┐
                    │           message_start                      │
                    │  Initialize aggregation state                │
                    └─────────────────────────────────────────────┘
                                        │
                                        ▼
┌───────────────────────────────────────────────────────────────────────────┐
│                        content_block_start                                 │
│  - Create new ContentBlock based on type (text, tool_use, thinking)       │
│  - Initialize accumulator for content                                      │
└───────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌───────────────────────────────────────────────────────────────────────────┐
│                        content_block_delta (loop)                          │
│  - text_delta: Append to text accumulator                                  │
│  - thinking_delta: Append to thinking accumulator                          │
│  - input_json_delta: Append to JSON string accumulator                     │
│  - signature_delta: Append to signature (for tool verification)            │
└───────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌───────────────────────────────────────────────────────────────────────────┐
│                        content_block_stop                                  │
│  - Parse accumulated JSON for tool_use blocks                              │
│  - YIELD ContentBlock (per-block, not per-message)                         │
│  - For tool_use: Trigger StreamingToolExecutor.add_tool()                  │
└───────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
                    ┌─────────────────────────────────────────────┐
                    │           message_stop                       │
                    │  Finalize response, return GenerateResponse  │
                    └─────────────────────────────────────────────┘
```

### Per-Block Yielding Strategy

Claude Code yields content blocks on `content_block_stop`, not `message_stop`:

```rust
/// Aggregator for SSE content blocks
pub struct ContentBlockAggregator {
    /// Current block index being aggregated
    current_index: i32,
    /// Block type (text, tool_use, thinking)
    block_type: ContentBlockType,
    /// Accumulated text content
    text_accumulator: String,
    /// Accumulated JSON for tool inputs
    json_accumulator: String,
    /// Accumulated signature (for tool verification)
    signature_accumulator: String,
}

impl ContentBlockAggregator {
    /// Process SSE event and potentially yield completed block
    pub fn process_event(&mut self, event: SseEvent) -> Option<ContentBlock> {
        match event {
            SseEvent::ContentBlockStart { index, content_block } => {
                self.current_index = index;
                self.block_type = content_block.block_type();
                self.reset_accumulators();
                None
            }
            SseEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, self.current_index);
                match delta {
                    Delta::TextDelta { text } => {
                        self.text_accumulator.push_str(&text);
                    }
                    Delta::ThinkingDelta { thinking } => {
                        self.text_accumulator.push_str(&thinking);
                    }
                    Delta::InputJsonDelta { partial_json } => {
                        self.json_accumulator.push_str(&partial_json);
                    }
                    Delta::SignatureDelta { signature } => {
                        self.signature_accumulator.push_str(&signature);
                    }
                }
                None // Don't yield yet
            }
            SseEvent::ContentBlockStop { index } => {
                assert_eq!(index, self.current_index);
                // YIELD on block stop, not message stop
                Some(self.finalize_block())
            }
            _ => None,
        }
    }

    fn finalize_block(&self) -> ContentBlock {
        match self.block_type {
            ContentBlockType::Text => ContentBlock::Text {
                text: self.text_accumulator.clone(),
            },
            ContentBlockType::ToolUse => ContentBlock::ToolUse {
                id: String::new(), // Set from content_block_start
                name: String::new(),
                input: serde_json::from_str(&self.json_accumulator)
                    .unwrap_or(Value::Null),
            },
            ContentBlockType::Thinking => ContentBlock::Thinking {
                thinking: self.text_accumulator.clone(),
                signature: self.signature_accumulator.clone(),
            },
        }
    }
}
```

### Stall Detection

Stream stall detection uses a 30-second threshold:

```rust
/// Stall detection constants
pub const STALL_THRESHOLD_MS: i32 = 30_000;  // 30 seconds

impl StreamProcessor {
    /// Process stream with stall detection
    async fn process_with_stall_detection(
        &mut self,
        stream: impl Stream<Item = Result<SseEvent>>,
    ) -> Result<GenerateResponse> {
        let mut last_event_time = Instant::now();

        pin_mut!(stream);
        loop {
            let timeout = Duration::from_millis(STALL_THRESHOLD_MS as u64);
            let event = tokio::select! {
                event = stream.next() => event,
                _ = tokio::time::sleep_until((last_event_time + timeout).into()) => {
                    return Err(StreamError::Stalled {
                        timeout_ms: STALL_THRESHOLD_MS,
                    });
                }
            };

            match event {
                Some(Ok(e)) => {
                    last_event_time = Instant::now();
                    if let Some(block) = self.aggregator.process_event(e) {
                        self.yield_block(block).await?;
                    }
                }
                Some(Err(e)) => return Err(e.into()),
                None => break,
            }
        }

        Ok(self.finalize_response())
    }
}
```

---

## Comprehensive Constants Reference

All configuration constants from Claude Code v2.1.7 implementation:

### Core Loop Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `MAX_OUTPUT_TOKEN_RECOVERY` | 3 | Max retry attempts for output token exhaustion |
| `DEFAULT_MIN_BLOCKING_OFFSET` | 13000 | Offset from context limit for blocking check (in `CompactConfig`) |
| `WARNING_OFFSET` | 20000 (`c97`) | Offset for context warning threshold |
| `ERROR_OFFSET` | 20000 (`p97`) | Offset for context error threshold |

### Streaming Tool Executor Constants

| Constant | Value | Environment Variable |
|----------|-------|---------------------|
| `MAX_TOOL_USE_CONCURRENCY` | 10 | `CLAUDE_CODE_MAX_TOOL_USE_CONCURRENCY` |
| `STALL_THRESHOLD_MS` | 30000 | - |

### Compaction Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `RECENT_TOOL_RESULTS_TO_KEEP` | 3 | Tool results kept after micro-compact |
| `MIN_SAVINGS_THRESHOLD` | 20000 | Min tokens to save for compact |
| `SESSION_MEMORY_MIN_SAVINGS` | 10000 | Min savings for session memory compact |
| `CONTEXT_RESTORATION_MAX_FILES` | 5 | Files restored after full compact |
| `CONTEXT_RESTORATION_BUDGET` | 50000 | Total token budget for restoration |
| `CONTEXT_RESTORATION_PER_FILE` | 5000 | Per-file token limit |

### Attachment Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `ATTACHMENT_TIMEOUT` | 1000 | Max ms for attachment generation |
| `TURNS_BETWEEN_ATTACHMENTS` | 5 | Turns between plan mode attachments |
| `FULL_REMINDER_EVERY_N_ATTACHMENTS` | 5 | Full reminder interval |
| `PROGRESS_TURN_THRESHOLD` | 3 | Turns between task progress |

### Session Memory Agent Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `TRIGGER_TOKEN_THRESHOLD` | 5000 | Min tokens before triggering |
| `TRIGGER_TOOL_CALL_THRESHOLD` | 10 | Min tool calls before triggering |

### Feature Flags

| Flag | Description |
|------|-------------|
| `tengu_streaming_tool_execution2` | Enable streaming tool execution |

---

## Key Patterns Summary

| Pattern | Description |
|---------|-------------|
| **Streaming Tool Execution** | Tools start executing as their blocks complete, not after full response |
| **Concurrency Safety** | Read-only tools run in parallel; write tools run sequentially |
| **Micro-compaction** | Remove low-value tool results without full summarization |
| **Auto-compaction** | Summarize older messages when context approaches limit |
| **Model Fallback** | Switch to fallback model on overload with orphan message handling |
| **Event Streaming** | All state changes emit events for UI integration |
| **Per-Block Yielding** | Content blocks yield on `content_block_stop`, enabling early tool execution |
| **Stall Detection** | 30s threshold for stream stall with configurable recovery |
