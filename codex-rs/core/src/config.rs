use std::path::PathBuf;

use dirs::home_dir;
use serde::Deserialize;

/// Embedded fallback instructions that mirror the TypeScript CLI’s default system prompt. These
/// are compiled into the binary so a clean install behaves correctly even if the user has not
/// created `~/.codex/instructions.md`.
const EMBEDDED_INSTRUCTIONS: &str = include_str!("../prompt.md");

#[derive(Default, Deserialize, Debug, Clone)]
pub struct Config {
    pub model: Option<String>,
    pub instructions: Option<String>,
}

impl Config {
    /// Load ~/.codex/config.toml and ~/.codex/instructions.md (if present).
    /// Returns `None` if neither file exists.
    pub fn load() -> Option<Self> {
        let mut cfg: Config = Self::load_from_toml().unwrap_or_default();

        // Highest precedence → user‑provided ~/.codex/instructions.md (if present)
        // Fallback           → embedded default instructions baked into the binary

        cfg.instructions =
            Self::load_instructions().or_else(|| Some(EMBEDDED_INSTRUCTIONS.to_string()));

        Some(cfg)
    }

    fn load_from_toml() -> Option<Self> {
        let mut p = codex_dir().ok()?;
        p.push("config.toml");
        let contents = std::fs::read_to_string(&p).ok()?;
        toml::from_str(&contents).ok()
    }

    fn load_instructions() -> Option<String> {
        let mut p = codex_dir().ok()?;
        p.push("instructions.md");
        std::fs::read_to_string(&p).ok()
    }
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
