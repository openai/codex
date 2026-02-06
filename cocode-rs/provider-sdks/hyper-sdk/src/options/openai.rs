//! OpenAI-specific options.

use super::ProviderMarker;
use super::ProviderOptionsData;
use super::TypedProviderOptions;
use serde::Deserialize;
use serde::Serialize;
use std::any::Any;
use std::collections::HashMap;

/// Reasoning effort level for OpenAI o1/o3 models.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    /// Low effort (faster, less thorough).
    Low,
    /// Medium effort (balanced).
    #[default]
    Medium,
    /// High effort (slower, more thorough).
    High,
}

/// Reasoning summary level for OpenAI o1/o3 models.
///
/// See <https://platform.openai.com/docs/guides/reasoning#reasoning-summaries>
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningSummary {
    /// No reasoning summary.
    None,
    /// Auto (provider decides).
    #[default]
    Auto,
    /// Concise summary.
    Concise,
    /// Detailed summary.
    Detailed,
}

/// OpenAI-specific options.
#[derive(Debug, Clone, Default)]
pub struct OpenAIOptions {
    /// Reasoning effort for o1/o3 models.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Reasoning summary level for o1/o3 models.
    pub reasoning_summary: Option<ReasoningSummary>,
    /// Include encrypted reasoning content in response.
    pub include_encrypted_content: Option<bool>,
    /// Previous response ID for conversation continuity.
    pub previous_response_id: Option<String>,
    /// Response format (e.g., "json_object" for JSON mode).
    pub response_format: Option<String>,
    /// Seed for deterministic sampling.
    pub seed: Option<i64>,
    /// Arbitrary extra parameters passed through to the API request body.
    #[doc(hidden)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl OpenAIOptions {
    /// Create new OpenAI options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set reasoning effort.
    pub fn with_reasoning_effort(mut self, effort: ReasoningEffort) -> Self {
        self.reasoning_effort = Some(effort);
        self
    }

    /// Set reasoning summary level.
    pub fn with_reasoning_summary(mut self, summary: ReasoningSummary) -> Self {
        self.reasoning_summary = Some(summary);
        self
    }

    /// Set whether to include encrypted content in response.
    pub fn with_include_encrypted_content(mut self, include: bool) -> Self {
        self.include_encrypted_content = Some(include);
        self
    }

    /// Set previous response ID for conversation continuity.
    pub fn with_previous_response_id(mut self, id: impl Into<String>) -> Self {
        self.previous_response_id = Some(id.into());
        self
    }

    /// Set response format.
    pub fn with_response_format(mut self, format: impl Into<String>) -> Self {
        self.response_format = Some(format.into());
        self
    }

    /// Set seed for deterministic sampling.
    pub fn with_seed(mut self, seed: i64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Convert to boxed ProviderOptions.
    pub fn boxed(self) -> Box<dyn ProviderOptionsData> {
        Box::new(self)
    }
}

impl ProviderMarker for OpenAIOptions {
    const PROVIDER_NAME: &'static str = "openai";
}

impl TypedProviderOptions for OpenAIOptions {}

impl ProviderOptionsData for OpenAIOptions {
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
    fn test_openai_options() {
        let opts = OpenAIOptions::new()
            .with_reasoning_effort(ReasoningEffort::High)
            .with_previous_response_id("resp_123");

        assert_eq!(opts.reasoning_effort, Some(ReasoningEffort::High));
        assert_eq!(opts.previous_response_id, Some("resp_123".to_string()));
    }

    #[test]
    fn test_downcast() {
        let opts: Box<dyn ProviderOptionsData> = OpenAIOptions::new()
            .with_reasoning_effort(ReasoningEffort::Low)
            .boxed();

        let openai_opts = downcast_options::<OpenAIOptions>(&opts);
        assert!(openai_opts.is_some());
        assert_eq!(
            openai_opts.unwrap().reasoning_effort,
            Some(ReasoningEffort::Low)
        );
    }

    #[test]
    fn test_reasoning_summary() {
        let opts = OpenAIOptions::new()
            .with_reasoning_summary(ReasoningSummary::Detailed)
            .with_include_encrypted_content(true);

        assert_eq!(opts.reasoning_summary, Some(ReasoningSummary::Detailed));
        assert_eq!(opts.include_encrypted_content, Some(true));
    }

    #[test]
    fn test_reasoning_summary_serde() {
        let summary = ReasoningSummary::Concise;
        let json = serde_json::to_string(&summary).unwrap();
        assert_eq!(json, "\"concise\"");

        let parsed: ReasoningSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ReasoningSummary::Concise);
    }
}
