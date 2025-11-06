//! Continuous Code Review Module
//!
//! This module provides continuous code review capabilities using local AI models.
//! It watches for code changes, analyzes them, and provides improvement suggestions.

mod analyzer;
mod config;
mod reviewer;
mod session;
mod tools;
mod watcher;

pub use analyzer::{AnalysisResult, CodeAnalyzer, Issue, IssueSeverity};
pub use config::{ReviewConfig, ReviewPolicy, ReviewTrigger};
pub use reviewer::{ContinuousReviewer, ReviewTask, ReviewType};
pub use session::{ReviewSession, ReviewSessionState};
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
