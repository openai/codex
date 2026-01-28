# Multi-Agent System Design

## Overview

The subagent system allows spawning child agents with different configurations, models, and tool access. This mirrors Claude Code's Task tool and built-in agents.

**Key Design Points:**
- Tool configuration uses `tools: Option<Vec<String>>` + `disallowed_tools: Option<Vec<String>>` (not ToolFilter)
- **6 built-in agents**: Bash, general-purpose, Explore, Plan, claude-code-guide, statusline-setup
- Additional agents (code-simplifier, etc.) can come from settings/plugins
- **Located in `core/subagent/`**, depends on `core/executor/` for base AgentExecutor
- Subagent lifecycle has dedicated hooks (SubagentStart, SubagentStop)

## Relationship with Executor

```
┌─────────────────────────────────────────────────────────────┐
│                      Entry Points                            │
│  Task tool    CLI --iter      /iter cmd     Collab tools    │
│      │            │               │              │          │
│      ▼            ▼               ▼              ▼          │
├──────┴────────────┴───────────────┴──────────────┴──────────┤
│                                                              │
│  ┌─────────────────┐     ┌────────────────────────────────┐ │
│  │ core/subagent   │     │        core/executor           │ │
│  │                 │     │                                │ │
│  │ SubagentManager │     │  AgentExecutor (base)          │ │
│  │ AgentDefinition │────▶│  IterativeExecutor             │ │
│  │ Context forking │     │  AgentCoordinator              │ │
│  │ Tool filtering  │     │  Collab tools                  │ │
│  └────────┬────────┘     └────────────┬───────────────────┘ │
│           │                           │                     │
│           └───────────┬───────────────┘                     │
│                       ▼                                     │
│               ┌───────────────┐                             │
│               │  core/loop    │                             │
│               │  AgentLoop    │                             │
│               └───────────────┘                             │
└─────────────────────────────────────────────────────────────┘
```

**Key distinction:**
- **Subagent** (Task tool): Inherits parent context, filters tools, spawned by main agent
- **AgentExecutor**: Independent execution, no parent context, used by iterative/collab

See [execution-modes.md](execution-modes.md) for advanced execution patterns.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    SubagentManager                          │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              Agent Definitions                       │   │
│  │  - Built-in (Bash, general-purpose, Explore, Plan)  │   │
│  │  - Custom (from settings/plugins)                    │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─────────────────────┐  ┌─────────────────────────────┐  │
│  │   Running Agents    │  │   Completed Agents          │  │
│  │   HashMap<id, ...>  │  │   HashMap<id, ...>          │  │
│  └─────────────────────┘  └─────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
              │
              │ spawn()
              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Child AgentLoop                        │
│  - Forked context (optional)                                │
│  - Filtered tools (three-layer filtering)                   │
│  - Selected model                                           │
│  - Own event channel                                        │
└─────────────────────────────────────────────────────────────┘
```

## Core Types

### AgentDefinition

**Uses tools[]/disallowed_tools[] pattern (aligned with Claude Code):**

```rust
/// Definition of an agent type
#[derive(Debug, Clone, Default)]
pub struct AgentDefinition {
    /// Unique identifier for this agent type
    pub agent_type: String,

    /// Description of when to use this agent (shown to main agent)
    pub when_to_use: String,

    /// Allow-list of tool names. Use vec!["*"] for all tools.
    /// If Some and not "*", only these tools are available.
    pub tools: Option<Vec<String>>,

    /// Deny-list of tool names. Always excluded.
    pub disallowed_tools: Option<Vec<String>>,

    /// Where this definition comes from
    pub source: AgentSource,

    /// Model selection strategy
    pub model: ModelSelection,

    /// Override permission mode
    pub permission_mode: Option<PermissionMode>,

    /// Whether to fork conversation context
    pub fork_context: bool,

    /// Display color for UI
    pub color: Option<String>,

    /// Critical reminder to inject into prompt
    pub critical_reminder: Option<String>,

    /// System prompt generator
    pub system_prompt: SystemPromptFn,

    /// Skills to load for this agent
    pub skills: Option<Vec<String>>,
}

/// Where an agent definition originates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentSource {
    #[default]
    BuiltIn,
    UserSettings,
    ProjectSettings,
    Plugin,
}

/// Model selection for subagent
#[derive(Debug, Clone, Default)]
pub enum ModelSelection {
    /// Use parent agent's model (default)
    #[default]
    Inherit,
    /// Use a specific provider/model (full model ID required)
    Specific { provider: String, model: String },
}

/// System prompt generator function
pub type SystemPromptFn = Arc<dyn Fn(&AgentPromptContext) -> String + Send + Sync>;
```

### Built-in Agents (6 agents)

```rust
/// Get all built-in agent definitions
pub fn builtin_agents() -> Vec<AgentDefinition> {
    vec![
        bash_agent(),
        general_purpose_agent(),
        explore_agent(),
        plan_agent(),
        claude_code_guide_agent(),
        statusline_setup_agent(),
    ]
}

/// Bash agent - command execution specialist
pub fn bash_agent() -> AgentDefinition {
    AgentDefinition {
        agent_type: "Bash".to_string(),
        when_to_use: "Command execution specialist for running bash commands. \
                      Use this for git operations, command execution, and terminal tasks.".to_string(),
        tools: Some(vec!["Bash".to_string()]),  // Allow-list: only Bash
        disallowed_tools: None,
        source: AgentSource::BuiltIn,
        model: ModelSelection::Inherit,
        permission_mode: None,
        fork_context: false,
        color: None,
        critical_reminder: None,
        system_prompt: Arc::new(|_| BASH_SYSTEM_PROMPT.to_string()),
        skills: None,
    }
}

/// General purpose agent - all tools, full capability
pub fn general_purpose_agent() -> AgentDefinition {
    AgentDefinition {
        agent_type: "general-purpose".to_string(),
        when_to_use: "General-purpose agent for researching complex questions, \
                      searching for code, and executing multi-step tasks.".to_string(),
        tools: Some(vec!["*".to_string()]),  // All tools
        disallowed_tools: Some(vec!["Task".to_string()]),  // Except Task (no recursive spawning)
        source: AgentSource::BuiltIn,
        model: ModelSelection::Inherit,
        permission_mode: None,
        fork_context: true,  // Gets conversation history
        color: None,
        critical_reminder: None,
        system_prompt: Arc::new(|ctx| {
            format!("{}\n\n{}", GENERAL_PURPOSE_PROMPT, ctx.parent_context_summary)
        }),
        skills: None,
    }
}

/// Explore agent - fast codebase exploration with thoroughness levels
pub fn explore_agent() -> AgentDefinition {
    AgentDefinition {
        agent_type: "Explore".to_string(),
        when_to_use: "Fast agent specialized for exploring codebases. \
                      Use for finding files, searching code, or answering codebase questions. \
                      Specify thoroughness level: 'quick' for basic searches, 'medium' for \
                      moderate exploration, 'very thorough' for comprehensive analysis.".to_string(),
        tools: Some(vec!["*".to_string()]),  // All tools
        disallowed_tools: Some(vec![
            "Task".to_string(),
            "ExitPlanMode".to_string(),
            "EnterPlanMode".to_string(),
            "AskUserQuestion".to_string(),
            "Edit".to_string(),
            "Write".to_string(),
            "NotebookEdit".to_string(),
        ]),
        source: AgentSource::BuiltIn,
        model: ModelSelection::Inherit,  // Use parent's model
        permission_mode: Some(PermissionMode::Bypass),  // Read-only, no permission needed
        fork_context: false,
        color: Some("cyan".to_string()),
        critical_reminder: Some("CRITICAL: This is a READ-ONLY exploration task.".to_string()),
        system_prompt: Arc::new(|ctx| {
            let thoroughness = ctx.extra.get("thoroughness")
                .and_then(|v| v.as_str())
                .unwrap_or("medium");
            format!("{}\n\nThoroughness level: {}", EXPLORE_SYSTEM_PROMPT, thoroughness)
        }),
        skills: None,
    }
}

/// Thoroughness levels for Explore agent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Thoroughness {
    /// Basic searches, single-pass exploration
    Quick,
    /// Moderate exploration with some follow-up
    #[default]
    Medium,
    /// Comprehensive analysis across multiple locations
    VeryThorough,
}

impl Thoroughness {
    pub fn as_str(&self) -> &'static str {
        match self {
            Thoroughness::Quick => "quick",
            Thoroughness::Medium => "medium",
            Thoroughness::VeryThorough => "very thorough",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "quick" => Thoroughness::Quick,
            "very thorough" | "thorough" => Thoroughness::VeryThorough,
            _ => Thoroughness::Medium,
        }
    }
}

/// Plan agent - architecture and planning
pub fn plan_agent() -> AgentDefinition {
    AgentDefinition {
        agent_type: "Plan".to_string(),
        when_to_use: "Software architect agent for designing implementation plans. \
                      Returns step-by-step plans and identifies critical files.".to_string(),
        tools: Some(vec![
            "Glob".to_string(),
            "Grep".to_string(),
            "Read".to_string(),
            "Bash".to_string(),
        ]),
        disallowed_tools: Some(vec![
            "Task".to_string(),
            "ExitPlanMode".to_string(),
            "EnterPlanMode".to_string(),
            "Edit".to_string(),
            "Write".to_string(),
            "NotebookEdit".to_string(),
        ]),
        source: AgentSource::BuiltIn,
        model: ModelSelection::Inherit,
        permission_mode: None,
        fork_context: false,
        color: Some("blue".to_string()),
        critical_reminder: Some("CRITICAL: This is a READ-ONLY planning task.".to_string()),
        system_prompt: Arc::new(|_| PLAN_SYSTEM_PROMPT.to_string()),
        skills: None,
    }
}

/// Claude Code Guide agent - help with Claude Code features
pub fn claude_code_guide_agent() -> AgentDefinition {
    AgentDefinition {
        agent_type: "claude-code-guide".to_string(),
        when_to_use: "Use this agent when the user asks questions about: \
                      (1) Claude Code (the CLI tool) - features, hooks, slash commands, \
                      MCP servers, settings, IDE integrations, keyboard shortcuts; \
                      (2) Claude Agent SDK - building custom agents; \
                      (3) Claude API - API usage, tool use, Anthropic SDK usage. \
                      IMPORTANT: Check if there's already a running/completed guide agent \
                      to resume using the 'resume' parameter.".to_string(),
        tools: Some(vec![
            "Glob".to_string(),
            "Grep".to_string(),
            "Read".to_string(),
            "WebFetch".to_string(),
            "WebSearch".to_string(),
        ]),
        disallowed_tools: Some(vec![
            "Task".to_string(),
            "ExitPlanMode".to_string(),
            "EnterPlanMode".to_string(),
            "Edit".to_string(),
            "Write".to_string(),
            "NotebookEdit".to_string(),
            "Bash".to_string(),
        ]),
        source: AgentSource::BuiltIn,
        model: ModelSelection::Inherit,  // Use parent's model
        permission_mode: Some(PermissionMode::Bypass),  // Read-only
        fork_context: false,
        color: Some("green".to_string()),
        critical_reminder: Some("CRITICAL: This is a READ-ONLY help task. \
                                 Do not modify any files.".to_string()),
        system_prompt: Arc::new(|_| CLAUDE_CODE_GUIDE_PROMPT.to_string()),
        skills: None,
    }
}

const CLAUDE_CODE_GUIDE_PROMPT: &str = r#"
You are the Claude Code Guide agent, specialized in helping users understand and use Claude Code effectively.

Your expertise covers:
1. **Claude Code CLI** - features, hooks, slash commands, MCP servers, settings, IDE integrations, keyboard shortcuts
2. **Claude Agent SDK** - building custom agents
3. **Claude API** - API usage, tool use, Anthropic SDK usage

When answering questions:
- Search for relevant documentation and code examples
- Provide clear, actionable guidance
- Include code snippets when helpful
- Reference official documentation when available

Remember: This is a READ-ONLY task. Do not modify any files.
"#;

// NOTE: The claude-code-guide agent is Claude Code-specific and provides help
// for Claude Code product features (hooks, MCP, settings, SDK).
// For cocode-rs implementation, this agent type can be:
// - Skipped entirely, or
// - Replaced with a cocode-specific help agent
// The agent is included here for documentation completeness with Claude Code v2.1.7.

/// Statusline setup agent - configures custom status line
pub fn statusline_setup_agent() -> AgentDefinition {
    AgentDefinition {
        agent_type: "statusline-setup".to_string(),
        when_to_use: "Use this agent to configure the user's Claude Code status line setting.".to_string(),
        tools: Some(vec!["Read".to_string(), "Edit".to_string()]),
        disallowed_tools: None,
        source: AgentSource::BuiltIn,
        model: ModelSelection::Specific {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
        },
        permission_mode: None,
        fork_context: false,
        color: Some("orange".to_string()),
        critical_reminder: None,
        system_prompt: Arc::new(|_| STATUSLINE_SETUP_PROMPT.to_string()),
        skills: None,
    }
}

const STATUSLINE_SETUP_PROMPT: &str = r#"
You are the statusline-setup agent, specialized in configuring the user's status line setting.

Your task is to help users customize their Claude Code status line by:
1. Reading the current settings configuration
2. Understanding what status line information the user wants
3. Creating or modifying the statusLine command in their settings

The statusLine setting accepts a shell command that outputs status text (newline-separated lines).
The command is executed with a 5-second timeout and receives session context as JSON on stdin.

Example settings.json:
{
  "statusLine": "echo 'Branch: '$(git branch --show-current)"
}

Remember: Only modify the statusLine setting. Do not change other configuration.
"#;
```

### Three-Layer Tool Filtering (filterToolsByAllowDeny)

**Claude Code uses three layers of tool filtering:**

```rust
/// System-wide blocked tools for subagents
const ALWAYS_EXCLUDED_TOOLS: &[&str] = &[
    "Task",              // No recursive spawning (unless explicitly allowed)
    "EnterPlanMode",     // Plan mode is main agent only
    "ExitPlanMode",
    "AskUserQuestion",   // Subagents cannot prompt user directly
];

/// Filter tools for subagent based on three layers
pub fn filter_tools_for_agent(
    tools: &[Arc<dyn Tool>],
    agent_def: &AgentDefinition,
) -> Vec<Arc<dyn Tool>> {
    let denied: HashSet<_> = agent_def.disallowed_tools
        .as_ref()
        .map(|v| v.iter().cloned().collect())
        .unwrap_or_default();

    let allowed: Option<HashSet<_>> = agent_def.tools.as_ref()
        .filter(|v| !v.iter().any(|s| s == "*"))  // "*" means all
        .map(|v| v.iter().cloned().collect());

    tools.iter()
        .filter(|t| {
            let name = t.name();

            // Layer 1: System-wide blocked
            if ALWAYS_EXCLUDED_TOOLS.contains(&name) {
                return false;
            }

            // Layer 2: Agent-specific deny-list
            if denied.contains(name) {
                return false;
            }

            // Layer 3: Agent-specific allow-list (if specified and not "*")
            if let Some(ref allow_set) = allowed {
                if !allow_set.contains(name) {
                    return false;
                }
            }

            true
        })
        .cloned()
        .collect()
}
```

#### Layer 4: Async-Safe Tool Filtering

For background/async agents, additional filtering ensures only safe tools are available:

```rust
/// Tools safe for async execution (no user interaction required)
const ASYNC_SAFE_TOOLS: &[&str] = &[
    "Read", "Edit", "Write", "Glob", "Grep", "Bash",
    "WebFetch", "WebSearch", "NotebookEdit", "TaskOutput",
    "KillShell", "LSP",
];

pub fn filter_tools_for_async_agent(
    tools: Vec<Arc<dyn Tool>>,
    agent_def: &AgentDefinition,
    is_async: bool,
) -> Vec<Arc<dyn Tool>> {
    let mut filtered = filter_tools_for_agent(&tools, agent_def);

    if is_async {
        filtered.retain(|t| ASYNC_SAFE_TOOLS.contains(&t.name()));
    }

    filtered
}
```

**Four-Layer Filtering Summary:**
1. System-wide blocked: Task, EnterPlanMode, ExitPlanMode, AskUserQuestion
2. Agent-specific deny-list: `disallowed_tools`
3. Agent-specific allow-list: `tools`
4. Async-safe filter: Only for `run_in_background=true` agents

## Complete Subagent Flow

```
Main Agent (Task tool call)
    │
    ▼
┌───────────────────────────────────────────────────────────────┐
│                   runSubagentLoop                              │
│                                                                │
│  1. resolveAgentModel()                                       │
│     priority: agentModel → mainLoopModel → modelOverride      │
│                                                                │
│  2. filterForkContextMessages()                               │
│     - Remove orphaned tool_use without matching tool_result   │
│     - Filter by message recency if needed                     │
│                                                                │
│  3. filterToolsByAllowDeny() (three layers)                   │
│     - Layer 1: System-wide blocks (Task, EnterPlanMode, etc.) │
│     - Layer 2: Agent-specific deny-list                       │
│     - Layer 3: Agent-specific allow-list                      │
│                                                                │
│  4. createChildToolUseContext()                               │
│     - Inherit: getAppState, setAppState, mcpClients           │
│     - Override: tools, abortController, readFileState         │
│     - Unique: agentId                                         │
│                                                                │
│  5. loadAgentSkills()                                         │
│     - Inject skill prompt content into messages               │
│                                                                │
│  6. setupMcpClients()                                         │
│     - Inherit parent MCP clients                              │
│     - Merge agent-specific MCP config                         │
│                                                                │
│  7. registerAgentHooks()                                      │
│     - Emit SubagentStart hook                                 │
│     - Convert Stop → SubagentStop for subagents               │
│                                                                │
│  8. recordSidechainTranscript()                               │
│     - Persist to ~/.claude/projects/{session}/subagents/      │
│                                                                │
│  9. delegate to coreMessageLoop()                             │
│     - Full agent loop with filtered tools                     │
│     - Yield LoopEvents for progress                           │
│                                                                │
│  10. cleanup & return result                                  │
│                                                                │
└───────────────────────────────────────────────────────────────┘
            │
            ▼
    SubagentResult { agent_id, content, tokens, ... }
```

### SubagentManager

```rust
pub struct SubagentManager {
    /// All available agent definitions
    definitions: Vec<AgentDefinition>,

    /// Currently running agents
    running: Arc<RwLock<HashMap<String, RunningAgent>>>,

    /// Completed agents (for resume)
    completed: Arc<RwLock<HashMap<String, CompletedAgent>>>,

    /// Registry of all available providers (multi-provider support)
    provider_registry: Arc<ProviderRegistry>,

    /// Parent tool registry
    parent_tools: ToolRegistry,
}

/// Registry for managing multiple LLM providers
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn get(&self, name: &str) -> Option<Arc<dyn Provider>> {
        self.providers.get(name).cloned()
    }
}

struct RunningAgent {
    id: String,
    definition: AgentDefinition,
    handle: JoinHandle<AgentResult>,
    cancel: CancellationToken,
    started_at: SystemTime,
}

struct CompletedAgent {
    id: String,
    definition: AgentDefinition,
    result: AgentResult,
    context: ConversationContext,  // For resume
    completed_at: SystemTime,
}

impl SubagentManager {
    pub fn new(
        provider_registry: Arc<ProviderRegistry>,
        parent_tools: ToolRegistry,
    ) -> Self {
        Self {
            definitions: builtin_agents(),
            running: Arc::new(RwLock::new(HashMap::new())),
            completed: Arc::new(RwLock::new(HashMap::new())),
            provider_registry,
            parent_tools,
        }
    }

    /// Add custom agent definitions (from settings/plugins)
    pub fn add_definitions(&mut self, defs: Vec<AgentDefinition>) {
        self.definitions.extend(defs);
    }

    /// Spawn a new subagent
    pub async fn spawn(
        &self,
        input: SpawnInput,
        parent_context: &ToolUseContext,
        event_tx: mpsc::Sender<LoopEvent>,
    ) -> Result<String, SubagentError> {
        // 1. Find agent definition
        let def = self.definitions.iter()
            .find(|d| d.agent_type == input.subagent_type)
            .ok_or_else(|| SubagentError::unknown_type(&input.subagent_type))?
            .clone();

        // 2. Generate agent ID
        let agent_id = format!("agent_{}", uuid::Uuid::new_v4());

        // 3. Select model
        let model = self.resolve_model(&def.model, parent_context).await?;

        // 4. Filter tools using three-layer filtering
        let tools = filter_tools_for_agent(
            &self.parent_tools.all().collect::<Vec<_>>(),
            &def,
        );
        let filtered_registry = ToolRegistry::from(tools);

        // 5. Fork context if needed
        let context = if def.fork_context {
            parent_context.fork_filtered()
        } else {
            ConversationContext::new()
        };

        // 6. Build system prompt
        let prompt_ctx = AgentPromptContext {
            agent_type: &def.agent_type,
            parent_context_summary: parent_context.summary(),
        };
        let system_prompt = (def.system_prompt)(&prompt_ctx);

        // 7. Create loop config
        let config = LoopConfig {
            max_turns: input.max_turns,
            permission_mode: def.permission_mode.unwrap_or(PermissionMode::Default),
            agent_id: Some(agent_id.clone()),
            parent_agent_id: parent_context.agent_id.clone(),
            ..Default::default()
        };

        // 8. Build initial message
        let mut prompt = input.prompt.clone();
        if let Some(reminder) = &def.critical_reminder {
            prompt = format!("{reminder}\n\n{prompt}");
        }
        let initial_msg = ConversationMessage::user(prompt);

        // 9. Emit spawn event
        event_tx.send(LoopEvent::SubagentSpawned {
            agent_id: agent_id.clone(),
            agent_type: def.agent_type.clone(),
            description: input.description.clone(),
        }).await?;

        // 10. Spawn or run
        if input.run_in_background {
            self.spawn_background(
                agent_id.clone(), def, model, filtered_registry,
                context, config, initial_msg, event_tx.clone()
            ).await;
        } else {
            let result = self.run_foreground(
                agent_id.clone(), def.clone(), model, filtered_registry, context.clone(),
                config, initial_msg, event_tx.clone()
            ).await?;

            // Store for resume
            self.completed.write().await.insert(agent_id.clone(), CompletedAgent {
                id: agent_id.clone(),
                definition: def,
                result,
                context,
                completed_at: SystemTime::now(),
            });

            event_tx.send(LoopEvent::SubagentCompleted {
                agent_id: agent_id.clone(),
                result: result.final_text().to_string(),
            }).await?;
        }

        Ok(agent_id)
    }

    async fn resolve_model(
        &self,
        selection: &ModelSelection,
        parent_context: &ToolUseContext,
    ) -> Result<Arc<dyn Model>, SubagentError> {
        let (provider_name, model_name) = self.resolve_model_ref(selection, parent_context);

        // Get provider by name, then get model from that provider
        let provider = self.provider_registry
            .get(&provider_name)
            .ok_or_else(|| SubagentError::provider_not_found(&provider_name))?;

        provider.model(&model_name)
            .map_err(SubagentError::model_error)
    }

    /// Resolve model selection to (provider, model) tuple
    fn resolve_model_ref(
        &self,
        selection: &ModelSelection,
        parent_context: &ToolUseContext,
    ) -> (String, String) {
        match selection {
            ModelSelection::Inherit => {
                // Use parent's provider and model
                (
                    parent_context.provider.clone(),
                    parent_context.model.clone(),
                )
            }
            ModelSelection::Specific { provider, model } => {
                (provider.clone(), model.clone())
            }
        }
    }
}
```

### SpawnInput

```rust
/// Input for spawning a subagent (matches Claude Code's Task tool input)
#[derive(Debug, Clone)]
pub struct SpawnInput {
    /// Short description (3-5 words)
    pub description: String,

    /// The task prompt for the agent
    pub prompt: String,

    /// Type of agent to spawn
    pub subagent_type: String,

    /// Optional model override (full model ID, e.g., "claude-sonnet-4-20250514")
    pub model: Option<String>,

    /// Agent ID to resume from
    pub resume: Option<String>,

    /// Run in background
    pub run_in_background: bool,

    /// Maximum turns
    pub max_turns: Option<i32>,

    /// Additional tools to grant (requires permission)
    pub allowed_tools: Option<Vec<String>>,
}
```

## ToolUseContext (Multi-Provider)

The parent context must include both provider and model information for multi-provider support:

```rust
/// Context passed to tool handlers, includes provider/model info
pub struct ToolUseContext {
    /// Provider name (e.g., "anthropic", "openai", "google")
    pub provider: String,

    /// Model ID (e.g., "claude-sonnet-4-20250514", "gpt-4o")
    pub model: String,

    /// Session identifier
    pub session_id: String,

    /// Agent identifier (None for main agent)
    pub agent_id: Option<String>,

    /// Conversation messages
    pub messages: Vec<ConversationMessage>,

    // ... other fields (app_state, query_tracking, etc.)
}
```

## Context Forking (Detailed)

When `fork_context: true`, the subagent receives filtered conversation history:

```rust
pub struct ChildToolUseContext {
    /// Unique agent identifier
    pub agent_id: String,

    /// Forked conversation messages (filtered)
    pub messages: Vec<ConversationMessage>,

    /// Fresh read file state (subagent starts clean)
    pub read_file_state: Arc<RwLock<ReadFileState>>,

    /// Separate abort controller
    pub abort_controller: AbortController,

    /// SHARED: App state accessor
    pub get_app_state: Arc<dyn Fn() -> AppState + Send + Sync>,
    pub set_app_state: Arc<dyn Fn(AppStateUpdater) + Send + Sync>,

    /// Filtered tool options
    pub options: ToolUseOptions,

    /// Query tracking for analytics
    pub query_tracking: QueryTracking,
}

impl ChildToolUseContext {
    pub fn from_parent(
        parent: &ToolUseContext,
        agent_def: &AgentDefinition,
        fork_messages: bool,
    ) -> Self {
        let messages = if fork_messages {
            filter_fork_context_messages(&parent.messages)
        } else {
            vec![]
        };

        Self {
            agent_id: generate_agent_id(),
            messages,
            read_file_state: Arc::new(RwLock::new(ReadFileState::default())),
            abort_controller: AbortController::new(),
            get_app_state: parent.get_app_state.clone(),
            set_app_state: parent.set_app_state.clone(),
            options: filter_tools_for_agent(&parent.options.tools, agent_def),
            query_tracking: QueryTracking {
                chain_id: parent.query_tracking.chain_id.clone(),
                depth: parent.query_tracking.depth + 1,
            },
        }
    }
}

fn filter_fork_context_messages(messages: &[ConversationMessage]) -> Vec<ConversationMessage> {
    // Build set of tool_use IDs that have matching tool_result
    let result_ids: HashSet<_> = messages.iter()
        .filter_map(|m| m.tool_result_for_id())
        .collect();

    // Filter out orphaned tool_use blocks
    messages.iter()
        .filter(|m| {
            if let Some(tool_use_id) = m.tool_use_id() {
                result_ids.contains(&tool_use_id)
            } else {
                true
            }
        })
        .cloned()
        .collect()
}
```

## Plan File Naming for Subagents

When subagents operate in plan mode, they write to separate plan files to avoid conflicts with the main agent's plan:

```rust
/// Get plan file path for an agent
pub fn get_plan_file_path(agent_id: Option<&str>) -> PathBuf {
    let plans_dir = PathBuf::from("~/.claude/plans/");

    // Generate slug: adjective-action-noun pattern
    let slug = generate_plan_slug();

    match agent_id {
        // Main agent: {slug}.md
        None => plans_dir.join(format!("{slug}.md")),
        // Subagent: {slug}-agent-{agentId}.md
        Some(id) => plans_dir.join(format!("{slug}-agent-{id}.md")),
    }
}

/// Generate plan slug with adjective-action-noun pattern
pub fn generate_plan_slug() -> String {
    let adjectives = ["sparkling", "gentle", "swift", "quiet", "bold", "calm"];
    let actions = ["baking", "dancing", "running", "flying", "swimming", "climbing"];
    let nouns = ["fox", "river", "mountain", "forest", "cloud", "meadow"];

    let mut rng = rand::thread_rng();
    format!(
        "{}-{}-{}",
        adjectives.choose(&mut rng).unwrap(),
        actions.choose(&mut rng).unwrap(),
        nouns.choose(&mut rng).unwrap()
    )
}
```

**Examples:**
- Main agent plan: `~/.claude/plans/swift-running-fox.md`
- Subagent plan: `~/.claude/plans/swift-running-fox-agent-abc123.md`

This ensures that when Plan subagents are spawned in parallel during plan mode, each can write its findings to a separate file without conflicts.

## Resume Capability

```rust
/// Load messages from previous agent execution for resume
pub async fn load_resume_messages(
    session_id: &str,
    agent_id: &str,
) -> Result<Vec<ConversationMessage>, SubagentError> {
    let transcript_path = PathBuf::from(format!(
        "~/.claude/projects/{session_id}/subagents/agent-{agent_id}.jsonl"
    ));

    let file = tokio::fs::File::open(&transcript_path).await?;
    let reader = tokio::io::BufReader::new(file);
    let mut lines = reader.lines();

    let mut entries = Vec::new();
    while let Some(line) = lines.next_line().await? {
        let entry: SessionLogEntry = serde_json::from_str(&line)?;
        entries.push(entry);
    }

    // Reconstruct message chain using parentUuid pointers
    reconstruct_message_chain(entries)
}

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

impl SubagentManager {
    /// Resume a completed agent
    pub async fn resume(
        &self,
        agent_id: &str,
        additional_prompt: Option<String>,
        event_tx: mpsc::Sender<LoopEvent>,
    ) -> Result<String, SubagentError> {
        let completed = self.completed.read().await;
        let prev = completed.get(agent_id)
            .ok_or_else(|| SubagentError::not_found(agent_id))?;

        // Create new agent with previous context
        let mut context = prev.context.clone();
        if let Some(prompt) = additional_prompt {
            context.add_message(ConversationMessage::user(prompt));
        }

        // Generate new ID for resumed agent
        let new_id = format!("agent_{}", uuid::Uuid::new_v4());

        // Run with previous context
        // ... similar to spawn ...

        Ok(new_id)
    }
}
```

## Subagent Hooks

Subagents emit dedicated hooks for lifecycle tracking:

```rust
/// Subagent hook event types (from HookEventType)
pub enum SubagentHookEvent {
    /// Subagent spawned
    SubagentStart {
        agent_id: String,
        agent_type: String,
        prompt: String,
    },
    /// Subagent completed or stopped
    SubagentStop {
        agent_id: String,
        result: SubagentResult,
    },
}

/// Register subagent hooks
pub fn register_subagent_hooks(
    hooks: &mut HookRegistry,
    manager: &SubagentManager,
) {
    // Emit SubagentStart when spawning
    // Convert Stop hook to SubagentStop for subagents

    hooks.register(HookDefinition {
        event: HookEventType::SubagentStart,
        handler: HookHandler::Inline {
            callback: Arc::new(|ctx| {
                // Log subagent spawn
                if let Some(agent_id) = &ctx.agent_id {
                    log::info!("Subagent started: {}", agent_id);
                }
                HookResult::Continue
            }),
        },
        ..Default::default()
    });

    hooks.register(HookDefinition {
        event: HookEventType::SubagentStop,
        handler: HookHandler::Inline {
            callback: Arc::new(|ctx| {
                // Log subagent completion
                if let Some(agent_id) = &ctx.agent_id {
                    log::info!("Subagent stopped: {}", agent_id);
                }
                HookResult::Continue
            }),
        },
        ..Default::default()
    });
}

### Hook Exit Code Behavior

Subagent hooks have specific exit code semantics:

| Exit Code | SubagentStart Behavior | SubagentStop Behavior |
|-----------|------------------------|----------------------|
| 0 | stdout shown to subagent | output hidden |
| 2 | stderr shown to user, continue | stderr shown to subagent, continue |
| Other | stderr shown to user only | stderr shown to user only |

```rust
impl HookResult {
    pub fn from_exit_code(code: i32, stdout: &str, stderr: &str) -> Self {
        match code {
            0 => HookResult::Continue {
                inject_to_agent: Some(stdout.to_string())
            },
            2 => HookResult::Continue {
                inject_to_agent: Some(stderr.to_string())
            },
            _ => HookResult::Continue {
                inject_to_agent: None  // stderr shown to user via event
            },
        }
    }
}
```

impl SubagentManager {
    async fn spawn_with_hooks(
        &self,
        input: SpawnInput,
        parent_context: &ToolUseContext,
        event_tx: mpsc::Sender<LoopEvent>,
        hooks: &HookRegistry,
    ) -> Result<String, SubagentError> {
        // Execute SubagentStart hook
        let hook_ctx = HookContext {
            event: HookEventType::SubagentStart,
            agent_id: Some(input.description.clone()),
            session_id: parent_context.session_id.clone(),
            ..Default::default()
        };
        let hook_result = hooks.execute(HookEventType::SubagentStart, hook_ctx).await;

        if let HookResult::Reject { reason } = hook_result {
            return Err(SubagentError::hook_rejected(reason));
        }

        // Normal spawn logic...
        let agent_id = self.spawn(input, parent_context, event_tx).await?;

        Ok(agent_id)
    }
}
```

## Agent Progress Tracking

Track subagent progress for UI feedback:

```rust
/// Agent progress information
#[derive(Debug, Clone, Default)]
pub struct AgentProgress {
    /// Total tokens used
    pub token_count: i64,
    /// Number of tool uses
    pub tool_use_count: i32,
    /// Recent activity descriptions
    pub recent_activities: Vec<String>,
    /// Current status message
    pub status_message: Option<String>,
}

impl AgentProgress {
    /// Add activity to recent list (keeps last 5, aligned with Claude Code)
    pub fn add_activity(&mut self, activity: String) {
        self.recent_activities.push(activity);
        if self.recent_activities.len() > 5 {
            self.recent_activities.remove(0);
        }
    }

    /// Update from loop event
    pub fn update_from_event(&mut self, event: &LoopEvent) {
        match event {
            LoopEvent::StreamRequestEnd { usage, .. } => {
                self.token_count += usage.total_tokens as i64;
            }
            LoopEvent::ToolUseCompleted { call_id, .. } => {
                self.tool_use_count += 1;
                self.add_activity(format!("Completed tool: {call_id}"));
            }
            LoopEvent::TextDelta { delta, .. } => {
                // Update status with truncated text
                let preview = if delta.len() > 50 {
                    format!("{}...", &delta[..50])
                } else {
                    delta.clone()
                };
                self.status_message = Some(preview);
            }
            _ => {}
        }
    }
}

/// Progress tracking in SubagentManager
impl SubagentManager {
    /// Get progress for a running agent
    pub async fn get_progress(&self, agent_id: &str) -> Option<AgentProgress> {
        self.running.read().await
            .get(agent_id)
            .map(|a| a.progress.clone())
    }
}
```

## Background Agents

See [background.md](background.md) for detailed background mode architecture.

```rust
impl SubagentManager {
    async fn spawn_background(
        &self,
        id: String,
        def: AgentDefinition,
        model: Arc<dyn Model>,
        tools: ToolRegistry,
        context: ConversationContext,
        config: LoopConfig,
        initial_msg: ConversationMessage,
        event_tx: mpsc::Sender<LoopEvent>,
    ) {
        let cancel = CancellationToken::new();
        let running = self.running.clone();
        let completed = self.completed.clone();
        let cancel_clone = cancel.clone();

        let handle = tokio::spawn(async move {
            // Create event channel (events go to log file)
            let (local_tx, mut local_rx) = mpsc::channel(100);
            let log_file = create_agent_log_file(&id).await;

            // Spawn log writer
            tokio::spawn(async move {
                while let Some(event) = local_rx.recv().await {
                    write_event_to_log(&log_file, &event).await;
                }
            });

            // Run agent loop
            let mut loop_driver = AgentLoop::new(
                model, tools, context.clone(), config, local_tx
            );
            loop_driver.set_cancel(cancel_clone);

            let result = loop_driver.run(initial_msg).await;

            // Move to completed
            running.write().await.remove(&id);
            completed.write().await.insert(id.clone(), CompletedAgent {
                id: id.clone(),
                definition: def,
                result: result.clone().unwrap_or_default(),
                context,
                completed_at: SystemTime::now(),
            });

            result
        });

        // Track running agent
        self.running.write().await.insert(id.clone(), RunningAgent {
            id,
            definition: def,
            handle,
            cancel,
            started_at: SystemTime::now(),
        });

        // Emit backgrounded event
        event_tx.send(LoopEvent::SubagentBackgrounded {
            agent_id: id,
            output_file: log_file,
        }).await.ok();
    }

    /// Get output from background agent
    pub async fn get_output(&self, agent_id: &str) -> Result<Option<String>, SubagentError> {
        // Check if still running
        if self.running.read().await.contains_key(agent_id) {
            return Ok(None);  // Still running
        }

        // Check completed
        if let Some(completed) = self.completed.read().await.get(agent_id) {
            return Ok(Some(completed.result.final_text().to_string()));
        }

        Err(SubagentError::not_found(agent_id))
    }
}
```

## Usage Example

```rust
// In main agent, when Task tool is called
let input = SpawnInput {
    description: "Explore authentication code".to_string(),
    prompt: "Find all files related to user authentication".to_string(),
    subagent_type: "Explore".to_string(),
    model: None,  // Inherits parent's model
    resume: None,
    run_in_background: false,
    max_turns: Some(5),
    allowed_tools: None,
};

let agent_id = manager.spawn(input, &context, event_tx).await?;
// Agent runs, result returned via LoopEvent::SubagentCompleted
```

## Custom Agents from Settings

Additional agents can be configured via settings or plugins:

```toml
# ~/.config/cocode/agents.toml

[[agents]]
agent_type = "code-reviewer"
when_to_use = "Review code changes for best practices and potential issues"
tools = ["*"]
disallowed_tools = ["Task", "Edit", "Write"]
# model: omit to inherit parent's model, or specify full model ID
# model = { provider = "anthropic", model = "claude-sonnet-4-20250514" }
fork_context = true
critical_reminder = "CRITICAL: Do not modify any files. Provide feedback only."

[[agents]]
agent_type = "test-runner"
when_to_use = "Run tests and report results"
tools = ["Bash", "Read", "Glob"]
# model: omit to inherit parent's model
```

## Summary: Tool Configuration Patterns

| Pattern | Usage | Example |
|---------|-------|---------|
| `tools: Some(vec!["*"])` | All tools | general-purpose agent |
| `tools: Some(vec!["A", "B"])` | Only A and B | Plan agent (Glob, Grep, Read, Bash) |
| `disallowed_tools: Some(vec!["X"])` | Exclude X | Explore (excludes Edit, Write) |
| Combined | Allow some, exclude others | tools=["*"], disallowed_tools=["Task"] |

**Three-layer filtering (in order):**
1. System-wide exclusions (Task, EnterPlanMode, ExitPlanMode)
2. Agent-specific `disallowed_tools`
3. Agent-specific `tools` allow-list
