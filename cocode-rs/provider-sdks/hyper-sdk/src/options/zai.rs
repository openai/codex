//! Z.AI / ZhipuAI-specific options.

use super::ProviderMarker;
use super::ProviderOptionsData;
use super::TypedProviderOptions;
use std::any::Any;
use std::collections::HashMap;

/// Z.AI / ZhipuAI-specific options.
#[derive(Debug, Clone, Default)]
pub struct ZaiOptions {
    /// Extended thinking budget tokens.
    pub thinking_budget_tokens: Option<i32>,
    /// Enable sampling (do_sample).
    pub do_sample: Option<bool>,
    /// Custom request ID.
    pub request_id: Option<String>,
    /// User ID for tracking.
    pub user_id: Option<String>,
    /// Arbitrary extra parameters passed through to the API request body.
    #[doc(hidden)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl ZaiOptions {
    /// Create new Z.AI options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set thinking budget in tokens.
    pub fn with_thinking_budget(mut self, tokens: i32) -> Self {
        self.thinking_budget_tokens = Some(tokens);
        self
    }

    /// Enable or disable sampling.
    pub fn with_do_sample(mut self, enabled: bool) -> Self {
        self.do_sample = Some(enabled);
        self
    }

    /// Set custom request ID.
    pub fn with_request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }

    /// Set user ID for tracking.
    pub fn with_user_id(mut self, id: impl Into<String>) -> Self {
        self.user_id = Some(id.into());
        self
    }

    /// Convert to boxed ProviderOptions.
    pub fn boxed(self) -> Box<dyn ProviderOptionsData> {
        Box::new(self)
    }
}

impl ProviderMarker for ZaiOptions {
    const PROVIDER_NAME: &'static str = "zhipuai";
}

impl TypedProviderOptions for ZaiOptions {}

impl ProviderOptionsData for ZaiOptions {
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
    fn test_zai_options() {
        let opts = ZaiOptions::new()
            .with_thinking_budget(4096)
            .with_do_sample(true)
            .with_request_id("req_123")
            .with_user_id("user_456");

        assert_eq!(opts.thinking_budget_tokens, Some(4096));
        assert_eq!(opts.do_sample, Some(true));
        assert_eq!(opts.request_id, Some("req_123".to_string()));
        assert_eq!(opts.user_id, Some("user_456".to_string()));
    }

    #[test]
    fn test_downcast() {
        let opts: Box<dyn ProviderOptionsData> =
            ZaiOptions::new().with_thinking_budget(8192).boxed();

        let zai_opts = downcast_options::<ZaiOptions>(&opts);
        assert!(zai_opts.is_some());
        assert_eq!(zai_opts.unwrap().thinking_budget_tokens, Some(8192));
    }
}
