//! Tool execution configuration.
//!
//! Defines settings for tool execution concurrency and timeouts.

use serde::{Deserialize, Serialize};

/// Default maximum number of concurrent tool executions.
pub const DEFAULT_MAX_TOOL_CONCURRENCY: i32 = 10;

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
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            max_tool_concurrency: DEFAULT_MAX_TOOL_CONCURRENCY,
            mcp_tool_timeout: None,
        }
    }
}

fn default_max_tool_concurrency() -> i32 {
    DEFAULT_MAX_TOOL_CONCURRENCY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_config_default() {
        let config = ToolConfig::default();
        assert_eq!(config.max_tool_concurrency, DEFAULT_MAX_TOOL_CONCURRENCY);
        assert!(config.mcp_tool_timeout.is_none());
    }

    #[test]
    fn test_tool_config_serde() {
        let json = r#"{"max_tool_concurrency": 5, "mcp_tool_timeout": 30000}"#;
        let config: ToolConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.max_tool_concurrency, 5);
        assert_eq!(config.mcp_tool_timeout, Some(30000));
    }

    #[test]
    fn test_tool_config_serde_defaults() {
        let json = r#"{}"#;
        let config: ToolConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.max_tool_concurrency, DEFAULT_MAX_TOOL_CONCURRENCY);
        assert!(config.mcp_tool_timeout.is_none());
    }
}
