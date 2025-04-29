use crate::flags::OPENAI_DEFAULT_MODEL;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPermission;
use crate::protocol::SandboxPolicy;
use dirs::home_dir;
use serde::Deserialize;
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

    /// Approval policy for executing commands.
    pub approval_policy: AskForApproval,

    pub sandbox_policy: SandboxPolicy,

    /// Disable server-side response storage (sends the full conversation
    /// context with every request). Currently necessary for OpenAI customers
    /// who have opted into Zero Data Retention (ZDR).
    pub disable_response_storage: bool,

    /// System instructions.
    pub instructions: Option<String>,
}

/// Base config deserialized from ~/.codex/config.toml.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct ConfigToml {
    /// Optional override of model selection.
    pub model: Option<String>,

    /// Default approval policy for executing commands.
    pub approval_policy: Option<AskForApproval>,

    pub sandbox_permissions: Option<Vec<SandboxPermission>>,

    /// Disable server-side response storage (sends the full conversation
    /// context with every request). Currently necessary for OpenAI customers
    /// who have opted into Zero Data Retention (ZDR).
    pub disable_response_storage: Option<bool>,

    /// System instructions.
    pub instructions: Option<String>,
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

/// Optional overrides for user configuration (e.g., from CLI flags).
#[derive(Default, Debug, Clone)]
pub struct ConfigOverrides {
    pub model: Option<String>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox_policy: Option<SandboxPolicy>,
    pub disable_response_storage: Option<bool>,
}

impl Config {
    /// Load configuration, optionally applying overrides (CLI flags). Merges
    /// ~/.codex/config.toml, ~/.codex/instructions.md, embedded defaults, and
    /// any values provided in `overrides` (highest precedence).
    pub fn load_with_overrides(overrides: ConfigOverrides) -> std::io::Result<Self> {
        let cfg: ConfigToml = ConfigToml::load_from_toml()?;
        tracing::warn!("Config parsed from config.toml: {cfg:?}");
        Ok(Self::load_from_base_config_with_overrides(cfg, overrides))
    }

    fn load_from_base_config_with_overrides(cfg: ConfigToml, overrides: ConfigOverrides) -> Self {
        // Instructions: user-provided instructions.md > embedded default.
        let instructions =
            Self::load_instructions().or_else(|| Some(EMBEDDED_INSTRUCTIONS.to_string()));

        // Destructure ConfigOverrides fully to ensure all overrides are applied.
        let ConfigOverrides {
            model,
            approval_policy,
            sandbox_policy,
            disable_response_storage,
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

        Self {
            model: model.or(cfg.model).unwrap_or_else(default_model),
            approval_policy: approval_policy
                .or(cfg.approval_policy)
                .unwrap_or_else(AskForApproval::default),
            sandbox_policy,
            disable_response_storage: disable_response_storage
                .or(cfg.disable_response_storage)
                .unwrap_or(false),
            instructions,
        }
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
