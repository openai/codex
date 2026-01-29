//! Retry context for agent loop with exponential backoff.
//!
//! This module provides [`RetryContext`] which extends hyper-sdk's retry
//! capabilities with exponential backoff and retry decisions.

use crate::error::ApiError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for retry behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    #[serde(default = "default_max_retries")]
    pub max_retries: i32,
    /// Base delay for exponential backoff (ms).
    #[serde(default = "default_base_delay")]
    pub base_delay_ms: i64,
    /// Maximum delay cap (ms).
    #[serde(default = "default_max_delay")]
    pub max_delay_ms: i64,
    /// Backoff multiplier.
    #[serde(default = "default_multiplier")]
    pub multiplier: f64,
}

fn default_max_retries() -> i32 {
    3
}
fn default_base_delay() -> i64 {
    1000
}
fn default_max_delay() -> i64 {
    30000
}
fn default_multiplier() -> f64 {
    2.0
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            base_delay_ms: default_base_delay(),
            max_delay_ms: default_max_delay(),
            multiplier: default_multiplier(),
        }
    }
}

impl RetryConfig {
    /// Create a config with no retries.
    pub fn no_retry() -> Self {
        Self {
            max_retries: 0,
            ..Default::default()
        }
    }

    /// Set maximum retry attempts.
    pub fn with_max_retries(mut self, max: i32) -> Self {
        self.max_retries = max;
        self
    }

    /// Set base delay.
    pub fn with_base_delay(mut self, delay: Duration) -> Self {
        self.base_delay_ms = delay.as_millis() as i64;
        self
    }

    /// Set maximum delay.
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay_ms = delay.as_millis() as i64;
        self
    }

    /// Set backoff multiplier.
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier;
        self
    }
}

/// Retry context that tracks attempts and provides backoff calculation.
///
/// This context is used during a single request's retry cycle. It tracks
/// the number of attempts and calculates appropriate delays for retries.
#[derive(Debug, Clone)]
pub struct RetryContext {
    config: RetryConfig,
    current_attempt: i32,
    last_error: Option<String>,
}

impl RetryContext {
    /// Create a new retry context with the given configuration.
    pub fn new(config: RetryConfig) -> Self {
        Self {
            config,
            current_attempt: 0,
            last_error: None,
        }
    }

    /// Create a retry context with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(RetryConfig::default())
    }

    /// Record an attempt and return if retry should be attempted.
    pub fn should_retry(&mut self, error: &ApiError) -> bool {
        self.current_attempt += 1;
        self.last_error = Some(error.to_string());

        // Check if retryable and within limits
        error.is_retryable() && self.current_attempt <= self.config.max_retries
    }

    /// Calculate the delay before the next retry.
    pub fn calculate_delay(&self, error: &ApiError) -> Duration {
        // Honor retry-after hint if available
        if let Some(delay) = error.retry_delay() {
            return delay.min(Duration::from_millis(self.config.max_delay_ms as u64));
        }

        // Exponential backoff
        let base = self.config.base_delay_ms as f64;
        let delay_ms = base * self.config.multiplier.powi(self.current_attempt - 1);
        let delay_ms = delay_ms.min(self.config.max_delay_ms as f64) as i64;

        Duration::from_millis(delay_ms as u64)
    }

    /// Get the current attempt number.
    pub fn current_attempt(&self) -> i32 {
        self.current_attempt
    }

    /// Get the last error message.
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Get the maximum retry attempts.
    pub fn max_retries(&self) -> i32 {
        self.config.max_retries
    }

    /// Reset the context for a new request.
    pub fn reset(&mut self) {
        self.current_attempt = 0;
        self.last_error = None;
    }

    /// Check if retries are exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.current_attempt > self.config.max_retries
    }

    /// Create an exhausted error.
    pub fn exhausted_error(&self) -> ApiError {
        ApiError::retries_exhausted(
            self.current_attempt,
            self.last_error
                .clone()
                .unwrap_or_else(|| "Unknown".to_string()),
        )
    }

    /// Make a retry decision based on the error.
    pub fn decide(&mut self, error: &ApiError) -> RetryDecision {
        if self.should_retry(error) {
            let delay = self.calculate_delay(error);
            RetryDecision::Retry { delay }
        } else {
            RetryDecision::GiveUp
        }
    }
}

/// Result of a retry decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryDecision {
    /// Retry the request after the specified delay.
    Retry { delay: Duration },
    /// Give up and return the error.
    GiveUp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_defaults() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 30000);
        assert_eq!(config.multiplier, 2.0);
    }

    #[test]
    fn test_retry_config_builder() {
        let config = RetryConfig::default()
            .with_max_retries(5)
            .with_base_delay(Duration::from_millis(500))
            .with_max_delay(Duration::from_secs(60))
            .with_multiplier(1.5);

        assert_eq!(config.max_retries, 5);
        assert_eq!(config.base_delay_ms, 500);
        assert_eq!(config.max_delay_ms, 60000);
        assert_eq!(config.multiplier, 1.5);
    }

    #[test]
    fn test_should_retry() {
        let mut ctx = RetryContext::new(RetryConfig::default().with_max_retries(3));

        // First attempt
        let error = ApiError::network("connection failed");
        assert!(ctx.should_retry(&error));
        assert_eq!(ctx.current_attempt(), 1);

        // Second attempt
        assert!(ctx.should_retry(&error));
        assert_eq!(ctx.current_attempt(), 2);

        // Third attempt
        assert!(ctx.should_retry(&error));
        assert_eq!(ctx.current_attempt(), 3);

        // Fourth attempt - should fail
        assert!(!ctx.should_retry(&error));
        assert_eq!(ctx.current_attempt(), 4);
    }

    #[test]
    fn test_non_retryable_error() {
        let mut ctx = RetryContext::with_defaults();

        let error = ApiError::authentication("invalid key");
        assert!(!ctx.should_retry(&error));
    }

    #[test]
    fn test_delay_calculation() {
        let ctx = RetryContext::new(
            RetryConfig::default()
                .with_base_delay(Duration::from_millis(100))
                .with_multiplier(2.0),
        );

        let error = ApiError::network("test");

        // Note: delay calculation uses current_attempt which starts at 0
        // After first should_retry, it becomes 1
        let mut ctx = ctx;
        ctx.current_attempt = 1;
        assert_eq!(ctx.calculate_delay(&error), Duration::from_millis(100));

        ctx.current_attempt = 2;
        assert_eq!(ctx.calculate_delay(&error), Duration::from_millis(200));

        ctx.current_attempt = 3;
        assert_eq!(ctx.calculate_delay(&error), Duration::from_millis(400));
    }

    #[test]
    fn test_delay_respects_max() {
        let mut ctx = RetryContext::new(
            RetryConfig::default()
                .with_base_delay(Duration::from_secs(10))
                .with_max_delay(Duration::from_secs(5)),
        );

        ctx.current_attempt = 1;
        let error = ApiError::network("test");
        // Should be capped at max_delay
        assert_eq!(ctx.calculate_delay(&error), Duration::from_secs(5));
    }

    #[test]
    fn test_delay_honors_retry_after() {
        let mut ctx =
            RetryContext::new(RetryConfig::default().with_base_delay(Duration::from_secs(10)));
        ctx.current_attempt = 1;

        let error = ApiError::rate_limited("test", 2000);
        assert_eq!(ctx.calculate_delay(&error), Duration::from_millis(2000));
    }

    #[test]
    fn test_retry_decision() {
        let mut ctx = RetryContext::new(RetryConfig::default().with_max_retries(3));

        // Network error - should retry
        let error = ApiError::network("test");
        match ctx.decide(&error) {
            RetryDecision::Retry { .. } => {}
            _ => panic!("Expected Retry"),
        }

        // Reset for next test
        ctx.reset();

        // Auth error - should give up
        let error = ApiError::authentication("test");
        assert_eq!(ctx.decide(&error), RetryDecision::GiveUp);
    }

    #[test]
    fn test_reset() {
        let mut ctx = RetryContext::with_defaults();

        let error = ApiError::network("test");
        ctx.should_retry(&error);
        ctx.should_retry(&error);
        assert_eq!(ctx.current_attempt(), 2);

        ctx.reset();
        assert_eq!(ctx.current_attempt(), 0);
        assert!(ctx.last_error().is_none());
    }
}
