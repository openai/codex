//! Plan mode state management.
//!
//! Tracks the state of plan mode across a session, including the active
//! plan file path and mode transitions.

use std::path::Path;
use std::path::PathBuf;

/// Plan mode state for a session.
///
/// Tracks whether plan mode is active, the current plan file path,
/// and state transitions for re-entry detection.
#[derive(Debug, Clone, Default)]
pub struct PlanModeState {
    /// Whether plan mode is currently active.
    pub is_active: bool,
    /// Path to the current plan file.
    pub plan_file_path: Option<PathBuf>,
    /// The plan slug for this session.
    pub plan_slug: Option<String>,
    /// Whether the user has exited plan mode at least once.
    pub has_exited: bool,
    /// Whether the exit notification needs to be attached (one-time).
    pub needs_exit_attachment: bool,
    /// Turn number when plan mode was entered.
    pub entered_at_turn: Option<i32>,
    /// Turn number when plan mode was exited.
    pub exited_at_turn: Option<i32>,
}

impl PlanModeState {
    /// Create a new empty plan mode state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enter plan mode with the given plan file path and slug.
    pub fn enter(&mut self, plan_file_path: PathBuf, slug: String, turn: i32) {
        self.is_active = true;
        self.plan_file_path = Some(plan_file_path);
        self.plan_slug = Some(slug);
        self.entered_at_turn = Some(turn);
        self.needs_exit_attachment = false;
    }

    /// Exit plan mode.
    pub fn exit(&mut self, turn: i32) {
        self.is_active = false;
        self.has_exited = true;
        self.exited_at_turn = Some(turn);
        self.needs_exit_attachment = true;
    }

    /// Clear the exit attachment flag after it has been sent.
    pub fn clear_exit_attachment(&mut self) {
        self.needs_exit_attachment = false;
    }

    /// Check if this is a re-entry into plan mode.
    pub fn is_reentry(&self) -> bool {
        self.has_exited && self.is_active
    }

    /// Get the plan file path if in plan mode.
    pub fn get_plan_file(&self) -> Option<&Path> {
        if self.is_active {
            self.plan_file_path.as_deref()
        } else {
            None
        }
    }
}

/// Check if a file path is the current plan file (safe for writing in plan mode).
///
/// This function is used by the permission system to allow Write/Edit tool
/// access to the plan file even when in plan mode.
///
/// # Arguments
///
/// * `path` - The file path to check
/// * `plan_file_path` - The current plan file path (if in plan mode)
///
/// # Returns
///
/// `true` if the path matches the plan file, allowing write access.
pub fn is_safe_file(path: &Path, plan_file_path: Option<&Path>) -> bool {
    plan_file_path.is_some_and(|plan_path| path == plan_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_mode_state_lifecycle() {
        let mut state = PlanModeState::new();

        // Initial state
        assert!(!state.is_active);
        assert!(state.plan_file_path.is_none());
        assert!(!state.has_exited);

        // Enter plan mode
        let path = PathBuf::from("/home/user/.cocode/plans/test-plan.md");
        state.enter(path.clone(), "test-plan".to_string(), 1);

        assert!(state.is_active);
        assert_eq!(state.plan_file_path, Some(path.clone()));
        assert_eq!(state.plan_slug, Some("test-plan".to_string()));
        assert_eq!(state.entered_at_turn, Some(1));
        assert!(!state.is_reentry());

        // Exit plan mode
        state.exit(5);

        assert!(!state.is_active);
        assert!(state.has_exited);
        assert!(state.needs_exit_attachment);
        assert_eq!(state.exited_at_turn, Some(5));

        // Clear exit attachment
        state.clear_exit_attachment();
        assert!(!state.needs_exit_attachment);

        // Re-enter plan mode
        state.enter(path.clone(), "test-plan".to_string(), 6);
        assert!(state.is_active);
        assert!(state.is_reentry());
    }

    #[test]
    fn test_get_plan_file() {
        let mut state = PlanModeState::new();
        let path = PathBuf::from("/home/user/.cocode/plans/test.md");

        // Not in plan mode
        assert!(state.get_plan_file().is_none());

        // In plan mode
        state.enter(path.clone(), "test".to_string(), 1);
        assert_eq!(state.get_plan_file(), Some(path.as_path()));

        // Exited plan mode
        state.exit(2);
        assert!(state.get_plan_file().is_none());
    }

    #[test]
    fn test_is_safe_file() {
        let plan_path = PathBuf::from("/home/user/.cocode/plans/test-plan.md");
        let other_path = PathBuf::from("/home/user/project/src/main.rs");

        // No plan file set
        assert!(!is_safe_file(&plan_path, None));
        assert!(!is_safe_file(&other_path, None));

        // Plan file set
        assert!(is_safe_file(&plan_path, Some(&plan_path)));
        assert!(!is_safe_file(&other_path, Some(&plan_path)));
    }
}
