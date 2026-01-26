# Hooks System Architecture

## Overview

The hooks system allows customization of agent behavior at key lifecycle points. Hooks can observe, modify, or reject operations based on custom logic.

**Key Features:**
- **12 Event Types**: Tool lifecycle, user interaction, session lifecycle, subagents, compaction, notifications
- **3 Handler Types**: Shell commands, webhooks, inline callbacks
- **3 Result Types**: Continue, reject, modify input
- **Configurable Timeout**: Default 10 minutes per hook

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Agent Loop                                │
│                                                                  │
│  Tool call → PreToolUse hooks → Tool Execution → PostToolUse    │
│                   ↓ (can reject)                    ↓           │
│              HookResult::Reject              (notification only) │
│                   ↓                                             │
│              Return error                                       │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Hook Event Types (12 Types)

### Tool Lifecycle Events

```rust
/// Hook event types (Claude Code v2.1.7 aligned)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HookEventType {
    // ============ Tool Lifecycle ============

    /// Before tool execution (can reject or modify input)
    PreToolUse,

    /// After successful tool execution (notification only)
    PostToolUse,

    /// After tool execution failure (notification only)
    PostToolUseFailure,

    // ============ User Interaction ============

    /// Before user prompt is processed (can modify prompt)
    PrePromptSubmit,

    /// After user message is added to context
    PostUserMessage,

    // ============ Session Lifecycle ============

    /// Session started
    SessionStart,

    /// Session ending
    SessionEnd,

    // ============ Subagent Lifecycle ============

    /// Subagent spawned
    SubagentStart,

    /// Subagent completed or stopped
    SubagentStop,

    // ============ Compaction ============

    /// Before context compaction
    PreCompact,

    // ============ Notifications ============

    /// Generic notification event
    NotificationEvent,

    // ============ Custom ============

    /// Status line update (for TUI integration)
    StatusLine,

    /// File suggestion for auto-complete
    FileSuggestion,
}
```

### Event Context

```rust
/// Context passed to hooks
#[derive(Debug, Clone)]
pub struct HookContext {
    /// Event type
    pub event: HookEventType,

    /// Tool name (for tool events)
    pub tool_name: Option<String>,

    /// Tool input (for PreToolUse)
    pub tool_input: Option<Value>,

    /// Tool output (for PostToolUse)
    pub tool_output: Option<ToolOutput>,

    /// Error (for PostToolUseFailure)
    pub error: Option<String>,

    /// User prompt (for PrePromptSubmit)
    pub user_prompt: Option<String>,

    /// Session ID
    pub session_id: String,

    /// Agent ID (for subagent events)
    pub agent_id: Option<String>,

    /// Working directory
    pub cwd: PathBuf,

    /// Environment variables
    pub env: HashMap<String, String>,
}
```

## Hook Definition

```rust
/// Hook definition
#[derive(Debug, Clone)]
pub struct HookDefinition {
    /// Event type to trigger on
    pub event: HookEventType,

    /// Handler implementation
    pub handler: HookHandler,

    /// Run once and auto-remove
    pub once: bool,

    /// Execution timeout (default: 10 minutes)
    pub timeout: Duration,

    /// Hook name (for logging)
    pub name: Option<String>,

    /// Matcher for tool-specific hooks (e.g., "Bash*", "Edit")
    pub tool_matcher: Option<String>,
}

impl Default for HookDefinition {
    fn default() -> Self {
        Self {
            event: HookEventType::PreToolUse,
            handler: HookHandler::Inline { callback: Arc::new(|_| HookResult::Continue) },
            once: false,
            timeout: Duration::from_secs(600),  // 10 minutes
            name: None,
            tool_matcher: None,
        }
    }
}
```

## Hook Handlers (3 Types)

```rust
/// Hook handler types
#[derive(Debug, Clone)]
pub enum HookHandler {
    /// Execute shell command
    Shell {
        command: String,
        /// Pass context as JSON to stdin
        pass_context: bool,
        /// Parse stdout as JSON result
        parse_result: bool,
    },

    /// Call webhook endpoint
    Webhook {
        url: String,
        method: HttpMethod,
        headers: HashMap<String, String>,
    },

    /// Inline callback function
    Inline {
        callback: Arc<dyn Fn(HookContext) -> HookResult + Send + Sync>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HttpMethod {
    #[default]
    Post,
    Put,
}
```

## Hook Results (3 Types)

```rust
/// Hook execution result
#[derive(Debug, Clone)]
pub enum HookResult {
    /// Continue normal execution
    Continue,

    /// Reject the operation with reason
    Reject { reason: String },

    /// Modify the input and continue
    ModifyInput { new_input: Value },
}

impl HookResult {
    pub fn is_reject(&self) -> bool {
        matches!(self, HookResult::Reject { .. })
    }

    pub fn is_modify(&self) -> bool {
        matches!(self, HookResult::ModifyInput { .. })
    }
}
```

## Hook Registry

```rust
/// Registry for managing hooks
pub struct HookRegistry {
    hooks: HashMap<HookEventType, Vec<HookDefinition>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self {
            hooks: HashMap::new(),
        }
    }

    /// Register a hook
    pub fn register(&mut self, hook: HookDefinition) {
        self.hooks
            .entry(hook.event.clone())
            .or_default()
            .push(hook);
    }

    /// Execute hooks for an event
    pub async fn execute(
        &mut self,
        event: HookEventType,
        ctx: HookContext,
    ) -> HookResult {
        let hooks = match self.hooks.get(&event) {
            Some(h) => h.clone(),
            None => return HookResult::Continue,
        };

        let mut final_ctx = ctx;

        for hook in &hooks {
            // Check tool matcher
            if let Some(matcher) = &hook.tool_matcher {
                if let Some(tool_name) = &final_ctx.tool_name {
                    if !matches_tool_pattern(matcher, tool_name) {
                        continue;
                    }
                }
            }

            // Execute with timeout
            let result = tokio::time::timeout(
                hook.timeout,
                execute_hook_handler(&hook.handler, final_ctx.clone()),
            ).await;

            match result {
                Ok(HookResult::Reject { reason }) => {
                    return HookResult::Reject { reason };
                }
                Ok(HookResult::ModifyInput { new_input }) => {
                    final_ctx.tool_input = Some(new_input);
                }
                Ok(HookResult::Continue) => {}
                Err(_) => {
                    // Timeout - log and continue
                    eprintln!("Hook timeout: {:?}", hook.name);
                }
            }
        }

        // Check if input was modified
        if final_ctx.tool_input != ctx.tool_input {
            if let Some(new_input) = final_ctx.tool_input {
                return HookResult::ModifyInput { new_input };
            }
        }

        HookResult::Continue
    }

    /// Remove once-executed hooks
    pub fn cleanup_once_hooks(&mut self) {
        for hooks in self.hooks.values_mut() {
            hooks.retain(|h| !h.once);
        }
    }
}

fn matches_tool_pattern(pattern: &str, tool_name: &str) -> bool {
    if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len() - 1];
        tool_name.starts_with(prefix)
    } else {
        pattern == tool_name
    }
}
```

## Hook Handler Execution

```rust
async fn execute_hook_handler(
    handler: &HookHandler,
    ctx: HookContext,
) -> HookResult {
    match handler {
        HookHandler::Shell { command, pass_context, parse_result } => {
            execute_shell_hook(command, &ctx, *pass_context, *parse_result).await
        }
        HookHandler::Webhook { url, method, headers } => {
            execute_webhook_hook(url, method, headers, &ctx).await
        }
        HookHandler::Inline { callback } => {
            callback(ctx)
        }
    }
}

async fn execute_shell_hook(
    command: &str,
    ctx: &HookContext,
    pass_context: bool,
    parse_result: bool,
) -> HookResult {
    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c").arg(command);
    cmd.current_dir(&ctx.cwd);

    // Set environment
    cmd.env("HOOK_EVENT", format!("{:?}", ctx.event));
    if let Some(tool) = &ctx.tool_name {
        cmd.env("HOOK_TOOL_NAME", tool);
    }
    cmd.env("HOOK_SESSION_ID", &ctx.session_id);

    if pass_context {
        cmd.stdin(Stdio::piped());
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("Failed to spawn hook process");

    // Pass context as JSON to stdin
    if pass_context {
        if let Some(mut stdin) = child.stdin.take() {
            let json = serde_json::to_string(&ctx).unwrap();
            stdin.write_all(json.as_bytes()).await.ok();
        }
    }

    let output = child.wait_with_output().await;

    match output {
        Ok(out) if out.status.success() => {
            if parse_result {
                // Parse stdout as HookResult
                if let Ok(result) = serde_json::from_slice(&out.stdout) {
                    return result;
                }
            }
            HookResult::Continue
        }
        Ok(out) => {
            // Non-zero exit = reject
            let stderr = String::from_utf8_lossy(&out.stderr);
            HookResult::Reject {
                reason: format!("Hook failed: {stderr}"),
            }
        }
        Err(e) => {
            HookResult::Reject {
                reason: format!("Hook error: {e}"),
            }
        }
    }
}

async fn execute_webhook_hook(
    url: &str,
    method: &HttpMethod,
    headers: &HashMap<String, String>,
    ctx: &HookContext,
) -> HookResult {
    let client = reqwest::Client::new();

    let mut request = match method {
        HttpMethod::Post => client.post(url),
        HttpMethod::Put => client.put(url),
    };

    for (key, value) in headers {
        request = request.header(key, value);
    }

    request = request.json(ctx);

    match request.send().await {
        Ok(response) if response.status().is_success() => {
            // Try to parse response as HookResult
            if let Ok(result) = response.json::<HookResult>().await {
                return result;
            }
            HookResult::Continue
        }
        Ok(response) => {
            HookResult::Reject {
                reason: format!("Webhook failed: {}", response.status()),
            }
        }
        Err(e) => {
            HookResult::Reject {
                reason: format!("Webhook error: {e}"),
            }
        }
    }
}
```

## Configuration

### TOML Configuration

```toml
# ~/.config/cocode/hooks.toml

[[hooks]]
event = "PreToolUse"
tool_matcher = "Bash*"
timeout_sec = 60

[hooks.shell]
command = "python ~/.config/cocode/hooks/validate_bash.py"
pass_context = true
parse_result = true

[[hooks]]
event = "PostToolUse"
tool_matcher = "Edit"

[hooks.webhook]
url = "https://api.example.com/hooks/file-changed"
method = "Post"
headers = { Authorization = "Bearer ${HOOK_API_KEY}" }

[[hooks]]
event = "SessionStart"
once = true

[hooks.shell]
command = "echo 'Session started' >> ~/.cocode/session.log"
```

### Loading Hooks

```rust
/// Load hooks from configuration
pub fn load_hooks_from_config(config_path: &Path) -> Result<HookRegistry, HookError> {
    let content = std::fs::read_to_string(config_path)?;
    let config: HooksConfig = toml::from_str(&content)?;

    let mut registry = HookRegistry::new();

    for hook_config in config.hooks {
        let handler = match (hook_config.shell, hook_config.webhook) {
            (Some(shell), None) => HookHandler::Shell {
                command: shell.command,
                pass_context: shell.pass_context.unwrap_or(false),
                parse_result: shell.parse_result.unwrap_or(false),
            },
            (None, Some(webhook)) => HookHandler::Webhook {
                url: webhook.url,
                method: webhook.method.unwrap_or_default(),
                headers: webhook.headers.unwrap_or_default(),
            },
            _ => continue,
        };

        registry.register(HookDefinition {
            event: parse_event_type(&hook_config.event)?,
            handler,
            once: hook_config.once.unwrap_or(false),
            timeout: Duration::from_secs(
                hook_config.timeout_sec.unwrap_or(600) as u64
            ),
            name: hook_config.name,
            tool_matcher: hook_config.tool_matcher,
        });
    }

    Ok(registry)
}
```

## Integration with Agent Loop

```rust
impl AgentLoop {
    async fn execute_tool(
        &mut self,
        tool: &dyn Tool,
        input: Value,
        tool_use_id: &str,
    ) -> ToolExecutionResult {
        // Build hook context
        let ctx = HookContext {
            event: HookEventType::PreToolUse,
            tool_name: Some(tool.name().to_string()),
            tool_input: Some(input.clone()),
            session_id: self.session_id.clone(),
            cwd: self.cwd.clone(),
            ..Default::default()
        };

        // Execute PreToolUse hooks
        let hook_result = self.hooks.execute(HookEventType::PreToolUse, ctx).await;

        let final_input = match hook_result {
            HookResult::Reject { reason } => {
                return ToolExecutionResult::error(tool_use_id, reason);
            }
            HookResult::ModifyInput { new_input } => new_input,
            HookResult::Continue => input,
        };

        // Execute tool
        let result = tool.call(final_input.clone(), &self.ctx, tool_use_id).await;

        // Execute PostToolUse or PostToolUseFailure hooks
        match &result {
            Ok(output) => {
                let ctx = HookContext {
                    event: HookEventType::PostToolUse,
                    tool_name: Some(tool.name().to_string()),
                    tool_input: Some(final_input),
                    tool_output: Some(output.clone()),
                    session_id: self.session_id.clone(),
                    cwd: self.cwd.clone(),
                    ..Default::default()
                };
                self.hooks.execute(HookEventType::PostToolUse, ctx).await;
            }
            Err(e) => {
                let ctx = HookContext {
                    event: HookEventType::PostToolUseFailure,
                    tool_name: Some(tool.name().to_string()),
                    tool_input: Some(final_input),
                    error: Some(e.to_string()),
                    session_id: self.session_id.clone(),
                    cwd: self.cwd.clone(),
                    ..Default::default()
                };
                self.hooks.execute(HookEventType::PostToolUseFailure, ctx).await;
            }
        }

        // Emit HookExecuted event
        self.emit(LoopEvent::HookExecuted {
            hook_type: HookEventType::PreToolUse,
            hook_name: tool.name().to_string(),
        }).await;

        result.into()
    }
}
```

## Example Use Cases

### 1. Validate Bash Commands

```toml
[[hooks]]
event = "PreToolUse"
tool_matcher = "Bash"
name = "validate-bash"

[hooks.shell]
command = '''
python3 -c "
import sys, json
ctx = json.load(sys.stdin)
cmd = ctx.get('tool_input', {}).get('command', '')
# Reject dangerous commands
if any(d in cmd for d in ['rm -rf /', 'sudo rm', ':(){:|:&};:']):
    print(json.dumps({'Reject': {'reason': 'Dangerous command blocked'}}))
    sys.exit(0)
print(json.dumps('Continue'))
"
'''
pass_context = true
parse_result = true
```

### 2. Log File Changes

```toml
[[hooks]]
event = "PostToolUse"
tool_matcher = "Edit"
name = "log-edits"

[hooks.shell]
command = '''
echo "$(date): File edited - $HOOK_TOOL_NAME" >> ~/.cocode/edits.log
'''
```

### 3. Notify on Session Start

```toml
[[hooks]]
event = "SessionStart"
name = "session-notify"
once = true

[hooks.webhook]
url = "https://hooks.slack.com/services/xxx"
method = "Post"
```

### 4. Custom Permission Check

```rust
// Inline hook for custom permission logic
registry.register(HookDefinition {
    event: HookEventType::PreToolUse,
    tool_matcher: Some("Write".to_string()),
    handler: HookHandler::Inline {
        callback: Arc::new(|ctx| {
            if let Some(input) = &ctx.tool_input {
                let path = input.get("file_path").and_then(|v| v.as_str());
                if let Some(p) = path {
                    // Block writes to sensitive paths
                    if p.contains(".env") || p.contains("credentials") {
                        return HookResult::Reject {
                            reason: "Cannot write to sensitive files".to_string(),
                        };
                    }
                }
            }
            HookResult::Continue
        }),
    },
    ..Default::default()
});
```

## Summary: Hook Event Flow

```
Session Start
    │
    ├─► SessionStart hooks
    │
    ▼
User Message
    │
    ├─► PrePromptSubmit hooks (can modify prompt)
    │
    ├─► PostUserMessage hooks
    │
    ▼
Tool Execution
    │
    ├─► PreToolUse hooks (can reject/modify)
    │         │
    │         ▼
    │    Tool.call()
    │         │
    │         ├─► PostToolUse hooks (on success)
    │         └─► PostToolUseFailure hooks (on error)
    │
    ▼
Subagent Spawn
    │
    ├─► SubagentStart hooks
    │         │
    │         ▼
    │    Subagent execution
    │         │
    │         ▼
    └─► SubagentStop hooks
    │
    ▼
Compaction
    │
    ├─► PreCompact hooks
    │
    ▼
Session End
    │
    └─► SessionEnd hooks
```

## Timeouts and Limits

| Parameter | Default | Description |
|-----------|---------|-------------|
| Hook timeout | 10 min | Maximum execution time per hook |
| Shell command timeout | 60 sec | Recommended shell hook timeout |
| Webhook timeout | 30 sec | HTTP request timeout |
| Max hooks per event | None | No limit |
