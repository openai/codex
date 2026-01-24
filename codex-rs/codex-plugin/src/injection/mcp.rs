//! MCP server injection.

use crate::error::Result;
use crate::loader::PluginMcpServer;
use std::collections::HashMap;

/// Injected MCP server ready for Config merging.
#[derive(Debug, Clone)]
pub struct InjectedMcpServer {
    /// Server name.
    pub name: String,
    /// Server command.
    pub command: Option<String>,
    /// Command arguments.
    pub args: Vec<String>,
    /// Environment variables.
    pub env: HashMap<String, String>,
    /// Server URL (for remote servers).
    pub url: Option<String>,
    /// Server type (stdio, sse, http).
    pub server_type: McpServerType,
    /// Source plugin ID.
    pub source_plugin: String,
}

/// MCP server type.
#[derive(Debug, Clone, Default)]
pub enum McpServerType {
    /// Standard I/O (default).
    #[default]
    Stdio,
    /// Server-sent events.
    Sse,
    /// HTTP transport.
    Http,
}

/// Convert a plugin MCP server to injectable format.
pub fn convert_mcp_server(server: &PluginMcpServer) -> Result<(String, InjectedMcpServer)> {
    // Determine server type
    let server_type = if server.url.is_some() {
        McpServerType::Sse
    } else {
        McpServerType::Stdio
    };

    Ok((
        server.name.clone(),
        InjectedMcpServer {
            name: server.name.clone(),
            command: server.command.clone(),
            args: server.args.clone(),
            env: server.env.clone(),
            url: server.url.clone(),
            server_type,
            source_plugin: server.source_plugin.clone(),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_mcp_server_stdio() {
        let plugin_server = PluginMcpServer {
            name: "test-server".to_string(),
            command: Some("npx".to_string()),
            args: vec!["-y".to_string(), "mcp-server".to_string()],
            env: HashMap::new(),
            url: None,
            source_plugin: "test-plugin".to_string(),
        };

        let (name, injected) = convert_mcp_server(&plugin_server).unwrap();
        assert_eq!(name, "test-server");
        assert!(matches!(injected.server_type, McpServerType::Stdio));
        assert_eq!(injected.command, Some("npx".to_string()));
    }

    #[test]
    fn test_convert_mcp_server_sse() {
        let plugin_server = PluginMcpServer {
            name: "remote-server".to_string(),
            command: None,
            args: vec![],
            env: HashMap::new(),
            url: Some("https://mcp.example.com".to_string()),
            source_plugin: "test-plugin".to_string(),
        };

        let (_, injected) = convert_mcp_server(&plugin_server).unwrap();
        assert!(matches!(injected.server_type, McpServerType::Sse));
        assert_eq!(injected.url, Some("https://mcp.example.com".to_string()));
    }
}
