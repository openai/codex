//! Pylint linter integration for Python

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::analyzer::{Issue, IssueCategory, IssueSeverity};
use super::Linter;

/// Pylint linter for Python
pub struct PylintLinter {
    /// Path to pylint executable
    pylint_path: String,
}

impl PylintLinter {
    /// Create a new Pylint linter
    pub fn new() -> Self {
        Self {
            pylint_path: "pylint".to_string(),
        }
    }

    /// Create with custom pylint path
    pub fn with_path(path: String) -> Self {
        Self { pylint_path: path }
    }

    /// Parse Pylint JSON output
    fn parse_pylint_output(&self, output: &str, _file_path: &Path) -> Result<Vec<Issue>> {
        let messages: Vec<PylintMessage> = serde_json::from_str(output)
            .context("Failed to parse Pylint JSON output")?;

        let mut issues = Vec::new();

        for message in messages {
            let severity = match message.message_type.as_str() {
                "error" | "fatal" => IssueSeverity::Error,
                "warning" => IssueSeverity::Warning,
                "convention" | "refactor" => IssueSeverity::Info,
                _ => IssueSeverity::Info,
            };

            let category = Self::categorize_message(&message.symbol);

            issues.push(Issue {
                severity,
                category,
                description: format!(
                    "{} [{}]",
                    message.message,
                    message.symbol
                ),
                line: Some(message.line),
                column: Some(message.column),
                snippet: None,
                suggested_fix: None,
            });
        }

        Ok(issues)
    }

    /// Categorize Pylint message by symbol
    fn categorize_message(symbol: &str) -> IssueCategory {
        if symbol.contains("security") || symbol.contains("sql-injection") {
            IssueCategory::Security
        } else if symbol.contains("complexity") || symbol.contains("too-many") {
            IssueCategory::Complexity
        } else if symbol.contains("doc") || symbol.contains("missing-") {
            IssueCategory::Documentation
        } else if symbol.contains("import") {
            IssueCategory::Style
        } else if symbol.starts_with("E") || symbol.contains("error") {
            IssueCategory::Bug
        } else if symbol.contains("performance") {
            IssueCategory::Performance
        } else {
            IssueCategory::Style
        }
    }
}

impl Default for PylintLinter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Linter for PylintLinter {
    fn name(&self) -> &str {
        "pylint"
    }

    async fn is_available(&self) -> bool {
        Command::new(&self.pylint_path)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|status| status.success())
            .unwrap_or(false)
    }

    async fn lint_file(&self, file_path: &Path) -> Result<Vec<Issue>> {
        info!("Running Pylint on {:?}", file_path);

        let output = Command::new(&self.pylint_path)
            .arg("--output-format=json")
            .arg("--score=no")
            .arg(file_path)
            .output()
            .await
            .context("Failed to run Pylint")?;

        // Pylint returns non-zero when issues are found
        let stdout = String::from_utf8_lossy(&output.stdout);

        if stdout.trim().is_empty() {
            debug!("Pylint produced no output");
            return Ok(vec![]);
        }

        self.parse_pylint_output(&stdout, file_path)
    }

    fn supported_extensions(&self) -> Vec<&str> {
        vec!["py", "pyw"]
    }
}

/// Pylint JSON message format
#[derive(Debug, Deserialize)]
struct PylintMessage {
    #[serde(rename = "type")]
    message_type: String,
    symbol: String,
    message: String,
    line: usize,
    column: usize,
    #[allow(dead_code)]
    #[serde(rename = "message-id")]
    message_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pylint_availability() {
        let linter = PylintLinter::new();
        // Just check that it doesn't panic
        let _ = linter.is_available().await;
    }

    #[test]
    fn test_parse_pylint_output() {
        let linter = PylintLinter::new();
        let output = r#"[
            {
                "type": "error",
                "symbol": "undefined-variable",
                "message": "Undefined variable 'foo'",
                "message-id": "E0602",
                "line": 10,
                "column": 5
            }
        ]"#;

        let issues = linter.parse_pylint_output(output, Path::new("test.py")).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, IssueSeverity::Error);
        assert_eq!(issues[0].line, Some(10));
    }

    #[test]
    fn test_categorize_message() {
        assert_eq!(
            PylintLinter::categorize_message("too-many-branches"),
            IssueCategory::Complexity
        );
        assert_eq!(
            PylintLinter::categorize_message("missing-docstring"),
            IssueCategory::Documentation
        );
        assert_eq!(
            PylintLinter::categorize_message("sql-injection"),
            IssueCategory::Security
        );
    }

    #[test]
    fn test_supported_extensions() {
        let linter = PylintLinter::new();
        let exts = linter.supported_extensions();
        assert!(exts.contains(&"py"));
    }
}
