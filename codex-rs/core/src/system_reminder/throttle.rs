//! Throttle system for system reminders.
//!
//! Manages frequency of reminder generation to avoid spam.
//! Matches GY2 and IH5 constants in Claude Code chunks.107.mjs.

use super::types::AttachmentType;
use std::collections::HashMap;
use std::sync::RwLock;

// ============================================
// Throttle Configuration
// ============================================

/// Throttle configuration per attachment type.
///
/// Matches GY2 and IH5 constants in Claude Code.
#[derive(Debug, Clone)]
pub struct ThrottleConfig {
    /// Minimum turns between reminders (0 = every turn).
    pub min_turns_between: i32,
    /// Minimum turns after triggering event (e.g., since last TodoWrite).
    pub min_turns_after_trigger: i32,
    /// Maximum reminders per session (None = unlimited).
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

/// Default throttle configurations matching Claude Code.
///
/// GY2.TURNS_SINCE_UPDATE = 5, GY2.TURNS_BETWEEN_REMINDERS = 3
/// IH5.TURNS_BETWEEN_ATTACHMENTS = varies
pub fn default_throttle_config(attachment_type: AttachmentType) -> ThrottleConfig {
    match attachment_type {
        AttachmentType::PlanToolReminder => ThrottleConfig {
            min_turns_between: 3,       // TURNS_BETWEEN_REMINDERS
            min_turns_after_trigger: 5, // TURNS_SINCE_UPDATE
            max_per_session: None,
        },
        AttachmentType::PlanMode => ThrottleConfig {
            min_turns_between: 5, // After first, every 5+ turns
            min_turns_after_trigger: 0,
            max_per_session: None,
        },
        AttachmentType::ChangedFiles => ThrottleConfig {
            min_turns_between: 0, // Immediate notification
            min_turns_after_trigger: 0,
            max_per_session: None,
        },
        _ => ThrottleConfig::default(),
    }
}

// ============================================
// Throttle State
// ============================================

/// Tracks throttle state per attachment type.
#[derive(Debug, Clone)]
pub struct ThrottleState {
    /// Last turn this attachment was generated.
    pub last_generated_turn: i32,
    /// Total count generated this session.
    pub session_count: i32,
}

impl ThrottleState {
    /// Create new throttle state with safe initial values.
    pub fn new() -> Self {
        Self {
            last_generated_turn: i32::MIN / 2, // Safe initial value to avoid overflow
            session_count: 0,
        }
    }
}

impl Default for ThrottleState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================
// Throttle Manager
// ============================================

/// Manages throttle state for all attachment types.
pub struct ThrottleManager {
    states: RwLock<HashMap<AttachmentType, ThrottleState>>,
}

impl ThrottleManager {
    /// Create a new throttle manager.
    pub fn new() -> Self {
        Self {
            states: RwLock::new(HashMap::new()),
        }
    }

    /// Check if a generator should run based on throttle rules.
    pub fn should_generate(
        &self,
        attachment_type: AttachmentType,
        current_turn: i32,
        trigger_turn: Option<i32>,
    ) -> bool {
        let config = default_throttle_config(attachment_type);

        // Check min_turns_after_trigger (applies even without prior state)
        if let Some(trigger) = trigger_turn {
            let turns_since_trigger = current_turn.saturating_sub(trigger);
            if turns_since_trigger < config.min_turns_after_trigger {
                return false;
            }
        }

        let states = self.states.read().expect("throttle lock poisoned");

        if let Some(state) = states.get(&attachment_type) {
            let turns_since = current_turn.saturating_sub(state.last_generated_turn);

            // Check min_turns_between
            if turns_since < config.min_turns_between {
                return false;
            }

            // Check max_per_session
            if let Some(max) = config.max_per_session {
                if state.session_count >= max {
                    return false;
                }
            }
        }

        true
    }

    /// Mark that a generator produced output.
    pub fn mark_generated(&self, attachment_type: AttachmentType, current_turn: i32) {
        let mut states = self.states.write().expect("throttle lock poisoned");
        let state = states
            .entry(attachment_type)
            .or_insert_with(ThrottleState::new);
        state.last_generated_turn = current_turn;
        state.session_count += 1;
    }

    /// Reset all throttle state (call at session start).
    pub fn reset(&self) {
        let mut states = self.states.write().expect("throttle lock poisoned");
        states.clear();
    }
}

impl Default for ThrottleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ThrottleManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThrottleManager").finish()
    }
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_throttle_config_default() {
        let config = ThrottleConfig::default();
        assert_eq!(config.min_turns_between, 0);
        assert_eq!(config.min_turns_after_trigger, 0);
        assert!(config.max_per_session.is_none());
    }

    #[test]
    fn test_default_throttle_config_plan_tool_reminder() {
        let config = default_throttle_config(AttachmentType::PlanToolReminder);
        assert_eq!(config.min_turns_between, 3);
        assert_eq!(config.min_turns_after_trigger, 5);
    }

    #[test]
    fn test_default_throttle_config_plan_mode() {
        let config = default_throttle_config(AttachmentType::PlanMode);
        assert_eq!(config.min_turns_between, 5);
    }

    #[test]
    fn test_throttle_manager_first_always_allowed() {
        let manager = ThrottleManager::new();
        assert!(manager.should_generate(AttachmentType::PlanToolReminder, 1, None));
    }

    #[test]
    fn test_throttle_manager_respects_min_turns() {
        let manager = ThrottleManager::new();

        // First generation always allowed
        assert!(manager.should_generate(AttachmentType::PlanToolReminder, 1, None));
        manager.mark_generated(AttachmentType::PlanToolReminder, 1);

        // Too soon (min_turns_between = 3)
        assert!(!manager.should_generate(AttachmentType::PlanToolReminder, 2, None));
        assert!(!manager.should_generate(AttachmentType::PlanToolReminder, 3, None));

        // After min_turns_between
        assert!(manager.should_generate(AttachmentType::PlanToolReminder, 4, None));
    }

    #[test]
    fn test_throttle_manager_respects_trigger_turn() {
        let manager = ThrottleManager::new();

        // Trigger turn too recent (min_turns_after_trigger = 5)
        assert!(!manager.should_generate(AttachmentType::PlanToolReminder, 3, Some(1)));

        // After min_turns_after_trigger
        assert!(manager.should_generate(AttachmentType::PlanToolReminder, 7, Some(1)));
    }

    #[test]
    fn test_throttle_manager_reset() {
        let manager = ThrottleManager::new();
        manager.mark_generated(AttachmentType::PlanToolReminder, 1);
        manager.reset();

        // After reset, should generate again from turn 1
        assert!(manager.should_generate(AttachmentType::PlanToolReminder, 2, None));
    }

    #[test]
    fn test_throttle_state_new() {
        let state = ThrottleState::new();
        assert!(state.last_generated_turn < 0);
        assert_eq!(state.session_count, 0);
    }

    #[test]
    fn test_changed_files_no_throttle() {
        let config = default_throttle_config(AttachmentType::ChangedFiles);
        assert_eq!(config.min_turns_between, 0);
        assert_eq!(config.min_turns_after_trigger, 0);
    }
}
