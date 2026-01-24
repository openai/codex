# Codex Subagent System v2 - Design Document

## 1. Overview

### 1.1 Background

This document describes the design and implementation of the Subagent System for codex-rs, a feature that allows the main agent to spawn specialized sub-agents for focused, autonomous tasks.

### 1.2 Goals

1. **Claude Code Compatibility**: Import and use Claude Code subagent YAML/MD definitions
2. **Modular Architecture**: Clean separation of concerns within `core/src/subagent/`
3. **Upstream Sync Friendly**: Minimal changes to existing core files, feature-gated
4. **Production Ready**: Robust error handling, cancellation, timeout support
5. **Gemini Best Practices**: Incorporate proven patterns (complete_task, Grace Period, structured I/O)

### 1.3 Key Improvements from Gemini-CLI

| Feature | Pattern | Benefit |
|---------|---------|---------|
| `complete_task` tool | Explicit completion signal | Clear termination, structured output |
| Grace Period | 60s recovery after timeout | Avoid losing work |
| OutputConfig | Schema validation | Structured, validated output |
| InputConfig | Typed parameters | Better type safety |
| ModelConfig | temp, top_p, thinkingBudget | Fine-grained model control |
| Activity Events | TOOL_CALL_START/END, THOUGHT_CHUNK | Better observability |

### 1.4 Non-Goals

- Full Claude Code agent feature parity (e.g., skills system, hooks)
- Custom agent definition UI
- Remote agent execution

---

## 2. Architecture

### 2.1 High-Level Design

```
┌─────────────────────────────────────────────────────────────────┐
│                        Main Agent Loop                          │
│                                                                 │
│  ┌─────────────┐    ┌─────────────────────────────────────┐    │
│  │ Tool Router │───▶│ SubagentToolHandler (Task tool)     │    │
│  └─────────────┘    │                                     │    │
│                     │  ┌─────────────────────────────────┐│    │
│                     │  │ AgentRegistry                   ││    │
│                     │  │  - Builtin agents (Explore,Plan)││    │
│                     │  │  - User agents (.claude/agents/)││    │
│                     │  └─────────────────────────────────┘│    │
│                     │                                     │    │
│                     │  ┌─────────────────────────────────┐│    │
│                     │  │ ToolFilter (three-tier)         ││    │
│                     │  │  - ALWAYS_BLOCKED               ││    │
│                     │  │  - NON_BUILTIN_BLOCKED          ││    │
│                     │  │  - Agent disallowedTools        ││    │
│                     │  └─────────────────────────────────┘│    │
│                     │                                     │    │
│                     │  ┌─────────────────────────────────┐│    │
│                     │  │ AgentExecutor                   ││    │
│                     │  │  - SubagentContext (isolated)   ││    │
│                     │  │  - Turn loop with timeout       ││    │
│                     │  │  - Cancellation support         ││    │
│                     │  └─────────────────────────────────┘│    │
│                     └─────────────────────────────────────┘    │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ BackgroundTaskStore (for async execution)               │   │
│  │  - DashMap<agent_id, BackgroundTask>                    │   │
│  │  - TaskOutputHandler retrieval                          │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Module Structure

```
codex-rs/core/src/subagent/
├── mod.rs                        # Module exports, public API
├── error.rs                      # SubagentErr error type
├── definition/
│   ├── mod.rs                    # AgentDefinition, ToolAccess, AgentModel, etc.
│   ├── parser.rs                 # YAML/MD parser with frontmatter support
│   ├── schema.rs                 # InputConfig → JSON Schema conversion (from Gemini)
│   └── builtin.rs                # Built-in Explore and Plan agents
├── registry.rs                   # AgentRegistry for agent discovery
├── executor/
│   ├── mod.rs                    # AgentExecutor, SubagentResult
│   ├── context.rs                # SubagentContext (isolated from parent)
│   ├── tool_filter.rs            # Three-tier tool restriction
│   ├── complete_task.rs          # complete_task tool generation (from Gemini)
│   └── grace_period.rs           # Grace Period recovery mechanism (from Gemini)
├── background.rs                 # BackgroundTaskStore, BackgroundTask
├── events.rs                     # SubagentActivityEvent types (from Gemini)
├── events_bridge.rs              # NEW: Event forwarding to parent (from current impl)
├── approval.rs                   # NEW: Approval routing modes (from current impl)
├── handlers/
│   ├── mod.rs                    # Handler exports
│   ├── task.rs                   # SubagentToolHandler (Task tool)
│   └── task_output.rs            # TaskOutputHandler (retrieve results)
└── prompts/
    ├── explore.md                # Explore agent system prompt
    └── plan.md                   # Plan agent system prompt
```

### 2.3 Component Relationships

```
AgentDefinition ─────────────────┐
       ▲                         │
       │ parse                   │ load
       │                         ▼
  YAML/MD file ◀─────────── AgentRegistry
                                 │
                                 │ get(agent_type)
                                 ▼
                          ToolFilter ────────────────┐
                                 │                   │ filter
                                 │                   ▼
                                 │              [ToolSpec]
                                 ▼
                          AgentExecutor
                                 │
                                 │ run(prompt)
                                 ▼
                          SubagentResult
```

---

## 3. Detailed Design

### 3.0 Error Types

#### SubagentErr (error.rs)

```rust
use thiserror::Error;

/// Errors specific to subagent execution
#[derive(Debug, Error)]
pub enum SubagentErr {
    /// Unknown agent type requested
    #[error("Unknown agent type: {0}")]
    UnknownAgentType(String),

    /// Agent definition parse error
    #[error("Agent definition parse error: {0}")]
    ParseError(String),

    /// Tool was rejected by the filter
    #[error("Tool '{0}' is not available in this subagent context")]
    ToolRejected(String),

    /// Model/LLM call error
    #[error("Model error: {0}")]
    ModelError(String),

    /// Transcript not found for resume
    #[error("Transcript not found for agent: {0}")]
    TranscriptNotFound(String),

    /// Approval request timed out
    #[error("Approval request timed out")]
    ApprovalTimeout,

    /// Execution was cancelled
    #[error("Subagent execution was cancelled")]
    Cancelled,

    /// Output validation failed
    #[error("Output validation failed: {0}")]
    OutputValidationError(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<SubagentErr> for CodexErr {
    fn from(err: SubagentErr) -> Self {
        match err {
            SubagentErr::Cancelled => CodexErr::Cancelled,
            SubagentErr::ModelError(msg) if msg.contains("context_length") => {
                CodexErr::ContextWindowExceeded(msg)
            }
            _ => CodexErr::Fatal(err.to_string()),
        }
    }
}
```

### 3.1 Agent Definition Types

#### 3.1.1 AgentDefinition (Claude Code Compatible + Gemini Enhanced)

```rust
/// Agent definition compatible with Claude Code YAML/MD format
/// Enhanced with Gemini patterns: ModelConfig, InputConfig, OutputConfig, PromptConfig
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentDefinition {
    /// Unique agent type identifier (e.g., "Explore", "Plan", "my-custom-agent")
    pub agent_type: String,

    /// Human-readable display name (NEW: from Gemini)
    #[serde(default)]
    pub display_name: Option<String>,

    /// Description of when to use this agent (shown to main agent)
    #[serde(default)]
    pub when_to_use: Option<String>,

    /// Tool access configuration
    #[serde(default)]
    pub tools: ToolAccess,

    /// Explicitly disallowed tools
    #[serde(default)]
    pub disallowed_tools: Vec<String>,

    /// Source of the agent definition
    #[serde(default)]
    pub source: AgentSource,

    /// Base directory for agent operations (defaults to parent cwd)
    #[serde(default)]
    pub base_dir: Option<PathBuf>,

    /// Detailed model configuration (NEW: from Gemini)
    #[serde(default)]
    pub model_config: ModelConfig,

    /// Whether to fork parent context (conversation history)
    #[serde(default)]
    pub fork_context: bool,

    /// Prompt configuration with systemPrompt and query (NEW: from Gemini)
    #[serde(default)]
    pub prompt_config: PromptConfig,

    /// Execution constraints (enhanced with gracePeriodSeconds)
    #[serde(default)]
    pub run_config: AgentRunConfig,

    /// Typed input parameters (NEW: from Gemini)
    #[serde(default)]
    pub input_config: Option<InputConfig>,

    /// Structured output configuration (NEW: from Gemini)
    #[serde(default)]
    pub output_config: Option<OutputConfig>,

    /// Features to disable for this agent (NEW: from current ReviewTask pattern)
    /// Example: ["WebSearchRequest", "ViewImageTool"]
    #[serde(default)]
    pub disabled_features: Vec<String>,

    /// Execution mode (NEW: from current implementation analysis)
    #[serde(default)]
    pub execution_mode: SubagentExecutionMode,

    /// Approval routing mode (NEW: from current implementation)
    #[serde(default)]
    pub approval_mode: ApprovalMode,
}

/// Execution mode determines how the subagent runs
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentExecutionMode {
    /// Lightweight: AgentExecutor with filtered tools (default)
    /// Fast, low overhead, good for read-only agents
    #[default]
    Lightweight,
    /// Full: Spawn complete Codex instance via codex_delegate.rs
    /// Required for agents that need approval handling or full features
    FullCodex,
}
```

#### 3.1.2 Supporting Types

```rust
/// Tool access configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum ToolAccess {
    /// All tools allowed (except disallowed)
    #[default]
    All,
    /// Specific list of tool names
    List(Vec<String>),
}

/// Agent source identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AgentSource {
    #[default]
    Builtin,
    User,
    Project,
}

/// Model selection for agent
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AgentModel {
    #[default]
    Inherit,  // Use parent's model
    Sonnet,
    Haiku,
    Opus,
    #[serde(untagged)]
    Custom(String),
}

/// Detailed model configuration (NEW: from Gemini)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfig {
    /// Model selection
    #[serde(default)]
    pub model: AgentModel,

    /// Temperature for sampling (0.0-2.0)
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Top-P sampling parameter
    #[serde(default = "default_top_p")]
    pub top_p: f32,

    /// Thinking budget (-1 = unlimited, 0 = disabled)
    #[serde(default)]
    pub thinking_budget: Option<i32>,
}

fn default_temperature() -> f32 { 0.7 }
fn default_top_p() -> f32 { 0.95 }

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            model: AgentModel::default(),
            temperature: default_temperature(),
            top_p: default_top_p(),
            thinking_budget: None,
        }
    }
}

/// Prompt configuration (NEW: from Gemini)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PromptConfig {
    /// System prompt template (supports ${variable} substitution)
    #[serde(default)]
    pub system_prompt: Option<String>,

    /// Query/trigger prompt (supports ${variable} substitution)
    /// Defaults to "Get Started!" if not provided
    #[serde(default)]
    pub query: Option<String>,

    /// Initial messages for few-shot prompting
    #[serde(default)]
    pub initial_messages: Option<Vec<Message>>,
}

/// Execution constraints (enhanced with grace period)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunConfig {
    /// Maximum execution time in seconds
    #[serde(default = "default_max_time_seconds")]
    pub max_time_seconds: i32,

    /// Maximum number of conversation turns
    #[serde(default = "default_max_turns")]
    pub max_turns: i32,

    /// Grace period in seconds for recovery after timeout/max_turns (NEW: from Gemini)
    #[serde(default = "default_grace_period_seconds")]
    pub grace_period_seconds: i32,
}

fn default_max_time_seconds() -> i32 { 300 }
fn default_max_turns() -> i32 { 50 }
fn default_grace_period_seconds() -> i32 { 60 }

impl Default for AgentRunConfig {
    fn default() -> Self {
        Self {
            max_time_seconds: default_max_time_seconds(),
            max_turns: default_max_turns(),
            grace_period_seconds: default_grace_period_seconds(),
        }
    }
}
```

#### 3.1.3 Input/Output Configuration (NEW: from Gemini)

```rust
/// Typed input parameters configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InputConfig {
    pub inputs: HashMap<String, InputDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDefinition {
    /// Description of the input parameter
    pub description: String,

    /// Type of the input
    #[serde(rename = "type")]
    pub input_type: InputType,

    /// Whether this input is required
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    String,
    Number,
    Boolean,
    Integer,
    #[serde(rename = "string[]")]
    StringArray,
    #[serde(rename = "number[]")]
    NumberArray,
}

/// Structured output configuration with schema validation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputConfig {
    /// Name of the output parameter (used in complete_task tool)
    pub output_name: String,

    /// Description of the expected output
    pub description: String,

    /// JSON Schema for output validation
    pub schema: serde_json::Value,
}

impl InputConfig {
    /// Convert to JSON Schema for tool parameter definition
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for (name, def) in &self.inputs {
            let schema = match def.input_type {
                InputType::String => json!({"type": "string", "description": def.description}),
                InputType::Number => json!({"type": "number", "description": def.description}),
                InputType::Boolean => json!({"type": "boolean", "description": def.description}),
                InputType::Integer => json!({"type": "integer", "description": def.description}),
                InputType::StringArray => json!({
                    "type": "array",
                    "items": {"type": "string"},
                    "description": def.description
                }),
                InputType::NumberArray => json!({
                    "type": "array",
                    "items": {"type": "number"},
                    "description": def.description
                }),
            };
            properties.insert(name.clone(), schema);
            if def.required {
                required.push(name.clone());
            }
        }

        json!({
            "type": "object",
            "properties": properties,
            "required": required
        })
    }
}
```

#### 3.1.4 YAML/MD Format Examples (Enhanced with Gemini Patterns)

**YAML Format** (`.claude/agents/my-agent.yaml`):

```yaml
agentType: code-reviewer
displayName: "Code Reviewer Agent"  # NEW: Human-readable name
whenToUse: "Use for reviewing code changes and suggesting improvements"
tools:
  - read_file
  - glob_files
  - grep_files
disallowedTools:
  - shell
  - write_file
source: user
forkContext: false

# NEW: Detailed model configuration (from Gemini)
modelConfig:
  model: inherit
  temperature: 0.1       # Low temperature for accuracy
  topP: 0.95
  thinkingBudget: -1     # Unlimited thinking

# NEW: Enhanced run config with grace period
runConfig:
  maxTimeSeconds: 180
  maxTurns: 20
  gracePeriodSeconds: 60  # NEW: Grace period for recovery

# NEW: Typed input parameters (from Gemini)
inputConfig:
  inputs:
    file_path:
      type: string
      required: true
      description: "Path to the file to review"
    review_focus:
      type: string
      required: false
      description: "Specific aspects to focus on"

# NEW: Structured output with schema (from Gemini)
outputConfig:
  outputName: "review_report"
  description: "The code review report"
  schema:
    type: object
    properties:
      summary:
        type: string
        description: "Summary of the review"
      issues:
        type: array
        items:
          type: object
          properties:
            severity: { type: string }
            description: { type: string }
            line: { type: integer }
      suggestions:
        type: array
        items: { type: string }
    required: [summary, issues]

# NEW: Separated prompt configuration (from Gemini)
promptConfig:
  systemPrompt: |
    You are a code review specialist.
    Current working directory: ${cwd}
  query: |
    Review the file: ${file_path}
    Focus: ${review_focus}

    Provide a thorough review and call complete_task with your report.
```

**Markdown Format** (`.claude/agents/my-agent.md`):

```markdown
---
agentType: code-reviewer
displayName: "Code Reviewer"
whenToUse: "Use for reviewing code changes"
tools: ["read_file", "glob_files", "grep_files"]
modelConfig:
  model: inherit
  temperature: 0.1
runConfig:
  maxTimeSeconds: 180
  gracePeriodSeconds: 60
inputConfig:
  inputs:
    file_path: { type: string, required: true, description: "File to review" }
---

You are a code review specialist.

Current working directory: ${cwd}

Task: Review ${file_path}

Review the code thoroughly and call complete_task with your findings.
```

### 3.2 Tool Restriction System

#### 3.2.1 Three-Tier Model

```rust
/// Tools blocked for ALL subagents (prevents recursion, UI conflicts)
pub const ALWAYS_BLOCKED_TOOLS: &[&str] = &[
    "Task",           // Cannot spawn nested subagents
    "TaskOutput",     // Cannot retrieve other subagent results
    "TodoWrite",      // Main agent UI only
];

/// Tools blocked for non-builtin agents (require elevated trust)
pub const NON_BUILTIN_BLOCKED_TOOLS: &[&str] = &[
    "shell",          // Command execution
    "exec_command",   // Command execution
    "write_file",     // File modification
    "apply_patch",    // Code modification
];

/// Tools allowed for async/background agents (NEW: from Claude Code Bf2)
/// Background agents may only use these safe, non-interactive tools
/// NOTE: TodoWrite is NOT included - it's in ALWAYS_BLOCKED_TOOLS
pub const ASYNC_SAFE_TOOLS: &[&str] = &[
    "Read",           // File reading
    "Edit",           // File editing (with approval)
    "Grep",           // Content search
    "WebSearch",      // Web search
    "Glob",           // File pattern matching
    "Bash",           // Shell commands (with approval)
    "Skill",          // Skill invocation
    "SlashCommand",   // Slash command execution
    "WebFetch",       // Web content fetching
];

/// Tool filter for subagent execution
pub struct ToolFilter {
    definition: AgentDefinition,
    parent_tools: HashSet<String>,
    /// Whether this agent is running in background/async mode
    is_async: bool,
}

impl ToolFilter {
    pub fn new(definition: AgentDefinition, parent_tools: HashSet<String>) -> Self {
        Self { definition, parent_tools, is_async: false }
    }

    /// Create a filter for async/background execution
    pub fn new_async(definition: AgentDefinition, parent_tools: HashSet<String>) -> Self {
        Self { definition, parent_tools, is_async: true }
    }

    /// Check if a tool is allowed for this subagent
    pub fn is_allowed(&self, tool_name: &str) -> bool {
        // Tier 0: Async agents can ONLY use ASYNC_SAFE_TOOLS (NEW: from Claude Code)
        if self.is_async && !ASYNC_SAFE_TOOLS.contains(&tool_name) {
            return false;
        }

        // Tier 1: Always blocked (all subagents)
        if ALWAYS_BLOCKED_TOOLS.contains(&tool_name) {
            return false;
        }

        // Tier 2: Check explicit disallow list
        if self.definition.disallowed_tools.contains(&tool_name.to_string()) {
            return false;
        }

        // Tier 3: Non-builtin blocked (user/project agents)
        if self.definition.source != AgentSource::Builtin
            && NON_BUILTIN_BLOCKED_TOOLS.contains(&tool_name)
        {
            return false;
        }

        // Tier 4: Check tool access configuration
        match &self.definition.tools {
            ToolAccess::All => self.parent_tools.contains(tool_name),
            ToolAccess::List(allowed) => {
                allowed.contains(&tool_name.to_string())
                    && self.parent_tools.contains(tool_name)
            }
        }
    }

    /// Filter tool specs for subagent
    pub fn filter_tools(&self, all_specs: &[ToolSpec]) -> Vec<ToolSpec> {
        all_specs
            .iter()
            .filter(|spec| self.is_allowed(spec.name()))
            .cloned()
            .collect()
    }
}
```

#### 3.2.2 Tool Access Matrix

| Agent Type | Source | tools | disallowedTools | Effective Access |
|------------|--------|-------|-----------------|------------------|
| Explore | Builtin | List(read-only) | [] | read_file, glob_files, grep_files, list_dir |
| Plan | Builtin | List(read-only) | [] | read_file, glob_files, grep_files |
| User Agent | User | All | [shell] | All except shell + NON_BUILTIN_BLOCKED |
| User Agent | User | List([A,B]) | [] | A, B only (no write/exec) |

### 3.3 Agent Registry

```rust
/// Registry for discovering and managing agent definitions
pub struct AgentRegistry {
    /// User/project agent definitions
    agents: RwLock<HashMap<String, AgentDefinition>>,
    /// Built-in agent definitions (immutable)
    builtin_agents: Vec<AgentDefinition>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            agents: RwLock::new(HashMap::new()),
            builtin_agents: Vec::new(),
        };
        registry.load_builtin_agents();
        registry
    }

    /// Load built-in agents
    fn load_builtin_agents(&mut self) {
        self.builtin_agents = vec![
            builtin::create_explore_agent(),
            builtin::create_plan_agent(),
        ];
    }

    /// Load agents from directory
    pub async fn load_from_directory(&self, path: &Path) -> Result<(), SubagentErr> {
        let loader = AgentLoader::new();
        let definitions = loader.load_directory(path).await?;

        let mut agents = self.agents.write().await;
        for def in definitions {
            agents.insert(def.agent_type.clone(), def);
        }
        Ok(())
    }

    /// Get agent by type (user/project agents shadow builtins)
    pub async fn get(&self, agent_type: &str) -> Option<AgentDefinition> {
        // Check user/project agents first
        let agents = self.agents.read().await;
        if let Some(def) = agents.get(agent_type) {
            return Some(def.clone());
        }
        drop(agents);

        // Fall back to builtins
        self.builtin_agents
            .iter()
            .find(|a| a.agent_type == agent_type)
            .cloned()
    }

    /// List all available agent types
    pub async fn list_types(&self) -> Vec<String> {
        let mut types: Vec<String> = self.builtin_agents
            .iter()
            .map(|a| a.agent_type.clone())
            .collect();

        let agents = self.agents.read().await;
        types.extend(agents.keys().cloned());
        types.sort();
        types.dedup();
        types
    }
}
```

### 3.4 Agent Executor

#### 3.4.1 SubagentContext

```rust
/// Isolated execution context for subagent
pub struct SubagentContext {
    /// The agent definition
    pub definition: AgentDefinition,

    /// Reference to parent session for shared services
    pub parent_session: Arc<Session>,

    /// Working directory for this subagent
    pub cwd: PathBuf,

    /// Tool filter based on definition
    pub tool_filter: ToolFilter,

    /// Unique identifier for this subagent instance
    pub agent_id: String,

    /// Whether permission prompts should be suppressed
    pub suppress_permissions: bool,

    /// Parent's cancellation token (for abort propagation)
    pub parent_cancellation: CancellationToken,

    /// Resolved model for this subagent (from priority chain)
    /// Set via SubagentContext::with_model() after resolve_agent_model()
    pub model: String,
}

impl SubagentContext {
    pub fn new(
        definition: AgentDefinition,
        parent_session: Arc<Session>,
        cwd: PathBuf,
        parent_tools: HashSet<String>,
        parent_cancellation: CancellationToken,
    ) -> Self {
        let tool_filter = ToolFilter::new(definition.clone(), parent_tools);
        let agent_id = format!("agent-{}", uuid::Uuid::new_v4());
        // Default to parent's model; caller should use with_model() for resolved model
        let model = parent_session.config.model.clone();

        Self {
            definition,
            parent_session,
            cwd,
            tool_filter,
            agent_id,
            suppress_permissions: true,  // Always suppress in subagent
            parent_cancellation,
            model,
        }
    }

    /// Set the resolved model (call after resolve_agent_model())
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }
}
```

#### 3.4.2 AgentExecutor

```rust
/// Result of subagent execution (Enhanced with execution metrics from Claude Code)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentResult {
    pub status: SubagentStatus,
    pub result: String,
    pub turns_used: i32,
    pub duration: Duration,
    pub agent_id: String,

    // NEW: Execution metrics (from Claude Code v2.0.59)
    /// Total number of tool invocations during execution
    pub total_tool_use_count: i32,
    /// Total execution time in milliseconds
    pub total_duration_ms: i64,
    /// Total tokens consumed (input + output)
    pub total_tokens: i32,
    /// Detailed token usage breakdown
    pub usage: Option<TokenUsage>,
}

/// Detailed token usage breakdown (NEW: from Claude Code)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub input_tokens: i32,
    pub output_tokens: i32,
    /// Tokens used for cache creation (if caching enabled)
    pub cache_creation_input_tokens: Option<i32>,
    /// Tokens read from cache (if caching enabled)
    pub cache_read_input_tokens: Option<i32>,
    /// Server-side tool usage metrics
    pub server_tool_use: Option<ServerToolUsage>,
    /// Service tier used for this execution
    pub service_tier: Option<String>,
}

/// Server-side tool usage metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerToolUsage {
    pub web_search_requests: i32,
    pub web_fetch_requests: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStatus {
    Goal,                       // Successfully called complete_task (from Gemini)
    Timeout,                    // Exceeded max_time_seconds
    MaxTurns,                   // Exceeded max_turns
    Aborted,                    // Cancelled by user/parent
    Error,                      // Execution error
    ErrorNoCompleteTaskCall,    // Ended without calling complete_task (NEW: from Gemini)
}

/// Executor for running subagent tasks
pub struct AgentExecutor {
    context: SubagentContext,
    cancellation_token: CancellationToken,
}

impl AgentExecutor {
    pub fn new(context: SubagentContext) -> Self {
        // Create child cancellation token that's cancelled when parent is
        let cancellation_token = context.parent_cancellation.child_token();
        Self { context, cancellation_token }
    }

    /// Execute the subagent with the given prompt
    pub async fn run(&self, prompt: String) -> Result<SubagentResult, SubagentErr> {
        let start = Instant::now();
        let mut turns = 0;

        // NEW: Execution metrics accumulators
        let mut total_tool_use_count = 0;
        let mut total_input_tokens = 0;
        let mut total_output_tokens = 0;

        let timeout = Duration::from_secs(
            self.context.definition.run_config.max_time_seconds as u64
        );
        let max_turns = self.context.definition.run_config.max_turns;

        // Build system prompt from template
        let system_prompt = self.build_system_prompt(&prompt)?;

        // Get filtered tools
        let tools = self.context.tool_filter.filter_tools(
            &self.get_parent_tools().await?
        );

        // Initialize conversation
        let mut messages: Vec<Message> = vec![
            Message::system(system_prompt),
            Message::user(prompt.clone()),
        ];

        // Helper to build SubagentResult with accumulated metrics
        let build_result = |status, result: String, turns, start: Instant,
                           tool_count, input_tokens, output_tokens| {
            SubagentResult {
                status,
                result,
                turns_used: turns,
                duration: start.elapsed(),
                agent_id: self.context.agent_id.clone(),
                // Execution metrics
                total_tool_use_count: tool_count,
                total_duration_ms: start.elapsed().as_millis() as i64,
                total_tokens: input_tokens + output_tokens,
                usage: Some(TokenUsage {
                    input_tokens,
                    output_tokens,
                    ..Default::default()
                }),
            }
        };

        // Main execution loop
        loop {
            // Check termination conditions
            if self.cancellation_token.is_cancelled() {
                return Ok(build_result(
                    SubagentStatus::Aborted,
                    "Subagent execution was cancelled".to_string(),
                    turns, start, total_tool_use_count, total_input_tokens, total_output_tokens
                ));
            }

            if start.elapsed() >= timeout {
                return Ok(build_result(
                    SubagentStatus::Timeout,
                    format!("Subagent timed out after {} seconds", timeout.as_secs()),
                    turns, start, total_tool_use_count, total_input_tokens, total_output_tokens
                ));
            }

            if turns >= max_turns {
                return Ok(build_result(
                    SubagentStatus::MaxTurns,
                    format!("Subagent reached max turns limit ({})", max_turns),
                    turns, start, total_tool_use_count, total_input_tokens, total_output_tokens
                ));
            }

            turns += 1;

            // Execute turn with timeout
            let turn_result = tokio::select! {
                result = self.execute_turn(&mut messages, &tools) => result,
                _ = tokio::time::sleep(timeout.saturating_sub(start.elapsed())) => {
                    return Ok(build_result(
                        SubagentStatus::Timeout,
                        "Turn timed out".to_string(),
                        turns, start, total_tool_use_count, total_input_tokens, total_output_tokens
                    ));
                }
                _ = self.cancellation_token.cancelled() => {
                    return Ok(build_result(
                        SubagentStatus::Aborted,
                        "Subagent cancelled during turn".to_string(),
                        turns, start, total_tool_use_count, total_input_tokens, total_output_tokens
                    ));
                }
            };

            match turn_result {
                Ok((TurnResult::Continue, metrics)) => {
                    // Accumulate metrics from this turn
                    total_tool_use_count += metrics.tool_use_count;
                    total_input_tokens += metrics.input_tokens;
                    total_output_tokens += metrics.output_tokens;
                    continue;
                }
                Ok((TurnResult::Completed(result), metrics)) => {
                    // Final accumulation
                    total_tool_use_count += metrics.tool_use_count;
                    total_input_tokens += metrics.input_tokens;
                    total_output_tokens += metrics.output_tokens;

                    return Ok(build_result(
                        SubagentStatus::Goal,
                        result,
                        turns, start, total_tool_use_count, total_input_tokens, total_output_tokens
                    ));
                }
                Err(e) => {
                    return Ok(build_result(
                        SubagentStatus::Error,
                        format!("Subagent error: {e}"),
                        turns, start, total_tool_use_count, total_input_tokens, total_output_tokens
                    ));
                }
            }
        }
    }

    fn build_system_prompt(&self, user_prompt: &str) -> Result<String, SubagentErr> {
        let template = self.context.definition.system_prompt
            .as_deref()
            .unwrap_or("You are a helpful assistant.");

        // Apply template substitution
        let prompt = template
            .replace("${prompt}", user_prompt)
            .replace("${cwd}", &self.context.cwd.display().to_string())
            .replace("${agent_type}", &self.context.definition.agent_type);

        Ok(prompt)
    }
}

enum TurnResult {
    Continue,
    Completed(String),
}

/// Turn execution result with metrics for accumulation
struct TurnMetrics {
    tool_use_count: i32,
    input_tokens: i32,
    output_tokens: i32,
}

impl AgentExecutor {
    /// Execute a single turn of the conversation
    /// Returns TurnResult and metrics for accumulation
    async fn execute_turn(
        &self,
        messages: &mut Vec<Message>,
        tools: &[ToolSpec],
    ) -> Result<(TurnResult, TurnMetrics), SubagentErr> {
        let mut metrics = TurnMetrics {
            tool_use_count: 0,
            input_tokens: 0,
            output_tokens: 0,
        };

        // 1. Call LLM with current messages
        let model_client = self.context.parent_session.model_client();
        let response = model_client
            .stream_response(&self.context.model, messages, tools)
            .await
            .map_err(|e| SubagentErr::ModelError(e.to_string()))?;

        // 2. Accumulate token usage from response
        if let Some(usage) = response.usage() {
            metrics.input_tokens = usage.input_tokens;
            metrics.output_tokens = usage.output_tokens;
        }

        // 3. Process response into message
        let assistant_msg = response.to_message();
        messages.push(assistant_msg.clone());

        // 4. Check for complete_task call
        if let Some(tool_calls) = &assistant_msg.tool_calls {
            for call in tool_calls {
                if call.name == "complete_task" {
                    let output = self.extract_complete_task_output(&call.arguments)?;
                    return Ok((TurnResult::Completed(output), metrics));
                }
            }

            // 5. Execute other tools
            metrics.tool_use_count = tool_calls.len() as i32;
            let tool_results = self.execute_tools(tool_calls).await?;
            messages.push(Message::tool_results(tool_results));
        }

        Ok((TurnResult::Continue, metrics))
    }

    /// Execute tool calls and return results
    async fn execute_tools(
        &self,
        tool_calls: &[ToolCall],
    ) -> Result<Vec<ToolResult>, SubagentErr> {
        let mut results = Vec::new();

        for call in tool_calls {
            // Check tool is allowed
            if !self.context.tool_filter.is_allowed(&call.name) {
                results.push(ToolResult {
                    tool_use_id: call.id.clone(),
                    content: format!("Tool '{}' is not available in this context", call.name),
                    is_error: true,
                });
                continue;
            }

            // Execute the tool via parent session's tool handler
            let result = self.context.parent_session
                .execute_tool(&call.name, &call.arguments)
                .await
                .map_err(|e| SubagentErr::Internal(e.to_string()))?;

            results.push(ToolResult {
                tool_use_id: call.id.clone(),
                content: result.content,
                is_error: !result.success.unwrap_or(true),
            });
        }

        Ok(results)
    }

    /// Extract output from complete_task call arguments
    fn extract_complete_task_output(
        &self,
        arguments: &serde_json::Value,
    ) -> Result<String, SubagentErr> {
        // If there's an output config, extract the named output
        if let Some(output_config) = &self.context.definition.output_config {
            if let Some(output) = arguments.get(&output_config.output_name) {
                return Ok(serde_json::to_string_pretty(output)
                    .unwrap_or_else(|_| output.to_string()));
            }
        }

        // Otherwise return the full arguments as the output
        Ok(serde_json::to_string_pretty(arguments)
            .unwrap_or_else(|_| arguments.to_string()))
    }
}
```

### 3.5 complete_task Tool (NEW: from Gemini)

Subagents must explicitly call `complete_task` to signal completion. This provides:
- Clear termination signal (vs implicit final message)
- Structured output validation via OutputConfig schema
- Consistent behavior across all agents

```rust
/// Generate complete_task tool dynamically based on OutputConfig
pub fn create_complete_task_tool(output_config: Option<&OutputConfig>) -> ToolSpec {
    let mut properties = BTreeMap::new();
    let mut required = Vec::new();

    if let Some(config) = output_config {
        // Add the output parameter from OutputConfig schema
        properties.insert(config.output_name.clone(), config.schema.clone());
        required.push(config.output_name.clone());
    }

    ToolSpec::Function(ResponsesApiTool {
        name: "complete_task".to_string(),
        description: if output_config.is_some() {
            "Call this tool to submit your final answer and complete the task. \
             You MUST provide the required output in the specified format."
        } else {
            "Call this tool to signal that you have completed your task."
        }.to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: if required.is_empty() { None } else { Some(required) },
            additional_properties: Some(false.into()),
        },
    })
}
```

### 3.6 Grace Period Mechanism (NEW: from Gemini)

When timeout or max_turns is reached, give agent a final chance to submit results:

```rust
const GRACE_PERIOD_DEFAULT_SECONDS: i32 = 60;

impl AgentExecutor {
    /// Execute grace period recovery when timeout/max_turns is reached
    async fn execute_grace_period(
        &self,
        messages: &mut Vec<Message>,
        reason: SubagentStatus,
    ) -> Option<String> {
        let warning = match reason {
            SubagentStatus::Timeout => "You have exceeded the time limit.",
            SubagentStatus::MaxTurns => "You have reached the maximum number of turns.",
            _ => return None,  // Grace period only for timeout/max_turns
        };

        let grace_message = format!(
            "{warning} You have one final chance to complete the task.\n\
             You MUST call `complete_task` immediately with your best answer.\n\
             Do not call any other tools."
        );

        messages.push(Message::user(grace_message));

        // Create grace period timeout
        let grace_timeout = Duration::from_secs(
            self.context.definition.run_config.grace_period_seconds as u64
        );

        // Execute one final turn with only complete_task available
        let complete_task_tool = create_complete_task_tool(
            self.context.definition.output_config.as_ref()
        );

        let result = tokio::time::timeout(grace_timeout, async {
            self.execute_turn(messages, &[complete_task_tool]).await
        }).await;

        match result {
            Ok(Ok(TurnResult::Completed(output))) => {
                tracing::info!("Grace period recovery successful");
                Some(output)
            }
            Ok(Err(e)) => {
                tracing::warn!("Grace period failed with error: {e}");
                None
            }
            Err(_) => {
                tracing::warn!("Grace period timed out");
                None
            }
        }
    }
}
```

#### Grace Period Execution Flow

```
Normal execution loop
        │
        ▼
Check termination (timeout/max_turns)
        │
        ├─── If Goal or Aborted ──► Return immediately
        │
        ▼
Grace Period (60s default)
        │
        ├─── Agent calls complete_task ──► Return with Goal status
        │
        └─── Timeout/Error ──► Return with original status + partial result
```

### 3.7 Background Task Execution

```rust
use dashmap::DashMap;
use tokio::task::JoinHandle;

/// Storage for background (async) subagent tasks
pub struct BackgroundTaskStore {
    tasks: DashMap<String, BackgroundTask>,
}

pub struct BackgroundTask {
    pub agent_id: String,
    pub description: String,
    pub prompt: String,
    pub status: BackgroundTaskStatus,
    pub result: Option<SubagentResult>,
    pub handle: Option<JoinHandle<SubagentResult>>,
    pub created_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundTaskStatus {
    Running,
    Completed,
    Failed,
}

impl BackgroundTaskStore {
    pub fn new() -> Self {
        Self {
            tasks: DashMap::new(),
        }
    }

    /// Spawn a background subagent task
    pub fn spawn(
        &self,
        executor: AgentExecutor,
        prompt: String,
        description: String,
        transcript_store: Arc<TranscriptStore>,  // For transcript recording
    ) -> String {
        let agent_id = executor.context.agent_id.clone();

        let handle = tokio::spawn(async move {
            // Background tasks use run_with_resume for transcript recording
            // (resume_agent_id is None since background tasks don't support resume)
            executor.run_with_resume(prompt.clone(), None, &transcript_store)
                .await
                .unwrap_or_else(|e| {
                    SubagentResult {
                        status: SubagentStatus::Error,
                        result: format!("Spawn error: {e}"),
                        turns_used: 0,
                        duration: Duration::ZERO,
                        agent_id: "unknown".to_string(),
                        total_tool_use_count: 0,
                        total_duration_ms: 0,
                        total_tokens: 0,
                        usage: None,
                    }
                })
        });

        let task = BackgroundTask {
            agent_id: agent_id.clone(),
            description,
            prompt,
            status: BackgroundTaskStatus::Running,
            result: None,
            handle: Some(handle),
            created_at: Instant::now(),
        };

        self.tasks.insert(agent_id.clone(), task);
        agent_id
    }

    /// Get task result (optionally blocking until complete)
    pub async fn get_result(
        &self,
        agent_id: &str,
        block: bool,
        timeout: Duration,
    ) -> Option<SubagentResult> {
        // Check if already completed
        if let Some(mut task) = self.tasks.get_mut(agent_id) {
            if task.status != BackgroundTaskStatus::Running {
                return task.result.clone();
            }

            if block {
                // Take the handle out to await it
                if let Some(handle) = task.handle.take() {
                    drop(task);  // Release the lock

                    let result = tokio::select! {
                        result = handle => result.ok(),
                        _ = tokio::time::sleep(timeout) => None,
                    };

                    // Update task with result
                    if let Some(result) = result {
                        if let Some(mut task) = self.tasks.get_mut(agent_id) {
                            task.status = if result.status == SubagentStatus::Goal {
                                BackgroundTaskStatus::Completed
                            } else {
                                BackgroundTaskStatus::Failed
                            };
                            task.result = Some(result.clone());
                        }
                        return Some(result);
                    }
                }
            }
        }

        None
    }

    /// Check task status without blocking
    pub fn get_status(&self, agent_id: &str) -> Option<BackgroundTaskStatus> {
        self.tasks.get(agent_id).map(|t| t.status)
    }

    /// List all task IDs
    pub fn list_tasks(&self) -> Vec<String> {
        self.tasks.iter().map(|t| t.agent_id.clone()).collect()
    }

    /// Cleanup completed tasks older than duration
    pub fn cleanup_old_tasks(&self, older_than: Duration) {
        let now = Instant::now();
        self.tasks.retain(|_, task| {
            task.status == BackgroundTaskStatus::Running
                || now.duration_since(task.created_at) < older_than
        });
    }
}
```

### 3.6 Tool Handlers

#### 3.6.1 Task Tool (SubagentToolHandler)

```rust
/// Arguments for Task tool invocation
#[derive(Debug, Deserialize)]
pub struct TaskToolArgs {
    /// Type of subagent to spawn
    pub subagent_type: String,

    /// The prompt/task for the subagent
    pub prompt: String,

    /// Short description (3-5 words) for display (REQUIRED - from Claude Code)
    pub description: String,

    /// Optional model override: "sonnet", "haiku", "opus", or "inherit"
    #[serde(default)]
    pub model: Option<String>,

    /// Run in background (async)
    #[serde(default)]
    pub run_in_background: bool,

    /// Agent ID to resume from previous execution (NEW: from Claude Code)
    /// If provided, the agent continues with full previous context preserved
    #[serde(default)]
    pub resume: Option<String>,
}

/// Handler for the Task tool
pub struct SubagentToolHandler {
    registry: Arc<AgentRegistry>,
    background_store: Arc<BackgroundTaskStore>,
    transcript_store: Arc<TranscriptStore>,  // For resume functionality
}

impl SubagentToolHandler {
    pub fn new(
        registry: Arc<AgentRegistry>,
        background_store: Arc<BackgroundTaskStore>,
        transcript_store: Arc<TranscriptStore>,
    ) -> Self {
        Self { registry, background_store, transcript_store }
    }
}

#[async_trait]
impl ToolHandler for SubagentToolHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true  // Subagents can be mutating
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let args: TaskToolArgs = parse_args(&invocation)?;

        // Get agent definition
        let definition = self.registry.get(&args.subagent_type).await
            .ok_or_else(|| FunctionCallError::RespondToModel(
                format!(
                    "Unknown subagent type '{}'. Available: {:?}",
                    args.subagent_type,
                    self.registry.list_types().await
                )
            ))?;

        // Build context
        let parent_tools = get_parent_tool_names(&invocation.session);
        let context = SubagentContext::new(
            definition,
            invocation.session.clone(),
            invocation.turn.cwd.clone(),
            parent_tools,
            invocation.turn.cancellation_token.clone(),
        );

        let executor = AgentExecutor::new(context);

        if args.run_in_background {
            // Spawn background task (background tasks don't support resume)
            let agent_id = self.background_store.spawn(
                executor,
                args.prompt.clone(),
                args.description.clone(),
                self.transcript_store.clone(),  // Pass transcript_store for recording
            );

            Ok(ToolOutput::Function {
                content: serde_json::json!({
                    "status": "async_launched",
                    "agent_id": agent_id,
                    "description": args.description,
                }).to_string(),
                content_items: None,
                success: Some(true),
            })
        } else {
            // Synchronous execution with resume support
            let result = executor.run_with_resume(
                args.prompt,
                args.resume.as_deref(),          // Resume from previous agent if provided
                &self.transcript_store,          // Transcript store for recording/resume
            ).await.map_err(|e| FunctionCallError::RespondToModel(
                format!("Subagent execution failed: {e}")
            ))?;

            Ok(ToolOutput::Function {
                content: serde_json::json!({
                    "status": result.status,
                    "result": result.result,
                    "turns_used": result.turns_used,
                    "duration_seconds": result.duration.as_secs_f32(),
                    "agent_id": result.agent_id,
                    // Include execution metrics in output
                    "total_tool_use_count": result.total_tool_use_count,
                    "total_tokens": result.total_tokens,
                }).to_string(),
                content_items: None,
                success: Some(result.status == SubagentStatus::Goal),
            })
        }
    }
}
```

#### 3.6.2 TaskOutput Tool

```rust
/// Arguments for TaskOutput tool invocation
#[derive(Debug, Deserialize)]
pub struct TaskOutputArgs {
    /// Agent ID to retrieve results for
    pub agent_id: String,

    /// Whether to block waiting for completion (default: true)
    #[serde(default = "default_block")]
    pub block: bool,

    /// Timeout in seconds (default: 300)
    #[serde(default = "default_timeout")]
    pub timeout: i32,
}

fn default_block() -> bool { true }
fn default_timeout() -> i32 { 300 }

/// Handler for TaskOutput tool
pub struct TaskOutputHandler {
    background_store: Arc<BackgroundTaskStore>,
}

#[async_trait]
impl ToolHandler for TaskOutputHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        false  // Read-only
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let args: TaskOutputArgs = parse_args(&invocation)?;

        let timeout = Duration::from_secs(args.timeout as u64);

        match self.background_store.get_result(&args.agent_id, args.block, timeout).await {
            Some(result) => {
                Ok(ToolOutput::Function {
                    content: serde_json::json!({
                        "status": result.status,
                        "result": result.result,
                        "turns_used": result.turns_used,
                        "duration_seconds": result.duration.as_secs_f32(),
                    }).to_string(),
                    content_items: None,
                    success: Some(result.status == SubagentStatus::Goal),
                })
            }
            None => {
                let status = self.background_store.get_status(&args.agent_id);
                Ok(ToolOutput::Function {
                    content: serde_json::json!({
                        "status": status.map(|s| format!("{:?}", s)).unwrap_or("not_found".to_string()),
                        "message": if status.is_some() {
                            "Task still running or timed out waiting"
                        } else {
                            "No task found with that agent_id"
                        },
                    }).to_string(),
                    content_items: None,
                    success: Some(false),
                })
            }
        }
    }
}
```

#### 3.6.3 Resume and Transcript Recording (NEW: from Claude Code v2.0.59)

Claude Code supports **resuming agents** via the `resume` parameter. This enables agents to continue work from a previous execution, preserving full context.

##### Transcript Storage

Each agent execution records its transcript for potential resume:

```rust
/// Storage for agent transcripts enabling resume functionality
pub struct TranscriptStore {
    transcripts: DashMap<String, AgentTranscript>,
}

/// Recorded transcript for an agent execution
#[derive(Debug, Clone)]
pub struct AgentTranscript {
    pub agent_id: String,
    pub agent_type: String,
    pub messages: Vec<TranscriptMessage>,
    pub created_at: Instant,
    pub is_sidechain: bool,  // True for subagent transcripts
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptMessage {
    pub role: MessageRole,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_results: Option<Vec<ToolResult>>,
    pub timestamp: i64,
}

impl TranscriptStore {
    pub fn new() -> Self {
        Self { transcripts: DashMap::new() }
    }

    /// Record a message to the agent's transcript
    pub fn record_message(&self, agent_id: &str, message: TranscriptMessage) {
        if let Some(mut transcript) = self.transcripts.get_mut(agent_id) {
            transcript.messages.push(message);
        }
    }

    /// Initialize transcript for a new agent execution
    pub fn init_transcript(&self, agent_id: String, agent_type: String) {
        let transcript = AgentTranscript {
            agent_id: agent_id.clone(),
            agent_type,
            messages: Vec::new(),
            created_at: Instant::now(),
            is_sidechain: true,
        };
        self.transcripts.insert(agent_id, transcript);
    }

    /// Load transcript for resume, reconstructing message chain from leaf to root
    pub fn load_transcript(&self, agent_id: &str) -> Option<Vec<TranscriptMessage>> {
        self.transcripts.get(agent_id).map(|t| t.messages.clone())
    }
}
```

##### Resume Implementation

When `resume` parameter is provided:

```rust
impl AgentExecutor {
    /// Execute agent with optional resume from previous transcript
    pub async fn run_with_resume(
        &self,
        prompt: String,
        resume_agent_id: Option<&str>,
        transcript_store: &TranscriptStore,
    ) -> Result<SubagentResult, SubagentErr> {
        let mut messages = if let Some(prev_id) = resume_agent_id {
            // Load previous transcript
            let prev_messages = transcript_store.load_transcript(prev_id)
                .ok_or_else(|| SubagentErr::TranscriptNotFound(prev_id.to_string()))?;

            tracing::info!(
                "Resuming agent from {} with {} previous messages",
                prev_id,
                prev_messages.len()
            );

            // Convert transcript messages to conversation messages
            self.convert_transcript_to_messages(prev_messages)
        } else {
            // Fresh start
            vec![
                Message::system(self.build_system_prompt(&prompt)?),
                Message::user(prompt.clone()),
            ]
        };

        // Initialize transcript recording for this execution
        transcript_store.init_transcript(
            self.context.agent_id.clone(),
            self.context.definition.agent_type.clone(),
        );

        // Continue with normal execution loop...
        self.run_loop(&mut messages, transcript_store).await
    }

    fn convert_transcript_to_messages(
        &self,
        transcript: Vec<TranscriptMessage>,
    ) -> Vec<Message> {
        transcript.into_iter().map(|tm| {
            match tm.role {
                MessageRole::System => Message::system(tm.content),
                MessageRole::User => Message::user(tm.content),
                MessageRole::Assistant => {
                    let mut msg = Message::assistant(tm.content);
                    if let Some(calls) = tm.tool_calls {
                        msg.tool_calls = Some(calls);
                    }
                    msg
                }
            }
        }).collect()
    }
}
```

##### Resume Flow

```
Task(resume: "agent-abc123")
        │
        ▼
SubagentToolHandler checks resume param
        │
        ├── resume is Some ──► Load transcript from TranscriptStore
        │                           │
        │                           ▼
        │                     AgentExecutor.run_with_resume()
        │                           │
        │                           ▼
        │                     Restore previous messages
        │                           │
        │                           ▼
        │                     Continue execution loop
        │
        └── resume is None ──► Fresh execution (normal path)
```

##### Transcript Cleanup

Transcripts should be cleaned up periodically to prevent memory growth:

```rust
impl TranscriptStore {
    /// Remove transcripts older than specified duration
    pub fn cleanup_old_transcripts(&self, older_than: Duration) {
        let now = Instant::now();
        self.transcripts.retain(|_, transcript| {
            now.duration_since(transcript.created_at) < older_than
        });
    }
}
```

### 3.7 Tool Specifications

```rust
use std::sync::LazyLock;
use std::collections::BTreeMap;

pub static TASK_TOOL: LazyLock<ToolSpec> = LazyLock::new(|| {
    let mut properties = BTreeMap::new();

    properties.insert("subagent_type".to_string(), JsonSchema::String {
        description: Some("The type of subagent to spawn (e.g., 'Explore', 'Plan')".to_string()),
    });

    properties.insert("prompt".to_string(), JsonSchema::String {
        description: Some("The task/prompt for the subagent to execute".to_string()),
    });

    properties.insert("description".to_string(), JsonSchema::String {
        description: Some("A short (3-5 word) description of the task (REQUIRED)".to_string()),
    });

    properties.insert("model".to_string(), JsonSchema::String {
        description: Some("Optional model override: 'sonnet', 'haiku', 'opus', 'inherit'".to_string()),
    });

    properties.insert("run_in_background".to_string(), JsonSchema::Boolean {
        description: Some("If true, run async and return immediately".to_string()),
    });

    // NEW: Resume parameter (from Claude Code)
    properties.insert("resume".to_string(), JsonSchema::String {
        description: Some("Optional agent ID to resume from. If provided, the agent continues from the previous execution transcript.".to_string()),
    });

    ToolSpec::Function(ResponsesApiTool {
        name: "Task".to_string(),
        description: r#"Spawns a specialized subagent for focused, autonomous tasks.

Available subagent types:
- Explore: Fast codebase exploration (read-only)
- Plan: Implementation planning and architecture design

Use this when a task requires:
- Focused exploration without context pollution
- Independent research or analysis
- Parallel work on separate concerns

The subagent runs in isolated context with restricted tools.

Agents can be resumed using the `resume` parameter by passing the agent ID from a previous invocation. When resumed, the agent continues with its full previous context preserved."#.to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            // description is now REQUIRED (from Claude Code)
            required: Some(vec![
                "subagent_type".to_string(),
                "prompt".to_string(),
                "description".to_string(),
            ]),
            additional_properties: Some(false.into()),
        },
    })
});

pub static TASK_OUTPUT_TOOL: LazyLock<ToolSpec> = LazyLock::new(|| {
    let mut properties = BTreeMap::new();

    properties.insert("agent_id".to_string(), JsonSchema::String {
        description: Some("The agent ID from async_launched status".to_string()),
    });

    properties.insert("block".to_string(), JsonSchema::Boolean {
        description: Some("Wait for completion (default: true)".to_string()),
    });

    properties.insert("timeout".to_string(), JsonSchema::Integer {
        description: Some("Seconds to wait (default: 300)".to_string()),
    });

    ToolSpec::Function(ResponsesApiTool {
        name: "TaskOutput".to_string(),
        description: "Retrieves results from a background subagent task.".to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["agent_id".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
});
```

### 3.8 Event Bridge (NEW: from Current Implementation)

The current codex implementation (`codex_delegate.rs`) provides a robust event forwarding pattern that should be adopted for v2. This enables proper event routing between subagent and parent session.

```rust
/// Bridge for forwarding subagent events to parent session
/// Pattern from: core/src/codex_delegate.rs
pub struct SubagentEventBridge {
    parent_session: Arc<Session>,
    parent_ctx: Arc<TurnContext>,
    agent_id: String,
    agent_type: String,
}

impl SubagentEventBridge {
    pub fn new(
        parent_session: Arc<Session>,
        parent_ctx: Arc<TurnContext>,
        agent_id: String,
        agent_type: String,
    ) -> Self {
        Self { parent_session, parent_ctx, agent_id, agent_type }
    }

    /// Forward event to parent, handling approval requests specially
    pub async fn forward(&self, event: EventMsg) {
        match event {
            // Approval requests must be routed to parent for user interaction
            EventMsg::ExecApprovalRequest(e) => {
                self.handle_exec_approval(e).await;
            }
            EventMsg::ApplyPatchApprovalRequest(e) => {
                self.handle_patch_approval(e).await;
            }
            // Tool call events → wrap as SubagentActivity
            EventMsg::AgentToolCallEvent(e) => {
                let activity = SubagentActivityEvent {
                    agent_id: self.agent_id.clone(),
                    agent_type: self.agent_type.clone(),
                    event_type: SubagentEventType::ToolCallStart,
                    data: [
                        ("tool_name".to_string(), json!(e.tool_name)),
                        ("arguments".to_string(), e.arguments.clone()),
                    ].into(),
                };
                self.parent_session.send_event(
                    &self.parent_ctx,
                    EventMsg::SubagentActivity(activity)
                ).await;
            }
            // Forward other events as-is or wrap appropriately
            other => {
                self.parent_session.send_event(&self.parent_ctx, other).await;
            }
        }
    }

    /// Handle shell command approval request
    async fn handle_exec_approval(&self, request: ExecApprovalRequest) {
        // Route to parent session's approval handler
        // The parent's TUI/CLI will prompt the user
        self.parent_session.send_event(
            &self.parent_ctx,
            EventMsg::ExecApprovalRequest(request)
        ).await;
    }

    /// Handle file modification approval request
    async fn handle_patch_approval(&self, request: ApplyPatchApprovalRequest) {
        self.parent_session.send_event(
            &self.parent_ctx,
            EventMsg::ApplyPatchApprovalRequest(request)
        ).await;
    }
}
```

### 3.9 Approval Routing Pattern (NEW: from Current Implementation)

For subagents that need to execute shell commands or modify files, approval requests must be routed back to the parent session for user confirmation.

```rust
/// Approval routing modes
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    /// Route approval requests to parent session (default for writeable agents)
    #[default]
    RouteToParent,
    /// Suppress all approval prompts (for read-only agents like Explore)
    Suppress,
    /// Auto-approve within sandbox (requires sandbox enabled)
    AutoApproveInSandbox,
}

/// Example: Handling approval response from parent
impl AgentExecutor {
    async fn wait_for_approval(
        &self,
        request_id: String,
        event_bridge: &SubagentEventBridge,
    ) -> Result<bool, SubagentErr> {
        // Send approval request to parent
        let request = ExecApprovalRequest {
            id: request_id.clone(),
            command: "...",
            // ...
        };
        event_bridge.handle_exec_approval(request).await;

        // Wait for response via channel
        let response = self.approval_rx.recv().await
            .map_err(|_| SubagentErr::ApprovalTimeout)?;

        Ok(response.approved)
    }
}
```

#### Approval Flow Diagram

```
Subagent wants to execute shell command
        │
        ▼
AgentExecutor checks ApprovalMode
        │
        ├── Suppress ──► Execute without approval (read-only agent)
        │
        ├── AutoApproveInSandbox ──► Check sandbox, auto-approve
        │
        └── RouteToParent ──► Send ExecApprovalRequest
                │
                ▼
        SubagentEventBridge.forward()
                │
                ▼
        Parent Session receives request
                │
                ▼
        TUI/CLI prompts user
                │
                ▼
        Response sent back to subagent
                │
                ▼
        Subagent continues or aborts
```

### 3.10 Model Resolution Priority Chain (NEW: from Claude Code v2.0.59)

Claude Code uses a 4-level priority chain for resolving which model a subagent should use:

```
┌────────────────────────────────────────────────────────────┐
│           Model Resolution Priority Chain                   │
│                                                            │
│  1. CODEX_SUBAGENT_MODEL env var     [HIGHEST PRIORITY]    │
│  2. Task tool "model" parameter                            │
│  3. Agent definition "model" property                      │
│  4. Parent model (for "inherit") or default [LOWEST]       │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

#### Implementation

```rust
/// Environment variable for global subagent model override
pub const SUBAGENT_MODEL_ENV_VAR: &str = "CODEX_SUBAGENT_MODEL";

/// Resolve which model to use for a subagent
pub fn resolve_agent_model(
    agent_model: Option<&AgentModel>,
    main_loop_model: &str,
    override_model: Option<&str>,
    permission_mode: &str,
) -> String {
    // Priority 1: Environment variable override (highest)
    if let Ok(env_model) = std::env::var(SUBAGENT_MODEL_ENV_VAR) {
        if !env_model.is_empty() {
            tracing::debug!("Using model from env var: {}", env_model);
            return resolve_model_name(&env_model);
        }
    }

    // Priority 2: Task tool "model" parameter
    if let Some(model) = override_model {
        if !model.is_empty() {
            tracing::debug!("Using model from task parameter: {}", model);
            return resolve_model_name(model);
        }
    }

    // Priority 3: Agent definition model property
    if let Some(agent_model) = agent_model {
        match agent_model {
            AgentModel::Inherit => {
                // Fall through to priority 4
            }
            AgentModel::Sonnet => return "claude-sonnet".to_string(),
            AgentModel::Haiku => return "claude-haiku".to_string(),
            AgentModel::Opus => return "claude-opus".to_string(),
            AgentModel::Custom(name) => return name.clone(),
        }
    }

    // Priority 4: Inherit from parent with special handling
    resolve_inherited_model(main_loop_model, permission_mode)
}

/// Resolve inherited model with special handling for plan mode
fn resolve_inherited_model(main_loop_model: &str, permission_mode: &str) -> String {
    // Special case: Opus/Plan in plan mode should use Opus for planning capability
    if permission_mode == "plan" {
        if main_loop_model.contains("opus") {
            return main_loop_model.to_string();
        }
        // Haiku in plan mode should be upgraded to Sonnet (planning needs capability)
        if main_loop_model.contains("haiku") {
            tracing::debug!("Upgrading haiku to sonnet for plan mode");
            return "claude-sonnet".to_string();
        }
    }

    // Default: use parent's model
    main_loop_model.to_string()
}

/// Map user-friendly model names to actual model identifiers
fn resolve_model_name(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "sonnet" => "claude-sonnet".to_string(),
        "haiku" => "claude-haiku".to_string(),
        "opus" => "claude-opus".to_string(),
        "inherit" => "inherit".to_string(),  // Signal to use parent model
        _ => name.to_string(),  // Custom model name, use as-is
    }
}
```

#### Usage in SubagentToolHandler

```rust
impl SubagentToolHandler {
    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let args: TaskToolArgs = parse_args(&invocation)?;

        // Get agent definition
        let definition = self.registry.get(&args.subagent_type).await?;

        // Resolve model using priority chain (see §3.10)
        let resolved_model = resolve_agent_model(
            definition.model_config.model.as_ref(),
            &invocation.session.config.model,  // Parent's model
            args.model.as_deref(),              // Task tool parameter
            &invocation.session.permission_mode,
        );

        // Build context with resolved model using builder pattern
        let parent_tools = get_parent_tool_names(&invocation.session);
        let context = SubagentContext::new(
            definition,
            invocation.session.clone(),
            invocation.turn.cwd.clone(),
            parent_tools,
            invocation.turn.cancellation_token.clone(),
        ).with_model(resolved_model);  // Set resolved model

        // Execute agent with transcript support
        let executor = AgentExecutor::new(context);
        let result = executor.run_with_resume(
            args.prompt,
            args.resume.as_deref(),
            &self.transcript_store,
        ).await?;
        // ...
    }
}
```

### 3.11 Fork Context Message Format (NEW: from Claude Code v2.0.59)

When `fork_context: true`, the agent receives parent conversation history. Claude Code uses specific boundary markers and filtering to ensure clean context transfer.

#### Entry Message Format

```rust
/// Create the fork context entry message with boundary markers
pub fn create_fork_context_entry() -> String {
    r#"### FORKING CONVERSATION CONTEXT ###
### ENTERING SUB-AGENT ROUTINE ###
Entered sub-agent context

PLEASE NOTE:
- The messages above this point are from the main thread prior to sub-agent execution.
- Context messages may include tool_use blocks for tools not available in sub-agent context.
- Only complete the specific sub-agent task you have been assigned below."#.to_string()
}

impl AgentExecutor {
    /// Build initial messages with optional fork context
    fn build_initial_messages(
        &self,
        prompt: &str,
        parent_messages: Option<&[Message]>,
    ) -> Vec<Message> {
        let mut messages = Vec::new();

        // Add system prompt
        messages.push(Message::system(self.build_system_prompt(prompt).unwrap()));

        // Fork context: add filtered parent messages
        if self.context.definition.fork_context {
            if let Some(parent_msgs) = parent_messages {
                // Filter out messages with unresolved tool uses
                let filtered = self.filter_unresolved_tool_uses(parent_msgs);
                messages.extend(filtered);

                // Add boundary marker
                messages.push(Message::user(create_fork_context_entry()));
            }
        }

        // Add the user's task prompt
        messages.push(Message::user(prompt.to_string()));

        messages
    }
}
```

#### filterUnresolvedToolUses

Claude Code filters messages that contain `tool_use` blocks without corresponding `tool_result` to prevent confusion:

```rust
impl AgentExecutor {
    /// Filter out messages with pending (unresolved) tool uses
    /// This prevents the subagent from seeing incomplete tool interactions
    fn filter_unresolved_tool_uses(&self, messages: &[Message]) -> Vec<Message> {
        let mut result = Vec::new();
        let mut pending_tool_use_ids: HashSet<String> = HashSet::new();

        for msg in messages {
            // Track tool_use blocks
            if let Some(tool_calls) = &msg.tool_calls {
                for call in tool_calls {
                    pending_tool_use_ids.insert(call.id.clone());
                }
            }

            // Remove tool_use IDs that have results
            if let Some(tool_results) = &msg.tool_results {
                for result in tool_results {
                    pending_tool_use_ids.remove(&result.tool_use_id);
                }
            }
        }

        // Second pass: filter out messages with unresolved tool uses
        for msg in messages {
            if let Some(tool_calls) = &msg.tool_calls {
                // Check if any tool_use in this message is unresolved
                let has_unresolved = tool_calls.iter()
                    .any(|call| pending_tool_use_ids.contains(&call.id));

                if has_unresolved {
                    // Skip this message or create a sanitized version
                    let sanitized = self.sanitize_message_for_fork(msg);
                    if let Some(clean_msg) = sanitized {
                        result.push(clean_msg);
                    }
                    continue;
                }
            }
            result.push(msg.clone());
        }

        result
    }

    /// Create a sanitized version of a message for fork context
    /// Removes tool_use blocks but preserves text content
    fn sanitize_message_for_fork(&self, msg: &Message) -> Option<Message> {
        // If message has text content, preserve it without tool_use
        if !msg.content.is_empty() {
            Some(Message {
                role: msg.role.clone(),
                content: msg.content.clone(),
                tool_calls: None,  // Remove tool calls
                tool_results: None,
            })
        } else {
            None  // Skip messages that are only tool calls
        }
    }
}
```

#### Fork Context Flow

```
Parent conversation: [M1, M2, M3(tool_use), M4(tool_result), M5(tool_use, pending)]
                                                                          │
                                                                          ▼
                                                        filterUnresolvedToolUses()
                                                                          │
                                                                          ▼
Filtered messages: [M1, M2, M3(tool_use), M4(tool_result), M5(text only)]
                                                                          │
                                                                          ▼
                                           Add boundary: "### FORKING CONVERSATION CONTEXT ###"
                                                                          │
                                                                          ▼
Final subagent messages: [System, M1, M2, M3, M4, M5(sanitized), Boundary, UserTask]
```

#### Why This Matters

1. **Prevents confusion**: Subagents won't try to respond to tool_use blocks they can't see results for
2. **Clean context**: The boundary marker clearly separates parent context from subagent task
3. **Tool availability note**: The message explicitly warns about tools that may not be available
4. **Focused execution**: Subagent knows to only work on the assigned task

---

## 4. Built-in Agents

### 4.1 Explore Agent

**Purpose**: Fast, read-only codebase exploration

```rust
pub fn create_explore_agent() -> AgentDefinition {
    AgentDefinition {
        agent_type: "Explore".to_string(),
        when_to_use: Some(
            "Fast exploration of codebases. Use for finding files, \
             searching code, understanding structure.".to_string()
        ),
        tools: ToolAccess::List(vec![
            "read_file".to_string(),
            "glob_files".to_string(),
            "grep_files".to_string(),
            "list_dir".to_string(),
        ]),
        disallowed_tools: vec![],
        source: AgentSource::Builtin,
        model: AgentModel::Inherit,
        fork_context: false,
        system_prompt: Some(include_str!("prompts/explore.md").to_string()),
        run_config: AgentRunConfig {
            max_time_seconds: 120,  // 2 minutes
            max_turns: 30,
        },
        ..Default::default()
    }
}
```

**prompts/explore.md**:

```markdown
You are a fast, focused codebase explorer.

Working directory: ${cwd}

Your task: ${prompt}

## Guidelines

1. **Be thorough but efficient** - Search multiple patterns/locations in parallel
2. **Report findings clearly** - Include file paths and relevant code snippets
3. **Stay focused** - Only explore what's relevant to the task
4. **Use tools effectively**:
   - `glob_files` for finding files by pattern
   - `grep_files` for searching content
   - `read_file` for examining specific files
   - `list_dir` for directory structure

## Output Format

Provide a clear summary with:
- Key findings
- Relevant file paths
- Code snippets if helpful
- Any patterns or observations
```

### 4.2 Plan Agent

**Purpose**: Implementation planning and architecture design

```rust
pub fn create_plan_agent() -> AgentDefinition {
    AgentDefinition {
        agent_type: "Plan".to_string(),
        when_to_use: Some(
            "Creating implementation plans, designing architecture, \
             analyzing trade-offs.".to_string()
        ),
        tools: ToolAccess::List(vec![
            "read_file".to_string(),
            "glob_files".to_string(),
            "grep_files".to_string(),
        ]),
        disallowed_tools: vec![],
        source: AgentSource::Builtin,
        model: AgentModel::Inherit,
        fork_context: true,  // Include conversation context
        system_prompt: Some(include_str!("prompts/plan.md").to_string()),
        run_config: AgentRunConfig {
            max_time_seconds: 300,  // 5 minutes
            max_turns: 50,
        },
        ..Default::default()
    }
}
```

**prompts/plan.md**:

```markdown
You are a software architect creating an implementation plan.

Working directory: ${cwd}

Task: ${prompt}

## Planning Process

1. **Understand the requirements** - Clarify scope and constraints
2. **Explore existing code** - Find relevant patterns and dependencies
3. **Design the approach** - Consider trade-offs and alternatives
4. **Create actionable steps** - Break down into concrete tasks

## Output Format

Provide a structured plan with:

### Overview
Brief summary of the approach

### Critical Files
List of files that will be modified or created

### Implementation Steps
1. Step 1 - Description
2. Step 2 - Description
...

### Considerations
- Trade-offs
- Risks
- Dependencies
```

---

## 5. Integration

### 5.1 Core Integration Points

#### 5.1.1 Module Declaration

**File**: `core/src/lib.rs`

```rust
// Add module declaration (feature-gated)
#[cfg(feature = "subagent")]
pub(crate) mod subagent;
```

#### 5.1.2 Tool Registration

**File**: `core/src/tools/spec_ext.rs`

```rust
#[cfg(feature = "subagent")]
use crate::subagent::{
    AgentRegistry, BackgroundTaskStore,
    SubagentToolHandler, TaskOutputHandler,
    TASK_TOOL, TASK_OUTPUT_TOOL,
};

#[cfg(feature = "subagent")]
pub fn register_subagent_tools(
    builder: &mut ToolRegistryBuilder,
    registry: Arc<AgentRegistry>,
    background_store: Arc<BackgroundTaskStore>,
) {
    builder.push_spec(TASK_TOOL.clone());
    builder.push_spec(TASK_OUTPUT_TOOL.clone());
    builder.register_handler(
        "Task",
        Arc::new(SubagentToolHandler::new(registry.clone(), background_store.clone()))
    );
    builder.register_handler(
        "TaskOutput",
        Arc::new(TaskOutputHandler::new(background_store))
    );
}
```

#### 5.1.3 Session Services

**File**: `core/src/codex.rs` (modify SessionServices)

```rust
pub struct SessionServices {
    // ... existing fields ...

    #[cfg(feature = "subagent")]
    pub agent_registry: Arc<AgentRegistry>,
    #[cfg(feature = "subagent")]
    pub background_tasks: Arc<BackgroundTaskStore>,
}
```

### 5.2 Protocol Extensions (Enhanced with Gemini Patterns + Current Implementation)

**File**: `protocol/src/protocol.rs`

```rust
/// Session source tracking (UPDATE existing enum)
pub enum SessionSource {
    Interactive,
    Exec,
    SubAgent(SubAgentSource),
}

/// Extended SubAgentSource for new agent types (NEW: extend existing)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubAgentSource {
    Review,              // Existing: code review
    Compact,             // Existing: (unused)
    Explore,             // NEW: codebase exploration
    Plan,                // NEW: implementation planning
    Custom(String),      // NEW: replaces Other(String)
}

pub enum EventMsg {
    // ... existing variants ...

    /// Subagent activity event for UI feedback
    SubagentActivity(SubagentActivityEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentActivityEvent {
    pub agent_id: String,
    pub agent_type: String,
    pub event_type: SubagentEventType,
    pub data: HashMap<String, serde_json::Value>,  // NEW: Flexible event data
}

/// Enhanced event types (from Gemini)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SubagentEventType {
    // Lifecycle events
    Started,
    Completed,
    Error,

    // Turn events
    TurnStart,
    TurnComplete,

    // Tool events (NEW: from Gemini)
    ToolCallStart,      // When tool execution begins
    ToolCallEnd,        // When tool execution completes

    // Streaming events (NEW: from Gemini)
    ThoughtChunk,       // Streaming thought/reasoning output

    // Grace period events (NEW)
    GracePeriodStart,
    GracePeriodEnd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentProgress {
    pub turns_completed: i32,
    pub max_turns: i32,
    pub elapsed_seconds: i32,
    pub max_seconds: i32,
}
```

**Event Data Examples**:

```rust
// ToolCallStart event data
{
    "tool_name": "read_file",
    "arguments": { "path": "/src/main.rs" }
}

// ToolCallEnd event data
{
    "tool_name": "read_file",
    "success": true,
    "duration_ms": 42
}

// ThoughtChunk event data (for streaming UI)
{
    "text": "I found the main entry point..."
}

// GracePeriodStart event data
{
    "reason": "timeout",
    "grace_seconds": 60
}
```

#### TUI Integration Note

The TUI must handle `EventMsg::SubagentActivity(SubagentActivityEvent)` in `tui/src/chatwidget.rs` to display subagent progress to users. Recommended display patterns:

```rust
// tui/src/chatwidget.rs - Add to EventMsg match arm

EventMsg::SubagentActivity(event) => {
    match event.event_type {
        SubagentEventType::Started => {
            // Display agent start with spinner
            // Format: "[agent_type] Starting: {description}"
            let line = format!(
                "[{}] Starting: {}",
                event.agent_type.cyan(),
                event.data.get("description").and_then(|v| v.as_str()).unwrap_or("")
            );
            self.add_system_message(line);
        }
        SubagentEventType::TurnStart | SubagentEventType::TurnComplete => {
            // Update progress indicator
            if let Some(progress) = event.data.get("progress") {
                // Format: "Turn 3/50 (12s/300s)"
                self.update_subagent_progress(&event.agent_id, progress);
            }
        }
        SubagentEventType::ToolCallStart => {
            // Show tool activity (collapsed by default)
            let tool_name = event.data.get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            // Format: "  → Calling {tool_name}..."
            self.add_tool_activity(&event.agent_id, tool_name);
        }
        SubagentEventType::Completed => {
            // Display completion status
            // Format: "[agent_type] Completed in {duration}s ({turns} turns)"
            let duration = event.data.get("duration_seconds")
                .and_then(|v| v.as_f64()).unwrap_or(0.0);
            let turns = event.data.get("turns_used")
                .and_then(|v| v.as_i64()).unwrap_or(0);
            let line = format!(
                "[{}] Completed in {:.1}s ({} turns)",
                event.agent_type.green(),
                duration,
                turns
            );
            self.add_system_message(line);
        }
        SubagentEventType::Error => {
            // Display error with details
            let error = event.data.get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            let line = format!("[{}] Error: {}", event.agent_type.red(), error);
            self.add_system_message(line);
        }
        SubagentEventType::GracePeriodStart => {
            // Warn user about recovery attempt
            let reason = event.data.get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let line = format!(
                "[{}] {} - attempting recovery...",
                event.agent_type.yellow(),
                reason
            );
            self.add_system_message(line);
        }
        _ => {}  // Handle other events as needed
    }
}
```

**Key Display Requirements**:
1. Use appropriate colors: `.cyan()` for start, `.green()` for success, `.red()` for error, `.yellow()` for warnings
2. Keep messages concise - subagent activity should not overwhelm the main conversation
3. Collapse detailed tool calls by default (expandable on user request)
4. Show progress indicator for long-running agents (turns completed, time elapsed)
5. Ensure all activity is prefixed with `[agent_type]` for clarity

### 5.3 Configuration

**File**: `protocol/src/config_types_ext.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct SubagentConfig {
    /// Enable/disable subagent feature
    pub enabled: bool,

    /// Directory for user-defined agent definitions
    pub agents_dir: Option<PathBuf>,

    /// Default timeout for subagents (seconds)
    #[serde(default = "default_timeout")]
    pub default_timeout_seconds: i32,

    /// Default max turns for subagents
    #[serde(default = "default_max_turns")]
    pub default_max_turns: i32,
}

fn default_timeout() -> i32 { 300 }
fn default_max_turns() -> i32 { 50 }
```

### 5.4 Dependencies

**File**: `core/Cargo.toml`

```toml
[features]
default = []
subagent = ["dashmap", "serde_yaml", "uuid"]

[dependencies]
# ... existing dependencies ...

# Subagent feature dependencies
dashmap = { version = "6", optional = true }
serde_yaml = { version = "0.9", optional = true }
uuid = { version = "1", features = ["v4"], optional = true }
```

---

## 6. Acceptance Criteria

### 6.1 Functional Requirements

| ID | Requirement | Verification |
|----|-------------|--------------|
| F1 | Task tool spawns subagent with correct definition | Unit test |
| F2 | Subagent respects tool restrictions (three-tier) | Unit test |
| F3 | Subagent respects timeout and max_turns | Integration test |
| F4 | Background execution returns agent_id immediately | Integration test |
| F5 | TaskOutput retrieves results correctly | Integration test |
| F6 | YAML/MD agent definitions are parsed correctly | Unit test |
| F7 | Built-in Explore agent works end-to-end | Integration test |
| F8 | Built-in Plan agent works end-to-end | Integration test |
| F9 | Cancellation propagates to subagent | Integration test |
| F10 | User agents can be loaded from directory | Integration test |

### 6.2 Non-Functional Requirements

| ID | Requirement | Verification |
|----|-------------|--------------|
| NF1 | No .unwrap() in non-test code | Code review |
| NF2 | All types are Send + Sync | Compile check |
| NF3 | Uses i32/i64 (not u32/u64) | Code review |
| NF4 | Errors use CodexErr | Code review |
| NF5 | Serde defaults for optional fields | Code review |
| NF6 | Feature-gated with `subagent` feature | Compile check |

### 6.3 Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yaml_parser_basic() {
        let yaml = r#"
agentType: test-agent
whenToUse: "Testing"
tools:
  - read_file
model: inherit
"#;
        let def: AgentDefinition = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.agent_type, "test-agent");
        assert_eq!(def.model, AgentModel::Inherit);
    }

    #[test]
    fn test_tool_filter_always_blocked() {
        let def = AgentDefinition::default();
        let filter = ToolFilter::new(def, HashSet::new());
        assert!(!filter.is_allowed("Task"));
        assert!(!filter.is_allowed("TodoWrite"));
    }

    #[test]
    fn test_tool_filter_non_builtin_blocked() {
        let mut def = AgentDefinition::default();
        def.source = AgentSource::User;
        let mut parent = HashSet::new();
        parent.insert("shell".to_string());
        let filter = ToolFilter::new(def, parent);
        assert!(!filter.is_allowed("shell"));
    }

    #[test]
    fn test_tool_filter_explicit_disallow() {
        let mut def = AgentDefinition::default();
        def.source = AgentSource::Builtin;
        def.disallowed_tools = vec!["read_file".to_string()];
        let mut parent = HashSet::new();
        parent.insert("read_file".to_string());
        let filter = ToolFilter::new(def, parent);
        assert!(!filter.is_allowed("read_file"));
    }

    #[tokio::test]
    async fn test_registry_builtin_agents() {
        let registry = AgentRegistry::new();
        assert!(registry.get("Explore").await.is_some());
        assert!(registry.get("Plan").await.is_some());
        assert!(registry.get("NonExistent").await.is_none());
    }

    #[tokio::test]
    async fn test_background_task_store() {
        let store = BackgroundTaskStore::new();
        // Test spawn and retrieval...
    }
}
```

---

## 7. Development Tasks

### Phase 1: Foundation (Est: 4 hours)

| Task ID | Task | Dependencies | Hours |
|---------|------|--------------|-------|
| T1.1 | Create module structure (`core/src/subagent/`) | None | 1 |
| T1.2 | Add dependencies to `core/Cargo.toml` with feature flag | T1.1 | 0.5 |
| T1.3 | Implement AgentDefinition types with serde | T1.2 | 2 |
| T1.4 | Add module declaration in `core/src/lib.rs` | T1.3 | 0.5 |

**Deliverables**:
- `core/src/subagent/mod.rs`
- `core/src/subagent/definition/mod.rs`
- `core/src/subagent/error.rs`
- Updated `core/Cargo.toml`

### Phase 2: Definition Parser (Est: 4.5 hours)

| Task ID | Task | Dependencies | Hours |
|---------|------|--------------|-------|
| T2.1 | Implement YAML parser (`definition/parser.rs`) | T1.4 | 2 |
| T2.2 | Implement MD parser (YAML frontmatter) | T2.1 | 1.5 |
| T2.3 | Implement template substitution (`${var}`) | T2.2 | 1 |

**Deliverables**:
- `core/src/subagent/definition/parser.rs`
- Unit tests for parsing

### Phase 3: Built-in Agents (Est: 3 hours)

| Task ID | Task | Dependencies | Hours |
|---------|------|--------------|-------|
| T3.1 | Create Explore agent definition | T2.3 | 1 |
| T3.2 | Create Plan agent definition | T2.3 | 1 |
| T3.3 | Create system prompt templates | T3.1, T3.2 | 1 |

**Deliverables**:
- `core/src/subagent/definition/builtin.rs`
- `core/src/subagent/prompts/explore.md`
- `core/src/subagent/prompts/plan.md`

### Phase 4: Registry (Est: 3 hours)

| Task ID | Task | Dependencies | Hours |
|---------|------|--------------|-------|
| T4.1 | Implement AgentRegistry core | T3.3 | 2 |
| T4.2 | Implement file-based agent loader | T4.1 | 1 |

**Deliverables**:
- `core/src/subagent/registry.rs`
- Unit tests for registry

### Phase 5: Tool Filter (Est: 3 hours)

| Task ID | Task | Dependencies | Hours |
|---------|------|--------------|-------|
| T5.1 | Implement ToolFilter with three-tier logic | T4.2 | 2 |
| T5.2 | Add tool filter unit tests | T5.1 | 1 |

**Deliverables**:
- `core/src/subagent/executor/tool_filter.rs`
- Comprehensive unit tests

### Phase 6: Executor (Est: 6 hours)

| Task ID | Task | Dependencies | Hours |
|---------|------|--------------|-------|
| T6.1 | Implement SubagentContext | T5.2 | 1.5 |
| T6.2 | Implement AgentExecutor main loop | T6.1 | 3 |
| T6.3 | Add timeout and max_turns handling | T6.2 | 1 |
| T6.4 | Add cancellation support | T6.3 | 0.5 |

**Deliverables**:
- `core/src/subagent/executor/mod.rs`
- `core/src/subagent/executor/context.rs`

### Phase 7: Tool Handler (Est: 4 hours)

| Task ID | Task | Dependencies | Hours |
|---------|------|--------------|-------|
| T7.1 | Implement SubagentToolHandler | T6.4 | 2 |
| T7.2 | Implement TASK_TOOL specification | T7.1 | 1 |
| T7.3 | Add integration hook in spec_ext.rs | T7.2 | 1 |

**Deliverables**:
- `core/src/subagent/handlers/mod.rs`
- `core/src/subagent/handlers/task.rs`

### Phase 8: Background Execution (Est: 4 hours)

| Task ID | Task | Dependencies | Hours |
|---------|------|--------------|-------|
| T8.1 | Implement BackgroundTaskStore | T7.3 | 2 |
| T8.2 | Implement TaskOutputHandler | T8.1 | 1.5 |
| T8.3 | Implement TASK_OUTPUT_TOOL specification | T8.2 | 0.5 |

**Deliverables**:
- `core/src/subagent/background.rs`
- `core/src/subagent/handlers/task_output.rs`

### Phase 9: Protocol & Config (Est: 2 hours)

| Task ID | Task | Dependencies | Hours |
|---------|------|--------------|-------|
| T9.1 | Add SubagentActivityEvent to protocol | T8.3 | 1 |
| T9.2 | Add SubagentConfig to config_types | T9.1 | 0.5 |
| T9.3 | Update SessionServices | T9.2 | 0.5 |

**Deliverables**:
- Updated `protocol/src/protocol.rs`
- Updated `protocol/src/config_types_ext.rs`

### Phase 10: Testing (Est: 6 hours)

| Task ID | Task | Dependencies | Hours |
|---------|------|--------------|-------|
| T10.1 | Unit tests for parser and registry | T9.3 | 2 |
| T10.2 | Unit tests for tool filter | T9.3 | 1 |
| T10.3 | Integration tests for executor | T9.3 | 3 |

**Deliverables**:
- Comprehensive test suite
- CI integration

### Task Dependency Graph

```
T1.1 → T1.2 → T1.3 → T1.4
                       ↓
T2.1 → T2.2 → T2.3
                ↓
        T3.1 ──┬── T3.3
        T3.2 ──┘     ↓
                   T4.1 → T4.2
                           ↓
                   T5.1 → T5.2
                           ↓
           T6.1 → T6.2 → T6.3 → T6.4
                                 ↓
                   T7.1 → T7.2 → T7.3
                                 ↓
                   T8.1 → T8.2 → T8.3
                                 ↓
                   T9.1 → T9.2 → T9.3
                                 ↓
               T10.1, T10.2, T10.3 (parallel)
```

### Summary

| Phase | Description | Hours |
|-------|-------------|-------|
| 1 | Foundation | 4 |
| 2 | Parser | 4.5 |
| 3 | Built-in Agents | 3 |
| 4 | Registry | 3 |
| 5 | Tool Filter | 3 |
| 6 | Executor | 6 |
| 7 | Tool Handler | 4 |
| 8 | Background | 4 |
| 9 | Protocol | 2 |
| 10 | Testing | 6 |
| **Total** | | **39.5** |

---

## 8. Risks and Mitigations

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Upstream sync conflicts | Medium | Medium | Feature-gated, minimal integration points |
| Model client complexity | High | Low | Reuse existing ModelClient infrastructure |
| Tool execution isolation | Medium | Medium | Strict ToolFilter, SubagentContext isolation |
| Memory leaks in background tasks | Medium | Low | Automatic cleanup with timeout |
| Infinite loops | High | Low | Strict max_turns and timeout enforcement |
| Performance overhead | Low | Medium | Lazy initialization, efficient DashMap |

---

## 9. Future Enhancements

1. **Agent Skills System**: Support for preloading skills into agents
2. **Custom Model Selection**: Per-agent model configuration with validation
3. **Agent Composition**: Agents calling other agents (with depth limits)
4. **Persistent Agent State**: Resume agents across sessions
5. **Agent Metrics**: Detailed usage and performance tracking
6. **Agent Marketplace**: Share and discover community agents
7. **Agent Debugging**: Step-through execution for development

---

## 10. References

- [Claude Code Subagent Analysis](./claude_code_subagent_analysis.md)
- [Current Codex Subagent Implementation](./subagent.md) - Existing Task/SubAgent patterns
- [Gemini-CLI Subagent Implementation Analysis](./gemini_subagent.md) - Detailed Gemini patterns
- [Gemini-CLI Agent Source](../gemini-cli/packages/core/src/agents/)
- [Codex Tools System](../codex-rs/core/src/tools/)

### Key Gemini Patterns Adopted

| Pattern | Source File | Description |
|---------|-------------|-------------|
| complete_task | executor.ts | Explicit completion signal with structured output |
| Grace Period | executor.ts | 60s recovery mechanism after timeout |
| InputConfig | types.ts | Typed input parameters with JSON Schema |
| OutputConfig | types.ts | Structured output with Zod schema validation |
| ModelConfig | types.ts | Temperature, top_p, thinkingBudget settings |
| Activity Events | types.ts | TOOL_CALL_START/END, THOUGHT_CHUNK |
| SubagentToolWrapper | subagent-tool-wrapper.ts | Agent-to-Tool encapsulation |
| AgentRegistry | registry.ts | Agent discovery and configuration merging |

### Key Current Implementation Patterns Adopted

| Pattern | Source File | Description |
|---------|-------------|-------------|
| Event Forwarding | codex_delegate.rs | Forward events from subagent to parent session |
| Approval Routing | codex_delegate.rs | Route ExecApproval/ApplyPatchApproval to parent |
| SessionSource | protocol.rs | Track SubAgent source for telemetry |
| Config Customization | tasks/review.rs | Disable features per-agent |
| CancellationToken | tasks/mod.rs | Abort propagation from parent |

---

## 11. Architecture Comparison

### Current Implementation vs v2

| Aspect | Current (`subagent.md`) | v2 Design |
|--------|------------------------|-----------|
| **Entry Point** | Slash Command → Op → Task | Tool Router → SubagentToolHandler |
| **Agent Definition** | Hardcoded in Task files | YAML/MD configuration files |
| **Execution** | Full Codex instance (Codex::spawn) | Lightweight AgentExecutor or FullCodex mode |
| **Tool Restriction** | Per-task hardcoded | Three-tier ToolFilter |
| **Configuration** | Code changes required | Runtime loading from `.claude/agents/` |
| **Event Handling** | forward_events() in codex_delegate.rs | SubagentEventBridge |
| **Background Tasks** | Not supported | BackgroundTaskStore with TaskOutput |
| **Completion Signal** | Implicit final message | Explicit complete_task tool |
| **Timeout Recovery** | None | Grace Period (60s) |
| **Agent Types** | Review only | Explore, Plan, Custom |

### Data Flow Comparison

**Current Implementation:**
```
User → SlashCommand → Op → Session → Task → Codex::spawn() → SubAgent
                                              ↓
                                    codex_delegate.rs
                                              ↓
                                    forward_events() → Parent
```

**v2 Design:**
```
LLM Tool Call → ToolRouter → SubagentToolHandler
                                    │
                    ┌───────────────┴───────────────┐
                    │                               │
             (Lightweight mode)              (FullCodex mode)
                    │                               │
             AgentExecutor                   Codex::spawn()
                    │                               │
             SubagentEventBridge ◄──────────────────┘
                    │
             Parent Session
```

### Migration Path

For existing ReviewTask:
1. Continue using current `Codex::spawn()` via `execution_mode: FullCodex`
2. Event forwarding handled by `SubagentEventBridge`
3. Approval routing preserved via `approval_mode: RouteToParent`

For new agents (Explore, Plan):
1. Use `execution_mode: Lightweight` (default)
2. `approval_mode: Suppress` (read-only tools)
3. Define via YAML/MD in `.claude/agents/`
