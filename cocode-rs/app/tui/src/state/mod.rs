//! Application state management for the TUI.
//!
//! This module provides:
//! - [`AppState`]: The complete application state
//! - [`SessionState`]: State from the agent session
//! - [`UiState`]: UI-specific state (input, scroll, overlays)

mod session;
mod ui;

pub use session::ChatMessage;
pub use session::McpServerStatus;
pub use session::MessageRole;
pub use session::PlanPhase;
pub use session::SessionState;
pub use session::SubagentInstance;
pub use session::SubagentStatus;
pub use session::ToolExecution;
pub use session::ToolStatus;
pub use ui::CommandAction;
pub use ui::CommandItem;
pub use ui::CommandPaletteOverlay;
pub use ui::FileSuggestionItem;
pub use ui::FileSuggestionState;
pub use ui::FocusTarget;
pub use ui::HistoryEntry;
pub use ui::InputState;
pub use ui::ModelPickerOverlay;
pub use ui::Overlay;
pub use ui::PermissionOverlay;
pub use ui::SessionBrowserOverlay;
pub use ui::SessionSummary;
pub use ui::SkillSuggestionItem;
pub use ui::SkillSuggestionState;
pub use ui::StreamingState;
pub use ui::UiState;

// Re-export theme types for convenience
pub use crate::theme::Theme;
pub use crate::theme::ThemeName;

use cocode_protocol::ReasoningEffort;
use cocode_protocol::ThinkingLevel;

/// The complete application state.
///
/// This is the "Model" in the Elm Architecture pattern. All application
/// state is contained here and updated immutably in response to events.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Session state (from the agent).
    pub session: SessionState,

    /// UI state (local to the TUI).
    pub ui: UiState,

    /// Current running state.
    pub running: RunningState,
}

impl AppState {
    /// Create a new application state with default values.
    pub fn new() -> Self {
        Self {
            session: SessionState::default(),
            ui: UiState::default(),
            running: RunningState::Running,
        }
    }

    /// Create a new application state with the specified model.
    pub fn with_model(model: impl Into<String>) -> Self {
        let mut state = Self::new();
        state.session.current_model = model.into();
        state
    }

    /// Check if the application should exit.
    pub fn should_exit(&self) -> bool {
        matches!(self.running, RunningState::Done)
    }

    /// Toggle plan mode.
    pub fn toggle_plan_mode(&mut self) {
        self.session.plan_mode = !self.session.plan_mode;
        tracing::info!(plan_mode = self.session.plan_mode, "Plan mode toggled");
    }

    /// Cycle to the next thinking level.
    pub fn cycle_thinking_level(&mut self) {
        let next_effort = match self.session.thinking_level.effort {
            ReasoningEffort::None => ReasoningEffort::Low,
            ReasoningEffort::Minimal => ReasoningEffort::Low,
            ReasoningEffort::Low => ReasoningEffort::Medium,
            ReasoningEffort::Medium => ReasoningEffort::High,
            ReasoningEffort::High => ReasoningEffort::XHigh,
            ReasoningEffort::XHigh => ReasoningEffort::None,
        };
        self.session.thinking_level = ThinkingLevel::new(next_effort);
        tracing::info!(
            thinking_level = ?self.session.thinking_level.effort,
            "Thinking level cycled"
        );
    }

    /// Set the running state to done.
    pub fn quit(&mut self) {
        self.running = RunningState::Done;
    }

    /// Check if there's an active overlay.
    pub fn has_overlay(&self) -> bool {
        self.ui.overlay.is_some()
    }

    /// Check if the agent is currently streaming a response.
    pub fn is_streaming(&self) -> bool {
        self.ui.streaming.is_some()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// The running state of the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunningState {
    /// The application is running normally.
    #[default]
    Running,

    /// The application is done and should exit.
    Done,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_new() {
        let state = AppState::new();
        assert!(!state.should_exit());
        assert!(!state.session.plan_mode);
        assert!(!state.has_overlay());
    }

    #[test]
    fn test_toggle_plan_mode() {
        let mut state = AppState::new();
        assert!(!state.session.plan_mode);

        state.toggle_plan_mode();
        assert!(state.session.plan_mode);

        state.toggle_plan_mode();
        assert!(!state.session.plan_mode);
    }

    #[test]
    fn test_cycle_thinking_level() {
        let mut state = AppState::new();

        // Start at None
        assert_eq!(state.session.thinking_level.effort, ReasoningEffort::None);

        // Cycle through levels
        state.cycle_thinking_level();
        assert_eq!(state.session.thinking_level.effort, ReasoningEffort::Low);

        state.cycle_thinking_level();
        assert_eq!(state.session.thinking_level.effort, ReasoningEffort::Medium);

        state.cycle_thinking_level();
        assert_eq!(state.session.thinking_level.effort, ReasoningEffort::High);

        state.cycle_thinking_level();
        assert_eq!(state.session.thinking_level.effort, ReasoningEffort::XHigh);

        state.cycle_thinking_level();
        assert_eq!(state.session.thinking_level.effort, ReasoningEffort::None);
    }

    #[test]
    fn test_quit() {
        let mut state = AppState::new();
        assert!(!state.should_exit());

        state.quit();
        assert!(state.should_exit());
    }

    #[test]
    fn test_with_model() {
        let state = AppState::with_model("gpt-4");
        assert_eq!(state.session.current_model, "gpt-4");
    }
}
