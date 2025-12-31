//! Agent definition types (Claude Code compatible).

mod builtin;
mod parser;

pub use builtin::get_builtin_agents;
pub use parser::parse_agent_definition;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Agent definition - compatible with Claude Code YAML format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDefinition {
    /// Unique agent type identifier (e.g., "Explore", "Plan").
    pub agent_type: String,

    /// Human-readable display name.
    #[serde(default)]
    pub display_name: Option<String>,

    /// Description of when to use this agent.
    #[serde(default)]
    pub when_to_use: Option<String>,

    /// Tool access configuration.
    #[serde(default)]
    pub tools: ToolAccess,

    /// Tools explicitly blocked for this agent.
    #[serde(default)]
    pub disallowed_tools: Vec<String>,

    /// Source of this agent definition.
    #[serde(default)]
    pub source: AgentSource,

    /// Model configuration.
    #[serde(default)]
    pub model_config: ModelConfig,

    /// Whether to fork parent conversation context.
    #[serde(default)]
    pub fork_context: bool,

    /// Prompt configuration.
    #[serde(default)]
    pub prompt_config: PromptConfig,

    /// Run configuration (limits).
    #[serde(default)]
    pub run_config: AgentRunConfig,

    /// Typed input parameters.
    #[serde(default)]
    pub input_config: Option<InputConfig>,

    /// Structured output configuration.
    #[serde(default)]
    pub output_config: Option<OutputConfig>,

    /// Approval mode for this agent.
    #[serde(default)]
    pub approval_mode: ApprovalMode,

    /// Critical system reminder (extra safety for read-only agents).
    #[serde(default)]
    pub critical_system_reminder: Option<String>,
}

impl AgentDefinition {
    /// Check if this is a built-in agent.
    pub fn is_builtin(&self) -> bool {
        matches!(self.source, AgentSource::Builtin)
    }
}

/// Tool access configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum ToolAccess {
    /// All tools (represented as "*").
    #[default]
    All,
    /// Specific list of allowed tools.
    List(Vec<String>),
}

impl ToolAccess {
    /// Check if a tool is allowed.
    pub fn allows(&self, tool_name: &str) -> bool {
        match self {
            ToolAccess::All => true,
            ToolAccess::List(tools) => tools.iter().any(|t| t == tool_name),
        }
    }
}

/// Source of agent definition.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentSource {
    /// Built-in agent (Explore, Plan).
    #[default]
    Builtin,
    /// User-defined agent.
    User,
    /// Project-level agent.
    Project,
}

/// The level of thinking tokens that the model should generate.
/// Compatible with google-genai's ThinkingLevel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThinkingLevel {
    #[default]
    ThinkingLevelUnspecified,
    Low,
    High,
}

/// Model configuration for agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfig {
    /// Provider name - references config.model_providers HashMap key.
    /// If None, inherits parent session's provider.
    #[serde(default)]
    pub provider: Option<String>,

    /// Model name override (optional).
    /// If None, uses the provider's default model.
    #[serde(default)]
    pub model: Option<String>,

    /// Temperature for sampling.
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Top-p for sampling.
    #[serde(default = "default_top_p")]
    pub top_p: f32,

    /// Thinking level (Low/High).
    #[serde(default)]
    pub thinking_level: Option<ThinkingLevel>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            provider: None,
            model: None,
            temperature: default_temperature(),
            top_p: default_top_p(),
            thinking_level: None,
        }
    }
}

fn default_temperature() -> f32 {
    0.7
}

fn default_top_p() -> f32 {
    0.95
}

/// Prompt configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PromptConfig {
    /// System prompt template (supports ${variable} substitution).
    #[serde(default)]
    pub system_prompt: Option<String>,

    /// Query template (the actual task).
    #[serde(default)]
    pub query: Option<String>,
}

/// Run configuration with limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunConfig {
    /// Maximum execution time in seconds.
    #[serde(default = "default_max_time")]
    pub max_time_seconds: i32,

    /// Maximum conversation turns.
    #[serde(default = "default_max_turns")]
    pub max_turns: i32,

    /// Grace period for recovery after timeout/max_turns.
    #[serde(default = "default_grace_period")]
    pub grace_period_seconds: i32,
}

impl Default for AgentRunConfig {
    fn default() -> Self {
        Self {
            max_time_seconds: default_max_time(),
            max_turns: default_max_turns(),
            grace_period_seconds: default_grace_period(),
        }
    }
}

fn default_max_time() -> i32 {
    300
}

fn default_max_turns() -> i32 {
    50
}

fn default_grace_period() -> i32 {
    60
}

/// Typed input parameters configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InputConfig {
    /// Map of parameter name to definition.
    #[serde(default)]
    pub inputs: HashMap<String, InputDefinition>,
}

impl InputConfig {
    /// Convert to JSON Schema format for tool parameter generation.
    pub fn to_json_schema(&self) -> JsonValue {
        use serde_json::json;

        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for (name, def) in &self.inputs {
            let schema = match def.input_type {
                InputType::String => json!({
                    "type": "string",
                    "description": def.description
                }),
                InputType::Number => json!({
                    "type": "number",
                    "description": def.description
                }),
                InputType::Boolean => json!({
                    "type": "boolean",
                    "description": def.description
                }),
                InputType::Integer => json!({
                    "type": "integer",
                    "description": def.description
                }),
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

/// Definition of a single input parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDefinition {
    /// Description of this input.
    pub description: String,

    /// Type of this input.
    #[serde(rename = "type")]
    pub input_type: InputType,

    /// Whether this input is required.
    #[serde(default)]
    pub required: bool,
}

/// Supported input types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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

/// Structured output configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputConfig {
    /// Name of the output parameter in complete_task.
    pub output_name: String,

    /// Description of the output.
    pub description: String,

    /// JSON Schema for validation.
    pub schema: JsonValue,
}

/// Approval mode for subagent tool execution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalMode {
    /// Route approvals to parent session.
    #[default]
    RouteToParent,
    /// Automatically approve (for read-only agents).
    AutoApprove,
    /// Never ask for approval (permission mode: dontAsk).
    DontAsk,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_access_all() {
        let access = ToolAccess::All;
        assert!(access.allows("read_file"));
        assert!(access.allows("anything"));
    }

    #[test]
    fn test_tool_access_list() {
        let access = ToolAccess::List(vec!["read_file".to_string(), "glob".to_string()]);
        assert!(access.allows("read_file"));
        assert!(access.allows("glob"));
        assert!(!access.allows("shell"));
    }

    #[test]
    fn test_default_run_config() {
        let config = AgentRunConfig::default();
        assert_eq!(config.max_time_seconds, 300);
        assert_eq!(config.max_turns, 50);
        assert_eq!(config.grace_period_seconds, 60);
    }

    #[test]
    fn test_agent_source() {
        let builtin = AgentSource::Builtin;
        let user = AgentSource::User;
        assert_ne!(builtin, user);
    }

    #[test]
    fn test_input_config_to_json_schema() {
        let mut inputs = HashMap::new();
        inputs.insert(
            "query".to_string(),
            InputDefinition {
                description: "Search query".to_string(),
                input_type: InputType::String,
                required: true,
            },
        );
        inputs.insert(
            "limit".to_string(),
            InputDefinition {
                description: "Max results".to_string(),
                input_type: InputType::Integer,
                required: false,
            },
        );

        let config = InputConfig { inputs };
        let schema = config.to_json_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"]["type"] == "string");
        assert!(schema["properties"]["limit"]["type"] == "integer");
        // Required should contain "query" only
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "query"));
        assert!(!required.iter().any(|v| v == "limit"));
    }
}
