//! Types used to define the fields of [`crate::config::Config`].

// Note this file should generally be restricted to simple struct/enum
// definitions that do not contain business logic.

use serde::Deserializer;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use wildmatch::WildMatchPattern;

use serde::Deserialize;
use serde::Serialize;
use serde::de::Error as SerdeError;

pub const DEFAULT_OTEL_ENVIRONMENT: &str = "dev";

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct McpServerConfig {
    #[serde(flatten)]
    pub transport: McpServerTransportConfig,

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
}

impl<'de> Deserialize<'de> for McpServerConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawMcpServerConfig {
            command: Option<String>,
            #[serde(default)]
            args: Option<Vec<String>>,
            #[serde(default)]
            env: Option<HashMap<String, String>>,

            url: Option<String>,
            bearer_token: Option<String>,

            #[serde(default)]
            startup_timeout_sec: Option<f64>,
            #[serde(default)]
            startup_timeout_ms: Option<u64>,
            #[serde(default, with = "option_duration_secs")]
            tool_timeout_sec: Option<Duration>,
        }

        let raw = RawMcpServerConfig::deserialize(deserializer)?;

        let startup_timeout_sec = match (raw.startup_timeout_sec, raw.startup_timeout_ms) {
            (Some(sec), _) => {
                let duration = Duration::try_from_secs_f64(sec).map_err(SerdeError::custom)?;
                Some(duration)
            }
            (None, Some(ms)) => Some(Duration::from_millis(ms)),
            (None, None) => None,
        };

        fn throw_if_set<E, T>(transport: &str, field: &str, value: Option<&T>) -> Result<(), E>
        where
            E: SerdeError,
        {
            if value.is_none() {
                return Ok(());
            }
            Err(E::custom(format!(
                "{field} is not supported for {transport}",
            )))
        }

        let transport = match raw {
            RawMcpServerConfig {
                command: Some(command),
                args,
                env,
                url,
                bearer_token,
                ..
            } => {
                throw_if_set("stdio", "url", url.as_ref())?;
                throw_if_set("stdio", "bearer_token", bearer_token.as_ref())?;
                McpServerTransportConfig::Stdio {
                    command,
                    args: args.unwrap_or_default(),
                    env,
                }
            }
            RawMcpServerConfig {
                url: Some(url),
                bearer_token,
                command,
                args,
                env,
                ..
            } => {
                throw_if_set("streamable_http", "command", command.as_ref())?;
                throw_if_set("streamable_http", "args", args.as_ref())?;
                throw_if_set("streamable_http", "env", env.as_ref())?;
                McpServerTransportConfig::StreamableHttp { url, bearer_token }
            }
            _ => return Err(SerdeError::custom("invalid transport")),
        };

        Ok(Self {
            transport,
            startup_timeout_sec,
            tool_timeout_sec: raw.tool_timeout_sec,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged, deny_unknown_fields, rename_all = "snake_case")]
pub enum McpServerTransportConfig {
    /// https://modelcontextprotocol.io/specification/2025-06-18/basic/transports#stdio
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        env: Option<HashMap<String, String>>,
    },
    /// https://modelcontextprotocol.io/specification/2025-06-18/basic/transports#streamable-http
    StreamableHttp {
        url: String,
        /// A plain text bearer token to use for authentication.
        /// This bearer token will be included in the HTTP request header as an `Authorization: Bearer <token>` header.
        /// This should be used with caution because it lives on disk in clear text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bearer_token: Option<String>,
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

#[derive(Deserialize, Debug, Copy, Clone, PartialEq)]
pub enum UriBasedFileOpener {
    #[serde(rename = "vscode")]
    VsCode,

    #[serde(rename = "vscode-insiders")]
    VsCodeInsiders,

    #[serde(rename = "windsurf")]
    Windsurf,

    #[serde(rename = "cursor")]
    Cursor,

    /// Option to disable the URI-based file opener.
    #[serde(rename = "none")]
    None,
}

impl UriBasedFileOpener {
    pub fn get_scheme(&self) -> Option<&str> {
        match self {
            UriBasedFileOpener::VsCode => Some("vscode"),
            UriBasedFileOpener::VsCodeInsiders => Some("vscode-insiders"),
            UriBasedFileOpener::Windsurf => Some("windsurf"),
            UriBasedFileOpener::Cursor => Some("cursor"),
            UriBasedFileOpener::None => None,
        }
    }
}

/// Settings that govern if and what will be written to `~/.codex/history.jsonl`.
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct History {
    /// If true, history entries will not be written to disk.
    pub persistence: HistoryPersistence,

    /// If set, the maximum size of the history file in bytes.
    /// TODO(mbolin): Not currently honored.
    pub max_bytes: Option<usize>,
}

#[derive(Deserialize, Debug, Copy, Clone, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HistoryPersistence {
    /// Save all history entries to disk.
    #[default]
    SaveAll,
    /// Do not write history to disk.
    None,
}

// ===== OTEL configuration =====

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum OtelHttpProtocol {
    /// Binary payload
    Binary,
    /// JSON payload
    Json,
}

/// Which OTEL exporter to use.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum OtelExporterKind {
    None,
    OtlpHttp {
        endpoint: String,
        headers: HashMap<String, String>,
        protocol: OtelHttpProtocol,
    },
    OtlpGrpc {
        endpoint: String,
        headers: HashMap<String, String>,
    },
}

/// OTEL settings loaded from config.toml. Fields are optional so we can apply defaults.
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct OtelConfigToml {
    /// Log user prompt in traces
    pub log_user_prompt: Option<bool>,

    /// Mark traces with environment (dev, staging, prod, test). Defaults to dev.
    pub environment: Option<String>,

    /// Exporter to use. Defaults to `otlp-file`.
    pub exporter: Option<OtelExporterKind>,
}

/// Effective OTEL settings after defaults are applied.
#[derive(Debug, Clone, PartialEq)]
pub struct OtelConfig {
    pub log_user_prompt: bool,
    pub environment: String,
    pub exporter: OtelExporterKind,
}

impl Default for OtelConfig {
    fn default() -> Self {
        OtelConfig {
            log_user_prompt: false,
            environment: DEFAULT_OTEL_ENVIRONMENT.to_owned(),
            exporter: OtelExporterKind::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Notifications {
    Enabled(bool),
    Custom(Vec<String>),
}

impl Default for Notifications {
    fn default() -> Self {
        Self::Enabled(false)
    }
}

/// Collection of settings that are specific to the TUI.
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Tui {
    /// Enable desktop notifications from the TUI when the terminal is unfocused.
    /// Defaults to `false`.
    #[serde(default)]
    pub notifications: Notifications,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct SandboxWorkspaceWrite {
    #[serde(default)]
    pub writable_roots: Vec<PathBuf>,
    #[serde(default)]
    pub network_access: bool,
    #[serde(default)]
    pub exclude_tmpdir_env_var: bool,
    #[serde(default)]
    pub exclude_slash_tmp: bool,
}

impl From<SandboxWorkspaceWrite> for codex_app_server_protocol::SandboxSettings {
    fn from(sandbox_workspace_write: SandboxWorkspaceWrite) -> Self {
        Self {
            writable_roots: sandbox_workspace_write.writable_roots,
            network_access: Some(sandbox_workspace_write.network_access),
            exclude_tmpdir_env_var: Some(sandbox_workspace_write.exclude_tmpdir_env_var),
            exclude_slash_tmp: Some(sandbox_workspace_write.exclude_slash_tmp),
        }
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ShellEnvironmentPolicyInherit {
    /// "Core" environment variables for the platform. On UNIX, this would
    /// include HOME, LOGNAME, PATH, SHELL, and USER, among others.
    Core,

    /// Inherits the full environment from the parent process.
    #[default]
    All,

    /// Do not inherit any environment variables from the parent process.
    None,
}

/// Policy for building the `env` when spawning a process via either the
/// `shell` or `local_shell` tool.
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
pub struct ShellEnvironmentPolicyToml {
    pub inherit: Option<ShellEnvironmentPolicyInherit>,

    pub ignore_default_excludes: Option<bool>,

    /// List of regular expressions.
    pub exclude: Option<Vec<String>>,

    pub r#set: Option<HashMap<String, String>>,

    /// List of regular expressions.
    pub include_only: Option<Vec<String>>,

    pub experimental_use_profile: Option<bool>,
}

pub type EnvironmentVariablePattern = WildMatchPattern<'*', '?'>;

/// Deriving the `env` based on this policy works as follows:
/// 1. Create an initial map based on the `inherit` policy.
/// 2. If `ignore_default_excludes` is false, filter the map using the default
///    exclude pattern(s), which are: `"*KEY*"` and `"*TOKEN*"`.
/// 3. If `exclude` is not empty, filter the map using the provided patterns.
/// 4. Insert any entries from `r#set` into the map.
/// 5. If non-empty, filter the map using the `include_only` patterns.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShellEnvironmentPolicy {
    /// Starting point when building the environment.
    pub inherit: ShellEnvironmentPolicyInherit,

    /// True to skip the check to exclude default environment variables that
    /// contain "KEY" or "TOKEN" in their name.
    pub ignore_default_excludes: bool,

    /// Environment variable names to exclude from the environment.
    pub exclude: Vec<EnvironmentVariablePattern>,

    /// (key, value) pairs to insert in the environment.
    pub r#set: HashMap<String, String>,

    /// Environment variable names to retain in the environment.
    pub include_only: Vec<EnvironmentVariablePattern>,

    /// If true, the shell profile will be used to run the command.
    pub use_profile: bool,
}

impl From<ShellEnvironmentPolicyToml> for ShellEnvironmentPolicy {
    fn from(toml: ShellEnvironmentPolicyToml) -> Self {
        // Default to inheriting the full environment when not specified.
        let inherit = toml.inherit.unwrap_or(ShellEnvironmentPolicyInherit::All);
        let ignore_default_excludes = toml.ignore_default_excludes.unwrap_or(false);
        let exclude = toml
            .exclude
            .unwrap_or_default()
            .into_iter()
            .map(|s| EnvironmentVariablePattern::new_case_insensitive(&s))
            .collect();
        let r#set = toml.r#set.unwrap_or_default();
        let include_only = toml
            .include_only
            .unwrap_or_default()
            .into_iter()
            .map(|s| EnvironmentVariablePattern::new_case_insensitive(&s))
            .collect();
        let use_profile = toml.experimental_use_profile.unwrap_or(false);

        Self {
            inherit,
            ignore_default_excludes,
            exclude,
            r#set,
            include_only,
            use_profile,
        }
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Default, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ReasoningSummaryFormat {
    #[default]
    None,
    Experimental,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn deserialize_stdio_command_server_config() {
        let cfg: McpServerConfig = toml::from_str(
            r#"
            command = "echo"
        "#,
        )
        .expect("should deserialize command config");

        assert_eq!(
            cfg.transport,
            McpServerTransportConfig::Stdio {
                command: "echo".to_string(),
                args: vec![],
                env: None
            }
        );
    }

    #[test]
    fn deserialize_stdio_command_server_config_with_args() {
        let cfg: McpServerConfig = toml::from_str(
            r#"
            command = "echo"
            args = ["hello", "world"]
        "#,
        )
        .expect("should deserialize command config");

        assert_eq!(
            cfg.transport,
            McpServerTransportConfig::Stdio {
                command: "echo".to_string(),
                args: vec!["hello".to_string(), "world".to_string()],
                env: None
            }
        );
    }

    #[test]
    fn deserialize_stdio_command_server_config_with_arg_with_args_and_env() {
        let cfg: McpServerConfig = toml::from_str(
            r#"
            command = "echo"
            args = ["hello", "world"]
            env = { "FOO" = "BAR" }
        "#,
        )
        .expect("should deserialize command config");

        assert_eq!(
            cfg.transport,
            McpServerTransportConfig::Stdio {
                command: "echo".to_string(),
                args: vec!["hello".to_string(), "world".to_string()],
                env: Some(HashMap::from([("FOO".to_string(), "BAR".to_string())]))
            }
        );
    }

    #[test]
    fn deserialize_streamable_http_server_config() {
        let cfg: McpServerConfig = toml::from_str(
            r#"
            url = "https://example.com/mcp"
        "#,
        )
        .expect("should deserialize http config");

        assert_eq!(
            cfg.transport,
            McpServerTransportConfig::StreamableHttp {
                url: "https://example.com/mcp".to_string(),
                bearer_token: None
            }
        );
    }

    #[test]
    fn deserialize_streamable_http_server_config_with_bearer_token() {
        let cfg: McpServerConfig = toml::from_str(
            r#"
            url = "https://example.com/mcp"
            bearer_token = "secret"
        "#,
        )
        .expect("should deserialize http config");

        assert_eq!(
            cfg.transport,
            McpServerTransportConfig::StreamableHttp {
                url: "https://example.com/mcp".to_string(),
                bearer_token: Some("secret".to_string())
            }
        );
    }

    #[test]
    fn deserialize_rejects_command_and_url() {
        toml::from_str::<McpServerConfig>(
            r#"
            command = "echo"
            url = "https://example.com"
        "#,
        )
        .expect_err("should reject command+url");
    }

    #[test]
    fn deserialize_rejects_env_for_http_transport() {
        toml::from_str::<McpServerConfig>(
            r#"
            url = "https://example.com"
            env = { "FOO" = "BAR" }
        "#,
        )
        .expect_err("should reject env for http transport");
    }

    #[test]
    fn deserialize_rejects_bearer_token_for_stdio_transport() {
        toml::from_str::<McpServerConfig>(
            r#"
            command = "echo"
            bearer_token = "secret"
        "#,
        )
        .expect_err("should reject bearer token for stdio transport");
    }
}
