//! High-level API client wrapper with retry support.
//!
//! This module provides [`ApiClient`] which wraps hyper-sdk [`Model`]
//! calls with additional features needed for the agent loop:
//! - Retry with exponential backoff
//! - Stall detection
//! - Prompt caching support

use crate::cache::PromptCacheConfig;
use crate::error::{ApiError, Result};
use crate::provider_factory;
use crate::retry::{RetryConfig, RetryContext, RetryDecision};
use crate::unified_stream::UnifiedStream;
use cocode_protocol::ProviderInfo;
use hyper_sdk::{GenerateRequest, GenerateResponse, Model};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, info};

/// Options for a streaming request.
#[derive(Debug, Clone, Default)]
pub struct StreamOptions {
    /// Enable streaming (default: true).
    pub streaming: bool,
    /// Event sender for UI updates.
    pub event_tx: Option<mpsc::Sender<hyper_sdk::StreamUpdate>>,
}

impl StreamOptions {
    /// Create options for streaming.
    pub fn streaming() -> Self {
        Self {
            streaming: true,
            event_tx: None,
        }
    }

    /// Create options for non-streaming.
    pub fn non_streaming() -> Self {
        Self {
            streaming: false,
            event_tx: None,
        }
    }

    /// Set the event sender.
    pub fn with_event_tx(mut self, tx: mpsc::Sender<hyper_sdk::StreamUpdate>) -> Self {
        self.event_tx = Some(tx);
        self
    }
}

/// Configuration for the API client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiClientConfig {
    /// Retry configuration.
    #[serde(default)]
    pub retry: RetryConfig,
    /// Prompt caching configuration.
    #[serde(default)]
    pub cache: PromptCacheConfig,
    /// Stall detection timeout.
    #[serde(default = "default_stall_timeout", with = "humantime_serde")]
    pub stall_timeout: Duration,
    /// Enable stall detection.
    #[serde(default = "default_stall_enabled")]
    pub stall_detection_enabled: bool,
    /// Fallback configuration for stream errors and context overflow.
    #[serde(default)]
    pub fallback: FallbackConfig,
}

fn default_stall_timeout() -> Duration {
    Duration::from_secs(30)
}
fn default_stall_enabled() -> bool {
    true
}
fn default_true() -> bool {
    true
}
fn default_fallback_max_tokens() -> Option<i32> {
    Some(21333)
}
fn default_min_output_tokens() -> i32 {
    3000
}
fn default_max_overflow_attempts() -> i32 {
    3
}

/// Configuration for fallback behavior.
///
/// Controls automatic fallback from streaming to non-streaming on stream errors,
/// and context overflow recovery by reducing max_tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    /// Enable automatic fallback from streaming to non-streaming on stream errors.
    #[serde(default = "default_true")]
    pub enable_stream_fallback: bool,

    /// Maximum tokens for fallback requests (prevents timeout).
    /// Claude Code uses 21333 for this.
    #[serde(default = "default_fallback_max_tokens")]
    pub fallback_max_tokens: Option<i32>,

    /// Enable context overflow recovery (auto-reduce max_tokens).
    #[serde(default = "default_true")]
    pub enable_overflow_recovery: bool,

    /// Minimum output tokens to preserve during overflow recovery.
    #[serde(default = "default_min_output_tokens")]
    pub min_output_tokens: i32,

    /// Maximum overflow recovery attempts.
    #[serde(default = "default_max_overflow_attempts")]
    pub max_overflow_attempts: i32,
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            enable_stream_fallback: true,
            fallback_max_tokens: default_fallback_max_tokens(),
            enable_overflow_recovery: true,
            min_output_tokens: default_min_output_tokens(),
            max_overflow_attempts: default_max_overflow_attempts(),
        }
    }
}

impl FallbackConfig {
    /// Disable all fallback mechanisms.
    pub fn disabled() -> Self {
        Self {
            enable_stream_fallback: false,
            fallback_max_tokens: None,
            enable_overflow_recovery: false,
            min_output_tokens: default_min_output_tokens(),
            max_overflow_attempts: 0,
        }
    }

    /// Builder: set stream fallback enabled.
    pub fn with_stream_fallback(mut self, enabled: bool) -> Self {
        self.enable_stream_fallback = enabled;
        self
    }

    /// Builder: set fallback max tokens.
    pub fn with_fallback_max_tokens(mut self, max_tokens: Option<i32>) -> Self {
        self.fallback_max_tokens = max_tokens;
        self
    }

    /// Builder: set overflow recovery enabled.
    pub fn with_overflow_recovery(mut self, enabled: bool) -> Self {
        self.enable_overflow_recovery = enabled;
        self
    }

    /// Builder: set min output tokens for overflow recovery.
    pub fn with_min_output_tokens(mut self, min_tokens: i32) -> Self {
        self.min_output_tokens = min_tokens;
        self
    }

    /// Builder: set max overflow attempts.
    pub fn with_max_overflow_attempts(mut self, max_attempts: i32) -> Self {
        self.max_overflow_attempts = max_attempts;
        self
    }
}

impl Default for ApiClientConfig {
    fn default() -> Self {
        Self {
            retry: RetryConfig::default(),
            cache: PromptCacheConfig::default(),
            stall_timeout: default_stall_timeout(),
            stall_detection_enabled: default_stall_enabled(),
            fallback: FallbackConfig::default(),
        }
    }
}

impl ApiClientConfig {
    /// Set the retry configuration.
    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }

    /// Set the cache configuration.
    pub fn with_cache(mut self, cache: PromptCacheConfig) -> Self {
        self.cache = cache;
        self
    }

    /// Set the stall timeout.
    pub fn with_stall_timeout(mut self, timeout: Duration) -> Self {
        self.stall_timeout = timeout;
        self
    }

    /// Enable or disable stall detection.
    pub fn with_stall_detection(mut self, enabled: bool) -> Self {
        self.stall_detection_enabled = enabled;
        self
    }

    /// Set the fallback configuration.
    pub fn with_fallback(mut self, fallback: FallbackConfig) -> Self {
        self.fallback = fallback;
        self
    }
}

/// High-level API client with retry and caching.
///
/// The client does not hold a model. Each request receives the model
/// as a parameter, allowing callers to choose the model per-request.
///
/// # Example
///
/// ```ignore
/// use cocode_api::{ApiClient, StreamOptions};
/// use hyper_sdk::{OpenAIProvider, Provider, GenerateRequest, Message};
///
/// let provider = OpenAIProvider::from_env()?;
/// let model = provider.model("gpt-4o")?;
///
/// let client = ApiClient::new();
/// let request = GenerateRequest::new(vec![
///     Message::user("Hello!"),
/// ]);
///
/// let stream = client.stream_request(&*model, request, StreamOptions::streaming()).await?;
/// ```
#[derive(Clone)]
pub struct ApiClient {
    config: ApiClientConfig,
}

impl ApiClient {
    /// Create a new API client with default configuration.
    pub fn new() -> Self {
        Self {
            config: ApiClientConfig::default(),
        }
    }

    /// Create a new API client with custom configuration.
    pub fn with_config(config: ApiClientConfig) -> Self {
        Self { config }
    }

    /// Get the current configuration.
    pub fn config(&self) -> &ApiClientConfig {
        &self.config
    }

    /// Create an ApiClient with a model from ProviderInfo.
    ///
    /// This is a convenience method that creates both the client and model
    /// from a ProviderInfo configuration.
    ///
    /// # Arguments
    ///
    /// * `info` - Provider configuration
    /// * `model_slug` - Model identifier (e.g., "gpt-4o", "claude-sonnet-4")
    /// * `config` - Client configuration
    ///
    /// # Example
    ///
    /// ```ignore
    /// use cocode_api::{ApiClient, ApiClientConfig};
    /// use cocode_protocol::{ProviderInfo, ProviderType};
    ///
    /// let info = ProviderInfo::new("OpenAI", ProviderType::Openai, "https://api.openai.com/v1")
    ///     .with_api_key("sk-xxx");
    ///
    /// let (client, model) = ApiClient::from_provider_info(&info, "gpt-4o", ApiClientConfig::default())?;
    /// ```
    pub fn from_provider_info(
        info: &ProviderInfo,
        model_slug: &str,
        config: ApiClientConfig,
    ) -> Result<(Self, Arc<dyn Model>)> {
        let model = provider_factory::create_model(info, model_slug)?;
        Ok((Self::with_config(config), model))
    }

    /// Make a streaming request with retry support.
    ///
    /// Returns a [`UnifiedStream`] that can be used to consume the response.
    ///
    /// This method implements automatic fallback and recovery mechanisms:
    ///
    /// 1. **Stream fallback**: If streaming fails with a stream error and
    ///    `enable_stream_fallback` is true, automatically retries with non-streaming.
    ///
    /// 2. **Context overflow recovery**: If the request fails due to context overflow
    ///    and `enable_overflow_recovery` is true, automatically reduces `max_tokens`
    ///    by 25% and retries (up to `max_overflow_attempts` times).
    ///
    /// 3. **Standard retry**: For other retryable errors, applies exponential backoff.
    pub async fn stream_request(
        &self,
        model: &dyn Model,
        request: GenerateRequest,
        options: StreamOptions,
    ) -> Result<UnifiedStream> {
        let mut retry_ctx = RetryContext::new(self.config.retry.clone());
        let mut current_request = request;
        let mut use_streaming = options.streaming;
        let mut overflow_attempts = 0;

        loop {
            debug!(
                model = %model.model_name(),
                attempt = retry_ctx.current_attempt(),
                streaming = use_streaming,
                "Making API request"
            );

            let result = if use_streaming {
                self.do_streaming_request(model, &current_request).await
            } else {
                self.do_non_streaming_request(model, &current_request).await
            };

            match result {
                Ok(stream) => {
                    let stream = if let Some(tx) = options.event_tx.clone() {
                        stream.with_event_sender(tx)
                    } else {
                        stream
                    };
                    return Ok(stream);
                }
                Err(api_error) => {
                    // 1. Context overflow recovery
                    if api_error.is_context_overflow()
                        && self.config.fallback.enable_overflow_recovery
                        && overflow_attempts < self.config.fallback.max_overflow_attempts
                    {
                        if let Some(new_max) = self.try_overflow_recovery(&current_request) {
                            info!(
                                old = ?current_request.max_tokens,
                                new = new_max,
                                attempt = overflow_attempts + 1,
                                max_attempts = self.config.fallback.max_overflow_attempts,
                                "Recovering from context overflow by reducing max_tokens"
                            );
                            current_request = current_request.max_tokens(new_max);
                            overflow_attempts += 1;
                            continue;
                        }
                    }

                    // 2. Stream fallback
                    if use_streaming
                        && self.config.fallback.enable_stream_fallback
                        && api_error.is_stream_error()
                    {
                        info!(
                            error = %api_error,
                            "Falling back to non-streaming due to stream error"
                        );
                        use_streaming = false;
                        if let Some(max) = self.config.fallback.fallback_max_tokens {
                            current_request = current_request.max_tokens(max);
                        }
                        continue;
                    }

                    // 3. Standard retry
                    let decision = retry_ctx.decide(&api_error);

                    match decision {
                        RetryDecision::Retry { delay } => {
                            info!(
                                attempt = retry_ctx.current_attempt(),
                                max = retry_ctx.max_retries(),
                                delay_ms = delay.as_millis() as i64,
                                error = %api_error,
                                "Retrying after error"
                            );
                            tokio::time::sleep(delay).await;
                        }
                        RetryDecision::GiveUp => {
                            return Err(api_error);
                        }
                    }
                }
            }
        }
    }

    /// Attempt to recover from context overflow by reducing max_tokens.
    ///
    /// Reduces max_tokens by 25%, but won't go below `min_output_tokens`.
    /// Returns `None` if reduction would violate the minimum.
    fn try_overflow_recovery(&self, request: &GenerateRequest) -> Option<i32> {
        let current_max = request.max_tokens.unwrap_or(8192);
        let min_tokens = self.config.fallback.min_output_tokens;

        // Reduce by 25%
        let new_max = (current_max as f32 * 0.75) as i32;

        if new_max >= min_tokens {
            Some(new_max)
        } else {
            None
        }
    }

    /// Make a non-streaming request with retry support.
    ///
    /// Calls `model.generate()` directly in a retry loop, returning the
    /// hyper-sdk `GenerateResponse` as-is.
    pub async fn generate(
        &self,
        model: &dyn Model,
        request: GenerateRequest,
    ) -> Result<GenerateResponse> {
        let mut retry_ctx = RetryContext::new(self.config.retry.clone());

        loop {
            debug!(
                model = %model.model_name(),
                attempt = retry_ctx.current_attempt(),
                "Making non-streaming API request"
            );

            let result = model
                .generate(request.clone())
                .await
                .map_err(ApiError::from);

            match result {
                Ok(response) => return Ok(response),
                Err(api_error) => {
                    let decision = retry_ctx.decide(&api_error);

                    match decision {
                        RetryDecision::Retry { delay } => {
                            info!(
                                attempt = retry_ctx.current_attempt(),
                                max = retry_ctx.max_retries(),
                                delay_ms = delay.as_millis() as i64,
                                error = %api_error,
                                "Retrying after error"
                            );
                            tokio::time::sleep(delay).await;
                        }
                        RetryDecision::GiveUp => {
                            return Err(api_error);
                        }
                    }
                }
            }
        }
    }

    /// Internal: make a streaming request.
    async fn do_streaming_request(
        &self,
        model: &dyn Model,
        request: &GenerateRequest,
    ) -> Result<UnifiedStream> {
        let stream_response = model
            .stream(request.clone())
            .await
            .map_err(ApiError::from)?;

        let processor = stream_response.into_processor();

        // Apply stall timeout if configured
        let processor = if self.config.stall_detection_enabled {
            processor.idle_timeout(self.config.stall_timeout)
        } else {
            processor
        };

        Ok(UnifiedStream::from_stream(processor))
    }

    /// Internal: make a non-streaming request.
    async fn do_non_streaming_request(
        &self,
        model: &dyn Model,
        request: &GenerateRequest,
    ) -> Result<UnifiedStream> {
        let response = model
            .generate(request.clone())
            .await
            .map_err(ApiError::from)?;

        Ok(UnifiedStream::from_response(response))
    }
}

impl Default for ApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ApiClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiClient")
            .field("config", &self.config)
            .finish()
    }
}

/// Builder for creating an API client.
pub struct ApiClientBuilder {
    config: ApiClientConfig,
}

impl ApiClientBuilder {
    /// Create a new builder with default configuration.
    pub fn new() -> Self {
        Self {
            config: ApiClientConfig::default(),
        }
    }

    /// Set the retry configuration.
    pub fn retry(mut self, retry: RetryConfig) -> Self {
        self.config.retry = retry;
        self
    }

    /// Set the cache configuration.
    pub fn cache(mut self, cache: PromptCacheConfig) -> Self {
        self.config.cache = cache;
        self
    }

    /// Set the stall timeout.
    pub fn stall_timeout(mut self, timeout: Duration) -> Self {
        self.config.stall_timeout = timeout;
        self
    }

    /// Enable or disable stall detection.
    pub fn stall_detection(mut self, enabled: bool) -> Self {
        self.config.stall_detection_enabled = enabled;
        self
    }

    /// Set the fallback configuration.
    pub fn fallback(mut self, fallback: FallbackConfig) -> Self {
        self.config.fallback = fallback;
        self
    }

    /// Build the API client.
    pub fn build(self) -> ApiClient {
        ApiClient::with_config(self.config)
    }
}

impl Default for ApiClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config_defaults() {
        let config = ApiClientConfig::default();
        assert!(config.cache.enabled);
        assert!(config.stall_detection_enabled);
        assert_eq!(config.stall_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_client_config_builder() {
        let config = ApiClientConfig::default()
            .with_stall_timeout(Duration::from_secs(60))
            .with_stall_detection(false);

        assert_eq!(config.stall_timeout, Duration::from_secs(60));
        assert!(!config.stall_detection_enabled);
    }

    #[test]
    fn test_stream_options() {
        let opts = StreamOptions::streaming();
        assert!(opts.streaming);

        let opts = StreamOptions::non_streaming();
        assert!(!opts.streaming);
    }

    #[test]
    fn test_builder() {
        let builder = ApiClientBuilder::new()
            .stall_timeout(Duration::from_secs(45))
            .stall_detection(false);

        assert_eq!(builder.config.stall_timeout, Duration::from_secs(45));
        assert!(!builder.config.stall_detection_enabled);
    }

    #[test]
    fn test_builder_with_fallback() {
        let builder = ApiClientBuilder::new().fallback(FallbackConfig::disabled());

        assert!(!builder.config.fallback.enable_stream_fallback);
        assert!(!builder.config.fallback.enable_overflow_recovery);
    }

    #[test]
    fn test_fallback_config_defaults() {
        let config = FallbackConfig::default();
        assert!(config.enable_stream_fallback);
        assert!(config.enable_overflow_recovery);
        assert_eq!(config.fallback_max_tokens, Some(21333));
        assert_eq!(config.min_output_tokens, 3000);
        assert_eq!(config.max_overflow_attempts, 3);
    }

    #[test]
    fn test_fallback_config_disabled() {
        let config = FallbackConfig::disabled();
        assert!(!config.enable_stream_fallback);
        assert!(!config.enable_overflow_recovery);
        assert_eq!(config.fallback_max_tokens, None);
        assert_eq!(config.max_overflow_attempts, 0);
    }

    #[test]
    fn test_fallback_config_builder() {
        let config = FallbackConfig::default()
            .with_stream_fallback(false)
            .with_fallback_max_tokens(Some(10000))
            .with_overflow_recovery(false)
            .with_min_output_tokens(1000)
            .with_max_overflow_attempts(5);

        assert!(!config.enable_stream_fallback);
        assert_eq!(config.fallback_max_tokens, Some(10000));
        assert!(!config.enable_overflow_recovery);
        assert_eq!(config.min_output_tokens, 1000);
        assert_eq!(config.max_overflow_attempts, 5);
    }

    #[test]
    fn test_api_client_config_with_fallback() {
        let config = ApiClientConfig::default().with_fallback(FallbackConfig::disabled());

        assert!(!config.fallback.enable_stream_fallback);
        assert!(!config.fallback.enable_overflow_recovery);
    }

    #[test]
    fn test_from_provider_info() {
        use cocode_protocol::{ProviderInfo, ProviderType};

        let info = ProviderInfo::new("Test", ProviderType::Openai, "https://api.openai.com/v1")
            .with_api_key("test-key");

        let result = ApiClient::from_provider_info(&info, "gpt-4o", ApiClientConfig::default());
        assert!(result.is_ok());

        let (client, model) = result.unwrap();
        assert_eq!(model.model_name(), "gpt-4o");
        assert_eq!(model.provider(), "openai");
        assert!(client.config().fallback.enable_stream_fallback);
    }
}
