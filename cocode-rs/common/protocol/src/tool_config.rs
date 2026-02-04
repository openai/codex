//! Tool execution configuration.
//!
//! Defines settings for tool execution concurrency and timeouts.

use serde::Deserialize;
use serde::Serialize;

/// Type of apply_patch tool to use.
///
/// This determines how the apply_patch tool is exposed to the model:
/// - `Function`: JSON function tool (default) - model provides structured JSON input
/// - `Freeform`: Grammar-based freeform tool - model outputs patch text directly
///
/// Freeform mode is designed for GPT-5 which has native support for the apply_patch grammar.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApplyPatchToolType {
    /// JSON function tool (default).
    #[default]
    Function,
    /// Freeform grammar tool (for GPT-5).
    Freeform,
}

/// Default maximum number of concurrent tool executions.
pub const DEFAULT_MAX_TOOL_CONCURRENCY: i32 = 10;

/// Default maximum tool result size before persistence to disk (400K characters).
///
/// Results larger than this threshold are saved to disk with only a preview
/// kept in the conversation context, significantly reducing token usage.
pub const DEFAULT_MAX_RESULT_SIZE: i32 = 400_000;

/// Default preview size for persisted large results (2K characters).
///
/// When a result exceeds `DEFAULT_MAX_RESULT_SIZE`, this many characters
/// from the start of the result are kept as a preview in the context.
pub const DEFAULT_RESULT_PREVIEW_SIZE: i32 = 2_000;

/// Tool execution configuration.
///
/// Controls how tools are executed, including concurrency limits and timeouts.
///
/// # Environment Variables
///
/// - `COCODE_MAX_TOOL_USE_CONCURRENCY`: Maximum concurrent tool executions (default: 10)
/// - `MCP_TOOL_TIMEOUT`: Timeout in milliseconds for MCP tool calls
///
/// # Example
///
/// ```json
/// {
///   "tool": {
///     "max_tool_concurrency": 5,
///     "mcp_tool_timeout": 30000
///   }
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ToolConfig {
    /// Maximum number of concurrent tool executions.
    #[serde(default = "default_max_tool_concurrency")]
    pub max_tool_concurrency: i32,

    /// Timeout in milliseconds for MCP tool calls.
    #[serde(default)]
    pub mcp_tool_timeout: Option<i32>,

    /// Maximum tool result size before persistence to disk.
    ///
    /// Results larger than this threshold are saved to disk with only a preview
    /// kept in the conversation context. This saves significant context tokens
    /// when tools return very large outputs.
    #[serde(default = "default_max_result_size")]
    pub max_result_size: i32,

    /// Preview size for persisted large results.
    ///
    /// When a result exceeds `max_result_size`, this many characters
    /// from the start are kept as a preview in the context.
    #[serde(default = "default_result_preview_size")]
    pub result_preview_size: i32,

    /// Enable large result persistence (default: true).
    ///
    /// When enabled, tool results exceeding `max_result_size` are automatically
    /// persisted to disk with a preview kept in context.
    #[serde(default = "default_true")]
    pub enable_result_persistence: bool,

    /// Type of apply_patch tool to use, if enabled.
    ///
    /// - `None`: Disabled - use Edit tool instead (default)
    /// - `Some(Function)`: JSON function tool
    /// - `Some(Freeform)`: Grammar-based freeform tool (for GPT-5)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_patch_tool_type: Option<ApplyPatchToolType>,
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            max_tool_concurrency: DEFAULT_MAX_TOOL_CONCURRENCY,
            mcp_tool_timeout: None,
            max_result_size: DEFAULT_MAX_RESULT_SIZE,
            result_preview_size: DEFAULT_RESULT_PREVIEW_SIZE,
            enable_result_persistence: true,
            apply_patch_tool_type: None,
        }
    }
}

fn default_max_tool_concurrency() -> i32 {
    DEFAULT_MAX_TOOL_CONCURRENCY
}

fn default_max_result_size() -> i32 {
    DEFAULT_MAX_RESULT_SIZE
}

fn default_result_preview_size() -> i32 {
    DEFAULT_RESULT_PREVIEW_SIZE
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_config_default() {
        let config = ToolConfig::default();
        assert_eq!(config.max_tool_concurrency, DEFAULT_MAX_TOOL_CONCURRENCY);
        assert!(config.mcp_tool_timeout.is_none());
        assert_eq!(config.max_result_size, DEFAULT_MAX_RESULT_SIZE);
        assert_eq!(config.result_preview_size, DEFAULT_RESULT_PREVIEW_SIZE);
        assert!(config.enable_result_persistence);
    }

    #[test]
    fn test_tool_config_serde() {
        let json = r#"{"max_tool_concurrency": 5, "mcp_tool_timeout": 30000}"#;
        let config: ToolConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.max_tool_concurrency, 5);
        assert_eq!(config.mcp_tool_timeout, Some(30000));
        // Defaults should apply for unspecified fields
        assert_eq!(config.max_result_size, DEFAULT_MAX_RESULT_SIZE);
        assert_eq!(config.result_preview_size, DEFAULT_RESULT_PREVIEW_SIZE);
        assert!(config.enable_result_persistence);
    }

    #[test]
    fn test_tool_config_serde_defaults() {
        let json = r#"{}"#;
        let config: ToolConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.max_tool_concurrency, DEFAULT_MAX_TOOL_CONCURRENCY);
        assert!(config.mcp_tool_timeout.is_none());
        assert_eq!(config.max_result_size, DEFAULT_MAX_RESULT_SIZE);
        assert_eq!(config.result_preview_size, DEFAULT_RESULT_PREVIEW_SIZE);
        assert!(config.enable_result_persistence);
    }

    #[test]
    fn test_tool_config_persistence_disabled() {
        let json = r#"{"enable_result_persistence": false}"#;
        let config: ToolConfig = serde_json::from_str(json).unwrap();
        assert!(!config.enable_result_persistence);
    }

    #[test]
    fn test_tool_config_custom_sizes() {
        let json = r#"{"max_result_size": 200000, "result_preview_size": 1000}"#;
        let config: ToolConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.max_result_size, 200_000);
        assert_eq!(config.result_preview_size, 1_000);
    }

    /// Verify constants match Claude Code v2.1.7 alignment.
    #[test]
    fn test_claude_code_v217_alignment() {
        assert_eq!(DEFAULT_MAX_RESULT_SIZE, 400_000);
        assert_eq!(DEFAULT_RESULT_PREVIEW_SIZE, 2_000);
        assert_eq!(DEFAULT_MAX_TOOL_CONCURRENCY, 10);
    }

    #[test]
    fn test_apply_patch_tool_type_serde() {
        // Default is None (disabled)
        let json = r#"{}"#;
        let config: ToolConfig = serde_json::from_str(json).unwrap();
        assert!(config.apply_patch_tool_type.is_none());

        // Function mode
        let json = r#"{"apply_patch_tool_type": "function"}"#;
        let config: ToolConfig = serde_json::from_str(json).unwrap();
        assert_eq!(
            config.apply_patch_tool_type,
            Some(ApplyPatchToolType::Function)
        );

        // Freeform mode
        let json = r#"{"apply_patch_tool_type": "freeform"}"#;
        let config: ToolConfig = serde_json::from_str(json).unwrap();
        assert_eq!(
            config.apply_patch_tool_type,
            Some(ApplyPatchToolType::Freeform)
        );
    }

    #[test]
    fn test_apply_patch_tool_type_default() {
        assert_eq!(ApplyPatchToolType::default(), ApplyPatchToolType::Function);
    }
}
