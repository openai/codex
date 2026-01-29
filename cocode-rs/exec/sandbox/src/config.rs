//! Sandbox configuration types.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Sandbox execution mode controlling filesystem and network access.
///
/// This is the sandbox crate's own mode enum, distinct from `cocode_protocol::SandboxMode`
/// which is focused on protocol-level configuration. This enum maps the protocol mode
/// into enforcement categories.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    /// No sandbox enforcement; all operations are allowed.
    #[default]
    None,
    /// Read-only mode; file writes are blocked.
    ReadOnly,
    /// Strict mode; only explicitly allowed paths are accessible,
    /// and network is blocked unless explicitly allowed.
    Strict,
}

/// Configuration for the sandbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// The sandbox enforcement mode.
    #[serde(default)]
    pub mode: SandboxMode,
    /// Paths that are explicitly allowed for read/write access.
    #[serde(default)]
    pub allowed_paths: Vec<PathBuf>,
    /// Paths that are explicitly denied (takes precedence over allowed).
    #[serde(default)]
    pub denied_paths: Vec<PathBuf>,
    /// Whether network access is allowed.
    #[serde(default)]
    pub allow_network: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            mode: SandboxMode::default(),
            allowed_paths: Vec::new(),
            denied_paths: Vec::new(),
            allow_network: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_mode_default() {
        assert_eq!(SandboxMode::default(), SandboxMode::None);
    }

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxConfig::default();
        assert_eq!(config.mode, SandboxMode::None);
        assert!(config.allowed_paths.is_empty());
        assert!(config.denied_paths.is_empty());
        assert!(!config.allow_network);
    }

    #[test]
    fn test_sandbox_mode_serde_roundtrip() {
        for mode in [
            SandboxMode::None,
            SandboxMode::ReadOnly,
            SandboxMode::Strict,
        ] {
            let json = serde_json::to_string(&mode).expect("serialize");
            let parsed: SandboxMode = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, mode);
        }
    }

    #[test]
    fn test_sandbox_mode_kebab_case() {
        assert_eq!(
            serde_json::to_string(&SandboxMode::None).expect("serialize"),
            "\"none\""
        );
        assert_eq!(
            serde_json::to_string(&SandboxMode::ReadOnly).expect("serialize"),
            "\"read-only\""
        );
        assert_eq!(
            serde_json::to_string(&SandboxMode::Strict).expect("serialize"),
            "\"strict\""
        );
    }

    #[test]
    fn test_sandbox_config_serde_roundtrip() {
        let config = SandboxConfig {
            mode: SandboxMode::Strict,
            allowed_paths: vec![PathBuf::from("/home/user/project")],
            denied_paths: vec![PathBuf::from("/etc/passwd")],
            allow_network: true,
        };

        let json = serde_json::to_string(&config).expect("serialize");
        let parsed: SandboxConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.mode, SandboxMode::Strict);
        assert_eq!(parsed.allowed_paths.len(), 1);
        assert_eq!(parsed.denied_paths.len(), 1);
        assert!(parsed.allow_network);
    }

    #[test]
    fn test_sandbox_config_from_empty_json() {
        let config: SandboxConfig = serde_json::from_str("{}").expect("parse");
        assert_eq!(config.mode, SandboxMode::None);
        assert!(config.allowed_paths.is_empty());
        assert!(config.denied_paths.is_empty());
        assert!(!config.allow_network);
    }

    #[test]
    fn test_sandbox_config_partial_json() {
        let config: SandboxConfig = serde_json::from_str(r#"{"mode":"strict"}"#).expect("parse");
        assert_eq!(config.mode, SandboxMode::Strict);
        assert!(config.allowed_paths.is_empty());
        assert!(!config.allow_network);
    }
}
