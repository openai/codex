//! ESLint linter integration for JavaScript/TypeScript

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::analyzer::{Issue, IssueCategory, IssueSeverity};
use super::Linter;

/// ESLint linter for JavaScript/TypeScript
pub struct ESLintLinter {
    /// Path to eslint executable
    eslint_path: String,
}

impl ESLintLinter {
    /// Create a new ESLint linter
    pub fn new() -> Self {
        Self {
            eslint_path: "eslint".to_string(),
        }
    }

    /// Create with custom eslint path
    pub fn with_path(path: String) -> Self {
        Self { eslint_path: path }
    }

    /// Parse ESLint JSON output
    fn parse_eslint_output(&self, output: &str, file_path: &Path) -> Result<Vec<Issue>> {
        let results: Vec<ESLintResult> = serde_json::from_str(output)
            .context("Failed to parse ESLint JSON output")?;

        let mut issues = Vec::new();

        for result in results {
            for message in result.messages {
                let severity = match message.severity {
                    2 => IssueSeverity::Error,
                    1 => IssueSeverity::Warning,
                    _ => IssueSeverity::Info,
                };

                let category = if message.rule_id.as_ref().map(|r| r.contains("security")).unwrap_or(false) {
                    IssueCategory::Security
                } else if message.rule_id.as_ref().map(|r| r.contains("complexity")).unwrap_or(false) {
                    IssueCategory::Complexity
                } else if message.rule_id.as_ref().map(|r| r.starts_with("no-")).unwrap_or(false) {
                    IssueCategory::Bug
                } else {
                    IssueCategory::Style
                };

                issues.push(Issue {
                    severity,
                    category,
                    description: format!(
                        "{} [{}]",
                        message.message,
                        message.rule_id.unwrap_or_else(|| "eslint".to_string())
                    ),
                    line: Some(message.line),
                    column: Some(message.column),
                    snippet: None,
                    suggested_fix: message.fix.map(|f| format!("Replace with: {}", f.text)),
                });
            }
        }

        Ok(issues)
    }
}

impl Default for ESLintLinter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Linter for ESLintLinter {
    fn name(&self) -> &str {
        "eslint"
    }

    async fn is_available(&self) -> bool {
        Command::new(&self.eslint_path)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|status| status.success())
            .unwrap_or(false)
    }

    async fn lint_file(&self, file_path: &Path) -> Result<Vec<Issue>> {
        info!("Running ESLint on {:?}", file_path);

        let output = Command::new(&self.eslint_path)
            .arg("--format=json")
            .arg("--no-color")
            .arg(file_path)
            .output()
            .await
            .context("Failed to run ESLint")?;

        // ESLint returns non-zero when issues are found, so we don't check status
        let stdout = String::from_utf8_lossy(&output.stdout);

        if stdout.trim().is_empty() {
            debug!("ESLint produced no output");
            return Ok(vec![]);
        }

        self.parse_eslint_output(&stdout, file_path)
    }

    fn supported_extensions(&self) -> Vec<&str> {
        vec!["js", "jsx", "ts", "tsx", "mjs", "cjs"]
    }
}

/// ESLint JSON output format
#[derive(Debug, Deserialize)]
struct ESLintResult {
    #[allow(dead_code)]
    #[serde(rename = "filePath")]
    file_path: String,
    messages: Vec<ESLintMessage>,
}

/// ESLint message
#[derive(Debug, Deserialize)]
struct ESLintMessage {
    #[serde(rename = "ruleId")]
    rule_id: Option<String>,
    severity: u8,
    message: String,
    line: usize,
    column: usize,
    #[allow(dead_code)]
    #[serde(rename = "nodeType")]
    node_type: Option<String>,
    #[serde(rename = "messageId")]
    #[allow(dead_code)]
    message_id: Option<String>,
    fix: Option<ESLintFix>,
}

/// ESLint fix suggestion
#[derive(Debug, Deserialize)]
struct ESLintFix {
    text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_eslint_availability() {
        let linter = ESLintLinter::new();
        // Just check that it doesn't panic
        let _ = linter.is_available().await;
    }

    #[test]
    fn test_parse_eslint_output() {
        let linter = ESLintLinter::new();
        let output = r#"[
            {
                "filePath": "/test/file.js",
                "messages": [
                    {
                        "ruleId": "no-unused-vars",
                        "severity": 2,
                        "message": "'x' is assigned a value but never used.",
                        "line": 1,
                        "column": 5,
                        "nodeType": "Identifier",
                        "messageId": "unusedVar"
                    }
                ]
            }
        ]"#;

        let issues = linter.parse_eslint_output(output, Path::new("test.js")).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, IssueSeverity::Error);
        assert_eq!(issues[0].line, Some(1));
    }

    #[test]
    fn test_supported_extensions() {
        let linter = ESLintLinter::new();
        let exts = linter.supported_extensions();
        assert!(exts.contains(&"js"));
        assert!(exts.contains(&"ts"));
        assert!(exts.contains(&"tsx"));
    }
}
