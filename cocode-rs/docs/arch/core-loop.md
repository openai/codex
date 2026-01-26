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

```
User Input
    │
    ▼
┌───────────────────────────────────────────────────────────────┐
│                    coreMessageLoop                             │
│                                                                │
│  1. Signal stream_request_start                               │
│  2. Message normalization (slice from last compact boundary)  │
│  3. Micro-compaction (remove low-value tool results)          │
│  4. Auto-compaction (summarize when approaching limit)        │
│                                                                │
│  ┌────────────────────────────────────────────────────────┐   │
│  │            API Streaming Loop (with fallback)           │   │
│  │                                                         │   │
│  │   Model.stream() ──► StreamEvent ──► emit to UI        │   │
│  │         │                                               │   │
│  │         ▼ (if overloaded)                              │   │
│  │   tombstone_orphaned_messages()                        │   │
│  │   switch_to_fallback_model()                           │   │
│  │   retry API call                                       │   │
│  └────────────────────────────────────────────────────────┘   │
│                         │                                      │
│                         ▼ (tool_use blocks)                   │
│  ┌────────────────────────────────────────────────────────┐   │
│  │          StreamingToolExecutor                          │   │
│  │                                                         │   │
│  │   Execute tools DURING API streaming                    │   │
│  │   - can_execute_tool() checks concurrency safety        │   │
│  │   - Parallel for safe tools                             │   │
│  │   - Sequential for unsafe tools                         │   │
│  │   - Results returned as they complete                   │   │
│  └────────────────────────────────────────────────────────┘   │
│                         │                                      │
│                         ▼                                      │
│  5. Add tool results to context                               │
│  6. Yield file_change_attachments, steering_attachments       │
│  7. Run hooks (Stop hooks can prevent continuation)           │
│  8. Check queued commands                                     │
│  9. Recursive call for next turn (if stop_reason == tool_use) │
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

    // Tool execution
    ToolUseQueued { call_id: String, name: String, input: Value },
    ToolUseStarted { call_id: String, name: String },
    ToolProgress { call_id: String, progress: ToolProgress },
    ToolUseCompleted { call_id: String, output: ToolResultContent, is_error: bool },

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

    // Model fallback
    ModelFallbackStarted { from: String, to: String, reason: String },
    ModelFallbackCompleted,

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

```rust
pub struct StreamingToolExecutor {
    /// Queued tools waiting to execute
    queued: VecDeque<QueuedTool>,

    /// Currently executing tools
    executing: HashMap<String, JoinHandle<ToolExecutionResult>>,

    /// Completed results
    completed: Vec<ToolExecutionResult>,

    /// Tool registry reference
    registry: Arc<ToolRegistry>,

    /// Execution context
    context: ToolContext,

    /// Whether a non-safe tool is currently executing
    unsafe_executing: bool,
}

struct QueuedTool {
    call_id: String,
    name: String,
    input: Value,
    is_concurrency_safe: bool,
    queued_at: Instant,
}

impl StreamingToolExecutor {
    /// Add tool from streaming response (called as tool_use blocks arrive)
    pub fn add_tool(&mut self, tool_use: ToolUseBlock, assistant_msg: &Message) {
        let tool = self.registry.get(&tool_use.name);
        let is_safe = tool
            .map(|t| t.is_concurrency_safe(&tool_use.input))
            .unwrap_or(true); // Unknown tools complete immediately

        self.queued.push_back(QueuedTool {
            call_id: tool_use.id,
            name: tool_use.name,
            input: tool_use.input,
            is_concurrency_safe: is_safe,
            queued_at: Instant::now(),
        });

        // Try to execute immediately if possible
        self.try_execute_next();
    }

    /// Check if we can execute the next tool
    pub fn can_execute_tool(&self, is_safe: bool) -> bool {
        if self.unsafe_executing {
            return false; // Unsafe tool blocks all
        }
        if !is_safe {
            // Unsafe tool can only run when no other tools executing
            return self.executing.is_empty();
        }
        true // Safe tools can run in parallel
    }

    /// Get completed results (non-blocking)
    pub fn get_completed_results(&mut self) -> Vec<ToolExecutionResult> {
        std::mem::take(&mut self.completed)
    }

    /// Drain remaining results (blocking, called after stream ends)
    pub async fn drain_remaining(&mut self) -> Vec<ToolExecutionResult> {
        // Wait for all executing tools
        let handles: Vec<_> = self.executing.drain().collect();
        for (call_id, handle) in handles {
            match handle.await {
                Ok(result) => self.completed.push(result),
                Err(e) => self.completed.push(ToolExecutionResult::error(&call_id, e)),
            }
        }
        std::mem::take(&mut self.completed)
    }

    fn try_execute_next(&mut self) {
        while let Some(tool) = self.queued.front() {
            if !self.can_execute_tool(tool.is_concurrency_safe) {
                break;
            }

            let tool = self.queued.pop_front().unwrap();
            if !tool.is_concurrency_safe {
                self.unsafe_executing = true;
            }

            // Spawn execution task
            let handle = self.spawn_execution(tool.clone());
            self.executing.insert(tool.call_id.clone(), handle);
        }
    }

    fn spawn_execution(&self, queued: QueuedTool) -> JoinHandle<ToolExecutionResult> {
        let registry = self.registry.clone();
        let context = self.context.clone();

        tokio::spawn(async move {
            let tool = registry.get(&queued.name);
            match tool {
                Some(t) => {
                    execute_single_tool(
                        t.as_ref(),
                        queued.input,
                        &context,
                        &queued.call_id,
                    ).await
                }
                None => ToolExecutionResult::error(
                    &queued.call_id,
                    format!("Tool not found: {}", queued.name),
                ),
            }
        })
    }
}
```

## Main Loop Algorithm

```rust
impl AgentLoop {
    pub async fn run(
        &mut self,
        initial_message: ConversationMessage,
    ) -> Result<LoopResult, LoopError> {
        // Add initial message to context
        self.context.add_message(initial_message);

        loop {
            // 1. Check turn limit
            if let Some(max) = self.config.max_turns {
                if self.turn_number >= max {
                    self.emit(LoopEvent::MaxTurnsReached).await;
                    return Ok(LoopResult::max_turns_reached());
                }
            }

            // 2. Check cancellation
            if self.cancel.is_cancelled() {
                self.emit(LoopEvent::Interrupted).await;
                return Ok(LoopResult::interrupted());
            }

            // 3. Emit stream request start
            self.emit(LoopEvent::StreamRequestStart).await;

            // 4. Check for micro-compaction
            if self.config.enable_micro_compaction {
                let removed = self.micro_compact();
                if removed > 0 {
                    self.emit(LoopEvent::MicroCompactionApplied { removed_results: removed }).await;
                }
            }

            // 5. Check for auto-compaction
            if self.should_compact() {
                self.compact().await?;
            }

            // 6. Generate turn ID
            let turn_id = uuid::Uuid::new_v4().to_string();
            self.turn_number += 1;
            self.emit(LoopEvent::TurnStarted {
                turn_id: turn_id.clone(),
                turn_number: self.turn_number
            }).await;

            // 7. Build request
            let request = self.build_request().await?;

            // 8. Stream response with tool executor
            let response = self.stream_with_tools(&turn_id, request).await?;

            // 9. Emit stream request end
            self.emit(LoopEvent::StreamRequestEnd { usage: response.usage.clone() }).await;

            // 10. Add assistant message to context
            let assistant_msg = ConversationMessage::from_response(response.clone());
            self.context.add_message(assistant_msg);

            // 11. Check stop reason
            match response.finish_reason {
                FinishReason::Stop => {
                    self.emit(LoopEvent::TurnCompleted {
                        turn_id,
                        usage: response.usage
                    }).await;
                    return Ok(LoopResult::completed(response));
                }
                FinishReason::ToolUse => {
                    // 12. Get tool results from executor
                    let results = self.tool_executor.drain_remaining().await;

                    // 13. Add tool results to context
                    let result_msg = ConversationMessage::tool_results(results);
                    self.context.add_message(result_msg);

                    // 14. Run hooks
                    self.run_stop_hooks().await?;

                    self.emit(LoopEvent::TurnCompleted {
                        turn_id,
                        usage: response.usage
                    }).await;

                    // Continue loop for next turn
                }
                FinishReason::MaxTokens => {
                    self.handle_max_tokens(&turn_id).await?;
                }
                _ => {
                    return Err(LoopError::unexpected_finish_reason(response.finish_reason));
                }
            }
        }
    }

    /// Stream response while executing tools in parallel
    async fn stream_with_tools(
        &mut self,
        turn_id: &str,
        request: ChatRequest,
    ) -> Result<GenerateResponse, LoopError> {
        let mut stream = self.model.stream(request).await
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

## Micro-Compaction

Remove low-value tool results to save context space.

```rust
impl AgentLoop {
    /// Remove tool results that provide little value
    fn micro_compact(&mut self) -> i32 {
        let mut removed = 0;
        for msg in self.context.messages_mut() {
            if let Some(tool_results) = msg.tool_results_mut() {
                for result in tool_results {
                    if self.is_low_value_result(result) {
                        result.content = ToolResultContent::Text("[Result compacted]".to_string());
                        removed += 1;
                    }
                }
            }
        }
        removed
    }

    fn is_low_value_result(&self, result: &ToolResult) -> bool {
        // Large Glob/Grep results that have been processed
        // Empty Read results
        // Redundant file contents
        // ... implementation specific logic
        false
    }
}
```

## Context Compaction

When context exceeds threshold, summarize older messages.

```rust
impl AgentLoop {
    fn should_compact(&self) -> bool {
        let usage = self.context.estimate_tokens();
        let max = self.model.context_window();
        (usage as f32 / max as f32) > self.config.auto_compact_threshold
    }

    async fn compact(&mut self) -> Result<(), LoopError> {
        // 1. Run PreCompact hook
        self.hooks.execute(HookEventType::PreCompact, &self.context).await?;

        // 2. Emit start event
        self.emit(LoopEvent::CompactionStarted).await;

        // 3. Summarize older messages
        let summary = self.summarize_context().await?;
        let removed = self.context.compact(summary);

        // 4. Restore session memory (recently read files)
        if self.config.session_memory.enabled {
            self.restore_session_memory().await?;
        }

        // 5. Emit completion event
        self.emit(LoopEvent::CompactionCompleted {
            removed_messages: removed,
            summary_tokens: self.context.estimate_tokens(),
        }).await;

        Ok(())
    }
}
```

### Session Memory Restoration

After compaction, restore recently read files to preserve context:

```rust
impl AgentLoop {
    /// Restore recently read files after compaction
    async fn restore_session_memory(&mut self) -> Result<(), LoopError> {
        let config = &self.config.session_memory;
        let budget = config.budget_tokens;

        // Get cached files from read file state
        let read_state = self.context.read_file_state.read().await;
        let mut files: Vec<_> = read_state.files.iter().collect();

        // Sort by restoration priority
        match config.restoration_priority {
            FileRestorationPriority::MostRecent => {
                files.sort_by(|a, b| b.1.timestamp.cmp(&a.1.timestamp));
            }
            FileRestorationPriority::MostAccessed => {
                files.sort_by(|a, b| b.1.access_count.cmp(&a.1.access_count));
            }
        }

        // Build session memory content within budget
        let mut used_tokens = 0;
        let mut memory_content = Vec::new();

        for (path, info) in files {
            let tokens = estimate_tokens(&info.content);
            if used_tokens + tokens > budget {
                break;
            }

            memory_content.push(SessionMemoryFile {
                path: path.clone(),
                content: info.content.clone(),
                last_read: info.timestamp,
            });
            used_tokens += tokens;
        }

        // Add session memory as system attachment
        if !memory_content.is_empty() {
            self.context.add_session_memory(memory_content);
        }

        Ok(())
    }
}

/// Session memory state for file restoration
#[derive(Debug, Clone)]
pub struct SessionMemory {
    pub files: HashMap<PathBuf, CachedFile>,
    pub budget_tokens: i32,
}

#[derive(Debug, Clone)]
pub struct CachedFile {
    pub content: String,
    pub tokens: i32,
    pub last_read: SystemTime,
    pub access_count: i32,
}

#[derive(Debug, Clone)]
pub struct SessionMemoryFile {
    pub path: PathBuf,
    pub content: String,
    pub last_read: SystemTime,
}
```

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

## Key Patterns Summary

| Pattern | Description |
|---------|-------------|
| **Streaming Tool Execution** | Tools start executing as their blocks complete, not after full response |
| **Concurrency Safety** | Read-only tools run in parallel; write tools run sequentially |
| **Micro-compaction** | Remove low-value tool results without full summarization |
| **Auto-compaction** | Summarize older messages when context approaches limit |
| **Model Fallback** | Switch to fallback model on overload with orphan message handling |
| **Event Streaming** | All state changes emit events for UI integration |
