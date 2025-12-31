//! LSP diagnostics attachment generator.
//!
//! Generates system reminders when new LSP diagnostics are detected.
//! Uses `<new-diagnostics>` XML tag for the output.
//!
//! Features:
//! - Severity filtering: Only inject diagnostics at or above configured severity level
//! - Path filtering: Only inject diagnostics from files within current working directory

use crate::config::system_reminder::LspDiagnosticsMinSeverity;
use crate::config::system_reminder::SystemReminderConfig;
use crate::error::Result;
use crate::system_reminder::generator::AttachmentGenerator;
use crate::system_reminder::generator::GeneratorContext;
use crate::system_reminder::throttle::ThrottleConfig;
use crate::system_reminder::types::AttachmentType;
use crate::system_reminder::types::ReminderTier;
use crate::system_reminder::types::SystemReminder;
use async_trait::async_trait;
use codex_lsp::DiagnosticEntry;
use codex_lsp::DiagnosticSeverityLevel;
use codex_lsp::DiagnosticsStore;
use std::path::Path;

/// LSP diagnostics generator.
///
/// Generates reminders when new diagnostics are available from LSP servers.
/// Matches Claude Code's lsp_diagnostics attachment (jH5 generator).
///
/// **Tier:** MainAgentOnly (only main agent, not sub-agents)
/// **Throttling:** None (immediate notification)
/// **XML Tag:** `<new-diagnostics>` (already included in content)
#[derive(Debug)]
pub struct LspDiagnosticsGenerator;

impl LspDiagnosticsGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LspDiagnosticsGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AttachmentGenerator for LspDiagnosticsGenerator {
    fn name(&self) -> &str {
        "lsp_diagnostics"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::LspDiagnostics
    }

    fn tier(&self) -> ReminderTier {
        ReminderTier::MainAgentOnly
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Only generate for main agent
        if !ctx.is_main_agent {
            return Ok(None);
        }

        // Get diagnostics store from context
        let store = match &ctx.diagnostics_store {
            Some(s) => s,
            None => return Ok(None), // LSP not enabled
        };

        // Take dirty diagnostics (debounced, clears after take)
        let all_diagnostics = store.take_dirty().await;

        if all_diagnostics.is_empty() {
            return Ok(None);
        }

        // Apply filtering: severity and path
        let filtered =
            filter_diagnostics(all_diagnostics, ctx.cwd, ctx.lsp_diagnostics_min_severity);

        if filtered.is_empty() {
            return Ok(None);
        }

        // Format using existing DiagnosticsStore method
        // Note: format_for_system_reminder already includes <new-diagnostics> tags
        let formatted = DiagnosticsStore::format_for_system_reminder(&filtered);

        // Since format_for_system_reminder already wraps with <new-diagnostics>,
        // wrap_xml() will pass through the content as-is for LspDiagnostics
        Ok(Some(SystemReminder {
            attachment_type: AttachmentType::LspDiagnostics,
            content: formatted,
            tier: ReminderTier::MainAgentOnly,
            is_meta: true,
        }))
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.attachments.lsp_diagnostics
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // No throttling - immediate notification like BackgroundTask
        ThrottleConfig {
            min_turns_between: 0,
            min_turns_after_trigger: 0,
            max_per_session: None,
        }
    }
}

// ============================================
// Filtering Helpers
// ============================================

/// Filter diagnostics by severity and path.
///
/// - Severity: Only include diagnostics at or above the minimum severity level
/// - Path: Only include diagnostics from files within the current working directory
fn filter_diagnostics(
    diagnostics: Vec<DiagnosticEntry>,
    cwd: &Path,
    min_severity: LspDiagnosticsMinSeverity,
) -> Vec<DiagnosticEntry> {
    let min_priority = min_severity_priority(min_severity);

    diagnostics
        .into_iter()
        .filter(|d| {
            // Filter by severity (higher priority = more severe)
            d.severity.priority() >= min_priority
        })
        .filter(|d| {
            // Filter by path (only files within cwd)
            d.file.starts_with(cwd)
        })
        .collect()
}

/// Convert min severity config to priority threshold.
fn min_severity_priority(min_severity: LspDiagnosticsMinSeverity) -> i32 {
    match min_severity {
        LspDiagnosticsMinSeverity::Error => DiagnosticSeverityLevel::Error.priority(),
        LspDiagnosticsMinSeverity::Warning => DiagnosticSeverityLevel::Warning.priority(),
        LspDiagnosticsMinSeverity::Info => DiagnosticSeverityLevel::Info.priority(),
        LspDiagnosticsMinSeverity::Hint => DiagnosticSeverityLevel::Hint.priority(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generator_name() {
        let generator = LspDiagnosticsGenerator::new();
        assert_eq!(generator.name(), "lsp_diagnostics");
    }

    #[test]
    fn test_generator_tier() {
        let generator = LspDiagnosticsGenerator::new();
        assert_eq!(generator.tier(), ReminderTier::MainAgentOnly);
    }

    #[test]
    fn test_generator_attachment_type() {
        let generator = LspDiagnosticsGenerator::new();
        assert_eq!(generator.attachment_type(), AttachmentType::LspDiagnostics);
    }

    #[test]
    fn test_generator_no_throttle() {
        let generator = LspDiagnosticsGenerator::new();
        let config = generator.throttle_config();
        assert_eq!(config.min_turns_between, 0);
        assert_eq!(config.min_turns_after_trigger, 0);
        assert!(config.max_per_session.is_none());
    }

    #[test]
    fn test_generator_is_enabled_default() {
        let generator = LspDiagnosticsGenerator::new();
        let config = SystemReminderConfig::default();
        assert!(generator.is_enabled(&config));
    }

    #[test]
    fn test_generator_is_enabled_disabled() {
        let generator = LspDiagnosticsGenerator::new();
        let mut config = SystemReminderConfig::default();
        config.attachments.lsp_diagnostics = false;
        assert!(!generator.is_enabled(&config));
    }
}
