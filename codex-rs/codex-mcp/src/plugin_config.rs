use codex_config::McpServerConfig;
use codex_config::McpServerEnvVar;
use codex_config::McpServerTransportConfig;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathUri;
use serde::Deserialize;
use serde_json::Map as JsonMap;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::path::Path;
use tracing::warn;

#[derive(Clone, Copy, Debug)]
enum PluginMcpSource<'a> {
    Host {
        root: &'a Path,
    },
    Environment {
        root: &'a PathUri,
        environment_id: &'a str,
    },
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
) -> Result<PluginMcpConfigParseOutcome, serde_json::Error> {
    parse_plugin_mcp_config_from(contents, PluginMcpSource::Host { root: plugin_root })
}

/// Parses executor-owned plugin MCP config without interpreting the plugin root
/// as a path on the orchestrator host.
pub fn parse_executor_plugin_mcp_config(
    plugin_root: &PathUri,
    contents: &str,
    environment_id: &str,
) -> Result<PluginMcpConfigParseOutcome, serde_json::Error> {
    parse_plugin_mcp_config_from(
        contents,
        PluginMcpSource::Environment {
            root: plugin_root,
            environment_id,
        },
    )
}

impl PluginMcpSource<'_> {
    fn display(self) -> String {
        match self {
            Self::Host { root } => root.display().to_string(),
            Self::Environment { root, .. } => root.to_string(),
        }
    }
}

fn parse_plugin_mcp_config_from(
    contents: &str,
    source: PluginMcpSource<'_>,
) -> Result<PluginMcpConfigParseOutcome, serde_json::Error> {
    let parsed = serde_json::from_str::<PluginMcpFile>(contents)?;
    let mut outcome = PluginMcpConfigParseOutcome::default();

    for (name, config_value) in parsed.into_mcp_servers() {
        match normalize_plugin_mcp_server(config_value, source) {
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
    value: JsonValue,
    source: PluginMcpSource<'_>,
) -> Result<McpServerConfig, String> {
    let mut object = normalize_plugin_mcp_server_value(value, source);
    if let PluginMcpSource::Environment {
        root,
        environment_id,
    } = source
    {
        object.insert(
            "environment_id".to_string(),
            JsonValue::String(environment_id.to_string()),
        );
        if object.contains_key("command") {
            match object.remove("cwd") {
                Some(JsonValue::String(cwd)) => object.insert(
                    "cwd".to_string(),
                    JsonValue::String(environment_cwd(root, Some(&cwd))?.into_string()),
                ),
                Some(JsonValue::Null) | None => object.insert(
                    "cwd".to_string(),
                    JsonValue::String(
                        environment_cwd(root, /*configured_cwd*/ None)?.into_string(),
                    ),
                ),
                Some(value) => object.insert("cwd".to_string(), value),
            };
        }
    }

    let mut config = serde_json::from_value::<McpServerConfig>(JsonValue::Object(object))
        .map_err(|err| err.to_string())?;
    if matches!(source, PluginMcpSource::Environment { .. }) {
        bind_environment_env_vars(&mut config)?;
    }
    Ok(config)
}

fn environment_cwd(
    root: &PathUri,
    configured_cwd: Option<&str>,
) -> Result<LegacyAppPathString, String> {
    let Some(configured_cwd) = configured_cwd else {
        return Ok(root.clone().into());
    };
    let cwd = PathUri::parse(configured_cwd)
        .or_else(|_| root.join(configured_cwd))
        .map_err(|err| format!("invalid cwd `{configured_cwd}`: {err}"))?;
    if !cwd.starts_with(root) {
        return Err(format!(
            "cwd `{configured_cwd}` must remain within plugin root `{root}`"
        ));
    }
    Ok(cwd.into())
}

fn bind_environment_env_vars(config: &mut McpServerConfig) -> Result<(), String> {
    let is_local_environment = config.is_local_environment();
    let env_vars = match &mut config.transport {
        McpServerTransportConfig::Stdio { env_vars, .. } => env_vars,
        // Never resolve executor-owned environment references in the host process.
        // Remove this rejection once the owning executor resolves these fields.
        McpServerTransportConfig::StreamableHttp {
            bearer_token_env_var,
            env_http_headers,
            ..
        } => {
            if is_local_environment {
                return Ok(());
            }
            if bearer_token_env_var.is_some() {
                return Err(
                    "`bearer_token_env_var` requires executor-side environment resolution for an executor-owned HTTP MCP"
                        .to_string(),
                );
            }
            if env_http_headers
                .as_ref()
                .is_some_and(|headers| !headers.is_empty())
            {
                return Err(
                    "`env_http_headers` requires executor-side environment resolution for an executor-owned HTTP MCP"
                        .to_string(),
                );
            }
            return Ok(());
        }
    };
    for env_var in env_vars {
        match env_var {
            McpServerEnvVar::Name(name) if !is_local_environment => {
                *env_var = McpServerEnvVar::Config {
                    name: std::mem::take(name),
                    source: Some("remote".to_string()),
                };
            }
            McpServerEnvVar::Name(_) => {}
            McpServerEnvVar::Config { name, source } => {
                match (is_local_environment, source.as_deref()) {
                    (true, None | Some("local")) | (false, Some("remote")) => {}
                    (true, Some("remote")) => {
                        return Err(format!(
                            "env_vars entry `{name}` cannot use source `remote` in a local environment"
                        ));
                    }
                    (false, None) => *source = Some("remote".to_string()),
                    (false, Some("local")) => {
                        return Err(format!(
                            "env_vars entry `{name}` cannot use source `local` in an executor-owned plugin"
                        ));
                    }
                    (_, Some(source)) => unreachable!("validated env_vars source `{source}`"),
                }
            }
        }
    }
    Ok(())
}

fn normalize_plugin_mcp_server_value(
    value: JsonValue,
    source: PluginMcpSource<'_>,
) -> JsonMap<String, JsonValue> {
    let mut object = match value {
        JsonValue::Object(object) => object,
        _ => return JsonMap::new(),
    };

    if let Some(JsonValue::String(transport_type)) = object.remove("type") {
        match transport_type.as_str() {
            "http" | "streamable_http" | "streamable-http" | "stdio" => {}
            other => {
                let plugin_display = source.display();
                warn!(
                    plugin = %plugin_display,
                    transport = other,
                    "plugin MCP server uses an unknown transport type"
                );
            }
        }
    }

    if let Some(JsonValue::Object(mut oauth)) = object.remove("oauth") {
        if oauth.remove("callbackPort").is_some() {
            let plugin_display = source.display();
            warn!(
                plugin = %plugin_display,
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

    if let PluginMcpSource::Host { root } = source
        && let Some(JsonValue::String(cwd)) = object.get("cwd")
        && !Path::new(cwd).is_absolute()
    {
        object.insert(
            "cwd".to_string(),
            JsonValue::String(root.join(cwd).display().to_string()),
        );
    }

    object
}

#[cfg(test)]
#[path = "plugin_config_tests.rs"]
mod tests;
