//! Clippy linter integration for Rust

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, warn};

use crate::analyzer::{Issue, IssueCategory, IssueSeverity};
use super::Linter;

/// Clippy linter
pub struct ClippyLinter {
    flags: Vec<String>,
}

impl ClippyLinter {
    /// Create a new Clippy linter
    pub fn new() -> Self {
        Self {
            flags: vec![
                "--message-format=json".to_string(),
                "--".to_string(),
                "-W".to_string(),
                "clippy::all".to_string(),
            ],
        }
    }

    /// Create with custom flags
    pub fn with_flags(flags: Vec<String>) -> Self {
        Self { flags }
    }

    /// Parse clippy JSON output
    fn parse_clippy_output(&self, output: &str) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();

        for line in output.lines() {
            // Skip non-JSON lines
            if !line.trim().starts_with('{') {
                continue;
            }

            match serde_json::from_str::<ClippyMessage>(line) {
                Ok(msg) => {
                    if let Some(issue) = self.convert_to_issue(msg) {
                        issues.push(issue);
                    }
                }
                Err(e) => {
                    debug!("Failed to parse clippy message: {}", e);
                }
            }
        }

        Ok(issues)
    }

    /// Convert clippy message to Issue
    fn convert_to_issue(&self, msg: ClippyMessage) -> Option<Issue> {
        // Only process compiler messages
        if msg.reason != "compiler-message" {
            return None;
        }

        let message = msg.message?;

        // Filter out non-clippy messages
        if !message.code.as_ref()
            .and_then(|c| c.code.as_ref())
            .map(|s| s.starts_with("clippy::"))
            .unwrap_or(false)
        {
            return None;
        }

        let severity = match message.level.as_str() {
            "error" => IssueSeverity::Error,
            "warning" => IssueSeverity::Warning,
            "help" | "note" => IssueSeverity::Info,
            _ => return None,
        };

        let (line, column, snippet) = if let Some(span) = message.spans.first() {
            (
                Some(span.line_start),
                Some(span.column_start),
                Some(span.text.first()?.text.clone()),
            )
        } else {
            (None, None, None)
        };

        Some(Issue {
            severity,
            category: IssueCategory::Style, // Clippy is primarily style/best practices
            description: message.message.clone(),
            line,
            column,
            snippet,
            suggested_fix: message.rendered.clone(),
        })
    }
}

impl Default for ClippyLinter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Linter for ClippyLinter {
    fn name(&self) -> &str {
        "clippy"
    }

    async fn is_available(&self) -> bool {
        Command::new("cargo")
            .arg("clippy")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    }

    async fn lint_file(&self, file_path: &Path) -> Result<Vec<Issue>> {
        debug!("Running clippy on {:?}", file_path);

        // Run clippy on the entire project (clippy doesn't support single files well)
        let output = Command::new("cargo")
            .arg("clippy")
            .args(&self.flags)
            .output()
            .await
            .context("Failed to run clippy")?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        let all_issues = self.parse_clippy_output(&stdout)?;

        // Filter issues for the specific file
        let file_issues: Vec<Issue> = all_issues
            .into_iter()
            .filter(|issue| {
                // This is a simplified check - in production you'd want more robust filtering
                issue.snippet.as_ref()
                    .map(|_| true) // For now, include all issues
                    .unwrap_or(false)
            })
            .collect();

        Ok(file_issues)
    }

    fn supported_extensions(&self) -> Vec<&str> {
        vec!["rs"]
    }
}

/// Clippy JSON message format
#[derive(Debug, Deserialize)]
struct ClippyMessage {
    reason: String,
    message: Option<CompilerMessage>,
}

/// Compiler message
#[derive(Debug, Deserialize)]
struct CompilerMessage {
    message: String,
    code: Option<DiagnosticCode>,
    level: String,
    spans: Vec<DiagnosticSpan>,
    rendered: Option<String>,
}

/// Diagnostic code
#[derive(Debug, Deserialize)]
struct DiagnosticCode {
    code: Option<String>,
}

/// Diagnostic span
#[derive(Debug, Deserialize)]
struct DiagnosticSpan {
    line_start: usize,
    line_end: usize,
    column_start: usize,
    column_end: usize,
    text: Vec<DiagnosticSpanText>,
}

/// Diagnostic span text
#[derive(Debug, Deserialize)]
struct DiagnosticSpanText {
    text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_clippy_availability() {
        let linter = ClippyLinter::new();
        // Just check it doesn't panic
        let _ = linter.is_available().await;
    }

    #[test]
    fn test_supported_extensions() {
        let linter = ClippyLinter::new();
        assert!(linter.supported_extensions().contains(&"rs"));
    }

    #[test]
    fn test_parse_clippy_output() {
        let linter = ClippyLinter::new();
        let output = r#"{"reason":"compiler-message","message":{"message":"this is a test","code":{"code":"clippy::test"},"level":"warning","spans":[{"line_start":10,"line_end":10,"column_start":5,"column_end":10,"text":[{"text":"test line"}]}],"rendered":"rendered message"}}"#;

        let issues = linter.parse_clippy_output(output).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].line, Some(10));
    }
}
