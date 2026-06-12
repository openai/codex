use codex_config::McpServerConfig;
use codex_config::McpServerEnvVar;
use codex_config::McpServerTransportConfig;
use serde::Deserialize;
use serde_json::Map as JsonMap;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use tracing::warn;

/// Placement applied while normalizing MCP servers declared by a plugin.
#[derive(Clone, Copy, Debug)]
pub enum PluginMcpServerPlacement<'a> {
    /// Preserve declared placement, resolving a relative working directory below the plugin root.
    Declared,
    /// Bind stdio servers to one environment and default their working directory to the plugin root.
    Environment { environment_id: &'a str },
}

/// One plugin MCP server that could not be normalized into runtime configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginMcpServerParseError {
    pub name: String,
    pub message: String,
}

/// Valid servers and per-server errors parsed from one plugin MCP file.
#[derive(Debug, Default, PartialEq)]
pub struct PluginMcpConfigParseOutcome {
    pub servers: BTreeMap<String, McpServerConfig>,
    pub errors: Vec<PluginMcpServerParseError>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginMcpServersFile {
    mcp_servers: BTreeMap<String, JsonValue>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PluginMcpFile {
    McpServersObject(PluginMcpServersFile),
    ServerMap(BTreeMap<String, JsonValue>),
}

impl PluginMcpFile {
    fn into_mcp_servers(self) -> BTreeMap<String, JsonValue> {
        match self {
            Self::McpServersObject(file) => file.mcp_servers,
            Self::ServerMap(mcp_servers) => mcp_servers,
        }
    }
}

/// Parses the two supported plugin MCP file shapes and normalizes each server.
///
/// Invalid individual servers are returned as errors without discarding valid
/// siblings. A malformed top-level document fails the whole parse.
pub fn parse_plugin_mcp_config(
    plugin_root: &Path,
    contents: &str,
    placement: PluginMcpServerPlacement<'_>,
) -> Result<PluginMcpConfigParseOutcome, serde_json::Error> {
    let parsed = serde_json::from_str::<PluginMcpFile>(contents)?;
    let mut outcome = PluginMcpConfigParseOutcome::default();

    for (name, config_value) in parsed.into_mcp_servers() {
        match normalize_plugin_mcp_server(plugin_root, config_value, placement) {
            Ok(config) => {
                outcome.servers.insert(name, config);
            }
            Err(message) => outcome
                .errors
                .push(PluginMcpServerParseError { name, message }),
        }
    }

    Ok(outcome)
}

fn normalize_plugin_mcp_server(
    plugin_root: &Path,
    value: JsonValue,
    placement: PluginMcpServerPlacement<'_>,
) -> Result<McpServerConfig, String> {
    let mut object = normalize_plugin_mcp_server_value(plugin_root, value, placement);
    if let PluginMcpServerPlacement::Environment { environment_id } = placement {
        object.insert(
            "environment_id".to_string(),
            JsonValue::String(environment_id.to_string()),
        );
        if object.contains_key("command") {
            match object.remove("cwd") {
                Some(JsonValue::String(cwd)) => object.insert(
                    "cwd".to_string(),
                    JsonValue::String(
                        executor_plugin_cwd(plugin_root, &cwd)?
                            .to_string_lossy()
                            .into_owned(),
                    ),
                ),
                Some(value) => object.insert("cwd".to_string(), value),
                None => object.insert(
                    "cwd".to_string(),
                    JsonValue::String(plugin_root.to_string_lossy().into_owned()),
                ),
            };
        }
    }

    let mut config = serde_json::from_value::<McpServerConfig>(JsonValue::Object(object))
        .map_err(|err| err.to_string())?;
    if matches!(placement, PluginMcpServerPlacement::Environment { .. })
        && !config.is_local_environment()
    {
        bind_remote_executor_env_vars(&mut config)?;
    }
    Ok(config)
}

fn executor_plugin_cwd(plugin_root: &Path, configured_cwd: &str) -> Result<PathBuf, String> {
    let cwd = Path::new(configured_cwd);
    if cwd.is_absolute() {
        return Ok(cwd.to_path_buf());
    }
    if cwd.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(format!(
            "relative cwd `{configured_cwd}` must remain within plugin root `{}`",
            plugin_root.display()
        ));
    }
    Ok(plugin_root.join(cwd))
}

fn bind_remote_executor_env_vars(config: &mut McpServerConfig) -> Result<(), String> {
    let McpServerTransportConfig::Stdio { env_vars, .. } = &mut config.transport else {
        return Ok(());
    };
    for env_var in env_vars {
        match env_var {
            McpServerEnvVar::Name(name) => {
                *env_var = McpServerEnvVar::Config {
                    name: std::mem::take(name),
                    source: Some("remote".to_string()),
                };
            }
            McpServerEnvVar::Config { name, source } => match source.as_deref() {
                None => *source = Some("remote".to_string()),
                Some("remote") => {}
                Some("local") => {
                    return Err(format!(
                        "env_vars entry `{name}` cannot use source `local` in an executor-owned plugin"
                    ));
                }
                Some(source) => unreachable!("validated env_vars source `{source}`"),
            },
        }
    }
    Ok(())
}

fn normalize_plugin_mcp_server_value(
    plugin_root: &Path,
    value: JsonValue,
    placement: PluginMcpServerPlacement<'_>,
) -> JsonMap<String, JsonValue> {
    let mut object = match value {
        JsonValue::Object(object) => object,
        _ => return JsonMap::new(),
    };

    if let Some(JsonValue::String(transport_type)) = object.remove("type") {
        match transport_type.as_str() {
            "http" | "streamable_http" | "streamable-http" | "stdio" => {}
            other => {
                warn!(
                    plugin = %plugin_root.display(),
                    transport = other,
                    "plugin MCP server uses an unknown transport type"
                );
            }
        }
    }

    if let Some(JsonValue::Object(mut oauth)) = object.remove("oauth") {
        if oauth.remove("callbackPort").is_some() {
            warn!(
                plugin = %plugin_root.display(),
                "plugin MCP server OAuth callbackPort is ignored; Codex uses global MCP OAuth callback settings"
            );
        }

        if let Some(client_id) = oauth.remove("clientId") {
            oauth.entry("client_id".to_string()).or_insert(client_id);
        }

        if !oauth.is_empty() {
            object.insert("oauth".to_string(), JsonValue::Object(oauth));
        }
    }

    if matches!(placement, PluginMcpServerPlacement::Declared)
        && let Some(JsonValue::String(cwd)) = object.get("cwd")
        && !Path::new(cwd).is_absolute()
    {
        object.insert(
            "cwd".to_string(),
            JsonValue::String(plugin_root.join(cwd).display().to_string()),
        );
    }

    object
}

#[cfg(test)]
#[path = "plugin_config_tests.rs"]
mod tests;
