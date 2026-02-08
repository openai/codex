//! Aggregate conversation context.
//!
//! Combines environment info, budget, tool state, memory, and configuration
//! into a single context value used by the prompt builder.

use cocode_protocol::CompactConfig;
use cocode_protocol::PermissionMode;
use cocode_protocol::SessionMemoryConfig;
use cocode_protocol::ThinkingLevel;
use serde::Deserialize;
use serde::Serialize;

use crate::budget::ContextBudget;
use crate::environment::EnvironmentInfo;

/// A memory file loaded into context (CLAUDE.md, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFile {
    /// File path (relative or display name).
    pub path: String,
    /// File content.
    pub content: String,
    /// Priority for ordering (lower = higher priority).
    pub priority: i32,
}

/// Content injected into the system prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextInjection {
    /// Label for this injection.
    pub label: String,
    /// Content to inject.
    pub content: String,
    /// Where to inject this content.
    pub position: InjectionPosition,
}

/// Position for injected content in the system prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InjectionPosition {
    /// Before tool definitions section.
    BeforeTools,
    /// After tool definitions section.
    AfterTools,
    /// At the end of the prompt.
    EndOfPrompt,
}

/// Type of subagent for specialized prompt generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentType {
    /// Codebase exploration subagent.
    Explore,
    /// Implementation planning subagent.
    Plan,
}

impl SubagentType {
    /// Get the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            SubagentType::Explore => "explore",
            SubagentType::Plan => "plan",
        }
    }
}

impl std::fmt::Display for SubagentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Aggregate conversation context for prompt generation.
#[derive(Debug, Clone)]
pub struct ConversationContext {
    /// Runtime environment information.
    pub environment: EnvironmentInfo,
    /// Token budget tracker.
    pub budget: ContextBudget,
    /// Available tool names.
    pub tool_names: Vec<String>,
    /// Connected MCP server names.
    pub mcp_server_names: Vec<String>,
    /// Loaded memory files.
    pub memory_files: Vec<MemoryFile>,
    /// Prompt injections.
    pub injections: Vec<ContextInjection>,
    /// Permission mode for the session.
    pub permission_mode: PermissionMode,
    /// Thinking/reasoning configuration.
    pub thinking_level: Option<ThinkingLevel>,
    /// Compaction configuration.
    pub compact_config: CompactConfig,
    /// Session memory configuration.
    pub session_memory_config: SessionMemoryConfig,
    /// Subagent type (if this is a subagent).
    pub subagent_type: Option<SubagentType>,
    /// Path to the conversation transcript file.
    pub transcript_path: Option<std::path::PathBuf>,
}

impl ConversationContext {
    /// Create a builder for constructing conversation context.
    pub fn builder() -> ConversationContextBuilder {
        ConversationContextBuilder::default()
    }

    /// Check if any MCP servers are connected.
    pub fn has_mcp_servers(&self) -> bool {
        !self.mcp_server_names.is_empty()
    }

    /// Check if any tools are available.
    pub fn has_tools(&self) -> bool {
        !self.tool_names.is_empty()
    }

    /// Check if this is a subagent context.
    pub fn is_subagent(&self) -> bool {
        self.subagent_type.is_some()
    }
}

/// Builder for [`ConversationContext`].
#[derive(Debug, Default)]
pub struct ConversationContextBuilder {
    environment: Option<EnvironmentInfo>,
    budget: Option<ContextBudget>,
    tool_names: Vec<String>,
    mcp_server_names: Vec<String>,
    memory_files: Vec<MemoryFile>,
    injections: Vec<ContextInjection>,
    permission_mode: PermissionMode,
    thinking_level: Option<ThinkingLevel>,
    compact_config: CompactConfig,
    session_memory_config: SessionMemoryConfig,
    subagent_type: Option<SubagentType>,
    transcript_path: Option<std::path::PathBuf>,
}

impl ConversationContextBuilder {
    pub fn environment(mut self, env: EnvironmentInfo) -> Self {
        self.environment = Some(env);
        self
    }

    pub fn budget(mut self, budget: ContextBudget) -> Self {
        self.budget = Some(budget);
        self
    }

    pub fn tool_names(mut self, names: Vec<String>) -> Self {
        self.tool_names = names;
        self
    }

    pub fn mcp_server_names(mut self, names: Vec<String>) -> Self {
        self.mcp_server_names = names;
        self
    }

    pub fn memory_files(mut self, files: Vec<MemoryFile>) -> Self {
        self.memory_files = files;
        self
    }

    pub fn injections(mut self, injections: Vec<ContextInjection>) -> Self {
        self.injections = injections;
        self
    }

    pub fn permission_mode(mut self, mode: PermissionMode) -> Self {
        self.permission_mode = mode;
        self
    }

    pub fn thinking_level(mut self, config: ThinkingLevel) -> Self {
        self.thinking_level = Some(config);
        self
    }

    pub fn compact_config(mut self, config: CompactConfig) -> Self {
        self.compact_config = config;
        self
    }

    pub fn session_memory_config(mut self, config: SessionMemoryConfig) -> Self {
        self.session_memory_config = config;
        self
    }

    pub fn subagent_type(mut self, agent_type: SubagentType) -> Self {
        self.subagent_type = Some(agent_type);
        self
    }

    pub fn transcript_path(mut self, path: std::path::PathBuf) -> Self {
        self.transcript_path = Some(path);
        self
    }

    /// Build the [`ConversationContext`].
    ///
    /// Returns `Err` if required fields are missing.
    pub fn build(self) -> crate::error::Result<ConversationContext> {
        let environment = self.environment.ok_or_else(|| {
            crate::error::context_error::BuildSnafu {
                message: "environment is required",
            }
            .build()
        })?;

        let budget = self.budget.unwrap_or_else(|| {
            ContextBudget::new(environment.context_window, environment.max_output_tokens)
        });

        Ok(ConversationContext {
            environment,
            budget,
            tool_names: self.tool_names,
            mcp_server_names: self.mcp_server_names,
            memory_files: self.memory_files,
            injections: self.injections,
            permission_mode: self.permission_mode,
            thinking_level: self.thinking_level,
            compact_config: self.compact_config,
            session_memory_config: self.session_memory_config,
            subagent_type: self.subagent_type,
            transcript_path: self.transcript_path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_env() -> EnvironmentInfo {
        EnvironmentInfo::builder()
            .cwd("/tmp/test")
            .model("test-model")
            .context_window(200000)
            .max_output_tokens(16384)
            .build()
            .unwrap()
    }

    #[test]
    fn test_builder_minimal() {
        let ctx = ConversationContext::builder()
            .environment(test_env())
            .build()
            .unwrap();

        assert_eq!(ctx.environment.model, "test-model");
        assert!(!ctx.has_tools());
        assert!(!ctx.has_mcp_servers());
        assert!(!ctx.is_subagent());
        assert_eq!(ctx.permission_mode, PermissionMode::Default);
    }

    #[test]
    fn test_builder_full() {
        let ctx = ConversationContext::builder()
            .environment(test_env())
            .tool_names(vec!["Read".to_string(), "Write".to_string()])
            .mcp_server_names(vec!["github".to_string()])
            .memory_files(vec![MemoryFile {
                path: "CLAUDE.md".to_string(),
                content: "instructions".to_string(),
                priority: 0,
            }])
            .permission_mode(PermissionMode::AcceptEdits)
            .subagent_type(SubagentType::Explore)
            .build()
            .unwrap();

        assert!(ctx.has_tools());
        assert!(ctx.has_mcp_servers());
        assert!(ctx.is_subagent());
        assert_eq!(ctx.subagent_type, Some(SubagentType::Explore));
        assert_eq!(ctx.permission_mode, PermissionMode::AcceptEdits);
        assert_eq!(ctx.memory_files.len(), 1);
    }

    #[test]
    fn test_builder_missing_environment() {
        let result = ConversationContext::builder().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_subagent_type_display() {
        assert_eq!(SubagentType::Explore.to_string(), "explore");
        assert_eq!(SubagentType::Plan.to_string(), "plan");
    }

    #[test]
    fn test_injection_position_serde() {
        let json = r#""before_tools""#;
        let pos: InjectionPosition = serde_json::from_str(json).unwrap();
        assert_eq!(pos, InjectionPosition::BeforeTools);
    }
}
