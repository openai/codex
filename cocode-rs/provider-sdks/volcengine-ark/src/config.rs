//! Client configuration for the Volcengine Ark SDK.

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// Request Hook Support
// ============================================================================

/// HTTP request information that can be modified by a hook.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// Request URL.
    pub url: String,
    /// Request headers as key-value pairs.
    pub headers: HashMap<String, String>,
    /// Request body as JSON.
    pub body: serde_json::Value,
}

/// Trait for request hooks that can modify HTTP requests before they are sent.
pub trait RequestHook: Send + Sync + Debug {
    /// Called before the HTTP request is sent.
    fn on_request(&self, request: &mut HttpRequest);
}

/// Configuration for the Ark API client.
pub struct ClientConfig {
    /// API key for authentication.
    pub api_key: String,

    /// Base URL for the API.
    pub base_url: String,

    /// Request timeout.
    pub timeout: Duration,

    /// Maximum number of retries for failed requests.
    pub max_retries: i32,

    /// Optional request hook for interceptor support.
    pub request_hook: Option<Arc<dyn RequestHook>>,
}

impl Debug for ClientConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientConfig")
            .field("api_key", &"[REDACTED]")
            .field("base_url", &self.base_url)
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .field("request_hook", &self.request_hook.is_some())
            .finish()
    }
}

impl Clone for ClientConfig {
    fn clone(&self) -> Self {
        Self {
            api_key: self.api_key.clone(),
            base_url: self.base_url.clone(),
            timeout: self.timeout,
            max_retries: self.max_retries,
            request_hook: self.request_hook.clone(),
        }
    }
}

impl ClientConfig {
    /// Default base URL for Volcengine Ark API.
    pub const DEFAULT_BASE_URL: &'static str = "https://ark.cn-beijing.volces.com/api/v3";

    /// Default request timeout (10 minutes).
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);

    /// Default maximum retries.
    pub const DEFAULT_MAX_RETRIES: i32 = 2;

    /// Create a new configuration with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: Self::DEFAULT_BASE_URL.to_string(),
            timeout: Self::DEFAULT_TIMEOUT,
            max_retries: Self::DEFAULT_MAX_RETRIES,
            request_hook: None,
        }
    }

    /// Set the base URL.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the maximum number of retries.
    pub fn max_retries(mut self, retries: i32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Set the request hook.
    pub fn request_hook(mut self, hook: Arc<dyn RequestHook>) -> Self {
        self.request_hook = Some(hook);
        self
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: Self::DEFAULT_BASE_URL.to_string(),
            timeout: Self::DEFAULT_TIMEOUT,
            max_retries: Self::DEFAULT_MAX_RETRIES,
            request_hook: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_new() {
        let config = ClientConfig::new("test-key");
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.base_url, ClientConfig::DEFAULT_BASE_URL);
        assert_eq!(config.timeout, ClientConfig::DEFAULT_TIMEOUT);
        assert_eq!(config.max_retries, ClientConfig::DEFAULT_MAX_RETRIES);
    }

    #[test]
    fn test_config_builder() {
        let config = ClientConfig::new("test-key")
            .base_url("https://custom.api.com")
            .timeout(Duration::from_secs(30))
            .max_retries(5);

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.base_url, "https://custom.api.com");
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_retries, 5);
    }
}
