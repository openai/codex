//! Sandbox configuration types.

use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

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

/// User/policy-level sandbox settings.
///
/// These settings control whether sandboxing is enabled and how bypass requests
/// are handled. Based on Claude Code's architecture where sandbox is **optional
/// and disabled by default**.
///
/// # Default Behavior
///
/// By default, sandbox is disabled (`enabled: false`), which means:
/// - Commands execute directly without any sandbox wrapping
/// - No Landlock/Seatbelt enforcement is applied
/// - `is_sandboxed()` returns `false`
///
/// This matches Claude Code's behavior where non-sandbox mode is the default
/// execution path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxSettings {
    /// Enable sandbox mode.
    ///
    /// When `false` (default), commands run directly without sandbox wrapping.
    /// When `true`, commands are wrapped with platform-specific sandbox
    /// (Landlock on Linux, Seatbelt on macOS).
    #[serde(default)]
    pub enabled: bool,

    /// Auto-approve bash commands when running in sandbox mode.
    ///
    /// When `true` (default), bash commands that would normally require
    /// approval can run automatically if the sandbox is enabled.
    #[serde(default = "default_true")]
    pub auto_allow_bash_if_sandboxed: bool,

    /// Allow the `dangerously_disable_sandbox` parameter to bypass sandbox.
    ///
    /// When `true` (default), individual commands can request sandbox bypass
    /// using the `dangerously_disable_sandbox` flag.
    #[serde(default = "default_true")]
    pub allow_unsandboxed_commands: bool,
}

fn default_true() -> bool {
    true
}

impl Default for SandboxSettings {
    fn default() -> Self {
        Self {
            enabled: false, // Sandbox disabled by default
            auto_allow_bash_if_sandboxed: true,
            allow_unsandboxed_commands: true,
        }
    }
}

impl SandboxSettings {
    /// Creates settings with sandbox enabled.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }

    /// Creates settings with sandbox disabled (same as default).
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Check if a command should run in sandbox mode.
    ///
    /// Returns `false` (no sandbox) if:
    /// 1. Sandbox is disabled (`!self.enabled`)
    /// 2. Bypass requested and allowed (`dangerously_disable_sandbox && allow_unsandboxed_commands`)
    /// 3. Command is empty
    ///
    /// # Arguments
    ///
    /// * `command` - The shell command to check
    /// * `dangerously_disable_sandbox` - Whether bypass was requested for this command
    pub fn is_sandboxed(&self, command: &str, dangerously_disable_sandbox: bool) -> bool {
        // 1. Sandbox disabled → no sandbox
        if !self.enabled {
            return false;
        }

        // 2. Bypass requested and allowed
        if dangerously_disable_sandbox && self.allow_unsandboxed_commands {
            return false;
        }

        // 3. Empty command
        if command.trim().is_empty() {
            return false;
        }

        // Otherwise: sandbox if enabled
        true
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

    // ==========================================================================
    // SandboxSettings tests
    // ==========================================================================

    #[test]
    fn test_sandbox_settings_default_disabled() {
        let settings = SandboxSettings::default();
        assert!(!settings.enabled);
        assert!(settings.auto_allow_bash_if_sandboxed);
        assert!(settings.allow_unsandboxed_commands);
    }

    #[test]
    fn test_sandbox_settings_enabled_constructor() {
        let settings = SandboxSettings::enabled();
        assert!(settings.enabled);
        assert!(settings.auto_allow_bash_if_sandboxed);
        assert!(settings.allow_unsandboxed_commands);
    }

    #[test]
    fn test_sandbox_settings_disabled_constructor() {
        let settings = SandboxSettings::disabled();
        assert!(!settings.enabled);
    }

    #[test]
    fn test_is_sandboxed_disabled_by_default() {
        let settings = SandboxSettings::default();
        // When sandbox is disabled, all commands return false
        assert!(!settings.is_sandboxed("echo hello", false));
        assert!(!settings.is_sandboxed("rm -rf /", false));
        assert!(!settings.is_sandboxed("echo hello", true));
    }

    #[test]
    fn test_is_sandboxed_enabled() {
        let settings = SandboxSettings::enabled();
        // When sandbox is enabled, normal commands return true
        assert!(settings.is_sandboxed("echo hello", false));
        assert!(settings.is_sandboxed("rm -rf /", false));
    }

    #[test]
    fn test_is_sandboxed_bypass_allowed() {
        let settings = SandboxSettings::enabled();
        // When bypass is requested and allowed, returns false
        assert!(!settings.is_sandboxed("echo hello", true));
    }

    #[test]
    fn test_is_sandboxed_bypass_disallowed() {
        let mut settings = SandboxSettings::enabled();
        settings.allow_unsandboxed_commands = false;
        // When bypass is requested but not allowed, returns true
        assert!(settings.is_sandboxed("echo hello", true));
    }

    #[test]
    fn test_is_sandboxed_empty_command() {
        let settings = SandboxSettings::enabled();
        // Empty commands are never sandboxed
        assert!(!settings.is_sandboxed("", false));
        assert!(!settings.is_sandboxed("   ", false));
        assert!(!settings.is_sandboxed("\t\n", false));
    }

    #[test]
    fn test_sandbox_settings_serde_roundtrip() {
        let settings = SandboxSettings {
            enabled: true,
            auto_allow_bash_if_sandboxed: false,
            allow_unsandboxed_commands: false,
        };

        let json = serde_json::to_string(&settings).expect("serialize");
        let parsed: SandboxSettings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.enabled, settings.enabled);
        assert_eq!(
            parsed.auto_allow_bash_if_sandboxed,
            settings.auto_allow_bash_if_sandboxed
        );
        assert_eq!(
            parsed.allow_unsandboxed_commands,
            settings.allow_unsandboxed_commands
        );
    }

    #[test]
    fn test_sandbox_settings_from_empty_json() {
        // Empty JSON should use defaults
        let settings: SandboxSettings = serde_json::from_str("{}").expect("parse");
        assert!(!settings.enabled);
        assert!(settings.auto_allow_bash_if_sandboxed);
        assert!(settings.allow_unsandboxed_commands);
    }

    #[test]
    fn test_sandbox_settings_partial_json() {
        // Only enabled=true, rest should default
        let settings: SandboxSettings = serde_json::from_str(r#"{"enabled":true}"#).expect("parse");
        assert!(settings.enabled);
        assert!(settings.auto_allow_bash_if_sandboxed);
        assert!(settings.allow_unsandboxed_commands);
    }
}
