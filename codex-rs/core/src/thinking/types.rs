//! Ultrathink configuration types.
//!
//! This module provides configuration for the ultrathink feature, which enables
//! enhanced reasoning via Tab toggle or "ultrathink" keyword detection.

use codex_protocol::openai_models::ReasoningEffort;
use serde::Deserialize;
use serde::Serialize;

/// Default budget tokens for ultrathink mode.
pub const DEFAULT_ULTRATHINK_BUDGET: i32 = 31999;

/// Ultrathink configuration for enhanced reasoning.
///
/// This configuration is activated by:
/// - Tab toggle (session-level)
/// - "ultrathink" keyword in message
///
/// When activated, the reasoning effort is elevated to the configured level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UltrathinkConfig {
    /// Reasoning effort level when ultrathink is triggered.
    /// Default: XHigh
    #[serde(default = "default_effort")]
    pub effort: ReasoningEffort,

    /// Token budget for budget-based models (Claude, Gemini).
    /// Default: 31999
    #[serde(default = "default_budget")]
    pub budget_tokens: i32,
}

fn default_effort() -> ReasoningEffort {
    ReasoningEffort::XHigh
}

fn default_budget() -> i32 {
    DEFAULT_ULTRATHINK_BUDGET
}

impl Default for UltrathinkConfig {
    fn default() -> Self {
        Self {
            effort: default_effort(),
            budget_tokens: default_budget(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ultrathink_config_default() {
        let config = UltrathinkConfig::default();
        assert_eq!(config.effort, ReasoningEffort::XHigh);
        assert_eq!(config.budget_tokens, 31999);
    }

    #[test]
    fn test_ultrathink_config_deserialize() {
        let toml_str = r#"
effort = "high"
budget_tokens = 16000
"#;
        let config: UltrathinkConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.effort, ReasoningEffort::High);
        assert_eq!(config.budget_tokens, 16000);
    }

    #[test]
    fn test_ultrathink_config_deserialize_defaults() {
        let toml_str = "";
        let config: UltrathinkConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.effort, ReasoningEffort::XHigh);
        assert_eq!(config.budget_tokens, 31999);
    }
}
