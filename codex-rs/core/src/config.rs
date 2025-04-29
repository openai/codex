use crate::flags::OPENAI_DEFAULT_MODEL;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;
use dirs::home_dir;
use serde::Deserialize;
use std::path::PathBuf;

/// Embedded fallback instructions that mirror the TypeScript CLI’s default
/// system prompt. These are compiled into the binary so a clean install behaves
/// correctly even if the user has not created `~/.codex/instructions.md`.
const EMBEDDED_INSTRUCTIONS: &str = include_str!("../prompt.md");

/// Application configuration loaded from disk and merged with overrides.
#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    /// Optional override of model selection.
    #[serde(default = "default_model")]
    pub model: String,
    /// Default approval policy for executing commands.
    #[serde(default)]
    pub approval_policy: AskForApproval,
    #[serde(default)]
    pub sandbox_policy: SandboxPolicy,

    /// Disable server-side response storage (sends the full conversation
    /// context with every request). Currently necessary for OpenAI customers
    /// who have opted into Zero Data Retention (ZDR).
    #[serde(default)]
    pub disable_response_storage: bool,

    /// System instructions.
    pub instructions: Option<String>,
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
        let mut cfg: Config = Self::load_from_toml()?;
        tracing::warn!("Config parsed from config.toml: {cfg:?}");

        // Instructions: user-provided instructions.md > embedded default.
        cfg.instructions =
            Self::load_instructions().or_else(|| Some(EMBEDDED_INSTRUCTIONS.to_string()));

        // Destructure ConfigOverrides fully to ensure all overrides are applied.
        let ConfigOverrides {
            model,
            approval_policy,
            sandbox_policy,
            disable_response_storage,
        } = overrides;

        if let Some(model) = model {
            cfg.model = model;
        }
        if let Some(approval_policy) = approval_policy {
            cfg.approval_policy = approval_policy;
        }
        if let Some(sandbox_policy) = sandbox_policy {
            cfg.sandbox_policy = sandbox_policy;
        }
        if let Some(disable_response_storage) = disable_response_storage {
            cfg.disable_response_storage = disable_response_storage;
        }
        Ok(cfg)
    }

    /// Attempt to parse the file at `~/.codex/config.toml` into a Config.
    fn load_from_toml() -> std::io::Result<Self> {
        let config_toml_path = codex_dir()?.join("config.toml");
        match std::fs::read_to_string(&config_toml_path) {
            Ok(contents) => toml::from_str::<Self>(&contents).map_err(|e| {
                tracing::error!("Failed to parse config.toml: {e}");
                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!("config.toml not found, using defaults");
                Ok(Self::load_default_config())
            }
            Err(e) => {
                tracing::error!("Failed to read config.toml: {e}");
                Err(e)
            }
        }
    }

    /// Meant to be used exclusively for tests: load_with_overrides() should be
    /// used in all other cases.
    pub fn load_default_config_for_test() -> Self {
        Self::load_default_config()
    }

    fn load_default_config() -> Self {
        // Load from an empty string to exercise #[serde(default)] to
        // get the default values for each field.
        toml::from_str::<Self>("").expect("empty string should parse as TOML")
    }

    fn load_instructions() -> Option<String> {
        let mut p = codex_dir().ok()?;
        p.push("instructions.md");
        std::fs::read_to_string(&p).ok()
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
