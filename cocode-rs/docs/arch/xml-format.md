# System Reminder XML Format Specification

## Overview

System reminders in cocode use XML-style tags to wrap metadata and contextual information injected into the conversation. This document catalogs all XML formats used for system reminders.

---

## Core XML Wrapper Functions

### 1. wrap_system_reminder_text

**Purpose**: Wraps text content in `<system-reminder>` XML tags

```rust
/// Wrap text in system-reminder XML tags
pub fn wrap_system_reminder_text(text: &str) -> String {
    format!("<system-reminder>\n{text}\n</system-reminder>")
}
```

**Output Format**:
```xml
<system-reminder>
[Content text here]
</system-reminder>
```

### 2. wrap_in_system_reminder

**Purpose**: Wraps an array of message objects, applying `<system-reminder>` tags to all text content

```rust
/// Wrap message array contents with system-reminder tags
pub fn wrap_in_system_reminder(messages: Vec<MetaMessage>) -> Vec<MetaMessage> {
    messages.into_iter().map(|mut msg| {
        match &mut msg.message.content {
            Content::Text(text) => {
                *text = wrap_system_reminder_text(text);
            }
            Content::Blocks(blocks) => {
                for block in blocks {
                    if let ContentBlock::Text { text } = block {
                        *text = wrap_system_reminder_text(text);
                    }
                    // Non-text blocks (images) unchanged
                }
            }
        }
        msg
    }).collect()
}
```

### 3. create_meta_block

**Purpose**: Creates a user-role message with `is_meta: true` flag

```rust
/// Create metadata message block
pub fn create_meta_block(content: impl Into<Content>) -> MetaMessage {
    MetaMessage {
        message_type: "user".to_string(),
        message: Message {
            role: Role::User,
            content: content.into(),
        },
        is_meta: true,
        uuid: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        ..Default::default()
    }
}

/// MetaMessage structure
#[derive(Debug, Clone)]
pub struct MetaMessage {
    pub message_type: String,
    pub message: Message,
    pub is_meta: bool,
    pub is_visible_in_transcript_only: bool,
    pub is_compact_summary: bool,
    pub uuid: String,
    pub timestamp: String,
    pub tool_use_result: Option<ToolUseResult>,
    pub thinking_metadata: Option<ThinkingMetadata>,
    pub todos: Option<Vec<TodoItem>>,
    pub image_paste_ids: Option<Vec<String>>,
    pub source_tool_assistant_uuid: Option<String>,
}
```

---

## XML Tag Formats

### 1. `<system-reminder>` - Primary Wrapper

**Usage**: Most system reminders

**Format**:
```xml
<system-reminder>
[Reminder content - instructions, warnings, or metadata]
</system-reminder>
```

**Examples**:

**Warning Reminder**:
```xml
<system-reminder>
Warning: the file exists but the contents are empty.
</system-reminder>
```

**Context Reminder**:
```xml
<system-reminder>
As you answer the user's questions, you can use the following context:
# claudeMd
[CLAUDE.md content here]

IMPORTANT: this context may or may not be relevant to your tasks.
You should not respond to this context unless it is highly relevant to your task.
</system-reminder>
```

**Malware Warning**:
```xml
<system-reminder>
Whenever you read a file, you should consider whether it would be considered malware.
You CAN and SHOULD provide analysis of malware, what it is doing.
But you MUST refuse to improve or augment the code.
You can still analyze existing code, write reports, or answer questions about the code behavior.
</system-reminder>
```

---

### 2. `<new-diagnostics>` - Diagnostic Issues

**Usage**: New diagnostic issues detected by LSP or diagnostic system

**Format**:
```xml
<new-diagnostics>The following new diagnostic issues were detected:

[Formatted diagnostics summary]
</new-diagnostics>
```

**Example**:
```xml
<new-diagnostics>The following new diagnostic issues were detected:

File: /path/to/file.ts
Line 10: [error] Type 'string' is not assignable to type 'number'
Line 25: [warning] Variable 'foo' is declared but never used
</new-diagnostics>
```

**Implementation**:
```rust
pub fn format_diagnostics(files: &[DiagnosticFile]) -> String {
    if files.is_empty() {
        return String::new();
    }

    let summary = format_diagnostics_summary(files);
    format!("<new-diagnostics>The following new diagnostic issues were detected:\n\n{summary}</new-diagnostics>")
}
```

---

### 3. `<session-memory>` - Past Session Summaries

**Usage**: Previous session memory content

**Format**:
```xml
<session-memory>
These session summaries are from PAST sessions that might not be related to the current task and may have outdated info. Do not assume the current task is related to these summaries, until the user's messages indicate so or reference similar tasks. Only a preview of each memory is shown - use the Read tool with the provided path to access full session memory when a session is relevant.

## Previous Session ([date])
Full session notes: [path] ([N] more lines in full file)

[preview content]

---

## Previous Session ([date])
[...]
</session-memory>
```

**Implementation**:
```rust
pub fn format_session_memory(memories: &[SessionMemoryItem]) -> String {
    let formatted = memories.iter().map(|m| {
        let extra = if m.remaining_lines > 0 {
            format!(" ({} more lines in full file)", m.remaining_lines)
        } else {
            String::new()
        };
        let date = m.last_modified.format("%Y-%m-%d");
        format!("## Previous Session ({date})\nFull session notes: {}{extra}\n\n{}",
            m.full_path.display(), m.content)
    }).collect::<Vec<_>>().join("\n\n---\n\n");

    format!(r#"<session-memory>
These session summaries are from PAST sessions that might not be related to the current task and may have outdated info. Do not assume the current task is related to these summaries, until the user's messages indicate so or reference similar tasks. Only a preview of each memory is shown - use the Read tool with the provided path to access full session memory when a session is relevant.

{formatted}
</session-memory>"#)
}
```

---

### 4. `<mcp-resource>` - MCP Resource Content

**Usage**: Content from MCP (Model Context Protocol) resources

**Format**:

**With Content**:
```
Full contents of resource:

[resource content]

Do NOT read this resource again unless you think it may have changed, since you already have the full contents.
```

**Empty Resource**:
```xml
<mcp-resource server="[server]" uri="[uri]">(No content)</mcp-resource>
```

**No Displayable Content**:
```xml
<mcp-resource server="[server]" uri="[uri]">(No displayable content)</mcp-resource>
```

**Implementation**:
```rust
pub fn format_mcp_resource(resource: &McpResource) -> String {
    match &resource.content {
        McpResourceContent::Empty => {
            format!(r#"<mcp-resource server="{}" uri="{}">(No content)</mcp-resource>"#,
                resource.server, resource.uri)
        }
        McpResourceContent::Text(text) => {
            format!("Full contents of resource:\n\n{text}\n\nDo NOT read this resource again unless you think it may have changed, since you already have the full contents.")
        }
        McpResourceContent::Blob { mime_type, .. } => {
            format!("[Binary content: {mime_type}]")
        }
    }
}
```

---

## Plan Mode Content Formats

### 5. Full Plan Mode Instructions

**Usage**: First reminder and every N-th reminder for main agent

**Format**:
```xml
<system-reminder>
Plan mode is active. The user indicated that they do not want you to execute yet -- you MUST NOT make any edits (with the exception of the plan file mentioned below), run any non-readonly tools (including changing configs or making commits), or otherwise make any changes to the system. This supercedes any other instructions you have received.

## Plan File Info:
[If plan exists:]
A plan file already exists at {plan_file_path}. You can read it and make incremental edits using the Edit tool.

[If no plan:]
No plan file exists yet. You should create your plan at {plan_file_path} using the Write tool.

You should build your plan incrementally by writing to or editing this file. NOTE that this is the only file you are allowed to edit - other than this you are only allowed to take READ-ONLY actions.

## Plan Workflow

### Phase 1: Initial Understanding
Goal: Gain a comprehensive understanding of the user's request by reading through code and asking them questions. Critical: In this phase you should only use the Explore subagent type.

1. Focus on understanding the user's request and the code associated with their request

2. **Launch up to {max_explore_agents} Explore agents IN PARALLEL** (single message, multiple tool calls) to efficiently explore the codebase.
   - Use 1 agent when the task is isolated to known files, the user provided specific file paths, or you're making a small targeted change.
   - Use multiple agents when: the scope is uncertain, multiple areas of the codebase are involved, or you need to understand existing patterns before planning.
   - Quality over quantity - {max_explore_agents} agents maximum, but you should try to use the minimum number of agents necessary (usually just 1)
   - If using multiple agents: Provide each agent with a specific search focus or area to explore.

3. After exploring the code, use the AskUserQuestion tool to clarify ambiguities in the user request up front.

### Phase 2: Design
Goal: Design an implementation approach.

Launch Plan agent(s) to design the implementation based on the user's intent and your exploration results from Phase 1.

You can launch up to {max_plan_agents} agent(s) in parallel.

**Guidelines:**
- **Default**: Launch at least 1 Plan agent for most tasks - it helps validate your understanding and consider alternatives
- **Skip agents**: Only for truly trivial tasks (typo fixes, single-line changes, simple renames)
- **Multiple agents**: Use up to {max_plan_agents} agents for complex tasks that benefit from different perspectives

### Phase 3: Review
Goal: Review the plan(s) from Phase 2 and ensure alignment with the user's intentions.
1. Read the critical files identified by agents to deepen your understanding
2. Ensure that the plans align with the user's original request
3. Use AskUserQuestion to clarify any remaining questions with the user

### Phase 4: Final Plan
Goal: Write your final plan to the plan file (the only file you can edit).
- Include only your recommended approach, not all alternatives
- Ensure that the plan file is concise enough to scan quickly, but detailed enough to execute effectively
- Include the paths of critical files to be modified
- Include a verification section describing how to test the changes end-to-end

### Phase 5: Call ExitPlanMode
At the very end of your turn, once you have asked the user questions and are happy with your final plan file - you should always call ExitPlanMode to indicate to the user that you are done planning.
This is critical - your turn should only end with either using the AskUserQuestion tool OR calling ExitPlanMode. Do not stop unless it's for these 2 reasons

**Important:** Use AskUserQuestion ONLY to clarify requirements or choose between approaches. Use ExitPlanMode to request plan approval. Do NOT ask about plan approval in any other way.

NOTE: At any point in time through this workflow you should feel free to ask the user questions or clarifications using the AskUserQuestion tool. Don't make large assumptions about user intent.
</system-reminder>
```

---

### 6. Sparse Plan Mode Instructions

**Usage**: Intermediate reminders between full reminders (to save tokens)

**Format**:
```xml
<system-reminder>
Plan mode still active (see full instructions earlier in conversation). Read-only except plan file ({plan_file_path}). Follow 5-phase workflow. End turns with AskUserQuestion (for clarifications) or ExitPlanMode (for plan approval). Never ask about plan approval via text or AskUserQuestion.
</system-reminder>
```

---

### 7. Sub-Agent Plan Mode Instructions

**Usage**: When a subagent operates in plan mode

**Format**:
```xml
<system-reminder>
Plan mode is active. The user indicated that they do not want you to execute yet -- you MUST NOT make any edits, run any non-readonly tools (including changing configs or making commits), or otherwise make any changes to the system. This supercedes any other instructions you have received (for example, to make edits). Instead, you should:

## Plan File Info:
[If plan exists:]
A plan file already exists at {plan_file_path}. You can read it and make incremental edits using the Edit tool if you need to.

[If no plan:]
No plan file exists yet. You should create your plan at {plan_file_path} using the Write tool if you need to.

You should build your plan incrementally by writing to or editing this file. NOTE that this is the only file you are allowed to edit - other than this you are only allowed to take READ-ONLY actions.
Answer the user's query comprehensively, using the AskUserQuestion tool if you need to ask the user clarifying questions.
</system-reminder>
```

---

### 8. Plan Mode Reentry

**Usage**: When re-entering plan mode after previously exiting

**Format**:
```xml
<system-reminder>
## Re-entering Plan Mode

You are returning to plan mode after having previously exited it. A plan file exists at {plan_file_path} from your previous planning session.

**Before proceeding with any new planning, you should:**
1. Read the existing plan file to understand what was previously planned
2. Evaluate the user's current request against that plan
3. Decide how to proceed:
   - **Different task**: If the user's request is for a different task—even if it's similar or related—start fresh by overwriting the existing plan
   - **Same task, continuing**: If this is explicitly a continuation or refinement of the exact same task, modify the existing plan while cleaning up outdated or irrelevant sections
4. Continue on with the plan process and most importantly you should always edit the plan file one way or the other before calling ExitPlanMode

Treat this as a fresh planning session. Do not assume the existing plan is relevant without evaluating it first.
</system-reminder>
```

---

### 9. Plan Mode Exit

**Usage**: Notification when exiting plan mode

**Format**:
```xml
<system-reminder>
## Exited Plan Mode

You have exited plan mode. You can now make edits, run tools, and take actions. The plan file is located at {plan_file_path} if you need to reference it.
</system-reminder>
```

---

## Delegate Mode Content Formats

### 10. Delegate Mode Exit

**Usage**: Notification when exiting delegate mode

**Format**:
```xml
<system-reminder>
## Exited Delegate Mode

You have exited delegate mode. You can now use all tools (Bash, Read, Write, Edit, etc.) and take actions directly. Continue with your tasks.
</system-reminder>
```

---

## Task Status Formats

### 11. Task Status

**Usage**: Unified task status notifications

**Format**:
```xml
<system-reminder>
Task {task_id} (type: {task_type}) (status: {status}) (description: {description}) [Delta: {delta_summary}] You can check its output using the TaskOutput tool.
</system-reminder>
```

**Example**:
```xml
<system-reminder>
Task task-123 (type: shell) (status: completed) (description: npm test) Delta: All tests passed. You can check its output using the TaskOutput tool.
</system-reminder>
```

### 12. Task Progress

**Usage**: Task progress updates

**Format**:
```xml
<system-reminder>
{progress_message}
</system-reminder>
```

---

## Other Content Formats

### 13. Invoked Skills

**Usage**: Skills invoked during session

**Format**:
```xml
<system-reminder>
The following skills were invoked in this session. Continue to follow these guidelines:

### Skill: {skill_name}
Path: {skill_path}

{skill_content}

---

### Skill: {skill_name_2}
[...]
</system-reminder>
```

### 14. Collaboration Notification

**Usage**: Collaboration message notifications

**Format**:
```xml
<system-reminder>
You have {N} unread collab message(s) from: @{handle1} ({X} new), @{handle2} ({Y} new). Use the CollabRead tool to read these messages.
</system-reminder>
```

### 15. Verify Plan Reminder

**Usage**: Reminder to verify plan completion

**Format**:
```xml
<system-reminder>
You have completed implementing the plan. Please verify that all plan items were completed correctly.
</system-reminder>
```

### 16. Token Usage

**Usage**: Token usage tracking

**Format**:
```xml
<system-reminder>
Token usage: {used}/{total}; {remaining} remaining
</system-reminder>
```

### 17. Budget USD

**Usage**: Budget tracking

**Format**:
```xml
<system-reminder>
USD budget: ${used}/${total}; ${remaining} remaining
</system-reminder>
```

### 18. Micro-Compact

**Usage**: Notification when micro-compaction clears tool results

**Format**:
```xml
<system-reminder>
Micro-compact cleared {N} tool result(s) to reduce context size.
</system-reminder>
```

---

## Hook-Related Formats

### 19. Hook Blocking Error

**Format**:
```xml
<system-reminder>
{hook_name} hook blocking error from command: "{command}": {blocking_error}
</system-reminder>
```

### 20. Hook Success

**Format**:
```xml
<system-reminder>
{hook_name} hook success: {content}
</system-reminder>
```

### 21. Hook Additional Context

**Format**:
```xml
<system-reminder>
{hook_name} hook additional context: {content_lines}
</system-reminder>
```

### 22. Hook Stopped Continuation

**Format**:
```xml
<system-reminder>
{hook_name} hook stopped continuation: {message}
</system-reminder>
```

---

## Processing Pipeline

```
Attachment Object
      │
      ▼
convert_attachment_to_system_message()
      │
      ├─────────────────────────────────────┐
      │                                     │
      ▼                                     ▼
┌─────────────────┐              ┌─────────────────┐
│  wrap_in_system │              │  create_meta_   │
│  _reminder()    │              │  block(wrap_    │
│                 │              │  system_reminder│
│  Most types:    │              │  _text())       │
│  - file         │              │                 │
│  - directory    │              │  Some types:    │
│  - todo_reminder│              │  - task_status  │
│  - plan_mode    │              │  - task_progress│
│  - diagnostics  │              │  - token_usage  │
│  - etc.         │              │  - budget_usd   │
│                 │              │  - hook_*       │
└────────┬────────┘              └────────┬────────┘
         │                                 │
         ▼                                 ▼
┌──────────────────────────────────────────────────┐
│          System Message with is_meta: true        │
│                                                   │
│  MetaMessage {                                    │
│    message_type: "user",                          │
│    message: Message {                             │
│      role: Role::User,                            │
│      content: "<system-reminder>...</system-reminder>"
│    },                                             │
│    is_meta: true,                                 │
│    uuid: "...",                                   │
│    timestamp: "..."                               │
│  }                                                │
└──────────────────────────────────────────────────┘
      │
      ▼
Inserted into conversation before API call
```

---

## Implementation Reference

### Attachment to Message Conversion

```rust
/// Convert attachment to system messages
pub fn convert_attachment_to_system_message(attachment: &Attachment) -> Vec<MetaMessage> {
    match attachment {
        // Pattern 1: Tool Simulation (wrap_in_system_reminder + tool use/result)
        Attachment::File(file) => {
            wrap_in_system_reminder(vec![
                create_tool_use_message("Read", json!({ "file_path": file.filename })),
                create_tool_result_message("Read", &file.content),
            ])
        }

        Attachment::Directory { path, content } => {
            wrap_in_system_reminder(vec![
                create_tool_use_message("Bash", json!({
                    "command": format!("ls {}", path.display()),
                    "description": format!("Lists files in {}", path.display()),
                })),
                create_tool_result_message("Bash", content),
            ])
        }

        // Pattern 2: Direct Meta Block (wrap_in_system_reminder + create_meta_block)
        Attachment::EditedTextFile { filename, snippet } => {
            wrap_in_system_reminder(vec![
                create_meta_block(format!(
                    "Note: {filename} was modified. Changes:\n{snippet}"
                ))
            ])
        }

        // Pattern 3: Direct Wrap (create_meta_block + wrap_system_reminder_text)
        Attachment::TaskStatus { task_id, task_type, status, description, delta_summary } => {
            let mut parts = vec![
                format!("Task {task_id}"),
                format!("(type: {task_type})"),
                format!("(status: {status})"),
                format!("(description: {description})"),
            ];
            if let Some(delta) = delta_summary {
                parts.push(format!("Delta: {delta}"));
            }
            parts.push("You can check its output using the TaskOutput tool.".to_string());

            vec![create_meta_block(wrap_system_reminder_text(&parts.join(" ")))]
        }

        // Pattern 4: Router (plan_mode variants)
        Attachment::PlanMode { reminder_type, is_sub_agent, plan_file_path, plan_exists } => {
            if *is_sub_agent {
                generate_sub_agent_plan_mode_instructions(plan_file_path, *plan_exists)
            } else if matches!(reminder_type, PlanReminderType::Sparse) {
                generate_sparse_plan_mode_instructions(plan_file_path)
            } else {
                generate_full_plan_mode_instructions(plan_file_path, *plan_exists)
            }
        }

        // Empty array (silent types)
        Attachment::AlreadyReadFile { .. } => vec![],
        Attachment::DelegateMode { .. } => vec![],

        // ... other attachment types
    }
}
```

### Helper Functions

```rust
/// Create tool use message (simulates tool being called)
pub fn create_tool_use_message(tool_name: &str, params: serde_json::Value) -> MetaMessage {
    create_meta_block(format!(
        "Called the {tool_name} tool with the following input: {}",
        serde_json::to_string(&params).unwrap_or_default()
    ))
}

/// Create tool result message (simulates tool returning result)
pub fn create_tool_result_message(tool_name: &str, result: &str) -> MetaMessage {
    create_meta_block(format!(
        "Result of calling the {tool_name} tool: {result}"
    ))
}
```

---

## Related Documentation

- [Attachments](./attachments.md) - All attachment type catalog
- [Plan Mode](./features.md#plan-mode) - Plan mode workflow
- [Agent Loop](./core-loop.md) - Message flow and injection points
