# Background Mode Architecture

## Overview

Background mode allows long-running tasks to execute without blocking the main agent. **Subagents and Bash commands use completely different background mechanisms.**

## Task Types Comparison

```
┌─────────────────────────────────────────────────────────────────┐
│         Background Task Types (Different Mechanisms)             │
│                                                                  │
│  ┌───────────────────────┐    ┌───────────────────────────┐    │
│  │   local_agent         │    │     local_bash            │    │
│  │   (Subagent/Task)     │    │     (Bash command)        │    │
│  ├───────────────────────┤    ├───────────────────────────┤    │
│  │ Mechanism:            │    │ Mechanism:                │    │
│  │ Message loop聚合      │    │ Child process spawning   │    │
│  │                       │    │                           │    │
│  │ Storage:              │    │ Storage:                  │    │
│  │ Transcript .jsonl     │    │ In-memory + output file  │    │
│  │                       │    │                           │    │
│  │ Resume:               │    │ Resume:                   │    │
│  │ ✓ Yes                 │    │ ✗ No                      │    │
│  │                       │    │                           │    │
│  │ Persistence:          │    │ Persistence:              │    │
│  │ ✓ Across sessions     │    │ ✗ Lost on exit           │    │
│  │                       │    │                           │    │
│  │ Ctrl+B transition:    │    │ Ctrl+B transition:       │    │
│  │ ✓ Supported           │    │ ✗ N/A (already bg)       │    │
│  └───────────────────────┘    └───────────────────────────┘    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Task ID Format Conventions

Task IDs follow a prefix convention for easy identification:

| Task Type | ID Format | Example |
|-----------|-----------|---------|
| local_bash | `s` + short UUID | `s4d5e6f` |
| local_agent | No prefix, full agent ID | `agent_abc123` |
| remote_agent | `r` + session ID | `r1a2b3c` |

```rust
pub fn generate_task_id(task_type: TaskType) -> String {
    match task_type {
        TaskType::LocalBash => format!("s{}", short_uuid()),
        TaskType::LocalAgent => format!("agent_{}", uuid::Uuid::new_v4()),
        TaskType::RemoteAgent => format!("r{}", short_uuid()),
    }
}

fn short_uuid() -> String {
    uuid::Uuid::new_v4().to_string()[..7].to_string()
}
```

## Subagent Background Modes (local_agent)

### Two Background Patterns

```
┌─────────────────────────────────────────────────────────────────┐
│                   Subagent Background Patterns                   │
│                                                                  │
│  Mode 1: Fully Backgrounded (run_in_background=true)            │
│  ─────────────────────────────────────────────────              │
│  Task tool input → createFullyBackgroundedAgent()               │
│       │                                                          │
│       ├─► Return immediately: { status: "async_launched" }      │
│       │                                                          │
│       └─► Background: aggregateAsyncAgentExecution()            │
│                 │                                                │
│                 ├─ Drain message generator                      │
│                 ├─ Update task progress in AppState             │
│                 └─ Mark completed when done                     │
│                                                                  │
│  Mode 2: Backgroundable (Ctrl+B transition)                     │
│  ──────────────────────────────────────────                     │
│  Task tool input → createBackgroundableAgent()                  │
│       │                                                          │
│       │  Register backgroundSignal Promise                       │
│       ▼                                                          │
│  ┌────────────────────────────────────────────┐                 │
│  │  select! {                                  │                 │
│  │    event = message_rx.recv() => ...        │                 │
│  │    _ = background_signal.recv() => 'bg'    │                 │
│  │  }                                          │                 │
│  └────────────────────────────────────────────┘                 │
│       │                                                          │
│       ├─ Normal: Continue foreground execution                  │
│       │                                                          │
│       └─ Background (Ctrl+B pressed):                           │
│            │                                                     │
│            ├─► aggregateAsyncAgentExecution() takes over        │
│            └─► Return: { status: "async_launched" }             │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Subagent Background Implementation

```rust
/// Global background signal map for Ctrl+B transitions
static BACKGROUND_SIGNAL_MAP: Lazy<RwLock<HashMap<String, oneshot::Sender<()>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Register a backgroundable agent (returns receiver for signal)
pub fn register_backgroundable_agent(agent_id: String) -> oneshot::Receiver<()> {
    let (tx, rx) = oneshot::channel();
    BACKGROUND_SIGNAL_MAP.write().unwrap().insert(agent_id, tx);
    rx
}

/// Trigger background transition (called on Ctrl+B)
pub fn trigger_background_transition(agent_id: &str) -> bool {
    if let Some(tx) = BACKGROUND_SIGNAL_MAP.write().unwrap().remove(agent_id) {
        tx.send(()).is_ok()
    } else {
        false
    }
}

/// Unregister agent (cleanup)
pub fn unregister_backgroundable_agent(agent_id: &str) {
    BACKGROUND_SIGNAL_MAP.write().unwrap().remove(agent_id);
}
```

### Async Agent Aggregation

Fire-and-forget background execution that drains the message generator:

```rust
/// Fire-and-forget background execution loop
pub fn aggregate_async_agent_execution(
    message_generator: Pin<Box<dyn Stream<Item = LoopEvent> + Send>>,
    task_id: String,
    set_app_state: Arc<dyn Fn(AppStateUpdater) + Send + Sync>,
    final_callback: Option<Box<dyn FnOnce(AgentResult) + Send>>,
    initial_messages: Vec<ConversationMessage>,
    abort_signal: CancellationToken,
) {
    tokio::spawn(async move {
        let mut all_messages = initial_messages;
        let mut token_count = 0;
        let mut tool_use_count = 0;
        let mut recent_activities = VecDeque::with_capacity(5);

        let mut stream = message_generator;

        loop {
            tokio::select! {
                _ = abort_signal.cancelled() => {
                    mark_agent_task_killed(&task_id, &set_app_state);
                    break;
                }
                event = stream.next() => {
                    match event {
                        Some(LoopEvent::TextDelta { delta, .. }) => {
                            // Accumulate text in messages
                            accumulate_text(&mut all_messages, delta);
                        }
                        Some(LoopEvent::ToolUseCompleted { call_id, .. }) => {
                            tool_use_count += 1;
                            recent_activities.push_back(format!("Tool: {call_id}"));
                            if recent_activities.len() > 5 {  // Keep last 5 (aligned with Claude Code)
                                recent_activities.pop_front();
                            }
                        }
                        Some(LoopEvent::StreamRequestEnd { usage, .. }) => {
                            token_count += usage.total_tokens;
                        }
                        None => {
                            // Stream complete
                            mark_agent_task_completed(&task_id, &set_app_state, AgentResult {
                                content: aggregate_content(&all_messages),
                                total_tokens: token_count,
                                tool_use_count,
                            });
                            break;
                        }
                        _ => {}
                    }

                    // Update progress in app state
                    update_task_progress(&task_id, &set_app_state, TaskProgress {
                        token_count,
                        tool_use_count,
                        recent_activities: recent_activities.iter().cloned().collect(),
                    });
                }
            }
        }

        if let Some(callback) = final_callback {
            callback(AgentResult {
                content: aggregate_content(&all_messages),
                total_tokens: token_count,
                tool_use_count,
            });
        }
    });
}
```

### Backgroundable Agent Loop

Foreground agent that can transition to background on Ctrl+B:

```rust
pub async fn run_backgroundable_agent(
    agent_id: &str,
    loop_driver: &mut AgentLoop,
    initial_msg: ConversationMessage,
    event_tx: mpsc::Sender<LoopEvent>,
    set_app_state: Arc<dyn Fn(AppStateUpdater) + Send + Sync>,
) -> Result<AgentResult, AgentError> {
    // Register for background signal
    let bg_signal = register_backgroundable_agent(agent_id.to_string());

    // Create message generator (lazy evaluation)
    let mut stream = loop_driver.run_streaming(initial_msg);

    loop {
        tokio::select! {
            // Normal execution path
            event = stream.next() => {
                match event {
                    Some(e) => {
                        event_tx.send(e.clone()).await?;
                        if matches!(e, LoopEvent::TurnCompleted { .. }) {
                            // Check if more turns needed
                            if !stream.has_more() {
                                break;
                            }
                        }
                    }
                    None => break,  // Stream complete
                }
            }

            // Background signal path
            _ = bg_signal => {
                // Emit backgrounded event
                event_tx.send(LoopEvent::SubagentBackgrounded {
                    agent_id: agent_id.to_string(),
                    output_file: get_output_file(agent_id),
                }).await?;

                // Hand off to background aggregation
                aggregate_async_agent_execution(
                    Box::pin(stream),
                    agent_id.to_string(),
                    set_app_state,
                    None,
                    loop_driver.context().messages().clone(),
                    loop_driver.cancel_token(),
                );

                // Return immediately
                return Ok(AgentResult::backgrounded(agent_id));
            }
        }
    }

    // Cleanup
    unregister_backgroundable_agent(agent_id);

    Ok(stream.result()?)
}
```

## Bash Background Mode (local_bash)

Bash background uses a completely different mechanism: child process spawning.

```rust
/// Bash background: spawn child process
pub async fn execute_background_command(
    command: &str,
    cwd: &Path,
    set_app_state: &dyn Fn(AppStateUpdater),
) -> Result<String, ToolError> {
    let task_id = generate_task_id("local_bash");

    // Spawn child process
    let mut child = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Create output file
    let output_path = create_task_output_file(&task_id).await?;

    // Register task in app state
    set_app_state(|state| {
        state.tasks.insert(task_id.clone(), TaskState {
            id: task_id.clone(),
            task_type: TaskType::LocalBash,
            status: TaskStatus::Running,
            command: Some(command.to_string()),
            child_process: Some(child.id()),
            output_file: Some(output_path.clone()),
            is_backgrounded: false,  // N/A for bash
            ..Default::default()
        });
    });

    // Spawn output collector task (fire-and-forget)
    let task_id_clone = task_id.clone();
    tokio::spawn(async move {
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        let mut output_file = tokio::fs::File::create(&output_path).await.unwrap();
        let mut combined_output = String::new();

        // Collect output concurrently
        loop {
            tokio::select! {
                line = stdout_reader.next_line() => {
                    match line {
                        Ok(Some(l)) => {
                            combined_output.push_str(&l);
                            combined_output.push('\n');
                            output_file.write_all(l.as_bytes()).await.ok();
                            output_file.write_all(b"\n").await.ok();
                        }
                        _ => break,
                    }
                }
                line = stderr_reader.next_line() => {
                    match line {
                        Ok(Some(l)) => {
                            combined_output.push_str(&l);
                            combined_output.push('\n');
                            output_file.write_all(l.as_bytes()).await.ok();
                            output_file.write_all(b"\n").await.ok();
                        }
                        _ => break,
                    }
                }
            }
        }

        // Wait for process completion
        let status = child.wait().await;

        // Update task state
        set_app_state(|state| {
            if let Some(task) = state.tasks.get_mut(&task_id_clone) {
                task.status = match status {
                    Ok(s) if s.success() => TaskStatus::Completed,
                    _ => TaskStatus::Failed,
                };
                task.result = Some(TaskResult {
                    output: combined_output,
                    exit_code: status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1),
                });
            }
        });
    });

    Ok(task_id)
}
```

## Task State Management

All background tasks share the same AppState tracking.

### Unified Tasks System (v2.1.7)

Claude Code v2.1.7 introduces a unified tasks system that consolidates the previously separate `background_shells` and `async_agents` tracking into a single `unified_tasks` abstraction.

```
┌─────────────────────────────────────────────────────────────────┐
│              Unified Tasks System (v2.1.7)                       │
│                                                                  │
│  Previous (separate tracking):                                   │
│  ┌─────────────────┐  ┌─────────────────┐                       │
│  │ background_shells│  │ async_agents    │                       │
│  │ (Bash commands) │  │ (Subagents)     │                       │
│  └─────────────────┘  └─────────────────┘                       │
│                                                                  │
│  Unified (v2.1.7):                                              │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                    unified_tasks                             ││
│  │  ┌───────────┐  ┌───────────┐  ┌───────────┐               ││
│  │  │local_bash │  │local_agent│  │remote_agent│               ││
│  │  └───────────┘  └───────────┘  └───────────┘               ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

#### Unified Tasks Attachment

The `unified_tasks` attachment replaces separate `background_shells` and `async_agents` attachments:

```rust
/// Unified tasks attachment for system reminders
#[derive(Debug, Clone)]
pub struct UnifiedTasksAttachment {
    /// All background tasks (shells + agents)
    pub tasks: Vec<UnifiedTask>,
}

#[derive(Debug, Clone)]
pub struct UnifiedTask {
    pub id: String,
    pub task_type: TaskType,
    pub status: TaskStatus,
    pub description: String,
    pub start_time: SystemTime,
    /// Recent activities (last 5)
    pub recent_activities: Vec<String>,
}

impl UnifiedTasksAttachment {
    pub fn to_system_reminder(&self) -> String {
        if self.tasks.is_empty() {
            return String::new();
        }

        let task_lines: Vec<String> = self.tasks.iter().map(|t| {
            format!("- [{}] {} ({}): {}",
                t.id,
                t.task_type.as_str(),
                t.status.as_str(),
                t.description
            )
        }).collect();

        format!("<system-reminder>
Background tasks:
{}

Use TaskOutput to check task results. Use TaskStop to terminate running tasks.
</system-reminder>",
            task_lines.join("\n")
        )
    }
}
```

#### Attachment Generation

```rust
/// Generate unified tasks attachment
pub async fn generate_unified_tasks(
    ctx: &ToolContext,
    conversation_history: &[ConversationMessage],
) -> Option<UnifiedTasksAttachment> {
    let app_state = (ctx.get_app_state)();

    // Collect all tasks (shells + agents)
    let tasks: Vec<UnifiedTask> = app_state.tasks
        .values()
        .filter(|t| t.status == TaskStatus::Running || t.status == TaskStatus::Pending)
        .map(|t| UnifiedTask {
            id: t.id.clone(),
            task_type: t.task_type,
            status: t.status,
            description: t.description.clone(),
            start_time: t.start_time,
            recent_activities: t.progress
                .as_ref()
                .map(|p| p.recent_activities.clone())
                .unwrap_or_default(),
        })
        .collect();

    if tasks.is_empty() {
        return None;
    }

    Some(UnifiedTasksAttachment { tasks })
}
```

### Task Status Restoration After Compact

When context is compacted, task statuses are restored via the unified tasks attachment:

```rust
/// Restore task context after compaction
pub fn restore_task_context(
    compact_result: &CompactResult,
    app_state: &AppState,
) -> Vec<ContentBlock> {
    let mut restoration = Vec::new();

    // Restore running tasks
    let running_tasks: Vec<_> = app_state.tasks.values()
        .filter(|t| t.status == TaskStatus::Running)
        .collect();

    if !running_tasks.is_empty() {
        let task_summary = running_tasks.iter()
            .map(|t| format!("- {}: {} ({})", t.id, t.description, t.task_type.as_str()))
            .collect::<Vec<_>>()
            .join("\n");

        restoration.push(ContentBlock::text(format!(
            "## Running Background Tasks\n\n{}\n\n\
             Use TaskOutput to check progress or results.",
            task_summary
        )));
    }

    restoration
}
```

### Invoked Skills Tracking

The unified tasks system also tracks invoked skills, sorted by recency:

```rust
/// Invoked skills tracking for context restoration
#[derive(Debug, Clone)]
pub struct InvokedSkillsAttachment {
    /// Skills invoked in this session, sorted by recency
    pub skills: Vec<InvokedSkill>,
}

#[derive(Debug, Clone)]
pub struct InvokedSkill {
    /// Skill name (e.g., "commit", "review-pr")
    pub name: String,
    /// Skill file path
    pub path: PathBuf,
    /// Skill content preview (first N lines)
    pub content_preview: String,
    /// Last invocation timestamp
    pub last_invoked: SystemTime,
}

/// Generate invoked skills attachment
pub fn generate_invoked_skills(ctx: &ToolContext) -> Option<InvokedSkillsAttachment> {
    let skills = ctx.get_invoked_skills();

    if skills.is_empty() {
        return None;
    }

    // Sort by recency (most recent first)
    let mut sorted_skills: Vec<_> = skills.into_iter().collect();
    sorted_skills.sort_by(|a, b| b.last_invoked.cmp(&a.last_invoked));

    Some(InvokedSkillsAttachment {
        skills: sorted_skills,
    })
}
```

### AppState Definition

```rust
#[derive(Debug, Clone)]
pub struct AppState {
    /// Running/completed tasks (both agent and bash)
    pub tasks: HashMap<String, TaskState>,

    /// Tool permission context
    pub tool_permission_context: ToolPermissionContext,

    /// Queued commands (from tools like AskUserQuestion)
    pub queued_commands: Vec<QueuedCommand>,

    /// Invoked skills (for restoration after compact)
    pub invoked_skills: Vec<InvokedSkill>,
}

#[derive(Debug, Clone)]
pub struct TaskState {
    pub id: String,
    pub task_type: TaskType,  // LocalAgent, LocalBash, RemoteAgent
    pub status: TaskStatus,   // Pending, Running, Completed, Failed, Killed

    pub description: String,
    pub start_time: SystemTime,
    pub output_file: Option<PathBuf>,
    pub output_offset: i64,

    pub progress: Option<TaskProgress>,
    pub result: Option<TaskResult>,

    /// For bash: child process ID
    pub child_process: Option<u32>,

    /// For agents: command to run
    pub command: Option<String>,

    /// Abort controller for cancellation
    pub abort_controller: Arc<AbortController>,

    /// Whether agent was backgrounded (Ctrl+B)
    pub is_backgrounded: bool,

    /// Cleanup callback
    pub unregister_cleanup: Option<Arc<dyn Fn() + Send + Sync>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    LocalAgent,   // Subagent via Task tool
    LocalBash,    // Background bash command
    RemoteAgent,  // Remote Claude session (future)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Killed,
}

#[derive(Debug, Clone)]
pub struct TaskProgress {
    pub token_count: i64,
    pub tool_use_count: i32,
    pub recent_activities: Vec<String>,
}

impl AppState {
    pub fn update_task(&mut self, task_id: &str, updater: impl FnOnce(&mut TaskState)) {
        if let Some(task) = self.tasks.get_mut(task_id) {
            updater(task);
        }
    }

    pub fn mark_task_completed(&mut self, task_id: &str, result: TaskResult) {
        self.update_task(task_id, |task| {
            task.status = TaskStatus::Completed;
            task.result = Some(result);
            if let Some(cleanup) = task.unregister_cleanup.take() {
                cleanup();
            }
        });
    }

    pub fn kill_background_task(&mut self, task_id: &str) -> bool {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.abort_controller.abort();
            task.status = TaskStatus::Killed;
            if let Some(cleanup) = task.unregister_cleanup.take() {
                cleanup();
            }
            true
        } else {
            false
        }
    }
}
```

## TaskOutput Tool

Retrieve output from any background task:

```rust
pub struct TaskOutputTool;

#[async_trait]
impl Tool for TaskOutputTool {
    fn name(&self) -> &str { "TaskOutput" }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition::full(
            "TaskOutput",
            "Retrieves output from a running or completed background task.",
            json!({
                "type": "object",
                "properties": {
                    "task_id": { "type": "string" },
                    "block": { "type": "boolean", "default": true },
                    "timeout": { "type": "number", "default": 30000 }
                },
                "required": ["task_id"]
            })
        )
    }

    fn is_read_only(&self) -> bool { true }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolContext,
        _tool_use_id: &str,
        _metadata: ToolMetadata,
        _progress: Option<ProgressCallback>,
    ) -> Result<ToolOutput, ToolError> {
        let args: TaskOutputArgs = serde_json::from_value(input)?;

        let app_state = (ctx.get_app_state)();
        let task = app_state.tasks.get(&args.task_id)
            .ok_or_else(|| ToolError::not_found(&args.task_id))?;

        match task.status {
            TaskStatus::Completed | TaskStatus::Failed => {
                // Return full result
                let result = task.result.clone().unwrap_or_default();
                Ok(ToolOutput::success(format!(
                    "Status: {:?}\nOutput:\n{}",
                    task.status,
                    result.output
                )))
            }
            TaskStatus::Running => {
                if args.block.unwrap_or(true) {
                    // Wait for completion (with timeout)
                    let timeout = Duration::from_millis(
                        args.timeout.unwrap_or(30000) as u64
                    );
                    // ... wait logic ...
                } else {
                    // Return current progress
                    let progress = task.progress.clone().unwrap_or_default();
                    Ok(ToolOutput::success(format!(
                        "Status: Running\nTokens: {}\nTools: {}\nRecent: {:?}",
                        progress.token_count,
                        progress.tool_use_count,
                        progress.recent_activities
                    )))
                }
            }
            TaskStatus::Killed => {
                Ok(ToolOutput::error("Task was killed"))
            }
            TaskStatus::Pending => {
                Ok(ToolOutput::success("Task is pending"))
            }
        }
    }
}
```


## KillShell Tool

Kill a running background bash shell:

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
                    "shell_id": { "type": "string" }
                },
                "required": ["shell_id"]
            })
        )
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

## Output Streaming to File

Stream task output to file during execution for real-time access:

```rust
/// Task output file configuration
#[derive(Debug, Clone)]
pub struct TaskOutputFile {
    /// Path to output file
    pub path: PathBuf,
    /// Append mode (vs overwrite)
    pub append_mode: bool,
    /// Maximum file size before rotation
    pub max_size: i64,
    /// Flush interval
    pub flush_interval: Duration,
}

impl Default for TaskOutputFile {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),  // Set per task
            append_mode: true,
            max_size: 10 * 1024 * 1024,  // 10 MB
            flush_interval: Duration::from_millis(100),
        }
    }
}

/// Output streaming task
pub struct OutputStreamer {
    file: tokio::fs::File,
    config: TaskOutputFile,
    bytes_written: i64,
}

impl OutputStreamer {
    pub async fn new(task_id: &str) -> Result<Self, io::Error> {
        let path = PathBuf::from(format!(
            "~/.claude/tasks/{task_id}/output.txt"
        ));
        tokio::fs::create_dir_all(path.parent().unwrap()).await?;

        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;

        Ok(Self {
            file,
            config: TaskOutputFile {
                path,
                ..Default::default()
            },
            bytes_written: 0,
        })
    }

    /// Write chunk to output file
    pub async fn write(&mut self, chunk: &[u8]) -> Result<(), io::Error> {
        // Check if rotation needed
        if self.bytes_written + chunk.len() as i64 > self.config.max_size {
            self.rotate().await?;
        }

        self.file.write_all(chunk).await?;
        self.bytes_written += chunk.len() as i64;

        Ok(())
    }

    /// Flush pending writes
    pub async fn flush(&mut self) -> Result<(), io::Error> {
        self.file.flush().await
    }

    /// Rotate output file when max size reached
    async fn rotate(&mut self) -> Result<(), io::Error> {
        let backup_path = self.config.path.with_extension("txt.1");
        tokio::fs::rename(&self.config.path, &backup_path).await?;

        self.file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.path)
            .await?;
        self.bytes_written = 0;

        Ok(())
    }
}

/// Integrate output streaming with background agent
impl SubagentManager {
    async fn spawn_background_with_streaming(
        &self,
        id: String,
        // ... other params ...
    ) {
        let output_streamer = OutputStreamer::new(&id).await.unwrap();

        tokio::spawn(async move {
            let mut streamer = output_streamer;

            // ... message loop ...
            while let Some(event) = stream.next().await {
                match event {
                    LoopEvent::TextDelta { delta, .. } => {
                        streamer.write(delta.as_bytes()).await.ok();
                    }
                    LoopEvent::ToolUseCompleted { output, .. } => {
                        let text = format!("\n[Tool completed]\n{}\n", output.as_text());
                        streamer.write(text.as_bytes()).await.ok();
                    }
                    _ => {}
                }
            }

            streamer.flush().await.ok();
        });
    }
}
```

## Teleport Support

Resume an agent from a different session (teleport):

```rust
/// Teleport configuration
#[derive(Debug, Clone)]
pub struct TeleportConfig {
    /// Enable teleport support
    pub enabled: bool,
    /// Sync working directory with original session
    pub cwd_sync: bool,
    /// Sync environment variables
    pub env_sync: bool,
}

impl Default for TeleportConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cwd_sync: true,
            env_sync: false,
        }
    }
}

/// Teleport context for resuming in different session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleportContext {
    /// Original session ID
    pub source_session_id: String,
    /// Agent ID to resume
    pub agent_id: String,
    /// Original working directory
    pub original_cwd: PathBuf,
    /// Original environment (if env_sync enabled)
    pub original_env: Option<HashMap<String, String>>,
    /// Transcript path
    pub transcript_path: PathBuf,
}

impl SubagentManager {
    /// Teleport an agent from another session
    pub async fn teleport(
        &self,
        teleport_ctx: TeleportContext,
        config: &TeleportConfig,
        new_session_id: &str,
    ) -> Result<String, SubagentError> {
        // Load messages from original transcript
        let messages = load_resume_messages(
            &teleport_ctx.source_session_id,
            &teleport_ctx.agent_id,
        ).await?;

        // Determine working directory
        let cwd = if config.cwd_sync {
            teleport_ctx.original_cwd.clone()
        } else {
            std::env::current_dir()?
        };

        // Create new agent with loaded context
        let new_id = format!("teleport_{}", uuid::Uuid::new_v4());

        // Build context from loaded messages
        let context = ConversationContext::from_messages(messages);

        // ... spawn agent with context ...

        Ok(new_id)
    }

    /// Export teleport context for another session
    pub fn export_teleport_context(&self, agent_id: &str) -> Option<TeleportContext> {
        let completed = self.completed.blocking_read();
        let agent = completed.get(agent_id)?;

        Some(TeleportContext {
            source_session_id: self.session_id.clone(),
            agent_id: agent_id.to_string(),
            original_cwd: self.cwd.clone(),
            original_env: None,  // Set if env_sync enabled
            transcript_path: PathBuf::from(format!(
                "~/.claude/projects/{}/subagents/agent-{}.jsonl",
                self.session_id, agent_id
            )),
        })
    }
}
```

## Sidechain Transcripts (Agent Only)

Subagent transcripts are persisted for resume capability:

```rust
/// Sidechain transcript entry
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionLogEntry {
    pub entry_type: String,  // "system", "user", "assistant"
    pub uuid: String,
    pub parent_uuid: Option<String>,
    pub is_sidechain: bool,
    pub agent_name: String,
    pub session_id: String,
    pub content: ConversationMessage,
    pub timestamp: i64,
}

/// Write event to agent transcript file
pub async fn write_to_transcript(
    agent_id: &str,
    session_id: &str,
    entry: &SessionLogEntry,
) -> Result<(), io::Error> {
    let path = PathBuf::from(format!(
        "~/.claude/projects/{session_id}/subagents/agent-{agent_id}.jsonl"
    ));

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await?;

    let line = serde_json::to_string(entry)?;
    file.write_all(line.as_bytes()).await?;
    file.write_all(b"\n").await?;

    Ok(())
}
```

## Summary: Key Differences

| Aspect | Subagent (local_agent) | Bash (local_bash) |
|--------|------------------------|-------------------|
| **Mechanism** | Message loop aggregation | Child process spawn |
| **Storage** | JSONL transcript + AppState | Output file + AppState |
| **Resume** | Yes (load transcript) | No |
| **Persistence** | Across sessions | Lost on exit |
| **Ctrl+B** | Supported (transition) | N/A |
| **Progress** | Token count, tool count, activities | Output lines |
| **Cancellation** | Abort controller | SIGTERM |
| **Output** | Aggregated from messages | Stdout + stderr |

## Events

```rust
pub enum LoopEvent {
    // Background task events
    BackgroundTaskStarted { task_id: String, task_type: TaskType },
    BackgroundTaskProgress { task_id: String, progress: TaskProgress },
    BackgroundTaskCompleted { task_id: String, result: String },

    // Subagent-specific
    SubagentBackgrounded { agent_id: String, output_file: PathBuf },

    // ... other events
}
```
