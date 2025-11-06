//! Auto-fix functionality for code issues

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

use crate::analyzer::{AnalysisResult, Issue, IssueCategory, Suggestion, SuggestionType};
use crate::config::ReviewPolicy;

/// Code fixer
pub struct CodeFixer {
    policy: ReviewPolicy,
}

impl CodeFixer {
    /// Create a new code fixer
    pub fn new(policy: ReviewPolicy) -> Self {
        Self { policy }
    }

    /// Apply fixes to a file based on analysis results
    pub async fn apply_fixes(&self, file_path: &Path, result: &AnalysisResult) -> Result<FixReport> {
        info!("Applying fixes to {:?}", file_path);

        let mut report = FixReport::new(file_path.to_path_buf());

        // Read original content
        let original_content = fs::read_to_string(file_path)
            .await
            .context("Failed to read file")?;

        let mut content = original_content.clone();

        // Apply style fixes if enabled
        if self.policy.auto_fix_style {
            content = self.apply_style_fixes(&content, &result.issues).await?;
            report.style_fixes_applied += result.issues.iter()
                .filter(|i| i.category == IssueCategory::Style)
                .count();
        }

        // Apply suggestions based on confidence
        for suggestion in &result.suggestions {
            if suggestion.confidence >= self.policy.confidence_threshold {
                if self.should_auto_apply(suggestion) {
                    content = self.apply_suggestion(&content, suggestion).await?;
                    report.suggestions_applied.push(suggestion.clone());
                }
            }
        }

        // Only write if changes were made
        if content != original_content {
            // Create backup if policy requires
            if self.policy.create_backup_branch {
                self.create_backup(file_path).await?;
                report.backup_created = true;
            }

            // Write modified content
            fs::write(file_path, &content)
                .await
                .context("Failed to write fixed file")?;

            report.changed = true;
            info!("Applied {} fixes to {:?}", report.total_fixes(), file_path);
        }

        Ok(report)
    }

    /// Apply style fixes
    async fn apply_style_fixes(&self, content: &str, issues: &[Issue]) -> Result<String> {
        let mut fixed_content = content.to_string();

        for issue in issues {
            if issue.category != IssueCategory::Style {
                continue;
            }

            // Apply simple style fixes
            fixed_content = match issue.description.as_str() {
                desc if desc.contains("exceeds 100 characters") => {
                    self.fix_long_lines(&fixed_content, issue)
                }
                desc if desc.contains("trailing whitespace") => {
                    self.fix_trailing_whitespace(&fixed_content)
                }
                desc if desc.contains("missing documentation") => {
                    self.add_documentation(&fixed_content, issue)
                }
                _ => fixed_content,
            };
        }

        Ok(fixed_content)
    }

    /// Fix long lines
    fn fix_long_lines(&self, content: &str, issue: &Issue) -> String {
        if let Some(line_num) = issue.line {
            let lines: Vec<&str> = content.lines().collect();
            if line_num > 0 && line_num <= lines.len() {
                let line = lines[line_num - 1];
                if line.len() > 100 {
                    // Simple fix: break at logical points
                    let mut result = lines[..line_num - 1].join("\n");
                    result.push('\n');
                    result.push_str(&self.break_long_line(line));
                    result.push('\n');
                    result.push_str(&lines[line_num..].join("\n"));
                    return result;
                }
            }
        }
        content.to_string()
    }

    /// Break a long line at logical points
    fn break_long_line(&self, line: &str) -> String {
        // Simple strategy: break at commas, dots, or operators
        if let Some(pos) = line[..100].rfind(|c| c == ',' || c == '.' || c == ' ') {
            let (first, rest) = line.split_at(pos + 1);
            format!("{}\n    {}", first.trim_end(), rest.trim_start())
        } else {
            line.to_string()
        }
    }

    /// Fix trailing whitespace
    fn fix_trailing_whitespace(&self, content: &str) -> String {
        content
            .lines()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Add documentation
    fn add_documentation(&self, content: &str, issue: &Issue) -> String {
        if let Some(line_num) = issue.line {
            let lines: Vec<&str> = content.lines().collect();
            if line_num > 0 && line_num <= lines.len() {
                let mut result = lines[..line_num - 1].join("\n");
                result.push('\n');

                // Add simple doc comment
                let indent = lines[line_num - 1].chars().take_while(|c| c.is_whitespace()).collect::<String>();
                result.push_str(&format!("{}/// TODO: Add documentation\n", indent));

                result.push_str(&lines[line_num - 1..].join("\n"));
                return result;
            }
        }
        content.to_string()
    }

    /// Apply a suggestion
    async fn apply_suggestion(&self, content: &str, suggestion: &Suggestion) -> Result<String> {
        debug!("Applying suggestion: {:?}", suggestion.suggestion_type);

        let result = match suggestion.suggestion_type {
            SuggestionType::AddComment => {
                if self.policy.auto_add_comments {
                    self.add_comments(content, suggestion)
                } else {
                    content.to_string()
                }
            }
            SuggestionType::OptimizeImports => {
                self.optimize_imports(content)
            }
            _ => {
                // Other suggestion types require manual approval
                content.to_string()
            }
        };

        Ok(result)
    }

    /// Add comments based on suggestion
    fn add_comments(&self, content: &str, _suggestion: &Suggestion) -> String {
        // Simplified implementation
        // In production, would use AI to generate meaningful comments
        content.to_string()
    }

    /// Optimize imports
    fn optimize_imports(&self, content: &str) -> String {
        // Simplified: remove duplicate imports
        let lines: Vec<&str> = content.lines().collect();
        let mut seen_imports = std::collections::HashSet::new();
        let mut result = Vec::new();

        for line in lines {
            if line.trim().starts_with("use ") {
                if seen_imports.insert(line.trim()) {
                    result.push(line);
                }
            } else {
                result.push(line);
            }
        }

        result.join("\n")
    }

    /// Check if suggestion should be auto-applied
    fn should_auto_apply(&self, suggestion: &Suggestion) -> bool {
        match suggestion.suggestion_type {
            SuggestionType::AddComment => self.policy.auto_add_comments,
            SuggestionType::AddTest => self.policy.auto_add_tests,
            SuggestionType::Refactor => self.policy.auto_refactor,
            SuggestionType::OptimizeImports => self.policy.auto_fix_style,
            _ => false,
        }
    }

    /// Create backup
    async fn create_backup(&self, file_path: &Path) -> Result<()> {
        let backup_path = file_path.with_extension("backup");
        fs::copy(file_path, &backup_path)
            .await
            .context("Failed to create backup")?;
        debug!("Created backup at {:?}", backup_path);
        Ok(())
    }

    /// Format code using external formatter
    pub async fn format_with_tool(&self, file_path: &Path, tool: &str) -> Result<()> {
        use tokio::process::Command;

        debug!("Formatting {:?} with {}", file_path, tool);

        let status = Command::new(tool)
            .arg(file_path)
            .status()
            .await
            .context(format!("Failed to run {}", tool))?;

        if !status.success() {
            warn!("Formatter {} returned non-zero exit code", tool);
        }

        Ok(())
    }

    /// Run rustfmt on Rust files
    pub async fn rustfmt(&self, file_path: &Path) -> Result<()> {
        self.format_with_tool(file_path, "rustfmt").await
    }

    /// Run prettier on JS/TS files
    pub async fn prettier(&self, file_path: &Path) -> Result<()> {
        self.format_with_tool(file_path, "prettier").await
    }
}

/// Fix report
#[derive(Debug, Clone)]
pub struct FixReport {
    pub file_path: PathBuf,
    pub changed: bool,
    pub style_fixes_applied: usize,
    pub suggestions_applied: Vec<Suggestion>,
    pub backup_created: bool,
}

impl FixReport {
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            changed: false,
            style_fixes_applied: 0,
            suggestions_applied: Vec::new(),
            backup_created: false,
        }
    }

    pub fn total_fixes(&self) -> usize {
        self.style_fixes_applied + self.suggestions_applied.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ReviewPolicy;

    #[test]
    fn test_fix_trailing_whitespace() {
        let fixer = CodeFixer::new(ReviewPolicy::default());
        let content = "line1  \nline2\t\nline3";
        let fixed = fixer.fix_trailing_whitespace(content);
        assert_eq!(fixed, "line1\nline2\nline3");
    }

    #[test]
    fn test_break_long_line() {
        let fixer = CodeFixer::new(ReviewPolicy::default());
        let long_line = "This is a very long line that exceeds one hundred characters and should be broken up into multiple lines for better readability";
        let broken = fixer.break_long_line(long_line);
        assert!(broken.contains('\n'));
    }

    #[test]
    fn test_optimize_imports() {
        let fixer = CodeFixer::new(ReviewPolicy::default());
        let content = "use std::io;\nuse std::fs;\nuse std::io;\nfn main() {}";
        let optimized = fixer.optimize_imports(content);
        assert_eq!(optimized.matches("use std::io;").count(), 1);
    }

    #[test]
    fn test_fix_report() {
        let report = FixReport::new(PathBuf::from("test.rs"));
        assert_eq!(report.total_fixes(), 0);
        assert!(!report.changed);
    }
}
