# System Reminder Attachments

## Overview

System reminders (attachments) are contextual information injected into the conversation at strategic points. They provide the AI with important context about session state, user configuration, file changes, and other dynamic information.

Attachments are generated on each turn and converted to system messages with `isMeta: true` flag, ensuring they don't appear as user-visible content.

## Attachment Categories

System reminders are organized into three categories:

1. **User Prompt Attachments** - Triggered by user input (@mentions, etc.)
2. **Core Attachments** - Always checked, available to all agents (main + sub-agents)
3. **Main Agent Attachments** - Only available to the primary agent (not sub-agents)

---

## Implementation Architecture

### Attachment Generation Pipeline

```rust
/// Generate all attachments for the current turn
pub async fn generate_all_attachments(
    user_prompt: Option<&str>,
    context: &ToolContext,
    ide_context: &IdeContext,
    queued_commands: &[QueuedCommand],
    conversation_history: &[ConversationMessage],
) -> Vec<Attachment> {
    // 1 second timeout to prevent blocking
    let timeout = Duration::from_secs(1);

    // Create enhanced context with timeout
    let ctx = context.with_timeout(timeout);

    let is_main_agent = context.agent_id.is_none();

    // User Prompt Attachments (only if user provided input)
    let user_prompt_attachments = if user_prompt.is_some() {
        join_all(vec![
            generate_at_mentioned_files(user_prompt, &ctx),
            generate_mcp_resources(user_prompt, &ctx),
            generate_agent_mentions(user_prompt, &ctx),
        ]).await
    } else {
        vec![]
    };

    // Core Attachments (always checked)
    let core_attachments = join_all(vec![
        generate_changed_files(&ctx),
        generate_nested_memory(&ctx),
        generate_plan_mode(conversation_history, context),
        generate_plan_mode_exit(&ctx),
        generate_delegate_mode(&ctx),
        generate_delegate_mode_exit(&ctx),
        generate_todo_reminders(conversation_history, &ctx),
        generate_collab_notification(&ctx),
        generate_critical_system_reminder(&ctx),
    ]).await;

    // Main Agent Attachments (only for primary agent)
    let main_agent_attachments = if is_main_agent {
        join_all(vec![
            generate_ide_selection(ide_context, &ctx),
            generate_ide_opened_file(ide_context, &ctx),
            generate_output_style(&ctx),
            generate_queued_commands(queued_commands),
            generate_diagnostics(&ctx),
            generate_lsp_diagnostics(&ctx),
            generate_unified_tasks(&ctx, conversation_history),
            generate_async_hook_responses(&ctx),
            generate_memory(&ctx, conversation_history),
            generate_token_usage(conversation_history),
            generate_budget_usd(&ctx),
            generate_verify_plan_reminder(conversation_history, &ctx),
        ]).await
    } else {
        vec![]
    };

    // Combine all attachments (order: user prompt -> core -> main agent)
    [
        user_prompt_attachments,
        core_attachments,
        main_agent_attachments,
    ]
    .into_iter()
    .flatten()
    .flatten()
    .collect()
}
```

### Error Handling Wrapper

Each attachment generator is wrapped with error handling to prevent individual failures from breaking the entire flow:

```rust
async fn wrap_with_error_handling<F, Fut>(
    label: &str,
    generator: F,
) -> Vec<Attachment>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Vec<Attachment>>,
{
    let start = Instant::now();

    match timeout(Duration::from_secs(1), generator()).await {
        Ok(attachments) => {
            // 5% sampling for telemetry
            if rand::random::<f32>() < 0.05 {
                emit_telemetry("attachment_compute_duration", &json!({
                    "label": label,
                    "duration_ms": start.elapsed().as_millis(),
                    "attachment_count": attachments.len(),
                }));
            }
            attachments
        }
        Err(_) | Err(e) => {
            log_error!("Attachment error in {label}: {e}");
            vec![]  // Return empty instead of failing
        }
    }
}
```

---

## User Prompt Attachments

### 1. at_mentioned_files

**Trigger**: User message contains `@filename` or `@"path/to/file"` syntax

**Content Format**:
```rust
pub enum AtMentionedFile {
    /// File content attachment
    File {
        filename: PathBuf,
        content: FileContent,  // text, image, notebook, pdf
        truncated: bool,
    },
    /// Directory listing attachment
    Directory {
        path: PathBuf,
        content: String,  // ls output
    },
}

pub enum FileContent {
    Text { file_path: PathBuf, content: String, num_lines: i32, start_line: i32, total_lines: i32 },
    Image { data: Vec<u8>, media_type: String },
    Notebook { cells: Vec<NotebookCell> },
    Pdf { pages: Vec<PdfPage> },
}
```

**Features**:
- Line range syntax: `@file.txt:10-20`
- Directory handling: Returns `ls` output
- Image support: Returns compressed image data
- Caching: Returns `already_read_file` if unchanged

### 2. mcp_resources

**Trigger**: User message contains `@server:resource_uri` syntax

**Content Format**:
```rust
pub struct McpResource {
    pub server: String,
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub content: McpResourceContent,
}

pub enum McpResourceContent {
    Text(String),
    Blob { data: Vec<u8>, mime_type: String },
    Empty,
}
```

### 3. agent_mentions

**Trigger**: User message contains `@agent-<agentType>` syntax

**Content Format**:
```rust
pub struct AgentMention {
    pub agent_type: String,  // "search", "edit", "custom"
}
```

---

## Core Attachments

These attachments are checked on every turn for all agents.

### 4. changed_files

**Trigger**: Any previously-read file has been modified on disk

**Content Format**:
```rust
pub enum ChangedFile {
    /// Text file with diff snippet
    TextFile {
        filename: PathBuf,
        snippet: String,  // Unified diff format
    },
    /// Image file change
    ImageFile {
        filename: PathBuf,
        content: String,  // Base64 compressed
    },
    /// Todo file change
    Todo {
        content: Vec<TodoItem>,
        item_count: i32,
        context: String,  // "file-watch"
    },
}
```

### 5. nested_memory

**Trigger**: Read tool triggers nested memory discovery (CLAUDE.md files, rules)

**Content Format**:
```rust
pub struct NestedMemory {
    pub path: PathBuf,
    pub content: FileContent,
}
```

**Discovery Sources** (in priority order):
1. Managed settings (system-controlled)
2. User settings (`~/.claude/rules/`)
3. Project settings (`./CLAUDE.md`, `./.claude/CLAUDE.md`, `./.claude/rules/`)
4. Local settings (`./CLAUDE.local.md`)
5. CWD-level rules (ancestors of cwd)

### 6. plan_mode

**Trigger**: Tool permission context mode is "plan"

**Content Format**:
```rust
pub struct PlanModeAttachment {
    pub reminder_type: PlanReminderType,  // Full, Sparse
    pub is_sub_agent: bool,
    pub plan_file_path: PathBuf,
    pub plan_exists: bool,
}

pub enum PlanReminderType {
    /// Complete instructions (first turn, every N turns)
    Full,
    /// Abbreviated version (intermediate turns)
    Sparse,
}
```

**Reminder Frequency**:
- Full reminder: Turn 1, then every `FULL_REMINDER_EVERY_N_ATTACHMENTS` turns
- Sparse reminder: All other turns
- Sub-agent reminder: Separate shorter version for Plan/Explore agents

### 7. plan_mode_reentry

**Trigger**: Re-entering plan mode after previously exiting

**Content Format**:
```rust
pub struct PlanModeReentry {
    pub plan_file_path: PathBuf,
}
```

### 8. plan_mode_exit

**Trigger**: Just exited plan mode

**Content Format**:
```rust
pub struct PlanModeExit {
    pub plan_file_path: PathBuf,
    pub plan_exists: bool,
}
```

### 9. delegate_mode

**Trigger**: Tool permission context mode is "delegate"

**Content Format**:
```rust
pub struct DelegateMode {
    pub team_name: String,
    pub task_list_path: PathBuf,
}
```

### 10. delegate_mode_exit

**Trigger**: Just exited delegate mode

**Content Format**:
```rust
pub struct DelegateModeExit;
```

### 11. todo_reminders

**Trigger**:
- Been 5+ assistant turns since last TodoWrite tool use
- Been 3+ assistant turns since last reminder

**Content Format**:
```rust
pub struct TodoReminder {
    pub content: Vec<TodoItem>,
    pub item_count: i32,
}
```

### 12. collab_notification

**Trigger**: Collaboration messages available

**Content Format**:
```rust
pub struct CollabNotification {
    pub chats: Vec<CollabChat>,
}

pub struct CollabChat {
    pub handle: String,  // "teammate" or "self"
    pub unread_count: i32,
}
```

### 13. critical_system_reminder

**Trigger**: User has set `critical_system_reminder_experimental` in configuration

**Content Format**:
```rust
pub struct CriticalSystemReminder {
    pub content: String,
}
```

### 14. invoked_skills

**Trigger**: Skills invoked in the session

**Content Format**:
```rust
pub struct InvokedSkills {
    pub skills: Vec<InvokedSkill>,
}

pub struct InvokedSkill {
    pub name: String,
    pub path: PathBuf,
    pub content: String,
}
```

---

## Main Agent Only Attachments

These attachments are only available to the primary agent (not sub-agents).

### 15. ide_selection

**Trigger**: User has selected text in IDE

**Content Format**:
```rust
pub struct IdeSelection {
    pub ide_name: String,  // "Cursor", "VS Code", "Zed"
    pub line_start: i32,
    pub line_end: i32,
    pub filename: PathBuf,
    pub content: String,  // Truncated at 2000 chars
}
```

### 16. ide_opened_file

**Trigger**: User opened a file in IDE (without selection)

**Content Format**:
```rust
pub struct IdeOpenedFile {
    pub filename: PathBuf,
}
```

### 17. output_style

**Trigger**: User has set output style preference (non-default)

**Content Format**:
```rust
pub struct OutputStyle {
    pub style: String,  // "concise", "detailed", "technical", etc.
}
```

### 18. queued_commands

**Trigger**: User has queued commands

**Content Format**:
```rust
pub struct QueuedCommand {
    pub prompt: QueuedPrompt,
    pub source_uuid: String,
    pub image_paste_ids: Option<Vec<String>>,
}

pub enum QueuedPrompt {
    Text(String),
    Mixed(Vec<ContentBlock>),  // Text + images
}
```

### 19. diagnostics

**Trigger**: New diagnostics available from diagnostic system

**Content Format**:
```rust
pub struct Diagnostics {
    pub files: Vec<DiagnosticFile>,
    pub is_new: bool,
}

pub struct DiagnosticFile {
    pub file_path: PathBuf,
    pub diagnostics: Vec<Diagnostic>,
}

pub struct Diagnostic {
    pub message: String,
    pub severity: DiagnosticSeverity,  // Error, Warning, Info, Hint
    pub line: i32,
}
```

### 20. lsp_diagnostics

**Trigger**: LSP server provides new diagnostics

**Content Format**: Same as `diagnostics`

### 21. task_status

**Trigger**: Background tasks have status updates

**Content Format**:
```rust
pub struct TaskStatus {
    pub task_id: String,
    pub task_type: TaskType,  // Shell, Agent, RemoteSession
    pub status: TaskStatusValue,  // Running, Completed, Failed
    pub description: String,
    pub delta_summary: Option<String>,
}
```

### 22. task_progress

**Trigger**: Tasks have progress updates (throttled by turn count)

**Content Format**:
```rust
pub struct TaskProgress {
    pub task_id: String,
    pub task_type: TaskType,
    pub message: String,
}
```

**Throttling**: `PROGRESS_TURN_THRESHOLD = 3` turns between updates

### 23. async_hook_responses

**Trigger**: Async hooks return responses

**Content Format**:
```rust
pub struct AsyncHookResponse {
    pub process_id: String,
    pub hook_name: String,
    pub hook_event: HookEventType,
    pub tool_name: Option<String>,
    pub response: HookResponse,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: Option<i32>,
}

pub struct HookResponse {
    pub system_message: Option<String>,
    pub additional_context: Option<String>,
}
```

### 24. memory

**Trigger**: Session memory available

**Content Format**:
```rust
pub struct Memory {
    pub memories: Vec<SessionMemoryItem>,
}

pub struct SessionMemoryItem {
    pub full_path: PathBuf,
    pub content: String,  // Preview
    pub last_modified: SystemTime,
    pub remaining_lines: i32,
}
```

### 25. token_usage

**Trigger**: Environment variable `CLAUDE_CODE_ENABLE_TOKEN_USAGE_ATTACHMENT` is set

**Content Format**:
```rust
pub struct TokenUsage {
    pub used: i32,
    pub total: i32,
    pub remaining: i32,
}
```

### 26. budget_usd

**Trigger**: `max_budget_usd` is configured in options

**Content Format**:
```rust
pub struct BudgetUsd {
    pub used: f64,
    pub total: f64,
    pub remaining: f64,
}
```

### 27. verify_plan_reminder

**Trigger**: Plan implementation completed

**Content Format**:
```rust
pub struct VerifyPlanReminder;
```

---

## Additional Attachment Types

### 28. compact_file_reference

**Trigger**: File was read before compaction but too large to re-include

**Content Format**:
```rust
pub struct CompactFileReference {
    pub filename: PathBuf,
}
```

### 29. plan_file_reference

**Trigger**: Plan file exists from previous session

**Content Format**:
```rust
pub struct PlanFileReference {
    pub plan_file_path: PathBuf,
    pub plan_content: String,
}
```

### 30. ultramemory

**Trigger**: Ultra memory content available

**Content Format**:
```rust
pub struct Ultramemory {
    pub content: String,
}
```

### 31. micro_compact

**Trigger**: Micro-compaction applied to clear tool results

**Content Format**:
```rust
pub struct MicroCompact {
    pub cleared_count: i32,
}
```

**Message**: `Micro-compact cleared {N} tool result(s) to reduce context size.`

---

## Hook-Related Attachment Types

### 32. hook_blocking_error

**Content Format**:
```rust
pub struct HookBlockingError {
    pub hook_name: String,
    pub command: String,
    pub blocking_error: String,
}
```

### 33. hook_success

**Content Format**:
```rust
pub struct HookSuccess {
    pub hook_event: HookEventType,
    pub hook_name: String,
    pub content: String,
}
```

### 34. hook_additional_context

**Content Format**:
```rust
pub struct HookAdditionalContext {
    pub hook_name: String,
    pub content: Vec<String>,
}
```

### 35. hook_stopped_continuation

**Content Format**:
```rust
pub struct HookStoppedContinuation {
    pub hook_name: String,
    pub message: String,
}
```

---

## Silent Attachment Types

These types return empty arrays from the conversion function (handled elsewhere):

| Type | Purpose |
|------|---------|
| `already_read_file` | File unchanged since last read (cached) |
| `command_permissions` | Permission context (handled elsewhere) |
| `edited_image_file` | Image file change (visual diff not supported) |
| `hook_cancelled` | Hook was cancelled |
| `hook_error_during_execution` | Hook execution error |
| `hook_non_blocking_error` | Non-blocking hook error |
| `hook_system_message` | Hook system message (handled elsewhere) |
| `structured_output` | Structured output (handled differently) |
| `hook_permission_decision` | Hook permission decision |
| `task_reminder` | Task reminder (currently disabled) |

---

## Attachment Priority and Ordering

Attachments are processed and inserted in this order:

1. **User Prompt Attachments** (if user input exists)
   - at_mentioned_files
   - mcp_resources
   - agent_mentions

2. **Core Attachments** (all agents)
   - changed_files
   - nested_memory
   - plan_mode / plan_mode_reentry
   - plan_mode_exit
   - delegate_mode
   - delegate_mode_exit
   - todo_reminders
   - collab_notification
   - critical_system_reminder
   - invoked_skills

3. **Main Agent Attachments** (main agent only)
   - ide_selection
   - ide_opened_file
   - output_style
   - queued_commands
   - diagnostics
   - lsp_diagnostics
   - task_status / task_progress
   - async_hook_responses
   - memory
   - token_usage
   - budget_usd
   - verify_plan_reminder

---

## Summary Table

| Type | Category | Trigger | Main Agent Only |
|------|----------|---------|-----------------|
| at_mentioned_files | User Prompt | @filename | No |
| mcp_resources | User Prompt | @server:uri | No |
| agent_mentions | User Prompt | @agent-type | No |
| changed_files | Core | File modified | No |
| nested_memory | Core | Related files | No |
| plan_mode | Core | Plan mode active | No |
| plan_mode_reentry | Core | Re-entering plan | No |
| plan_mode_exit | Core | Exiting plan mode | No |
| delegate_mode | Core | Delegate mode | No |
| delegate_mode_exit | Core | Exiting delegate | No |
| todo_reminders | Core | Turns threshold | No |
| collab_notification | Core | Collab messages | No |
| critical_system_reminder | Core | User config | No |
| invoked_skills | Core | Skills invoked | No |
| ide_selection | Main Agent | IDE selection | Yes |
| ide_opened_file | Main Agent | IDE file open | Yes |
| output_style | Main Agent | Style set | Yes |
| queued_commands | Main Agent | Queued cmds | Yes |
| diagnostics | Main Agent | New diagnostics | Yes |
| lsp_diagnostics | Main Agent | LSP diagnostics | Yes |
| task_status | Main Agent | Task updates | Yes |
| task_progress | Main Agent | Task progress | Yes |
| async_hook_responses | Main Agent | Hook responses | Yes |
| memory | Main Agent | Session memory | Yes |
| token_usage | Main Agent | Env var set | Yes |
| budget_usd | Main Agent | Budget set | Yes |
| verify_plan_reminder | Main Agent | Plan complete | Yes |

---

## Configuration Constants

| Constant | Value  | Description |
|----------|--------|-------------|
| `ATTACHMENT_TIMEOUT` | 1000ms | Max time for all attachment generation |
| `TELEMETRY_SAMPLE_RATE` | 0.05   | 5% sampling for performance monitoring |
| `TURNS_SINCE_TODO_WRITE` | 5      | Turns before showing todo reminder |
| `TURNS_BETWEEN_TODO_REMINDERS` | 3      | Turns between todo reminders |
| `PROGRESS_TURN_THRESHOLD` | 3      | Turns between task progress updates |
| `FULL_REMINDER_EVERY_N_ATTACHMENTS` | 5      | Full plan mode reminder interval |
| `MAX_FILE_CONTENT_SIZE` | 40000  | Max content size (40KB) |
| `MAX_FILE_LINES` | 3000   | Max lines per file |
| `MAX_IMPORT_DEPTH` | 5      | Max @import recursion depth |

---

## Related Documentation

- [XML Format Specification](./xml-format.md) - XML tag formats for system reminders
- [Plan Mode](./features.md#plan-mode) - Plan mode workflow and reminders
- [Context Compaction](./features.md#context-compaction) - Micro-compact reminders
- [Agent Loop](./core-loop.md) - Attachment injection in message flow
