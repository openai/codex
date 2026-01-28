//! Model information configuration.

use super::Capability;
use super::ConfigShellToolType;
use super::ReasoningEffort;
use super::TruncationPolicyConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configurable model info for merging (all fields optional).
///
/// This struct contains all model parameters including sampling settings,
/// reasoning configuration, and tool settings. It supports layered configuration
/// where values can be overridden at different levels (built-in → provider → model).
///
/// Note: `supports_reasoning_summaries` and `supports_parallel_tool_calls`
/// are expressed via `capabilities` field using `Capability::ReasoningSummaries`
/// and `Capability::ParallelToolCalls`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    // === Identity ===
    /// Model identifier (slug).
    pub slug: String,

    /// Human-readable display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Model description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    // === Capacity ===
    /// Maximum context window in tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<i64>,

    /// Maximum output tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i64>,

    /// Request timeout in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<i64>,

    // === Capabilities ===
    /// Capabilities this model supports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<Capability>>,

    // === Sampling Parameters ===
    /// Sampling temperature (0.0 - 2.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p nucleus sampling (0.0 - 1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Frequency penalty (-2.0 - 2.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    /// Presence penalty (-2.0 - 2.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    // === Reasoning/Thinking ===
    /// Default reasoning effort level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_reasoning_effort: Option<ReasoningEffort>,

    /// Supported reasoning effort levels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_reasoning_levels: Option<Vec<ReasoningEffort>>,

    /// Default thinking budget in tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<i32>,

    /// Whether to include thoughts in response (Gemini thinking display).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_thoughts: Option<bool>,

    // === Context Management ===
    /// Token limit before auto-compaction triggers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_compact_token_limit: Option<i64>,

    /// Effective context window as percentage (0-100).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_context_window_percent: Option<i32>,

    // === Tool Related ===
    /// Shell execution type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_type: Option<ConfigShellToolType>,

    /// Truncation policy for tool output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation_policy: Option<TruncationPolicyConfig>,

    /// Experimental supported tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental_supported_tools: Option<Vec<String>>,

    // === Instructions ===
    /// Base instructions for this model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,

    /// Path to base instructions file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_instructions_file: Option<String>,
}

/// Well-known override keys for `apply_overrides()`.
pub mod override_keys {
    pub const TIMEOUT_SECS: &str = "timeout_secs";
    pub const CONTEXT_WINDOW: &str = "context_window";
    pub const MAX_OUTPUT_TOKENS: &str = "max_output_tokens";
    pub const TEMPERATURE: &str = "temperature";
    pub const TOP_P: &str = "top_p";
    pub const FREQUENCY_PENALTY: &str = "frequency_penalty";
    pub const PRESENCE_PENALTY: &str = "presence_penalty";
    pub const THINKING_BUDGET: &str = "thinking_budget";
    pub const REASONING_EFFORT: &str = "reasoning_effort";
    pub const INCLUDE_THOUGHTS: &str = "include_thoughts";
    pub const BASE_INSTRUCTIONS: &str = "base_instructions";
    pub const BASE_INSTRUCTIONS_FILE: &str = "base_instructions_file";
}

impl ModelInfo {
    /// Create a new empty model info config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge another config into this one.
    ///
    /// Values from `other` override values in `self` only if they are Some.
    pub fn merge_from(&mut self, other: &Self) {
        macro_rules! merge_field {
            ($field:ident) => {
                if other.$field.is_some() {
                    self.$field.clone_from(&other.$field);
                }
            };
        }
        // Identity
        merge_field!(display_name);
        merge_field!(description);
        // Capacity
        merge_field!(context_window);
        merge_field!(max_output_tokens);
        merge_field!(timeout_secs);
        // Capabilities
        merge_field!(capabilities);
        // Sampling
        merge_field!(temperature);
        merge_field!(top_p);
        merge_field!(frequency_penalty);
        merge_field!(presence_penalty);
        // Reasoning
        merge_field!(default_reasoning_effort);
        merge_field!(supported_reasoning_levels);
        merge_field!(thinking_budget);
        merge_field!(include_thoughts);
        // Context management
        merge_field!(auto_compact_token_limit);
        merge_field!(effective_context_window_percent);
        // Tool related
        merge_field!(shell_type);
        merge_field!(truncation_policy);
        merge_field!(experimental_supported_tools);
        // Instructions
        merge_field!(base_instructions);
        merge_field!(base_instructions_file);
    }

    /// Apply overrides from a HashMap.
    ///
    /// This allows applying key-value overrides from config files where
    /// the keys are strings. Unknown keys are silently ignored.
    pub fn apply_overrides(&mut self, overrides: &HashMap<String, serde_json::Value>) {
        use override_keys::*;

        for (key, value) in overrides {
            match key.as_str() {
                TIMEOUT_SECS => {
                    if let Some(v) = value.as_i64() {
                        self.timeout_secs = Some(v);
                    }
                }
                CONTEXT_WINDOW => {
                    if let Some(v) = value.as_i64() {
                        self.context_window = Some(v);
                    }
                }
                MAX_OUTPUT_TOKENS => {
                    if let Some(v) = value.as_i64() {
                        self.max_output_tokens = Some(v);
                    }
                }
                TEMPERATURE => {
                    if let Some(v) = value.as_f64() {
                        self.temperature = Some(v as f32);
                    }
                }
                TOP_P => {
                    if let Some(v) = value.as_f64() {
                        self.top_p = Some(v as f32);
                    }
                }
                FREQUENCY_PENALTY => {
                    if let Some(v) = value.as_f64() {
                        self.frequency_penalty = Some(v as f32);
                    }
                }
                PRESENCE_PENALTY => {
                    if let Some(v) = value.as_f64() {
                        self.presence_penalty = Some(v as f32);
                    }
                }
                THINKING_BUDGET => {
                    if let Some(v) = value.as_i64() {
                        self.thinking_budget = Some(v as i32);
                    }
                }
                REASONING_EFFORT => {
                    if let Some(s) = value.as_str() {
                        if let Ok(effort) = serde_json::from_value(serde_json::json!(s)) {
                            self.default_reasoning_effort = Some(effort);
                        }
                    }
                }
                INCLUDE_THOUGHTS => {
                    if let Some(v) = value.as_bool() {
                        self.include_thoughts = Some(v);
                    }
                }
                BASE_INSTRUCTIONS => {
                    if let Some(s) = value.as_str() {
                        self.base_instructions = Some(s.to_string());
                    }
                }
                BASE_INSTRUCTIONS_FILE => {
                    if let Some(s) = value.as_str() {
                        self.base_instructions_file = Some(s.to_string());
                    }
                }
                _ => {
                    // Unknown keys are silently ignored for forward compatibility
                }
            }
        }
    }

    /// Check if model has a specific capability.
    pub fn has_capability(&self, cap: Capability) -> bool {
        self.capabilities
            .as_ref()
            .is_some_and(|caps| caps.contains(&cap))
    }

    // Builder methods

    /// Set the display name.
    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    /// Set the description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the context window size.
    pub fn with_context_window(mut self, tokens: i64) -> Self {
        self.context_window = Some(tokens);
        self
    }

    /// Set the max output tokens.
    pub fn with_max_output_tokens(mut self, tokens: i64) -> Self {
        self.max_output_tokens = Some(tokens);
        self
    }

    /// Set the timeout in seconds.
    pub fn with_timeout_secs(mut self, secs: i64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Set the temperature.
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Set the capabilities.
    pub fn with_capabilities(mut self, caps: Vec<Capability>) -> Self {
        self.capabilities = Some(caps);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_from() {
        let mut base = ModelInfo {
            display_name: Some("Base Model".to_string()),
            context_window: Some(4096),
            max_output_tokens: Some(1024),
            capabilities: Some(vec![Capability::TextGeneration]),
            temperature: Some(0.7),
            ..Default::default()
        };

        let other = ModelInfo {
            context_window: Some(8192),
            default_reasoning_effort: Some(ReasoningEffort::High),
            temperature: Some(0.9),
            timeout_secs: Some(300),
            ..Default::default()
        };

        base.merge_from(&other);

        assert_eq!(base.display_name, Some("Base Model".to_string())); // Not overridden
        assert_eq!(base.context_window, Some(8192)); // Overridden
        assert_eq!(base.max_output_tokens, Some(1024)); // Not overridden
        assert_eq!(base.default_reasoning_effort, Some(ReasoningEffort::High)); // New value
        assert_eq!(base.temperature, Some(0.9)); // Overridden
        assert_eq!(base.timeout_secs, Some(300)); // New value
    }

    #[test]
    fn test_has_capability() {
        let config = ModelInfo {
            capabilities: Some(vec![Capability::TextGeneration, Capability::Vision]),
            ..Default::default()
        };

        assert!(config.has_capability(Capability::TextGeneration));
        assert!(config.has_capability(Capability::Vision));
        assert!(!config.has_capability(Capability::Audio));
    }

    #[test]
    fn test_builder() {
        let config = ModelInfo::new()
            .with_display_name("Test Model")
            .with_context_window(128000)
            .with_temperature(0.5)
            .with_timeout_secs(120)
            .with_capabilities(vec![Capability::TextGeneration, Capability::Streaming]);

        assert_eq!(config.display_name, Some("Test Model".to_string()));
        assert_eq!(config.context_window, Some(128000));
        assert_eq!(config.temperature, Some(0.5));
        assert_eq!(config.timeout_secs, Some(120));
        assert!(config.has_capability(Capability::Streaming));
    }

    #[test]
    fn test_serde() {
        let config = ModelInfo {
            display_name: Some("Test".to_string()),
            context_window: Some(4096),
            capabilities: Some(vec![Capability::TextGeneration]),
            temperature: Some(0.7),
            timeout_secs: Some(300),
            ..Default::default()
        };

        let json = serde_json::to_string(&config).expect("serialize");
        let parsed: ModelInfo = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(config, parsed);
    }

    #[test]
    fn test_apply_overrides() {
        let mut config = ModelInfo {
            display_name: Some("Test Model".to_string()),
            context_window: Some(4096),
            ..Default::default()
        };

        let mut overrides = HashMap::new();
        overrides.insert("timeout_secs".to_string(), serde_json::json!(300));
        overrides.insert("temperature".to_string(), serde_json::json!(0.8));
        overrides.insert("max_output_tokens".to_string(), serde_json::json!(8192));
        overrides.insert("include_thoughts".to_string(), serde_json::json!(true));
        overrides.insert("unknown_key".to_string(), serde_json::json!("ignored"));

        config.apply_overrides(&overrides);

        assert_eq!(config.timeout_secs, Some(300));
        assert_eq!(config.temperature, Some(0.8));
        assert_eq!(config.max_output_tokens, Some(8192));
        assert_eq!(config.include_thoughts, Some(true));
        // Original values preserved
        assert_eq!(config.display_name, Some("Test Model".to_string()));
        assert_eq!(config.context_window, Some(4096));
    }
}
