//! Ultrathink integration with LLM requests.
//!
//! This module provides the core logic for computing effective reasoning effort
//! based on keyword detection, session toggle, and configuration hierarchy.

use crate::thinking::detector;
use crate::thinking::state::ThinkingState;
use crate::thinking::types::UltrathinkConfig;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort;

/// Result of computing the effective reasoning effort for a turn.
#[derive(Debug, Clone)]
pub struct EffortResult {
    /// The effective reasoning effort for this turn.
    pub effort: ReasoningEffort,
    /// Token budget when ultrathink is triggered (for budget-based models).
    pub budget_tokens: i32,
    /// Whether the "ultrathink" keyword was detected in the message.
    pub keyword_detected: bool,
    /// Whether ultrathink mode is active (via keyword or toggle).
    pub ultrathink_active: bool,
    /// Whether effort was explicitly set (not from hardcoded fallback).
    /// When true, effort should be included in the request.
    /// When false, effort should be omitted (model family has no default).
    pub effort_explicit: bool,
}

impl Default for EffortResult {
    fn default() -> Self {
        Self {
            effort: ReasoningEffort::Medium,
            budget_tokens: 0,
            keyword_detected: false,
            ultrathink_active: false,
            effort_explicit: false,
        }
    }
}

/// Compute the effective reasoning effort for this turn.
///
/// Priority (highest to lowest):
/// 1. "ultrathink" keyword in message → ultrathink_config.effort
/// 2. Session toggle ON → ultrathink_config.effort
/// 3. Per-turn effort override
/// 4. Global config effort
/// 5. ModelInfo.default_reasoning_level
///
/// # Arguments
/// * `message` - The user's message to check for "ultrathink" keyword
/// * `thinking_state` - Session-level ultrathink toggle state
/// * `per_turn_effort` - Per-turn effort override from UI
/// * `config_effort` - Global config.model_reasoning_effort
/// * `model_info` - Model info for default effort
/// * `ultrathink_config` - Optional ultrathink configuration from provider
pub fn compute_effective_effort(
    message: &str,
    thinking_state: &ThinkingState,
    per_turn_effort: Option<ReasoningEffort>,
    config_effort: Option<ReasoningEffort>,
    model_info: &ModelInfo,
    ultrathink_config: Option<&UltrathinkConfig>,
) -> EffortResult {
    let keyword_detected = detector::detect_ultrathink(message);
    let ultrathink = ultrathink_config.cloned().unwrap_or_default();

    // Priority 1: "ultrathink" keyword
    if keyword_detected {
        return EffortResult {
            effort: ultrathink.effort,
            budget_tokens: ultrathink.budget_tokens,
            keyword_detected: true,
            ultrathink_active: true,
            effort_explicit: true,
        };
    }

    // Priority 2: Session toggle
    if thinking_state.ultrathink_enabled {
        return EffortResult {
            effort: ultrathink.effort,
            budget_tokens: ultrathink.budget_tokens,
            keyword_detected: false,
            ultrathink_active: true,
            effort_explicit: true,
        };
    }

    // Priority 3: Per-turn effort override
    if let Some(effort) = per_turn_effort {
        return EffortResult {
            effort,
            budget_tokens: 0,
            keyword_detected: false,
            ultrathink_active: false,
            effort_explicit: true,
        };
    }

    // Priority 4: Global config
    if let Some(effort) = config_effort {
        return EffortResult {
            effort,
            budget_tokens: 0,
            keyword_detected: false,
            ultrathink_active: false,
            effort_explicit: true,
        };
    }

    // Priority 5: Model info default (if defined)
    if let Some(effort) = model_info.default_reasoning_level {
        return EffortResult {
            effort,
            budget_tokens: 0,
            keyword_detected: false,
            ultrathink_active: false,
            effort_explicit: true,
        };
    }

    // Fallback: No explicit effort configured - use Medium but mark as not explicit
    // so the request won't include the effort field
    EffortResult {
        effort: ReasoningEffort::Medium,
        budget_tokens: 0,
        keyword_detected: false,
        ultrathink_active: false,
        effort_explicit: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models_manager::model_info::find_model_info_for_slug;

    fn test_model_info() -> ModelInfo {
        find_model_info_for_slug("gpt-5.2-codex")
    }

    #[test]
    fn test_compute_effective_effort_default_fallback() {
        let state = ThinkingState::default();
        // gpt-5.2-codex does NOT have default_reasoning_level set
        let model_info = test_model_info();
        let result = compute_effective_effort("hello world", &state, None, None, &model_info, None);
        // Falls back to Medium but effort_explicit is false
        assert_eq!(result.effort, ReasoningEffort::Medium);
        assert!(!result.keyword_detected);
        assert!(!result.ultrathink_active);
        assert!(!result.effort_explicit); // No explicit default, fallback used
    }

    #[test]
    fn test_compute_effective_effort_with_model_default() {
        let state = ThinkingState::default();
        // gpt-5.1 HAS default_reasoning_level = Some(Medium)
        let model_info = find_model_info_for_slug("gpt-5.1");
        let result = compute_effective_effort("hello world", &state, None, None, &model_info, None);
        // Model info has explicit default, so effort_explicit is true
        assert_eq!(result.effort, ReasoningEffort::Medium);
        assert!(!result.keyword_detected);
        assert!(!result.ultrathink_active);
        assert!(result.effort_explicit); // Explicit model default
    }

    #[test]
    fn test_compute_effective_effort_no_default() {
        let state = ThinkingState::default();
        // gpt-5.1-codex does NOT have default_reasoning_level
        let model_info = find_model_info_for_slug("gpt-5.1-codex");
        let result = compute_effective_effort("hello world", &state, None, None, &model_info, None);
        // Falls back to Medium but effort_explicit is false
        assert_eq!(result.effort, ReasoningEffort::Medium);
        assert!(!result.keyword_detected);
        assert!(!result.ultrathink_active);
        assert!(!result.effort_explicit); // No explicit default, should be false
    }

    #[test]
    fn test_compute_effective_effort_keyword() {
        let state = ThinkingState::default();
        let model_info = test_model_info();
        let result = compute_effective_effort(
            "ultrathink about this problem",
            &state,
            None,
            None,
            &model_info,
            None,
        );
        assert_eq!(result.effort, ReasoningEffort::XHigh);
        assert!(result.keyword_detected);
        assert!(result.ultrathink_active);
        assert_eq!(result.budget_tokens, 31999);
        assert!(result.effort_explicit);
    }

    #[test]
    fn test_compute_effective_effort_toggle() {
        let mut state = ThinkingState::default();
        state.toggle();
        let model_info = test_model_info();
        let result = compute_effective_effort("hello world", &state, None, None, &model_info, None);
        assert_eq!(result.effort, ReasoningEffort::XHigh);
        assert!(!result.keyword_detected);
        assert!(result.ultrathink_active);
        assert!(result.effort_explicit);
    }

    #[test]
    fn test_compute_effective_effort_per_turn_override() {
        let state = ThinkingState::default();
        let model_info = test_model_info();
        let result = compute_effective_effort(
            "hello",
            &state,
            Some(ReasoningEffort::High),
            None,
            &model_info,
            None,
        );
        assert_eq!(result.effort, ReasoningEffort::High);
        assert!(!result.ultrathink_active);
        assert!(result.effort_explicit);
    }

    #[test]
    fn test_compute_effective_effort_config_override() {
        let state = ThinkingState::default();
        let model_info = test_model_info();
        let result = compute_effective_effort(
            "hello",
            &state,
            None,
            Some(ReasoningEffort::Low),
            &model_info,
            None,
        );
        assert_eq!(result.effort, ReasoningEffort::Low);
        assert!(result.effort_explicit);
    }

    #[test]
    fn test_compute_effective_effort_custom_ultrathink_config() {
        let state = ThinkingState::default();
        let model_info = test_model_info();
        let custom_config = UltrathinkConfig {
            effort: ReasoningEffort::High,
            budget_tokens: 16000,
        };
        let result = compute_effective_effort(
            "ultrathink",
            &state,
            None,
            None,
            &model_info,
            Some(&custom_config),
        );
        assert_eq!(result.effort, ReasoningEffort::High);
        assert_eq!(result.budget_tokens, 16000);
        assert!(result.ultrathink_active);
        assert!(result.effort_explicit);
    }

    #[test]
    fn test_keyword_beats_toggle() {
        // Both keyword and toggle active - keyword wins (same result)
        let mut state = ThinkingState::default();
        state.toggle();
        let model_info = test_model_info();
        let result =
            compute_effective_effort("ultrathink please", &state, None, None, &model_info, None);
        assert!(result.keyword_detected);
        assert!(result.ultrathink_active);
        assert_eq!(result.effort, ReasoningEffort::XHigh);
        assert!(result.effort_explicit);
    }
}
