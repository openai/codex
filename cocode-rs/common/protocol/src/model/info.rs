//! Model information configuration.

use super::Capability;
use super::ConfigShellToolType;
use super::ReasoningEffort;
use super::TruncationPolicyConfig;
use serde::{Deserialize, Serialize};

/// Configurable model info for merging (all fields optional).
///
/// Note: `supports_reasoning_summaries` and `supports_parallel_tool_calls`
/// are expressed via `capabilities` field using `Capability::ReasoningSummaries`
/// and `Capability::ParallelToolCalls`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    // Basic fields
    /// Human-readable display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Model description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Maximum context window in tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<i64>,

    /// Maximum output tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i64>,

    /// Capabilities this model supports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<Capability>>,

    // Context management
    /// Token limit before auto-compaction triggers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_compact_token_limit: Option<i64>,

    /// Effective context window as percentage (0-100).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_context_window_percent: Option<i32>,

    // Reasoning related
    /// Default reasoning effort level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_reasoning_effort: Option<ReasoningEffort>,

    /// Supported reasoning effort levels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_reasoning_levels: Option<Vec<ReasoningEffort>>,

    /// Default thinking budget in tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<i32>,

    // Tool related
    /// Shell execution type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_type: Option<ConfigShellToolType>,

    /// Truncation policy for tool output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation_policy: Option<TruncationPolicyConfig>,

    /// Experimental supported tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental_supported_tools: Option<Vec<String>>,

    // Instruction related
    /// Base instructions for this model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,

    /// Path to base instructions file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_instructions_file: Option<String>,
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
        merge_field!(display_name);
        merge_field!(description);
        merge_field!(context_window);
        merge_field!(max_output_tokens);
        merge_field!(capabilities);
        merge_field!(auto_compact_token_limit);
        merge_field!(effective_context_window_percent);
        merge_field!(default_reasoning_effort);
        merge_field!(supported_reasoning_levels);
        merge_field!(thinking_budget);
        merge_field!(shell_type);
        merge_field!(truncation_policy);
        merge_field!(experimental_supported_tools);
        merge_field!(base_instructions);
        merge_field!(base_instructions_file);
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
            ..Default::default()
        };

        let other = ModelInfo {
            context_window: Some(8192),
            default_reasoning_effort: Some(ReasoningEffort::High),
            ..Default::default()
        };

        base.merge_from(&other);

        assert_eq!(base.display_name, Some("Base Model".to_string())); // Not overridden
        assert_eq!(base.context_window, Some(8192)); // Overridden
        assert_eq!(base.max_output_tokens, Some(1024)); // Not overridden
        assert_eq!(base.default_reasoning_effort, Some(ReasoningEffort::High)); // New value
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
            .with_capabilities(vec![Capability::TextGeneration, Capability::Streaming]);

        assert_eq!(config.display_name, Some("Test Model".to_string()));
        assert_eq!(config.context_window, Some(128000));
        assert!(config.has_capability(Capability::Streaming));
    }

    #[test]
    fn test_serde() {
        let config = ModelInfo {
            display_name: Some("Test".to_string()),
            context_window: Some(4096),
            capabilities: Some(vec![Capability::TextGeneration]),
            ..Default::default()
        };

        let json = serde_json::to_string(&config).expect("serialize");
        let parsed: ModelInfo = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(config, parsed);
    }
}
