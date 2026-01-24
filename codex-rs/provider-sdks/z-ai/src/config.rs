//! Client configuration for Z.AI SDK.

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

/// Configuration for the Z.AI / ZhipuAI client.
pub struct ClientConfig {
    /// API key for authentication.
    pub api_key: String,
    /// Base URL for the API.
    pub base_url: String,
    /// Request timeout.
    pub timeout: Duration,
    /// Maximum number of retries for failed requests.
    pub max_retries: i32,
    /// Whether to disable JWT token caching (use raw API key).
    pub disable_token_cache: bool,
    /// Source channel identifier.
    pub source_channel: Option<String>,
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
            .field("disable_token_cache", &self.disable_token_cache)
            .field("source_channel", &self.source_channel)
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
            disable_token_cache: self.disable_token_cache,
            source_channel: self.source_channel.clone(),
            request_hook: self.request_hook.clone(),
        }
    }
}

impl ClientConfig {
    /// Default base URL for Z.AI API.
    pub const ZAI_BASE_URL: &'static str = "https://api.z.ai/api/paas/v4";

    /// Default base URL for ZhipuAI API.
    pub const ZHIPUAI_BASE_URL: &'static str = "https://open.bigmodel.cn/api/paas/v4";

    /// Default timeout (10 minutes).
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);

    /// Default max retries.
    pub const DEFAULT_MAX_RETRIES: i32 = 2;

    /// Create a new configuration for Z.AI client.
    pub fn zai(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: Self::ZAI_BASE_URL.to_string(),
            timeout: Self::DEFAULT_TIMEOUT,
            max_retries: Self::DEFAULT_MAX_RETRIES,
            disable_token_cache: true,
            source_channel: None,
            request_hook: None,
        }
    }

    /// Create a new configuration for ZhipuAI client.
    pub fn zhipuai(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: Self::ZHIPUAI_BASE_URL.to_string(),
            timeout: Self::DEFAULT_TIMEOUT,
            max_retries: Self::DEFAULT_MAX_RETRIES,
            disable_token_cache: true,
            source_channel: None,
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

    /// Set the maximum retries.
    pub fn max_retries(mut self, max_retries: i32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Enable JWT token caching.
    pub fn enable_token_cache(mut self) -> Self {
        self.disable_token_cache = false;
        self
    }

    /// Set source channel.
    pub fn source_channel(mut self, channel: impl Into<String>) -> Self {
        self.source_channel = Some(channel.into());
        self
    }

    /// Set the request hook.
    pub fn request_hook(mut self, hook: Arc<dyn RequestHook>) -> Self {
        self.request_hook = Some(hook);
        self
    }
}
