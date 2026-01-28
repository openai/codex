# Hooks System Architecture

## Overview

The hooks system allows customization of agent behavior at key lifecycle points. Hooks can observe, modify, or reject operations based on custom logic.

**Key Features:**
- **12 Event Types**: Tool lifecycle, user interaction, session lifecycle, subagents, compaction, notifications, permissions
- **5 Handler Types**: Command (shell), Prompt (LLM), Agent (sub-agent), Webhook, Inline
- **3 Result Types**: Continue, reject, modify input
- **4 Execution Outcomes**: Success, blocking, non_blocking_error, cancelled
- **4 Scoping Levels**: Policy > Plugin > Session > Skill
- **2 Execution Contexts**: REPL (interactive) and Non-REPL (batch)
- **Configurable Timeout**: Default 10 minutes per hook (5s for special hooks)

## Architecture (Claude Code v2.1.7 Aligned)

```
┌─────────────────────────────────────────────────────────────────┐
│                        Agent Loop                                │
│                                                                  │
│  Tool call → Validation → PreToolUse hooks → Permissions → Exec │
│                              ↓ (can reject/modify)               │
│                         HookResult::Reject → Return error        │
│                         HookResult::ModifyInput → Use new input  │
│                              ↓                                   │
│                         Tool Execution → PostToolUse hooks       │
│                                                                  │
│  Order: enabled → validation → hooks → permissions → execution   │
└─────────────────────────────────────────────────────────────────┘
```

**Key Points:**
- Validation runs BEFORE hooks (modified input bypasses validation)
- Permissions run AFTER hooks (on possibly-modified input)
- This matches Claude Code v2.1.7's `packages/core/src/tools/execution.ts`

## Hook Event Types (12 Types)

### Claude Code v2.1.7 Alignment

| Event | cocode-rs | Claude Code | Category |
|-------|-----------|-------------|----------|
| PreToolUse | ✓ | ✓ | Tool Lifecycle |
| PostToolUse | ✓ | ✓ | Tool Lifecycle |
| PostToolUseFailure | ✓ | ✓ | Tool Lifecycle |
| UserPromptSubmit | ✓ | ✓ | User Interaction |
| SessionStart | ✓ | ✓ | Session Lifecycle |
| SessionEnd | ✓ | ✓ | Session Lifecycle |
| Stop | ✓ | ✓ | Session Lifecycle |
| SubagentStart | ✓ | ✓ | Subagent Lifecycle |
| SubagentStop | ✓ | ✓ | Subagent Lifecycle |
| PreCompact | ✓ | ✓ | Compaction |
| Notification | ✓ | ✓ | Notifications |
| PermissionRequest | ✓ | ✓ | Permissions |

### Event Type Definition

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
    UserPromptSubmit,

    // ============ Session Lifecycle ============

    /// Session started
    SessionStart,

    /// Session ending
    SessionEnd,

    /// Agent stop requested
    Stop,

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
    Notification,

    // ============ Permissions ============

    /// Permission dialog shown
    PermissionRequest,
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

## Hook Matcher Patterns

### Claude Code v2.1.7 Alignment

| Pattern Type | Syntax | Example | Description |
|--------------|--------|---------|-------------|
| Exact | `"ToolName"` | `"Write"` | Matches only "Write" |
| Wildcard | `"Prefix*"` | `"Bash*"` | Matches "Bash", "BashExec", etc. |
| OR | `"A\|B\|C"` | `"Write\|Read\|Edit"` | Matches any of listed tools |
| Regex | `"^pattern.*"` | `"^Bash.*"` | Full regex matching |
| All | `""` | `""` | Matches all values |

```rust
/// Hook matcher for filtering events
#[derive(Debug, Clone)]
pub struct HookMatcher {
    pub pattern: String,
    pub match_type: MatchType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchType {
    Exact,
    Wildcard,
    Or,
    Regex,
    All,
}

impl HookMatcher {
    /// Parse matcher from string (auto-detect type)
    pub fn parse(pattern: &str) -> Self {
        let match_type = if pattern.is_empty() {
            MatchType::All
        } else if pattern.contains('|') {
            MatchType::Or
        } else if pattern.starts_with('^') || pattern.contains(".*") {
            MatchType::Regex
        } else if pattern.ends_with('*') {
            MatchType::Wildcard
        } else {
            MatchType::Exact
        };
        Self { pattern: pattern.to_string(), match_type }
    }

    /// Check if value matches pattern
    pub fn matches(&self, value: &str) -> bool {
        match self.match_type {
            MatchType::All => true,
            MatchType::Exact => self.pattern == value,
            MatchType::Wildcard => {
                let prefix = &self.pattern[..self.pattern.len() - 1];
                value.starts_with(prefix)
            }
            MatchType::Or => {
                self.pattern.split('|').any(|p| p == value)
            }
            MatchType::Regex => {
                regex::Regex::new(&self.pattern)
                    .map(|re| re.is_match(value))
                    .unwrap_or(false)
            }
        }
    }
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

    /// Matcher for tool-specific hooks
    pub matcher: Option<HookMatcher>,
}

impl Default for HookDefinition {
    fn default() -> Self {
        Self {
            event: HookEventType::PreToolUse,
            handler: HookHandler::Inline { callback: Arc::new(|_| HookResult::Continue) },
            once: false,
            timeout: Duration::from_secs(600),  // 10 minutes
            name: None,
            matcher: None,
        }
    }
}
```

## Hook Handlers (5 Types)

### Claude Code v2.1.7 Alignment

| Handler | cocode-rs | Claude Code | Description |
|---------|-----------|-------------|-------------|
| Command | ✓ (Shell) | ✓ (command) | Execute shell command with JSON I/O |
| Prompt | ✓ | ✓ | LLM-based verification |
| Agent | ✓ | ✓ | Sub-agent verification |
| Webhook | ✓ | - | HTTP endpoint (cocode extension) |
| Inline | ✓ | ✓ (callback/function) | Native callback |

**Note:** Claude Code uses `callback` and `function` types for internal hooks.
Webhook is a cocode-rs extension for HTTP-based hooks not present in Claude Code.

```rust
/// Hook handler types (Claude Code v2.1.7 aligned)
#[derive(Debug, Clone)]
pub enum HookHandler {
    /// Execute shell command (Claude Code: command)
    Command {
        command: String,
        /// Execution timeout in seconds
        timeout: Option<i32>,
        /// Status message during execution
        status_message: Option<String>,
        /// Run once and auto-remove
        once: bool,
    },

    /// LLM-based verification (Claude Code: prompt)
    Prompt {
        prompt: String,
        /// Model override
        model: Option<String>,
        /// Execution timeout
        timeout: Option<i32>,
        once: bool,
    },

    /// Sub-agent verification (Claude Code: agent)
    Agent {
        prompt: String,
        model: Option<String>,
        timeout: Option<i32>,
        once: bool,
    },

    /// Call webhook endpoint (cocode extension)
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

### Prompt Hook Execution (Claude Code v2.1.7 Aligned)

The Prompt handler uses LLM to verify operations:

1. Substitute `$ARGUMENTS` placeholder with hook input JSON
2. Build message with conversation history
3. Add assistant prefix `{` to force JSON output
4. Query LLM with system prompt: `"Return only valid JSON: {"ok": true} or {"ok": false, "reason": "..."}"`
5. Parse and validate response
6. `ok=false` → outcome="blocking", stops execution

```rust
async fn execute_prompt_hook(
    prompt: &str,
    ctx: &HookContext,
    model: Option<&str>,
) -> HookResult {
    // Replace $ARGUMENTS with context JSON
    let context_json = serde_json::to_string(&ctx).unwrap();
    let prompt_with_args = prompt.replace("$ARGUMENTS", &context_json);

    // Build request with assistant prefix to force JSON
    let request = LLMRequest {
        model: model.unwrap_or("default"),
        messages: vec![
            Message::system("Return only valid JSON: {\"ok\": true} or {\"ok\": false, \"reason\": \"...\"}"),
            Message::user(&prompt_with_args),
        ],
        // Assistant prefix forces JSON output
        assistant_prefix: Some("{".to_string()),
    };

    let response = llm_client.complete(request).await?;

    // Parse response (prepend the prefix back)
    let json_str = format!("{{{}", response.content);
    match serde_json::from_str::<PromptHookResponse>(&json_str) {
        Ok(resp) if resp.ok => HookResult::Continue,
        Ok(resp) => HookResult::Reject {
            reason: resp.reason.unwrap_or_else(|| "Hook rejected".to_string()),
        },
        Err(_) => HookResult::Continue,  // Parse error = continue
    }
}

#[derive(Deserialize)]
struct PromptHookResponse {
    ok: bool,
    reason: Option<String>,
}
```

### Agent Hook Execution (Claude Code v2.1.7 Aligned)

The Agent handler spawns a sub-agent for verification:

- Spawns sub-agent with **limited tool set** (no Task tool)
- Supports `$ARGUMENTS` placeholder
- Can search conversation transcript
- **Max 50 turns** per agent
- Returns `{ ok: boolean, reason?: string }`
- Timeout/cancellation → outcome="cancelled"

```rust
async fn execute_agent_hook(
    prompt: &str,
    ctx: &HookContext,
    model: Option<&str>,
    timeout: Duration,
) -> HookResult {
    // Replace $ARGUMENTS with context JSON
    let context_json = serde_json::to_string(&ctx).unwrap();
    let prompt_with_args = prompt.replace("$ARGUMENTS", &context_json);

    // Create sub-agent with limited tools (no Task)
    let sub_agent = SubAgent::new(SubAgentConfig {
        model: model.unwrap_or("default"),
        max_turns: 50,  // Hard limit
        allowed_tools: vec!["Read", "Grep", "Glob"],  // Limited set
        system_prompt: "You are a verification agent. Analyze the operation and return JSON: {\"ok\": true} or {\"ok\": false, \"reason\": \"...\"}",
    });

    // Execute with timeout
    let result = tokio::time::timeout(timeout, sub_agent.run(&prompt_with_args)).await;

    match result {
        Ok(Ok(response)) => {
            match serde_json::from_str::<AgentHookResponse>(&response) {
                Ok(resp) if resp.ok => HookResult::Continue,
                Ok(resp) => HookResult::Reject {
                    reason: resp.reason.unwrap_or_else(|| "Agent rejected".to_string()),
                },
                Err(_) => HookResult::Continue,
            }
        }
        Ok(Err(_)) => HookResult::Continue,  // Agent error = continue
        Err(_) => HookResult::Continue,      // Timeout = cancelled, continue
    }
}

#[derive(Deserialize)]
struct AgentHookResponse {
    ok: bool,
    reason: Option<String>,
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

## Hook Execution Outcomes (Claude Code v2.1.7 Aligned)

Distinct from `HookResult` (what hook returns), **outcomes** describe the execution status:

| Outcome | Description | Effect |
|---------|-------------|--------|
| `success` | Hook completed successfully | Continue execution |
| `blocking` | Hook returned rejection (`ok=false`) | Stop tool execution |
| `non_blocking_error` | Hook failed but non-fatal | Continue with warning |
| `cancelled` | Hook timeout/abort | Continue |

```rust
/// Hook execution outcome (distinct from HookResult)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookOutcome {
    /// Hook completed successfully
    Success,
    /// Hook rejected the operation (blocking)
    Blocking,
    /// Hook errored but non-fatal (warning)
    NonBlockingError,
    /// Hook was cancelled (timeout/abort)
    Cancelled,
}

impl HookOutcome {
    pub fn from_result(result: &HookResult, execution_error: Option<&str>) -> Self {
        match (result, execution_error) {
            (HookResult::Reject { .. }, _) => Self::Blocking,
            (_, Some(_)) => Self::NonBlockingError,
            _ => Self::Success,
        }
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

## Hook Scoping Hierarchy (Claude Code v2.1.7 Aligned)

Hooks are aggregated from multiple sources with priority order:

| Scope | Priority | Source | Skipped When |
|-------|----------|--------|--------------|
| Policy | 1 (Highest) | `policySettings.hooks` | Never |
| Plugin | 2 | Plugin manifests | `allowManagedHooksOnly=true` |
| Session | 3 | Session state | `allowManagedHooksOnly=true` |
| Skill | 4 (Lowest) | Skill frontmatter | `allowManagedHooksOnly=true` |

**Policy hooks ALWAYS run.** Other hooks can be disabled with settings.

```rust
/// Hook scope hierarchy (Claude Code v2.1.7 aligned)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HookScope {
    /// Policy-level hooks (highest priority, always run)
    Policy = 0,
    /// Plugin-defined hooks
    Plugin = 1,
    /// Session-registered hooks
    Session = 2,
    /// Skill frontmatter hooks (lowest priority)
    Skill = 3,
}

impl HookScope {
    /// Check if hook should run based on settings
    pub fn should_run(&self, allow_managed_hooks_only: bool) -> bool {
        match self {
            HookScope::Policy => true,  // Always run
            _ => !allow_managed_hooks_only,
        }
    }
}
```

### Hook Aggregation

```rust
/// Aggregate hooks from all scopes
pub fn aggregate_hooks(
    policy_hooks: &[HookDefinition],
    plugin_hooks: &[HookDefinition],
    session_hooks: &[HookDefinition],
    skill_hooks: &[HookDefinition],
    settings: &HookSettings,
) -> Vec<(HookScope, HookDefinition)> {
    let mut all_hooks = Vec::new();

    // Policy hooks always included
    for hook in policy_hooks {
        all_hooks.push((HookScope::Policy, hook.clone()));
    }

    // Other hooks only if not disabled
    if !settings.allow_managed_hooks_only {
        for hook in plugin_hooks {
            all_hooks.push((HookScope::Plugin, hook.clone()));
        }
        for hook in session_hooks {
            all_hooks.push((HookScope::Session, hook.clone()));
        }
        for hook in skill_hooks {
            all_hooks.push((HookScope::Skill, hook.clone()));
        }
    }

    // Sort by priority (lower = higher priority)
    all_hooks.sort_by_key(|(scope, _)| *scope);
    all_hooks
}
```

## Hook Settings (Claude Code v2.1.7 Aligned)

| Setting | Default | Effect |
|---------|---------|--------|
| `disableAllHooks` | `false` | Disables ALL hooks including StatusLine |
| `allowManagedHooksOnly` | `false` | Only Policy hooks run (Plugin/Session/Skill skipped) |

```rust
/// Hook system settings
#[derive(Debug, Clone, Default)]
pub struct HookSettings {
    /// Disable all hooks (including StatusLine)
    #[serde(default)]
    pub disable_all_hooks: bool,

    /// Only run policy-level hooks (managed hooks)
    #[serde(default)]
    pub allow_managed_hooks_only: bool,
}

impl HookSettings {
    pub fn should_execute_hooks(&self) -> bool {
        !self.disable_all_hooks
    }
}
```

### Configuration Example

```json
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "command": "validate.sh" }
    ]
  },
  "disableAllHooks": false,
  "allowManagedHooksOnly": false
}
```

```toml
# hooks.toml
disable_all_hooks = false
allow_managed_hooks_only = false

[[hooks]]
event = "PreToolUse"
tool_matcher = "Bash"
[hooks.shell]
command = "validate.sh"
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

## Integration with Agent Loop (Claude Code v2.1.7 Aligned)

The execution order is: **enabled → validation → hooks → permissions → execution**

This matches Claude Code v2.1.7's actual implementation in `packages/core/src/tools/execution.ts`.

```rust
impl AgentLoop {
    async fn execute_tool(
        &mut self,
        tool: &dyn Tool,
        input: Value,
        tool_use_id: &str,
    ) -> ToolExecutionResult {
        // 1. Check if enabled
        if !tool.is_enabled(&self.ctx) {
            return ToolExecutionResult::error(tool_use_id, "Tool is disabled");
        }

        // 2. Validation FIRST (before hooks) - Claude Code v2.1.7 aligned
        if let Err(e) = validate_input(&input, tool.input_schema()) {
            return ToolExecutionResult::error(tool_use_id, e);
        }

        // 3. PreToolUse hooks (can modify input)
        let ctx = HookContext {
            event: HookEventType::PreToolUse,
            tool_name: Some(tool.name().to_string()),
            tool_input: Some(input.clone()),
            session_id: self.session_id.clone(),
            cwd: self.cwd.clone(),
            ..Default::default()
        };

        let hook_result = self.hooks.execute(HookEventType::PreToolUse, ctx).await;

        let final_input = match hook_result {
            HookResult::Reject { reason } => {
                return ToolExecutionResult::error(tool_use_id, reason);
            }
            HookResult::ModifyInput { new_input } => {
                // WARNING: Modified input NOT re-validated against schema
                // This matches Claude Code v2.1.7 behavior
                new_input
            }
            HookResult::Continue => input,
        };

        // 4. Permission check (AFTER hooks, on possibly-modified input)
        let permission = tool.check_permissions(&final_input, &self.ctx).await;
        match permission {
            PermissionResult::Denied { reason } => {
                return ToolExecutionResult::error(tool_use_id, reason);
            }
            PermissionResult::NeedsApproval { request } => {
                let approved = request_user_approval(request, &self.ctx).await?;
                if !approved {
                    return ToolExecutionResult::error(tool_use_id, "User denied permission");
                }
            }
            PermissionResult::Allowed => {}
        }

        // 5. Execute tool
        let result = tool.call(final_input.clone(), &self.ctx, tool_use_id).await;

        // 6. PostToolUse or PostToolUseFailure hooks
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

## Hook Input Modification Security Model (Claude Code v2.1.7 Aligned)

### Execution Flow with Input Modification

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

### Key Security Points

| Aspect | Behavior | Implication |
|--------|----------|-------------|
| Validation | Runs BEFORE hooks | Hooks can bypass schema constraints |
| Permissions | Runs AFTER hooks | Modified input is permission-checked |
| Re-validation | Does NOT occur | Trust is placed in hooks |

### Recommendations

1. **Policy hooks only:** Consider allowing `updatedInput` only from policy-level hooks
2. **Audit logging:** Log when hooks modify input for security review
3. **Sensitive tools:** Add extra validation in `tool.call()` for security-critical tools
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

## Summary: Hook Event Flow (Claude Code v2.1.7 Aligned)

```
Session Start
    │
    ├─► SessionStart hooks
    │
    ▼
User Message
    │
    ├─► UserPromptSubmit hooks (can modify prompt)
    │
    ▼
Tool Execution (enabled → validation → hooks → permissions → execute)
    │
    ├─► Enabled check
    │
    ├─► Validation (schema + custom) ← Runs BEFORE hooks
    │
    ├─► PreToolUse hooks (can reject/modify input)
    │         │
    │         └─► Modified input does NOT go through re-validation
    │
    ├─► Permission check ← Runs AFTER hooks (on modified input)
    │
    ├─► Tool.call() ← Uses final input
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

## Skill-Level Hooks (Claude Code v2.1.7 Aligned)

Skills can define hooks in their frontmatter that are registered when the skill is executed:

```yaml
# In SKILL.md frontmatter
hooks:
  PreToolUse:
    - matcher: "Write|Edit"
      hooks:
        - type: command
          command: "npm run lint --fix"
          timeout: 60
          once: true
  PostToolUse:
    - matcher: "Bash"
      hooks:
        - type: command
          command: "echo 'Command completed'"
```

### Registration Flow

```
Skill Execution Start
    │
    ├─► Parse skill frontmatter hooks
    │
    ├─► Register hooks in session state
    │       │
    │       └─► registerSkillFrontmatterHooks(skill)
    │
    ├─► Execute skill prompt with LLM
    │
    ├─► Tool uses trigger registered hooks
    │       │
    │       ├─► PreToolUse hooks (can reject/modify)
    │       │
    │       ├─► Tool execution
    │       │
    │       └─► PostToolUse hooks (notification)
    │
    └─► Hooks with `once: true` auto-removed after success
```

### Hook State Management

```rust
/// Session hook state
pub struct HookState {
    /// Active hooks by event type
    hooks: HashMap<HookEventType, Vec<RegisteredHook>>,
}

pub struct RegisteredHook {
    pub definition: HookDefinition,
    pub source: HookSource,
    pub on_success: Option<Box<dyn FnOnce() + Send>>,
}

pub enum HookSource {
    Config,           // From settings/config file
    Plugin,           // From plugin
    Skill { name: String },  // From skill frontmatter
}

impl HookState {
    /// Register skill frontmatter hooks
    pub fn register_skill_hooks(&mut self, skill: &Skill) {
        if let Some(hooks_config) = &skill.hooks {
            for (event, matchers) in hooks_config {
                for matcher_config in matchers {
                    for hook in &matcher_config.hooks {
                        self.add_hook(RegisteredHook {
                            definition: hook.clone(),
                            source: HookSource::Skill { name: skill.name.clone() },
                            on_success: if hook.once {
                                Some(Box::new(|| { /* remove hook */ }))
                            } else {
                                None
                            },
                        });
                    }
                }
            }
        }
    }
}
```

## Parallel Hook Execution (Claude Code v2.1.7 Aligned)

Multiple hooks for the same event execute concurrently:

```rust
impl HookRegistry {
    /// Execute hooks in parallel
    pub async fn execute_parallel(
        &mut self,
        event: HookEventType,
        ctx: HookContext,
    ) -> Vec<HookResult> {
        let hooks = match self.hooks.get(&event) {
            Some(h) => h.clone(),
            None => return vec![],
        };

        // Filter matching hooks
        let matching: Vec<_> = hooks.iter()
            .filter(|h| h.matcher.as_ref().map_or(true, |m| {
                ctx.tool_name.as_ref().map_or(false, |t| m.matches(t))
            }))
            .collect();

        // Execute in parallel
        let futures: Vec<_> = matching.iter()
            .map(|h| self.execute_single(h, ctx.clone()))
            .collect();

        futures::future::join_all(futures).await
    }

    /// Aggregate results (first Reject wins)
    pub fn aggregate_results(results: Vec<HookResult>) -> HookResult {
        for result in &results {
            if let HookResult::Reject { .. } = result {
                return result.clone();
            }
        }
        // Find ModifyInput or return Continue
        results.into_iter()
            .find(|r| matches!(r, HookResult::ModifyInput { .. }))
            .unwrap_or(HookResult::Continue)
    }
}
```

## Hook Output Schema (Claude Code v2.1.7 Aligned)

Hooks return structured JSON output:

```rust
/// Hook output (from command stdout)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookOutput {
    /// Continue or stop execution
    #[serde(default = "default_true")]
    pub r#continue: bool,

    /// Suppress hook output in UI
    #[serde(default)]
    pub suppress_output: bool,

    /// Reason for stopping
    pub stop_reason: Option<String>,

    /// Permission decision (for PermissionRequest)
    pub decision: Option<PermissionDecision>,

    /// Reason for decision
    pub reason: Option<String>,

    // Event-specific fields
    /// Updated tool input (for PreToolUse)
    pub updated_input: Option<Value>,

    /// Additional context (for PostToolUse)
    pub additional_context: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionDecision {
    Allow,
    Deny,
}
```

## Execution Contexts (Claude Code v2.1.7 Aligned)

Hook behavior varies based on execution context:

### REPL Context (Interactive)

- All hook types supported (Command, Prompt, Agent)
- Results streamed as async iterator
- Progress events visible to user
- User can cancel long-running hooks

```rust
/// REPL hook execution
pub async fn execute_hooks_repl(
    hooks: &[HookDefinition],
    ctx: HookContext,
) -> impl Stream<Item = HookProgressEvent> {
    async_stream::stream! {
        for hook in hooks {
            yield HookProgressEvent::Started { name: hook.name.clone() };
            let result = execute_hook(hook, ctx.clone()).await;
            yield HookProgressEvent::Completed { name: hook.name.clone(), result };
        }
    }
}
```

### Non-REPL Context (Background/Batch)

- **Only Command hooks supported**
- Prompt/Agent hooks return error (require LLM interaction)
- Results returned as array (not streaming)
- No user cancellation

```rust
/// Non-REPL hook execution
pub async fn execute_hooks_non_repl(
    hooks: &[HookDefinition],
    ctx: HookContext,
) -> Result<Vec<HookResult>, HookError> {
    let mut results = Vec::new();

    for hook in hooks {
        match &hook.handler {
            HookHandler::Command { .. } => {
                results.push(execute_command_hook(hook, &ctx).await?);
            }
            HookHandler::Prompt { .. } | HookHandler::Agent { .. } => {
                // Not supported in non-REPL context
                return Err(HookError::UnsupportedInNonRepl {
                    handler: format!("{:?}", hook.handler),
                });
            }
            _ => results.push(HookResult::Continue),
        }
    }

    Ok(results)
}
```

## Special Settings (Not Hook Events)

**Important:** StatusLine and FileSuggestion are **settings configurations** in Claude Code, not hook event types. They are handled via `settings.statusLine` and `settings.fileSuggestion` with dedicated 5-second timeouts.

### StatusLine Setting

Provides custom status line content for TUI integration (configured via `settings.statusLine`):

| Property | Value |
|----------|-------|
| Timeout | 5 seconds |
| Disabled when | `disableAllHooks=true` |
| Input | JSON with session context |
| Output | Multi-line status text (newline-separated) |

```rust
/// StatusLine setting execution
pub async fn execute_status_line_setting(
    command: &str,
    ctx: &StatusLineContext,
) -> Option<Vec<String>> {
    // Note: StatusLine is a settings configuration, not a hook event
    // It uses a simple command string, not a HookDefinition
    // 5 second timeout for StatusLine hooks
    let timeout = Duration::from_secs(5);

    let result = tokio::time::timeout(timeout, async {
        execute_command_hook(hook, ctx).await
    }).await;

    match result {
        Ok(Ok(output)) => {
            // Parse multi-line output
            Some(output.lines().map(String::from).collect())
        }
        _ => None,  // Timeout or error = no status
    }
}
```

### FileSuggestion Setting

Provides custom file suggestions for `@` mentions in TUI (configured via `settings.fileSuggestion`):

| Property | Value |
|----------|-------|
| Timeout | 5 seconds |
| Input | Current query prefix |
| Output | Newline-separated file paths |

```rust
/// FileSuggestion setting execution
pub async fn execute_file_suggestion_setting(
    command: &str,
    query: &str,
) -> Vec<PathBuf> {
    // Note: FileSuggestion is a settings configuration, not a hook event
    // It uses a simple command string, not a HookDefinition

    // 5 second timeout
    let timeout = Duration::from_secs(5);

    let result = tokio::time::timeout(timeout, async {
        execute_command_hook(hook, &ctx).await
    }).await;

    match result {
        Ok(Ok(output)) => {
            output.lines()
                .filter(|l| !l.is_empty())
                .map(PathBuf::from)
                .collect()
        }
        _ => vec![],  // Timeout or error = no suggestions
    }
}
```

## Async Hook Support (Claude Code v2.1.7 Aligned)

Hooks can run in the background using async mode:

```rust
/// Async hook configuration
#[derive(Debug, Clone)]
pub struct AsyncHookConfig {
    /// Run hook asynchronously (don't wait for completion)
    #[serde(default)]
    pub async_mode: bool,

    /// Timeout for async hooks (default: 15 seconds)
    #[serde(default = "default_async_timeout")]
    pub async_timeout: Duration,
}

fn default_async_timeout() -> Duration {
    Duration::from_secs(15)
}
```

### Async Hook Output Format

Return `{ "async": true }` to signal background execution:

```json
{
  "async": true,
  "asyncTimeout": 15000
}
```

```rust
/// Handle async hook response
pub fn handle_async_hook_response(output: &HookOutput) -> HookResult {
    if output.async_mode.unwrap_or(false) {
        // Hook will complete in background
        // Don't block on result
        HookResult::Continue
    } else {
        // Normal synchronous handling
        if output.continue_execution {
            HookResult::Continue
        } else {
            HookResult::Reject {
                reason: output.stop_reason.clone().unwrap_or_default(),
            }
        }
    }
}
```

### Background Hook Execution

```rust
/// Execute async hooks in background
pub fn spawn_async_hook(
    hook: HookDefinition,
    ctx: HookContext,
    timeout: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let _ = tokio::time::timeout(timeout, async {
            if let Err(e) = execute_hook_handler(&hook.handler, ctx).await {
                tracing::warn!("Async hook failed: {}", e);
            }
        }).await;
    })
}
```

## Timeouts and Limits

| Parameter | Default | Description |
|-----------|---------|-------------|
| Hook timeout | 10 min | Maximum execution time per hook |
| Shell command timeout | 60 sec | Recommended shell hook timeout |
| Webhook timeout | 30 sec | HTTP request timeout |
| StatusLine timeout | 5 sec | StatusLine hook timeout |
| FileSuggestion timeout | 5 sec | FileSuggestion hook timeout |
| Async hook timeout | 15 sec | Default async hook timeout |
| Max hooks per event | None | No limit |

## Claude Code v2.1.7 Alignment Summary

### Fully Aligned
- **Tool execution order: enabled → validation → hooks → permissions → execution**
- **Modified input from hooks does NOT go through re-validation**
- **Permission check happens AFTER hooks (on modified input)**
- 12 event types (PreToolUse, PostToolUse, etc.)
- Command handler with JSON I/O
- Prompt handler with `$ARGUMENTS` and assistant prefix
- Agent handler (50 turns max, limited tools)
- Hook scoping hierarchy (Policy > Plugin > Session > Skill)
- Hook settings (`disableAllHooks`, `allowManagedHooksOnly`)
- Hook execution outcomes (success, blocking, non_blocking_error, cancelled)
- Hook matcher patterns (exact, wildcard, OR, regex)
- REPL vs Non-REPL execution contexts
- StatusLine and FileSuggestion as settings configurations (5s timeout)
- Async hook support (`async: true`, `asyncTimeout`)
- Skill frontmatter hooks
- Parallel hook execution with aggregation
- Hook output schema validation
- `once` auto-cleanup mechanism

### cocode Extensions
- Webhook handler (HTTP endpoints)
- TOML configuration format (in addition to JSON)

### Environment Variables

| Variable | Description |
|----------|-------------|
| `HOOK_EVENT` | Current event type |
| `HOOK_TOOL_NAME` | Tool name (for tool events) |
| `HOOK_SESSION_ID` | Session ID |
| `CLAUDE_PROJECT_DIR` | Project root directory |
| `CLAUDE_PLUGIN_ROOT` | Plugin installation directory |
| `CLAUDE_ENV_FILE` | Path to env file (SessionStart only) |
