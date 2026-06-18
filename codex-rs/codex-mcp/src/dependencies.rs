use std::collections::HashMap;
use std::collections::HashSet;

use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use tracing::warn;

/// One MCP server requested by a capability selected for the current turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpServerDependency {
    pub source_name: String,
    pub name: String,
    pub transport: Option<String>,
    pub command: Option<String>,
    pub url: Option<String>,
}

/// Turn-scoped MCP dependencies contributed by capability owners.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct McpServerDependencies {
    dependencies: Vec<McpServerDependency>,
}

impl McpServerDependencies {
    pub fn push(&mut self, dependency: McpServerDependency) {
        self.dependencies.push(dependency);
    }

    pub fn is_empty(&self) -> bool {
        self.dependencies.is_empty()
    }

    pub fn missing_from(
        &self,
        installed: &HashMap<String, McpServerConfig>,
    ) -> HashMap<String, McpServerConfig> {
        let mut missing = HashMap::new();
        let installed_keys = installed
            .iter()
            .map(|(name, config)| canonical_server_key(name, config))
            .collect::<HashSet<_>>();
        let mut seen_keys = HashSet::new();

        for dependency in &self.dependencies {
            let dependency_key = match canonical_dependency_key(dependency) {
                Ok(key) => key,
                Err(err) => {
                    warn!(
                        "unable to auto-install MCP dependency {} for {}: {err}",
                        dependency.name, dependency.source_name
                    );
                    continue;
                }
            };
            if installed_keys.contains(&dependency_key) || !seen_keys.insert(dependency_key.clone())
            {
                continue;
            }

            match dependency_to_server_config(dependency) {
                Ok(config) => {
                    missing.insert(dependency.name.clone(), config);
                }
                Err(err) => warn!(
                    "unable to auto-install MCP dependency {dependency_key} for {}: {err}",
                    dependency.source_name
                ),
            }
        }
        missing
    }
}

pub fn canonical_server_key(name: &str, config: &McpServerConfig) -> String {
    match &config.transport {
        McpServerTransportConfig::Stdio { command, .. } => canonical_key("stdio", command, name),
        McpServerTransportConfig::StreamableHttp { url, .. } => {
            canonical_key("streamable_http", url, name)
        }
    }
}

fn canonical_dependency_key(dependency: &McpServerDependency) -> Result<String, String> {
    let transport = dependency.transport.as_deref().unwrap_or("streamable_http");
    if transport.eq_ignore_ascii_case("streamable_http") {
        let url = dependency
            .url
            .as_ref()
            .ok_or_else(|| "missing url for streamable_http dependency".to_string())?;
        return Ok(canonical_key("streamable_http", url, &dependency.name));
    }
    if transport.eq_ignore_ascii_case("stdio") {
        let command = dependency
            .command
            .as_ref()
            .ok_or_else(|| "missing command for stdio dependency".to_string())?;
        return Ok(canonical_key("stdio", command, &dependency.name));
    }
    Err(format!("unsupported transport {transport}"))
}

fn dependency_to_server_config(
    dependency: &McpServerDependency,
) -> Result<McpServerConfig, String> {
    let transport = dependency.transport.as_deref().unwrap_or("streamable_http");
    let transport = if transport.eq_ignore_ascii_case("streamable_http") {
        McpServerTransportConfig::StreamableHttp {
            url: dependency
                .url
                .clone()
                .ok_or_else(|| "missing url for streamable_http dependency".to_string())?,
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        }
    } else if transport.eq_ignore_ascii_case("stdio") {
        McpServerTransportConfig::Stdio {
            command: dependency
                .command
                .clone()
                .ok_or_else(|| "missing command for stdio dependency".to_string())?,
            args: Vec::new(),
            env: None,
            env_vars: Vec::new(),
            cwd: None,
        }
    } else {
        return Err(format!("unsupported transport {transport}"));
    };

    Ok(McpServerConfig {
        transport,
        environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
        enabled: true,
        required: false,
        supports_parallel_tool_calls: false,
        disabled_reason: None,
        startup_timeout_sec: None,
        tool_timeout_sec: None,
        default_tools_approval_mode: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
        oauth: None,
        oauth_resource: None,
        tools: HashMap::new(),
    })
}

fn canonical_key(transport: &str, identifier: &str, fallback: &str) -> String {
    let identifier = identifier.trim();
    if identifier.is_empty() {
        fallback.to_string()
    } else {
        format!("mcp__{transport}__{identifier}")
    }
}

#[cfg(test)]
#[path = "dependencies_tests.rs"]
mod tests;
