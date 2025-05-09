use crate::flags::OPENAI_DEFAULT_MODEL;
use crate::mcp_server_config::McpServerConfig;
use crate::model_provider_info::ModelProviderInfo;
use crate::model_provider_info::built_in_model_providers;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPermission;
use crate::protocol::SandboxPolicy;
use dirs::home_dir;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// Embedded fallback instructions that mirror the TypeScript CLI’s default
/// system prompt. These are compiled into the binary so a clean install behaves
/// correctly even if the user has not created `~/.codex/instructions.md`.
const EMBEDDED_INSTRUCTIONS: &str = include_str!("../prompt.md");

/// Application configuration loaded from disk and merged with overrides.
#[derive(Debug, Clone)]
pub struct Config {
    /// Optional override of model selection.
    pub model: String,

    /// Key into the model_providers map that specifies which provider to use.
    pub model_provider_id: String,

    /// Info needed to make an API request to the model.
    pub model_provider: ModelProviderInfo,

    /// Approval policy for executing commands.
    pub approval_policy: AskForApproval,

    pub sandbox_policy: SandboxPolicy,

    /// Disable server-side response storage (sends the full conversation
    /// context with every request). Currently necessary for OpenAI customers
    /// who have opted into Zero Data Retention (ZDR).
    pub disable_response_storage: bool,

    /// System instructions.
    pub instructions: Option<String>,

    /// Optional external notifier command. When set, Codex will spawn this
    /// program after each completed *turn* (i.e. when the agent finishes
    /// processing a user submission). The value must be the full command
    /// broken into argv tokens **without** the trailing JSON argument - Codex
    /// appends one extra argument containing a JSON payload describing the
    /// event.
    ///
    /// Example `~/.codex/config.toml` snippet:
    ///
    /// ```toml
    /// notify = ["notify-send", "Codex"]
    /// ```
    ///
    /// which will be invoked as:
    ///
    /// ```shell
    /// notify-send Codex '{"type":"agent-turn-complete","turn-id":"12345"}'
    /// ```
    ///
    /// If unset the feature is disabled.
    pub notify: Option<Vec<String>>,

    /// The directory that should be treated as the current working directory
    /// for the session. All relative paths inside the business-logic layer are
    /// resolved against this path.
    pub cwd: PathBuf,

    /// Definition for MCP servers that Codex can reach out to for tool calls.
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// Combined provider map (defaults merged with user-defined overrides).
    pub model_providers: HashMap<String, ModelProviderInfo>,
}

/// Base config deserialized from ~/.codex/config.toml.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct ConfigToml {
    /// Optional override of model selection.
    pub model: Option<String>,

    /// Provider to use from the model_providers map.
    pub model_provider: Option<String>,

    /// Default approval policy for executing commands.
    pub approval_policy: Option<AskForApproval>,

    // The `default` attribute ensures that the field is treated as `None` when
    // the key is omitted from the TOML. Without it, Serde treats the field as
    // required because we supply a custom deserializer.
    #[serde(default, deserialize_with = "deserialize_sandbox_permissions")]
    pub sandbox_permissions: Option<Vec<SandboxPermission>>,

    /// Disable server-side response storage (sends the full conversation
    /// context with every request). Currently necessary for OpenAI customers
    /// who have opted into Zero Data Retention (ZDR).
    pub disable_response_storage: Option<bool>,

    /// Optional external command to spawn for end-user notifications.
    #[serde(default)]
    pub notify: Option<Vec<String>>,

    /// System instructions.
    pub instructions: Option<String>,

    /// Definition for MCP servers that Codex can reach out to for tool calls.
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// User-defined provider entries that extend/override the built-in list.
    #[serde(default)]
    pub model_providers: HashMap<String, ModelProviderInfo>,
}

impl ConfigToml {
    /// Attempt to parse the file at `~/.codex/config.toml`. If it does not
    /// exist, return a default config. Though if it exists and cannot be
    /// parsed, report that to the user and force them to fix it.
    fn load_from_toml() -> std::io::Result<Self> {
        let config_toml_path = codex_dir()?.join("config.toml");
        match std::fs::read_to_string(&config_toml_path) {
            Ok(contents) => toml::from_str::<Self>(&contents).map_err(|e| {
                tracing::error!("Failed to parse config.toml: {e}");
                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!("config.toml not found, using defaults");
                Ok(Self::default())
            }
            Err(e) => {
                tracing::error!("Failed to read config.toml: {e}");
                Err(e)
            }
        }
    }
}

fn deserialize_sandbox_permissions<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<SandboxPermission>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let permissions: Option<Vec<String>> = Option::deserialize(deserializer)?;

    match permissions {
        Some(raw_permissions) => {
            let base_path = codex_dir().map_err(serde::de::Error::custom)?;

            let converted = raw_permissions
                .into_iter()
                .map(|raw| {
                    parse_sandbox_permission_with_base_path(&raw, base_path.clone())
                        .map_err(serde::de::Error::custom)
                })
                .collect::<Result<Vec<_>, D::Error>>()?;

            Ok(Some(converted))
        }
        None => Ok(None),
    }
}

/// Optional overrides for user configuration (e.g., from CLI flags).
#[derive(Default, Debug, Clone)]
pub struct ConfigOverrides {
    pub model: Option<String>,
    pub cwd: Option<PathBuf>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox_policy: Option<SandboxPolicy>,
    pub disable_response_storage: Option<bool>,
    pub provider: Option<String>,
}

impl Config {
    /// Load configuration, optionally applying overrides (CLI flags). Merges
    /// ~/.codex/config.toml, ~/.codex/instructions.md, embedded defaults, and
    /// any values provided in `overrides` (highest precedence).
    pub fn load_with_overrides(overrides: ConfigOverrides) -> std::io::Result<Self> {
        let cfg: ConfigToml = ConfigToml::load_from_toml()?;
        tracing::warn!("Config parsed from config.toml: {cfg:?}");
        Self::load_from_base_config_with_overrides(cfg, overrides)
    }

    fn load_from_base_config_with_overrides(
        cfg: ConfigToml,
        overrides: ConfigOverrides,
    ) -> std::io::Result<Self> {
        // Instructions: user-provided instructions.md > embedded default.
        let instructions =
            Self::load_instructions().or_else(|| Some(EMBEDDED_INSTRUCTIONS.to_string()));

        // Destructure ConfigOverrides fully to ensure all overrides are applied.
        let ConfigOverrides {
            model,
            cwd,
            approval_policy,
            sandbox_policy,
            disable_response_storage,
            provider,
        } = overrides;

        let sandbox_policy = match sandbox_policy {
            Some(sandbox_policy) => sandbox_policy,
            None => {
                // Derive a SandboxPolicy from the permissions in the config.
                match cfg.sandbox_permissions {
                    // Note this means the user can explicitly set permissions
                    // to the empty list in the config file, granting it no
                    // permissions whatsoever.
                    Some(permissions) => SandboxPolicy::from(permissions),
                    // Default to read only rather than completely locked down.
                    None => SandboxPolicy::new_read_only_policy(),
                }
            }
        };

        let mut model_providers = built_in_model_providers();
        // Merge user-defined providers into the built-in list.
        for (key, provider) in cfg.model_providers.into_iter() {
            model_providers.entry(key).or_insert(provider);
        }

        let model_provider_id = provider
            .or(cfg.model_provider)
            .unwrap_or_else(|| "openai".to_string());
        let model_provider = model_providers
            .get(&model_provider_id)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Model provider `{model_provider_id}` not found"),
                )
            })?
            .clone();

        let config = Self {
            model: model.or(cfg.model).unwrap_or_else(default_model),
            model_provider_id,
            model_provider,
            cwd: cwd.map_or_else(
                || {
                    tracing::info!("cwd not set, using current dir");
                    std::env::current_dir().expect("cannot determine current dir")
                },
                |p| {
                    if p.is_absolute() {
                        p
                    } else {
                        // Resolve relative paths against the current working directory.
                        tracing::info!("cwd is relative, resolving against current dir");
                        let mut cwd = std::env::current_dir().expect("cannot determine cwd");
                        cwd.push(p);
                        cwd
                    }
                },
            ),
            approval_policy: approval_policy
                .or(cfg.approval_policy)
                .unwrap_or_else(AskForApproval::default),
            sandbox_policy,
            disable_response_storage: disable_response_storage
                .or(cfg.disable_response_storage)
                .unwrap_or(false),
            notify: cfg.notify,
            instructions,
            mcp_servers: cfg.mcp_servers,
            model_providers,
        };
        Ok(config)
    }

    fn load_instructions() -> Option<String> {
        let mut p = codex_dir().ok()?;
        p.push("instructions.md");
        std::fs::read_to_string(&p).ok()
    }

    /// Meant to be used exclusively for tests: `load_with_overrides()` should
    /// be used in all other cases.
    pub fn load_default_config_for_test() -> Self {
        Self::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
        )
        .expect("defaults for test should always succeed")
    }
}

fn default_model() -> String {
    OPENAI_DEFAULT_MODEL.to_string()
}

/// Returns the path to the Codex configuration directory, which is `~/.codex`.
/// Does not verify that the directory exists.
pub fn codex_dir() -> std::io::Result<PathBuf> {
    let mut p = home_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not find home directory",
        )
    })?;
    p.push(".codex");
    Ok(p)
}

/// Returns the path to the folder where Codex logs are stored. Does not verify
/// that the directory exists.
pub fn log_dir() -> std::io::Result<PathBuf> {
    let mut p = codex_dir()?;
    p.push("log");
    Ok(p)
}

pub fn parse_sandbox_permission_with_base_path(
    raw: &str,
    base_path: PathBuf,
) -> std::io::Result<SandboxPermission> {
    use SandboxPermission::*;

    if let Some(path) = raw.strip_prefix("disk-write-folder=") {
        return if path.is_empty() {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "--sandbox-permission disk-write-folder=<PATH> requires a non-empty PATH",
            ))
        } else {
            use path_absolutize::*;

            let file = PathBuf::from(path);
            let absolute_path = if file.is_relative() {
                file.absolutize_from(base_path)
            } else {
                file.absolutize()
            }
            .map(|path| path.into_owned())?;
            Ok(DiskWriteFolder {
                folder: absolute_path,
            })
        };
    }

    match raw {
        "disk-full-read-access" => Ok(DiskFullReadAccess),
        "disk-write-platform-user-temp-folder" => Ok(DiskWritePlatformUserTempFolder),
        "disk-write-platform-global-temp-folder" => Ok(DiskWritePlatformGlobalTempFolder),
        "disk-write-cwd" => Ok(DiskWriteCwd),
        "disk-full-write-access" => Ok(DiskFullWriteAccess),
        "network-full-access" => Ok(NetworkFullAccess),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "`{raw}` is not a recognised permission.\nRun with `--help` to see the accepted values."
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that the `sandbox_permissions` field on `ConfigToml` correctly
    /// differentiates between a value that is completely absent in the
    /// provided TOML (i.e. `None`) and one that is explicitly specified as an
    /// empty array (i.e. `Some(vec![])`). This ensures that downstream logic
    /// that treats these two cases differently (default read-only policy vs a
    /// fully locked-down sandbox) continues to function.
    #[test]
    fn test_sandbox_permissions_none_vs_empty_vec() {
        // Case 1: `sandbox_permissions` key is *absent* from the TOML source.
        let toml_source_without_key = "";
        let cfg_without_key: ConfigToml = toml::from_str(toml_source_without_key)
            .expect("TOML deserialization without key should succeed");
        assert!(cfg_without_key.sandbox_permissions.is_none());

        // Case 2: `sandbox_permissions` is present but set to an *empty array*.
        let toml_source_with_empty = "sandbox_permissions = []";
        let cfg_with_empty: ConfigToml = toml::from_str(toml_source_with_empty)
            .expect("TOML deserialization with empty array should succeed");
        assert_eq!(Some(vec![]), cfg_with_empty.sandbox_permissions);

        // Case 3: `sandbox_permissions` contains a non-empty list of valid values.
        let toml_source_with_values = r#"
            sandbox_permissions = ["disk-full-read-access", "network-full-access"]
        "#;
        let cfg_with_values: ConfigToml = toml::from_str(toml_source_with_values)
            .expect("TOML deserialization with valid permissions should succeed");

        assert_eq!(
            Some(vec![
                SandboxPermission::DiskFullReadAccess,
                SandboxPermission::NetworkFullAccess
            ]),
            cfg_with_values.sandbox_permissions
        );
    }

    /// Deserializing a TOML string containing an *invalid* permission should
    /// fail with a helpful error rather than silently defaulting or
    /// succeeding.
    #[test]
    fn test_sandbox_permissions_illegal_value() {
        let toml_bad = r#"sandbox_permissions = ["not-a-real-permission"]"#;

        let err = toml::from_str::<ConfigToml>(toml_bad)
            .expect_err("Deserialization should fail for invalid permission");

        // Make sure the error message contains the invalid value so users have
        // useful feedback.
        let msg = err.to_string();
        assert!(msg.contains("not-a-real-permission"));
    }
}
