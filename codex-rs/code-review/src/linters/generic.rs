//! Generic linter wrapper for external tools

use anyhow::{Context, Result};
use async_trait::async_trait;
use regex::Regex;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::debug;

use crate::analyzer::{Issue, IssueCategory, IssueSeverity};
use super::Linter;

/// Generic command-line linter
pub struct GenericLinter {
    name: String,
    command: String,
    args: Vec<String>,
    extensions: Vec<String>,
    output_parser: OutputParser,
}

impl GenericLinter {
    /// Create a new generic linter
    pub fn new(
        name: String,
        command: String,
        args: Vec<String>,
        extensions: Vec<String>,
        output_parser: OutputParser,
    ) -> Self {
        Self {
            name,
            command,
            args,
            extensions,
            output_parser,
        }
    }

    /// Create ESLint linter
    pub fn eslint() -> Self {
        Self {
            name: "eslint".to_string(),
            command: "eslint".to_string(),
            args: vec!["--format=json".to_string()],
            extensions: vec!["js".to_string(), "jsx".to_string(), "ts".to_string(), "tsx".to_string()],
            output_parser: OutputParser::Json,
        }
    }

    /// Create Pylint linter
    pub fn pylint() -> Self {
        Self {
            name: "pylint".to_string(),
            command: "pylint".to_string(),
            args: vec!["--output-format=json".to_string()],
            extensions: vec!["py".to_string()],
            output_parser: OutputParser::Json,
        }
    }

    /// Create Shellcheck linter
    pub fn shellcheck() -> Self {
        Self {
            name: "shellcheck".to_string(),
            command: "shellcheck".to_string(),
            args: vec!["--format=json".to_string()],
            extensions: vec!["sh".to_string(), "bash".to_string()],
            output_parser: OutputParser::Json,
        }
    }
}

#[async_trait]
impl Linter for GenericLinter {
    fn name(&self) -> &str {
        &self.name
    }

    async fn is_available(&self) -> bool {
        Command::new(&self.command)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    }

    async fn lint_file(&self, file_path: &Path) -> Result<Vec<Issue>> {
        debug!("Running {} on {:?}", self.name, file_path);

        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args);
        cmd.arg(file_path);

        let output = cmd.output()
            .await
            .context(format!("Failed to run {}", self.name))?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        match &self.output_parser {
            OutputParser::Json => self.parse_json_output(&stdout),
            OutputParser::Regex(pattern) => self.parse_regex_output(&stdout, pattern),
        }
    }

    async fn lint_content(&self, _content: &str, _language: &str) -> Result<Vec<Issue>> {
        // Most linters require files
        Ok(vec![])
    }

    fn supported_extensions(&self) -> Vec<&str> {
        self.extensions.iter().map(|s| s.as_str()).collect()
    }
}

/// Output parser type
pub enum OutputParser {
    Json,
    Regex(String),
}

impl GenericLinter {
    /// Parse JSON output (simplified - would need specific parsers for each tool)
    fn parse_json_output(&self, output: &str) -> Result<Vec<Issue>> {
        // This is a simplified parser
        // In production, you'd implement specific parsers for each linter format
        debug!("Parsing JSON output from {}", self.name);
        Ok(vec![])
    }

    /// Parse output using regex
    fn parse_regex_output(&self, output: &str, pattern: &str) -> Result<Vec<Issue>> {
        let re = Regex::new(pattern)?;
        let mut issues = Vec::new();

        for line in output.lines() {
            if let Some(captures) = re.captures(line) {
                let issue = Issue {
                    severity: IssueSeverity::Warning,
                    category: IssueCategory::Style,
                    description: captures.get(0).map(|m| m.as_str().to_string()).unwrap_or_default(),
                    line: captures.get(1).and_then(|m| m.as_str().parse().ok()),
                    column: None,
                    snippet: None,
                    suggested_fix: None,
                };
                issues.push(issue);
            }
        }

        Ok(issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eslint_creation() {
        let linter = GenericLinter::eslint();
        assert_eq!(linter.name(), "eslint");
        assert!(linter.supported_extensions().contains(&"js"));
    }

    #[test]
    fn test_pylint_creation() {
        let linter = GenericLinter::pylint();
        assert_eq!(linter.name(), "pylint");
        assert!(linter.supported_extensions().contains(&"py"));
    }

    #[tokio::test]
    async fn test_linter_availability() {
        let linter = GenericLinter::eslint();
        // Just check it doesn't panic
        let _ = linter.is_available().await;
    }
}
