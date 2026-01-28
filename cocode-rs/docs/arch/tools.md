# Tool System Architecture

## Overview

The tool system provides an abstraction for executable functions that the LLM can call.

**Key Features:**
- **5-Stage Execution Pipeline**: enabled → validation → hooks → permissions → execution (Claude Code v2.1.7 aligned)
- **Concurrency Safety**: Safe tools run in parallel, unsafe run sequentially
- **Streaming Execution**: Tools execute during API streaming, not after
- **Permission Checking**: Approval flow for sensitive operations

## Core Types

### Tool Trait (Complete)

```rust
use async_trait::async_trait;
use hyper_sdk::ToolDefinition;
use serde_json::Value;

/// Concurrency safety marker
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConcurrencySafety {
    /// Tool can run in parallel with other safe tools
    Safe,
    /// Tool must run sequentially (modifies state)
    #[default]
    Unsafe,
}

/// Core tool trait with 5-stage execution pipeline
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name for matching
    fn name(&self) -> &str;

    /// Generate tool definition for LLM
    fn definition(&self) -> ToolDefinition;

    /// Dynamic description (can vary based on context)
    async fn description(&self, ctx: &ToolContext) -> String {
        self.definition().description.unwrap_or_default()
    }

    /// Input schema for validation
    fn input_schema(&self) -> Value;

    /// Output schema for documentation
    fn output_schema(&self) -> Option<Value> {
        None
    }

    // ============ 5-Stage Execution Pipeline (Claude Code v2.1.7 Aligned) ============
    // Order: enabled → validation → hooks → permissions → execution

    /// Stage 1: Is this tool enabled in current context?
    fn is_enabled(&self, ctx: &ToolContext) -> bool {
        true
    }

    /// Stage 2: Validate input (schema + custom validation)
    /// NOTE: Runs BEFORE hooks - modified input from hooks bypasses validation
    async fn validate_input(
        &self,
        input: &Value,
        ctx: &ToolContext,
    ) -> ValidationResult {
        // Default: schema validation only
        validate_against_schema(input, &self.input_schema())
    }

    /// Stage 3: Hooks (PreToolUse) - handled by executor, not tool trait
    /// Hooks can reject or modify input

    /// Stage 4: Check permissions (may prompt user)
    /// NOTE: Runs AFTER hooks - permission check applies to modified input
    async fn check_permissions(
        &self,
        input: &Value,
        ctx: &ToolContext,
    ) -> PermissionResult {
        PermissionResult::Allowed
    }

    /// Stage 5: Execute the tool
    async fn call(
        &self,
        input: Value,
        ctx: &ToolContext,
        tool_use_id: &str,
        metadata: ToolMetadata,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<ToolOutput, ToolError>;

    /// Stage 5: Map result to API format
    fn map_result_to_content_block(
        &self,
        result: ToolOutput,
        tool_use_id: &str,
    ) -> ContentBlock {
        ContentBlock::tool_result(tool_use_id, result.content, result.is_error)
    }

    // ============ Rendering ============

    /// Render tool use for display (optional)
    fn render_tool_use(&self, input: &Value) -> Option<String> {
        None  // Default: no custom rendering
    }

    /// Render tool result for display (optional)
    fn render_tool_result(&self, result: &ToolOutput) -> Option<String> {
        None  // Default: no custom rendering
    }

    // ============ Metadata ============

    /// Whether tool can run in parallel with other safe tools
    fn is_concurrency_safe(&self, input: &Value) -> bool {
        self.is_read_only()
    }

    /// Whether tool is read-only (doesn't modify state)
    fn is_read_only(&self) -> bool {
        false
    }

    /// Maximum result size before truncation
    fn max_result_size_chars(&self) -> i32 {
        30000
    }
}

#[derive(Debug, Clone)]
pub enum PermissionResult {
    Allowed,
    Denied { reason: String },
    NeedsApproval { request: ApprovalRequest },
}

#[derive(Debug, Clone)]
pub enum ValidationResult {
    Valid,
    Invalid { errors: Vec<ValidationError> },
}
```

### File Edit Watch System (Claude Code v2.1.7 Aligned)

The File Edit Watch system ensures data integrity by enforcing Read-Before-Edit semantics and detecting external file modifications. This is a critical safety mechanism that prevents data loss from concurrent modifications.

#### Three-Layer Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    File Edit Watch System                        │
│                                                                  │
│  Layer 1: File System Abstraction                               │
│  ├── All Read/Edit/Write operations go through monitored APIs   │
│  └── Tracks file state at operation time                        │
│                                                                  │
│  Layer 2: readFileState Map                                     │
│  ├── Tracks {content, timestamp, offset, limit, file_mtime}     │
│  └── Used for Edit/Write validation                             │
│                                                                  │
│  Layer 3: File System Watcher (Chokidar-style)                  │
│  ├── Monitors previously-read files for external changes        │
│  └── Generates changed_files attachments                        │
└─────────────────────────────────────────────────────────────────┘
```

#### ReadFileState Types

```rust
/// Read file state for tracking file reads
#[derive(Debug, Clone, Default)]
pub struct ReadFileState {
    /// Files that have been read
    pub files: HashMap<PathBuf, FileReadInfo>,
    /// File system watcher handle (for external change detection)
    watcher: Option<Arc<RwLock<FileWatcher>>>,
}

#[derive(Debug, Clone)]
pub struct FileReadInfo {
    /// File content when read
    pub content: String,
    /// When the file was read (internal timestamp)
    pub timestamp: SystemTime,
    /// Offset used when reading (if partial)
    pub offset: Option<i32>,
    /// Limit used when reading (if partial)
    pub limit: Option<i32>,
    /// File modification time when read (from filesystem)
    pub file_mtime: SystemTime,
    /// Access count for session memory prioritization
    pub access_count: i32,
    /// Whether this was a complete read (no offset/limit)
    pub is_complete_read: bool,
}

/// File watcher for detecting external modifications
pub struct FileWatcher {
    /// Set of paths being watched
    watched_paths: HashSet<PathBuf>,
    /// Pending change notifications
    pending_changes: Vec<FileChange>,
    /// Debounce duration for rapid changes
    debounce: Duration,
}

#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub change_type: FileChangeType,
    pub detected_at: SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeType {
    Modified,
    Deleted,
    Created,
}
```

#### Read-Before-Edit Enforcement Flow

```
Read Tool Called
    │
    ▼
Record in readFileState
├── content: file content
├── timestamp: SystemTime::now()
├── file_mtime: fs::metadata(path).modified()
├── offset/limit: if partial read
├── access_count: increment
└── is_complete_read: offset.is_none() && limit.is_none()
    │
    ▼
Add path to file watcher (if not already watched)
    │
    ▼
(Later) Edit/Write Tool Called
    │
    ▼
Validation Flow:
1. Check readFileState.files.contains(path)
   └── If missing: Error "Must read file before editing"
    │
    ▼
2. Check timestamp validation (primary)
   ├── Get current file_mtime from filesystem
   └── Compare with stored file_mtime
       ├── If mtime changed → Step 3 (content validation)
       └── If mtime same → Allow edit
    │
    ▼
3. Content validation fallback (v2.1.7 enhancement)
   ├── Only for complete reads (is_complete_read == true)
   ├── Read current file content
   └── Compare with stored content
       ├── If content same → Allow edit (false positive mtime change)
       └── If content different → Error "File modified externally"
    │
    ▼
4. Execute Edit/Write
    │
    ▼
5. Update readFileState with new content
```

#### Implementation

```rust
impl ReadFileState {
    /// Create new read file state with optional watcher
    pub fn new(enable_watcher: bool) -> Self {
        Self {
            files: HashMap::new(),
            watcher: if enable_watcher {
                Some(Arc::new(RwLock::new(FileWatcher::new())))
            } else {
                None
            },
        }
    }

    /// Check if file can be edited (has been read and not modified since)
    pub fn can_edit(&self, path: &Path) -> Result<(), EditError> {
        let info = self.files.get(path)
            .ok_or_else(|| EditError::NotRead {
                path: path.to_path_buf(),
            })?;

        // Primary check: timestamp validation
        let current_mtime = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok();

        if let Some(mtime) = current_mtime {
            if mtime > info.file_mtime {
                // v2.1.7 enhancement: Content validation fallback
                // Handle "false positive" timestamp changes (e.g., touch without content change)
                if info.is_complete_read {
                    if let Ok(current_content) = std::fs::read_to_string(path) {
                        if current_content == info.content {
                            // Content unchanged despite mtime change - allow edit
                            return Ok(());
                        }
                    }
                }

                return Err(EditError::ModifiedSinceRead {
                    path: path.to_path_buf(),
                    read_at: info.timestamp,
                    modified_at: mtime,
                });
            }
        }

        Ok(())
    }

    /// Record a file read
    pub fn record_read(
        &mut self,
        path: &Path,
        content: &str,
        offset: Option<i32>,
        limit: Option<i32>,
    ) {
        let file_mtime = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| SystemTime::now());

        let is_complete_read = offset.is_none() && limit.is_none();

        // Update or insert file info
        if let Some(existing) = self.files.get_mut(path) {
            existing.content = content.to_string();
            existing.timestamp = SystemTime::now();
            existing.offset = offset;
            existing.limit = limit;
            existing.file_mtime = file_mtime;
            existing.access_count += 1;
            existing.is_complete_read = is_complete_read;
        } else {
            self.files.insert(path.to_path_buf(), FileReadInfo {
                content: content.to_string(),
                timestamp: SystemTime::now(),
                offset,
                limit,
                file_mtime,
                access_count: 1,
                is_complete_read,
            });
        }

        // Add to watcher
        if let Some(watcher) = &self.watcher {
            let mut w = watcher.write().unwrap();
            w.watch(path);
        }
    }

    /// Update file state after successful edit
    pub fn record_edit(&mut self, path: &Path, new_content: &str) {
        if let Some(info) = self.files.get_mut(path) {
            info.content = new_content.to_string();
            info.timestamp = SystemTime::now();
            info.file_mtime = std::fs::metadata(path)
                .and_then(|m| m.modified())
                .unwrap_or_else(|_| SystemTime::now());
            info.access_count += 1;
            info.is_complete_read = true;  // After edit, we have full content
        }
    }

    /// Get pending file changes from watcher
    pub fn get_pending_changes(&self) -> Vec<FileChange> {
        if let Some(watcher) = &self.watcher {
            let mut w = watcher.write().unwrap();
            w.drain_changes()
        } else {
            vec![]
        }
    }
}

#[derive(Debug)]
pub enum EditError {
    /// File has not been read
    NotRead { path: PathBuf },
    /// File was modified since it was read
    ModifiedSinceRead {
        path: PathBuf,
        read_at: SystemTime,
        modified_at: SystemTime,
    },
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRead { path } => {
                write!(f, "Must read file before editing: {}", path.display())
            }
            Self::ModifiedSinceRead { path, .. } => {
                write!(f, "File was modified externally since last read: {}", path.display())
            }
        }
    }
}

impl std::error::Error for EditError {}
```

#### File Watcher Implementation

```rust
impl FileWatcher {
    pub fn new() -> Self {
        Self {
            watched_paths: HashSet::new(),
            pending_changes: Vec::new(),
            debounce: Duration::from_millis(100),
        }
    }

    /// Add path to watch list
    pub fn watch(&mut self, path: &Path) {
        self.watched_paths.insert(path.to_path_buf());
    }

    /// Remove path from watch list
    pub fn unwatch(&mut self, path: &Path) {
        self.watched_paths.remove(path);
    }

    /// Record a file change event
    pub fn notify_change(&mut self, path: PathBuf, change_type: FileChangeType) {
        // Deduplicate rapid changes
        if let Some(last) = self.pending_changes.last() {
            if last.path == path && last.detected_at.elapsed().unwrap_or_default() < self.debounce {
                return;
            }
        }

        self.pending_changes.push(FileChange {
            path,
            change_type,
            detected_at: SystemTime::now(),
        });
    }

    /// Drain pending changes
    pub fn drain_changes(&mut self) -> Vec<FileChange> {
        std::mem::take(&mut self.pending_changes)
    }
}
```

#### Integration with changed_files Attachment

The File Edit Watch system integrates with the `changed_files` attachment to notify the model about external file modifications:

```rust
/// Generate changed_files attachment from file watcher
pub async fn generate_changed_files_attachment(
    read_file_state: &ReadFileState,
) -> Option<Attachment> {
    let changes = read_file_state.get_pending_changes();

    if changes.is_empty() {
        return None;
    }

    let mut changed_files = Vec::new();

    for change in changes {
        match change.change_type {
            FileChangeType::Modified => {
                // Generate unified diff if we have the previous content
                if let Some(info) = read_file_state.files.get(&change.path) {
                    if let Ok(current) = tokio::fs::read_to_string(&change.path).await {
                        let diff = generate_unified_diff(&info.content, &current, &change.path);
                        changed_files.push(ChangedFile::TextFile {
                            filename: change.path.clone(),
                            snippet: diff,
                        });
                    }
                }
            }
            FileChangeType::Deleted => {
                changed_files.push(ChangedFile::TextFile {
                    filename: change.path.clone(),
                    snippet: "[File deleted]".to_string(),
                });
            }
            FileChangeType::Created => {
                // Typically ignored - we only track previously-read files
            }
        }
    }

    if changed_files.is_empty() {
        return None;
    }

    Some(Attachment::ChangedFiles { files: changed_files })
}
```

#### Configuration

| Constant | Value | Description |
|----------|-------|-------------|
| `FILE_WATCHER_DEBOUNCE_MS` | 100 | Debounce duration for rapid file changes |
| `MAX_DIFF_LINES` | 100 | Max lines in unified diff for changed_files |
| `CONTENT_VALIDATION_MAX_SIZE` | 1MB | Max file size for content validation fallback |

### Tool Execution Context

```rust
/// Tool execution context
#[derive(Clone)]
pub struct ToolContext {
    /// Unique ID for this tool call
    pub call_id: String,

    /// Current turn ID
    pub turn_id: String,

    /// Agent ID (if subagent)
    pub agent_id: Option<String>,

    /// Working directory
    pub cwd: PathBuf,

    /// Additional working directories
    pub additional_working_directories: Vec<PathBuf>,

    /// Current conversation messages
    pub messages: Arc<RwLock<Vec<ConversationMessage>>>,

    /// Read file state cache (for read-before-edit enforcement)
    pub read_file_state: Arc<RwLock<ReadFileState>>,

    /// Cancellation token
    pub cancel: CancellationToken,

    /// Permission context
    pub permission_context: Arc<PermissionContext>,

    /// App state accessors
    pub get_app_state: Arc<dyn Fn() -> AppState + Send + Sync>,
    pub set_app_state: Arc<dyn Fn(AppStateUpdater) + Send + Sync>,

    /// Tool options
    pub options: ToolUseOptions,

    /// Query tracking for analytics
    pub query_tracking: QueryTracking,

    /// Progress callback
    pub progress_tx: Option<mpsc::Sender<ToolProgress>>,
}

#[derive(Clone)]
pub struct ToolUseOptions {
    /// Available tools (filtered for this context)
    pub tools: Vec<ToolDefinition>,

    /// Model for the main loop
    pub main_loop_model: String,

    /// Max thinking tokens
    pub max_thinking_tokens: Option<i32>,

    /// MCP clients
    pub mcp_clients: Vec<Arc<McpClient>>,

    /// Agent definitions (for Task tool)
    pub agent_definitions: Vec<AgentDefinition>,
}

#[derive(Clone)]
pub struct QueryTracking {
    /// Chain ID for tracking related queries
    pub chain_id: String,

    /// Depth in subagent chain
    pub depth: i32,
}
```

### Tool Output

```rust
/// Tool output
#[derive(Debug, Clone)]
pub struct ToolOutput {
    /// Result content
    pub content: ToolResultContent,
    /// Whether this is an error
    pub is_error: bool,
    /// Context modifiers (state updates)
    pub modifiers: Vec<ContextModifier>,
}

impl ToolOutput {
    pub fn success(content: impl Into<ToolResultContent>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
            modifiers: vec![],
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: ToolResultContent::Text(message.into()),
            is_error: true,
            modifiers: vec![],
        }
    }

    pub fn with_modifier(mut self, modifier: ContextModifier) -> Self {
        self.modifiers.push(modifier);
        self
    }
}

/// Context modifiers that tools can apply
#[derive(Debug, Clone)]
pub enum ContextModifier {
    /// Update read file state
    FileRead { path: PathBuf, content: String },
    /// Set permission for tool
    PermissionGranted { tool: String, pattern: String },
    /// Queue a command for later execution
    QueueCommand { command: QueuedCommand },
}
```

### Tool Registry

```rust
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn register(&mut self, tool: impl Tool + 'static) {
        let name = tool.name().to_string();
        self.tools.insert(name, Arc::new(tool));
    }

    pub fn register_arc(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn all(&self) -> impl Iterator<Item = &Arc<dyn Tool>> {
        self.tools.values()
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values()
            .map(|t| t.definition())
            .collect()
    }

    /// Create registry from filtered tool list
    pub fn from(tools: Vec<Arc<dyn Tool>>) -> Self {
        let mut registry = Self::new();
        for tool in tools {
            registry.register_arc(tool);
        }
        registry
    }
}
```

## 5-Stage Tool Execution Pipeline (Claude Code v2.1.7 Aligned)

The execution order is: **enabled → validation → hooks → permissions → execution**

This ordering matches Claude Code v2.1.7's actual implementation (`packages/core/src/tools/execution.ts`).

```rust
pub async fn execute_single_tool(
    tool: &dyn Tool,
    input: Value,
    ctx: &ToolContext,
    tool_use_id: &str,
    hooks: &HookRegistry,
) -> ToolExecutionResult {
    // Stage 1: Check if enabled
    if !tool.is_enabled(ctx) {
        return ToolExecutionResult::error(
            tool_use_id,
            "Tool is disabled in current context"
        );
    }

    // Stage 2: Validate input (BEFORE hooks - Claude Code v2.1.7 aligned)
    let validation_result = tool.validate_input(&input, ctx).await;
    if let ValidationResult::Invalid { errors } = validation_result {
        return ToolExecutionResult::error(
            tool_use_id,
            format!("Validation failed: {:?}", errors)
        );
    }

    // Stage 3: PreToolUse hooks (can reject or modify input)
    let hook_result = hooks.execute(HookEventType::PreToolUse, HookContext {
        tool_name: tool.name(),
        input: &input,
        ctx,
    }).await;

    // Handle hook result - including ModifyInput
    let input = match hook_result {
        HookResult::Reject { reason } => {
            return ToolExecutionResult::error(tool_use_id, reason);
        }
        HookResult::ModifyInput { new_input } => {
            // NOTE: Modified input does NOT go through re-validation
            // This is Claude Code v2.1.7 behavior
            new_input
        }
        HookResult::Continue => input,
    };

    // Stage 4: Check permissions (AFTER hooks, on possibly-modified input)
    let permission_result = tool.check_permissions(&input, ctx).await;
    match permission_result {
        PermissionResult::Denied { reason } => {
            return ToolExecutionResult::error(tool_use_id, reason);
        }
        PermissionResult::NeedsApproval { request } => {
            // Send approval request, wait for response
            let approved = request_user_approval(request, ctx).await?;
            if !approved {
                return ToolExecutionResult::error(tool_use_id, "User denied permission");
            }
        }
        PermissionResult::Allowed => {}
    }

    // Stage 5: Execute (uses final possibly-modified input)
    let start = Instant::now();
    let result = tool.call(
        input.clone(),
        ctx,
        tool_use_id,
        ToolMetadata::default(),
        None,
    ).await;
    let duration = start.elapsed();

    match result {
        Ok(output) => {
            // Post-tool hook (success)
            hooks.execute(HookEventType::PostToolUse, HookContext {
                tool_name: tool.name(),
                input: &input,
                output: Some(&output),
                ctx,
            }).await;

            // Map to content block
            let content = tool.map_result_to_content_block(output.clone(), tool_use_id);

            // Handle oversized results
            let content = if content.size() > tool.max_result_size_chars() {
                persist_oversized_result(tool_use_id, &content, ctx).await?
            } else {
                content
            };

            ToolExecutionResult {
                tool_use_id: tool_use_id.to_string(),
                content,
                is_error: output.is_error,
                duration,
                modifiers: output.modifiers,
            }
        }
        Err(error) => {
            // Post-tool hook (failure)
            hooks.execute(HookEventType::PostToolUseFailure, HookContext {
                tool_name: tool.name(),
                input: &input,
                error: Some(&error),
                ctx,
            }).await;

            ToolExecutionResult::error(tool_use_id, error.to_string())
        }
    }
}
```

### Hook Input Modification Security Model (Claude Code v2.1.7 Aligned)

When `PreToolUse` hooks return `ModifyInput`, the modified input follows this path:

| Check | Runs on modified input? | Notes |
|-------|------------------------|-------|
| Schema Validation | **NO** | Already completed before hooks |
| Custom Validation | **NO** | Already completed before hooks |
| Permission Check | **YES** | Happens after hooks |

**Security Implications:**

1. Hooks can bypass schema validation by modifying input after validation completes
2. Permission checks still apply to modified input (providing some protection)
3. Consider restricting `updatedInput` capability to policy-level hooks only

**Execution Flow:**

```
Original Input
    │
    ├─► Stage 1: Enabled check
    │
    ├─► Stage 2: Schema Validation (Zod) ✓
    │
    ├─► Stage 2: Custom Validation ✓
    │
    ├─► Stage 3: PreToolUse Hooks
    │       │
    │       └─► Can return { updatedInput: modified }
    │
    ▼
Modified Input
    │
    ├─► Stage 4: Permission Check ✓ (on modified input)
    │       │
    │       └─► Permission decision can also modify input
    │
    └─► Stage 5: Tool Execution (NO schema re-validation)
```

**Recommendations:**

1. **Policy hooks only:** Consider allowing `updatedInput` only from policy-level hooks
2. **Audit logging:** Log when hooks modify input for security review
3. **Sensitive tools:** Add extra validation in `tool.call()` for security-critical tools

## Built-in Tools

For detailed implementation of all built-in tools, see **[tools-builtin.md](./tools-builtin.md)**.

The built-in tools include:
- **Read** - File reading (supports images, PDFs, notebooks)
- **Edit** - Exact string replacement in files
- **Write** - File writing
- **Glob** - File pattern matching
- **Grep** - Content searching (ripgrep-based)
- **Bash** - Shell command execution
- **Task** - Subagent spawning
- **TodoWrite** - Progress tracking
- **EnterPlanMode / ExitPlanMode** - Plan mode workflow
- **KillShell** - Background task termination
- **TaskOutput** - Background task output retrieval

## MCP Tool Integration

```rust
/// MCP tool naming convention: mcp__<server>__<tool>
pub fn mcp_tool_name(server_name: &str, tool_name: &str) -> String {
    format!("mcp__{server_name}__{tool_name}")
}

/// Parse MCP tool name
pub fn parse_mcp_tool_name(name: &str) -> Option<(String, String)> {
    if !name.starts_with("mcp__") {
        return None;
    }
    let parts: Vec<_> = name.strip_prefix("mcp__")?.split("__").collect();
    if parts.len() == 2 {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
}

/// MCP tool wrapper
pub struct McpToolWrapper {
    full_name: String,
    server_name: String,
    tool_def: McpToolDefinition,
    client: Arc<McpClient>,
}

#[async_trait]
impl Tool for McpToolWrapper {
    fn name(&self) -> &str {
        &self.full_name
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::full(
            &self.full_name,
            &self.tool_def.description.clone().unwrap_or_default(),
            self.tool_def.input_schema.clone(),
        )
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true  // MCP tools are assumed safe (external service)
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolContext,
        _tool_use_id: &str,
        _metadata: ToolMetadata,
        _progress: Option<ProgressCallback>,
    ) -> Result<ToolOutput, ToolError> {
        // Emit McpToolCallBegin event
        ctx.emit_event(LoopEvent::McpToolCallBegin {
            server: self.server_name.clone(),
            tool: self.tool_def.name.clone(),
            call_id: ctx.call_id.clone(),
        });

        // Call MCP server
        let result = self.client.tools_call(
            &self.tool_def.name,
            input,
        ).await?;

        // Emit McpToolCallEnd event
        ctx.emit_event(LoopEvent::McpToolCallEnd {
            server: self.server_name.clone(),
            tool: self.tool_def.name.clone(),
            call_id: ctx.call_id.clone(),
            is_error: result.is_error.unwrap_or(false),
        });

        if result.is_error.unwrap_or(false) {
            Ok(ToolOutput::error(format_mcp_content(&result.content)))
        } else {
            Ok(ToolOutput::success(format_mcp_content(&result.content)))
        }
    }
}
```

## Tool Registration

```rust
// cocode-tools/src/lib.rs

pub fn register_all_tools(registry: &mut ToolRegistry) {
    // File operations
    registry.register(ReadTool);
    registry.register(WriteTool);
    registry.register(EditTool);
    registry.register(GlobTool);
    registry.register(GrepTool);
    registry.register(NotebookEditTool);

    // Command execution
    registry.register(BashTool::new());

    // Web operations
    registry.register(WebFetchTool);
    registry.register(WebSearchTool);

    // User interaction
    registry.register(AskUserQuestionTool);

    // Plan mode
    registry.register(EnterPlanModeTool);
    registry.register(ExitPlanModeTool);

    // Task management (progress tracking)
    registry.register(TodoWriteTool);

    // Background task
    registry.register(TaskOutputTool);
    registry.register(KillShellTool);

    // Skills
    registry.register(SkillTool);

    // LSP (optional, if server manager available)
    // registry.register(LSPTool::new(lsp_manager));
}

pub fn register_task_tool(registry: &mut ToolRegistry, manager: Arc<SubagentManager>) {
    registry.register(TaskTool::new(manager));
}

pub fn register_mcp_tools(
    registry: &mut ToolRegistry,
    connection_manager: &McpConnectionManager,
) -> Result<(), ToolError> {
    for (server_name, client) in connection_manager.clients() {
        let tools = client.list_tools().await?;
        for tool_def in tools {
            let wrapper = McpToolWrapper {
                full_name: mcp_tool_name(&server_name, &tool_def.name),
                server_name: server_name.clone(),
                tool_def,
                client: client.clone(),
            };
            registry.register(wrapper);
        }
    }
    Ok(())
}
```

## Concurrency Model

### Concurrent-Safe Tools

These tools can run in parallel:
- `Read` - File reading
- `Glob` - File pattern matching
- `Grep` - Content searching
- `WebFetch` - URL fetching
- `WebSearch` - Web searching
- `Task` - Spawning subagents (launches separate process)
- `TaskOutput` - Get background task output
- `KillShell` - Stop background tasks (stateless operation)
- `LSP` - Language Server Protocol operations
- MCP tools (external services)

### Sequential Tools

These tools must run one at a time:
- `Write` - File writing
- `Edit` - File editing
- `Bash` (write commands) - Shell commands that modify state
- `NotebookEdit` - Jupyter notebook editing
- `EnterPlanMode` / `ExitPlanMode` - Mode changes
- `TodoWrite` - Task list updates
- `Skill` - Skill execution
- `AskUserQuestion` - User interaction

### Execution Flow

```
Tool calls from LLM: [Read A, Edit B, Read C, Bash "ls"]

1. Check concurrency safety for each:
   Read A     → Safe (read-only)
   Edit B     → Unsafe (writes)
   Read C     → Safe (read-only)
   Bash "ls"  → Safe (is_read_only_command)

2. Partition:
   Safe: [Read A, Read C, Bash "ls"]
   Unsafe: [Edit B]

3. Execute safe tools in parallel:
   spawn(Read A)  ─┐
   spawn(Read C)  ─┼─► collect results
   spawn(Bash ls) ─┘

4. Execute unsafe tools sequentially:
   await(Edit B) ─► result

5. Combine all results in original order
```

### Max Concurrency Configuration

The tool execution system limits parallel tool execution to prevent resource exhaustion:

```rust
/// Maximum concurrent tool executions
pub const DEFAULT_MAX_TOOL_CONCURRENCY: i32 = 10;

/// Environment variable to override
pub const MAX_TOOL_CONCURRENCY_ENV: &str = "COCODE_MAX_TOOL_USE_CONCURRENCY";

pub fn get_max_tool_concurrency() -> i32 {
    std::env::var(MAX_TOOL_CONCURRENCY_ENV)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_TOOL_CONCURRENCY)
}
```

## Built-in Tool List

| Tool | Read-Only | Concurrency | Max Result Size | Description |
|------|-----------|-------------|-----------------|-------------|
| Read | Yes | Safe | 100,000 chars | Read files (supports images, PDFs, notebooks) |
| Write | No | Unsafe | N/A | Write files |
| Edit | No | Unsafe | N/A | Exact string replacement in files |
| Glob | Yes | Safe | 30,000 chars | Find files by pattern |
| Grep | Yes | Safe | 20,000 chars | Search file contents (ripgrep-based) |
| Bash | Varies | Varies | 30,000 chars | Execute shell commands (timeout: 2min default, 10min max) |
| WebFetch | Yes | Safe | N/A | Fetch URL content |
| WebSearch | Yes | Safe | N/A | Search the web |
| Task | Yes | Safe | N/A | Spawn subagents (Bash, Explore, Plan, etc.) |
| TaskOutput | Yes | Safe | N/A | Get background task output |
| KillShell | No | Safe | N/A | Stop background bash shell by ID |
| AskUserQuestion | No | Unsafe | N/A | Ask user for input (1-4 questions) |
| EnterPlanMode | No | Unsafe | N/A | Enter plan mode |
| ExitPlanMode | No | Unsafe | N/A | Exit plan mode with approval |
| TodoWrite | No | Unsafe | N/A | Create/update task list (atomic replace) |
| NotebookEdit | No | Unsafe | N/A | Edit Jupyter notebooks |
| Skill | No | Unsafe | N/A | Execute a skill (slash command) |
| LSP | Yes | Safe | N/A | LSP operations (goto_definition, hover, find_references) |

### LSP Tool (Phase 6)

For LSP tool implementation details, see **[tools-lsp.md](./tools-lsp.md)**.

The LSP tool provides Language Server Protocol operations for code intelligence including `goto_definition`, `hover`, and `find_references`.

## Permission System

### PermissionBehavior Types (Claude Code v2.1.7 Aligned)

The permission system uses `PermissionBehavior` to determine how each tool permission check should be handled:

```rust
/// Permission check behavior
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionBehavior {
    /// Tool is always allowed without prompting
    Allow,
    /// Tool requires user approval before execution
    Ask,
    /// Tool is always denied
    Deny,
}

/// Result of permission check with behavior guidance
#[derive(Debug, Clone)]
pub struct PermissionCheckResult {
    /// The behavior to apply
    pub behavior: PermissionBehavior,
    /// Optional message to display
    pub message: Option<String>,
    /// Detected risks (for Bash commands)
    pub risks: Vec<SecurityRisk>,
}

#[derive(Debug, Clone)]
pub struct SecurityRisk {
    pub risk_type: RiskType,
    pub severity: RiskSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskSeverity {
    Low,
    Medium,
    High,
    Critical,
}
```

### Sensitive Path Patterns

The permission system automatically detects and protects sensitive files:

```rust
/// Sensitive path patterns that require extra approval
pub struct SensitivePathConfig {
    /// Patterns that always require approval
    pub sensitive_patterns: Vec<&'static str>,
    /// Patterns that are always denied (cannot edit)
    pub protected_patterns: Vec<&'static str>,
}

impl Default for SensitivePathConfig {
    fn default() -> Self {
        Self {
            sensitive_patterns: vec![
                // Credentials and secrets
                ".env",
                ".env.*",
                "*.pem",
                "*.key",
                "*.p12",
                "*.pfx",
                "credentials.*",
                "secrets.*",
                "**/credentials/**",
                "**/secrets/**",

                // SSH and GPG
                ".ssh/*",
                ".gnupg/*",
                "id_rsa*",
                "id_ed25519*",
                "*.gpg",

                // Cloud provider configs
                ".aws/credentials",
                ".aws/config",
                ".azure/*",
                ".gcloud/*",
                ".kube/config",

                // Package manager tokens
                ".npmrc",
                ".pypirc",
                ".gem/credentials",
                ".docker/config.json",

                // Database configs
                "database.yml",
                "database.json",
                "**/db/seeds/**",

                // CI/CD secrets
                ".github/workflows/*.yml",
                ".gitlab-ci.yml",
                "Jenkinsfile",
            ],
            protected_patterns: vec![
                // System files
                "/etc/passwd",
                "/etc/shadow",
                "/etc/sudoers",

                // Claude Code internal
                "~/.claude/settings.json",  // Requires JSON validation
            ],
        }
    }
}

impl SensitivePathConfig {
    /// Check if path matches sensitive pattern
    pub fn is_sensitive(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        self.sensitive_patterns.iter().any(|pattern| {
            glob_matches(pattern, &path_str)
        })
    }

    /// Check if path is protected (cannot be edited)
    pub fn is_protected(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        self.protected_patterns.iter().any(|pattern| {
            glob_matches(pattern, &path_str)
        })
    }
}
```

### Permission Check Flow

```
Tool Permission Check
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│ 1. Check PermissionMode                                          │
│    ├── Bypass → Allow                                           │
│    ├── DontAsk → Check granted only, else Deny                  │
│    ├── Plan → Allow read-only tools, Ask for others            │
│    ├── AcceptEdits → Allow writes, Ask for Bash                │
│    └── Default → Continue to step 2                            │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│ 2. Check Tool-Specific Rules                                     │
│    ├── Edit/Write: Check sensitive paths                        │
│    │   ├── Protected path → Deny                                │
│    │   └── Sensitive path → Ask (with warning)                  │
│    ├── Bash: Run security analysis                              │
│    │   ├── Allow-phase risks → Deny                             │
│    │   └── Ask-phase risks → Ask (with risk details)            │
│    └── Other tools → Continue to step 3                        │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│ 3. Check Granted Permissions                                     │
│    ├── Exact match → Allow                                      │
│    ├── Wildcard match → Allow                                   │
│    └── No match → Ask                                           │
└─────────────────────────────────────────────────────────────────┘
```

### PermissionContext Implementation

```rust
pub struct PermissionContext {
    /// Granted permissions by tool and pattern
    granted: HashMap<String, Vec<PermissionPattern>>,

    /// Permission mode
    mode: PermissionMode,

    /// Sensitive path configuration
    sensitive_paths: SensitivePathConfig,

    /// Pre-approval configuration for web tools
    pre_approval: PreApprovalConfig,

    /// Disabled tools
    disabled_tools: HashMap<String, Vec<PermissionPattern>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    /// Normal operation - prompt for permissions
    Default,
    /// Plan mode - read-only, auto-approve reads
    Plan,
    /// Accept edits - auto-approve write operations
    AcceptEdits,
    /// Bypass - auto-approve everything (for subagents)
    Bypass,
    /// Don't ask - auto-decline unknown tools
    DontAsk,
}

impl PermissionContext {
    pub fn is_allowed(&self, tool: &str, action: &str) -> bool {
        match self.mode {
            PermissionMode::Bypass => true,
            PermissionMode::Plan => {
                // Only read operations allowed without approval
                matches!(tool, "Read" | "Glob" | "Grep" | "WebFetch" | "WebSearch")
            }
            PermissionMode::AcceptEdits => {
                // Auto-approve writes
                true
            }
            PermissionMode::DontAsk => {
                // Auto-decline if not already approved
                self.granted.get(tool)
                    .map(|patterns| patterns.iter().any(|p| p.matches(action)))
                    .unwrap_or(false)
            }
            PermissionMode::Default => {
                // Check granted patterns
                self.granted.get(tool)
                    .map(|patterns| patterns.iter().any(|p| p.matches(action)))
                    .unwrap_or(false)
            }
        }
    }

    pub fn grant(&mut self, tool: &str, pattern: &str) {
        self.granted
            .entry(tool.to_string())
            .or_default()
            .push(PermissionPattern::new(pattern));
    }
}
```

### Wildcard Permission Patterns

Claude Code v2.1.7 supports wildcard patterns for more flexible permission management.

#### Bash Wildcards

Bash permissions support glob-style wildcards:

| Pattern | Matches | Example Commands |
|---------|---------|------------------|
| `Bash(npm *)` | Any npm command | `npm install`, `npm run test` |
| `Bash(* install)` | Commands ending in "install" | `npm install`, `pip install` |
| `Bash(git * main)` | Git commands on main | `git push main`, `git pull main` |
| `Bash(cargo *)` | Any cargo command | `cargo build`, `cargo test` |
| `Bash(make *)` | Any make command | `make build`, `make test` |

```rust
/// Wildcard pattern for Bash permissions
#[derive(Debug, Clone)]
pub struct BashWildcardPattern {
    /// Pattern with wildcards (* for any characters)
    pub pattern: String,
    /// Pre-compiled regex
    regex: Regex,
}

impl BashWildcardPattern {
    pub fn new(pattern: &str) -> Self {
        // Convert glob pattern to regex
        let regex_pattern = pattern
            .replace("*", ".*")
            .replace("?", ".");
        let regex = Regex::new(&format!("^{}$", regex_pattern))
            .unwrap_or_else(|_| Regex::new("^$").unwrap());

        Self {
            pattern: pattern.to_string(),
            regex,
        }
    }

    pub fn matches(&self, command: &str) -> bool {
        self.regex.is_match(command)
    }
}

// Example usage in permission config:
// "allowed_tools": ["Bash(npm *)", "Bash(cargo *)", "Bash(git status)"]
```

#### MCP Wildcards

MCP tool permissions support server-level wildcards:

| Pattern | Matches | Description |
|---------|---------|-------------|
| `mcp__server__*` | All tools from server | Grant all tools from a specific MCP server |
| `mcp__*__read` | All read tools | Grant "read" tool from all servers |
| `mcp__filesystem__*` | All filesystem tools | Grant all filesystem server tools |

```rust
/// MCP wildcard pattern
#[derive(Debug, Clone)]
pub struct McpWildcardPattern {
    /// Server pattern (supports *)
    pub server_pattern: String,
    /// Tool pattern (supports *)
    pub tool_pattern: String,
}

impl McpWildcardPattern {
    /// Parse MCP permission pattern
    pub fn parse(pattern: &str) -> Option<Self> {
        // Pattern format: mcp__<server>__<tool>
        let parts: Vec<&str> = pattern.split("__").collect();
        if parts.len() != 3 || parts[0] != "mcp" {
            return None;
        }

        Some(Self {
            server_pattern: parts[1].to_string(),
            tool_pattern: parts[2].to_string(),
        })
    }

    /// Check if pattern matches an MCP tool
    pub fn matches(&self, server: &str, tool: &str) -> bool {
        let server_match = self.server_pattern == "*" ||
                          self.server_pattern == server ||
                          glob_matches(&self.server_pattern, server);
        let tool_match = self.tool_pattern == "*" ||
                        self.tool_pattern == tool ||
                        glob_matches(&self.tool_pattern, tool);

        server_match && tool_match
    }
}

fn glob_matches(pattern: &str, text: &str) -> bool {
    let regex_pattern = pattern.replace("*", ".*");
    Regex::new(&format!("^{}$", regex_pattern))
        .map(|r| r.is_match(text))
        .unwrap_or(false)
}
```

#### Task-Specific Disabling

Disable specific agent types using `Task(AgentName)`:

| Pattern | Effect | Description |
|---------|--------|-------------|
| `Task(Explore)` | Disable Explore agent | Prevent spawning Explore subagents |
| `Task(Plan)` | Disable Plan agent | Prevent spawning Plan subagents |
| `Task(Bash)` | Disable Bash agent | Prevent spawning Bash subagents |
| `Task(*)` | Disable all agents | Prevent spawning any subagents |

```rust
/// Task (subagent) permission pattern
#[derive(Debug, Clone)]
pub struct TaskPermissionPattern {
    /// Agent type to allow/disallow
    pub agent_type: String,
}

impl TaskPermissionPattern {
    pub fn parse(pattern: &str) -> Option<Self> {
        // Pattern format: Task(<AgentType>)
        if !pattern.starts_with("Task(") || !pattern.ends_with(")") {
            return None;
        }

        let agent_type = pattern
            .strip_prefix("Task(")?
            .strip_suffix(")")?
            .to_string();

        Some(Self { agent_type })
    }

    pub fn matches(&self, requested_agent: &str) -> bool {
        self.agent_type == "*" || self.agent_type == requested_agent
    }
}

/// Check if Task tool is allowed for specific agent type
impl PermissionContext {
    pub fn is_task_allowed(&self, agent_type: &str) -> bool {
        // Check for explicit disabling
        if let Some(patterns) = self.granted.get("Task") {
            for pattern in patterns {
                if let Some(task_pattern) = TaskPermissionPattern::parse(&pattern.raw) {
                    if task_pattern.matches(agent_type) {
                        return true;
                    }
                }
            }
        }

        // Default: allow all agent types unless explicitly disabled
        !self.is_task_disabled(agent_type)
    }

    fn is_task_disabled(&self, agent_type: &str) -> bool {
        if let Some(disabled) = self.disabled_tools.get("Task") {
            for pattern in disabled {
                if let Some(task_pattern) = TaskPermissionPattern::parse(&pattern.raw) {
                    if task_pattern.matches(agent_type) {
                        return true;
                    }
                }
            }
        }
        false
    }
}
```

#### Pre-Approval Lists

Domain-based pre-approval for WebFetch and WebSearch:

```rust
/// Pre-approval configuration for web tools
#[derive(Debug, Clone)]
pub struct PreApprovalConfig {
    /// Pre-approved domains for WebFetch
    pub allowed_domains: Vec<String>,
    /// Blocked domains (never auto-approve)
    pub blocked_domains: Vec<String>,
    /// Auto-approve all HTTPS URLs
    pub auto_approve_https: bool,
}

impl Default for PreApprovalConfig {
    fn default() -> Self {
        Self {
            allowed_domains: vec![
                "docs.rs".to_string(),
                "crates.io".to_string(),
                "github.com".to_string(),
                "stackoverflow.com".to_string(),
                "developer.mozilla.org".to_string(),
            ],
            blocked_domains: vec![],
            auto_approve_https: false,
        }
    }
}

impl PermissionContext {
    /// Check if URL is pre-approved
    pub fn is_url_preapproved(&self, url: &str) -> bool {
        let parsed = url::Url::parse(url).ok();
        let host = parsed.as_ref().and_then(|u| u.host_str());

        if let Some(domain) = host {
            // Check blocked first
            if self.pre_approval.blocked_domains.iter().any(|d| domain.ends_with(d)) {
                return false;
            }

            // Check allowed
            if self.pre_approval.allowed_domains.iter().any(|d| domain.ends_with(d)) {
                return true;
            }

            // Check HTTPS auto-approve
            if self.pre_approval.auto_approve_https {
                if let Some(u) = parsed {
                    return u.scheme() == "https";
                }
            }
        }

        false
    }
}
```

#### Configuration Example

```json
{
  "permissions": {
    "allowed_tools": [
      "Read",
      "Glob",
      "Grep",
      "Bash(npm *)",
      "Bash(cargo *)",
      "Bash(git status)",
      "Bash(git diff)",
      "mcp__filesystem__*",
      "Task(Explore)",
      "Task(Plan)"
    ],
    "disabled_tools": [
      "Task(Bash)"
    ],
    "pre_approval": {
      "allowed_domains": ["docs.rs", "crates.io", "github.com"],
      "auto_approve_https": false
    }
  }
}
```

## Oversized Result Handling

Handle tool outputs that exceed the context limit:

```rust
/// Oversized result configuration
#[derive(Debug, Clone)]
pub struct OversizedResultConfig {
    /// Maximum characters before truncation (default: 30000)
    pub max_chars: i32,
    /// Directory for persisting oversized results
    pub persist_dir: PathBuf,
    /// Truncation strategy
    pub truncation_strategy: TruncationStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TruncationStrategy {
    /// Keep end of output (most recent)
    #[default]
    KeepEnd,
    /// Keep start of output
    KeepStart,
    /// Keep both start and end
    KeepBoth,
}

impl Default for OversizedResultConfig {
    fn default() -> Self {
        Self {
            max_chars: 30000,
            persist_dir: PathBuf::from("~/.claude/oversized/"),
            truncation_strategy: TruncationStrategy::KeepEnd,
        }
    }
}

/// Persist oversized result to file and return reference
async fn persist_oversized_result(
    tool_use_id: &str,
    content: &ContentBlock,
    config: &OversizedResultConfig,
) -> Result<ContentBlock, ToolError> {
    // Create persist directory if needed
    tokio::fs::create_dir_all(&config.persist_dir).await?;

    // Write full content to file
    let filename = format!("{tool_use_id}.txt");
    let filepath = config.persist_dir.join(&filename);
    let full_content = content.as_text();
    tokio::fs::write(&filepath, full_content).await?;

    // Truncate based on strategy
    let truncated = match config.truncation_strategy {
        TruncationStrategy::KeepEnd => {
            let skip = full_content.len().saturating_sub(config.max_chars as usize);
            format!(
                "[Output truncated. Full content saved to: {}]\n\n...{}",
                filepath.display(),
                &full_content[skip..]
            )
        }
        TruncationStrategy::KeepStart => {
            format!(
                "{}...\n\n[Output truncated. Full content saved to: {}]",
                &full_content[..config.max_chars as usize],
                filepath.display()
            )
        }
        TruncationStrategy::KeepBoth => {
            let half = (config.max_chars / 2) as usize;
            let skip = full_content.len().saturating_sub(half);
            format!(
                "{}...\n\n[{} chars truncated]\n\n...{}\n\n[Full content saved to: {}]",
                &full_content[..half],
                full_content.len() - config.max_chars as usize,
                &full_content[skip..],
                filepath.display()
            )
        }
    };

    Ok(ContentBlock::tool_result(tool_use_id, truncated, false))
}
```

## Settings JSON Validation (Claude Code v2.1.7 Aligned)

When editing settings files (e.g., `~/.claude/settings.json`), the Edit tool performs JSON schema validation to prevent configuration corruption.

### Validation Flow

```
Edit Tool Called (settings file)
    │
    ▼
1. Check if target is a settings file
   └── Match against known settings paths
    │
    ▼
2. Load JSON schema for file type
   ├── settings.json → SettingsSchema
   ├── hooks.json → HooksSchema
   └── permissions.json → PermissionsSchema
    │
    ▼
3. Parse current file as JSON
   └── If parse fails → Error (file already corrupted)
    │
    ▼
4. Apply edit to get new content
    │
    ▼
5. Parse new content as JSON
   └── If parse fails → Error "Edit would create invalid JSON"
    │
    ▼
6. Validate against schema
   └── If validation fails → Error with schema violation details
    │
    ▼
7. Execute edit
```

### Implementation

```rust
/// Settings file paths that require JSON validation
pub const SETTINGS_FILES: &[&str] = &[
    "~/.claude/settings.json",
    "~/.claude/settings.local.json",
    ".claude/settings.json",
    ".claude/settings.local.json",
    "~/.claude/permissions.json",
    "~/.claude/hooks.json",
];

/// Check if path is a settings file requiring validation
pub fn is_settings_file(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    SETTINGS_FILES.iter().any(|p| {
        let expanded = expand_tilde(p);
        path_str.ends_with(&expanded) || path_str == expanded
    })
}

/// Validate settings file edit
pub fn validate_settings_edit(
    path: &Path,
    old_content: &str,
    new_content: &str,
) -> Result<(), SettingsValidationError> {
    // 1. Parse old content (should be valid)
    let _old_json: serde_json::Value = serde_json::from_str(old_content)
        .map_err(|e| SettingsValidationError::CurrentFileInvalid {
            path: path.to_path_buf(),
            error: e.to_string(),
        })?;

    // 2. Parse new content
    let new_json: serde_json::Value = serde_json::from_str(new_content)
        .map_err(|e| SettingsValidationError::NewContentInvalidJson {
            path: path.to_path_buf(),
            error: e.to_string(),
        })?;

    // 3. Get schema for file type
    let schema = get_schema_for_settings_file(path)?;

    // 4. Validate against schema
    validate_against_schema(&new_json, &schema)?;

    Ok(())
}

#[derive(Debug)]
pub enum SettingsValidationError {
    /// Current file content is not valid JSON
    CurrentFileInvalid { path: PathBuf, error: String },
    /// New content would not be valid JSON
    NewContentInvalidJson { path: PathBuf, error: String },
    /// New content violates schema
    SchemaViolation { path: PathBuf, violations: Vec<String> },
    /// Unknown settings file type
    UnknownSettingsFile { path: PathBuf },
}

impl std::fmt::Display for SettingsValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CurrentFileInvalid { path, error } => {
                write!(f, "Current settings file is invalid JSON: {} - {}", path.display(), error)
            }
            Self::NewContentInvalidJson { path, error } => {
                write!(f, "Edit would create invalid JSON in {}: {}", path.display(), error)
            }
            Self::SchemaViolation { path, violations } => {
                write!(f, "Edit violates settings schema for {}: {}", path.display(), violations.join(", "))
            }
            Self::UnknownSettingsFile { path } => {
                write!(f, "Unknown settings file type: {}", path.display())
            }
        }
    }
}

impl std::error::Error for SettingsValidationError {}
```

### Settings Schema (Partial)

```rust
/// Get JSON schema for settings file
fn get_schema_for_settings_file(path: &Path) -> Result<Value, SettingsValidationError> {
    let filename = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    match filename {
        "settings.json" | "settings.local.json" => Ok(json!({
            "type": "object",
            "properties": {
                "model": { "type": "string" },
                "apiKey": { "type": "string" },
                "customApiUrl": { "type": "string" },
                "theme": { "type": "string", "enum": ["light", "dark", "system"] },
                "permissions": {
                    "type": "object",
                    "properties": {
                        "allowed_tools": { "type": "array", "items": { "type": "string" } },
                        "disabled_tools": { "type": "array", "items": { "type": "string" } }
                    }
                },
                "hooks": {
                    "type": "array",
                    "items": { "$ref": "#/definitions/hook" }
                }
            },
            "definitions": {
                "hook": {
                    "type": "object",
                    "required": ["event"],
                    "properties": {
                        "event": { "type": "string" },
                        "command": { "type": "string" },
                        "timeout_sec": { "type": "integer", "minimum": 1 }
                    }
                }
            }
        })),
        "permissions.json" => Ok(json!({
            "type": "object",
            "properties": {
                "allowed_tools": { "type": "array", "items": { "type": "string" } },
                "disabled_tools": { "type": "array", "items": { "type": "string" } },
                "allowed_domains": { "type": "array", "items": { "type": "string" } },
                "blocked_domains": { "type": "array", "items": { "type": "string" } }
            }
        })),
        "hooks.json" => Ok(json!({
            "type": "array",
            "items": {
                "type": "object",
                "required": ["event"],
                "properties": {
                    "event": { "type": "string" },
                    "command": { "type": "string" },
                    "timeout_sec": { "type": "integer" },
                    "tool_matcher": { "type": "string" }
                }
            }
        })),
        _ => Err(SettingsValidationError::UnknownSettingsFile {
            path: path.to_path_buf(),
        }),
    }
}
```

### Integration with Edit Tool

```rust
impl EditTool {
    async fn validate_input(
        &self,
        input: &Value,
        ctx: &ToolContext,
    ) -> ValidationResult {
        let path = input.get("file_path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from);

        if let Some(path) = &path {
            // Check read-before-edit
            let state = ctx.read_file_state.read().await;
            if let Err(e) = state.can_edit(path) {
                return ValidationResult::Invalid {
                    errors: vec![ValidationError::new(e.to_string())],
                };
            }

            // For settings files, validate JSON schema
            if is_settings_file(path) {
                let old_string = input.get("old_string")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let new_string = input.get("new_string")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if let Some(info) = state.files.get(path) {
                    // Simulate the edit
                    let new_content = info.content.replace(old_string, new_string);

                    if let Err(e) = validate_settings_edit(path, &info.content, &new_content) {
                        return ValidationResult::Invalid {
                            errors: vec![ValidationError::new(e.to_string())],
                        };
                    }
                }
            }
        }

        ValidationResult::Valid
    }
}
```

---

## Summary: Key Patterns

| Pattern | Description |
|---------|-------------|
| **5-Stage Pipeline** | enabled → validation → hooks → permissions → execution (Claude Code v2.1.7 aligned) |
| **Concurrency Safety** | Input-dependent safety check via `is_concurrency_safe(input)` |
| **Permission Flow** | Check context, then tool-specific, then prompt user |
| **Hook Integration** | PreToolUse, PostToolUse, PostToolUseFailure hooks |
| **MCP Integration** | Qualified names `mcp__server__tool`, wrapper implements Tool trait |
| **Oversized Results** | Persist to file, return reference |
| **Context Modifiers** | Tools can request state changes via modifiers |
| **Read-Before-Edit** | Track file reads, enforce read before edit with content validation fallback |
| **File Edit Watch** | Three-layer system: abstraction, readFileState, watcher |
| **Settings Validation** | JSON schema validation for settings file edits |
| **Sensitive Paths** | Auto-detection and protection of credential/secret files |
| **Tool Rendering** | Optional custom rendering for tool use/result display |

## Alignment with Claude Code v2.1.7

This documentation aligns tool definitions with Claude Code v2.1.7:

| Feature | Status | Description |
|---------|--------|-------------|
| `TodoWrite` | ✅ Aligned | Replaces `TaskCreate/TaskUpdate/TaskList/TaskGet` with atomic list replacement |
| `KillShell` | ✅ Aligned | Renamed from `TaskStop` for exact Claude Code alignment |
| `LSP` | ✅ Aligned | Renamed from `Lsp` for consistent uppercase naming |
| Full descriptions | ✅ Aligned | All tools include complete system prompt descriptions |
| Max result sizes | ✅ Aligned | Documented for Read (100k), Grep (20k), Glob (30k), Bash (30k) |
| Timeouts | ✅ Aligned | Bash default 2min, max 10min documented |
| **File Edit Watch** | ✅ Aligned | Three-layer architecture with readFileState tracking |
| **Content Validation** | ✅ Aligned | v2.1.7 enhancement: content fallback for false-positive mtime changes |
| **PermissionBehavior** | ✅ Aligned | Allow/Ask/Deny behavior types with risk detection |
| **Sensitive Paths** | ✅ Aligned | Auto-protection for .env, credentials, SSH keys, etc. |
| **Settings Validation** | ✅ Aligned | JSON schema validation before editing settings files |

### New in This Update

1. **Enhanced File Edit Watch System**
   - Three-layer architecture (abstraction, readFileState, watcher)
   - Content validation fallback for complete file reads
   - File watcher integration for external change detection
   - `changed_files` attachment generation

2. **PermissionBehavior Types**
   - `Allow`, `Ask`, `Deny` behaviors
   - Security risk detection and classification
   - Sensitive path pattern matching

3. **Settings JSON Validation**
   - Pre-edit validation against JSON schema
   - Protection against configuration corruption
   - Schema-based validation for settings, hooks, permissions files
