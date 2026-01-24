//! Component extraction from plugin manifests.

use crate::error::Result;
use crate::frontmatter::extract_description;
use crate::frontmatter::extract_name;
use crate::frontmatter::parse_frontmatter;
use crate::manifest::AgentsConfig;
use crate::manifest::CommandsConfig;
use crate::manifest::HooksConfig;
use crate::manifest::LspServersConfig;
use crate::manifest::McpServersConfig;
use crate::manifest::PluginManifest;
use crate::manifest::SkillsConfig;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use tracing::debug;

/// Extracted skill from a plugin.
#[derive(Debug, Clone)]
pub struct PluginSkill {
    /// Skill name.
    pub name: String,
    /// Skill description.
    pub description: String,
    /// Path to SKILL.md file.
    pub path: PathBuf,
    /// Source plugin ID.
    pub source_plugin: String,
}

/// Extracted agent from a plugin.
#[derive(Debug, Clone)]
pub struct PluginAgent {
    /// Agent type identifier.
    pub agent_type: String,
    /// Path to agent definition file.
    pub path: PathBuf,
    /// Source plugin ID.
    pub source_plugin: String,
}

/// Extracted hook from a plugin.
#[derive(Debug, Clone)]
pub struct PluginHook {
    /// Hook event type.
    pub event_type: String,
    /// Matcher pattern.
    pub matcher: String,
    /// Hook configuration.
    pub config: PluginHookConfig,
    /// Source plugin ID.
    pub source_plugin: String,
}

/// Hook configuration.
#[derive(Debug, Clone)]
pub struct PluginHookConfig {
    /// Hook type (command, script, http).
    pub hook_type: String,
    /// Command (for command type).
    pub command: Option<String>,
    /// Script path (for script type).
    pub script: Option<String>,
    /// URL (for http type).
    pub url: Option<String>,
    /// Timeout in milliseconds.
    pub timeout: Option<i32>,
}

/// Extracted MCP server from a plugin.
#[derive(Debug, Clone)]
pub struct PluginMcpServer {
    /// Server name.
    pub name: String,
    /// Server command.
    pub command: Option<String>,
    /// Command arguments.
    pub args: Vec<String>,
    /// Environment variables.
    pub env: std::collections::HashMap<String, String>,
    /// Server URL (for remote servers).
    pub url: Option<String>,
    /// Source plugin ID.
    pub source_plugin: String,
}

/// Extracted LSP server from a plugin.
#[derive(Debug, Clone)]
pub struct PluginLspServer {
    /// Server name.
    pub name: String,
    /// Server command.
    pub command: String,
    /// Command arguments.
    pub args: Vec<String>,
    /// Supported languages.
    pub languages: Vec<String>,
    /// Source plugin ID.
    pub source_plugin: String,
}

/// Extracted command from a plugin.
#[derive(Debug, Clone)]
pub struct PluginCommand {
    /// Command name (plugin-name:command-name format).
    pub name: String,
    /// Command description.
    pub description: String,
    /// Path to command markdown file (empty for inline content).
    pub path: PathBuf,
    /// Inline markdown content (alternative to path).
    pub content: Option<String>,
    /// Source plugin ID.
    pub source_plugin: String,
    /// Argument hint (e.g., "[file]").
    pub argument_hint: Option<String>,
    /// Default model for command.
    pub model: Option<String>,
    /// Allowed tools for this command.
    pub allowed_tools: Option<Vec<String>>,
}

/// Extract skills from a plugin manifest.
pub async fn extract_skills(
    manifest: &PluginManifest,
    base_path: &Path,
) -> Result<Vec<PluginSkill>> {
    let mut skills = Vec::new();

    let skills_config = match &manifest.skills {
        Some(config) => config,
        None => return Ok(skills),
    };

    let paths = match skills_config {
        SkillsConfig::Path(path) => vec![base_path.join(path)],
        SkillsConfig::Files(files) => files.iter().map(|f| base_path.join(f)).collect(),
    };

    for skills_dir in paths {
        let skill_files = super::discovery::find_skill_files(&skills_dir).await?;
        for skill_file in skill_files {
            if let Some(skill) = parse_skill_file(&skill_file, &manifest.name).await? {
                skills.push(skill);
            }
        }
    }

    Ok(skills)
}

/// Parse a SKILL.md file.
async fn parse_skill_file(path: &Path, plugin_name: &str) -> Result<Option<PluginSkill>> {
    let content = fs::read_to_string(path).await?;
    let parsed = parse_frontmatter(&content);

    // Extract name from frontmatter or directory name
    let fallback_name = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| plugin_name.to_string());

    let name = extract_name(&parsed, &fallback_name);
    let description = extract_description(&parsed);

    Ok(Some(PluginSkill {
        name,
        description,
        path: path.to_path_buf(),
        source_plugin: plugin_name.to_string(),
    }))
}

/// Extract agents from a plugin manifest.
pub async fn extract_agents(
    manifest: &PluginManifest,
    base_path: &Path,
) -> Result<Vec<PluginAgent>> {
    let mut agents = Vec::new();

    let agents_config = match &manifest.agents {
        Some(config) => config,
        None => return Ok(agents),
    };

    let paths = match agents_config {
        AgentsConfig::Path(path) => {
            super::discovery::find_markdown_files(&base_path.join(path)).await?
        }
        AgentsConfig::Files(files) => files.iter().map(|f| base_path.join(f)).collect(),
    };

    for agent_path in paths {
        let agent_type = agent_path
            .file_stem()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        agents.push(PluginAgent {
            agent_type,
            path: agent_path,
            source_plugin: manifest.name.clone(),
        });
    }

    Ok(agents)
}

/// Parse hooks from a JSON file.
async fn parse_hooks_file(path: &Path, plugin_name: &str) -> Result<Vec<PluginHook>> {
    let mut hooks = Vec::new();

    if !path.exists() {
        debug!("Hooks file not found: {}", path.display());
        return Ok(hooks);
    }

    let content = fs::read_to_string(path).await?;
    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            debug!("Failed to parse hooks file {}: {e}", path.display());
            return Ok(hooks);
        }
    };

    let obj = match parsed.as_object() {
        Some(o) => o,
        None => return Ok(hooks),
    };

    for (event_type, matchers) in obj {
        let matchers_arr = match matchers.as_array() {
            Some(a) => a,
            None => continue,
        };

        for matcher_def in matchers_arr {
            let matcher = match matcher_def.get("matcher").and_then(|m| m.as_str()) {
                Some(m) => m,
                None => continue,
            };

            let hook_defs = match matcher_def.get("hooks").and_then(|h| h.as_array()) {
                Some(h) => h,
                None => continue,
            };

            for hook_def in hook_defs {
                hooks.push(PluginHook {
                    event_type: event_type.clone(),
                    matcher: matcher.to_string(),
                    config: PluginHookConfig {
                        hook_type: hook_def
                            .get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("command")
                            .to_string(),
                        command: hook_def
                            .get("command")
                            .and_then(|c| c.as_str())
                            .map(String::from),
                        script: hook_def
                            .get("script")
                            .and_then(|s| s.as_str())
                            .map(String::from),
                        url: hook_def
                            .get("url")
                            .and_then(|u| u.as_str())
                            .map(String::from),
                        timeout: hook_def
                            .get("timeout")
                            .and_then(|t| t.as_i64())
                            .map(|t| t as i32),
                    },
                    source_plugin: plugin_name.to_string(),
                });
            }
        }
    }

    Ok(hooks)
}

/// Extract hooks from a plugin manifest.
pub async fn extract_hooks(manifest: &PluginManifest, base_path: &Path) -> Result<Vec<PluginHook>> {
    let mut hooks = Vec::new();

    let hooks_config = match &manifest.hooks {
        Some(config) => config,
        None => return Ok(hooks),
    };

    match hooks_config {
        HooksConfig::Path(path) => {
            let hooks_path = base_path.join(path);
            let parsed = parse_hooks_file(&hooks_path, &manifest.name).await?;
            hooks.extend(parsed);
        }
        HooksConfig::Inline(inline) => {
            for (event_type, matchers) in inline {
                for matcher_def in matchers {
                    for hook_def in &matcher_def.hooks {
                        hooks.push(PluginHook {
                            event_type: event_type.clone(),
                            matcher: matcher_def.matcher.clone(),
                            config: PluginHookConfig {
                                hook_type: hook_def.hook_type.clone(),
                                command: hook_def.command.clone(),
                                script: hook_def.script.clone(),
                                url: hook_def.url.clone(),
                                timeout: hook_def.timeout,
                            },
                            source_plugin: manifest.name.clone(),
                        });
                    }
                }
            }
        }
        HooksConfig::Files(files) => {
            for file in files {
                let hooks_path = base_path.join(file);
                let parsed = parse_hooks_file(&hooks_path, &manifest.name).await?;
                hooks.extend(parsed);
            }
        }
    }

    Ok(hooks)
}

/// Parse MCP servers from a JSON file (.mcp.json format).
async fn parse_mcp_servers_file(path: &Path, plugin_name: &str) -> Result<Vec<PluginMcpServer>> {
    let mut servers = Vec::new();

    if !path.exists() {
        debug!("MCP servers file not found: {}", path.display());
        return Ok(servers);
    }

    let content = fs::read_to_string(path).await?;
    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            debug!("Failed to parse MCP servers file {}: {e}", path.display());
            return Ok(servers);
        }
    };

    // Support both root-level servers and mcpServers key
    let mcp_servers = parsed
        .get("mcpServers")
        .and_then(|s| s.as_object())
        .or_else(|| parsed.as_object());

    let mcp_servers = match mcp_servers {
        Some(s) => s,
        None => return Ok(servers),
    };

    for (name, def) in mcp_servers {
        servers.push(PluginMcpServer {
            name: name.clone(),
            command: def
                .get("command")
                .and_then(|c| c.as_str())
                .map(String::from),
            args: def
                .get("args")
                .and_then(|a| a.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            env: def
                .get("env")
                .and_then(|e| e.as_object())
                .map(|o| {
                    o.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default(),
            url: def.get("url").and_then(|u| u.as_str()).map(String::from),
            source_plugin: plugin_name.to_string(),
        });
    }

    Ok(servers)
}

/// Extract MCP servers from a plugin manifest.
pub async fn extract_mcp_servers(
    manifest: &PluginManifest,
    base_path: &Path,
) -> Result<Vec<PluginMcpServer>> {
    let mut servers = Vec::new();

    let mcp_config = match &manifest.mcp_servers {
        Some(config) => config,
        None => return Ok(servers),
    };

    match mcp_config {
        McpServersConfig::Map(map) => {
            for (name, def) in map {
                servers.push(PluginMcpServer {
                    name: name.clone(),
                    command: def.command.clone(),
                    args: def.args.clone().unwrap_or_default(),
                    env: def.env.clone().unwrap_or_default(),
                    url: def.url.clone(),
                    source_plugin: manifest.name.clone(),
                });
            }
        }
        McpServersConfig::Path(path) => {
            let mcp_path = base_path.join(path);
            let parsed = parse_mcp_servers_file(&mcp_path, &manifest.name).await?;
            servers.extend(parsed);
        }
        McpServersConfig::Files(files) => {
            for file in files {
                let mcp_path = base_path.join(file);
                let parsed = parse_mcp_servers_file(&mcp_path, &manifest.name).await?;
                servers.extend(parsed);
            }
        }
    }

    Ok(servers)
}

/// Parse LSP servers from a JSON file.
async fn parse_lsp_servers_file(path: &Path, plugin_name: &str) -> Result<Vec<PluginLspServer>> {
    let mut servers = Vec::new();

    if !path.exists() {
        debug!("LSP servers file not found: {}", path.display());
        return Ok(servers);
    }

    let content = fs::read_to_string(path).await?;
    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            debug!("Failed to parse LSP servers file {}: {e}", path.display());
            return Ok(servers);
        }
    };

    // Support both root-level servers and lspServers key
    let lsp_servers = parsed
        .get("lspServers")
        .and_then(|s| s.as_object())
        .or_else(|| parsed.as_object());

    let lsp_servers = match lsp_servers {
        Some(s) => s,
        None => return Ok(servers),
    };

    for (name, def) in lsp_servers {
        let command = match def.get("command").and_then(|c| c.as_str()) {
            Some(c) => c.to_string(),
            None => continue,
        };

        servers.push(PluginLspServer {
            name: name.clone(),
            command,
            args: def
                .get("args")
                .and_then(|a| a.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            languages: def
                .get("languages")
                .and_then(|l| l.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            source_plugin: plugin_name.to_string(),
        });
    }

    Ok(servers)
}

/// Extract LSP servers from a plugin manifest.
pub async fn extract_lsp_servers(
    manifest: &PluginManifest,
    base_path: &Path,
) -> Result<Vec<PluginLspServer>> {
    let mut servers = Vec::new();

    let lsp_config = match &manifest.lsp_servers {
        Some(config) => config,
        None => return Ok(servers),
    };

    match lsp_config {
        LspServersConfig::Map(map) => {
            for (name, def) in map {
                servers.push(PluginLspServer {
                    name: name.clone(),
                    command: def.command.clone(),
                    args: def.args.clone(),
                    languages: def.languages.clone(),
                    source_plugin: manifest.name.clone(),
                });
            }
        }
        LspServersConfig::Path(path) => {
            let lsp_path = base_path.join(path);
            let parsed = parse_lsp_servers_file(&lsp_path, &manifest.name).await?;
            servers.extend(parsed);
        }
        LspServersConfig::Files(files) => {
            for file in files {
                let lsp_path = base_path.join(file);
                let parsed = parse_lsp_servers_file(&lsp_path, &manifest.name).await?;
                servers.extend(parsed);
            }
        }
    }

    Ok(servers)
}

/// Extract commands from a plugin manifest.
pub async fn extract_commands(
    manifest: &PluginManifest,
    base_path: &Path,
) -> Result<Vec<PluginCommand>> {
    let mut commands = Vec::new();

    let commands_config = match &manifest.commands {
        Some(config) => config,
        None => return Ok(commands),
    };

    match commands_config {
        CommandsConfig::Path(path) => {
            let cmd_dir = base_path.join(path);
            let md_files = super::discovery::find_markdown_files(&cmd_dir).await?;
            for md_file in md_files {
                let cmd_name = md_file
                    .file_stem()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                // Extract description from file frontmatter
                let description = if let Ok(file_content) = fs::read_to_string(&md_file).await {
                    let parsed = parse_frontmatter(&file_content);
                    extract_description(&parsed)
                } else {
                    String::new()
                };

                commands.push(PluginCommand {
                    name: format!("{}:{}", manifest.name, cmd_name),
                    description,
                    path: md_file,
                    content: None,
                    source_plugin: manifest.name.clone(),
                    argument_hint: None,
                    model: None,
                    allowed_tools: None,
                });
            }
        }
        CommandsConfig::Files(files) => {
            for file in files {
                let file_path = base_path.join(file);

                // Handle both files and directories
                if file_path.is_dir() {
                    let md_files = super::discovery::find_markdown_files(&file_path).await?;
                    for md_file in md_files {
                        let cmd_name = md_file
                            .file_stem()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();

                        let description =
                            if let Ok(file_content) = fs::read_to_string(&md_file).await {
                                let parsed = parse_frontmatter(&file_content);
                                extract_description(&parsed)
                            } else {
                                String::new()
                            };

                        commands.push(PluginCommand {
                            name: format!("{}:{}", manifest.name, cmd_name),
                            description,
                            path: md_file,
                            content: None,
                            source_plugin: manifest.name.clone(),
                            argument_hint: None,
                            model: None,
                            allowed_tools: None,
                        });
                    }
                } else {
                    let cmd_name = file_path
                        .file_stem()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();

                    let description = if let Ok(file_content) = fs::read_to_string(&file_path).await
                    {
                        let parsed = parse_frontmatter(&file_content);
                        extract_description(&parsed)
                    } else {
                        String::new()
                    };

                    commands.push(PluginCommand {
                        name: format!("{}:{}", manifest.name, cmd_name),
                        description,
                        path: file_path,
                        content: None,
                        source_plugin: manifest.name.clone(),
                        argument_hint: None,
                        model: None,
                        allowed_tools: None,
                    });
                }
            }
        }
        CommandsConfig::Map(map) => {
            for (cmd_name, metadata) in map {
                // Handle inline content XOR file source
                let (path, content, description) = if let Some(inline_content) = &metadata.content {
                    // Inline content - extract description from frontmatter
                    let parsed = parse_frontmatter(inline_content);
                    let desc = metadata
                        .description
                        .clone()
                        .unwrap_or_else(|| extract_description(&parsed));
                    (PathBuf::new(), Some(inline_content.clone()), desc)
                } else if let Some(source) = &metadata.source {
                    // File source
                    let source_path = base_path.join(source);
                    let desc = if let Some(d) = &metadata.description {
                        d.clone()
                    } else if source_path.exists() {
                        if let Ok(file_content) = fs::read_to_string(&source_path).await {
                            let parsed = parse_frontmatter(&file_content);
                            extract_description(&parsed)
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };
                    (source_path, None, desc)
                } else {
                    // Neither content nor source - use empty defaults
                    let desc = metadata.description.clone().unwrap_or_default();
                    (PathBuf::new(), None, desc)
                };

                commands.push(PluginCommand {
                    name: format!("{}:{}", manifest.name, cmd_name),
                    description,
                    path,
                    content,
                    source_plugin: manifest.name.clone(),
                    argument_hint: metadata.argument_hint.clone(),
                    model: metadata.model.clone(),
                    allowed_tools: metadata.allowed_tools.clone(),
                });
            }
        }
    }

    Ok(commands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_extract_skills() {
        let dir = tempdir().unwrap();

        // Create skill directory structure
        let skills_dir = dir.path().join("skills").join("my-skill");
        std::fs::create_dir_all(&skills_dir).unwrap();
        std::fs::write(skills_dir.join("SKILL.md"), "# My Skill\n\nA test skill").unwrap();

        let manifest = PluginManifest {
            name: "test-plugin".to_string(),
            skills: Some(SkillsConfig::Path("skills".to_string())),
            ..Default::default()
        };

        let skills = extract_skills(&manifest, dir.path()).await.unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "my-skill");
    }

    #[tokio::test]
    async fn test_extract_mcp_servers() {
        let dir = tempdir().unwrap();

        let manifest = PluginManifest {
            name: "test-plugin".to_string(),
            mcp_servers: Some(McpServersConfig::Map(
                [(
                    "test-server".to_string(),
                    crate::manifest::McpServerDef {
                        command: Some("npx".to_string()),
                        args: Some(vec!["-y".to_string(), "mcp-server".to_string()]),
                        env: None,
                        url: None,
                        server_type: None,
                    },
                )]
                .into_iter()
                .collect(),
            )),
            ..Default::default()
        };

        let servers = extract_mcp_servers(&manifest, dir.path()).await.unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "test-server");
        assert_eq!(servers[0].command, Some("npx".to_string()));
    }
}
