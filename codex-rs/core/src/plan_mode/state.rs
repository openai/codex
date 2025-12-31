//! Plan Mode state management.

use std::path::PathBuf;

use codex_protocol::ConversationId;

use super::file_management::get_plan_file_path;
use crate::error::CodexErr;

/// Plan Mode session state.
///
/// Tracks the current session's Plan Mode state, including whether it's active,
/// the plan file path, and re-entry detection.
/// Stored in SubagentStores with session lifetime.
///
/// ## Re-entry Detection
///
/// Uses slug caching (aligned with Claude Code): same session = same plan file.
/// When user re-enters Plan Mode with `has_approved = true` and plan file exists,
/// re-entry content is injected to guide decision (overwrite vs continue).
#[derive(Debug, Clone, Default)]
pub struct PlanModeState {
    /// Whether Plan Mode is currently active.
    pub is_active: bool,

    /// Plan file path (e.g., ~/.codex/plans/bright-exploring-aurora.md).
    /// Uses cached slug - same session always gets the same path.
    pub plan_file_path: Option<PathBuf>,

    /// Whether user has approved a plan before (for re-entry detection).
    /// Set to true when user approves plan and exits Plan Mode.
    /// If re-entering Plan Mode with existing plan file, triggers re-entry logic.
    pub has_approved: bool,

    /// Conversation ID (used for file name generation).
    pub conversation_id: Option<ConversationId>,
}

impl PlanModeState {
    /// Create a new Plan Mode state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enter Plan Mode.
    ///
    /// Generates a new plan file path and activates Plan Mode.
    ///
    /// # Arguments
    /// * `conversation_id` - Current conversation ID
    ///
    /// # Returns
    /// Plan file path on success, or error if unable to get directory
    pub fn enter(&mut self, conversation_id: ConversationId) -> Result<PathBuf, CodexErr> {
        let plan_file_path = get_plan_file_path(&conversation_id)?;

        self.is_active = true;
        self.plan_file_path = Some(plan_file_path.clone());
        self.conversation_id = Some(conversation_id);
        // has_approved preserved for re-entry detection

        Ok(plan_file_path)
    }

    /// Exit Plan Mode.
    ///
    /// # Arguments
    /// * `approved` - Whether user approved the plan
    pub fn exit(&mut self, approved: bool) {
        self.is_active = false;
        if approved {
            self.has_approved = true;
        }
        // plan_file_path preserved for re-entry to read old plan
    }

    /// Check if this is a re-entry situation.
    ///
    /// Re-entry conditions:
    /// 1. Previously approved a plan (has_approved == true)
    /// 2. Plan file still exists
    pub fn is_reentry(&self) -> bool {
        if !self.has_approved {
            return false;
        }

        match &self.plan_file_path {
            Some(path) => path.exists(),
            None => false,
        }
    }

    /// Clear re-entry flag after re-entry prompt is sent.
    pub fn clear_reentry(&mut self) {
        self.has_approved = false;
    }
}
