//! Codex plugin manifest schema.

use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

/// Plugin manifest - validates .codex-plugin/plugin.json.
///
/// This schema is compatible with Claude Code plugin manifests.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    /// Plugin name (required, kebab-case, no spaces).
    pub name: String,

    /// Semantic version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Brief description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Author information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<AuthorInfo>,

    /// Plugin homepage URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,

    /// Repository URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,

    /// SPDX license identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    /// Keywords for discovery.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,

    /// Hook definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HooksConfig>,

    /// Command definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commands: Option<CommandsConfig>,

    /// Agent definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents: Option<AgentsConfig>,

    /// Skill definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<SkillsConfig>,

    /// Output style definitions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_styles: Option<OutputStylesConfig>,

    /// MCP server configurations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<McpServersConfig>,

    /// LSP server configurations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lsp_servers: Option<LspServersConfig>,
}

/// Author information.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthorInfo {
    /// Author name.
    pub name: String,

    /// Author email.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// Author URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

// ============================================================================
// Component configuration types
// ============================================================================

/// Hooks configuration - path, inline, or array.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HooksConfig {
    /// Path to hooks.json file.
    Path(String),
    /// Inline hook definitions.
    Inline(HashMap<String, Vec<HookMatcherDef>>),
    /// Array of hook definition files.
    Files(Vec<String>),
}

/// Hook matcher definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMatcherDef {
    /// Matcher pattern.
    pub matcher: String,
    /// Hooks for this matcher.
    pub hooks: Vec<HookDef>,
}

/// Individual hook definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookDef {
    /// Hook type (command, script, http).
    #[serde(rename = "type")]
    pub hook_type: String,

    /// Shell command (for command type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Script path (for script type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,

    /// HTTP URL (for http type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Timeout in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<i32>,

    /// Status message during execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
}

/// Commands configuration - path, array, or map.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CommandsConfig {
    /// Path to commands directory.
    Path(String),
    /// Array of command file paths.
    Files(Vec<String>),
    /// Map of command name to metadata.
    Map(HashMap<String, CommandMetadata>),
}

/// Command metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandMetadata {
    /// Path to command markdown file (XOR with content).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Inline markdown content (XOR with source).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Command description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Argument hint (e.g., "[file]").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument_hint: Option<String>,

    /// Default model for command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Allowed tools for this command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
}

/// Agents configuration - path or array.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AgentsConfig {
    /// Path to agents directory.
    Path(String),
    /// Array of agent definition files.
    Files(Vec<String>),
}

/// Skills configuration - path or array.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SkillsConfig {
    /// Path to skills directory.
    Path(String),
    /// Array of skill directory paths.
    Files(Vec<String>),
}

/// Output styles configuration - path or array.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OutputStylesConfig {
    /// Path to styles directory.
    Path(String),
    /// Array of style definition files.
    Files(Vec<String>),
}

/// MCP servers configuration - path, map, or array.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpServersConfig {
    /// Path to MCP config file (.mcp.json or .mcpb).
    Path(String),
    /// Array of MCP config file paths.
    Files(Vec<String>),
    /// Map of server name to config.
    Map(HashMap<String, McpServerDef>),
}

/// MCP server definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerDef {
    /// Server command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Command arguments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,

    /// Environment variables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    /// Server URL (for remote servers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Server type (stdio, sse, http).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub server_type: Option<String>,
}

/// LSP servers configuration - path, map, or array.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LspServersConfig {
    /// Path to LSP config file.
    Path(String),
    /// Map of server name to config.
    Map(HashMap<String, LspServerDef>),
    /// Array of LSP config variants.
    Files(Vec<String>),
}

/// LSP server definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerDef {
    /// Server command.
    pub command: String,

    /// Command arguments.
    #[serde(default)]
    pub args: Vec<String>,

    /// Supported languages.
    #[serde(default)]
    pub languages: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_manifest() {
        let json = r#"{"name": "test-plugin"}"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, "test-plugin");
        assert!(manifest.version.is_none());
    }

    #[test]
    fn test_full_manifest() {
        let json = r#"{
            "name": "test-plugin",
            "version": "1.0.0",
            "description": "A test plugin",
            "author": {
                "name": "Test Author",
                "email": "test@example.com"
            },
            "keywords": ["test", "example"],
            "skills": "skills/",
            "agents": ["agents/explore.md"],
            "hooks": "hooks/hooks.json"
        }"#;

        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, "test-plugin");
        assert_eq!(manifest.version, Some("1.0.0".to_string()));
        assert_eq!(manifest.author.as_ref().unwrap().name, "Test Author");
        assert_eq!(manifest.keywords.len(), 2);
    }

    #[test]
    fn test_commands_config_variants() {
        // Path variant
        let json = r#"{"name": "p", "commands": "commands/"}"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert!(matches!(manifest.commands, Some(CommandsConfig::Path(_))));

        // Files variant
        let json = r#"{"name": "p", "commands": ["cmd1.md", "cmd2.md"]}"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert!(matches!(manifest.commands, Some(CommandsConfig::Files(_))));

        // Map variant
        let json = r#"{"name": "p", "commands": {"cmd1": {"description": "Test"}}}"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert!(matches!(manifest.commands, Some(CommandsConfig::Map(_))));
    }

    #[test]
    fn test_mcp_servers_config() {
        let json = r#"{
            "name": "p",
            "mcpServers": {
                "server1": {
                    "command": "npx",
                    "args": ["-y", "mcp-server"]
                }
            }
        }"#;

        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        if let Some(McpServersConfig::Map(servers)) = manifest.mcp_servers {
            let server = servers.get("server1").unwrap();
            assert_eq!(server.command, Some("npx".to_string()));
        } else {
            panic!("Expected Map variant");
        }
    }
}
