//! Session-level thinking state.

/// Session-level thinking state (not persisted to config).
///
/// This state is held by the TUI/App and tracks whether the user has
/// toggled ultrathink mode on for the current session.
#[derive(Debug, Clone)]
pub struct ThinkingState {
    /// Whether ultrathink toggle is ON for this session.
    ///
    /// When true, the effective think level uses the model's `ultrathink_level`
    /// instead of `default_think_level`.
    pub ultrathink_enabled: bool,
}

impl Default for ThinkingState {
    fn default() -> Self {
        Self {
            ultrathink_enabled: false,
        }
    }
}

impl ThinkingState {
    /// Create a new thinking state with ultrathink disabled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle ultrathink mode and return the new state.
    pub fn toggle(&mut self) -> bool {
        self.ultrathink_enabled = !self.ultrathink_enabled;
        self.ultrathink_enabled
    }

    /// Check if ultrathink is enabled.
    pub fn is_ultrathink_enabled(&self) -> bool {
        self.ultrathink_enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state() {
        let state = ThinkingState::default();
        assert!(!state.is_ultrathink_enabled());
    }

    #[test]
    fn test_toggle() {
        let mut state = ThinkingState::new();
        assert!(!state.is_ultrathink_enabled());

        let new_state = state.toggle();
        assert!(new_state);
        assert!(state.is_ultrathink_enabled());

        let new_state = state.toggle();
        assert!(!new_state);
        assert!(!state.is_ultrathink_enabled());
    }
}
