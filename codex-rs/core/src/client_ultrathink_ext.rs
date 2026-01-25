//! Ultrathink extension for client.rs.
//!
//! Provides helper functions for ultrathink keyword detection and effort resolution
//! to minimize changes to client.rs and reduce merge conflicts.

use crate::thinking::EffortResult;
use crate::thinking::ThinkingState;
use crate::thinking::UltrathinkConfig;
use crate::thinking::compute_effective_effort;
use codex_api::common::Reasoning;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort;

/// Result of building reasoning for an API request.
#[derive(Debug, Clone)]
pub struct ReasoningResult {
    /// The Reasoning struct to pass to the API (None if model doesn't support it).
    pub reasoning: Option<Reasoning>,
    /// The computed reasoning effort level.
    pub effort: ReasoningEffort,
    /// Token budget when ultrathink is triggered (for budget-based models).
    pub budget_tokens: i32,
    /// Whether ultrathink mode is active (via keyword or toggle).
    pub ultrathink_active: bool,
}

/// Resolve the effective reasoning parameters for a prompt.
///
/// Checks for "ultrathink" keyword in the last user message and computes
/// the effective effort and budget based on the priority chain:
/// 1. Keyword "ultrathink" → ultrathink_config.effort
/// 2. Session toggle ON → ultrathink_config.effort
/// 3. Per-turn effort → base_effort
/// 4. Global config → config_effort
/// 5. Model info default → model_info.default_reasoning_level
///
/// # Arguments
/// * `input` - The prompt input items to check for user messages
/// * `thinking_state` - Session-level ultrathink toggle state
/// * `base_effort` - The base effort from per-turn (UI slider)
/// * `config_effort` - Global config.model_reasoning_effort
/// * `model_info` - The model info for default effort
/// * `ultrathink_config` - Optional provider-level ultrathink configuration
///
/// # Returns
/// EffortResult containing effort, budget_tokens, and ultrathink status.
pub fn resolve_reasoning(
    input: &[ResponseItem],
    thinking_state: &ThinkingState,
    base_effort: Option<ReasoningEffort>,
    config_effort: Option<ReasoningEffort>,
    model_info: &ModelInfo,
    ultrathink_config: Option<&UltrathinkConfig>,
) -> EffortResult {
    let user_message_text = extract_user_message_text(input);
    compute_effective_effort(
        &user_message_text,
        thinking_state,
        base_effort,
        config_effort,
        model_info,
        ultrathink_config,
    )
}

/// Extract user message text from response input items.
///
/// Looks for the last user message and extracts its text content.
fn extract_user_message_text(input: &[ResponseItem]) -> String {
    for item in input.iter().rev() {
        if let ResponseItem::Message { role, content, .. } = item {
            if role == "user" {
                return content
                    .iter()
                    .filter_map(|c| {
                        if let ContentItem::InputText { text } = c {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("");
            }
        }
    }
    String::new()
}

/// Build reasoning parameters for an API request.
///
/// This function encapsulates all reasoning-related logic to minimize changes
/// in client.rs. It handles:
/// - Ultrathink keyword detection and effort resolution
/// - Building the Reasoning struct for the API
/// - Returning budget_tokens for adapter-specific handling
///
/// # Arguments
/// * `input` - The prompt input items to check for user messages
/// * `per_turn_effort` - Per-turn effort override from UI
/// * `config_effort` - Global config.model_reasoning_effort
/// * `model_info` - The model info for default effort and capability checks
/// * `ultrathink_config` - Optional provider-level ultrathink configuration
/// * `summary_config` - Reasoning summary configuration
///
/// # Returns
/// ReasoningResult containing reasoning struct, budget_tokens, and status.
pub fn build_reasoning_for_request(
    input: &[ResponseItem],
    per_turn_effort: Option<ReasoningEffort>,
    config_effort: Option<ReasoningEffort>,
    model_info: &ModelInfo,
    ultrathink_config: Option<&UltrathinkConfig>,
    summary_config: ReasoningSummaryConfig,
) -> ReasoningResult {
    // TODO: Wire ThinkingState from TUI to core (Phase 2)
    let thinking_state = ThinkingState::default();

    let effort_result = resolve_reasoning(
        input,
        &thinking_state,
        per_turn_effort,
        config_effort,
        model_info,
        ultrathink_config,
    );

    let reasoning = if model_info.supports_reasoning_summaries {
        Some(Reasoning {
            // Only include effort when explicitly configured
            effort: if effort_result.effort_explicit {
                Some(effort_result.effort)
            } else {
                None
            },
            summary: if summary_config == ReasoningSummaryConfig::None {
                None
            } else {
                Some(summary_config)
            },
        })
    } else {
        None
    };

    ReasoningResult {
        reasoning,
        effort: effort_result.effort,
        budget_tokens: effort_result.budget_tokens,
        ultrathink_active: effort_result.ultrathink_active,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models_manager::model_info::find_model_info_for_slug;

    fn test_model_info() -> ModelInfo {
        find_model_info_for_slug("gpt-5.2-codex")
    }

    fn make_user_message(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: text.to_string(),
            }],
            end_turn: None,
        }
    }

    #[test]
    fn test_resolve_effort_no_ultrathink() {
        let input = vec![make_user_message("hello world")];
        let model_info = test_model_info();
        let state = ThinkingState::default();
        let result = resolve_reasoning(&input, &state, None, None, &model_info, None);
        assert_eq!(
            result.effort,
            model_info
                .default_reasoning_level
                .unwrap_or(ReasoningEffort::Medium)
        );
        assert!(!result.ultrathink_active);
    }

    #[test]
    fn test_resolve_effort_with_ultrathink_keyword() {
        let input = vec![make_user_message("ultrathink about this")];
        let model_info = test_model_info();
        let state = ThinkingState::default();
        let result = resolve_reasoning(&input, &state, None, None, &model_info, None);
        assert_eq!(result.effort, ReasoningEffort::XHigh);
        assert!(result.ultrathink_active);
        assert!(result.keyword_detected);
        assert_eq!(result.budget_tokens, 31999); // Default budget
    }

    #[test]
    fn test_resolve_effort_with_base_effort() {
        let input = vec![make_user_message("hello")];
        let model_info = test_model_info();
        let state = ThinkingState::default();
        let result = resolve_reasoning(
            &input,
            &state,
            Some(ReasoningEffort::High),
            None,
            &model_info,
            None,
        );
        assert_eq!(result.effort, ReasoningEffort::High);
        assert!(!result.ultrathink_active);
    }

    #[test]
    fn test_resolve_effort_ultrathink_overrides_base() {
        let input = vec![make_user_message("ultrathink please")];
        let model_info = test_model_info();
        let state = ThinkingState::default();
        // Even with base effort set, ultrathink keyword takes priority
        let result = resolve_reasoning(
            &input,
            &state,
            Some(ReasoningEffort::Low),
            None,
            &model_info,
            None,
        );
        assert_eq!(result.effort, ReasoningEffort::XHigh);
        assert!(result.ultrathink_active);
    }

    #[test]
    fn test_resolve_effort_custom_ultrathink_config() {
        let input = vec![make_user_message("ultrathink")];
        let model_info = test_model_info();
        let state = ThinkingState::default();
        let custom_config = UltrathinkConfig {
            effort: ReasoningEffort::High,
            budget_tokens: 16000,
        };
        let result = resolve_reasoning(
            &input,
            &state,
            None,
            None,
            &model_info,
            Some(&custom_config),
        );
        assert_eq!(result.effort, ReasoningEffort::High);
        assert_eq!(result.budget_tokens, 16000);
        assert!(result.ultrathink_active);
    }

    #[test]
    fn test_resolve_effort_session_toggle() {
        let input = vec![make_user_message("hello world")];
        let model_info = test_model_info();
        let mut state = ThinkingState::default();
        state.toggle(); // Enable ultrathink via toggle
        let result = resolve_reasoning(&input, &state, None, None, &model_info, None);
        assert_eq!(result.effort, ReasoningEffort::XHigh);
        assert!(result.ultrathink_active);
        assert!(!result.keyword_detected); // Toggle, not keyword
    }

    #[test]
    fn test_resolve_effort_config_effort() {
        let input = vec![make_user_message("hello")];
        let model_info = test_model_info();
        let state = ThinkingState::default();
        // Global config effort should be used when no per-turn effort
        let result = resolve_reasoning(
            &input,
            &state,
            None,
            Some(ReasoningEffort::Low),
            &model_info,
            None,
        );
        assert_eq!(result.effort, ReasoningEffort::Low);
        assert!(!result.ultrathink_active);
    }

    #[test]
    fn test_resolve_effort_per_turn_beats_config() {
        let input = vec![make_user_message("hello")];
        let model_info = test_model_info();
        let state = ThinkingState::default();
        // Per-turn effort should override global config
        let result = resolve_reasoning(
            &input,
            &state,
            Some(ReasoningEffort::High),
            Some(ReasoningEffort::Low),
            &model_info,
            None,
        );
        assert_eq!(result.effort, ReasoningEffort::High);
    }

    // Tests for build_reasoning_for_request

    #[test]
    fn test_build_reasoning_basic() {
        let input = vec![make_user_message("hello world")];
        let model_info = test_model_info();
        let result = build_reasoning_for_request(
            &input,
            None,
            None,
            &model_info,
            None,
            ReasoningSummaryConfig::Auto,
        );

        // Model supports reasoning summaries
        assert!(result.reasoning.is_some());
        let reasoning = result.reasoning.unwrap();
        // gpt-5.2-codex does NOT have default_reasoning_level,
        // so effort should be None (not included in request)
        assert_eq!(reasoning.effort, model_info.default_reasoning_level);
        assert_eq!(reasoning.summary, Some(ReasoningSummaryConfig::Auto));
        assert!(!result.ultrathink_active);
    }

    #[test]
    fn test_build_reasoning_with_ultrathink() {
        let input = vec![make_user_message("ultrathink about this")];
        let model_info = test_model_info();
        let result = build_reasoning_for_request(
            &input,
            None,
            None,
            &model_info,
            None,
            ReasoningSummaryConfig::Auto,
        );

        assert!(result.reasoning.is_some());
        let reasoning = result.reasoning.unwrap();
        assert_eq!(reasoning.effort, Some(ReasoningEffort::XHigh));
        assert!(result.ultrathink_active);
        assert_eq!(result.budget_tokens, 31999);
    }

    #[test]
    fn test_build_reasoning_summary_none() {
        let input = vec![make_user_message("hello")];
        let model_info = test_model_info();
        let result = build_reasoning_for_request(
            &input,
            None,
            None,
            &model_info,
            None,
            ReasoningSummaryConfig::None,
        );

        assert!(result.reasoning.is_some());
        let reasoning = result.reasoning.unwrap();
        // Summary should be None when config is None
        assert!(reasoning.summary.is_none());
    }

    #[test]
    fn test_build_reasoning_no_support() {
        let input = vec![make_user_message("hello")];
        // Use a model that doesn't support reasoning summaries
        let mut model_info = test_model_info();
        model_info.supports_reasoning_summaries = false;

        let result = build_reasoning_for_request(
            &input,
            None,
            None,
            &model_info,
            None,
            ReasoningSummaryConfig::Auto,
        );

        // No reasoning struct when model doesn't support it
        assert!(result.reasoning.is_none());
    }

    #[test]
    fn test_build_reasoning_custom_config() {
        let input = vec![make_user_message("ultrathink")];
        let model_info = test_model_info();
        let custom_config = UltrathinkConfig {
            effort: ReasoningEffort::High,
            budget_tokens: 16000,
        };
        let result = build_reasoning_for_request(
            &input,
            None,
            None,
            &model_info,
            Some(&custom_config),
            ReasoningSummaryConfig::Auto,
        );

        assert!(result.reasoning.is_some());
        let reasoning = result.reasoning.unwrap();
        assert_eq!(reasoning.effort, Some(ReasoningEffort::High));
        assert_eq!(result.budget_tokens, 16000);
        assert!(result.ultrathink_active);
    }

    #[test]
    fn test_build_reasoning_no_default_effort() {
        let input = vec![make_user_message("hello world")];
        // gpt-5.1-codex does NOT have default_reasoning_level
        let model_info = find_model_info_for_slug("gpt-5.1-codex");
        let result = build_reasoning_for_request(
            &input,
            None,
            None,
            &model_info,
            None,
            ReasoningSummaryConfig::Auto,
        );

        // Model supports reasoning summaries but has no default effort
        assert!(result.reasoning.is_some());
        let reasoning = result.reasoning.unwrap();
        // effort should be None when model has no default_reasoning_level
        assert!(reasoning.effort.is_none());
        assert_eq!(reasoning.summary, Some(ReasoningSummaryConfig::Auto));
        assert!(!result.ultrathink_active);
    }

    #[test]
    fn test_build_reasoning_with_config_effort() {
        let input = vec![make_user_message("hello world")];
        // gpt-5.1-codex does NOT have default_reasoning_level
        let model_info = find_model_info_for_slug("gpt-5.1-codex");
        let result = build_reasoning_for_request(
            &input,
            None,
            Some(ReasoningEffort::Low), // Explicit config effort
            &model_info,
            None,
            ReasoningSummaryConfig::Auto,
        );

        // With explicit config effort, effort should be included
        assert!(result.reasoning.is_some());
        let reasoning = result.reasoning.unwrap();
        assert_eq!(reasoning.effort, Some(ReasoningEffort::Low));
    }
}
