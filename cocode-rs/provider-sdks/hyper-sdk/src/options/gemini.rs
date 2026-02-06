//! Gemini-specific options.

use super::ProviderMarker;
use super::ProviderOptionsData;
use super::TypedProviderOptions;
use serde::Deserialize;
use serde::Serialize;
use std::any::Any;
use std::collections::HashMap;

/// Thinking level for Gemini models.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ThinkingLevel {
    /// No thinking.
    #[default]
    None,
    /// Low thinking effort.
    Low,
    /// Medium thinking effort.
    Medium,
    /// High thinking effort.
    High,
}

/// Safety setting category for Gemini.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HarmCategory {
    /// Harassment content.
    HarmCategoryHarassment,
    /// Hate speech.
    HarmCategoryHateSpeech,
    /// Sexually explicit content.
    HarmCategorySexuallyExplicit,
    /// Dangerous content.
    HarmCategoryDangerousContent,
}

/// Safety threshold for Gemini.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HarmBlockThreshold {
    /// Block none.
    BlockNone,
    /// Block only high probability.
    BlockOnlyHigh,
    /// Block medium and high probability.
    #[default]
    BlockMediumAndAbove,
    /// Block low, medium, and high probability.
    BlockLowAndAbove,
}

/// Safety setting for Gemini.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetySetting {
    /// The category to configure.
    pub category: HarmCategory,
    /// The threshold for blocking.
    pub threshold: HarmBlockThreshold,
}

/// Gemini-specific options.
#[derive(Debug, Clone, Default)]
pub struct GeminiOptions {
    /// Thinking level for extended reasoning.
    pub thinking_level: Option<ThinkingLevel>,
    /// Whether to include thoughts in the response.
    pub include_thoughts: Option<bool>,
    /// Enable grounding with Google Search.
    pub grounding: Option<bool>,
    /// Safety settings.
    pub safety_settings: Option<Vec<SafetySetting>>,
    /// Stop sequences.
    pub stop_sequences: Option<Vec<String>>,
    /// Arbitrary extra parameters passed through to the API request body.
    #[doc(hidden)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl GeminiOptions {
    /// Create new Gemini options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set thinking level.
    pub fn with_thinking_level(mut self, level: ThinkingLevel) -> Self {
        self.thinking_level = Some(level);
        self
    }

    /// Set whether to include thoughts in the response.
    pub fn with_include_thoughts(mut self, include: bool) -> Self {
        self.include_thoughts = Some(include);
        self
    }

    /// Enable or disable grounding.
    pub fn with_grounding(mut self, enabled: bool) -> Self {
        self.grounding = Some(enabled);
        self
    }

    /// Set safety settings.
    pub fn with_safety_settings(mut self, settings: Vec<SafetySetting>) -> Self {
        self.safety_settings = Some(settings);
        self
    }

    /// Set stop sequences.
    pub fn with_stop_sequences(mut self, sequences: Vec<String>) -> Self {
        self.stop_sequences = Some(sequences);
        self
    }

    /// Convert to boxed ProviderOptions.
    pub fn boxed(self) -> Box<dyn ProviderOptionsData> {
        Box::new(self)
    }
}

impl ProviderMarker for GeminiOptions {
    const PROVIDER_NAME: &'static str = "gemini";
}

impl TypedProviderOptions for GeminiOptions {}

impl ProviderOptionsData for GeminiOptions {
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
    fn test_gemini_options() {
        let opts = GeminiOptions::new()
            .with_thinking_level(ThinkingLevel::High)
            .with_grounding(true);

        assert_eq!(opts.thinking_level, Some(ThinkingLevel::High));
        assert_eq!(opts.grounding, Some(true));
    }

    #[test]
    fn test_downcast() {
        let opts: Box<dyn ProviderOptionsData> = GeminiOptions::new()
            .with_thinking_level(ThinkingLevel::Medium)
            .boxed();

        let gemini_opts = downcast_options::<GeminiOptions>(&opts);
        assert!(gemini_opts.is_some());
        assert_eq!(
            gemini_opts.unwrap().thinking_level,
            Some(ThinkingLevel::Medium)
        );
    }

    #[test]
    fn test_safety_settings() {
        let opts = GeminiOptions::new().with_safety_settings(vec![SafetySetting {
            category: HarmCategory::HarmCategoryHarassment,
            threshold: HarmBlockThreshold::BlockOnlyHigh,
        }]);

        assert!(opts.safety_settings.is_some());
        assert_eq!(opts.safety_settings.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_include_thoughts() {
        let opts = GeminiOptions::new()
            .with_thinking_level(ThinkingLevel::High)
            .with_include_thoughts(true);

        assert_eq!(opts.thinking_level, Some(ThinkingLevel::High));
        assert_eq!(opts.include_thoughts, Some(true));
    }
}
