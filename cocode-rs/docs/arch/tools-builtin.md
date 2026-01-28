# Built-in Tools Implementation

> Architecture reference: [tools.md](./tools.md)

This document details the implementation of all built-in tools. For the tool system architecture, 5-stage pipeline, and concurrency model, see [tools.md](./tools.md).

---

## Read Tool

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

## Edit Tool

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

**LSP Integration (v2.1.7):** After a successful edit, the Edit tool notifies the LSP server about file changes. This enables IDE features like diagnostics refresh and symbol updates without requiring a file save.

```rust
// Post-edit LSP notification (if LSP server is connected)
if let Some(lsp) = ctx.lsp_client() {
    lsp.notify_did_change(&path, &new_content).await;
}
```

## Write Tool

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

## Glob Tool

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

## Grep Tool

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

## Bash Tool

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

### Shell Command Security Analysis

The Bash tool uses the `shell-parser` crate (`cocode-rs/utils/shell-parser/`) for comprehensive security analysis before executing commands.

**Security Features:**
- Tree-sitter + tokenizer dual parsing strategy
- 14 risk types with 2-phase classification (Allow/Ask)
- Pipe segment extraction and analysis
- Redirection parsing (9 kinds)
- Safe command whitelist extraction

**Risk Categories (14 total):**

| Phase | Risk Type | Severity | Description |
|-------|-----------|----------|-------------|
| Allow | JqDanger | High | `jq` with `system()` calls |
| Allow | ObfuscatedFlags | Medium | ANSI-C quoting `$'...'` |
| Allow | ShellMetacharacters | Medium | Metacharacters in `find -exec` |
| Allow | DangerousVariables | Medium | Variables piped (`$VAR \|`) |
| Allow | NewlineInjection | High | Literal `\n` followed by commands |
| Allow | IfsInjection | High | IFS environment manipulation |
| Allow | ProcEnvironAccess | High | `/proc/*/environ` access |
| Ask | DangerousSubstitution | Medium | Command/process substitution |
| Ask | MalformedTokens | Low | Syntax errors, unbalanced brackets |
| Ask | SensitiveRedirect | High | Redirects to sensitive paths |
| Ask | NetworkExfiltration | Critical | curl/wget/ssh with data flags |
| Ask | PrivilegeEscalation | Critical | sudo/su/setuid operations |
| Ask | FileSystemTampering | High | rm -rf, dd, mkfs operations |
| Ask | CodeExecution | Critical | eval, exec, shell -c |

**Usage in Bash Tool:**
```rust
use cocode_shell_parser::{ShellParser, SecurityAnalysis, RiskPhase};

async fn check_permissions(&self, input: &Value, ctx: &ToolContext) -> PermissionResult {
    let command = input.get("command").and_then(|v| v.as_str()).unwrap_or("");

    let (parsed, analysis) = ShellParser::parse_and_analyze(command);

    // Check Allow phase risks
    let allow_risks: Vec<_> = analysis.risks.iter()
        .filter(|r| r.phase == RiskPhase::Allow)
        .collect();

    if !allow_risks.is_empty() {
        // Auto-reject if any Allow phase risks detected
        return PermissionResult::Denied {
            reason: format!("Security risk: {}", allow_risks[0].message)
        };
    }

    // Check Ask phase risks
    let ask_risks: Vec<_> = analysis.risks.iter()
        .filter(|r| r.phase == RiskPhase::Ask)
        .collect();

    if !ask_risks.is_empty() {
        return PermissionResult::NeedsApproval {
            request: ApprovalRequest {
                tool: "Bash".to_string(),
                action: command.to_string(),
                risks: ask_risks.iter().map(|r| r.message.clone()).collect(),
            },
        };
    }

    PermissionResult::Allowed
}
```

### Bash Output Path Extraction (Optional Optimization)

When a fast model is configured, the Bash tool can automatically extract file paths from command output and pre-read them for faster subsequent access. This mirrors Claude Code v2.1.7's Haiku-based optimization.

**Trigger:** Fast model is configured in `config.models.fast`

**Flow:**
1. Bash command executes and returns output
2. Fast model extracts file paths from output (truncated to 2000 chars for efficiency)
3. Valid existing paths are pre-read into `readFileState`
4. Files are available in context for subsequent operations

**Configuration:**
```toml
# ~/.cocode/config.toml
[models]
fast = { provider = "anthropic", model = "claude-haiku-4-5-20250514" }
```

**Implementation:**
```rust
impl BashTool {
    /// Extract file paths from bash output for pre-reading
    async fn extract_file_paths(&self, output: &str, ctx: &ToolContext) -> Vec<PathBuf> {
        // Only if fast model is configured
        if !ctx.config().models.has_fast_model() {
            return vec![];
        }

        let fast_model = ctx.config().models.get_fast_model();

        // Use fast model to extract paths (truncate to 2000 chars)
        let response = ctx.llm_client()
            .with_model(&fast_model.provider, &fast_model.model)
            .generate(&[Message::user(format!(
                "Extract all file paths from this output. Return only the paths, one per line:\n\n{}",
                output.chars().take(2000).collect::<String>()
            ))])
            .await?;

        // Parse paths and filter to existing files
        parse_file_paths(&response.text)
            .filter(|p| p.exists())
            .collect()
    }
}
```

**Example:**
```bash
$ git status
modified:   src/main.rs
modified:   src/lib.rs
new file:   src/utils.rs
```
-> `src/main.rs`, `src/lib.rs`, and `src/utils.rs` are automatically pre-read into context

**Benefits:**
- Reduces round-trips when working with git/build outputs
- Fast model (Haiku-class) has minimal cost/latency
- Graceful degradation: if fast model not configured, no path extraction

## Task Tool (Subagent Spawning)

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

## TodoWrite Tool (Progress Tracking)

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

## EnterPlanMode Tool

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

## ExitPlanMode Tool

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

## KillShell Tool (Background Task Termination)

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

## TaskOutput Tool (Background Task Output Retrieval)

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
