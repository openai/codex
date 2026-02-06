//! Volcengine Ark-specific options.

use super::ProviderMarker;
use super::ProviderOptionsData;
use super::TypedProviderOptions;
use serde::Deserialize;
use serde::Serialize;
use std::any::Any;
use std::collections::HashMap;

/// Reasoning effort level for Volcengine Ark models.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    /// Minimal reasoning effort.
    Minimal,
    /// Low reasoning effort.
    Low,
    /// Medium reasoning effort.
    #[default]
    Medium,
    /// High reasoning effort.
    High,
}

/// Volcengine Ark-specific options.
#[derive(Debug, Clone, Default)]
pub struct VolcengineOptions {
    /// Extended thinking budget tokens (min 1024).
    pub thinking_budget_tokens: Option<i32>,
    /// Previous response ID for conversation continuity.
    pub previous_response_id: Option<String>,
    /// Enable prompt caching.
    pub caching_enabled: Option<bool>,
    /// Reasoning effort level.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Arbitrary extra parameters passed through to the API request body.
    #[doc(hidden)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl VolcengineOptions {
    /// Create new Volcengine options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set thinking budget in tokens.
    pub fn with_thinking_budget(mut self, tokens: i32) -> Self {
        self.thinking_budget_tokens = Some(tokens);
        self
    }

    /// Set previous response ID for conversation continuity.
    pub fn with_previous_response_id(mut self, id: impl Into<String>) -> Self {
        self.previous_response_id = Some(id.into());
        self
    }

    /// Enable or disable prompt caching.
    pub fn with_caching(mut self, enabled: bool) -> Self {
        self.caching_enabled = Some(enabled);
        self
    }

    /// Set reasoning effort level.
    pub fn with_reasoning_effort(mut self, effort: ReasoningEffort) -> Self {
        self.reasoning_effort = Some(effort);
        self
    }

    /// Convert to boxed ProviderOptions.
    pub fn boxed(self) -> Box<dyn ProviderOptionsData> {
        Box::new(self)
    }
}

impl ProviderMarker for VolcengineOptions {
    const PROVIDER_NAME: &'static str = "volcengine";
}

impl TypedProviderOptions for VolcengineOptions {}

impl ProviderOptionsData for VolcengineOptions {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn ProviderOptionsData> {
        Box::new(self.clone())
    }

    fn provider_name(&self) -> Option<&'static str> {
        Some(Self::PROVIDER_NAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::downcast_options;

    #[test]
    fn test_volcengine_options() {
        let opts = VolcengineOptions::new()
            .with_thinking_budget(2048)
            .with_previous_response_id("resp_123")
            .with_caching(true)
            .with_reasoning_effort(ReasoningEffort::High);

        assert_eq!(opts.thinking_budget_tokens, Some(2048));
        assert_eq!(opts.previous_response_id, Some("resp_123".to_string()));
        assert_eq!(opts.caching_enabled, Some(true));
        assert_eq!(opts.reasoning_effort, Some(ReasoningEffort::High));
    }

    #[test]
    fn test_downcast() {
        let opts: Box<dyn ProviderOptionsData> =
            VolcengineOptions::new().with_thinking_budget(4096).boxed();

        let volcengine_opts = downcast_options::<VolcengineOptions>(&opts);
        assert!(volcengine_opts.is_some());
        assert_eq!(volcengine_opts.unwrap().thinking_budget_tokens, Some(4096));
    }
}
