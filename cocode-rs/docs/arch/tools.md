# Tool System Architecture

## Overview

The tool system provides an abstraction for executable functions that the LLM can call.

**Key Features:**
- **5-Stage Execution Pipeline**: enabled → permissions → validation → execution → result mapping
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

    // ============ 5-Stage Execution Pipeline ============

    /// Stage 1: Is this tool enabled in current context?
    fn is_enabled(&self, ctx: &ToolContext) -> bool {
        true
    }

    /// Stage 2: Check permissions (may prompt user)
    async fn check_permissions(
        &self,
        input: &Value,
        ctx: &ToolContext,
    ) -> PermissionResult {
        PermissionResult::Allowed
    }

    /// Stage 3: Validate input (schema + custom validation)
    async fn validate_input(
        &self,
        input: &Value,
        ctx: &ToolContext,
    ) -> ValidationResult {
        // Default: schema validation only
        validate_against_schema(input, &self.input_schema())
    }

    /// Stage 4: Execute the tool
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

### Read-Before-Edit Enforcement

Track file read state to ensure Edit tool cannot modify files that haven't been read:

```rust
/// Read file state for tracking file reads
#[derive(Debug, Clone, Default)]
pub struct ReadFileState {
    /// Files that have been read
    pub files: HashMap<PathBuf, FileReadInfo>,
}

#[derive(Debug, Clone)]
pub struct FileReadInfo {
    /// File content when read
    pub content: String,
    /// When the file was read
    pub timestamp: SystemTime,
    /// Offset used when reading (if partial)
    pub offset: Option<i32>,
    /// Limit used when reading (if partial)
    pub limit: Option<i32>,
    /// File modification time when read
    pub file_mtime: SystemTime,
}

impl ReadFileState {
    /// Check if file can be edited (has been read and not modified since)
    pub fn can_edit(&self, path: &Path) -> Result<(), EditError> {
        let info = self.files.get(path)
            .ok_or_else(|| EditError::NotRead {
                path: path.to_path_buf(),
            })?;

        // Check if file was modified since we read it
        let current_mtime = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok();

        if let Some(mtime) = current_mtime {
            if mtime > info.file_mtime {
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
    pub fn record_read(&mut self, path: &Path, content: &str, offset: Option<i32>, limit: Option<i32>) {
        let file_mtime = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| SystemTime::now());

        self.files.insert(path.to_path_buf(), FileReadInfo {
            content: content.to_string(),
            timestamp: SystemTime::now(),
            offset,
            limit,
            file_mtime,
        });
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
```

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

## 5-Stage Tool Execution Pipeline

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

    // Stage 2: Check permissions
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

    // Stage 3: Validate input
    let validation_result = tool.validate_input(&input, ctx).await;
    if let ValidationResult::Invalid { errors } = validation_result {
        return ToolExecutionResult::error(
            tool_use_id,
            format!("Validation failed: {:?}", errors)
        );
    }

    // Pre-tool hooks
    let hook_result = hooks.execute(HookEventType::PreToolUse, HookContext {
        tool_name: tool.name(),
        input: &input,
        ctx,
    }).await;

    if let HookResult::Reject { reason } = hook_result {
        return ToolExecutionResult::error(tool_use_id, reason);
    }

    // Stage 4: Execute
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

            // Stage 5: Map to content block
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

## Built-in Tools

### Read Tool

**Full Description (System Prompt):**
```
Reads a file from the local filesystem. You can access any file directly by using this tool.
Assume this tool is able to read all files on the machine. If the User provides a path to a file assume that path is valid. It is okay to read a file that does not exist; an error will be returned.

Usage:
- The file_path parameter must be an absolute path, not a relative path
- By default, it reads up to 2000 lines starting from the beginning of the file
- You can optionally specify a line offset and limit (especially handy for long files), but it's recommended to read the whole file by not providing these parameters
- Any lines longer than 2000 characters will be truncated
- Results are returned using cat -n format, with line numbers starting at 1
- This tool allows Claude Code to read images (eg PNG, JPG, etc). When reading an image file the contents are presented visually as Claude Code is a multimodal LLM.
- This tool can read PDF files (.pdf). PDFs are processed page by page, extracting both text and visual content for analysis.
- This tool can read Jupyter notebooks (.ipynb files) and returns all cells with their outputs, combining code, text, and visualizations.
- This tool can only read files, not directories. To read a directory, use an ls command via the Bash tool.
- You can call multiple tools in a single response. It is always better to speculatively read multiple potentially useful files in parallel.
- You will regularly be asked to read screenshots. If the user provides a path to a screenshot, ALWAYS use this tool to view the file at the path. This tool will work with all temporary file paths.
- If you read a file that exists but has empty contents you will receive a system reminder warning in place of file contents.
```

**Max Result Size:** 100,000 characters

```rust
pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str { "Read" }

    fn max_result_size_chars(&self) -> i32 { 100000 }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::full(
            "Read",
            "Reads a file from the local filesystem.",
            json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The absolute path to the file to read"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line number to start reading from"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Number of lines to read"
                    }
                },
                "required": ["file_path"]
            })
        )
    }

    fn is_read_only(&self) -> bool { true }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true  // Always safe - read-only
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolContext,
        _tool_use_id: &str,
        _metadata: ToolMetadata,
        _progress: Option<ProgressCallback>,
    ) -> Result<ToolOutput, ToolError> {
        let args: ReadArgs = serde_json::from_value(input)?;

        // Read file
        let content = tokio::fs::read_to_string(&args.file_path).await
            .map_err(|e| ToolError::io(&args.file_path, e))?;

        // Apply offset/limit
        let lines: Vec<&str> = content.lines().collect();
        let start = args.offset.unwrap_or(0) as usize;
        let end = args.limit.map(|l| start + l as usize).unwrap_or(lines.len());
        let selected: String = lines[start..end.min(lines.len())].join("\n");

        // Update read file state
        {
            let mut state = ctx.read_file_state.write().await;
            state.files.insert(args.file_path.clone(), FileReadInfo {
                content: content.clone(),
                timestamp: std::time::SystemTime::now(),
            });
        }

        Ok(ToolOutput::success(selected)
            .with_modifier(ContextModifier::FileRead {
                path: args.file_path,
                content,
            }))
    }
}
```

### Edit Tool

**Full Description (System Prompt):**
```
Performs exact string replacements in files.

Usage:
- You must use your `Read` tool at least once in the conversation before editing. This tool will error if you attempt an edit without reading the file.
- When editing text from Read tool output, ensure you preserve the exact indentation (tabs/spaces) as it appears AFTER the line number prefix. The line number prefix format is: spaces + line number + tab. Everything after that tab is the actual file content to match. Never include any part of the line number prefix in the old_string or new_string.
- ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.
- Only use emojis if the user explicitly requests it. Avoid adding emojis to files unless asked.
- The edit will FAIL if `old_string` is not unique in the file. Either provide a larger string with more surrounding context to make it unique or use `replace_all` to change every instance of `old_string`.
- Use `replace_all` for replacing and renaming strings across the file. This parameter is useful if you want to rename a variable for instance.
```

```rust
pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str { "Edit" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::full(
            "Edit",
            "Performs exact string replacements in files.",
            json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The absolute path to the file to modify"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "The text to replace"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "The text to replace it with (must be different from old_string)"
                    },
                    "replace_all": {
                        "type": "boolean",
                        "default": false,
                        "description": "Replace all occurrences of old_string (default false)"
                    }
                },
                "required": ["file_path", "old_string", "new_string"]
            })
        )
    }

    fn is_read_only(&self) -> bool { false }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false  // Modifies files
    }

    async fn validate_input(
        &self,
        input: &Value,
        ctx: &ToolContext,
    ) -> ValidationResult {
        let path = input.get("file_path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from);

        if let Some(path) = path {
            // Check that file was read first
            let state = ctx.read_file_state.read().await;
            if let Err(e) = state.can_edit(&path) {
                return ValidationResult::Invalid {
                    errors: vec![ValidationError::new(format!(
                        "Must read file before editing: {e:?}"
                    ))],
                };
            }
        }

        ValidationResult::Valid
    }
}
```

### Write Tool

**Full Description (System Prompt):**
```
Writes a file to the local filesystem.

Usage:
- This tool will overwrite the existing file if there is one at the provided path.
- If this is an existing file, you MUST use the Read tool first to read the file's contents. This tool will fail if you did not read the file first.
- ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.
- NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.
- Only use emojis if the user explicitly requests it. Avoid writing emojis to files unless asked.
```

```rust
pub struct WriteTool;

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str { "Write" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::full(
            "Write",
            "Writes a file to the local filesystem.",
            json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The absolute path to the file to write (must be absolute, not relative)"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file"
                    }
                },
                "required": ["file_path", "content"]
            })
        )
    }

    fn is_read_only(&self) -> bool { false }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false  // Modifies files
    }
}
```

### Glob Tool

**Full Description (System Prompt):**
```
Fast file pattern matching tool that works with any codebase size
- Supports glob patterns like "**/*.js" or "src/**/*.ts"
- Returns matching file paths sorted by modification time
- Use this tool when you need to find files by name patterns
- When you are doing an open ended search that may require multiple rounds of globbing and grepping, use the Agent tool instead
- You can call multiple tools in a single response. It is always better to speculatively perform multiple searches in parallel if they are potentially useful.
```

**Max Result Size:** 30,000 characters

```rust
pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str { "Glob" }

    fn max_result_size_chars(&self) -> i32 { 30000 }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::full(
            "Glob",
            "Fast file pattern matching tool that works with any codebase size.",
            json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The glob pattern to match files against"
                    },
                    "path": {
                        "type": "string",
                        "description": "The directory to search in. Defaults to current working directory."
                    }
                },
                "required": ["pattern"]
            })
        )
    }

    fn is_read_only(&self) -> bool { true }

    fn is_concurrency_safe(&self, _input: &Value) -> bool { true }
}
```

### Grep Tool

**Full Description (System Prompt):**
```
A powerful search tool built on ripgrep

Usage:
- ALWAYS use Grep for search tasks. NEVER invoke `grep` or `rg` as a Bash command. The Grep tool has been optimized for correct permissions and access.
- Supports full regex syntax (e.g., "log.*Error", "function\\s+\\w+")
- Filter files with glob parameter (e.g., "*.js", "**/*.tsx") or type parameter (e.g., "js", "py", "rust")
- Output modes: "content" shows matching lines, "files_with_matches" shows only file paths (default), "count" shows match counts
- Use Task tool for open-ended searches requiring multiple rounds
- Pattern syntax: Uses ripgrep (not grep) - literal braces need escaping (use `interface\\{\\}` to find `interface{}` in Go code)
- Multiline matching: By default patterns match within single lines only. For cross-line patterns like `struct \\{[\\s\\S]*?field`, use `multiline: true`
```

**Max Result Size:** 20,000 characters

```rust
pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str { "Grep" }

    fn max_result_size_chars(&self) -> i32 { 20000 }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::full(
            "Grep",
            "A powerful search tool built on ripgrep.",
            json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The regular expression pattern to search for in file contents"
                    },
                    "path": {
                        "type": "string",
                        "description": "File or directory to search in. Defaults to current working directory."
                    },
                    "glob": {
                        "type": "string",
                        "description": "Glob pattern to filter files (e.g. \"*.js\", \"*.{ts,tsx}\")"
                    },
                    "type": {
                        "type": "string",
                        "description": "File type to search (e.g., js, py, rust)"
                    },
                    "output_mode": {
                        "type": "string",
                        "enum": ["content", "files_with_matches", "count"],
                        "description": "Output mode. Defaults to files_with_matches."
                    },
                    "multiline": {
                        "type": "boolean",
                        "description": "Enable multiline mode where . matches newlines. Default: false."
                    },
                    "-i": { "type": "boolean", "description": "Case insensitive search" },
                    "-n": { "type": "boolean", "description": "Show line numbers in output" },
                    "-A": { "type": "integer", "description": "Lines to show after each match" },
                    "-B": { "type": "integer", "description": "Lines to show before each match" },
                    "-C": { "type": "integer", "description": "Lines to show before and after each match" },
                    "head_limit": { "type": "integer", "description": "Limit output to first N entries" },
                    "offset": { "type": "integer", "description": "Skip first N entries" }
                },
                "required": ["pattern"]
            })
        )
    }

    fn is_read_only(&self) -> bool { true }

    fn is_concurrency_safe(&self, _input: &Value) -> bool { true }
}
```

### Bash Tool

**Full Description (System Prompt):**
```
Executes a given bash command with optional timeout. Working directory persists between commands; shell state (everything else) does not. The shell environment is initialized from the user's profile (bash or zsh).

IMPORTANT: This tool is for terminal operations like git, npm, docker, etc. DO NOT use it for file operations (reading, writing, editing, searching, finding files) - use the specialized tools for this instead.

Before executing the command, please follow these steps:

1. Directory Verification:
   - If the command will create new directories or files, first use `ls` to verify the parent directory exists and is the correct location
   - For example, before running "mkdir foo/bar", first use `ls foo` to check that "foo" exists and is the intended parent directory

2. Command Execution:
   - Always quote file paths that contain spaces with double quotes (e.g., cd "path with spaces/file.txt")
   - After ensuring proper quoting, execute the command.
   - Capture the output of the command.

Usage notes:
  - The command argument is required.
  - You can specify an optional timeout in milliseconds (up to 600000ms / 10 minutes). If not specified, commands will timeout after 120000ms (2 minutes).
  - It is very helpful if you write a clear, concise description of what this command does.
  - If the output exceeds 30000 characters, output will be truncated before being returned to you.
  - You can use the `run_in_background` parameter to run the command in the background. Only use this if you don't need the result immediately.
  - Avoid using Bash with the `find`, `grep`, `cat`, `head`, `tail`, `sed`, `awk`, or `echo` commands, unless explicitly instructed. Instead, always prefer using the dedicated tools (Glob, Grep, Read, Edit, Write).
  - When issuing multiple commands: If the commands are independent and can run in parallel, make multiple Bash tool calls in a single message.
  - Try to maintain your current working directory throughout the session by using absolute paths and avoiding usage of `cd`.

# Committing changes with git

Only create commits when requested by the user. If unclear, ask first. When the user asks you to create a new git commit, follow these steps carefully:

Git Safety Protocol:
- NEVER update the git config
- NEVER run destructive git commands (push --force, reset --hard, checkout ., restore ., clean -f, branch -D) unless explicitly requested
- NEVER skip hooks (--no-verify, --no-gpg-sign, etc) unless explicitly requested
- NEVER run force push to main/master, warn the user if they request it
- CRITICAL: Always create NEW commits rather than amending, unless explicitly requested
- When staging files, prefer adding specific files by name rather than using "git add -A" or "git add ."
- NEVER commit changes unless the user explicitly asks you to

1. Run git status and git diff to understand changes
2. Run git log to see recent commit message style
3. Analyze changes and draft a commit message
4. Add relevant files and create the commit with Co-Authored-By: Claude <noreply@anthropic.com>
5. If the commit fails due to pre-commit hook: fix the issue and create a NEW commit

# Creating pull requests

Use the gh command via the Bash tool for ALL GitHub-related tasks. When creating a pull request:
1. Run git status, git diff, and git log to understand the changes
2. Analyze all changes that will be included (ALL commits, not just the latest)
3. Create PR using gh pr create with ## Summary and ## Test plan sections
```

**Max Result Size:** 30,000 characters

**Default Timeout:** 120,000ms (2 minutes)

**Max Timeout:** 600,000ms (10 minutes)

```rust
pub struct BashTool {
    executor: ShellExecutor,
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str { "Bash" }

    fn max_result_size_chars(&self) -> i32 { 30000 }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::full(
            "Bash",
            "Executes a given bash command with optional timeout.",
            json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The command to execute" },
                    "description": { "type": "string", "description": "Clear, concise description of what this command does" },
                    "timeout": { "type": "integer", "description": "Optional timeout in milliseconds (max 600000)" },
                    "run_in_background": { "type": "boolean", "description": "Set to true to run this command in the background" },
                    "dangerously_disable_sandbox": {
                        "type": "boolean",
                        "description": "Set to true to dangerously override sandbox mode and run commands without sandboxing"
                    },
                    "_simulated_sed_edit": {
                        "type": "object",
                        "description": "Internal: pre-computed sed edit result from preview",
                        "properties": {
                            "file_path": { "type": "string" },
                            "new_content": { "type": "string" }
                        },
                        "required": ["file_path", "new_content"]
                    }
                },
                "required": ["command"]
            })
        )
    }

    fn is_concurrency_safe(&self, input: &Value) -> bool {
        // Check if read-only command
        if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
            is_read_only_command(cmd)
        } else {
            false
        }
    }

    async fn check_permissions(
        &self,
        input: &Value,
        ctx: &ToolContext,
    ) -> PermissionResult {
        let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("");

        // Check if command matches any granted patterns
        if ctx.permission_context.is_allowed("Bash", cmd) {
            return PermissionResult::Allowed;
        }

        // Request approval
        PermissionResult::NeedsApproval {
            request: ApprovalRequest {
                tool: "Bash".to_string(),
                action: cmd.to_string(),
                description: input.get("description")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            },
        }
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolContext,
        _tool_use_id: &str,
        _metadata: ToolMetadata,
        _progress: Option<ProgressCallback>,
    ) -> Result<ToolOutput, ToolError> {
        let args: BashArgs = serde_json::from_value(input)?;

        if args.run_in_background {
            // Spawn background process
            let task_id = self.executor.spawn_background(&args.command, &ctx.cwd).await?;
            return Ok(ToolOutput::success(format!("Background task started: {task_id}")));
        }

        // Execute command
        let result = self.executor.execute(
            &args.command,
            &ctx.cwd,
            args.timeout.map(Duration::from_millis),
            ctx.cancel.clone(),
        ).await?;

        // Format output
        let output = format_command_output(&result);

        if result.exit_code != 0 {
            Ok(ToolOutput::error(output))
        } else {
            Ok(ToolOutput::success(output))
        }
    }
}

fn is_read_only_command(cmd: &str) -> bool {
    let read_only_patterns = [
        "ls", "cat", "head", "tail", "grep", "find", "which", "pwd",
        "echo", "date", "whoami", "hostname", "uname", "env", "printenv",
        "git status", "git log", "git diff", "git show", "git branch",
        "cargo check", "cargo test", "cargo build", "npm test", "npm run",
    ];

    let cmd_lower = cmd.to_lowercase().trim_start().to_string();
    read_only_patterns.iter().any(|&p| cmd_lower.starts_with(p))
}
```

### Task Tool (Subagent Spawning)

**Full Description (System Prompt):**
```
Launch a new agent to handle complex, multi-step tasks autonomously.

The Task tool launches specialized agents (subprocesses) that autonomously handle complex tasks. Each agent type has specific capabilities and tools available to it.

Available agent types and the tools they have access to:
- Bash: Command execution specialist for running bash commands. Use this for git operations, command execution, and other terminal tasks.
- general-purpose: General-purpose agent for researching complex questions, searching for code, and executing multi-step tasks.
- statusline-setup: Use this agent to configure the user's Claude Code status line setting.
- Explore: Fast agent specialized for exploring codebases. Use this when you need to quickly find files by patterns, search code for keywords, or answer questions about the codebase. Specify thoroughness level: "quick", "medium", or "very thorough".
- Plan: Software architect agent for designing implementation plans. Use this when you need to plan the implementation strategy for a task.
- claude-code-guide: Use this agent when the user asks questions about Claude Code, Claude Agent SDK, or Claude API.

When using the Task tool, you must specify a subagent_type parameter to select which agent type to use.

When NOT to use the Task tool:
- If you want to read a specific file path, use the Read or Glob tool instead
- If you are searching for a specific class definition like "class Foo", use the Glob tool instead
- If you are searching for code within a specific file or set of 2-3 files, use the Read tool instead

Usage notes:
- Always include a short description (3-5 words) summarizing what the agent will do
- Launch multiple agents concurrently whenever possible, to maximize performance
- When the agent is done, it will return a single message back to you
- You can optionally run agents in the background using the run_in_background parameter
- Agents can be resumed using the `resume` parameter by passing the agent ID from a previous invocation
- Provide clear, detailed prompts so the agent can work autonomously
- The agent's outputs should generally be trusted
- Clearly tell the agent whether you expect it to write code or just to do research
```

```rust
pub struct TaskTool {
    manager: Arc<SubagentManager>,
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str { "Task" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::full(
            "Task",
            "Launch a new agent to handle complex, multi-step tasks autonomously.",
            json!({
                "type": "object",
                "properties": {
                    "description": { "type": "string" },
                    "prompt": { "type": "string" },
                    "subagent_type": { "type": "string" },
                    "model": { "type": "string" },
                    "resume": { "type": "string" },
                    "run_in_background": { "type": "boolean" },
                    "max_turns": { "type": "integer" },
                    "allowed_tools": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "required": ["description", "prompt", "subagent_type"]
            })
        )
    }

    fn is_read_only(&self) -> bool { true }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true  // Task spawning is safe (launches separate process)
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolContext,
        _tool_use_id: &str,
        _metadata: ToolMetadata,
        _progress: Option<ProgressCallback>,
    ) -> Result<ToolOutput, ToolError> {
        let args: TaskArgs = serde_json::from_value(input)?;

        let spawn_input = SpawnInput {
            description: args.description,
            prompt: args.prompt,
            subagent_type: args.subagent_type,
            model: args.model,
            resume: args.resume,
            run_in_background: args.run_in_background.unwrap_or(false),
            max_turns: args.max_turns,
            allowed_tools: args.allowed_tools,
        };

        let agent_id = self.manager.spawn(spawn_input, ctx).await?;

        if spawn_input.run_in_background {
            Ok(ToolOutput::success(format!("Background agent started: {agent_id}")))
        } else {
            // Wait for completion
            let result = self.manager.wait(&agent_id).await?;
            Ok(ToolOutput::success(result))
        }
    }
}
```

### TodoWrite Tool (Progress Tracking)

**Full Description (System Prompt):**
```
Use this tool to create a structured task list for your current coding session. This helps you track progress, organize complex tasks, and demonstrate thoroughness to the user.
It also helps the user understand the progress of the task and overall progress of their requests.

## When to Use This Tool

Use this tool proactively in these scenarios:

- Complex multi-step tasks - When a task requires 3 or more distinct steps or actions
- Non-trivial and complex tasks - Tasks that require careful planning or multiple operations
- Plan mode - When using plan mode, create a task list to track the work
- User explicitly requests todo list - When the user directly asks you to use the todo list
- User provides multiple tasks - When users provide a list of things to be done (numbered or comma-separated)
- After receiving new instructions - Immediately capture user requirements as tasks
- When you start working on a task - Mark it as in_progress BEFORE beginning work
- After completing a task - Mark it as completed and add any new follow-up tasks discovered during implementation

## When NOT to Use This Tool

Skip using this tool when:
- There is only a single, straightforward task
- The task is trivial and tracking it provides no organizational benefit
- The task can be completed in less than 3 trivial steps
- The task is purely conversational or informational

## Task Fields

- **content**: A brief, actionable title in imperative form (e.g., "Fix authentication bug in login flow")
- **status**: 'pending' | 'in_progress' | 'completed'
- **activeForm**: Present continuous form shown in spinner when task is in_progress (e.g., "Fixing authentication bug")

**IMPORTANT**: Exactly ONE task must be in_progress at a time. The content should be imperative ("Run tests") while activeForm should be present continuous ("Running tests").
```

**Key Design Decision:** Claude Code uses a single `TodoWrite` tool that atomically replaces the entire task list, rather than exposing split create/update/list/get todo tools. This simplifies the API at the cost of sending the full list on each update.

```rust
pub struct TodoWriteTool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    /// Brief, actionable title in imperative form
    pub content: String,
    /// Task status: pending, in_progress, or completed
    pub status: TodoStatus,
    /// Present continuous form shown in spinner when in_progress
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_form: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str { "TodoWrite" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::full(
            "TodoWrite",
            "Create or update a structured task list. Replaces entire list atomically.",
            json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "content": {
                                    "type": "string",
                                    "description": "Brief, actionable title in imperative form"
                                },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed"],
                                    "description": "Task status"
                                },
                                "activeForm": {
                                    "type": "string",
                                    "description": "Present continuous form for spinner display"
                                }
                            },
                            "required": ["content", "status"]
                        },
                        "description": "The complete list of todos (replaces existing list)"
                    }
                },
                "required": ["todos"]
            })
        )
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false  // Modifies state
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolContext,
        _tool_use_id: &str,
        _metadata: ToolMetadata,
        _progress: Option<ProgressCallback>,
    ) -> Result<ToolOutput, ToolError> {
        let args: TodoWriteArgs = serde_json::from_value(input)?;

        // Validate: exactly one task should be in_progress
        let in_progress_count = args.todos.iter()
            .filter(|t| t.status == TodoStatus::InProgress)
            .count();

        if in_progress_count > 1 {
            return Ok(ToolOutput::error(
                "Exactly one task should be in_progress at a time"
            ));
        }

        // Update the todo state (atomically replaces the list)
        ctx.set_todos(args.todos.clone());

        // Find the in_progress task for display
        let active_task = args.todos.iter()
            .find(|t| t.status == TodoStatus::InProgress);

        if let Some(task) = active_task {
            let display = task.active_form.as_ref()
                .unwrap_or(&task.content);
            Ok(ToolOutput::success(format!("Todo list updated. Active: {display}")))
        } else {
            Ok(ToolOutput::success("Todo list updated."))
        }
    }
}
```

### EnterPlanMode Tool

**Full Description (System Prompt):**
```
Use this tool proactively when you're about to start a non-trivial implementation task. Getting user sign-off on your approach before writing code prevents wasted effort and ensures alignment.
```

```rust
pub struct EnterPlanModeTool;

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn name(&self) -> &str { "EnterPlanMode" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::full(
            "EnterPlanMode",
            "Enter plan mode for structured planning workflow.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {},
            })
        )
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false  // Mode change
    }
}
```

### ExitPlanMode Tool

**Full Description (System Prompt):**
```
Use this tool when you are in plan mode and have finished writing your plan to the plan file and are ready for user approval.

## How This Tool Works
- You should have already written your plan to the plan file specified in the plan mode system message
- This tool does NOT take the plan content as a parameter - it will read the plan from the file you wrote
- This tool simply signals that you're done planning and ready for the user to review and approve

## When to Use This Tool
IMPORTANT: Only use this tool when the task requires planning the implementation steps of a task that requires writing code. For research tasks where you're gathering information, searching files, reading files or in general trying to understand the codebase - do NOT use this tool.
```

```rust
pub struct ExitPlanModeTool;

#[derive(Debug, Clone, Deserialize)]
pub struct ExitPlanModeInput {
    /// Prompt-based permissions needed to implement the plan.
    /// These describe categories of actions rather than specific commands.
    #[serde(default)]
    pub allowed_prompts: Option<Vec<AllowedPrompt>>,

    /// Whether to push the plan to a remote Claude.ai session
    #[serde(default)]
    pub push_to_remote: Option<bool>,

    /// Remote session ID if pushed to remote
    #[serde(default)]
    pub remote_session_id: Option<String>,

    /// Remote session title if pushed to remote
    #[serde(default)]
    pub remote_session_title: Option<String>,

    /// Remote session URL if pushed to remote
    #[serde(default)]
    pub remote_session_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowedPrompt {
    /// The tool this prompt applies to (currently only "Bash")
    pub tool: String,
    /// Semantic description of the action, e.g., "run tests", "install dependencies"
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExitPlanModeOutput {
    /// Whether awaiting enterprise leader approval
    #[serde(skip_serializing_if = "Option::is_none")]
    pub awaiting_leader_approval: Option<bool>,

    /// Request ID for enterprise approval workflow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str { "ExitPlanMode" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::full(
            "ExitPlanMode",
            "Exit plan mode with plan file ready for user approval.",
            json!({
                "type": "object",
                "additionalProperties": true,
                "properties": {
                    "allowedPrompts": {
                        "type": "array",
                        "description": "Prompt-based permissions needed to implement the plan. \
                                        These describe categories of actions rather than specific commands.",
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "tool": {
                                    "type": "string",
                                    "enum": ["Bash"],
                                    "description": "The tool this prompt applies to"
                                },
                                "prompt": {
                                    "type": "string",
                                    "description": "Semantic description of the action, e.g., 'run tests', 'install dependencies'"
                                }
                            },
                            "required": ["tool", "prompt"]
                        }
                    },
                    "pushToRemote": {
                        "type": "boolean",
                        "description": "Whether to push the plan to a remote Claude.ai session"
                    },
                    "remoteSessionId": {
                        "type": "string",
                        "description": "The remote session ID if pushed to remote"
                    },
                    "remoteSessionTitle": {
                        "type": "string",
                        "description": "The remote session title if pushed to remote"
                    },
                    "remoteSessionUrl": {
                        "type": "string",
                        "description": "The remote session URL if pushed to remote"
                    }
                }
            })
        )
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false  // Mode change
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolContext,
        _tool_use_id: &str,
        _metadata: ToolMetadata,
        _progress: Option<ProgressCallback>,
    ) -> Result<ToolOutput, ToolError> {
        let args: ExitPlanModeInput = serde_json::from_value(input)?;

        // Read plan file content
        let plan_path = get_plan_file_path(ctx.agent_id.as_deref());
        let plan_content = tokio::fs::read_to_string(&plan_path).await
            .map_err(|e| ToolError::io(&plan_path, e))?;

        // Store allowed prompts for post-approval permission granting
        if let Some(allowed) = &args.allowed_prompts {
            ctx.set_pending_allowed_prompts(allowed.clone());
        }

        // Check if enterprise approval is needed
        let output = if ctx.requires_leader_approval() {
            let request_id = ctx.submit_approval_request(&plan_content).await?;
            ExitPlanModeOutput {
                awaiting_leader_approval: Some(true),
                request_id: Some(request_id),
            }
        } else {
            ExitPlanModeOutput {
                awaiting_leader_approval: None,
                request_id: None,
            }
        };

        // Exit plan mode (will wait for user approval)
        ctx.exit_plan_mode();

        Ok(ToolOutput::success(serde_json::to_string(&output)?))
    }
}
```

**Key Features:**

1. **allowedPrompts**: Pre-declares semantic Bash permissions the plan needs. When the user approves the plan, these prompts are granted as permissions, avoiding repeated approval prompts during implementation. Examples:
   - `{ "tool": "Bash", "prompt": "run tests" }`
   - `{ "tool": "Bash", "prompt": "install dependencies" }`

2. **Enterprise Approval**: For organizations with leader approval workflows, the tool returns `awaitingLeaderApproval: true` and a `requestId` to track the approval status.

### KillShell Tool (Background Task Termination)

**Full Description (System Prompt):**
```
- Kills a running background bash shell by its ID
- Takes a shell_id parameter identifying the shell to kill
- Returns a success or failure status
- Use this tool when you need to terminate a long-running shell
- Shell IDs can be found using the /tasks command
```

```rust
pub struct KillShellTool;

#[async_trait]
impl Tool for KillShellTool {
    fn name(&self) -> &str { "KillShell" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::full(
            "KillShell",
            "Kills a running background bash shell by its ID.",
            json!({
                "type": "object",
                "properties": {
                    "shell_id": {
                        "type": "string",
                        "description": "The ID of the background shell to kill"
                    }
                },
                "required": ["shell_id"]
            })
        )
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true  // Stateless operation - safe for parallel execution
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolContext,
        _tool_use_id: &str,
        _metadata: ToolMetadata,
        _progress: Option<ProgressCallback>,
    ) -> Result<ToolOutput, ToolError> {
        let args: KillShellArgs = serde_json::from_value(input)?;
        ctx.background_tasks.kill(&args.shell_id).await?;
        Ok(ToolOutput::success(format!("Successfully killed shell: {}", args.shell_id)))
    }
}
```

### TaskOutput Tool (Background Task Output Retrieval)

**Full Description (System Prompt):**
```
- Retrieves output from a running or completed task (background shell, agent, or remote session)
- Takes a task_id parameter identifying the task
- Returns the task output along with status information
- Use block=true (default) to wait for task completion
- Use block=false for non-blocking check of current status
- Task IDs can be found using the /tasks command
- Works with all task types: background shells, async agents, and remote sessions
```

```rust
pub struct TaskOutputTool;

#[async_trait]
impl Tool for TaskOutputTool {
    fn name(&self) -> &str { "TaskOutput" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::full(
            "TaskOutput",
            "Retrieves output from a running or completed task.",
            json!({
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "The task ID to get output from"
                    },
                    "block": {
                        "type": "boolean",
                        "default": true,
                        "description": "Whether to wait for completion"
                    },
                    "timeout": {
                        "type": "number",
                        "default": 30000,
                        "minimum": 0,
                        "maximum": 600000,
                        "description": "Max wait time in ms"
                    }
                },
                "required": ["task_id", "block", "timeout"]
            })
        )
    }

    fn is_read_only(&self) -> bool { true }

    fn is_concurrency_safe(&self, _input: &Value) -> bool { true }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolContext,
        _tool_use_id: &str,
        _metadata: ToolMetadata,
        _progress: Option<ProgressCallback>,
    ) -> Result<ToolOutput, ToolError> {
        let args: TaskOutputArgs = serde_json::from_value(input)?;

        let output = ctx.background_tasks.get_output(
            &args.task_id,
            args.block,
            Duration::from_millis(args.timeout as u64),
        ).await?;

        Ok(ToolOutput::success(serde_json::json!({
            "retrieval_status": output.status,
            "task": {
                "output": output.content,
                "is_completed": output.is_completed,
            }
        })))
    }
}
```

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

The LSP tool provides Language Server Protocol operations for code intelligence:

```rust
pub struct LSPTool {
    manager: Arc<LspServerManager>,
}

#[async_trait]
impl Tool for LSPTool {
    fn name(&self) -> &str { "LSP" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::full(
            "LSP",
            "Perform LSP operations: go-to-definition, hover, find-references.",
            json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["goto_definition", "hover", "find_references"],
                        "description": "LSP operation to perform"
                    },
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file"
                    },
                    "line": {
                        "type": "integer",
                        "description": "Line number (1-indexed)"
                    },
                    "column": {
                        "type": "integer",
                        "description": "Column number (1-indexed)"
                    }
                },
                "required": ["operation", "file_path", "line", "column"]
            })
        )
    }

    fn is_read_only(&self) -> bool { true }

    fn is_concurrency_safe(&self, _input: &Value) -> bool { true }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolContext,
        _tool_use_id: &str,
        _metadata: ToolMetadata,
        _progress: Option<ProgressCallback>,
    ) -> Result<ToolOutput, ToolError> {
        let args: LspArgs = serde_json::from_value(input)?;

        let server = self.manager.get_server_for_file(&args.file_path).await?;

        match args.operation.as_str() {
            "goto_definition" => {
                let locations = server.goto_definition(
                    &args.file_path,
                    args.line,
                    args.column,
                ).await?;
                Ok(ToolOutput::success(format_locations(&locations)))
            }
            "hover" => {
                let hover = server.hover(
                    &args.file_path,
                    args.line,
                    args.column,
                ).await?;
                Ok(ToolOutput::success(hover.contents))
            }
            "find_references" => {
                let refs = server.find_references(
                    &args.file_path,
                    args.line,
                    args.column,
                ).await?;
                Ok(ToolOutput::success(format_references(&refs)))
            }
            _ => Ok(ToolOutput::error(format!("Unknown operation: {}", args.operation)))
        }
    }
}

/// LSP server manager
pub struct LspServerManager {
    servers: HashMap<String, Arc<LspServer>>,
    config: LspConfig,
}

impl LspServerManager {
    /// Get or start LSP server for a file
    pub async fn get_server_for_file(&self, path: &str) -> Result<Arc<LspServer>, LspError> {
        let language = detect_language(path)?;
        if let Some(server) = self.servers.get(&language) {
            return Ok(server.clone());
        }
        self.start_server(&language).await
    }
}
```

## Permission System

```rust
pub struct PermissionContext {
    /// Granted permissions by tool and pattern
    granted: HashMap<String, Vec<PermissionPattern>>,

    /// Permission mode
    mode: PermissionMode,
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

## Summary: Key Patterns

| Pattern | Description |
|---------|-------------|
| **5-Stage Pipeline** | enabled → permissions → validation → execution → result mapping |
| **Concurrency Safety** | Input-dependent safety check via `is_concurrency_safe(input)` |
| **Permission Flow** | Check context, then tool-specific, then prompt user |
| **Hook Integration** | PreToolUse, PostToolUse, PostToolUseFailure hooks |
| **MCP Integration** | Qualified names `mcp__server__tool`, wrapper implements Tool trait |
| **Oversized Results** | Persist to file, return reference |
| **Context Modifiers** | Tools can request state changes via modifiers |
| **Read-Before-Edit** | Track file reads, enforce read before edit |
| **Tool Rendering** | Optional custom rendering for tool use/result display |

## Alignment with Claude Code v2.1.7

This documentation aligns tool definitions with Claude Code v2.1.7:

| Change | Description |
|--------|-------------|
| `TodoWrite` | Replaces `TaskCreate/TaskUpdate/TaskList/TaskGet` with atomic list replacement |
| `KillShell` | Renamed from `TaskStop` for exact Claude Code alignment |
| `LSP` | Renamed from `Lsp` for consistent uppercase naming |
| Full descriptions | All tools include complete system prompt descriptions |
| Max result sizes | Documented for Read (100k), Grep (20k), Glob (30k), Bash (30k) |
| Timeouts | Bash default 2min, max 10min documented |
