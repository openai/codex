# System Reminder Design for Codex-RS

> **Status**: Design Complete - Ready for Implementation
> **Version**: 2.1 (Reviewed and updated with extension pattern and existing code integration)
> **Reference**: Claude Code v2.0.59 (`chunks.107.mjs`, `chunks.154.mjs`, `chunks.153.mjs`)
>
> **Key Updates in v2.1**:
> - Phase 1 scope reduced to 5 attachment types (from 8)
> - Extension pattern clarified: `*_ext.rs` only for files outside `system_reminder/` module
> - Config moved to `core/src/config/system_reminder.rs`
> - Added subagent executor integration (existing `critical_system_reminder` field)
> - Added compact integration (`is_system_reminder_message()` predicate)
> - Two injection paths documented (main conversation vs subagent)

---

## Table of Contents

1. [Overview](#1-overview)
2. [Architecture](#2-architecture)
3. [Data Structures](#3-data-structures)
4. [XML Tag System](#4-xml-tag-system)
5. [Trigger Scenarios](#5-trigger-scenarios)
6. [Generator System](#6-generator-system)
7. [File Tracking System](#7-file-tracking-system)
8. [Integration Points](#8-integration-points)
9. [Multi-LLM Compatibility](#9-multi-llm-compatibility)
10. [Configuration](#10-configuration)
11. [Implementation Tasks](#11-implementation-tasks)
12. [Acceptance Criteria](#12-acceptance-criteria)
13. [Testing Strategy](#13-testing-strategy)

---

## 1. Overview

### 1.1 What is System Reminder?

System reminders are a **contextual injection mechanism** that automatically inserts metadata, state information, and instructions into conversations at strategic points. This mechanism:

- Provides rich context to the LLM without cluttering user-visible output
- Uses XML-tagged messages (`<system-reminder>`, `<system-notification>`, etc.)
- Runs parallel generators with timeout protection (1 second max)
- Supports throttling to avoid spam
- Marks all reminders with `isMeta: true` flag

### 1.2 Claude Code Reference (v2.0.59)

Claude Code implements **34+ attachment types** organized in **3 tiers**:

| Tier | Description | Count | Agent Scope |
|------|-------------|-------|-------------|
| **User Prompt** | @mentioned files, MCP resources, agent mentions | 3 | All |
| **Core** | changed_files, plan_mode, todo_reminders, critical instructions | 7 | All |
| **Main Agent** | IDE state, diagnostics, background tasks, memory, budget | 13+ | Main only |

**Key Implementation Files:**
- `chunks.107.mjs:1813-1829` - `JH5()` generateAllAttachments orchestrator
- `chunks.154.mjs:3-322` - `kb3()` attachment to message converter
- `chunks.153.mjs:2850-2883` - XML wrapper functions (`Qu`, `NG`, `R0`)

### 1.3 Scope for Codex-RS (Phase 1)

**Target: Core attachments (5 attachment types)**

| Type | Tier | Description | Priority |
|------|------|-------------|----------|
| `critical_instruction` | Core | User-defined always-on reminder | P0 |
| `plan_mode` | Core | Plan mode instructions and workflow | P0 |
| `todo_reminder` | Core | Periodic reminder about empty/stale todo list | P0 |
| `changed_files` | Core | Notify when previously-read files change | P1 |
| `background_task` | MainAgentOnly | Status of background shell tasks | P1 |

**Phase 2** (Future):
- `nested_memory` - Auto-include related files (CLAUDE.md, etc.)
- `tool_result` - Metadata about tool call results
- `session_memory` - Past session summaries

**Excluded**: IDE-specific attachments (ide_selection, diagnostics) - CLI focus

### 1.4 Existing Integration Points

**Note**: The following already exists in the codebase but is **NOT YET INJECTED**:

- `AgentDefinition.critical_system_reminder` at `core/src/subagent/definition/mod.rs:71-73`
- Built-in agents (Explore, Plan) define `critical_system_reminder` in `builtin.rs`

This field should be injected into conversation history during subagent execution.

---

## 2. Architecture

### 2.1 Module Structure

**Key Principle**: Files within `system_reminder/` are a **new module** and do NOT need the `*_ext.rs` pattern. The extension pattern only applies to modifications of existing files outside the module.

```
codex-rs/core/src/system_reminder/
├── mod.rs                    # Module exports and SystemReminderOrchestrator
├── types.rs                  # SystemReminder, AttachmentType, ReminderTier, XmlTag
├── generator.rs              # AttachmentGenerator trait, GeneratorContext
├── throttle.rs               # ThrottleConfig, ThrottleState, ThrottleManager
├── file_tracker.rs           # ReadFileState for change detection
└── attachments/              # Individual attachment implementations (Phase 1)
    ├── mod.rs                # Re-exports all generators
    ├── critical_instruction.rs  # Critical instruction generator
    ├── plan_mode.rs          # Plan mode instructions generator
    ├── todo_reminder.rs      # Todo list reminder generator
    ├── changed_files.rs      # File change detection generator
    └── background_task.rs    # Background task status generator
```

### 2.2 Integration Files (Extension Pattern)

**Extension pattern applies ONLY to files outside the `system_reminder/` module:**

```
codex-rs/core/src/
├── system_reminder_ext.rs    # Integration hooks (~60 lines) - USES *_ext.rs PATTERN
├── lib.rs                    # Add: pub mod system_reminder; (1 line)
│
├── config/
│   ├── mod.rs                # Add system_reminder config (3 lines)
│   └── system_reminder.rs    # SystemReminderConfig struct (NEW FILE ~50 lines)
│
├── subagent/
│   ├── executor/
│   │   └── mod.rs            # Inject critical_system_reminder (~15 lines)
│   └── stores.rs             # Add ReminderState to stores (~10 lines)
│
└── compact_v2/
    └── message_filter.rs     # Add is_system_reminder_message() (~20 lines)
```

**Why this pattern?**
- `system_reminder/` is a new module → organize freely inside
- `system_reminder_ext.rs` integrates with existing `codex.rs` → use extension pattern
- `subagent/executor/mod.rs` already exists → minimal changes only
- `compact_v2/message_filter.rs` already exists → add predicate function

### 2.3 Message Flow (Two Injection Paths)

**There are TWO distinct injection paths** depending on whether it's the main conversation or a subagent:

#### Path 1: Main Conversation (`codex.rs`)

```
User Input / Turn Start
        ↓
┌───────────────────────────────────────────────────────────┐
│  SystemReminderOrchestrator::generate_all()               │
│  (Matches JH5 in chunks.107.mjs:1813-1829)                │
│  ├── Create timeout with 1-second max                     │
│  ├── Filter generators by tier (Core vs MainAgentOnly)    │
│  ├── Run ALL generators in parallel (join_all)            │
│  └── Collect results, filter None, handle timeouts        │
└───────────────────────────────────────────────────────────┘
        ↓
Vec<SystemReminder>
        ↓
┌───────────────────────────────────────────────────────────┐
│  system_reminder_ext::inject_system_reminders()           │
│  Location: codex.rs ~line 2177 (after get_history_for_prompt)
│  ├── Convert to ResponseItem::Message with XML tags       │
│  ├── Find insert position (after environment_context)     │
│  └── Insert into history in order                         │
└───────────────────────────────────────────────────────────┘
        ↓
Prompt { input: [..., reminders, ...] }
        ↓
Adapter routing (OpenAI, Gemini, etc.)
        ↓
LLM API Call
```

#### Path 2: Subagent Executor (`subagent/executor/mod.rs`)

```
Subagent Spawn (Task tool)
        ↓
┌───────────────────────────────────────────────────────────┐
│  SubagentExecutor::build_initial_messages()               │
│  Location: subagent/executor/mod.rs:193-230               │
│  ├── Add system_prompt if configured                      │
│  ├── ** INJECT critical_system_reminder HERE **           │
│  ├── Add resumed messages                                 │
│  └── Add user prompt                                      │
└───────────────────────────────────────────────────────────┘
        ↓
conversation_history: Vec<ResponseItem>
        ↓
SubagentExecutor::run_turn()
        ↓
LLM API Call
```

**Key difference**:
- Main conversation: Full orchestrator with all generators, throttling, file tracking
- Subagent: Simple injection of `critical_system_reminder` field from `AgentDefinition`

---

## 3. Data Structures

### 3.1 Core Types (types.rs)

```rust
use codex_protocol::models::{ContentItem, ResponseItem};
use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================
// XML Tags (matching Claude Code chunks.153.mjs)
// ============================================

/// Primary wrapper tag for most reminders
pub const SYSTEM_REMINDER_OPEN_TAG: &str = "<system-reminder>";
pub const SYSTEM_REMINDER_CLOSE_TAG: &str = "</system-reminder>";

/// Specialized tags for specific attachment types
pub const SYSTEM_NOTIFICATION_OPEN_TAG: &str = "<system-notification>";
pub const SYSTEM_NOTIFICATION_CLOSE_TAG: &str = "</system-notification>";

pub const NEW_DIAGNOSTICS_OPEN_TAG: &str = "<new-diagnostics>";
pub const NEW_DIAGNOSTICS_CLOSE_TAG: &str = "</new-diagnostics>";

pub const SESSION_MEMORY_OPEN_TAG: &str = "<session-memory>";
pub const SESSION_MEMORY_CLOSE_TAG: &str = "</session-memory>";

pub const TEAMMATE_MESSAGE_OPEN_TAG: &str = "<teammate-message";
pub const TEAMMATE_MESSAGE_CLOSE_TAG: &str = "</teammate-message>";

/// XML tag selection for different attachment types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XmlTag {
    /// `<system-reminder>` - Default for most types
    SystemReminder,
    /// `<system-notification>` - For async agent status
    SystemNotification,
    /// `<new-diagnostics>` - For diagnostic issues
    NewDiagnostics,
    /// `<session-memory>` - For past session summaries
    SessionMemory,
    /// No wrapping (direct content)
    None,
}

impl XmlTag {
    pub fn wrap(&self, content: &str) -> String {
        match self {
            XmlTag::SystemReminder => format!(
                "{SYSTEM_REMINDER_OPEN_TAG}\n{content}\n{SYSTEM_REMINDER_CLOSE_TAG}"
            ),
            XmlTag::SystemNotification => format!(
                "{SYSTEM_NOTIFICATION_OPEN_TAG}{content}{SYSTEM_NOTIFICATION_CLOSE_TAG}"
            ),
            XmlTag::NewDiagnostics => format!(
                "{NEW_DIAGNOSTICS_OPEN_TAG}{content}{NEW_DIAGNOSTICS_CLOSE_TAG}"
            ),
            XmlTag::SessionMemory => format!(
                "{SESSION_MEMORY_OPEN_TAG}\n{content}\n{SESSION_MEMORY_CLOSE_TAG}"
            ),
            XmlTag::None => content.to_string(),
        }
    }
}

// ============================================
// Tier and Type Enums
// ============================================

/// Categories of system reminders (matching Claude Code 3-tier system)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReminderTier {
    /// Always checked, available to all agents (sub-agents too)
    Core,
    /// Only for main agent, not sub-agents
    MainAgentOnly,
    /// Only when user input exists
    UserPrompt,
}

/// Types of system reminder attachments
/// Matches Claude Code's 34+ types (implementing subset)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentType {
    // === Core tier ===
    /// Periodic todo list reminder
    TodoReminder,
    /// Plan mode instructions
    PlanMode,
    /// Plan mode re-entry instructions
    PlanModeReentry,
    /// File change notification
    ChangedFiles,
    /// User-defined critical instruction
    CriticalInstruction,
    /// Tool call result metadata
    ToolResult,
    /// Auto-included related files (CLAUDE.md, etc.)
    NestedMemory,

    // === Main agent only ===
    /// Background shell task status
    BackgroundTask,
    /// Async agent completion notification
    AsyncAgentStatus,
    /// Past session summaries
    SessionMemory,
    /// Token usage tracking
    TokenUsage,
    /// Budget tracking
    BudgetUsd,
}

impl AttachmentType {
    /// Get the XML tag for this attachment type
    pub fn xml_tag(&self) -> XmlTag {
        match self {
            AttachmentType::AsyncAgentStatus => XmlTag::SystemNotification,
            AttachmentType::SessionMemory => XmlTag::SessionMemory,
            _ => XmlTag::SystemReminder,
        }
    }

    /// Get the tier for this attachment type
    pub fn tier(&self) -> ReminderTier {
        match self {
            AttachmentType::BackgroundTask
            | AttachmentType::AsyncAgentStatus
            | AttachmentType::SessionMemory
            | AttachmentType::TokenUsage
            | AttachmentType::BudgetUsd => ReminderTier::MainAgentOnly,
            _ => ReminderTier::Core,
        }
    }
}

impl fmt::Display for AttachmentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            AttachmentType::TodoReminder => "todo_reminder",
            AttachmentType::PlanMode => "plan_mode",
            AttachmentType::PlanModeReentry => "plan_mode_reentry",
            AttachmentType::ChangedFiles => "changed_files",
            AttachmentType::CriticalInstruction => "critical_instruction",
            AttachmentType::ToolResult => "tool_result",
            AttachmentType::NestedMemory => "nested_memory",
            AttachmentType::BackgroundTask => "background_task",
            AttachmentType::AsyncAgentStatus => "async_agent_status",
            AttachmentType::SessionMemory => "session_memory",
            AttachmentType::TokenUsage => "token_usage",
            AttachmentType::BudgetUsd => "budget_usd",
        };
        write!(f, "{name}")
    }
}

// ============================================
// SystemReminder Struct
// ============================================

/// A system reminder attachment
/// Matches structure from Claude Code's kb3() output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemReminder {
    /// Type of this attachment
    pub attachment_type: AttachmentType,
    /// Content to be injected (before XML wrapping)
    pub content: String,
    /// Which tier this belongs to (derived from attachment_type)
    pub tier: ReminderTier,
    /// Whether this is metadata (always true for system reminders)
    /// Matches isMeta: true in Claude Code
    pub is_meta: bool,
}

impl SystemReminder {
    /// Create a new system reminder
    pub fn new(attachment_type: AttachmentType, content: String) -> Self {
        Self {
            tier: attachment_type.tier(),
            attachment_type,
            content,
            is_meta: true,  // Always true, matching Claude Code
        }
    }

    /// Wrap content with appropriate XML tag
    pub fn wrap_xml(&self) -> String {
        self.attachment_type.xml_tag().wrap(&self.content)
    }

    /// Check if a message is a system reminder
    pub fn is_system_reminder(message: &[ContentItem]) -> bool {
        if let [ContentItem::InputText { text }] = message {
            text.starts_with(SYSTEM_REMINDER_OPEN_TAG)
                || text.starts_with(SYSTEM_NOTIFICATION_OPEN_TAG)
                || text.starts_with(SESSION_MEMORY_OPEN_TAG)
                || text.starts_with(NEW_DIAGNOSTICS_OPEN_TAG)
        } else {
            false
        }
    }
}

/// Convert SystemReminder to ResponseItem (API message format)
/// Matches R0() function in Claude Code chunks.153.mjs:2179-2204
impl From<SystemReminder> for ResponseItem {
    fn from(sr: SystemReminder) -> Self {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),  // All reminders use user role
            content: vec![ContentItem::InputText {
                text: sr.wrap_xml(),
            }],
            // Note: isMeta would be stored in metadata field if available
        }
    }
}
```

### 3.2 Generator Context

```rust
/// Context provided to attachment generators
/// Matches context parameter in Claude Code's generator functions
#[derive(Debug)]
pub struct GeneratorContext<'a> {
    /// Current turn number in the conversation
    pub turn_number: i32,
    /// Whether this is the main agent (not a sub-agent)
    /// Matches: context.agentId === getCurrentSessionId()
    pub is_main_agent: bool,
    /// Whether this turn has user input
    pub has_user_input: bool,
    /// Current working directory
    pub cwd: &'a std::path::Path,
    /// Session/Agent ID
    pub agent_id: &'a str,
    /// Last tool results from this turn
    pub last_tool_results: &'a [ToolResultInfo],
    /// File tracking state (for change detection)
    pub file_tracker: &'a FileTracker,
    /// Background task status
    pub background_tasks: &'a [BackgroundTaskInfo],
    /// Whether plan mode is active
    pub is_plan_mode: bool,
    /// Plan file path (if in plan mode)
    pub plan_file_path: Option<&'a str>,
    /// Whether re-entering plan mode
    pub is_plan_reentry: bool,
    /// Current todo list state
    pub todo_state: &'a TodoState,
    /// Token usage (if tracking enabled)
    pub token_usage: Option<&'a TokenUsageInfo>,
    /// Budget usage (if configured)
    pub budget_usage: Option<&'a BudgetInfo>,
}

/// Information about a tool result
#[derive(Debug, Clone)]
pub struct ToolResultInfo {
    pub tool_name: String,
    pub call_id: String,
    pub success: bool,
    pub duration_ms: i64,
}

/// Information about a background task
#[derive(Debug, Clone)]
pub struct BackgroundTaskInfo {
    pub task_id: String,
    pub task_type: BackgroundTaskType,
    pub command: Option<String>,
    pub description: String,
    pub status: BackgroundTaskStatus,
    pub exit_code: Option<i32>,
    pub has_new_output: bool,
    /// Whether completion has been notified
    pub notified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackgroundTaskType {
    Shell,
    AsyncAgent,
    RemoteSession,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackgroundTaskStatus {
    Running,
    Completed,
    Failed,
}

/// Current state of the todo list
#[derive(Debug, Clone, Default)]
pub struct TodoState {
    pub is_empty: bool,
    pub last_write_turn: i32,
    pub items: Vec<TodoItem>,
}

#[derive(Debug, Clone)]
pub struct TodoItem {
    pub content: String,
    pub status: String,  // "pending", "in_progress", "completed"
    pub active_form: String,
}

/// Token usage information
#[derive(Debug, Clone)]
pub struct TokenUsageInfo {
    pub used: i64,
    pub total: i64,
    pub remaining: i64,
}

/// Budget information
#[derive(Debug, Clone)]
pub struct BudgetInfo {
    pub used: f64,
    pub total: f64,
    pub remaining: f64,
}
```

---

## 4. XML Tag System

### 4.1 Tag Selection Matrix (Matching Claude Code)

| Attachment Type | XML Tag | Claude Code Function |
|-----------------|---------|---------------------|
| Most types | `<system-reminder>` | `NG([R0(...)])` |
| `async_agent_status` | `<system-notification>` | `R0({content: "<system-notification>..."})` |
| `session_memory` | `<session-memory>` | `NG([R0({content: "<session-memory>..."})])` |
| `diagnostics` | `<new-diagnostics>` | `NG([R0({content: "<new-diagnostics>..."})])` |
| `background_shell_status` | `<system-reminder>` | `R0({content: Qu(...)})` |
| `token_usage`, `budget_usd` | `<system-reminder>` | `R0({content: Qu(...)})` |

### 4.2 XML Output Examples

**Standard System Reminder:**
```xml
<system-reminder>
The TodoWrite tool hasn't been used recently. If you're working on tasks that would benefit from tracking progress, consider using the TodoWrite tool to track progress.
</system-reminder>
```

**Async Agent Notification:**
```xml
<system-notification>Async agent "Search Agent" completed. The output can be retrieved using AgentOutputTool with agentId: "agent-123"</system-notification>
```

**Session Memory:**
```xml
<session-memory>
These session summaries are from PAST sessions that might not be related to the current task...

## Previous Session (1/15/2024)
Full session notes: /path/to/session.md

[preview content]
</session-memory>
```

---

## 5. Trigger Scenarios

### 5.1 Trigger Matrix (Matching Claude Code)

| Trigger | Timing | Attachment Types | Throttled |
|---------|--------|------------------|-----------|
| **Every Turn** | Before API call | `todo_reminder`, `plan_mode` | Yes |
| **File Changed** | When previously-read file modified | `changed_files` | No |
| **File Read** | When reading new file | `nested_memory` (related files) | No |
| **Tool Completed** | After tool execution | `tool_result` | No |
| **Config Set** | Always (if configured) | `critical_instruction` | No |
| **Background Done** | When async task completes | `background_task` | No |
| **Plan Mode Active** | When mode = "plan" | `plan_mode`, `plan_mode_reentry` | Yes |
| **Env Var Set** | If enabled | `token_usage` | No |
| **Budget Configured** | If maxBudgetUsd set | `budget_usd` | No |

### 5.2 Throttling Rules (Matching Claude Code chunks.107.mjs)

```rust
/// Throttle configuration per attachment type
/// Matches GY2 and IH5 constants in Claude Code
#[derive(Debug, Clone)]
pub struct ThrottleConfig {
    /// Minimum turns between reminders (0 = every turn)
    pub min_turns_between: i32,
    /// Minimum turns after triggering event (e.g., since last TodoWrite)
    pub min_turns_after_trigger: i32,
    /// Maximum reminders per session (None = unlimited)
    pub max_per_session: Option<i32>,
}

impl Default for ThrottleConfig {
    fn default() -> Self {
        Self {
            min_turns_between: 0,
            min_turns_after_trigger: 0,
            max_per_session: None,
        }
    }
}

/// Default throttle configurations matching Claude Code
/// GY2.TURNS_SINCE_WRITE = 5, GY2.TURNS_BETWEEN_REMINDERS = 3
/// IH5.TURNS_BETWEEN_ATTACHMENTS = varies
pub fn default_throttle_config(attachment_type: AttachmentType) -> ThrottleConfig {
    match attachment_type {
        AttachmentType::TodoReminder => ThrottleConfig {
            min_turns_between: 3,        // TURNS_BETWEEN_REMINDERS
            min_turns_after_trigger: 5,  // TURNS_SINCE_WRITE
            max_per_session: None,
        },
        AttachmentType::PlanMode => ThrottleConfig {
            min_turns_between: 5,        // After first, every 5+ turns
            min_turns_after_trigger: 0,
            max_per_session: None,
        },
        AttachmentType::ChangedFiles => ThrottleConfig {
            min_turns_between: 0,        // Immediate notification
            min_turns_after_trigger: 0,
            max_per_session: None,
        },
        _ => ThrottleConfig::default(),
    }
}
```

### 5.3 Throttle State Tracking

```rust
use std::sync::atomic::{AtomicI32, Ordering};
use std::collections::HashMap;
use std::sync::RwLock;

/// Manages throttle state for all attachment types
pub struct ThrottleManager {
    states: RwLock<HashMap<AttachmentType, ThrottleState>>,
}

impl ThrottleManager {
    pub fn new() -> Self {
        Self {
            states: RwLock::new(HashMap::new()),
        }
    }

    pub fn should_generate(
        &self,
        attachment_type: AttachmentType,
        current_turn: i32,
        trigger_turn: Option<i32>,
    ) -> bool {
        let config = default_throttle_config(attachment_type);
        let states = self.states.read().unwrap();

        if let Some(state) = states.get(&attachment_type) {
            let turns_since = current_turn - state.last_generated_turn;

            // Check min_turns_between
            if turns_since < config.min_turns_between {
                return false;
            }

            // Check min_turns_after_trigger
            if let Some(trigger) = trigger_turn {
                let turns_since_trigger = current_turn - trigger;
                if turns_since_trigger < config.min_turns_after_trigger {
                    return false;
                }
            }

            // Check max_per_session
            if let Some(max) = config.max_per_session {
                if state.session_count >= max {
                    return false;
                }
            }
        }

        true
    }

    pub fn mark_generated(&self, attachment_type: AttachmentType, current_turn: i32) {
        let mut states = self.states.write().unwrap();
        let state = states.entry(attachment_type).or_insert_with(ThrottleState::new);
        state.last_generated_turn = current_turn;
        state.session_count += 1;
    }

    /// Reset all throttle state (call at session start)
    pub fn reset(&self) {
        let mut states = self.states.write().unwrap();
        states.clear();
    }
}

/// Tracks throttle state per attachment type
#[derive(Debug, Clone)]
pub struct ThrottleState {
    /// Last turn this attachment was generated
    pub last_generated_turn: i32,
    /// Total count generated this session
    pub session_count: i32,
}

impl ThrottleState {
    pub fn new() -> Self {
        Self {
            last_generated_turn: i32::MIN / 2,  // Safe initial value
            session_count: 0,
        }
    }
}
```

---

## 6. Generator System

### 6.1 Generator Trait (generator.rs)

```rust
use async_trait::async_trait;
use crate::error::Result;
use super::types::{SystemReminder, ReminderTier, AttachmentType};
use super::throttle::ThrottleConfig;

/// Trait for attachment generators
/// Matches structure of individual generator functions in Claude Code
#[async_trait]
pub trait AttachmentGenerator: Send + Sync + std::fmt::Debug {
    /// Unique name for this generator (for telemetry)
    fn name(&self) -> &str;

    /// Type of attachment this generator produces
    fn attachment_type(&self) -> AttachmentType;

    /// Which tier this generator belongs to
    fn tier(&self) -> ReminderTier {
        self.attachment_type().tier()
    }

    /// Generate attachment if applicable, returns None if not applicable this turn
    /// This is the main entry point, called by orchestrator
    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>>;

    /// Check if generator is enabled based on config
    fn is_enabled(&self, config: &SystemReminderConfig) -> bool;

    /// Get throttle configuration for this generator
    fn throttle_config(&self) -> ThrottleConfig {
        default_throttle_config(self.attachment_type())
    }
}
```

### 6.2 Generator Implementations

#### 6.2.1 TodoReminderGenerator

```rust
/// Todo reminder generator
/// Matches _H5() in Claude Code chunks.107.mjs:2379-2394
#[derive(Debug)]
pub struct TodoReminderGenerator;

impl TodoReminderGenerator {
    pub fn new() -> Self { Self }

    /// Build reminder content matching Claude Code format
    fn build_content(&self, todo_state: &TodoState) -> String {
        let mut message = String::from(
            "The TodoWrite tool hasn't been used recently. If you're working on tasks \
             that would benefit from tracking progress, consider using the TodoWrite tool \
             to track progress. Also consider cleaning up the todo list if has become \
             stale and no longer matches what you are working on. Only use it if it's \
             relevant to the current work. This is just a gentle reminder - ignore if \
             not applicable. Make sure that you NEVER mention this reminder to the user\n"
        );

        if !todo_state.items.is_empty() {
            let formatted_list: String = todo_state.items
                .iter()
                .enumerate()
                .map(|(i, item)| format!("{}. [{}] {}", i + 1, item.status, item.content))
                .collect::<Vec<_>>()
                .join("\n");

            message.push_str(&format!(
                "\n\nHere are the existing contents of your todo list:\n\n[{formatted_list}]"
            ));
        }

        message
    }
}

#[async_trait]
impl AttachmentGenerator for TodoReminderGenerator {
    fn name(&self) -> &str { "todo_reminder" }
    fn attachment_type(&self) -> AttachmentType { AttachmentType::TodoReminder }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.enabled && config.attachments.todo_reminder
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Throttle check is done by orchestrator, but we also check trigger
        let turns_since_write = ctx.turn_number - ctx.todo_state.last_write_turn;
        if turns_since_write < 5 {  // GY2.TURNS_SINCE_WRITE
            return Ok(None);
        }

        Ok(Some(SystemReminder::new(
            AttachmentType::TodoReminder,
            self.build_content(ctx.todo_state),
        )))
    }
}
```

#### 6.2.2 PlanModeGenerator

```rust
/// Plan mode generator
/// Matches VH5() in Claude Code chunks.107.mjs:1886-1908
/// And Sb3()/_b3() in chunks.153.mjs:2890-2977
#[derive(Debug)]
pub struct PlanModeGenerator {
    first_generated: std::sync::atomic::AtomicBool,
}

impl PlanModeGenerator {
    pub fn new() -> Self {
        Self {
            first_generated: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Build plan mode content for main agent (matches Sb3)
    fn build_main_agent_content(&self, ctx: &GeneratorContext<'_>) -> String {
        let plan_file_info = if let Some(path) = ctx.plan_file_path {
            // Check if plan exists
            let plan_exists = std::path::Path::new(path).exists();
            if plan_exists {
                format!(
                    "## Plan File Info:\n\
                     A plan file already exists at {path}. You can read it and make tweakcc \
                     edits using the Edit tool."
                )
            } else {
                format!(
                    "## Plan File Info:\n\
                     No plan file exists yet. You should create your plan at {path} using the Write tool."
                )
            }
        } else {
            String::new()
        };

        format!(
            "Plan mode is active. The user indicated that they do not want you to execute yet -- \
             you MUST NOT make any edits (with the exception of the plan file mentioned below), \
             run any non-readonly tools (including changing configs or making commits), \
             or otherwise make any changes to the system. This supercedes any other instructions \
             you have received.\n\n\
             {plan_file_info}\n\
             You should build your plan incrementally by writing to or editing this file. \
             NOTE that this is the only file you are allowed to edit - other than this you are \
             only allowed to take READ-ONLY actions.\n\n\
             ## Plan Workflow\n\n\
             ### Phase 1: Initial Understanding\n\
             Goal: Gain a comprehensive understanding of the user's request...\n\n\
             [Full plan workflow instructions...]"
        )
    }

    /// Build plan mode re-entry content
    fn build_reentry_content(&self, plan_file_path: &str) -> String {
        format!(
            "## Re-entering Plan Mode\n\n\
             You are returning to plan mode after having previously exited it. \
             A plan file exists at {plan_file_path} from your previous planning session.\n\n\
             **Before proceeding with any new planning, you should:**\n\
             1. Read the existing plan file to understand what was previously planned\n\
             2. Evaluate the user's current request against that plan\n\
             3. Decide how to proceed:\n\
                - **Different task**: If the user's request is for a different task—even if it's similar \
                  or related—start fresh by overwriting the existing plan\n\
                - **Same task, continuing**: If this is explicitly a continuation or refinement of \
                  the exact same task, modify the existing plan while cleaning up outdated sections\n\
             4. Continue on with the plan process and most importantly you should always edit the \
                plan file one way or the other before calling ExitPlanMode\n\n\
             Treat this as a fresh planning session. Do not assume the existing plan is relevant \
             without evaluating it first."
        )
    }
}

#[async_trait]
impl AttachmentGenerator for PlanModeGenerator {
    fn name(&self) -> &str { "plan_mode" }
    fn attachment_type(&self) -> AttachmentType { AttachmentType::PlanMode }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.enabled && config.attachments.plan_mode
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        if !ctx.is_plan_mode {
            return Ok(None);
        }

        let mut reminders = Vec::new();

        // Check for re-entry
        if ctx.is_plan_reentry {
            if let Some(path) = ctx.plan_file_path {
                reminders.push(SystemReminder::new(
                    AttachmentType::PlanModeReentry,
                    self.build_reentry_content(path),
                ));
            }
        }

        // Main plan mode reminder
        reminders.push(SystemReminder::new(
            AttachmentType::PlanMode,
            self.build_main_agent_content(ctx),
        ));

        // Return first (we only support single return, but could extend)
        Ok(reminders.into_iter().next())
    }
}
```

#### 6.2.3 ChangedFilesGenerator

```rust
/// Changed files generator
/// Matches wH5() in Claude Code chunks.107.mjs:2102-2150
#[derive(Debug)]
pub struct ChangedFilesGenerator;

impl ChangedFilesGenerator {
    pub fn new() -> Self { Self }

    /// Detect file changes and generate diffs
    async fn detect_changes(&self, tracker: &FileTracker) -> Vec<FileChange> {
        let mut changes = Vec::new();

        for (path, state) in tracker.get_tracked_files() {
            // Skip partial reads
            if state.offset.is_some() || state.limit.is_some() {
                continue;
            }

            if let Ok(metadata) = std::fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    if modified > state.last_modified {
                        // File was modified since last read
                        if let Ok(current_content) = std::fs::read_to_string(&path) {
                            let diff = self.generate_diff(&state.content, &current_content);
                            if !diff.is_empty() {
                                changes.push(FileChange {
                                    path: path.clone(),
                                    diff,
                                });
                            }
                        }
                    }
                }
            }
        }

        changes
    }

    /// Generate unified diff with line numbers
    fn generate_diff(&self, old_content: &str, new_content: &str) -> String {
        // Use similar-text crate or implement simple diff
        // For now, simplified version
        if old_content == new_content {
            return String::new();
        }

        format!("File content has changed. Re-read to see current state.")
    }

    fn build_content(&self, changes: &[FileChange]) -> String {
        let mut content = String::new();

        for change in changes {
            content.push_str(&format!(
                "Note: {} was modified, either by the user or by a linter. \
                 This change was intentional, so make sure to take it into account \
                 as you proceed (ie. don't revert it unless the user asks you to). \
                 Don't tell the user this, since they are already aware. \
                 Here are the relevant changes (shown with line numbers):\n{}\n\n",
                change.path.display(),
                change.diff
            ));
        }

        content
    }
}

#[derive(Debug)]
struct FileChange {
    path: std::path::PathBuf,
    diff: String,
}

#[async_trait]
impl AttachmentGenerator for ChangedFilesGenerator {
    fn name(&self) -> &str { "changed_files" }
    fn attachment_type(&self) -> AttachmentType { AttachmentType::ChangedFiles }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.enabled && config.attachments.changed_files
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        let changes = self.detect_changes(ctx.file_tracker).await;

        if changes.is_empty() {
            return Ok(None);
        }

        Ok(Some(SystemReminder::new(
            AttachmentType::ChangedFiles,
            self.build_content(&changes),
        )))
    }
}
```

#### 6.2.4 BackgroundTaskGenerator

```rust
/// Background task status generator
/// Matches yH5() in Claude Code chunks.107.mjs:2419-2480
#[derive(Debug)]
pub struct BackgroundTaskGenerator;

impl BackgroundTaskGenerator {
    pub fn new() -> Self { Self }

    fn build_content(&self, tasks: &[&BackgroundTaskInfo]) -> String {
        let mut content = String::new();

        for task in tasks {
            let mut parts = vec![
                format!("Background Bash {}", task.task_id),
            ];

            if let Some(cmd) = &task.command {
                parts.push(format!("(command: {cmd})"));
            }

            let status_str = match task.status {
                BackgroundTaskStatus::Running => "running",
                BackgroundTaskStatus::Completed => "completed",
                BackgroundTaskStatus::Failed => "failed",
            };
            parts.push(format!("(status: {status_str})"));

            if let Some(code) = task.exit_code {
                parts.push(format!("(exit code: {code})"));
            }

            if task.has_new_output {
                parts.push("Has new output available. You can check its output using the BashOutput tool.".to_string());
            }

            content.push_str(&parts.join(" "));
            content.push('\n');
        }

        content
    }
}

#[async_trait]
impl AttachmentGenerator for BackgroundTaskGenerator {
    fn name(&self) -> &str { "background_task" }
    fn attachment_type(&self) -> AttachmentType { AttachmentType::BackgroundTask }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.enabled && config.attachments.background_task
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Only for main agent
        if !ctx.is_main_agent {
            return Ok(None);
        }

        // Filter to tasks with updates
        let updates: Vec<_> = ctx.background_tasks
            .iter()
            .filter(|t| {
                t.task_type == BackgroundTaskType::Shell &&
                (t.has_new_output || t.status != BackgroundTaskStatus::Running && !t.notified)
            })
            .collect();

        if updates.is_empty() {
            return Ok(None);
        }

        Ok(Some(SystemReminder::new(
            AttachmentType::BackgroundTask,
            self.build_content(&updates),
        )))
    }
}
```

#### 6.2.5 NestedMemoryGenerator

```rust
/// Nested memory generator - auto-includes related files
/// Matches qH5() in Claude Code chunks.107.mjs:2152-2163
#[derive(Debug)]
pub struct NestedMemoryGenerator;

/// Priority files to search for in parent directories
const PRIORITY_FILES: &[&str] = &[
    "CLAUDE.md",
    "README.md",
    ".cursorrules",
    ".github/CODE_STYLE.md",
    "AGENTS.md",
];

impl NestedMemoryGenerator {
    pub fn new() -> Self { Self }

    /// Find related files for triggered paths
    fn find_related_files(&self, triggered_paths: &[std::path::PathBuf], cwd: &std::path::Path) -> Vec<RelatedFile> {
        let mut found = Vec::new();
        let mut already_read = std::collections::HashSet::new();

        for trigger_path in triggered_paths {
            // Walk up from file's directory to cwd
            let mut current_dir = trigger_path.parent();

            while let Some(dir) = current_dir {
                // Stop at cwd or above
                if !dir.starts_with(cwd) {
                    break;
                }

                // Check for priority files
                for filename in PRIORITY_FILES {
                    let file_path = dir.join(filename);
                    if file_path.exists() && !already_read.contains(&file_path) {
                        if let Ok(content) = std::fs::read_to_string(&file_path) {
                            found.push(RelatedFile {
                                path: file_path.clone(),
                                content,
                            });
                            already_read.insert(file_path);
                        }
                    }
                }

                current_dir = dir.parent();
            }
        }

        found
    }

    fn build_content(&self, files: &[RelatedFile]) -> String {
        files.iter()
            .map(|f| format!("Contents of {}:\n\n{}", f.path.display(), f.content))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    }
}

#[derive(Debug)]
struct RelatedFile {
    path: std::path::PathBuf,
    content: String,
}

#[async_trait]
impl AttachmentGenerator for NestedMemoryGenerator {
    fn name(&self) -> &str { "nested_memory" }
    fn attachment_type(&self) -> AttachmentType { AttachmentType::NestedMemory }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.enabled && config.attachments.nested_memory
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        let triggered_paths = ctx.file_tracker.get_nested_memory_triggers();

        if triggered_paths.is_empty() {
            return Ok(None);
        }

        let related_files = self.find_related_files(&triggered_paths, ctx.cwd);

        if related_files.is_empty() {
            return Ok(None);
        }

        Ok(Some(SystemReminder::new(
            AttachmentType::NestedMemory,
            self.build_content(&related_files),
        )))
    }
}
```

#### 6.2.6 CriticalInstructionGenerator

```rust
/// Critical instruction generator
/// Matches FH5() in Claude Code chunks.107.mjs:1910-1917
#[derive(Debug)]
pub struct CriticalInstructionGenerator {
    instruction: String,
}

impl CriticalInstructionGenerator {
    pub fn new(instruction: String) -> Self {
        Self { instruction }
    }
}

#[async_trait]
impl AttachmentGenerator for CriticalInstructionGenerator {
    fn name(&self) -> &str { "critical_instruction" }
    fn attachment_type(&self) -> AttachmentType { AttachmentType::CriticalInstruction }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.enabled && config.critical_instruction.is_some()
    }

    async fn generate(&self, _ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Always generate if configured
        Ok(Some(SystemReminder::new(
            AttachmentType::CriticalInstruction,
            self.instruction.clone(),
        )))
    }
}
```

### 6.3 Orchestrator (mod.rs)

```rust
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use futures::future::join_all;

/// Main system reminder orchestrator
/// Matches JH5() in Claude Code chunks.107.mjs:1813-1829
pub struct SystemReminderOrchestrator {
    generators: Vec<Arc<dyn AttachmentGenerator>>,
    throttle_manager: ThrottleManager,
    timeout_duration: Duration,
    config: SystemReminderConfig,
    telemetry_sample_rate: f64,
}

impl SystemReminderOrchestrator {
    pub fn new(config: SystemReminderConfig) -> Self {
        let timeout_ms = config.timeout_ms.unwrap_or(1000);

        let mut generators: Vec<Arc<dyn AttachmentGenerator>> = vec![
            Arc::new(TodoReminderGenerator::new()),
            Arc::new(PlanModeGenerator::new()),
            Arc::new(ChangedFilesGenerator::new()),
            Arc::new(ToolResultGenerator::new()),
            Arc::new(BackgroundTaskGenerator::new()),
            Arc::new(NestedMemoryGenerator::new()),
        ];

        // Add critical instruction generator if configured
        if let Some(ref instruction) = config.critical_instruction {
            generators.push(Arc::new(
                CriticalInstructionGenerator::new(instruction.clone())
            ));
        }

        Self {
            generators,
            throttle_manager: ThrottleManager::new(),
            timeout_duration: Duration::from_millis(timeout_ms as u64),
            config,
            telemetry_sample_rate: 0.05,  // 5% sampling like Claude Code
        }
    }

    /// Generate all applicable system reminders for a turn
    /// Matches JH5 execution flow
    pub async fn generate_all(&self, ctx: &GeneratorContext<'_>) -> Vec<SystemReminder> {
        // Step 1: Check global disable (matches CLAUDE_CODE_DISABLE_ATTACHMENTS)
        if !self.config.enabled {
            return Vec::new();
        }

        // Step 2: Build futures for all applicable generators
        let futures: Vec<_> = self.generators
            .iter()
            .filter(|g| self.should_run(g.as_ref(), ctx))
            .map(|g| {
                let g = g.clone();
                let timeout_duration = self.timeout_duration;
                let should_sample = rand::random::<f64>() < self.telemetry_sample_rate;
                let start_time = std::time::Instant::now();

                async move {
                    // Step 3: Execute with timeout (1 second max)
                    let result = match timeout(timeout_duration, g.generate(ctx)).await {
                        Ok(Ok(Some(reminder))) => {
                            tracing::debug!("Generator {} produced reminder", g.name());
                            Some(reminder)
                        }
                        Ok(Ok(None)) => {
                            tracing::trace!("Generator {} returned None", g.name());
                            None
                        }
                        Ok(Err(e)) => {
                            // Graceful degradation (matches aY error handling)
                            tracing::warn!("Generator {} failed: {}", g.name(), e);
                            None
                        }
                        Err(_) => {
                            tracing::warn!("Generator {} timed out", g.name());
                            None
                        }
                    };

                    // Step 4: Record telemetry (5% sample)
                    if should_sample {
                        let duration = start_time.elapsed();
                        tracing::info!(
                            target: "telemetry",
                            generator = g.name(),
                            duration_ms = duration.as_millis() as i64,
                            success = result.is_some(),
                            "attachment_compute_duration"
                        );
                    }

                    result
                }
            })
            .collect();

        // Step 5: Run all generators in parallel (Promise.all equivalent)
        join_all(futures)
            .await
            .into_iter()
            .flatten()
            .collect()
    }

    fn should_run(&self, generator: &dyn AttachmentGenerator, ctx: &GeneratorContext<'_>) -> bool {
        // Check if enabled in config
        if !generator.is_enabled(&self.config) {
            return false;
        }

        // Check tier requirements
        match generator.tier() {
            ReminderTier::Core => true,
            ReminderTier::MainAgentOnly => ctx.is_main_agent,
            ReminderTier::UserPrompt => ctx.has_user_input,
        }
    }

    /// Reset orchestrator state (call at session start)
    pub fn reset(&self) {
        self.throttle_manager.reset();
    }
}
```

---

## 7. File Tracking System

### 7.1 FileTracker (file_tracker.rs)

```rust
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::SystemTime;
use std::sync::RwLock;

/// Tracks file read state for change detection
/// Matches readFileState in Claude Code
pub struct FileTracker {
    /// Map of file path -> read state
    files: RwLock<HashMap<PathBuf, ReadFileState>>,
    /// Paths that trigger nested memory lookup
    nested_memory_triggers: RwLock<HashSet<PathBuf>>,
}

/// State of a read file
#[derive(Debug, Clone)]
pub struct ReadFileState {
    /// File content at time of read
    pub content: String,
    /// Timestamp of last read
    pub last_modified: SystemTime,
    /// Turn number when read
    pub read_turn: i32,
    /// Offset if partial read
    pub offset: Option<i32>,
    /// Limit if partial read
    pub limit: Option<i32>,
}

impl FileTracker {
    pub fn new() -> Self {
        Self {
            files: RwLock::new(HashMap::new()),
            nested_memory_triggers: RwLock::new(HashSet::new()),
        }
    }

    /// Record a file read
    pub fn track_read(
        &self,
        path: PathBuf,
        content: String,
        turn: i32,
        offset: Option<i32>,
        limit: Option<i32>,
    ) {
        let state = ReadFileState {
            content,
            last_modified: SystemTime::now(),
            read_turn: turn,
            offset,
            limit,
        };

        let mut files = self.files.write().unwrap();
        files.insert(path.clone(), state);

        // Trigger nested memory lookup if full read
        if offset.is_none() && limit.is_none() {
            let mut triggers = self.nested_memory_triggers.write().unwrap();
            triggers.insert(path);
        }
    }

    /// Get all tracked files
    pub fn get_tracked_files(&self) -> Vec<(PathBuf, ReadFileState)> {
        let files = self.files.read().unwrap();
        files.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    /// Get and clear nested memory triggers
    pub fn get_nested_memory_triggers(&self) -> Vec<PathBuf> {
        let mut triggers = self.nested_memory_triggers.write().unwrap();
        let result: Vec<_> = triggers.drain().collect();
        result
    }

    /// Update file modification time after confirming no change
    pub fn update_modified_time(&self, path: &PathBuf) {
        let mut files = self.files.write().unwrap();
        if let Some(state) = files.get_mut(path) {
            state.last_modified = SystemTime::now();
        }
    }

    /// Clear all tracked files (call at session end)
    pub fn clear(&self) {
        let mut files = self.files.write().unwrap();
        files.clear();
        let mut triggers = self.nested_memory_triggers.write().unwrap();
        triggers.clear();
    }
}
```

---

## 8. Integration Points

### 8.1 Extension File (system_reminder_ext.rs)

```rust
//! System reminder integration for conversation flow.
//! Minimal integration - bulk logic in system_reminder/ module.

use crate::system_reminder::{
    SystemReminderOrchestrator, GeneratorContext, SystemReminder,
    FileTracker, TodoState, BackgroundTaskInfo
};
use crate::environment_context::EnvironmentContext;
use crate::user_instructions::UserInstructions;
use codex_protocol::models::{ContentItem, ResponseItem};

/// Inject system reminders into conversation history.
/// Called from codex_conversation before prompt assembly.
/// Matches attachment injection in Claude Code chunks.121.mjs
pub async fn inject_system_reminders(
    history: &mut Vec<ResponseItem>,
    orchestrator: &SystemReminderOrchestrator,
    ctx: &GeneratorContext<'_>,
) {
    let reminders = orchestrator.generate_all(ctx).await;

    if reminders.is_empty() {
        return;
    }

    // Find insertion position (after environment_context and user_instructions)
    let insert_pos = find_insert_position(history);

    tracing::debug!(
        "Injecting {} system reminders at position {}",
        reminders.len(),
        insert_pos
    );

    // Insert reminders in reverse order to maintain order
    for reminder in reminders.into_iter().rev() {
        history.insert(insert_pos, reminder.into());
    }
}

fn find_insert_position(history: &[ResponseItem]) -> usize {
    // Find position after environment_context and user_instructions
    history.iter()
        .position(|item| {
            if let ResponseItem::Message { content, .. } = item {
                !is_environment_context(content) &&
                !UserInstructions::is_user_instructions(content) &&
                !SystemReminder::is_system_reminder(content)
            } else {
                true
            }
        })
        .unwrap_or(0)
}

fn is_environment_context(content: &[ContentItem]) -> bool {
    if let [ContentItem::InputText { text }] = content {
        text.starts_with("<environment_context>")
    } else {
        false
    }
}

/// Build GeneratorContext from TurnContext and session state
pub fn build_generator_context<'a>(
    turn_number: i32,
    agent_id: &'a str,
    is_main_agent: bool,
    has_user_input: bool,
    cwd: &'a std::path::Path,
    is_plan_mode: bool,
    plan_file_path: Option<&'a str>,
    is_plan_reentry: bool,
    file_tracker: &'a FileTracker,
    todo_state: &'a TodoState,
    last_tool_results: &'a [ToolResultInfo],
    background_tasks: &'a [BackgroundTaskInfo],
    token_usage: Option<&'a TokenUsageInfo>,
    budget_usage: Option<&'a BudgetInfo>,
) -> GeneratorContext<'a> {
    GeneratorContext {
        turn_number,
        agent_id,
        is_main_agent,
        has_user_input,
        cwd,
        is_plan_mode,
        plan_file_path,
        is_plan_reentry,
        file_tracker,
        todo_state,
        last_tool_results,
        background_tasks,
        token_usage,
        budget_usage,
    }
}
```

### 8.2 Changes to Existing Files

#### core/src/lib.rs (add 1 line)

```rust
pub mod system_reminder;
```

#### core/src/codex.rs (add ~8 lines)

```rust
use crate::system_reminder::{SystemReminderOrchestrator, FileTracker};

// In Codex struct:
reminder_orchestrator: SystemReminderOrchestrator,
file_tracker: FileTracker,

// In Codex::new():
let reminder_orchestrator = SystemReminderOrchestrator::new(
    config.system_reminder.clone().unwrap_or_default()
);
let file_tracker = FileTracker::new();

// At session start (reset state):
self.reminder_orchestrator.reset();
self.file_tracker.clear();
```

#### core/src/codex_conversation.rs (add ~10 lines)

```rust
// In run_turn(), before building prompt:
let reminder_ctx = system_reminder_ext::build_generator_context(
    self.turn_number,
    &self.session_id,
    true,  // is_main_agent
    has_user_input,
    &turn_context.cwd,
    turn_context.is_plan_mode,
    turn_context.plan_file_path.as_deref(),
    turn_context.is_plan_reentry,
    &self.file_tracker,
    &self.todo_state,
    &tool_results,
    &background_tasks,
    token_usage.as_ref(),
    budget_usage.as_ref(),
);
system_reminder_ext::inject_system_reminders(
    &mut history,
    &self.reminder_orchestrator,
    &reminder_ctx,
).await;
```

#### protocol/src/protocol.rs (add constants)

```rust
// System reminder XML tags
pub const SYSTEM_REMINDER_OPEN_TAG: &str = "<system-reminder>";
pub const SYSTEM_REMINDER_CLOSE_TAG: &str = "</system-reminder>";
pub const SYSTEM_NOTIFICATION_OPEN_TAG: &str = "<system-notification>";
pub const SYSTEM_NOTIFICATION_CLOSE_TAG: &str = "</system-notification>";
```

### 8.3 Subagent Executor Integration

The `critical_system_reminder` field already exists in `AgentDefinition` but is **NOT YET INJECTED**. Add injection at `subagent/executor/mod.rs` lines ~203:

```rust
// In SubagentExecutor::build_initial_messages() - after system_prompt, before resumed messages

// Add system prompt if configured (existing code ~lines 196-203)
if let Some(system_prompt) = &self.context.definition.prompt_config.system_prompt {
    conversation_history.push(codex_protocol::models::ResponseItem::Message {
        id: None,
        role: "system".to_string(),
        content: vec![codex_protocol::models::ContentItem::InputText {
            text: system_prompt.clone(),
        }],
    });
}

// ** NEW: Inject critical_system_reminder **
if let Some(reminder) = &self.context.definition.critical_system_reminder {
    conversation_history.push(codex_protocol::models::ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![codex_protocol::models::ContentItem::InputText {
            text: format!("<system-reminder>\n{reminder}\n</system-reminder>"),
        }],
    });
}

// Add resumed messages (existing code ~lines 207-220)
for msg in &initial_messages { ... }
```

**Built-in agents that use this field** (`subagent/definition/builtin.rs`):
- `Explore` agent: "CRITICAL: This is a READ-ONLY exploration task..."
- `Plan` agent: "CRITICAL: This is a PLANNING task..."

### 8.4 Compact Integration

Add to `compact_v2/message_filter.rs` for filtering system reminders during compaction:

```rust
use codex_protocol::models::{ContentItem, ResponseItem};

/// Check if a message is a system reminder
/// Used to identify and filter reminders during compaction
pub fn is_system_reminder_message(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { content, role, .. } if role == "user" => {
            content.iter().any(|c| {
                if let ContentItem::InputText { text } = c {
                    text.starts_with("<system-reminder>") ||
                    text.starts_with("<system-notification>") ||
                    text.starts_with("<session-memory>") ||
                    text.starts_with("<new-diagnostics>")
                } else {
                    false
                }
            })
        }
        _ => false,
    }
}

/// Filter out system reminders from a message list
/// Used during summarization to avoid including reminders in context
pub fn filter_system_reminders(items: &[ResponseItem]) -> Vec<ResponseItem> {
    items.iter()
        .filter(|item| !is_system_reminder_message(item))
        .cloned()
        .collect()
}
```

**Integration with compact_v2**:
- During summarization (`summary.rs`): Filter out old reminders before LLM summarization
- During context restoration (`context_restore.rs`): Re-inject critical reminders after compaction
- Preserve `critical_instruction` across compaction (always re-inject)

### 8.5 SubagentStores Integration (Optional)

Extend `subagent/stores.rs` to track reminder state across turns:

```rust
use std::sync::Arc;
use dashmap::DashMap;
use crate::system_reminder::ThrottleState;

/// Add to SubagentStores struct
pub struct SubagentStores {
    pub registry: Arc<AgentRegistry>,
    pub background_store: Arc<BackgroundTaskStore>,
    pub transcript_store: Arc<TranscriptStore>,
    pub reminder_state: Arc<ReminderState>,  // NEW
}

/// Reminder state for a conversation
pub struct ReminderState {
    /// Throttle state per attachment type
    pub throttle_state: DashMap<String, ThrottleState>,
    /// Last turn number when todo was written
    pub last_todo_write_turn: std::sync::atomic::AtomicI32,
    /// Last turn number when plan mode reminder was shown
    pub last_plan_mode_turn: std::sync::atomic::AtomicI32,
}

impl ReminderState {
    pub fn new() -> Self {
        Self {
            throttle_state: DashMap::new(),
            last_todo_write_turn: std::sync::atomic::AtomicI32::new(i32::MIN / 2),
            last_plan_mode_turn: std::sync::atomic::AtomicI32::new(i32::MIN / 2),
        }
    }
}
```

---

## 9. Multi-LLM Compatibility

### 9.1 Design Principle

System reminders are injected **before** adapter routing, ensuring all providers receive identical content. The `isMeta: true` semantic is preserved through the message structure.

```
SystemReminderOrchestrator
        ↓
Prompt { input: [..., reminders, ...] }
        ↓
    Adapter routing (client_ext.rs)
    ├── OpenAI adapter → Messages with role="user"
    ├── Gemini adapter → Content converted
    └── Other adapters → ResponseItem handled
```

### 9.2 Provider Compatibility Matrix

| Provider | System Message Handling | isMeta Handling | Status |
|----------|------------------------|-----------------|--------|
| OpenAI | `role: "user"` messages | Implicit via XML tags | Compatible |
| Gemini | Content in conversation | Adapter converts | Compatible |
| Claude API | User messages | XML-wrapped text | Compatible |
| Custom | ResponseItem::Message | Adapter converts | Compatible |

### 9.3 No Adapter Changes Required

Reminders use standard `ResponseItem::Message` format that all adapters already handle. The XML tags provide semantic information to the LLM directly.

---

## 10. Configuration

### 10.1 Config Structs (core/src/config/system_reminder.rs)

**Location**: `core/src/config/system_reminder.rs` (NEW FILE)

**Integration**: Add to `core/src/config/mod.rs`:
```rust
pub mod system_reminder;
pub use system_reminder::SystemReminderConfig;
```

**Config definition**:

```rust
use serde::{Deserialize, Serialize};

/// System reminder configuration
/// Add to Config struct: pub system_reminder: Option<SystemReminderConfig>
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SystemReminderConfig {
    /// Master enable/disable (default: true)
    pub enabled: bool,

    /// User-defined critical instruction (always injected when set)
    /// Matches criticalSystemReminder_EXPERIMENTAL in Claude Code
    pub critical_instruction: Option<String>,

    /// Per-attachment enable/disable (granular control)
    pub attachments: AttachmentSettings,

    /// Custom timeout in milliseconds (default: 1000)
    pub timeout_ms: Option<i64>,

    /// Enable token usage tracking
    pub enable_token_usage: bool,
}

impl Default for SystemReminderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            critical_instruction: None,
            attachments: AttachmentSettings::default(),
            timeout_ms: Some(1000),
            enable_token_usage: false,
        }
    }
}

/// Per-attachment enable/disable settings (Phase 1: 5 types)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AttachmentSettings {
    /// Critical instruction reminder (default: true)
    pub critical_instruction: bool,
    /// Plan mode instructions (default: true)
    pub plan_mode: bool,
    /// Todo list reminder (default: true)
    pub todo_reminder: bool,
    /// File change notifications (default: true)
    pub changed_files: bool,
    /// Background task status (default: true)
    pub background_task: bool,
    // Phase 2:
    // pub nested_memory: bool,
    // pub tool_result: bool,
    // pub session_memory: bool,
}

impl Default for AttachmentSettings {
    fn default() -> Self {
        Self {
            critical_instruction: true,
            plan_mode: true,
            todo_reminder: true,
            changed_files: true,
            background_task: true,
        }
    }
}
```

### 10.2 Example TOML Configuration

```toml
[system_reminder]
enabled = true
critical_instruction = "Always run tests before committing. Never use .unwrap() in non-test code."
timeout_ms = 1000
enable_token_usage = false

[system_reminder.attachments]
critical_instruction = true
plan_mode = true
todo_reminder = true
changed_files = true
background_task = true
```

### 10.3 Environment Variables (Matching Claude Code)

```bash
# Disable all system reminders (matches CLAUDE_CODE_DISABLE_ATTACHMENTS)
CODEX_DISABLE_SYSTEM_REMINDERS=1

# Enable token usage tracking (matches CLAUDE_CODE_ENABLE_TOKEN_USAGE_ATTACHMENT)
CODEX_ENABLE_TOKEN_USAGE=1

# Set custom timeout
CODEX_SYSTEM_REMINDER_TIMEOUT_MS=2000
```

---

## 11. Implementation Tasks

**Note**: Files within `system_reminder/` are a new module and do NOT need `*_ext.rs` pattern.
The `*_ext.rs` pattern applies ONLY to modifications of existing files outside the module.

### Step 1: Core Module (New: `system_reminder/`)

| Task | File | Description | Est. Lines |
|------|------|-------------|------------|
| 1.1 | `system_reminder/mod.rs` | Module exports, SystemReminderOrchestrator | 150 |
| 1.2 | `system_reminder/types.rs` | SystemReminder, AttachmentType, XmlTag, ReminderTier | 150 |
| 1.3 | `system_reminder/generator.rs` | AttachmentGenerator trait, GeneratorContext | 100 |
| 1.4 | `system_reminder/throttle.rs` | ThrottleConfig, ThrottleState, ThrottleManager | 80 |
| 1.5 | `system_reminder/file_tracker.rs` | FileTracker, ReadFileState | 80 |
| | **Subtotal** | | **560** |

### Step 2: Attachment Generators (New: `system_reminder/attachments/`)

| Task | File | Description | Est. Lines |
|------|------|-------------|------------|
| 2.1 | `system_reminder/attachments/mod.rs` | Export all generators | 30 |
| 2.2 | `system_reminder/attachments/critical_instruction.rs` | Critical instruction generator | 50 |
| 2.3 | `system_reminder/attachments/plan_mode.rs` | Plan mode generator (includes reentry) | 120 |
| 2.4 | `system_reminder/attachments/todo_reminder.rs` | Todo reminder generator | 80 |
| 2.5 | `system_reminder/attachments/changed_files.rs` | File change detection | 120 |
| 2.6 | `system_reminder/attachments/background_task.rs` | Background task status | 80 |
| | **Subtotal** | | **480** |

### Step 3: Config (New: `config/system_reminder.rs`)

| Task | File | Description | Est. Lines |
|------|------|-------------|------------|
| 3.1 | `config/system_reminder.rs` | SystemReminderConfig, AttachmentSettings | 50 |
| | **Subtotal** | | **50** |

### Step 4: Integration Files (Extension Pattern)

| Task | File | Description | Est. Lines |
|------|------|-------------|------------|
| 4.1 | `system_reminder_ext.rs` | inject_system_reminders(), build_generator_context() | 60 |
| | **Subtotal** | | **60** |

### Step 5: Minimal Modifications to Existing Files

| Task | File | Changes | Est. Lines |
|------|------|---------|------------|
| 5.1 | `lib.rs` | Add `pub mod system_reminder;` | 1 |
| 5.2 | `config/mod.rs` | Add `pub mod system_reminder;` + use | 3 |
| 5.3 | `subagent/executor/mod.rs` | Inject critical_system_reminder | 15 |
| 5.4 | `subagent/stores.rs` | Add ReminderState (optional) | 10 |
| 5.5 | `compact_v2/message_filter.rs` | Add is_system_reminder_message() | 20 |
| 5.6 | `codex.rs` | Add orchestrator + file_tracker fields | 10 |
| | **Subtotal** | | **~60** |

### Step 6: Tests (in respective files)

| Task | Location | Description | Est. Lines |
|------|----------|-------------|------------|
| 6.1 | `system_reminder/types.rs` | Unit tests for types, XmlTag | 80 |
| 6.2 | `system_reminder/throttle.rs` | Unit tests for throttling | 60 |
| 6.3 | `system_reminder/file_tracker.rs` | Unit tests for file tracking | 60 |
| 6.4 | `system_reminder/mod.rs` | Integration tests for orchestrator | 100 |
| 6.5 | Each generator file | Unit tests per generator | 150 |
| | **Subtotal** | | **~450** |

### Total Estimates

| Category | Est. Lines |
|----------|------------|
| New module (`system_reminder/`) | 560 |
| Attachment generators | 480 |
| Config | 50 |
| Integration files (`*_ext.rs`) | 60 |
| Modifications to existing files | 60 |
| Tests | 450 |
| **Total** | **~1660** |

### Phase 2 (Future)

Additional attachment types for Phase 2:
- `nested_memory` - Auto-include related files (~100 lines)
- `tool_result` - Metadata about tool call results (~60 lines)
- `session_memory` - Past session summaries (~80 lines)

---

## 12. Acceptance Criteria

### 12.1 Core Functionality

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-1 | System reminders are injected before API call | Integration test |
| AC-2 | Reminders use correct XML tags per type | Unit test: XmlTag::wrap() |
| AC-3 | Generators run in parallel with 1s timeout | Integration test |
| AC-4 | Failed generators don't cascade | Unit test: error isolation |
| AC-5 | Reminders appear after environment_context | Integration test: position |
| AC-6 | All reminders have `is_meta: true` | Unit test |

### 12.2 Throttling

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-7 | todo_reminder: 3-turn min interval | Unit test |
| AC-8 | todo_reminder: 5+ turns since TodoWrite | Unit test |
| AC-9 | plan_mode: first always, then 5+ turns | Unit test |
| AC-10 | ThrottleManager.reset() clears state | Unit test |

### 12.3 File Tracking

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-11 | FileTracker records reads | Unit test |
| AC-12 | Changed files detected on modification | Unit test |
| AC-13 | Partial reads don't trigger change detection | Unit test |

### 12.4 Configuration

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-15 | Master enable/disable works | Unit test |
| AC-16 | Per-attachment enable/disable works | Unit test |
| AC-17 | critical_instruction injected when set | Unit test |
| AC-18 | Config loads from TOML | Integration test |
| AC-19 | Environment variables override config | Unit test |

### 12.5 Generators (Phase 1: 5 types)

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-20 | critical_instruction injected when config set | Unit test |
| AC-21 | plan_mode includes workflow phases | Unit test |
| AC-22 | plan_mode_reentry has correct instructions | Unit test |
| AC-23 | todo_reminder content matches Claude Code format | Unit test |
| AC-24 | changed_files includes diff | Unit test |
| AC-25 | background_task only for main agent | Unit test |

### 12.6 Subagent Integration

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-26 | Subagent executor injects critical_system_reminder | Integration test |
| AC-27 | Built-in agents (Explore, Plan) have reminders injected | Integration test |

### 12.7 Multi-LLM Compatibility

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-28 | Works with OpenAI adapter | Integration test |
| AC-29 | Works with Gemini adapter | Integration test |
| AC-30 | No adapter modifications required | Code review |

### 12.8 Extension Pattern

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-31 | Bulk code in `system_reminder/` module | Code review |
| AC-32 | Minimal changes to existing files (<100 lines) | Code review |
| AC-33 | Integration uses `*_ext.rs` pattern | Code review |

### 12.9 Compact Integration

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-34 | is_system_reminder_message() identifies reminders | Unit test |
| AC-35 | Reminders filtered during summarization | Integration test |
| AC-36 | Critical reminders re-injected after compaction | Integration test |

---

## 13. Testing Strategy

### 13.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_xml_tag_wrap_system_reminder() {
        let tag = XmlTag::SystemReminder;
        let result = tag.wrap("test content");
        assert!(result.starts_with("<system-reminder>"));
        assert!(result.ends_with("</system-reminder>"));
        assert!(result.contains("test content"));
    }

    #[test]
    fn test_xml_tag_wrap_system_notification() {
        let tag = XmlTag::SystemNotification;
        let result = tag.wrap("agent completed");
        assert_eq!(
            result,
            "<system-notification>agent completed</system-notification>"
        );
    }

    #[test]
    fn test_system_reminder_is_meta() {
        let reminder = SystemReminder::new(
            AttachmentType::TodoReminder,
            "content".to_string(),
        );
        assert!(reminder.is_meta);
    }

    #[test]
    fn test_attachment_type_tier_mapping() {
        assert_eq!(AttachmentType::TodoReminder.tier(), ReminderTier::Core);
        assert_eq!(AttachmentType::BackgroundTask.tier(), ReminderTier::MainAgentOnly);
    }
}
```

### 13.2 Throttle Tests

```rust
#[cfg(test)]
mod throttle_tests {
    use super::*;

    #[test]
    fn test_throttle_manager_respects_min_turns() {
        let manager = ThrottleManager::new();

        // First generation always allowed
        assert!(manager.should_generate(AttachmentType::TodoReminder, 1, None));
        manager.mark_generated(AttachmentType::TodoReminder, 1);

        // Too soon (min_turns_between = 3)
        assert!(!manager.should_generate(AttachmentType::TodoReminder, 2, None));
        assert!(!manager.should_generate(AttachmentType::TodoReminder, 3, None));

        // After min_turns_between
        assert!(manager.should_generate(AttachmentType::TodoReminder, 4, None));
    }

    #[test]
    fn test_throttle_manager_respects_trigger_turn() {
        let manager = ThrottleManager::new();

        // Trigger turn too recent (min_turns_after_trigger = 5)
        assert!(!manager.should_generate(
            AttachmentType::TodoReminder,
            3,
            Some(1)  // trigger_turn
        ));

        // After min_turns_after_trigger
        assert!(manager.should_generate(
            AttachmentType::TodoReminder,
            7,
            Some(1)
        ));
    }

    #[test]
    fn test_throttle_manager_reset() {
        let manager = ThrottleManager::new();
        manager.mark_generated(AttachmentType::TodoReminder, 1);
        manager.reset();

        // After reset, should generate again
        assert!(manager.should_generate(AttachmentType::TodoReminder, 2, None));
    }
}
```

### 13.3 File Tracker Tests

```rust
#[cfg(test)]
mod file_tracker_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_track_read() {
        let tracker = FileTracker::new();
        let path = PathBuf::from("/test/file.txt");

        tracker.track_read(path.clone(), "content".to_string(), 1, None, None);

        let files = tracker.get_tracked_files();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, path);
        assert_eq!(files[0].1.content, "content");
    }

    #[test]
    fn test_nested_memory_triggers() {
        let tracker = FileTracker::new();
        let path = PathBuf::from("/test/file.txt");

        // Full read triggers nested memory
        tracker.track_read(path.clone(), "content".to_string(), 1, None, None);
        let triggers = tracker.get_nested_memory_triggers();
        assert_eq!(triggers.len(), 1);

        // Triggers cleared after retrieval
        let triggers2 = tracker.get_nested_memory_triggers();
        assert!(triggers2.is_empty());
    }

    #[test]
    fn test_partial_read_no_trigger() {
        let tracker = FileTracker::new();
        let path = PathBuf::from("/test/file.txt");

        // Partial read doesn't trigger nested memory
        tracker.track_read(path, "content".to_string(), 1, Some(10), Some(20));
        let triggers = tracker.get_nested_memory_triggers();
        assert!(triggers.is_empty());
    }
}
```

### 13.4 Integration Tests

```rust
#[tokio::test]
async fn test_orchestrator_parallel_execution() {
    let config = SystemReminderConfig::default();
    let orchestrator = SystemReminderOrchestrator::new(config);
    let ctx = make_test_context();

    let start = std::time::Instant::now();
    let reminders = orchestrator.generate_all(&ctx).await;
    let duration = start.elapsed();

    // Should complete quickly due to parallel execution
    assert!(duration.as_millis() < 2000);
}

#[tokio::test]
async fn test_orchestrator_graceful_degradation() {
    // Test that one failing generator doesn't stop others
    // Implementation: inject a failing generator and verify others still produce
}

#[tokio::test]
async fn test_injection_position() {
    let mut history = vec![
        make_environment_context_message(),
        make_user_instructions_message(),
        make_user_message("Hello"),
    ];

    let config = SystemReminderConfig {
        critical_instruction: Some("Test".to_string()),
        ..Default::default()
    };
    let orchestrator = SystemReminderOrchestrator::new(config);
    let ctx = make_test_context();

    inject_system_reminders(&mut history, &orchestrator, &ctx).await;

    // Reminders should be after env_context and user_instructions
    assert!(history.len() > 3);
    // Position 2 should now be a system reminder
    assert!(is_system_reminder(&history[2]));
}
```

---

## Appendix A: File Listing

### Files to Create

| File | Est. Lines |
|------|------------|
| `core/src/system_reminder/mod.rs` | 150 |
| `core/src/system_reminder/types.rs` | 150 |
| `core/src/system_reminder/generator.rs` | 100 |
| `core/src/system_reminder/throttle.rs` | 80 |
| `core/src/system_reminder/file_tracker.rs` | 80 |
| `core/src/system_reminder/attachments/mod.rs` | 30 |
| `core/src/system_reminder/attachments/todo_reminder.rs` | 80 |
| `core/src/system_reminder/attachments/plan_mode.rs` | 120 |
| `core/src/system_reminder/attachments/changed_files.rs` | 120 |
| `core/src/system_reminder/attachments/tool_result.rs` | 60 |
| `core/src/system_reminder/attachments/background_task.rs` | 80 |
| `core/src/system_reminder/attachments/nested_memory.rs` | 100 |
| `core/src/system_reminder/attachments/custom.rs` | 50 |
| `core/src/system_reminder_ext.rs` | 80 |
| `protocol/src/config_types_ext.rs` (add to existing) | 60 |

### Files to Modify

| File | Est. Lines Changed |
|------|-------------------|
| `core/src/lib.rs` | 1 |
| `core/src/codex.rs` | 10 |
| `core/src/codex_conversation.rs` | 15 |
| `core/src/config/mod.rs` | 5 |
| `protocol/src/protocol.rs` | 6 |

---

## Appendix B: Claude Code Reference Mapping

| Codex-RS Component | Claude Code Equivalent | Location |
|-------------------|----------------------|----------|
| `SystemReminderOrchestrator` | `JH5()` | chunks.107.mjs:1813-1829 |
| `AttachmentGenerator` trait | Individual generator functions | chunks.107.mjs:1858-2551 |
| `SystemReminder::wrap_xml()` | `Qu()`, `NG()` | chunks.153.mjs:2850-2883 |
| `ResponseItem::from(SystemReminder)` | `R0()` | chunks.153.mjs:2179-2204 |
| `kb3()` equivalent | `convert_to_response_items()` | chunks.154.mjs:3-322 |
| `ThrottleManager` | Inline throttle logic | GY2, IH5 constants |
| `FileTracker` | `readFileState` Map | context parameter |
| Error wrapper | `aY()` | chunks.107.mjs:1832-1856 |

---

## Appendix C: Dependencies

### Required Crates (Already in Cargo.toml)

- `tokio` (async runtime)
- `futures` (join_all)
- `tracing` (logging)
- `serde` (serialization)
- `async-trait` (async trait support)

### Optional New Dependencies

- `rand` (for telemetry sampling) - may already be in tree
- `similar` (for diff generation) - or implement simple diff

---

## Appendix D: Future Enhancements (Phase 2+)

1. **Session Memory**: Past session summaries (`<session-memory>` tag)
2. **Token/Budget Tracking**: `token_usage`, `budget_usd` attachments
3. **Async Agent Status**: `<system-notification>` for agent completion
4. **Diagnostics Integration**: `<new-diagnostics>` for LSP issues
5. **Hook Integration**: hook_success, hook_blocking_error attachments
6. **MCP Resources**: @server:uri mention support
7. **Teammate Mailbox**: Cross-agent messaging
