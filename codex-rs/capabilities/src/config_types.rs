use codex_config::RequirementSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::de::Error as SerdeError;
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpServerDisabledReason {
    Unknown,
    Requirements { source: RequirementSource },
}

impl fmt::Display for McpServerDisabledReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown"),
            Self::Requirements { source } => {
                write!(f, "requirements ({source})")
            }
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct McpServerConfig {
    #[serde(flatten)]
    pub transport: McpServerTransportConfig,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub required: bool,
    #[serde(skip)]
    pub disabled_reason: Option<McpServerDisabledReason>,
    #[serde(
        default,
        with = "option_duration_secs",
        skip_serializing_if = "Option::is_none"
    )]
    pub startup_timeout_sec: Option<Duration>,
    #[serde(default, with = "option_duration_secs")]
    pub tool_timeout_sec: Option<Duration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled_tools: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_tools: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth_resource: Option<String>,
}

#[derive(Deserialize, Clone, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct RawMcpServerConfig {
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
    pub url: Option<String>,
    pub bearer_token: Option<String>,
    pub bearer_token_env_var: Option<String>,
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
}

impl<'de> Deserialize<'de> for McpServerConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut raw = RawMcpServerConfig::deserialize(deserializer)?;

        let startup_timeout_sec = match (raw.startup_timeout_sec, raw.startup_timeout_ms) {
            (Some(sec), _) => {
                let duration = Duration::try_from_secs_f64(sec).map_err(SerdeError::custom)?;
                Some(duration)
            }
            (None, Some(ms)) => Some(Duration::from_millis(ms)),
            (None, None) => None,
        };
        let tool_timeout_sec = raw.tool_timeout_sec;
        let enabled = raw.enabled.unwrap_or_else(default_enabled);
        let required = raw.required.unwrap_or_default();
        let enabled_tools = raw.enabled_tools.clone();
        let disabled_tools = raw.disabled_tools.clone();
        let scopes = raw.scopes.clone();
        let oauth_resource = raw.oauth_resource.clone();

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

        let transport = if let Some(command) = raw.command.clone() {
            throw_if_set("stdio", "url", raw.url.as_ref())?;
            throw_if_set(
                "stdio",
                "bearer_token_env_var",
                raw.bearer_token_env_var.as_ref(),
            )?;
            throw_if_set("stdio", "bearer_token", raw.bearer_token.as_ref())?;
            throw_if_set("stdio", "http_headers", raw.http_headers.as_ref())?;
            throw_if_set("stdio", "env_http_headers", raw.env_http_headers.as_ref())?;
            throw_if_set("stdio", "oauth_resource", raw.oauth_resource.as_ref())?;
            McpServerTransportConfig::Stdio {
                command,
                args: raw.args.clone().unwrap_or_default(),
                env: raw.env.clone(),
                env_vars: raw.env_vars.clone().unwrap_or_default(),
                cwd: raw.cwd.take(),
            }
        } else if let Some(url) = raw.url.clone() {
            throw_if_set("streamable_http", "args", raw.args.as_ref())?;
            throw_if_set("streamable_http", "env", raw.env.as_ref())?;
            throw_if_set("streamable_http", "env_vars", raw.env_vars.as_ref())?;
            throw_if_set("streamable_http", "cwd", raw.cwd.as_ref())?;
            throw_if_set("streamable_http", "bearer_token", raw.bearer_token.as_ref())?;
            McpServerTransportConfig::StreamableHttp {
                url,
                bearer_token_env_var: raw.bearer_token_env_var.clone(),
                http_headers: raw.http_headers.clone(),
                env_http_headers: raw.env_http_headers.take(),
            }
        } else {
            return Err(SerdeError::custom("invalid transport"));
        };

        Ok(Self {
            transport,
            startup_timeout_sec,
            tool_timeout_sec,
            enabled,
            required,
            disabled_reason: None,
            enabled_tools,
            disabled_tools,
            scopes,
            oauth_resource,
        })
    }
}

const fn default_enabled() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(untagged, deny_unknown_fields, rename_all = "snake_case")]
pub enum McpServerTransportConfig {
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
    StreamableHttp {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bearer_token_env_var: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        http_headers: Option<HashMap<String, String>>,
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolSuggestDiscoverableType {
    Connector,
    Plugin,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ToolSuggestDiscoverable {
    #[serde(rename = "type")]
    pub kind: ToolSuggestDiscoverableType,
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ToolSuggestConfig {
    #[serde(default)]
    pub discoverables: Vec<ToolSuggestDiscoverable>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct SkillConfig {
    pub path: AbsolutePathBuf,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct PluginConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct SkillsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundled: Option<BundledSkillsConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config: Vec<SkillConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct BundledSkillsConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

impl Default for BundledSkillsConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}
