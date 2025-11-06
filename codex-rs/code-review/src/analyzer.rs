//! Code analyzer for detecting issues and suggesting improvements

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, info};

/// Analysis result for a code file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// File path analyzed
    pub file_path: PathBuf,

    /// Issues found
    pub issues: Vec<Issue>,

    /// Suggestions for improvement
    pub suggestions: Vec<Suggestion>,

    /// Code metrics
    pub metrics: CodeMetrics,

    /// Analysis timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Analysis duration
    pub duration: Duration,
}

impl AnalysisResult {
    /// Check if there are any critical issues
    pub fn has_critical_issues(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| matches!(issue.severity, IssueSeverity::Critical))
    }

    /// Get all issues above a certain severity
    pub fn issues_above_severity(&self, min_severity: IssueSeverity) -> Vec<&Issue> {
        self.issues
            .iter()
            .filter(|issue| issue.severity >= min_severity)
            .collect()
    }

    /// Get suggestions by type
    pub fn suggestions_by_type(&self, suggestion_type: SuggestionType) -> Vec<&Suggestion> {
        self.suggestions
            .iter()
            .filter(|s| s.suggestion_type == suggestion_type)
            .collect()
    }
}

/// Issue severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Code issue detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    /// Issue severity
    pub severity: IssueSeverity,

    /// Issue category
    pub category: IssueCategory,

    /// Description
    pub description: String,

    /// Location in file (line number)
    pub line: Option<usize>,

    /// Column number
    pub column: Option<usize>,

    /// Code snippet
    pub snippet: Option<String>,

    /// Suggested fix
    pub suggested_fix: Option<String>,
}

/// Issue categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueCategory {
    Style,
    Bug,
    Performance,
    Security,
    Complexity,
    Documentation,
    Testing,
    Maintainability,
}

/// Suggestion for improvement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    /// Suggestion type
    pub suggestion_type: SuggestionType,

    /// Description
    pub description: String,

    /// Confidence score (0.0-1.0)
    pub confidence: f32,

    /// Estimated impact
    pub impact: Impact,

    /// Code changes (optional)
    pub code_changes: Option<CodeChange>,

    /// Rationale
    pub rationale: String,
}

/// Types of suggestions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuggestionType {
    Refactor,
    AddTest,
    AddComment,
    ExtractFunction,
    RemoveDuplication,
    OptimizeImports,
    ImproveNaming,
    SimplifyLogic,
}

/// Impact levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Impact {
    Low,
    Medium,
    High,
}

/// Code change description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChange {
    /// Original code
    pub original: String,

    /// Modified code
    pub modified: String,

    /// Start line
    pub start_line: usize,

    /// End line
    pub end_line: usize,
}

/// Code metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeMetrics {
    /// Lines of code
    pub loc: usize,

    /// Cyclomatic complexity
    pub complexity: u32,

    /// Number of functions
    pub num_functions: usize,

    /// Number of comments
    pub num_comments: usize,

    /// Documentation coverage (0.0-1.0)
    pub doc_coverage: f32,

    /// Test coverage (0.0-1.0, if available)
    pub test_coverage: Option<f32>,
}

/// Code analyzer
pub struct CodeAnalyzer {
    /// Analysis configuration
    config: crate::config::AnalysisConfig,
}

impl CodeAnalyzer {
    /// Create a new code analyzer
    pub fn new(config: crate::config::AnalysisConfig) -> Self {
        Self { config }
    }

    /// Analyze a code file
    pub async fn analyze_file(&self, file_path: &Path) -> Result<AnalysisResult> {
        let start = std::time::Instant::now();
        info!("Analyzing file: {:?}", file_path);

        // Read file content
        let content = tokio::fs::read_to_string(file_path)
            .await
            .context("Failed to read file")?;

        // Perform analysis
        let issues = self.detect_issues(&content, file_path).await?;
        let suggestions = self.generate_suggestions(&content, file_path, &issues).await?;
        let metrics = self.calculate_metrics(&content).await?;

        let duration = start.elapsed();

        Ok(AnalysisResult {
            file_path: file_path.to_path_buf(),
            issues,
            suggestions,
            metrics,
            timestamp: chrono::Utc::now(),
            duration,
        })
    }

    /// Analyze multiple files
    pub async fn analyze_files(&self, file_paths: &[PathBuf]) -> Result<Vec<AnalysisResult>> {
        let mut results = Vec::new();

        for file_path in file_paths {
            match self.analyze_file(file_path).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    tracing::warn!("Failed to analyze {:?}: {}", file_path, e);
                }
            }
        }

        Ok(results)
    }

    /// Detect issues in code
    async fn detect_issues(&self, content: &str, _file_path: &Path) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();

        // Basic analysis - in real implementation, this would use AI models
        // or integrate with linters like clippy, eslint, etc.

        if self.config.check_style {
            // Check for long lines
            for (line_num, line) in content.lines().enumerate() {
                if line.len() > 100 {
                    issues.push(Issue {
                        severity: IssueSeverity::Warning,
                        category: IssueCategory::Style,
                        description: "Line exceeds 100 characters".to_string(),
                        line: Some(line_num + 1),
                        column: Some(100),
                        snippet: Some(line.to_string()),
                        suggested_fix: None,
                    });
                }
            }
        }

        if self.config.check_complexity {
            // Simplified complexity check
            let complexity = self.estimate_complexity(content);
            if complexity > self.config.max_complexity {
                issues.push(Issue {
                    severity: IssueSeverity::Warning,
                    category: IssueCategory::Complexity,
                    description: format!(
                        "Code complexity ({}) exceeds threshold ({})",
                        complexity, self.config.max_complexity
                    ),
                    line: None,
                    column: None,
                    snippet: None,
                    suggested_fix: Some("Consider breaking down complex functions".to_string()),
                });
            }
        }

        debug!("Detected {} issues", issues.len());
        Ok(issues)
    }

    /// Generate improvement suggestions
    async fn generate_suggestions(
        &self,
        content: &str,
        _file_path: &Path,
        issues: &[Issue],
    ) -> Result<Vec<Suggestion>> {
        let mut suggestions = Vec::new();

        // Generate suggestions based on issues
        for issue in issues {
            if issue.category == IssueCategory::Complexity {
                suggestions.push(Suggestion {
                    suggestion_type: SuggestionType::ExtractFunction,
                    description: "Extract complex logic into separate functions".to_string(),
                    confidence: 0.8,
                    impact: Impact::High,
                    code_changes: None,
                    rationale: "Reducing complexity improves readability and maintainability"
                        .to_string(),
                });
            }
        }

        // Check for missing documentation
        if self.config.check_documentation {
            let doc_coverage = self.calculate_doc_coverage(content);
            if doc_coverage < self.config.min_doc_coverage {
                suggestions.push(Suggestion {
                    suggestion_type: SuggestionType::AddComment,
                    description: "Add documentation to improve code clarity".to_string(),
                    confidence: 0.9,
                    impact: Impact::Medium,
                    code_changes: None,
                    rationale: "Documentation helps other developers understand the code"
                        .to_string(),
                });
            }
        }

        debug!("Generated {} suggestions", suggestions.len());
        Ok(suggestions)
    }

    /// Calculate code metrics
    async fn calculate_metrics(&self, content: &str) -> Result<CodeMetrics> {
        let lines: Vec<&str> = content.lines().collect();
        let loc = lines.len();

        // Count functions (simplified - would use proper parser in real impl)
        let num_functions = content.matches("fn ").count();

        // Count comments
        let num_comments = lines.iter().filter(|line| line.trim().starts_with("//")).count();

        // Calculate documentation coverage
        let doc_coverage = self.calculate_doc_coverage(content);

        // Estimate complexity
        let complexity = self.estimate_complexity(content);

        Ok(CodeMetrics {
            loc,
            complexity,
            num_functions,
            num_comments,
            doc_coverage,
            test_coverage: None, // Would integrate with coverage tools
        })
    }

    /// Calculate documentation coverage
    fn calculate_doc_coverage(&self, content: &str) -> f32 {
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return 0.0;
        }

        let doc_lines = lines
            .iter()
            .filter(|line| {
                let trimmed = line.trim();
                trimmed.starts_with("///") || trimmed.starts_with("//!")
            })
            .count();

        doc_lines as f32 / lines.len() as f32
    }

    /// Estimate cyclomatic complexity (simplified)
    fn estimate_complexity(&self, content: &str) -> u32 {
        let mut complexity = 1;

        // Count decision points
        complexity += content.matches("if ").count() as u32;
        complexity += content.matches("else ").count() as u32;
        complexity += content.matches("for ").count() as u32;
        complexity += content.matches("while ").count() as u32;
        complexity += content.matches("match ").count() as u32;
        complexity += content.matches("&&").count() as u32;
        complexity += content.matches("||").count() as u32;

        complexity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AnalysisConfig;

    #[tokio::test]
    async fn test_calculate_metrics() {
        let analyzer = CodeAnalyzer::new(AnalysisConfig::default());
        let code = r#"
fn test_function() {
    // This is a comment
    if true {
        println!("test");
    }
}
"#;
        let metrics = analyzer.calculate_metrics(code).await.unwrap();
        assert!(metrics.loc > 0);
        assert!(metrics.complexity > 1);
    }

    #[test]
    fn test_doc_coverage() {
        let analyzer = CodeAnalyzer::new(AnalysisConfig::default());
        let code = r#"
/// This is documentation
fn test() {}
"#;
        let coverage = analyzer.calculate_doc_coverage(code);
        assert!(coverage > 0.0);
    }
}
