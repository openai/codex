# Advanced Execution Modes

## Overview

Beyond the basic subagent pattern (Task tool), cocode-rs supports additional execution modes for advanced scenarios:

| Pattern | Purpose | Use Case |
|---------|---------|----------|
| **Subagent** | One-shot child agent spawned by main | Delegate subtask: "explore auth code" |
| **Iterative** | Multi-run for same requirement | Run 5 iterations to refine implementation |
| **Collab** | Multi-agent coordination | Spawn workers, send input, wait for completion |

## Entry Points

These execution modes can be invoked from multiple entry points:

| Entry Point | Example | Main Agent | Background |
|-------------|---------|------------|------------|
| CLI argument | `cocode --iter 5 "task"` | No | N/A (独立进程) |
| Slash command | `/iter 5 "task"` | Yes | Optional |
| Slash command | `/iter --background 5 "task"` | Yes | Yes |
| Collab tool | `spawn_agent(...)` | Yes | Depends on wait |
| Task tool | `Task(run_in_background=true)` | Yes | Yes |

## Architecture Layers

```
┌─────────────────────────────────────────────────────────────┐
│                      app/cli                                 │
│   --iter, --collab 参数直接使用 executor                      │
├─────────────────────────────────────────────────────────────┤
│                    features/skill                            │
│   /iter, /collab slash commands                              │
├───────────────────────┬─────────────────────────────────────┤
│    core/subagent      │         core/executor                │
│    Task tool          │   AgentExecutor (base)               │
│    上下文继承          │   IterativeExecutor                  │
│    工具过滤            │   AgentCoordinator                   │
│         │             │   Collab tools                       │
│         └─────────────┼──────────────────────────────────────┤
│                       ▼                                      │
│              AgentExecutor (共享基础)                         │
├─────────────────────────────────────────────────────────────┤
│                    core/loop                                 │
│                    AgentLoop                                 │
└─────────────────────────────────────────────────────────────┘
```

**Key relationships:**
- **core/subagent** depends on **core/executor** (uses AgentExecutor internally)
- **Subagent** (Task tool): Inherits context, filters tools, then uses AgentExecutor
- **AgentExecutor**: Base primitive for running independent agent session
- **IterativeExecutor**: Multi-run wrapper over AgentExecutor
- **AgentCoordinator**: Manages multiple AgentExecutor instances

---

## AgentExecutor (Base Primitive)

### Purpose

Run an independent, full-featured agent without main agent context. This is the base primitive for all higher-level execution patterns.

### Core Types

```rust
/// Independent agent execution (no parent context)
pub struct AgentExecutor {
    model: Arc<dyn Model>,
    tools: ToolRegistry,
    config: AgentExecutorConfig,
    event_tx: mpsc::Sender<AgentEvent>,
}

pub struct AgentExecutorConfig {
    pub max_turns: Option<i32>,
    pub permission_mode: PermissionMode,
    pub cwd: PathBuf,
    pub mcp_clients: Vec<Arc<McpClient>>,
    pub cancel: CancellationToken,
    pub auto_approve_plan_mode: bool,
}

impl AgentExecutor {
    /// Create a new independent agent
    pub fn new(
        provider: &dyn Provider,
        model_name: &str,
        tools: ToolRegistry,
        config: AgentExecutorConfig,
        event_tx: mpsc::Sender<AgentEvent>,
    ) -> Result<Self, AgentError>;

    /// Run agent with initial prompt
    pub async fn run(&mut self, prompt: &str) -> Result<AgentResult, AgentError>;

    /// Run with existing context (for resume)
    pub async fn run_with_context(
        &mut self,
        prompt: &str,
        context: ConversationContext,
    ) -> Result<AgentResult, AgentError>;
}

pub struct AgentResult {
    pub final_text: String,
    pub total_tokens: i64,
    pub tool_use_count: i32,
    pub stop_reason: StopReason,
    pub messages: Vec<ConversationMessage>,
}
```

### Comparison with Subagent

| Aspect | Subagent (Task tool) | AgentExecutor |
|--------|---------------------|---------------|
| Parent context | Inherits from main | None (fresh) |
| Tool filtering | 3-layer filtering | Caller provides |
| Event channel | Shared with main | Independent |
| Use case | Delegate subtask | Independent task |

---

## IterativeExecutor (Multi-Run)

### Purpose

Run a task N times (or for T duration) with context passing between iterations. Each iteration runs an independent agent, but context flows via prompt injection.

Reference: codex-rs `core/src/loop_driver/`

### Iteration Condition

```rust
#[derive(Debug, Clone)]
pub enum IterationCondition {
    /// Run exactly N iterations
    Count { count: i32 },
    /// Run until duration elapsed
    Duration { seconds: i64 },
}

impl IterationCondition {
    /// Parse from string: "5" → Count(5), "2h" → Duration(7200)
    pub fn parse(s: &str) -> Result<Self, ParseError>;
}

// Examples:
// "5"  → Count { count: 5 }
// "10m" → Duration { seconds: 600 }
// "2h" → Duration { seconds: 7200 }
// "1d" → Duration { seconds: 86400 }
```

### Core Types

```rust
pub struct IterativeExecutorConfig {
    /// When to stop
    pub condition: IterationCondition,
    /// Base agent configuration
    pub agent_config: AgentExecutorConfig,
    /// Custom prompt for subsequent iterations
    pub continuation_prompt: Option<String>,
    /// Enable context passing via prompt injection
    pub enable_context_passing: bool,
    /// Enable git commits between iterations
    pub enable_git_commits: bool,
    /// Summarizer for iteration results
    pub summarizer: Option<Arc<dyn IterationSummarizer>>,
}

pub struct IterativeExecutor {
    config: IterativeExecutorConfig,
    provider: Arc<dyn Provider>,
    tools: ToolRegistry,
    event_tx: mpsc::Sender<IterativeEvent>,
    // State
    iteration: i32,
    iterations_failed: i32,
    start_time: Instant,
    context: IterationContext,
}

pub struct IterationContext {
    pub initial_prompt: String,
    pub base_commit: Option<String>,
    pub plan_content: Option<String>,
    pub iterations: Vec<IterationRecord>,
}

pub struct IterationRecord {
    pub iteration: i32,
    pub commit_id: Option<String>,
    pub changed_files: Vec<String>,
    pub summary: String,
    pub success: bool,
    pub timestamp: DateTime<Utc>,
    pub tokens_used: i64,
}
```

### Execution Flow

```
┌─────────────────────────────────────────────────────────────┐
│                  IterativeExecutor.run()                     │
│                                                              │
│  Initial setup:                                              │
│  - Record base_commit (git HEAD)                             │
│  - Initialize IterationContext                               │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │   while should_continue():                              │ │
│  │                                                         │ │
│  │   1. Build prompt with context injection                │ │
│  │      - Iteration 0: original + complexity assessment    │ │
│  │      - Iteration N: context block + original            │ │
│  │                                                         │ │
│  │   2. Create AgentExecutor (independent agent)           │ │
│  │                                                         │ │
│  │   3. Run agent → AgentResult                           │ │
│  │      - Continue-on-error: log failure, don't stop      │ │
│  │                                                         │ │
│  │   4. Process iteration result:                          │ │
│  │      - Summarize (LLM-based or file-based)             │ │
│  │      - Get changed files                                │ │
│  │      - Create git commit (if enabled)                   │ │
│  │      - Record IterationRecord                           │ │
│  │                                                         │ │
│  │   5. Emit IterativeEvent::IterationCompleted            │ │
│  │                                                         │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  Return IterativeResult                                      │
└─────────────────────────────────────────────────────────────┘
```

### Context Passing via Prompt Injection

**First iteration:**
```
<task_assessment>
## Task Complexity Assessment
[complexity check instructions]
</task_assessment>

Implement user authentication
```

**Subsequent iterations:**
```
<task_context>
## Original Task
Implement user authentication

## Progress
Iteration: 2 of 5
Base commit: abc123def

## Previous Iterations
### Iteration 0 → commit def456789
Files: auth.rs, auth_tests.rs
Summary: Implemented JWT token generation with basic claim validation

### Iteration 1 → commit 789abc012
Files: auth.rs, sessions.rs
Summary: Added bcrypt password hashing and session store integration
</task_context>

<task_assessment>
[complexity check]
</task_assessment>

Implement user authentication
```

### Events

```rust
pub enum IterativeEvent {
    Started { condition: IterationCondition, initial_prompt: String },
    IterationStarted { iteration: i32 },
    IterationCompleted {
        iteration: i32,
        success: bool,
        summary: String,
        commit_id: Option<String>,
        tokens_used: i64,
    },
    Completed { result: IterativeResult },
    Progress { progress: IterativeProgress },
}

pub struct IterativeResult {
    pub iterations_attempted: i32,
    pub iterations_succeeded: i32,
    pub iterations_failed: i32,
    pub stop_reason: IterativeStopReason,
    pub elapsed_seconds: i64,
    pub total_tokens: i64,
    pub records: Vec<IterationRecord>,
}
```

### Slash Command: /iter

The iterative executor can be invoked via slash command from within a session:

```rust
// features/skill/builtin/iter.rs
pub struct IterSlashCommand;

impl Skill for IterSlashCommand {
    fn name(&self) -> &str { "iter" }
    fn user_invocable(&self) -> bool { true }

    async fn execute(&self, args: &str, ctx: &SkillContext) -> Result<SkillOutput> {
        // Parse: /iter [--background] <count|duration> "<prompt>"
        let (background, condition, prompt) = parse_iter_args(args)?;

        if background {
            // Background mode - returns immediately
            let task_id = ctx.spawn_background_iterative(condition, prompt).await?;
            Ok(SkillOutput::text(format!("Started iterative task: {task_id}")))
        } else {
            // Foreground mode - blocks until complete
            let result = ctx.run_iterative(condition, prompt).await?;
            Ok(SkillOutput::text(format!(
                "Completed: {}/{} iterations",
                result.iterations_succeeded,
                result.iterations_attempted
            )))
        }
    }
}
```

**Usage examples:**
```
/iter 5 "Implement user authentication"           # 5 iterations, foreground
/iter 2h "Keep improving code quality"            # 2 hours, foreground
/iter --background 10 "Refactor authentication"   # 10 iterations, background
```

### Example: 3-Iteration Refinement

```
Command: cocode --iter 3 "Implement user authentication"

=== ITERATION 0 ===
Prompt: "Implement user authentication"
Agent: Creates auth module, JWT handling, basic tests
Changes: auth.rs, auth_tests.rs, Cargo.toml
Summary: "Implemented JWT-based authentication with bearer token validation"
Commit: [iter-0] JWT authentication foundation

=== ITERATION 1 ===
Prompt: [context block] + "Implement user authentication"
Agent: Reads git log, sees iteration 0 work
Agent: Adds password hashing, session management
Changes: auth.rs, sessions.rs
Summary: "Added bcrypt password hashing and session store integration"
Commit: [iter-1] Password hashing and sessions

=== ITERATION 2 ===
Prompt: [context block with both previous iterations] + "Implement..."
Agent: Adds error handling, edge case tests
Changes: auth.rs, errors.rs, auth_tests.rs
Summary: "Added comprehensive error handling and edge case coverage"
Commit: [iter-2] Error handling and test coverage

=== RESULT ===
Iterations: 3/3 succeeded (0 failed)
Feature fully implemented through iterative refinement
```

---

## AgentCoordinator (Multi-Agent)

### Purpose

Coordinate multiple agents with explicit communication. Based on the `collab` tools pattern in codex-rs.

Reference: codex-rs `core/src/tools/handlers/collab.rs`

### Four Collab Tools

| Tool | Purpose |
|------|---------|
| `spawn_agent` | Create a new agent with initial prompt |
| `send_input` | Send message to existing agent (multi-turn) |
| `wait` | Block until agents reach final state |
| `close_agent` | Shutdown an agent |

### Agent Lifecycle

```
               spawn_agent
                    │
                    ▼
              ┌───────────┐
              │PendingInit│
              └─────┬─────┘
                    │ (initial prompt processed)
                    ▼
              ┌───────────┐
          ┌───│  Running  │◄──┐
          │   └─────┬─────┘   │
          │         │         │ send_input (multi-turn)
          │   ┌─────┴─────────┘
          │   │
          ▼   ▼
     ┌─────────────────────────────────────────┐
     │                                          │
     ▼                ▼                         ▼
┌─────────┐     ┌─────────┐              ┌───────────┐
│Completed│     │ Errored │              │  Shutdown │
└─────────┘     └─────────┘              └───────────┘
```

### Core Types

```rust
#[derive(Debug, Clone)]
pub enum AgentStatus {
    PendingInit,
    Running,
    Completed(Option<String>),  // Final message
    Errored(String),
    Shutdown,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThreadId(Uuid);

pub struct AgentCoordinator {
    agents: HashMap<ThreadId, ManagedAgent>,
    provider: Arc<dyn Provider>,
    tools: ToolRegistry,
    config: CoordinatorConfig,
    event_tx: mpsc::Sender<CollabEvent>,
    guards: Guards,  // Resource limits
}

pub struct CoordinatorConfig {
    pub max_agents: Option<i32>,
    pub default_wait_timeout: Duration,
    pub max_wait_timeout: Duration,
}

pub enum AgentRole {
    Default,
    Orchestrator,  // Special model/instructions
    Worker,        // Lightweight model
}
```

### Operations

```rust
impl AgentCoordinator {
    /// Spawn a new agent
    pub async fn spawn_agent(
        &mut self,
        prompt: &str,
        role: AgentRole,
    ) -> Result<ThreadId, CollabError>;

    /// Send input to agent (multi-turn)
    pub async fn send_input(
        &mut self,
        thread_id: &ThreadId,
        prompt: &str,
        interrupt: bool,
    ) -> Result<(), CollabError>;

    /// Wait for agents to reach final state
    pub async fn wait(
        &mut self,
        thread_ids: &[ThreadId],
        timeout_seconds: Option<i64>,
    ) -> Result<HashMap<ThreadId, AgentStatus>, CollabError>;

    /// Close an agent
    pub async fn close_agent(
        &mut self,
        thread_id: &ThreadId,
    ) -> Result<AgentStatus, CollabError>;
}
```

### Example: Orchestrator Pattern

```
┌─────────────────────────────────────────────────────────────┐
│                  Orchestrator Agent                          │
│                                                              │
│  Turn 1: Spawn workers                                       │
│  ─────────────────────                                       │
│  spawn_agent("Analyze auth code")     → T1                  │
│  spawn_agent("Analyze API routes")    → T2                  │
│  spawn_agent("Analyze database code") → T3                  │
│                                                              │
│  Turn 2: Wait for workers                                    │
│  ─────────────────────                                       │
│  wait([T1, T2, T3], timeout=300)                            │
│  → { T1: Completed("auth analysis..."),                     │
│      T2: Running,                                            │
│      T3: Completed("db analysis...") }                       │
│                                                              │
│  Turn 3: Follow up on incomplete work                        │
│  ─────────────────────────────────────                       │
│  send_input(T2, "Focus on error handling")                  │
│  wait([T2])                                                  │
│  → { T2: Completed("API analysis...") }                     │
│                                                              │
│  Turn 4: Cleanup                                             │
│  ─────────────                                               │
│  close_agent(T1)                                             │
│  close_agent(T2)                                             │
│  close_agent(T3)                                             │
│                                                              │
│  Return: Aggregated analysis from all workers                │
└─────────────────────────────────────────────────────────────┘
```

### Events

```rust
pub enum CollabEvent {
    SpawnBegin { call_id: String, prompt: String },
    SpawnEnd { thread_id: ThreadId, status: AgentStatus },
    InteractionBegin { call_id: String, thread_id: ThreadId },
    InteractionEnd { thread_id: ThreadId, status: AgentStatus },
    WaitingBegin { thread_ids: Vec<ThreadId> },
    WaitingEnd { statuses: HashMap<ThreadId, AgentStatus> },
    CloseBegin { call_id: String, thread_id: ThreadId },
    CloseEnd { thread_id: ThreadId, status: AgentStatus },
}
```

---

## Comparison Matrix

| Aspect | Subagent (Task) | AgentExecutor | IterativeExecutor | AgentCoordinator |
|--------|-----------------|---------------|-------------------|------------------|
| Parent context | Inherits | None | None | None |
| Spawning | Via main agent | Direct | Direct | Via collab tools |
| Communication | One-shot | One-shot | Prompt injection | Multi-turn |
| Tool filtering | 3-layer | Caller provides | Caller provides | Caller provides |
| Persistence | Sidechain JSONL | None | Git commits | None |
| Resume | Yes | With context | From any iteration | No |
| Use case | Delegate subtask | Independent task | Iterative refinement | Multi-agent coordination |

---

## Integration Points

```
┌─────────────────────────────────────────────────────────────┐
│                     User / Application                       │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │ Task Tool   │  │ --iter CLI   │  │ Collab Tools     │   │
│  │ (subagent)  │  │ (iterative)  │  │ (coordination)   │   │
│  └──────┬──────┘  └──────┬───────┘  └────────┬─────────┘   │
│         │                │                    │             │
│         ▼                ▼                    ▼             │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │SubagentMgr  │  │ Iterative    │  │ Agent            │   │
│  │             │  │ Executor     │  │ Coordinator      │   │
│  └──────┬──────┘  └──────┬───────┘  └────────┬─────────┘   │
│         │                │                    │             │
│         │                ▼                    │             │
│         │        ┌──────────────┐             │             │
│         │        │ Agent        │◄────────────┘             │
│         │        │ Executor     │                           │
│         │        └──────┬───────┘                           │
│         │               │                                   │
│         ▼               ▼                                   │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                    AgentLoop                          │  │
│  │  (turn-based execution, tools, streaming, events)    │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## Crate Organization

```
core/
├── loop/           # AgentLoop (core loop driver)
├── subagent/       # SubagentManager (Task tool, context inheritance)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── manager.rs      # SubagentManager
│       ├── definition.rs   # AgentDefinition (4 built-in)
│       ├── filter.rs       # 3-layer tool filtering
│       ├── context.rs      # Context forking
│       └── transcript.rs   # Sidechain JSONL
│
└── executor/       # Execution primitives (subagent depends on this)
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── base.rs              # AgentExecutor (shared base)
        ├── background.rs        # Background execution support
        ├── iterative/
        │   ├── mod.rs
        │   ├── executor.rs      # IterativeExecutor
        │   ├── condition.rs     # Count/Duration
        │   ├── context.rs       # IterationContext, IterationRecord
        │   └── summarizer.rs    # LLM/file-based summarization
        └── coordinator/
            ├── mod.rs
            ├── manager.rs       # AgentCoordinator
            ├── lifecycle.rs     # AgentStatus, ThreadId
            └── tools/           # Collab tools
                ├── mod.rs
                ├── spawn_agent.rs
                ├── send_input.rs
                ├── wait.rs
                └── close_agent.rs
```

**Dependency:**
```
core/subagent → core/executor → core/loop
```

---

## Future: Workflow Engine

The workflow engine builds on top of these primitives to support complex agent workflows:

### Sequential Workflow
```
Agent A → Agent B → Agent C
(output of A feeds into B, B into C)
```

### Conditional Branching
```
Agent A → (success?) → Agent B
                    → (failure?) → Agent C
```

### Fan-out/Fan-in
```
          ┌─ Agent B ─┐
Agent A ──┼─ Agent C ─┼─► Aggregator ─► Agent E
          └─ Agent D ─┘
```

### Error Propagation
- Fail-fast: Stop workflow on first error
- Continue-on-error: Log and proceed
- Retry with fallback: Try alternative agents
