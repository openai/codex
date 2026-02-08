//! Model information configuration.

use super::Capability;
use super::ConfigShellToolType;
use super::ReasoningSummary;
use crate::thinking::ThinkingLevel;
use crate::tool_config::ApplyPatchToolType;
use serde::Deserialize;
use serde::Serialize;
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

    // === Thinking/Reasoning (Unified) ===
    /// Default thinking level for this model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_thinking_level: Option<ThinkingLevel>,

    /// Supported thinking levels (ordered from low to high).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_thinking_levels: Option<Vec<ThinkingLevel>>,

    /// Whether to include thoughts in response (Gemini thinking display).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_thoughts: Option<bool>,

    /// Reasoning summary level for OpenAI o1/o3 models.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_summary: Option<ReasoningSummary>,

    // === Context Management ===
    /// Effective context window as percentage (0-100).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_context_window_percent: Option<i32>,

    // === Tool Related ===
    /// Shell execution type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_type: Option<ConfigShellToolType>,

    /// Model-level cap on tool output size (characters).
    /// When set, overrides per-tool max_result_size_chars() if this value is smaller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_output_chars: Option<i32>,

    /// Experimental supported tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental_supported_tools: Option<Vec<String>>,

    /// Apply patch tool type for this model.
    ///
    /// - `None`: No apply_patch tool (default for Claude, Gemini, non-OpenAI)
    /// - `Some(Function)`: JSON function tool with "input" parameter (for gpt-oss)
    /// - `Some(Freeform)`: String-schema function tool (for GPT-5.2+, codex models)
    /// - `Some(Shell)`: Shell-based, prompt instructions only (for GPT-5, o3, o4-mini)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_patch_tool_type: Option<ApplyPatchToolType>,

    // === Instructions ===
    /// Base instructions for this model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,

    /// Path to base instructions file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_instructions_file: Option<String>,

    // === Provider-Specific Extensions ===
    /// Options for provider SDK passthrough.
    ///
    /// These are merged across configuration layers and passed to provider SDKs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<HashMap<String, serde_json::Value>>,
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
        // Thinking/Reasoning (unified)
        merge_field!(default_thinking_level);
        merge_field!(supported_thinking_levels);
        merge_field!(include_thoughts);
        merge_field!(reasoning_summary);
        // Context management
        merge_field!(effective_context_window_percent);
        // Tool related
        merge_field!(shell_type);
        merge_field!(max_tool_output_chars);
        merge_field!(experimental_supported_tools);
        merge_field!(apply_patch_tool_type);
        // Instructions
        merge_field!(base_instructions);
        merge_field!(base_instructions_file);
        // Request options: merge maps, other takes precedence for overlapping keys
        if let Some(other_opts) = &other.options {
            let self_opts = self.options.get_or_insert_with(HashMap::new);
            for (key, value) in other_opts {
                self_opts.insert(key.clone(), value.clone());
            }
        }
    }

    /// Get display name or fall back to slug.
    pub fn display_name_or_slug(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.slug)
    }

    /// Get timeout in seconds or default (600).
    pub fn timeout_secs_or_default(&self) -> i64 {
        self.timeout_secs.unwrap_or(600)
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

    /// Set the default thinking level.
    pub fn with_thinking_level(mut self, level: ThinkingLevel) -> Self {
        self.default_thinking_level = Some(level);
        self
    }

    /// Set the supported thinking levels.
    pub fn with_supported_thinking_levels(mut self, levels: Vec<ThinkingLevel>) -> Self {
        self.supported_thinking_levels = Some(levels);
        self
    }

    /// Set request options for provider SDK passthrough.
    pub fn with_request_options(mut self, opts: HashMap<String, serde_json::Value>) -> Self {
        self.options = Some(opts);
        self
    }

    /// Set the apply_patch tool type.
    pub fn with_apply_patch_tool_type(mut self, tool_type: ApplyPatchToolType) -> Self {
        self.apply_patch_tool_type = Some(tool_type);
        self
    }

    /// Get a request option value by key.
    pub fn get_request_option(&self, key: &str) -> Option<&serde_json::Value> {
        self.options.as_ref().and_then(|e| e.get(key))
    }

    /// Find nearest supported thinking level to target.
    ///
    /// Compares by effort level and returns the closest match.
    pub fn nearest_supported_level(&self, target: &ThinkingLevel) -> Option<ThinkingLevel> {
        self.supported_thinking_levels.as_ref().and_then(|levels| {
            levels
                .iter()
                .min_by_key(|l| (l.effort as i32 - target.effort as i32).abs())
                .cloned()
        })
    }

    /// Resolve requested thinking level against supported levels.
    ///
    /// If the exact effort level is supported, returns a matching level.
    /// Otherwise, returns the nearest supported level.
    pub fn resolve_thinking_level(&self, requested: &ThinkingLevel) -> ThinkingLevel {
        match &self.supported_thinking_levels {
            Some(levels) if !levels.is_empty() => levels
                .iter()
                .find(|l| l.effort == requested.effort)
                .cloned()
                .unwrap_or_else(|| self.nearest_supported_level(requested).unwrap()),
            _ => requested.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ReasoningEffort;

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
            default_thinking_level: Some(ThinkingLevel::high()),
            temperature: Some(0.9),
            timeout_secs: Some(300),
            ..Default::default()
        };

        base.merge_from(&other);

        assert_eq!(base.display_name, Some("Base Model".to_string())); // Not overridden
        assert_eq!(base.context_window, Some(8192)); // Overridden
        assert_eq!(base.max_output_tokens, Some(1024)); // Not overridden
        assert_eq!(base.default_thinking_level, Some(ThinkingLevel::high())); // New value
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
            .with_capabilities(vec![Capability::TextGeneration, Capability::Streaming])
            .with_thinking_level(ThinkingLevel::medium());

        assert_eq!(config.display_name, Some("Test Model".to_string()));
        assert_eq!(config.context_window, Some(128000));
        assert_eq!(config.temperature, Some(0.5));
        assert_eq!(config.timeout_secs, Some(120));
        assert!(config.has_capability(Capability::Streaming));
        assert_eq!(config.default_thinking_level, Some(ThinkingLevel::medium()));
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
    fn test_nearest_supported_level() {
        let config = ModelInfo {
            supported_thinking_levels: Some(vec![
                ThinkingLevel::low(),
                ThinkingLevel::medium(),
                ThinkingLevel::high(),
            ]),
            ..Default::default()
        };

        // Exact match
        let result = config
            .nearest_supported_level(&ThinkingLevel::medium())
            .unwrap();
        assert_eq!(result.effort, ReasoningEffort::Medium);

        // None -> Low (nearest)
        let result = config
            .nearest_supported_level(&ThinkingLevel::none())
            .unwrap();
        assert_eq!(result.effort, ReasoningEffort::Low);

        // XHigh -> High (nearest)
        let result = config
            .nearest_supported_level(&ThinkingLevel::xhigh())
            .unwrap();
        assert_eq!(result.effort, ReasoningEffort::High);
    }

    #[test]
    fn test_resolve_thinking_level() {
        let config = ModelInfo {
            supported_thinking_levels: Some(vec![
                ThinkingLevel::low(),
                ThinkingLevel::medium(),
                ThinkingLevel::high(),
            ]),
            ..Default::default()
        };

        // Exact match
        let result = config.resolve_thinking_level(&ThinkingLevel::medium());
        assert_eq!(result.effort, ReasoningEffort::Medium);

        // XHigh -> High (nearest)
        let result = config.resolve_thinking_level(&ThinkingLevel::xhigh());
        assert_eq!(result.effort, ReasoningEffort::High);
    }

    #[test]
    fn test_resolve_thinking_level_no_supported() {
        let config = ModelInfo::default();

        // When no supported levels, return requested as-is
        let requested = ThinkingLevel::high();
        let result = config.resolve_thinking_level(&requested);
        assert_eq!(result, requested);
    }

    #[test]
    fn test_merge_reasoning_summary() {
        use super::ReasoningSummary;

        let mut base = ModelInfo {
            reasoning_summary: Some(ReasoningSummary::Auto),
            ..Default::default()
        };

        let other = ModelInfo {
            reasoning_summary: Some(ReasoningSummary::Concise),
            ..Default::default()
        };

        base.merge_from(&other);

        assert_eq!(base.reasoning_summary, Some(ReasoningSummary::Concise));
    }

    #[test]
    fn test_request_options_field_serde() {
        let mut opts = HashMap::new();
        opts.insert(
            "response_format".to_string(),
            serde_json::json!({"type": "json_object"}),
        );
        opts.insert("seed".to_string(), serde_json::json!(42));

        let config = ModelInfo {
            slug: "test-model".to_string(),
            options: Some(opts),
            ..Default::default()
        };

        let json = serde_json::to_string(&config).expect("serialize");
        let parsed: ModelInfo = serde_json::from_str(&json).expect("deserialize");

        assert!(parsed.options.is_some());
        let parsed_opts = parsed.options.unwrap();
        assert_eq!(parsed_opts.get("seed"), Some(&serde_json::json!(42)));
        assert_eq!(
            parsed_opts.get("response_format"),
            Some(&serde_json::json!({"type": "json_object"}))
        );
    }

    #[test]
    fn test_merge_from_request_options_maps() {
        let mut base_opts = HashMap::new();
        base_opts.insert("key1".to_string(), serde_json::json!("value1"));
        base_opts.insert("key2".to_string(), serde_json::json!("base_value"));

        let mut other_opts = HashMap::new();
        other_opts.insert("key2".to_string(), serde_json::json!("other_value")); // Override
        other_opts.insert("key3".to_string(), serde_json::json!("value3")); // New key

        let mut base = ModelInfo {
            options: Some(base_opts),
            ..Default::default()
        };

        let other = ModelInfo {
            options: Some(other_opts),
            ..Default::default()
        };

        base.merge_from(&other);

        let merged = base.options.unwrap();
        assert_eq!(merged.get("key1"), Some(&serde_json::json!("value1"))); // Preserved
        assert_eq!(merged.get("key2"), Some(&serde_json::json!("other_value"))); // Overridden
        assert_eq!(merged.get("key3"), Some(&serde_json::json!("value3"))); // Added
    }

    #[test]
    fn test_merge_from_request_options_none_to_some() {
        let mut base = ModelInfo::default();
        assert!(base.options.is_none());

        let mut other_opts = HashMap::new();
        other_opts.insert("new_key".to_string(), serde_json::json!("new_value"));

        let other = ModelInfo {
            options: Some(other_opts),
            ..Default::default()
        };

        base.merge_from(&other);

        assert!(base.options.is_some());
        let merged = base.options.unwrap();
        assert_eq!(merged.get("new_key"), Some(&serde_json::json!("new_value")));
    }

    #[test]
    fn test_get_request_option_helper() {
        let mut opts = HashMap::new();
        opts.insert("key".to_string(), serde_json::json!("value"));

        let config = ModelInfo {
            options: Some(opts),
            ..Default::default()
        };

        assert_eq!(
            config.get_request_option("key"),
            Some(&serde_json::json!("value"))
        );
        assert_eq!(config.get_request_option("nonexistent"), None);

        // None request_options
        let empty_config = ModelInfo::default();
        assert_eq!(empty_config.get_request_option("key"), None);
    }

    #[test]
    fn test_with_request_options_builder() {
        let mut opts = HashMap::new();
        opts.insert(
            "response_format".to_string(),
            serde_json::json!({"type": "json_object"}),
        );

        let config = ModelInfo::new()
            .with_display_name("Test")
            .with_request_options(opts.clone());

        assert_eq!(config.options, Some(opts));
    }

    #[test]
    fn test_merge_max_tool_output_chars() {
        let mut base = ModelInfo {
            max_tool_output_chars: Some(50_000),
            ..Default::default()
        };

        let other = ModelInfo {
            max_tool_output_chars: Some(20_000),
            ..Default::default()
        };

        base.merge_from(&other);
        assert_eq!(base.max_tool_output_chars, Some(20_000));
    }

    #[test]
    fn test_merge_max_tool_output_chars_none_preserves() {
        let mut base = ModelInfo {
            max_tool_output_chars: Some(50_000),
            ..Default::default()
        };

        let other = ModelInfo::default(); // max_tool_output_chars is None

        base.merge_from(&other);
        assert_eq!(base.max_tool_output_chars, Some(50_000)); // Preserved
    }

    #[test]
    fn test_max_tool_output_chars_serde() {
        let config = ModelInfo {
            slug: "test-model".to_string(),
            max_tool_output_chars: Some(30_000),
            ..Default::default()
        };

        let json = serde_json::to_string(&config).expect("serialize");
        assert!(json.contains("max_tool_output_chars"));
        let parsed: ModelInfo = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.max_tool_output_chars, Some(30_000));
    }
}
