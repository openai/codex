//! Configuration for system reminders.
//!
//! This module provides the configuration structures for controlling
//! which system reminders are enabled and how they behave.

use serde::Deserialize;
use serde::Serialize;

/// Configuration for the system reminder system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SystemReminderConfig {
    /// Master enable/disable flag.
    pub enabled: bool,

    /// Timeout in milliseconds for each generator (default: 1000ms).
    pub timeout_ms: i64,

    /// Per-attachment enable/disable settings.
    pub attachments: AttachmentSettings,

    /// Nested memory configuration.
    pub nested_memory: NestedMemoryConfig,

    /// User-defined critical instruction (injected every turn).
    pub critical_instruction: Option<String>,

    /// Output style configuration.
    pub output_style: OutputStyleConfig,
}

impl Default for SystemReminderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_ms: 1000,
            attachments: AttachmentSettings::default(),
            nested_memory: NestedMemoryConfig::default(),
            critical_instruction: None,
            output_style: OutputStyleConfig::default(),
        }
    }
}

/// Per-attachment enable/disable settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AttachmentSettings {
    /// Enable critical instruction injection.
    pub critical_instruction: bool,
    /// Enable plan mode enter instructions.
    pub plan_mode_enter: bool,
    /// Enable plan tool reminder.
    pub plan_tool_reminder: bool,
    /// Enable plan mode exit instructions.
    pub plan_mode_exit: bool,
    /// Enable changed files detection.
    pub changed_files: bool,
    /// Enable background task status.
    pub background_task: bool,
    /// Enable LSP diagnostics.
    pub lsp_diagnostics: bool,
    /// Enable nested memory (CLAUDE.md discovery).
    pub nested_memory: bool,
    /// Enable available skills listing.
    pub available_skills: bool,
    /// Enable @file mentioned files.
    pub at_mentioned_files: bool,
    /// Enable @agent mentions.
    pub agent_mentions: bool,
    /// Enable output style instructions.
    pub output_style: bool,
    /// Enable todo/task reminders.
    pub todo_reminders: bool,
    /// Enable delegate mode instructions.
    pub delegate_mode: bool,
    /// Enable collaboration notifications.
    pub collab_notifications: bool,
    /// Enable plan verification reminders.
    pub plan_verification: bool,
    /// Enable token usage display.
    pub token_usage: bool,

    /// Minimum severity for LSP diagnostics (error, warning, info, hint).
    pub lsp_diagnostics_min_severity: DiagnosticSeverity,
}

impl Default for AttachmentSettings {
    fn default() -> Self {
        Self {
            critical_instruction: true,
            plan_mode_enter: true,
            plan_tool_reminder: true,
            plan_mode_exit: true,
            changed_files: true,
            background_task: true,
            lsp_diagnostics: true,
            nested_memory: true,
            available_skills: true,
            at_mentioned_files: true,
            agent_mentions: true,
            output_style: true,
            todo_reminders: true,
            delegate_mode: true,
            collab_notifications: true,
            plan_verification: true,
            token_usage: true,
            lsp_diagnostics_min_severity: DiagnosticSeverity::Warning,
        }
    }
}

/// Diagnostic severity levels for LSP filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    /// Only show errors.
    Error = 1,
    /// Show errors and warnings.
    #[default]
    Warning = 2,
    /// Show errors, warnings, and info.
    Info = 3,
    /// Show all diagnostics including hints.
    Hint = 4,
}

/// Configuration for nested memory discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NestedMemoryConfig {
    /// Enable auto-discovery of CLAUDE.md files.
    pub enabled: bool,
    /// Maximum content size in bytes (default: 40KB).
    pub max_content_bytes: i64,
    /// Maximum number of lines (default: 3000).
    pub max_lines: i32,
    /// Maximum import depth for nested includes (default: 5).
    pub max_import_depth: i32,
    /// File patterns to auto-discover.
    pub patterns: Vec<String>,
}

impl Default for NestedMemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_content_bytes: 40 * 1024, // 40KB
            max_lines: 3000,
            max_import_depth: 5,
            patterns: vec![
                "CLAUDE.md".to_string(),
                "AGENTS.md".to_string(),
                ".claude/settings.json".to_string(),
            ],
        }
    }
}

/// Configuration for output style instructions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputStyleConfig {
    /// Enable output style instructions.
    pub enabled: bool,
    /// Custom output style instruction.
    pub instruction: Option<String>,
}

impl Default for OutputStyleConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            instruction: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SystemReminderConfig::default();
        assert!(config.enabled);
        assert_eq!(config.timeout_ms, 1000);
        assert!(config.attachments.changed_files);
        assert!(config.attachments.plan_mode_enter);
        assert!(config.nested_memory.enabled);
    }

    #[test]
    fn test_diagnostic_severity_ordering() {
        assert!(DiagnosticSeverity::Error < DiagnosticSeverity::Warning);
        assert!(DiagnosticSeverity::Warning < DiagnosticSeverity::Info);
        assert!(DiagnosticSeverity::Info < DiagnosticSeverity::Hint);
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = SystemReminderConfig {
            enabled: true,
            timeout_ms: 2000,
            critical_instruction: Some("Always be helpful".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&config).expect("serialize");
        let parsed: SystemReminderConfig = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed.enabled, config.enabled);
        assert_eq!(parsed.timeout_ms, config.timeout_ms);
        assert_eq!(parsed.critical_instruction, config.critical_instruction);
    }

    #[test]
    fn test_nested_memory_defaults() {
        let config = NestedMemoryConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_content_bytes, 40 * 1024);
        assert_eq!(config.max_lines, 3000);
        assert_eq!(config.max_import_depth, 5);
        assert!(config.patterns.contains(&"CLAUDE.md".to_string()));
    }
}
