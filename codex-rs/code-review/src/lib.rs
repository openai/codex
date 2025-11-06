//! Continuous Code Review Module
//!
//! This module provides continuous code review capabilities using local AI models.
//! It watches for code changes, analyzes them, and provides improvement suggestions.
//!
//! # Features
//!
//! - **AI-Powered Analysis**: Integration with Ollama and local AI models
//! - **Linter Integration**: Support for Clippy, ESLint, Pylint, and more
//! - **Auto-Fix**: Automatically fix style issues and apply improvements
//! - **Continuous Monitoring**: Watch files and review changes in real-time
//! - **Session Management**: Track review sessions and statistics
//! - **AST Analysis**: Deep code analysis using syn for Rust
//! - **Git Integration**: Auto-commit improvements with detailed messages
//! - **Test Generation**: AI-powered test creation
//!
//! # Example
//!
//! ```rust,no_run
//! use codex_code_review::{ReviewConfig, ContinuousReviewer};
//! use std::path::PathBuf;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = ReviewConfig::default();
//! let session_dir = PathBuf::from("~/.codex/sessions");
//! let watch_dir = PathBuf::from("./src");
//!
//! let reviewer = ContinuousReviewer::new(config, session_dir, watch_dir)?;
//! reviewer.start().await?;
//! # Ok(())
//! # }
//! ```

mod ai_client;
mod analyzer;
mod ast_analyzer;
mod config;
mod enhanced_analyzer;
mod fixer;
mod git_integration;
pub mod linters;
mod reviewer;
mod session;
mod test_generator;
mod tools;
mod watcher;

// Core exports
pub use ai_client::{AIClient, AIAnalysisResult, QuickCheckType};
pub use analyzer::{AnalysisResult, CodeAnalyzer, Issue, IssueSeverity, IssueCategory, Suggestion};
pub use config::{ReviewConfig, ReviewPolicy, ReviewTrigger, LocalAIConfig, AnalysisConfig};
pub use fixer::{CodeFixer, FixReport};
pub use linters::{Linter, LinterRegistry};
pub use reviewer::{ContinuousReviewer, ReviewTask, ReviewType};
pub use session::{ReviewSession, ReviewSessionState, SessionManager};
pub use tools::{register_review_tools, ReviewTool};
pub use watcher::{FileWatcher, WatchEvent};

// Phase 3 exports
pub use ast_analyzer::{AstAnalyzer, AstAnalysisResult, FunctionInfo};
pub use enhanced_analyzer::{EnhancedAnalyzer, EnhancedAnalysisResult, QuickAnalysisResult};
pub use git_integration::{GitIntegration, GitStatus, CommitInfo};
pub use test_generator::{TestGenerator, GeneratedTests, Test, TestType, TestFileWriter};

use anyhow::Result;

/// Initialize the code review module
pub fn init() -> Result<()> {
    tracing::info!("Initializing continuous code review module");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        assert!(init().is_ok());
    }
}
