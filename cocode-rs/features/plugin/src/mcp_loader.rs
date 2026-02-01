//! MCP server loading from plugin directories.
//!
//! Loads MCP.toml files from plugin-specified MCP server directories.

use std::path::Path;

use tracing::debug;
use tracing::warn;
use walkdir::WalkDir;

use crate::contribution::PluginContribution;
use crate::mcp::McpServerConfig;

/// MCP server manifest filename.
pub const MCP_TOML: &str = "MCP.toml";

/// Load MCP server configurations from a directory.
///
/// Scans the directory for MCP.toml files and loads them into
/// PluginContribution::McpServer variants.
///
/// # Arguments
/// * `dir` - Directory to scan for MCP.toml files
/// * `plugin_name` - Name of the plugin providing these servers
///
/// # Example MCP.toml format:
/// ```toml
/// name = "filesystem"
/// description = "Provides file system access"
/// auto_start = true
///
/// [transport]
/// type = "stdio"
/// command = "npx"
/// args = ["-y", "@anthropic/mcp-server-filesystem"]
///
/// [env]
/// MCP_DEBUG = "true"
/// ```
pub fn load_mcp_servers_from_dir(dir: &Path, plugin_name: &str) -> Vec<PluginContribution> {
    if !dir.is_dir() {
        debug!(
            plugin = %plugin_name,
            path = %dir.display(),
            "MCP server path not found or not a directory"
        );
        return Vec::new();
    }

    let mut results = Vec::new();

    // Walk the directory looking for MCP.toml files
    for entry in WalkDir::new(dir)
        .max_depth(3)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_dir() {
            let mcp_path = entry.path().join(MCP_TOML);
            if mcp_path.is_file() {
                match load_mcp_server_from_file(&mcp_path, plugin_name) {
                    Ok(contrib) => results.push(contrib),
                    Err(e) => {
                        warn!(
                            plugin = %plugin_name,
                            path = %mcp_path.display(),
                            error = %e,
                            "Failed to load MCP server configuration"
                        );
                    }
                }
            }
        }
    }

    debug!(
        plugin = %plugin_name,
        path = %dir.display(),
        count = results.len(),
        "Loaded MCP servers from plugin"
    );

    results
}

/// Load a single MCP server configuration from a TOML file.
fn load_mcp_server_from_file(path: &Path, plugin_name: &str) -> anyhow::Result<PluginContribution> {
    let content = std::fs::read_to_string(path)?;
    let config: McpServerConfig = toml::from_str(&content)?;

    debug!(
        plugin = %plugin_name,
        server = %config.name,
        "Loaded MCP server configuration"
    );

    Ok(PluginContribution::McpServer {
        config,
        plugin_name: plugin_name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::McpTransport;
    use std::fs;

    #[test]
    fn test_load_mcp_servers_from_empty_dir() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let results = load_mcp_servers_from_dir(tmp.path(), "test-plugin");
        assert!(results.is_empty());
    }

    #[test]
    fn test_load_mcp_servers_from_nonexistent_dir() {
        let results = load_mcp_servers_from_dir(Path::new("/nonexistent"), "test-plugin");
        assert!(results.is_empty());
    }

    #[test]
    fn test_load_mcp_server_stdio() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let server_dir = tmp.path().join("filesystem");
        fs::create_dir_all(&server_dir).expect("mkdir");

        fs::write(
            server_dir.join("MCP.toml"),
            r#"
name = "filesystem"
description = "File system access"
auto_start = true

[transport]
type = "stdio"
command = "npx"
args = ["-y", "@anthropic/mcp-server-filesystem"]

[env]
MCP_DEBUG = "true"
"#,
        )
        .expect("write");

        let results = load_mcp_servers_from_dir(tmp.path(), "test-plugin");
        assert_eq!(results.len(), 1);

        if let PluginContribution::McpServer {
            config,
            plugin_name,
        } = &results[0]
        {
            assert_eq!(config.name, "filesystem");
            assert_eq!(config.description, Some("File system access".to_string()));
            assert!(config.auto_start);
            assert_eq!(plugin_name, "test-plugin");

            if let McpTransport::Stdio { command, args } = &config.transport {
                assert_eq!(command, "npx");
                assert_eq!(args.len(), 2);
            } else {
                panic!("Expected Stdio transport");
            }
        } else {
            panic!("Expected McpServer contribution");
        }
    }

    #[test]
    fn test_load_mcp_server_http() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let server_dir = tmp.path().join("remote");
        fs::create_dir_all(&server_dir).expect("mkdir");

        fs::write(
            server_dir.join("MCP.toml"),
            r#"
name = "remote-server"
auto_start = false

[transport]
type = "http"
url = "http://localhost:3000"
"#,
        )
        .expect("write");

        let results = load_mcp_servers_from_dir(tmp.path(), "test-plugin");
        assert_eq!(results.len(), 1);

        if let PluginContribution::McpServer { config, .. } = &results[0] {
            assert_eq!(config.name, "remote-server");
            assert!(!config.auto_start);

            if let McpTransport::Http { url } = &config.transport {
                assert_eq!(url, "http://localhost:3000");
            } else {
                panic!("Expected Http transport");
            }
        } else {
            panic!("Expected McpServer contribution");
        }
    }

    #[test]
    fn test_load_mcp_server_invalid_toml() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let server_dir = tmp.path().join("invalid");
        fs::create_dir_all(&server_dir).expect("mkdir");

        fs::write(server_dir.join("MCP.toml"), "invalid { toml").expect("write");

        let results = load_mcp_servers_from_dir(tmp.path(), "test-plugin");
        assert!(results.is_empty()); // Invalid TOML should be skipped
    }

    #[test]
    fn test_load_multiple_mcp_servers() {
        let tmp = tempfile::tempdir().expect("create temp dir");

        // Server 1
        let server1 = tmp.path().join("server1");
        fs::create_dir_all(&server1).expect("mkdir");
        fs::write(
            server1.join("MCP.toml"),
            r#"
name = "server1"

[transport]
type = "http"
url = "http://localhost:3001"
"#,
        )
        .expect("write");

        // Server 2
        let server2 = tmp.path().join("server2");
        fs::create_dir_all(&server2).expect("mkdir");
        fs::write(
            server2.join("MCP.toml"),
            r#"
name = "server2"

[transport]
type = "http"
url = "http://localhost:3002"
"#,
        )
        .expect("write");

        let results = load_mcp_servers_from_dir(tmp.path(), "test-plugin");
        assert_eq!(results.len(), 2);

        let names: Vec<&str> = results
            .iter()
            .filter_map(|c| {
                if let PluginContribution::McpServer { config, .. } = c {
                    Some(config.name.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert!(names.contains(&"server1"));
        assert!(names.contains(&"server2"));
    }
}
