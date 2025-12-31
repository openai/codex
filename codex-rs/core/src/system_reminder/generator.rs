//! Attachment generator trait and context.
//!
//! Defines the interface for generating system reminder attachments.

use super::file_tracker::FileTracker;
use super::throttle::ThrottleConfig;
use super::types::AttachmentType;
use super::types::ReminderTier;
use super::types::SystemReminder;
use crate::config::output_style::OutputStyle;
use crate::config::system_reminder::LspDiagnosticsMinSeverity;
use crate::config::system_reminder::SystemReminderConfig;
use crate::error::Result;
use async_trait::async_trait;
use codex_lsp::DiagnosticsStore;
use std::path::Path;
use std::sync::Arc;

// ============================================
// Generator Trait
// ============================================

/// Trait for attachment generators.
///
/// Matches structure of individual generator functions in Claude Code.
#[async_trait]
pub trait AttachmentGenerator: Send + Sync + std::fmt::Debug {
    /// Unique name for this generator (for telemetry).
    fn name(&self) -> &str;

    /// Type of attachment this generator produces.
    fn attachment_type(&self) -> AttachmentType;

    /// Which tier this generator belongs to.
    fn tier(&self) -> ReminderTier {
        self.attachment_type().tier()
    }

    /// Generate attachment if applicable, returns None if not applicable this turn.
    /// This is the main entry point, called by orchestrator.
    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>>;

    /// Check if generator is enabled based on config.
    fn is_enabled(&self, config: &SystemReminderConfig) -> bool;

    /// Get throttle configuration for this generator.
    fn throttle_config(&self) -> ThrottleConfig {
        ThrottleConfig::default()
    }
}

// ============================================
// Generator Context
// ============================================

/// Context provided to attachment generators.
///
/// Matches context parameter in Claude Code's generator functions.
#[derive(Debug)]
pub struct GeneratorContext<'a> {
    /// Current turn number in the conversation.
    pub turn_number: i32,
    /// Whether this is the main agent (not a sub-agent).
    pub is_main_agent: bool,
    /// Whether this turn has user input.
    pub has_user_input: bool,
    /// Raw user prompt text (for @mention parsing in UserPrompt tier generators).
    pub user_prompt: Option<&'a str>,
    /// Current working directory.
    pub cwd: &'a Path,
    /// Session/Agent ID.
    pub agent_id: &'a str,
    /// File tracking state (for change detection).
    pub file_tracker: &'a FileTracker,
    /// Whether plan mode is active.
    pub is_plan_mode: bool,
    /// Plan file path (if in plan mode).
    pub plan_file_path: Option<&'a str>,
    /// Whether re-entering plan mode.
    pub is_plan_reentry: bool,
    /// Current plan state (for reminder tracking).
    pub plan_state: &'a PlanState,
    /// Background task status.
    pub background_tasks: &'a [BackgroundTaskInfo],
    /// Critical instruction from config.
    pub critical_instruction: Option<&'a str>,
    /// LSP diagnostics store (optional, only available when LSP is enabled).
    pub diagnostics_store: Option<Arc<DiagnosticsStore>>,
    /// Minimum severity level for LSP diagnostics filtering.
    pub lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity,
    /// Currently active output style (None = default).
    pub output_style: Option<&'a OutputStyle>,
    /// Approved plan content for post-ExitPlanMode injection (one-time).
    pub approved_plan: Option<ApprovedPlanInfo>,
}

/// Approved plan info for one-time injection.
#[derive(Debug, Clone)]
pub struct ApprovedPlanInfo {
    /// Full plan content
    pub content: String,
    /// Path to the plan file
    pub file_path: String,
}

// ============================================
// Supporting Types
// ============================================

/// Current state of the plan (for reminder tracking).
#[derive(Debug, Clone)]
pub struct PlanState {
    /// Whether the plan is empty.
    pub is_empty: bool,
    /// Inject call count when plan was last updated.
    pub last_update_count: i32,
    /// Current plan steps.
    pub steps: Vec<PlanStep>,
}

impl Default for PlanState {
    fn default() -> Self {
        Self {
            is_empty: true,
            last_update_count: 0,
            steps: vec![],
        }
    }
}

/// A single plan step.
#[derive(Debug, Clone)]
pub struct PlanStep {
    /// Step description.
    pub step: String,
    /// Status: "pending", "in_progress", "completed".
    pub status: String,
}

/// Information about a background task.
#[derive(Debug, Clone)]
pub struct BackgroundTaskInfo {
    /// Unique task identifier.
    pub task_id: String,
    /// Type of background task.
    pub task_type: BackgroundTaskType,
    /// Command being executed (for shell tasks).
    pub command: Option<String>,
    /// Human-readable description.
    pub description: String,
    /// Current status.
    pub status: BackgroundTaskStatus,
    /// Exit code (if completed).
    pub exit_code: Option<i32>,
    /// Whether there's new output available.
    pub has_new_output: bool,
    /// Whether completion has been notified.
    pub notified: bool,
}

/// Type of background task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackgroundTaskType {
    /// Background shell command.
    Shell,
    /// Async agent execution.
    AsyncAgent,
    /// Remote session.
    RemoteSession,
}

/// Status of a background task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackgroundTaskStatus {
    /// Task is currently running.
    Running,
    /// Task completed successfully.
    Completed,
    /// Task failed.
    Failed,
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_state_default() {
        let state = PlanState::default();
        assert!(state.is_empty);
        assert_eq!(state.last_update_count, 0);
        assert!(state.steps.is_empty());
    }

    #[test]
    fn test_background_task_info() {
        let task = BackgroundTaskInfo {
            task_id: "task-1".to_string(),
            task_type: BackgroundTaskType::Shell,
            command: Some("npm test".to_string()),
            description: "Running tests".to_string(),
            status: BackgroundTaskStatus::Running,
            exit_code: None,
            has_new_output: true,
            notified: false,
        };

        assert_eq!(task.task_type, BackgroundTaskType::Shell);
        assert_eq!(task.status, BackgroundTaskStatus::Running);
        assert!(task.has_new_output);
    }

    #[test]
    fn test_background_task_status_equality() {
        assert_eq!(BackgroundTaskStatus::Running, BackgroundTaskStatus::Running);
        assert_ne!(
            BackgroundTaskStatus::Running,
            BackgroundTaskStatus::Completed
        );
    }
}
