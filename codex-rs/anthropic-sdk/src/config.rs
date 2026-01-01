use std::time::Duration;

/// Configuration for the Anthropic client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// API key for authentication.
    pub api_key: String,

    /// Base URL for the API (default: https://api.anthropic.com).
    pub base_url: String,

    /// Request timeout.
    pub timeout: Duration,

    /// Maximum number of retries for failed requests.
    pub max_retries: u32,
}

impl ClientConfig {
    /// Default base URL for the Anthropic API.
    pub const DEFAULT_BASE_URL: &'static str = "https://api.anthropic.com";

    /// Default timeout (10 minutes for long-running requests).
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);

    /// Default max retries.
    pub const DEFAULT_MAX_RETRIES: u32 = 2;

    /// Create a new client configuration.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: Self::DEFAULT_BASE_URL.to_string(),
            timeout: Self::DEFAULT_TIMEOUT,
            max_retries: Self::DEFAULT_MAX_RETRIES,
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
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
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
        }
    }
}
