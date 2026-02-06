//! LSP diagnostics generator.
//!
//! Injects diagnostic information from language servers to help
//! the agent identify and fix issues.

use async_trait::async_trait;

use crate::Result;
use crate::config::DiagnosticSeverity;
use crate::config::SystemReminderConfig;
use crate::generator::AttachmentGenerator;
use crate::generator::DiagnosticInfo;
use crate::generator::GeneratorContext;
use crate::throttle::ThrottleConfig;
use crate::types::AttachmentType;
use crate::types::ReminderTier;
use crate::types::SystemReminder;
use crate::types::XmlTag;

/// Generator for LSP diagnostics.
#[derive(Debug)]
pub struct LspDiagnosticsGenerator;

#[async_trait]
impl AttachmentGenerator for LspDiagnosticsGenerator {
    fn name(&self) -> &str {
        "LspDiagnosticsGenerator"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::LspDiagnostics
    }

    fn tier(&self) -> ReminderTier {
        ReminderTier::MainAgentOnly
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.attachments.lsp_diagnostics
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // No throttle - always show new diagnostics
        ThrottleConfig::none()
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        if !ctx.has_diagnostics() {
            return Ok(None);
        }

        // Filter by severity
        let min_severity = &ctx.config.attachments.lsp_diagnostics_min_severity;
        let filtered: Vec<_> = ctx
            .diagnostics
            .iter()
            .filter(|d| severity_passes_filter(&d.severity, min_severity))
            .collect();

        if filtered.is_empty() {
            return Ok(None);
        }

        let content = format_diagnostics(&filtered);

        // LSP diagnostics use a special XML tag
        let reminder = SystemReminder::new(AttachmentType::LspDiagnostics, content);

        // Note: The XML tag is already set correctly via attachment_type
        debug_assert_eq!(reminder.xml_tag(), XmlTag::NewDiagnostics);

        Ok(Some(reminder))
    }
}

/// Check if a severity passes the minimum severity filter.
fn severity_passes_filter(severity: &str, min_severity: &DiagnosticSeverity) -> bool {
    let severity_level = match severity.to_lowercase().as_str() {
        "error" => DiagnosticSeverity::Error,
        "warning" | "warn" => DiagnosticSeverity::Warning,
        "information" | "info" => DiagnosticSeverity::Info,
        "hint" => DiagnosticSeverity::Hint,
        _ => DiagnosticSeverity::Hint, // Unknown = show
    };

    severity_level <= *min_severity
}

/// Format diagnostics into a readable string.
fn format_diagnostics(diagnostics: &[&DiagnosticInfo]) -> String {
    let mut content = String::new();
    content.push_str("New diagnostics detected:\n\n");

    // Group by file
    let mut by_file: std::collections::HashMap<&std::path::PathBuf, Vec<&&DiagnosticInfo>> =
        std::collections::HashMap::new();

    for diag in diagnostics {
        by_file.entry(&diag.file_path).or_default().push(diag);
    }

    for (file, diags) in by_file {
        content.push_str(&format!("**{}**:\n", file.display()));

        for diag in diags {
            let code_str = diag
                .code
                .as_ref()
                .map(|c| format!(" [{c}]"))
                .unwrap_or_default();

            content.push_str(&format!(
                "  - Line {}, Col {}: [{}]{} {}\n",
                diag.line, diag.column, diag.severity, code_str, diag.message
            ));
        }

        content.push('\n');
    }

    content.push_str("Please review and address these issues.");

    content
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_config() -> SystemReminderConfig {
        SystemReminderConfig::default()
    }

    #[tokio::test]
    async fn test_no_diagnostics() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = LspDiagnosticsGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_with_diagnostics() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .diagnostics(vec![
                DiagnosticInfo {
                    file_path: PathBuf::from("/src/main.rs"),
                    line: 10,
                    column: 5,
                    severity: "error".to_string(),
                    message: "cannot find value `foo`".to_string(),
                    code: Some("E0425".to_string()),
                },
                DiagnosticInfo {
                    file_path: PathBuf::from("/src/main.rs"),
                    line: 15,
                    column: 1,
                    severity: "warning".to_string(),
                    message: "unused variable".to_string(),
                    code: None,
                },
            ])
            .build();

        let generator = LspDiagnosticsGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content().unwrap().contains("main.rs"));
        assert!(reminder.content().unwrap().contains("cannot find value"));
        assert!(reminder.content().unwrap().contains("[E0425]"));
        assert!(reminder.content().unwrap().contains("unused variable"));
    }

    #[tokio::test]
    async fn test_severity_filtering() {
        let mut config = test_config();
        config.attachments.lsp_diagnostics_min_severity = DiagnosticSeverity::Error;

        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .diagnostics(vec![DiagnosticInfo {
                file_path: PathBuf::from("/src/main.rs"),
                line: 15,
                column: 1,
                severity: "warning".to_string(), // Only warning, but filter is Error
                message: "unused variable".to_string(),
                code: None,
            }])
            .build();

        let generator = LspDiagnosticsGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none()); // Filtered out
    }

    #[test]
    fn test_severity_filter_logic() {
        // Error passes Error filter
        assert!(severity_passes_filter("error", &DiagnosticSeverity::Error));

        // Warning doesn't pass Error filter
        assert!(!severity_passes_filter(
            "warning",
            &DiagnosticSeverity::Error
        ));

        // Warning passes Warning filter
        assert!(severity_passes_filter(
            "warning",
            &DiagnosticSeverity::Warning
        ));

        // Error passes Warning filter
        assert!(severity_passes_filter(
            "error",
            &DiagnosticSeverity::Warning
        ));

        // Hint passes Hint filter
        assert!(severity_passes_filter("hint", &DiagnosticSeverity::Hint));
    }

    #[test]
    fn test_generator_properties() {
        let generator = LspDiagnosticsGenerator;
        assert_eq!(generator.name(), "LspDiagnosticsGenerator");
        assert_eq!(generator.tier(), ReminderTier::MainAgentOnly);
        assert_eq!(generator.attachment_type(), AttachmentType::LspDiagnostics);
    }
}
