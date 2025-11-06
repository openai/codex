//! Configuration types for continuous code review

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Main configuration for continuous code review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewConfig {
    /// Enable continuous review
    pub enabled: bool,

    /// Patterns to watch (glob patterns)
    pub watch_patterns: Vec<String>,

    /// Patterns to ignore
    pub ignore_patterns: Vec<String>,

    /// Review triggers
    pub triggers: ReviewTrigger,

    /// Review policies
    pub policies: ReviewPolicy,

    /// Local AI configuration
    pub local_ai: LocalAIConfig,

    /// Analysis settings
    pub analysis: AnalysisConfig,
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            watch_patterns: vec![
                "**/*.rs".to_string(),
                "**/*.py".to_string(),
                "**/*.js".to_string(),
                "**/*.ts".to_string(),
            ],
            ignore_patterns: vec![
                "**/target/**".to_string(),
                "**/node_modules/**".to_string(),
                "**/.git/**".to_string(),
                "**/dist/**".to_string(),
                "**/build/**".to_string(),
            ],
            triggers: ReviewTrigger::default(),
            policies: ReviewPolicy::default(),
            local_ai: LocalAIConfig::default(),
            analysis: AnalysisConfig::default(),
        }
    }
}

/// Review trigger configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewTrigger {
    /// Trigger review on file save
    pub on_file_save: bool,

    /// Trigger review on git commit
    pub on_git_commit: bool,

    /// Periodic check interval in minutes
    pub periodic_check_minutes: Option<u64>,

    /// Trigger on manual request
    pub on_manual_request: bool,
}

impl Default for ReviewTrigger {
    fn default() -> Self {
        Self {
            on_file_save: true,
            on_git_commit: true,
            periodic_check_minutes: Some(30),
            on_manual_request: true,
        }
    }
}

/// Review policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewPolicy {
    /// Automatically fix code style issues
    pub auto_fix_style: bool,

    /// Automatically add tests (requires approval if false)
    pub auto_add_tests: bool,

    /// Automatically refactor code (requires approval if false)
    pub auto_refactor: bool,

    /// Automatically add comments
    pub auto_add_comments: bool,

    /// Require tests to pass before applying changes
    pub require_tests_pass: bool,

    /// Create backup branch before changes
    pub create_backup_branch: bool,

    /// Commit each improvement separately
    pub commit_each_improvement: bool,

    /// Confidence threshold for auto-apply (0.0-1.0)
    pub confidence_threshold: f32,
}

impl Default for ReviewPolicy {
    fn default() -> Self {
        Self {
            auto_fix_style: true,
            auto_add_tests: false,
            auto_refactor: false,
            auto_add_comments: true,
            require_tests_pass: true,
            create_backup_branch: true,
            commit_each_improvement: true,
            confidence_threshold: 0.85,
        }
    }
}

/// Local AI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalAIConfig {
    /// Primary model for code generation
    pub coding_model: String,

    /// Model for code review
    pub review_model: String,

    /// Fast model for quick checks
    pub quick_check_model: String,

    /// Ollama base URL
    pub ollama_base_url: String,

    /// Maximum concurrent requests
    pub concurrent_requests: usize,

    /// Request timeout in seconds
    pub timeout_secs: u64,
}

impl Default for LocalAIConfig {
    fn default() -> Self {
        Self {
            coding_model: "deepseek-coder:33b".to_string(),
            review_model: "codellama:13b-instruct".to_string(),
            quick_check_model: "qwen2.5-coder:7b".to_string(),
            ollama_base_url: "http://localhost:11434/v1".to_string(),
            concurrent_requests: 3,
            timeout_secs: 300,
        }
    }
}

/// Analysis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    /// Analyze code style
    pub check_style: bool,

    /// Detect potential bugs
    pub detect_bugs: bool,

    /// Suggest refactoring
    pub suggest_refactoring: bool,

    /// Check code complexity
    pub check_complexity: bool,

    /// Generate tests
    pub generate_tests: bool,

    /// Check documentation coverage
    pub check_documentation: bool,

    /// Maximum complexity threshold
    pub max_complexity: u32,

    /// Minimum documentation coverage (0.0-1.0)
    pub min_doc_coverage: f32,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            check_style: true,
            detect_bugs: true,
            suggest_refactoring: true,
            check_complexity: true,
            generate_tests: false,
            check_documentation: true,
            max_complexity: 10,
            min_doc_coverage: 0.7,
        }
    }
}

/// Improvement scope configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementScope {
    /// Improve code style
    pub code_style: bool,

    /// Add comments
    pub add_comments: bool,

    /// Extract functions
    pub extract_functions: bool,

    /// Remove duplication
    pub remove_duplication: bool,

    /// Optimize imports
    pub optimize_imports: bool,
}

impl Default for ImprovementScope {
    fn default() -> Self {
        Self {
            code_style: true,
            add_comments: true,
            extract_functions: true,
            remove_duplication: true,
            optimize_imports: true,
        }
    }
}

/// Auto-improve configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoImproveConfig {
    /// Enable auto-improve (opt-in)
    pub enabled: bool,

    /// Maximum iterations
    pub max_iterations: u32,

    /// Confidence threshold
    pub confidence_threshold: f32,

    /// Improvement scope
    pub scope: ImprovementScope,

    /// Safety settings
    pub require_tests_pass: bool,

    /// Create backup branch
    pub create_backup_branch: bool,

    /// Commit each improvement
    pub commit_each_improvement: bool,
}

impl Default for AutoImproveConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_iterations: 10,
            confidence_threshold: 0.85,
            scope: ImprovementScope::default(),
            require_tests_pass: true,
            create_backup_branch: true,
            commit_each_improvement: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ReviewConfig::default();
        assert!(!config.enabled);
        assert!(!config.watch_patterns.is_empty());
        assert!(config.policies.confidence_threshold > 0.0);
    }

    #[test]
    fn test_local_ai_config() {
        let config = LocalAIConfig::default();
        assert!(config.ollama_base_url.contains("localhost"));
        assert!(config.concurrent_requests > 0);
    }
}
