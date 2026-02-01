//! MCP (Model Context Protocol) server configuration types.
//!
//! These types define how plugins can contribute MCP servers. The actual
//! MCP client integration is deferred to the MCP client implementation.

use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;

/// Default function for auto_start field.
fn default_true() -> bool {
    true
}

/// Configuration for an MCP server contributed by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Unique name for this MCP server.
    pub name: String,

    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,

    /// Transport configuration.
    pub transport: McpTransport,

    /// Environment variables to set when starting the server.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Whether to automatically start this server.
    #[serde(default = "default_true")]
    pub auto_start: bool,
}

/// Transport configuration for MCP servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpTransport {
    /// Standard input/output transport (subprocess).
    Stdio {
        /// Command to execute.
        command: String,
        /// Command arguments.
        #[serde(default)]
        args: Vec<String>,
    },

    /// HTTP transport.
    Http {
        /// Server URL.
        url: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_stdio() {
        let config = McpServerConfig {
            name: "file-server".to_string(),
            description: Some("File system MCP server".to_string()),
            transport: McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["-y".to_string(), "@anthropic/file-server".to_string()],
            },
            env: HashMap::new(),
            auto_start: true,
        };

        assert_eq!(config.name, "file-server");
        assert!(config.auto_start);

        if let McpTransport::Stdio { command, args } = &config.transport {
            assert_eq!(command, "npx");
            assert_eq!(args.len(), 2);
        } else {
            panic!("Expected Stdio transport");
        }
    }

    #[test]
    fn test_mcp_server_http() {
        let config = McpServerConfig {
            name: "remote-server".to_string(),
            description: None,
            transport: McpTransport::Http {
                url: "http://localhost:3000".to_string(),
            },
            env: HashMap::new(),
            auto_start: false,
        };

        if let McpTransport::Http { url } = &config.transport {
            assert_eq!(url, "http://localhost:3000");
        } else {
            panic!("Expected Http transport");
        }
    }

    #[test]
    fn test_mcp_server_serialize_deserialize() {
        let config = McpServerConfig {
            name: "test-server".to_string(),
            description: Some("A test server".to_string()),
            transport: McpTransport::Stdio {
                command: "node".to_string(),
                args: vec!["server.js".to_string()],
            },
            env: {
                let mut map = HashMap::new();
                map.insert("NODE_ENV".to_string(), "production".to_string());
                map
            },
            auto_start: true,
        };

        let toml_str = toml::to_string(&config).expect("serialize");
        let back: McpServerConfig = toml::from_str(&toml_str).expect("deserialize");

        assert_eq!(back.name, "test-server");
        assert_eq!(back.env.get("NODE_ENV"), Some(&"production".to_string()));
    }

    #[test]
    fn test_mcp_server_from_toml() {
        let toml_str = r#"
name = "filesystem"
description = "Provides file system access"
auto_start = true

[transport]
type = "stdio"
command = "npx"
args = ["-y", "@anthropic/mcp-server-filesystem"]

[env]
MCP_DEBUG = "true"
"#;

        let config: McpServerConfig = toml::from_str(toml_str).expect("deserialize");
        assert_eq!(config.name, "filesystem");
        assert_eq!(
            config.description,
            Some("Provides file system access".to_string())
        );
        assert!(config.auto_start);
        assert_eq!(config.env.get("MCP_DEBUG"), Some(&"true".to_string()));

        if let McpTransport::Stdio { command, args } = &config.transport {
            assert_eq!(command, "npx");
            assert_eq!(args.len(), 2);
        } else {
            panic!("Expected Stdio transport");
        }
    }

    #[test]
    fn test_mcp_server_defaults() {
        let toml_str = r#"
name = "minimal"

[transport]
type = "http"
url = "http://localhost:8080"
"#;

        let config: McpServerConfig = toml::from_str(toml_str).expect("deserialize");
        assert_eq!(config.name, "minimal");
        assert!(config.description.is_none());
        assert!(config.auto_start); // Default is true
        assert!(config.env.is_empty());
    }
}
