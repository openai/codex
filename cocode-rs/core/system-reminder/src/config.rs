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

    /// @mentioned files configuration.
    pub at_mentioned_files: AtMentionedFilesConfig,

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
            at_mentioned_files: AtMentionedFilesConfig::default(),
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
    /// Enable invoked skills injection.
    pub invoked_skills: bool,
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
    /// Enable security guidelines (dual-placed for compaction survival).
    pub security_guidelines: bool,

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
            invoked_skills: true,
            output_style: true,
            todo_reminders: true,
            delegate_mode: true,
            collab_notifications: true,
            plan_verification: true,
            token_usage: true,
            security_guidelines: true,
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

/// Configuration for @mentioned files.
///
/// Controls limits for file content injection when users use @file mentions.
/// Aligns with Claude Code's Read tool limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AtMentionedFilesConfig {
    /// Maximum file size in bytes (default: 100KB, matches codex-rs).
    pub max_file_size: i64,
    /// Maximum number of lines to read (default: 2000, matches Read tool).
    pub max_lines: i32,
    /// Maximum line length before truncation (default: 2000 chars).
    pub max_line_length: i32,
}

impl Default for AtMentionedFilesConfig {
    fn default() -> Self {
        Self {
            max_file_size: 100 * 1024, // 100KB (codex-rs default)
            max_lines: 2000,           // Read tool default
            max_line_length: 2000,     // Read tool default
        }
    }
}

/// Configuration for output style instructions.
///
/// Output styles modify the model's response style. You can use:
/// - A built-in style by name (e.g., "explanatory", "learning")
/// - A custom instruction text
///
/// Custom instruction takes precedence over style_name if both are set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputStyleConfig {
    /// Enable output style instructions.
    pub enabled: bool,
    /// Built-in style name (e.g., "explanatory", "learning").
    /// Use `cocode_config::builtin::list_builtin_output_styles()` to see available styles.
    pub style_name: Option<String>,
    /// Custom output style instruction text.
    /// Takes precedence over style_name if both are set.
    pub instruction: Option<String>,
}

impl Default for OutputStyleConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            style_name: None,
            instruction: None,
        }
    }
}

impl OutputStyleConfig {
    /// Resolve the output style instruction content.
    ///
    /// Resolution order:
    /// 1. Custom instruction (if set) - takes precedence
    /// 2. Built-in style (looked up by style_name)
    ///
    /// Returns `None` if neither custom instruction nor valid style_name is set.
    pub fn resolve_instruction(&self) -> Option<String> {
        // Custom instruction takes precedence
        if let Some(instruction) = &self.instruction {
            if !instruction.is_empty() {
                return Some(instruction.clone());
            }
        }

        // Look up by name from builtin
        if let Some(name) = &self.style_name {
            if let Some(content) = cocode_config::builtin::get_output_style(name) {
                return Some(content.to_string());
            }
        }

        None
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

    #[test]
    fn test_at_mentioned_files_defaults() {
        let config = AtMentionedFilesConfig::default();
        assert_eq!(config.max_file_size, 100 * 1024); // 100KB
        assert_eq!(config.max_lines, 2000);
        assert_eq!(config.max_line_length, 2000);
    }

    #[test]
    fn test_output_style_config_defaults() {
        let config = OutputStyleConfig::default();
        assert!(!config.enabled);
        assert!(config.style_name.is_none());
        assert!(config.instruction.is_none());
    }

    #[test]
    fn test_output_style_config_resolve_builtin() {
        let config = OutputStyleConfig {
            enabled: true,
            style_name: Some("explanatory".to_string()),
            instruction: None,
        };
        let instruction = config.resolve_instruction().unwrap();
        assert!(instruction.contains("Explanatory Style Active"));
    }

    #[test]
    fn test_output_style_config_custom_takes_precedence() {
        let config = OutputStyleConfig {
            enabled: true,
            style_name: Some("explanatory".to_string()),
            instruction: Some("My custom style".to_string()),
        };
        let instruction = config.resolve_instruction().unwrap();
        assert_eq!(instruction, "My custom style");
    }

    #[test]
    fn test_output_style_config_empty_instruction_fallback() {
        // Empty string instruction should fall back to style_name
        let config = OutputStyleConfig {
            enabled: true,
            style_name: Some("learning".to_string()),
            instruction: Some(String::new()),
        };
        let instruction = config.resolve_instruction().unwrap();
        assert!(instruction.contains("Learning Style Active"));
    }

    #[test]
    fn test_output_style_config_unknown_style() {
        let config = OutputStyleConfig {
            enabled: true,
            style_name: Some("nonexistent".to_string()),
            instruction: None,
        };
        assert!(config.resolve_instruction().is_none());
    }

    #[test]
    fn test_output_style_config_neither_set() {
        let config = OutputStyleConfig {
            enabled: true,
            style_name: None,
            instruction: None,
        };
        assert!(config.resolve_instruction().is_none());
    }
}
