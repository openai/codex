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
mod config;
mod fixer;
pub mod linters;
mod reviewer;
mod session;
mod tools;
mod watcher;

pub use ai_client::{AIClient, AIAnalysisResult, QuickCheckType};
pub use analyzer::{AnalysisResult, CodeAnalyzer, Issue, IssueSeverity, IssueCategory, Suggestion};
pub use config::{ReviewConfig, ReviewPolicy, ReviewTrigger, LocalAIConfig, AnalysisConfig};
pub use fixer::{CodeFixer, FixReport};
pub use linters::{Linter, LinterRegistry};
pub use reviewer::{ContinuousReviewer, ReviewTask, ReviewType};
pub use session::{ReviewSession, ReviewSessionState, SessionManager};
pub use tools::{register_review_tools, ReviewTool};
pub use watcher::{FileWatcher, WatchEvent};

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
