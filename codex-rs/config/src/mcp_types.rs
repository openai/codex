//! MCP server configuration types.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::de::Error as SerdeError;

use crate::RequirementSource;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AppToolApproval {
    #[default]
    Auto,
    Prompt,
    Approve,
}

/// Human-readable reason a configured MCP server was disabled after requirements
/// were applied.
///
/// `Display` is intentionally implemented for CLI/TUI status output; avoid
/// relying on `Debug` because enum variant syntax is not part of the user-facing
/// message contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpServerDisabledReason {
    /// The server is disabled, but there is no more specific user-facing reason.
    Unknown,
    /// The server was disabled by config requirements from the given source.
    Requirements { source: RequirementSource },
}

impl fmt::Display for McpServerDisabledReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpServerDisabledReason::Unknown => write!(f, "unknown"),
            McpServerDisabledReason::Requirements { source } => {
                write!(f, "requirements ({source})")
            }
        }
    }
}

/// Per-tool approval settings for a single MCP server tool.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct McpServerToolConfig {
    /// Approval mode for this tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_mode: Option<AppToolApproval>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct McpServerConfig {
    #[serde(flatten)]
    pub transport: McpServerTransportConfig,

    /// When `false`, Codex skips initializing this MCP server.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// When `true`, `codex exec` exits with an error if this MCP server fails to initialize.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub required: bool,

    /// Reason this server was disabled after applying requirements.
    #[serde(skip)]
    pub disabled_reason: Option<McpServerDisabledReason>,

    /// Startup timeout in seconds for initializing MCP server & initially listing tools.
    #[serde(
        default,
        with = "option_duration_secs",
        skip_serializing_if = "Option::is_none"
    )]
    pub startup_timeout_sec: Option<Duration>,

    /// Default timeout for MCP tool calls initiated via this server.
    #[serde(default, with = "option_duration_secs")]
    pub tool_timeout_sec: Option<Duration>,

    /// Explicit allow-list of tools exposed from this server. When set, only these tools will be registered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled_tools: Option<Vec<String>>,

    /// Explicit deny-list of tools. These tools will be removed after applying `enabled_tools`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_tools: Option<Vec<String>>,

    /// Optional OAuth scopes to request during MCP login.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,

    /// Optional OAuth resource parameter to include during MCP login (RFC 8707).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth_resource: Option<String>,

    /// Per-tool approval settings keyed by tool name.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub tools: HashMap<String, McpServerToolConfig>,
}

/// Raw MCP config shape used for deserialization and JSON Schema generation.
///
/// Keep `TryFrom<RawMcpServerConfig> for McpServerConfig` exhaustively
/// destructuring this struct so new TOML fields cannot be added here without
/// updating the validation/mapping logic that produces [`McpServerConfig`].
#[derive(Deserialize, Clone, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct RawMcpServerConfig {
    // stdio
    pub command: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub env_vars: Option<Vec<String>>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    pub http_headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub env_http_headers: Option<HashMap<String, String>>,

    // streamable_http
    pub url: Option<String>,
    pub bearer_token: Option<String>,
    pub bearer_token_env_var: Option<String>,

    // shared
    #[serde(default)]
    pub startup_timeout_sec: Option<f64>,
    #[serde(default)]
    pub startup_timeout_ms: Option<u64>,
    #[serde(default, with = "option_duration_secs")]
    #[schemars(with = "Option<f64>")]
    pub tool_timeout_sec: Option<Duration>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub required: Option<bool>,
    #[serde(default)]
    pub enabled_tools: Option<Vec<String>>,
    #[serde(default)]
    pub disabled_tools: Option<Vec<String>>,
    #[serde(default)]
    pub scopes: Option<Vec<String>>,
    #[serde(default)]
    pub oauth_resource: Option<String>,
    /// Legacy display-name field accepted for backward compatibility.
    #[serde(default, rename = "name")]
    pub _name: Option<String>,
    #[serde(default)]
    pub tools: Option<HashMap<String, McpServerToolConfig>>,
}

impl TryFrom<RawMcpServerConfig> for McpServerConfig {
    type Error = String;

    fn try_from(raw: RawMcpServerConfig) -> Result<Self, Self::Error> {
        let RawMcpServerConfig {
            command,
            args,
            env,
            env_vars,
            cwd,
            http_headers,
            env_http_headers,
            url,
            bearer_token,
            bearer_token_env_var,
            startup_timeout_sec,
            startup_timeout_ms,
            tool_timeout_sec,
            enabled,
            required,
            enabled_tools,
            disabled_tools,
            scopes,
            oauth_resource,
            _name: _,
            tools,
        } = raw;

        let startup_timeout_sec = match (startup_timeout_sec, startup_timeout_ms) {
            (Some(sec), _) => {
                Some(Duration::try_from_secs_f64(sec).map_err(|err| err.to_string())?)
            }
            (None, Some(ms)) => Some(Duration::from_millis(ms)),
            (None, None) => None,
        };

        fn throw_if_set<T>(transport: &str, field: &str, value: Option<&T>) -> Result<(), String> {
            if value.is_none() {
                return Ok(());
            }
            Err(format!("{field} is not supported for {transport}"))
        }

        let transport = if let Some(command) = command {
            throw_if_set("stdio", "url", url.as_ref())?;
            throw_if_set(
                "stdio",
                "bearer_token_env_var",
                bearer_token_env_var.as_ref(),
            )?;
            throw_if_set("stdio", "bearer_token", bearer_token.as_ref())?;
            throw_if_set("stdio", "http_headers", http_headers.as_ref())?;
            throw_if_set("stdio", "env_http_headers", env_http_headers.as_ref())?;
            throw_if_set("stdio", "oauth_resource", oauth_resource.as_ref())?;
            McpServerTransportConfig::Stdio {
                command,
                args: args.unwrap_or_default(),
                env,
                env_vars: env_vars.unwrap_or_default(),
                cwd,
            }
        } else if let Some(url) = url {
            throw_if_set("streamable_http", "args", args.as_ref())?;
            throw_if_set("streamable_http", "env", env.as_ref())?;
            throw_if_set("streamable_http", "env_vars", env_vars.as_ref())?;
            throw_if_set("streamable_http", "cwd", cwd.as_ref())?;
            throw_if_set("streamable_http", "bearer_token", bearer_token.as_ref())?;
            McpServerTransportConfig::StreamableHttp {
                url,
                bearer_token_env_var,
                http_headers,
                env_http_headers,
            }
        } else {
            return Err("invalid transport".to_string());
        };

        Ok(Self {
            transport,
            startup_timeout_sec,
            tool_timeout_sec,
            enabled: enabled.unwrap_or_else(default_enabled),
            required: required.unwrap_or_default(),
            disabled_reason: None,
            enabled_tools,
            disabled_tools,
            scopes,
            oauth_resource,
            tools: tools.unwrap_or_default(),
        })
    }
}

impl<'de> Deserialize<'de> for McpServerConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        RawMcpServerConfig::deserialize(deserializer)?
            .try_into()
            .map_err(SerdeError::custom)
    }
}

const fn default_enabled() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(untagged, deny_unknown_fields, rename_all = "snake_case")]
pub enum McpServerTransportConfig {
    /// https://modelcontextprotocol.io/specification/2025-06-18/basic/transports#stdio
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        env: Option<HashMap<String, String>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        env_vars: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<PathBuf>,
    },
    /// https://modelcontextprotocol.io/specification/2025-06-18/basic/transports#streamable-http
    StreamableHttp {
        url: String,
        /// Name of the environment variable to read for an HTTP bearer token.
        /// When set, requests will include the token via `Authorization: Bearer <token>`.
        /// The actual secret value must be provided via the environment.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bearer_token_env_var: Option<String>,
        /// Additional HTTP headers to include in requests to this server.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        http_headers: Option<HashMap<String, String>>,
        /// HTTP headers where the value is sourced from an environment variable.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        env_http_headers: Option<HashMap<String, String>>,
    },
}

mod option_duration_secs {
    use serde::Deserialize;
    use serde::Deserializer;
    use serde::Serializer;
    use std::time::Duration;

    pub fn serialize<S>(value: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(duration) => serializer.serialize_some(&duration.as_secs_f64()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = Option::<f64>::deserialize(deserializer)?;
        secs.map(|secs| Duration::try_from_secs_f64(secs).map_err(serde::de::Error::custom))
            .transpose()
    }
}

#[cfg(test)]
#[path = "mcp_types_tests.rs"]
mod tests;
