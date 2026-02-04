//! Throttle management for system reminder generators.
//!
//! This module provides rate limiting for generators to prevent
//! excessive reminder injection.

use std::collections::HashMap;
use std::sync::RwLock;

use crate::types::AttachmentType;

/// Throttle configuration for a generator.
#[derive(Debug, Clone, Copy)]
pub struct ThrottleConfig {
    /// Minimum turns between generating this reminder.
    pub min_turns_between: i32,
    /// Minimum turns after a triggering event before generating.
    pub min_turns_after_trigger: i32,
    /// Maximum times to generate per session (None = unlimited).
    pub max_per_session: Option<i32>,
}

impl Default for ThrottleConfig {
    fn default() -> Self {
        Self {
            min_turns_between: 0,
            min_turns_after_trigger: 0,
            max_per_session: None,
        }
    }
}

impl ThrottleConfig {
    /// No throttling - generate every turn.
    pub fn none() -> Self {
        Self::default()
    }

    /// Standard throttle for plan mode reminders.
    pub fn plan_mode() -> Self {
        Self {
            min_turns_between: 5,
            min_turns_after_trigger: 0,
            max_per_session: None,
        }
    }

    /// Standard throttle for plan tool reminders.
    pub fn plan_tool_reminder() -> Self {
        Self {
            min_turns_between: 3,
            min_turns_after_trigger: 5,
            max_per_session: None,
        }
    }

    /// Standard throttle for todo reminders.
    pub fn todo_reminder() -> Self {
        Self {
            min_turns_between: 5,
            min_turns_after_trigger: 0,
            max_per_session: None,
        }
    }

    /// Standard throttle for output style.
    /// Injects once per session at the start, consistent with Claude Code behavior.
    pub fn output_style() -> Self {
        Self {
            min_turns_between: 0,
            min_turns_after_trigger: 0,
            max_per_session: Some(1),
        }
    }
}

/// State tracking for a single attachment type.
#[derive(Debug, Clone, Default)]
pub struct ThrottleState {
    /// Turn number when this was last generated.
    pub last_generated_turn: Option<i32>,
    /// Number of times generated this session.
    pub session_count: i32,
    /// Turn number when the trigger event occurred.
    pub trigger_turn: Option<i32>,
}

/// Manager for tracking throttle state across generators.
#[derive(Debug, Default)]
pub struct ThrottleManager {
    state: RwLock<HashMap<AttachmentType, ThrottleState>>,
}

impl ThrottleManager {
    /// Create a new throttle manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a generator should be allowed to run.
    ///
    /// # Arguments
    ///
    /// * `attachment_type` - The type of attachment being generated
    /// * `config` - The throttle configuration for this generator
    /// * `current_turn` - The current turn number
    /// * `trigger_turn` - Optional turn when a trigger event occurred
    pub fn should_generate(
        &self,
        attachment_type: AttachmentType,
        config: &ThrottleConfig,
        current_turn: i32,
    ) -> bool {
        let state = self.state.read().expect("lock poisoned");
        let entry = state.get(&attachment_type);

        match entry {
            None => true, // Never generated, allow
            Some(s) => {
                // Check min_turns_after_trigger
                if let Some(trigger) = s.trigger_turn {
                    if current_turn - trigger < config.min_turns_after_trigger {
                        return false;
                    }
                }

                // Check min_turns_between
                if let Some(last) = s.last_generated_turn {
                    if current_turn - last < config.min_turns_between {
                        return false;
                    }
                }

                // Check max_per_session
                if let Some(max) = config.max_per_session {
                    if s.session_count >= max {
                        return false;
                    }
                }

                true
            }
        }
    }

    /// Mark that a generator successfully generated output.
    pub fn mark_generated(&self, attachment_type: AttachmentType, turn: i32) {
        let mut state = self.state.write().expect("lock poisoned");
        let entry = state.entry(attachment_type).or_default();
        entry.last_generated_turn = Some(turn);
        entry.session_count += 1;
    }

    /// Set the trigger turn for an attachment type.
    pub fn set_trigger_turn(&self, attachment_type: AttachmentType, turn: i32) {
        let mut state = self.state.write().expect("lock poisoned");
        let entry = state.entry(attachment_type).or_default();
        entry.trigger_turn = Some(turn);
    }

    /// Clear the trigger turn for an attachment type.
    pub fn clear_trigger_turn(&self, attachment_type: AttachmentType) {
        let mut state = self.state.write().expect("lock poisoned");
        if let Some(entry) = state.get_mut(&attachment_type) {
            entry.trigger_turn = None;
        }
    }

    /// Reset all throttle state (e.g., at session start).
    pub fn reset(&self) {
        let mut state = self.state.write().expect("lock poisoned");
        state.clear();
    }

    /// Get the current state for an attachment type.
    pub fn get_state(&self, attachment_type: AttachmentType) -> Option<ThrottleState> {
        let state = self.state.read().expect("lock poisoned");
        state.get(&attachment_type).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_throttle_config() {
        let config = ThrottleConfig::default();
        assert_eq!(config.min_turns_between, 0);
        assert_eq!(config.min_turns_after_trigger, 0);
        assert!(config.max_per_session.is_none());
    }

    #[test]
    fn test_throttle_manager_first_time() {
        let manager = ThrottleManager::new();
        let config = ThrottleConfig::default();

        // First time should always be allowed
        assert!(manager.should_generate(AttachmentType::ChangedFiles, &config, 1));
    }

    #[test]
    fn test_throttle_manager_min_turns_between() {
        let manager = ThrottleManager::new();
        let config = ThrottleConfig {
            min_turns_between: 3,
            min_turns_after_trigger: 0,
            max_per_session: None,
        };

        // Mark as generated at turn 1
        manager.mark_generated(AttachmentType::ChangedFiles, 1);

        // Should be blocked at turns 2, 3
        assert!(!manager.should_generate(AttachmentType::ChangedFiles, &config, 2));
        assert!(!manager.should_generate(AttachmentType::ChangedFiles, &config, 3));

        // Should be allowed at turn 4
        assert!(manager.should_generate(AttachmentType::ChangedFiles, &config, 4));
    }

    #[test]
    fn test_throttle_manager_max_per_session() {
        let manager = ThrottleManager::new();
        let config = ThrottleConfig {
            min_turns_between: 0,
            min_turns_after_trigger: 0,
            max_per_session: Some(2),
        };

        // First two should be allowed
        assert!(manager.should_generate(AttachmentType::TodoReminders, &config, 1));
        manager.mark_generated(AttachmentType::TodoReminders, 1);

        assert!(manager.should_generate(AttachmentType::TodoReminders, &config, 2));
        manager.mark_generated(AttachmentType::TodoReminders, 2);

        // Third should be blocked
        assert!(!manager.should_generate(AttachmentType::TodoReminders, &config, 3));
    }

    #[test]
    fn test_throttle_manager_trigger_turn() {
        let manager = ThrottleManager::new();
        let config = ThrottleConfig {
            min_turns_between: 0,
            min_turns_after_trigger: 5,
            max_per_session: None,
        };

        // Set trigger at turn 1
        manager.set_trigger_turn(AttachmentType::PlanToolReminder, 1);

        // Should be blocked until turn 6
        assert!(!manager.should_generate(AttachmentType::PlanToolReminder, &config, 2));
        assert!(!manager.should_generate(AttachmentType::PlanToolReminder, &config, 5));
        assert!(manager.should_generate(AttachmentType::PlanToolReminder, &config, 6));
    }

    #[test]
    fn test_throttle_manager_reset() {
        let manager = ThrottleManager::new();
        let config = ThrottleConfig {
            min_turns_between: 10,
            min_turns_after_trigger: 0,
            max_per_session: None,
        };

        manager.mark_generated(AttachmentType::ChangedFiles, 1);
        assert!(!manager.should_generate(AttachmentType::ChangedFiles, &config, 2));

        manager.reset();
        assert!(manager.should_generate(AttachmentType::ChangedFiles, &config, 2));
    }

    #[test]
    fn test_predefined_configs() {
        let plan_mode = ThrottleConfig::plan_mode();
        assert_eq!(plan_mode.min_turns_between, 5);

        let plan_tool = ThrottleConfig::plan_tool_reminder();
        assert_eq!(plan_tool.min_turns_between, 3);
        assert_eq!(plan_tool.min_turns_after_trigger, 5);

        let todo = ThrottleConfig::todo_reminder();
        assert_eq!(todo.min_turns_between, 5);

        let output_style = ThrottleConfig::output_style();
        assert_eq!(output_style.min_turns_between, 0);
        assert_eq!(output_style.max_per_session, Some(1));
    }
}
