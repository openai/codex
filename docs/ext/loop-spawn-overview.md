# Loop Driver & SpawnTask Overview

> Architecture, implementation guide, and reference for loop-driven execution and SpawnTask spawning in codex-rs.

**Related Documents:**
- [Loop Driver Design](./loop-driver.md) - LoopCondition, LoopPromptBuilder, LoopDriver
- [SpawnTask Design](./spawn-task.md) - SpawnTask trait, SpawnAgent, SpawnWorkflow, Manager, Worktree, Merge

---

## Table of Contents

1. [Naming Clarification](#1-naming-clarification)
2. [Problem & Solution](#2-problem--solution)
3. [Architecture](#3-architecture)
4. [Architecture Decision](#4-architecture-decision)
5. [File Structure](#5-file-structure)
6. [Implementation Guide](#6-implementation-guide)
7. [Testing Strategy](#7-testing-strategy)
8. [Error Handling](#8-error-handling)
9. [Quick Reference](#9-quick-reference)

---

## 1. Naming Clarification

**Key Insight**: "Background" is an execution MODE (async vs sync), not a task TYPE. "SpawnTask" is a **generic framework** that can spawn different task types.

| Term | Meaning |
|------|---------|
| **SpawnTask** | Trait for all spawnable task types (generic interface) |
| **SpawnAgent** | Task type: Full Codex agent with loop-driven execution |
| **SpawnWorkflow** | Task type: YAML workflow executor (future) |
| **SpawnTaskManager** | Unified lifecycle manager for ALL spawn task types |
| **LoopDriver** | Generic iteration mechanism that can drive ANY Codex session |

### Task Type Hierarchy

```
SpawnTask (trait)
├── SpawnAgent (impl SpawnTask)     ← Phase 1: Full Codex agent with loop
│   └── Uses LoopDriver for iteration
├── SpawnWorkflow (impl SpawnTask)  ← Future: YAML workflow executor
│   └── Uses workflow executor
└── Future task types...
```

### Framework vs Implementation

```
┌─────────────────────────────────────────────────────────────┐
│                    SpawnTask Framework                       │
│  ┌─────────────────────────────────────────────────────────┐│
│  │ SpawnTaskManager (unified lifecycle)                    ││
│  │ - start(Box<dyn SpawnTask>, use_worktree: bool)        ││
│  │ - list() / status() / kill() / drop()                  ││
│  └─────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────┐│
│  │ WorktreeManager (GENERIC - for ALL spawn types)        ││
│  │ - create_worktree() / cleanup_worktree()               ││
│  └─────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────┐│
│  │ SpawnTaskMetadata (unified persistence)                ││
│  │ - task_id, task_type, status, worktree_info, ...       ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
     ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
     │  SpawnAgent  │ │SpawnWorkflow │ │ Future Type  │
     │impl SpawnTask│ │impl SpawnTask│ │impl SpawnTask│
     └──────────────┘ └──────────────┘ └──────────────┘
```

---

## 2. Problem & Solution

### 2.1 Problem Statement

Current codex agent runs a single turn and waits for user input. For complex tasks that require iterative refinement (like implementing a feature end-to-end), users must manually provide follow-up prompts.

### 2.2 Solution

Two complementary features:

| Feature | Description | Entry Point | Use Case |
|---------|-------------|-------------|----------|
| **Loop Driver** | Add iter/time conditions to any agent | `exec --iter 5` | Automated iterative refinement |
| **SpawnTask** | Spawn tasks with unified lifecycle | `/spawn --name task1 --iter 5 query...` | Parallel long-running tasks |

### 2.3 Design Decisions (Confirmed)

| Feature | Decision | Rationale |
|---------|----------|-----------|
| **Extensible Architecture** | `SpawnTask` trait with multiple implementations | Future workflow/other types |
| **Worktree** | Framework-level (for ALL spawn types), enabled by default | Isolation for all task types |
| **Error Handling** | Continue-on-error mode | AutoCoder pattern, more resilient |
| **Model Override** | Not supported (use session model) | Simpler, consistent context |
| **Merge conflicts** | Main agent drives LLM to resolve | Intelligent conflict resolution |
| **Name format** | No spaces allowed (only `a-z`, `0-9`, `-`, `_`) | Filesystem/git compatibility |
| **Auto PR** | Optional `--pr` flag, requires worktree | AutoCoder pattern, streamlined workflow |
| **Concurrency** | Configurable limit (default: 5) | Prevents resource exhaustion |

---

## 3. Architecture

### 3.1 Architecture Diagram

```
                                User
                                  │
          ┌───────────────────────┼───────────────────────┐
          │                       │                       │
          ▼                       ▼                       ▼
    ┌──────────┐           ┌──────────┐           ┌──────────┐
    │   CLI    │           │   TUI    │           │   TUI    │
    │  (exec)  │           │ (normal) │           │ (/spawn) │
    └────┬─────┘           └────┬─────┘           └────┬─────┘
         │                      │                      │
         │ --iter               │                      │ /spawn --name x --iter 5 q
         │                      │                      │ /spawn --workflow file.yaml
         ▼                      ▼                      ▼
    ┌─────────────────────────────────────────────────────────┐
    │                    core/src/loop_driver/                 │
    │  ┌─────────────────┐                                     │
    │  │  LoopCondition  │  enum: Iters(i32) | Duration(i64)   │
    │  └────────┬────────┘                                     │
    │           │                                              │
    │  ┌────────▼────────┐  ┌─────────────────────────────┐   │
    │  │   LoopDriver    │  │    LoopPromptBuilder        │   │
    │  │ - should_continue│  │ - enhance(query) -> query'  │   │
    │  │ - build_query    │  │ - git-based instructions    │   │
    │  │ - run_with_loop  │  │ - continue-on-error        │   │
    │  │ ⭐ progress_cb   │  └─────────────────────────────┘   │
    │  │ ⭐ sink param    │                                    │
    │  └────────┬────────┘                                     │
    └───────────┼──────────────────────────────────────────────┘
                │
                ▼
    ┌─────────────────────────────────────────────────────────┐
    │                core/src/spawn_task/                      │
    │  ┌──────────────────────────────────────────────────┐   │
    │  │   SpawnTask (trait)                               │   │
    │  │   - task_id() / task_type() / spawn() / cancel()  │   │
    │  └────────────────────────┬─────────────────────────┘   │
    │                           │                              │
    │  ┌────────────────────────▼─────────────────────────┐   │
    │  │   SpawnTaskManager (unified lifecycle)            │   │
    │  │   - start(task, use_worktree) / kill / list / drop│   │
    │  └────────────────────────┬─────────────────────────┘   │
    │                           │                              │
    │  ┌────────────────────────▼─────────────────────────┐   │
    │  │   WorktreeManager (GENERIC)                       │   │
    │  │   - create_worktree() / cleanup_worktree()        │   │
    │  │   ⭐ Uses tokio::process::Command (async)         │   │
    │  └──────────────────────────────────────────────────┘   │
    │                           │                              │
    │  ┌──────────────────────────────────────────────────┐   │
    │  │   ⭐ LogFileSink (same-process event logging)     │   │
    │  │   - log() / log_iteration()                       │   │
    │  │   - Path: ~/.codex/spawn-tasks/logs/<task>.log    │   │
    │  └──────────────────────────────────────────────────┘   │
    │                           │                              │
    │           ┌───────────────┼───────────────┐              │
    │           ▼               ▼               ▼              │
    │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      │
    │  │ SpawnAgent  │  │SpawnWorkflow│  │ Future...   │      │
    │  │(LoopDriver) │  │ (executor)  │  │             │      │
    │  │ ⭐+LogSink  │  │             │  │             │      │
    │  └─────────────┘  └─────────────┘  └─────────────┘      │
    └─────────────────────────────────────────────────────────┘
                │
                ▼
    ┌─────────────────────────────────────────────────────────┐
    │                   Existing Infrastructure                │
    │  ┌────────────────────────────────────────────────────┐ │
    │  │ Codex::spawn() -> Session -> run_task() -> turns   │ │
    │  └────────────────────────────────────────────────────┘ │
    └─────────────────────────────────────────────────────────┘
```

### 3.2 Worktree as Framework Feature

**Key Insight:** Worktree is a **spawn framework feature**, not task-specific.

```rust
// In SpawnTaskManager::start()
pub async fn start(&self, task: Box<dyn SpawnTask>, use_worktree: bool) -> Result<String> {
    // 1. Create worktree BEFORE spawning task (if enabled)
    let worktree_info = if use_worktree {
        Some(self.worktree_manager.create_worktree(&task.task_id()).await?)
    } else {
        None
    };

    // 2. Update task's working directory to worktree path
    if let Some(ref wt) = worktree_info {
        task.set_cwd(wt.worktree_path.clone());
    }

    // 3. Spawn the task (agent, workflow, or other)
    let handle = task.spawn();

    // 4. Store metadata with worktree info
    // ...
}
```

**Behavior:**
- **All `/spawn` commands create worktree by default**
- Use `--noworktree` to disable for any task type
- Worktree cleanup handled by manager on `--drop`

---

## 4. Architecture Decision

### 4.1 Why SpawnTask Trait (Extensible Design)

This design implements `SpawnTask` as a **trait** that can have multiple implementations.

#### Comparison: Different Spawn Task Types

| Factor | SpawnAgent | SpawnWorkflow (Future) |
|--------|------------|------------------------|
| **Primary Purpose** | Long-running autonomous work | Workflow step execution |
| **Execution** | LoopDriver with iterations | YAML workflow executor |
| **Tool Access** | Full access (like main agent) | Depends on workflow config |

#### Key Benefits of Trait-Based Design

1. **Unified Lifecycle**: One manager for all task types (start/kill/list/drop)
2. **Consistent UX**: Same `/spawn --list`, `/spawn --kill` for all types
3. **Shared Infrastructure**: Worktree, metadata, system reminders
4. **Easy Extension**: Add new task types without modifying core

### 4.2 Why Separate from Subagent

| Factor | Subagent | SpawnTask |
|--------|----------|-----------|
| **Purpose** | Delegated specialized tasks | Long-running autonomous work |
| **Tool Access** | Filtered | Full access |
| **Duration** | Short (60s default) | Extended (hours/days) |
| **Iteration** | Single execution | Multiple iterations |
| **Upstream Risk** | Core infrastructure | Isolated new code |

### 4.3 What We Reuse

- `Codex::spawn()` pattern from `codex_delegate.rs` for full isolation
- `ApprovalMode` enum for approval policy (consistent with subagent pattern)
- `BackgroundTaskInfo` type for system reminder integration
- Event processing patterns from existing subagent

### 4.4 What We Create New

- `SpawnTask` trait for extensibility
- `SpawnTaskManager` for unified lifecycle
- `LoopDriver` for iteration control
- `WorktreeManager` for git isolation (framework-level)
- Protocol events for spawn task status

---

## 5. File Structure

### 5.1 New Files (Extensible Architecture)

```
codex-rs/
├── core/src/
│   ├── loop_driver/
│   │   ├── mod.rs              # Module exports
│   │   ├── condition.rs        # LoopCondition enum
│   │   ├── driver.rs           # LoopDriver (with continue-on-error)
│   │   └── prompt.rs           # LoopPromptBuilder
│   │
│   └── spawn_task/             # ⭐ Unified task framework
│       ├── mod.rs              # SpawnTask trait, SpawnTaskType
│       ├── metadata.rs         # SpawnTaskMetadata (unified)
│       ├── manager.rs          # SpawnTaskManager (unified lifecycle)
│       ├── result.rs           # SpawnTaskResult, SpawnTaskStatus
│       ├── worktree.rs         # ⭐ WorktreeManager (GENERIC, async)
│       ├── log_sink.rs         # ⭐ LogFileSink (same-process logging)
│       │
│       ├── agent/              # SpawnAgent implementation
│       │   ├── mod.rs
│       │   ├── agent.rs        # impl SpawnTask for SpawnAgent
│       │   └── merge.rs        # Agent-specific merge prompt
│       │
│       └── workflow/           # Future: SpawnWorkflow
│           └── mod.rs          # Placeholder
│
├── core/src/system_reminder/attachments/
│   └── spawn_task.rs           # SpawnTaskGenerator (unified)
│
└── tui/src/
    └── spawn_command_ext.rs    # /spawn command handler (all types)
```

### 5.2 Modified Files

| File | Changes |
|------|---------|
| `core/src/lib.rs` | Add `pub mod loop_driver;` and `pub mod spawn_task;` |
| `exec/src/lib.rs` | Add `--iter` and `--time` CLI args |
| `tui/src/slash_command.rs` | Add `Spawn` variant (+5 lines) |
| `tui/src/chatwidget.rs` | Add dispatch to `spawn_command_ext` (+10 lines) |
| `protocol/src/protocol_ext.rs` | Add SpawnTask* events (+40 lines) |

---

## 6. Implementation Guide

### 6.1 Phase 1: Loop Driver Foundation

1. Create `core/src/loop_driver/mod.rs`
2. Create `core/src/loop_driver/condition.rs`
3. Create `core/src/loop_driver/prompt.rs`
4. Create `core/src/loop_driver/driver.rs` (with **continue-on-error**)
5. Add to `core/src/lib.rs`:
   ```rust
   pub mod loop_driver;
   ```
6. Run `cargo check -p codex-core`

### 6.2 Phase 2: SpawnTask Framework

1. Create `core/src/spawn_task/mod.rs` with `SpawnTask` trait
2. Create `core/src/spawn_task/result.rs` (SpawnTaskResult, SpawnTaskStatus)
3. Create `core/src/spawn_task/metadata.rs` (unified persistence)
4. Create `core/src/spawn_task/worktree.rs` (**GENERIC** WorktreeManager)
5. Create `core/src/spawn_task/manager.rs` (SpawnTaskManager)
6. Add to `core/src/lib.rs`:
   ```rust
   pub mod spawn_task;
   ```
7. Run `cargo check -p codex-core`

### 6.3 Phase 3: SpawnAgent Implementation

1. Create `core/src/spawn_task/agent/mod.rs`
2. Create `core/src/spawn_task/agent/agent.rs` (impl SpawnTask)
3. Create `core/src/spawn_task/agent/merge.rs`
4. Add protocol events to `protocol/src/protocol_ext.rs`
5. Run `cargo check -p codex-core`

### 6.4 Phase 4: CLI Integration

1. Add CLI args to `exec/src/lib.rs`:
   ```rust
   #[arg(long)]
   iter: Option<i32>,

   #[arg(long)]
   time: Option<String>,
   ```
2. Integrate LoopDriver into exec main flow
3. Test: `just exec --iter 2 "ls files"`

### 6.5 Phase 5: TUI Integration

1. Add `SlashCommand::Spawn` to `tui/src/slash_command.rs`
2. Create `tui/src/spawn_command_ext.rs`
3. Update `tui/src/chatwidget.rs` dispatch
4. Run `cargo check -p codex-tui`

### 6.6 Phase 6: Future Extensibility

1. Create `core/src/spawn_task/workflow/mod.rs` (placeholder)
2. Document how to add new SpawnTask implementations

---

## 7. Testing Strategy

### 7.1 Unit Tests

| Module | Test Cases |
|--------|------------|
| `LoopCondition` | Parse count, parse duration, invalid input |
| `LoopPromptBuilder` | First iteration unchanged, subsequent enhanced |
| `LoopDriver` | should_continue count, should_continue duration, cancelled, **continue-on-error** |
| `SpawnTaskMetadata` | Save/load, mark completed/failed/cancelled |
| `SpawnTaskManager` | Start, list, kill, drop (for any task type) |
| `WorktreeManager` | Create worktree, cleanup worktree (generic) |
| `SpawnAgent` | Spawn with loop, spawn without worktree |

### 7.2 Integration Tests

```rust
#[tokio::test]
async fn loop_driver_continue_on_error() {
    // Set up mock session that fails on iteration 2
    // Run with count = 5
    // Verify all 5 iterations attempted (not stopped at 2)
}

#[tokio::test]
async fn spawn_task_with_worktree() {
    // Start any SpawnTask type with worktree
    // Verify worktree created
    // Drop task
    // Verify worktree cleaned up
}

#[tokio::test]
async fn spawn_manager_unified_lifecycle() {
    // Start SpawnAgent
    // Verify in list
    // Kill via manager
    // Verify status updated
}
```

### 7.3 Manual Tests

```bash
# Loop Driver (exec CLI)
just exec --iter 3 "echo hello"         # Verify 3 iterations
just exec --time 30s "ls"               # Verify time-based

# SpawnAgent (TUI) - with worktree (default)
/spawn --name task1 --iter 2 list files

# SpawnAgent (TUI) - without worktree
/spawn --name task2 --iter 2 --noworktree list files

# Management (unified for all types)
/spawn --list                           # Shows all task types
/spawn --status task1
/spawn --kill task1
/spawn --drop task1

# Future: Workflow (same management)
/spawn --workflow file.yaml --name task3
/spawn --list                           # Shows agent AND workflow
```

---

## 8. Error Handling

### 8.1 Continue-on-Error (LoopDriver)

**Key Behavior:** Iterations continue after single iteration failure.

```rust
// In LoopDriver::run_with_loop()
if let Err(e) = codex.submit(Op::UserInput { items: input }).await {
    warn!(iteration = self.iteration, error = %e, "Iteration failed, continuing...");
    self.iteration += 1;  // Still count as completed
    continue;  // Don't break - continue to next iteration
}
```

### 8.2 User Errors

| Error | Message | Action |
|-------|---------|--------|
| Invalid duration | "Invalid loop condition: 'abc'. Expected count (e.g., '5') or duration (e.g., '1h')" | Return early |
| Task exists | "Task 'x' already exists" | Return early |
| Task not found | "Task 'x' not found" | Return early |
| Task running (drop) | "Task 'x' is still running. Use /spawn --kill first." | Return early |
| Worktree exists | "Worktree for task 'x' already exists" | Return early |

### 8.3 System Errors

| Error | Handling |
|-------|----------|
| Filesystem error | Log + continue (best effort persistence) |
| Codex spawn failure | Log + update metadata to failed |
| Task panic | Log warning + cleanup handle |
| Git worktree create failure | Log + update metadata to failed |
| Git worktree cleanup failure | Log warning + continue (orphaned worktree) |

### 8.4 Recovery

- Stale "running" status: Detected on next start, user prompted to `/spawn --drop`
- Orphaned metadata: `list_metadata` shows all, user can `/spawn --drop` manually
- Crash during iteration: Metadata shows last successful iteration count
- Orphaned worktree: `git worktree list` shows all, user can manually cleanup

---

## 9. Quick Reference

### 9.1 Commands

```bash
# Loop Driver (exec CLI)
exec --iter 5 "query"                               # 5 iterations
exec --time 1h "query"                              # 1 hour duration

# SpawnAgent (TUI) - worktree enabled by default
/spawn --name task1 --iter 5 implement user auth    # 5 iterations
/spawn --name task1 --time 1h fix all bugs          # 1 hour duration
/spawn --name task1 --iter 5 --noworktree do task   # Disable worktree
/spawn --name task1 --iter 5 --pr implement auth    # Auto PR creation
/spawn --name task1 --iter 5 --pr "Add auth" query  # PR with custom title

# Future: SpawnWorkflow (same worktree behavior)
/spawn --workflow file.yaml --name task2            # With worktree (default)
/spawn --workflow file.yaml --name task2 --noworktree  # Without worktree

# Management (unified for ALL spawn types)
/spawn --list                                       # List all types
/spawn --status task1                               # Show details
/spawn --kill task1                                 # Stop running
/spawn --drop task1                                 # Delete metadata

# Merge (agent-specific)
/spawn --merge task1,task2                          # Git merge, LLM on conflict
/spawn --merge task1 review and integrate           # LLM-driven with query
```

**Note:** Name must not contain spaces. Query is all remaining text after flags.

### 9.2 Key Types

```rust
// SpawnTask trait (generic interface)
trait SpawnTask: Send + Sync {
    fn task_id(&self) -> &str;
    fn task_type(&self) -> SpawnTaskType;
    fn spawn(self: Box<Self>) -> AbortOnDropHandle<SpawnTaskResult>;
    fn cancel(&self);
    fn metadata(&self) -> SpawnTaskMetadata;
}

// Task type discriminator
enum SpawnTaskType {
    Agent,      // SpawnAgent (full Codex with loop)
    Workflow,   // SpawnWorkflow (YAML executor) - future
}

// Loop condition
enum LoopCondition {
    Iters { count: i32 },
    Duration { seconds: i64 },
}

// ⭐ Progress info for callback
struct LoopProgress {
    iteration: i32,
    succeeded: i32,
    failed: i32,
    elapsed_seconds: i64,
}

// Task status (unified for all types)
enum SpawnTaskStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

// Loop stop reason
enum LoopStopReason {
    Completed,
    DurationElapsed,
    Cancelled,
    IterationFailed,  // Continue-on-error: logged but continued
}

// Worktree info (GENERIC - for all task types)
struct WorktreeInfo {
    task_id: String,
    worktree_path: PathBuf,
    branch_name: String,
    base_branch: String,
}

// ⭐ Log file sink (same-process event logging)
struct LogFileSink {
    file: Arc<Mutex<File>>,  // Thread-safe append
}

impl LogFileSink {
    fn log(&self, msg: &str);           // [HH:MM:SS] msg
    fn log_iteration(&self, i, s, f);   // === Iteration N complete ===
}
```

### 9.3 File Paths

```
~/.codex/spawn-tasks/
├── task1.json                    # Metadata (any task type)
├── task2.json
├── logs/                         # ⭐ Log files (same-process events)
│   ├── task1.log                 # Events from task1
│   └── task2.log                 # Events from task2
├── worktrees/                    # Git worktrees (for ANY task type)
│   ├── task1/                    # Worktree for task1 (agent)
│   └── task2/                    # Worktree for task2 (workflow)
└── ...
```

### 9.4 Extensibility Example

Adding a new SpawnTask type requires:

1. Create `spawn_task/new_type/mod.rs`
2. Implement `SpawnTask` trait
3. Register in TUI command parser
4. Done - manager/worktree/metadata just work!

```rust
// Example: Adding SpawnWorkflow
impl SpawnTask for SpawnWorkflow {
    fn task_id(&self) -> &str { &self.id }
    fn task_type(&self) -> SpawnTaskType { SpawnTaskType::Workflow }
    fn spawn(self: Box<Self>) -> AbortOnDropHandle<SpawnTaskResult> {
        tokio::spawn(async move {
            // Execute workflow steps...
        })
    }
    // ...
}
```
