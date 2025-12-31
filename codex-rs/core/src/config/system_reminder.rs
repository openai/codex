//! System reminder configuration.
//!
//! Configuration for the system reminder attachment system.

use serde::Deserialize;
use serde::Serialize;

// ============================================
// Nested Memory Configuration
// ============================================

/// Nested memory configuration.
///
/// Controls automatic discovery and injection of AGENTS.md and rules files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NestedMemoryConfig {
    /// Enable nested memory discovery (default: true).
    pub enabled: bool,
    /// Enable user rules from ~/.codex/rules/ (default: true).
    pub user_rules: bool,
    /// Enable project AGENTS.md files (default: true).
    pub project_settings: bool,
    /// Enable local AGENTS.local.md files (default: true).
    pub local_settings: bool,
    /// Maximum content size in bytes (default: 40000).
    pub max_content_size: i32,
    /// Maximum lines per file (default: 3000).
    pub max_lines: i32,
    /// Maximum @import recursion depth (default: 5).
    pub max_import_depth: i32,
}

impl Default for NestedMemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            user_rules: true,
            project_settings: true,
            local_settings: true,
            max_content_size: 40000,
            max_lines: 3000,
            max_import_depth: 5,
        }
    }
}

/// Minimum severity level for LSP diagnostics to be injected.
///
/// Only diagnostics at or above this severity level will be included in system reminders.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LspDiagnosticsMinSeverity {
    /// Only inject errors (most restrictive, default for production).
    #[default]
    Error,
    /// Inject errors and warnings.
    Warning,
    /// Inject errors, warnings, and info messages.
    Info,
    /// Inject all diagnostics including hints (least restrictive).
    Hint,
}

/// Output style configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct OutputStyleConfig {
    /// Currently selected output style name (default: "default").
    pub current_style: String,
    /// Enable output style attachment (default: true).
    pub enabled: bool,
}

impl Default for OutputStyleConfig {
    fn default() -> Self {
        Self {
            current_style: "default".to_string(),
            enabled: true,
        }
    }
}

/// System reminder configuration.
///
/// Controls the behavior of the system reminder attachment system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SystemReminderConfig {
    /// Master enable/disable (default: true).
    pub enabled: bool,

    /// User-defined critical instruction (always injected when set).
    /// Matches criticalSystemReminder_EXPERIMENTAL in Claude Code.
    #[serde(default)]
    pub critical_instruction: Option<String>,

    /// Per-attachment enable/disable (granular control).
    #[serde(default)]
    pub attachments: AttachmentSettings,

    /// Custom timeout in milliseconds (default: 1000).
    #[serde(default)]
    pub timeout_ms: Option<i64>,

    /// Nested memory configuration.
    #[serde(default)]
    pub nested_memory: NestedMemoryConfig,

    /// Output style configuration.
    #[serde(default)]
    pub output_style: OutputStyleConfig,
}

impl Default for SystemReminderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            critical_instruction: None,
            attachments: AttachmentSettings::default(),
            timeout_ms: Some(1000),
            nested_memory: NestedMemoryConfig::default(),
            output_style: OutputStyleConfig::default(),
        }
    }
}

/// Per-attachment enable/disable settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AttachmentSettings {
    /// Critical instruction reminder (default: true).
    pub critical_instruction: bool,
    /// Plan mode instructions (default: true).
    pub plan_mode: bool,
    /// Plan tool reminder - update_plan tool usage (default: true).
    pub plan_tool_reminder: bool,
    /// File change notifications (default: true).
    pub changed_files: bool,
    /// Background task status (default: true).
    pub background_task: bool,
    /// LSP diagnostics notifications (default: true).
    pub lsp_diagnostics: bool,
    /// Minimum severity for LSP diagnostics (default: error only).
    #[serde(default)]
    pub lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity,
    /// Nested memory - auto-included AGENTS.md and rules (default: true).
    pub nested_memory: bool,
    /// @mentioned files - auto-include files from @file syntax (default: true).
    #[serde(default = "default_true")]
    pub at_mentioned_files: bool,
    /// Agent mentions - invoke agents from @agent-type syntax (default: true).
    #[serde(default = "default_true")]
    pub agent_mentions: bool,
    /// Output style instructions (default: true).
    #[serde(default = "default_true")]
    pub output_style: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AttachmentSettings {
    fn default() -> Self {
        Self {
            critical_instruction: true,
            plan_mode: true,
            plan_tool_reminder: true,
            changed_files: true,
            background_task: true,
            lsp_diagnostics: true,
            lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity::default(),
            nested_memory: true,
            at_mentioned_files: true,
            agent_mentions: true,
            output_style: true,
        }
    }
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_reminder_config_default() {
        let config = SystemReminderConfig::default();
        assert!(config.enabled);
        assert!(config.critical_instruction.is_none());
        assert_eq!(config.timeout_ms, Some(1000));
    }

    #[test]
    fn test_attachment_settings_default() {
        let settings = AttachmentSettings::default();
        assert!(settings.critical_instruction);
        assert!(settings.plan_mode);
        assert!(settings.plan_tool_reminder);
        assert!(settings.changed_files);
        assert!(settings.background_task);
        assert!(settings.lsp_diagnostics);
        assert!(settings.nested_memory);
    }

    #[test]
    fn test_nested_memory_config_default() {
        let config = NestedMemoryConfig::default();
        assert!(config.enabled);
        assert!(config.user_rules);
        assert!(config.project_settings);
        assert!(config.local_settings);
        assert_eq!(config.max_content_size, 40000);
        assert_eq!(config.max_lines, 3000);
        assert_eq!(config.max_import_depth, 5);
    }

    #[test]
    fn test_config_deserialize() {
        let toml = r#"
            enabled = true
            critical_instruction = "Always run tests"
            timeout_ms = 2000

            [attachments]
            critical_instruction = true
            plan_mode = false
            plan_tool_reminder = true
            changed_files = false
            background_task = true
        "#;

        let config: SystemReminderConfig = toml::from_str(toml).unwrap();
        assert!(config.enabled);
        assert_eq!(
            config.critical_instruction,
            Some("Always run tests".to_string())
        );
        assert_eq!(config.timeout_ms, Some(2000));
        assert!(config.attachments.critical_instruction);
        assert!(!config.attachments.plan_mode);
        assert!(config.attachments.plan_tool_reminder);
        assert!(!config.attachments.changed_files);
        assert!(config.attachments.background_task);
    }

    #[test]
    fn test_config_deserialize_partial() {
        let toml = r#"
            enabled = false
        "#;

        let config: SystemReminderConfig = toml::from_str(toml).unwrap();
        assert!(!config.enabled);
        // Defaults should apply
        assert!(config.critical_instruction.is_none());
        assert!(config.attachments.plan_mode);
    }

    #[test]
    fn test_config_serialize() {
        let config = SystemReminderConfig {
            enabled: true,
            critical_instruction: Some("Test instruction".to_string()),
            attachments: AttachmentSettings {
                critical_instruction: true,
                plan_mode: false,
                ..Default::default()
            },
            timeout_ms: Some(1500),
            ..Default::default()
        };

        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("enabled = true"));
        assert!(toml_str.contains("Test instruction"));
    }
}
