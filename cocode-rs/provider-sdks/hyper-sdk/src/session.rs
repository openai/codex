//! Session configuration for per-session defaults.
//!
//! `SessionConfig` provides session-level defaults that are merged into requests.
//! This allows setting common parameters once instead of on every request.
//!
//! # Parameter Priority
//!
//! Parameters are merged with the following priority (highest to lowest):
//! 1. Per-request values (highest priority)
//! 2. Session config values
//! 3. Provider defaults (lowest priority)
//!
//! # Example
//!
//! ```no_run
//! use hyper_sdk::session::SessionConfig;
//! use hyper_sdk::{GenerateRequest, Message, ToolDefinition};
//!
//! // Configure session defaults
//! let config = SessionConfig::new()
//!     .temperature(0.7)
//!     .max_tokens(4096);
//!
//! // Create a request (temperature defaults to 0.7)
//! let mut request = GenerateRequest::new(vec![Message::user("Hello")]);
//!
//! // Merge session config
//! config.merge_into(&mut request);
//!
//! assert_eq!(request.temperature, Some(0.7));
//! assert_eq!(request.max_tokens, Some(4096));
//! ```

use crate::options::ProviderOptions;
use crate::options::ThinkingConfig;
use crate::request::GenerateRequest;
use crate::tools::ToolChoice;
use crate::tools::ToolDefinition;

/// Session-level configuration merged into requests.
///
/// Session config provides default values for request parameters.
/// Request-specific values always take precedence over session defaults.
#[derive(Debug, Clone, Default)]
pub struct SessionConfig {
    /// Default sampling temperature.
    pub temperature: Option<f64>,
    /// Default maximum tokens to generate.
    pub max_tokens: Option<i32>,
    /// Default top-p nucleus sampling.
    pub top_p: Option<f64>,
    /// Default tools available in the session.
    pub tools: Option<Vec<ToolDefinition>>,
    /// Default tool choice behavior.
    pub tool_choice: Option<ToolChoice>,
    /// Default thinking configuration.
    pub thinking_config: Option<ThinkingConfig>,
    /// Default provider-specific options.
    pub provider_options: Option<ProviderOptions>,
}

impl SessionConfig {
    /// Create a new empty session config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the default temperature.
    pub fn temperature(mut self, t: f64) -> Self {
        self.temperature = Some(t);
        self
    }

    /// Set the default maximum tokens.
    pub fn max_tokens(mut self, n: i32) -> Self {
        self.max_tokens = Some(n);
        self
    }

    /// Set the default top-p.
    pub fn top_p(mut self, p: f64) -> Self {
        self.top_p = Some(p);
        self
    }

    /// Set the default tools.
    pub fn tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set the default tool choice.
    pub fn tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = Some(choice);
        self
    }

    /// Set the default thinking configuration.
    pub fn thinking_config(mut self, config: ThinkingConfig) -> Self {
        self.thinking_config = Some(config);
        self
    }

    /// Enable thinking with a token budget.
    pub fn with_thinking(mut self, budget_tokens: i32) -> Self {
        self.thinking_config = Some(ThinkingConfig::with_budget(budget_tokens));
        self
    }

    /// Set provider-specific options.
    pub fn provider_options(mut self, options: ProviderOptions) -> Self {
        self.provider_options = Some(options);
        self
    }

    /// Merge session config into a request.
    ///
    /// Request values take precedence over session defaults.
    /// Only unset fields in the request are filled from the session config.
    pub fn merge_into(&self, request: &mut GenerateRequest) {
        if request.temperature.is_none() {
            request.temperature = self.temperature;
        }

        if request.max_tokens.is_none() {
            request.max_tokens = self.max_tokens;
        }

        if request.top_p.is_none() {
            request.top_p = self.top_p;
        }

        if request.tools.is_none() {
            request.tools = self.tools.clone();
        }

        if request.tool_choice.is_none() {
            request.tool_choice = self.tool_choice.clone();
        }

        if request.thinking_config.is_none() {
            request.thinking_config = self.thinking_config.clone();
        }

        // Note: provider_options are not merged by default as they are type-erased
        // and may require special handling per provider
    }

    /// Check if any config values are set.
    pub fn is_empty(&self) -> bool {
        self.temperature.is_none()
            && self.max_tokens.is_none()
            && self.top_p.is_none()
            && self.tools.is_none()
            && self.tool_choice.is_none()
            && self.thinking_config.is_none()
            && self.provider_options.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::Message;

    #[test]
    fn test_session_config_builder() {
        let config = SessionConfig::new()
            .temperature(0.7)
            .max_tokens(4096)
            .top_p(0.9);

        assert_eq!(config.temperature, Some(0.7));
        assert_eq!(config.max_tokens, Some(4096));
        assert_eq!(config.top_p, Some(0.9));
    }

    #[test]
    fn test_merge_fills_empty_fields() {
        let config = SessionConfig::new().temperature(0.7).max_tokens(4096);

        let mut request = GenerateRequest::new(vec![Message::user("Hello")]);
        assert!(request.temperature.is_none());
        assert!(request.max_tokens.is_none());

        config.merge_into(&mut request);

        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.max_tokens, Some(4096));
    }

    #[test]
    fn test_merge_preserves_request_values() {
        let config = SessionConfig::new().temperature(0.7).max_tokens(4096);

        let mut request = GenerateRequest::new(vec![Message::user("Hello")])
            .temperature(0.3)
            .max_tokens(1000);

        config.merge_into(&mut request);

        // Request values should be preserved
        assert_eq!(request.temperature, Some(0.3));
        assert_eq!(request.max_tokens, Some(1000));
    }

    #[test]
    fn test_merge_partial() {
        let config = SessionConfig::new().temperature(0.7).max_tokens(4096);

        let mut request = GenerateRequest::new(vec![Message::user("Hello")]).temperature(0.3);

        config.merge_into(&mut request);

        // Temperature from request, max_tokens from config
        assert_eq!(request.temperature, Some(0.3));
        assert_eq!(request.max_tokens, Some(4096));
    }

    #[test]
    fn test_with_thinking() {
        let config = SessionConfig::new().with_thinking(10000);

        assert!(config.thinking_config.is_some());
        assert_eq!(
            config.thinking_config.as_ref().unwrap().budget_tokens,
            Some(10000)
        );
    }

    #[test]
    fn test_with_tools() {
        let tools = vec![ToolDefinition::new(
            "test",
            serde_json::json!({"type": "object"}),
        )];

        let config = SessionConfig::new().tools(tools.clone());

        let mut request = GenerateRequest::new(vec![Message::user("Hello")]);
        config.merge_into(&mut request);

        assert!(request.tools.is_some());
        assert_eq!(request.tools.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_is_empty() {
        let empty = SessionConfig::new();
        assert!(empty.is_empty());

        let with_temp = SessionConfig::new().temperature(0.5);
        assert!(!with_temp.is_empty());
    }
}
