//! Retry configuration and execution.
//!
//! This module provides exponential backoff retry support with configurable
//! parameters and telemetry integration.
//!
//! # Example
//!
//! ```ignore
//! use hyper_sdk::retry::{RetryConfig, RetryExecutor};
//!
//! let config = RetryConfig::default()
//!     .with_max_attempts(5)
//!     .with_initial_backoff(Duration::from_millis(200));
//!
//! let executor = RetryExecutor::new(config);
//! let result = executor.execute(|| async {
//!     make_api_call().await
//! }).await;
//! ```

use crate::error::HyperError;
use crate::telemetry::RequestTelemetry;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

/// Retry configuration with exponential backoff.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of attempts (1 = no retry).
    pub max_attempts: i32,
    /// Initial backoff delay.
    pub initial_backoff: Duration,
    /// Maximum backoff delay.
    pub max_backoff: Duration,
    /// Backoff multiplier.
    pub backoff_multiplier: f64,
    /// Jitter ratio (0.0-1.0).
    pub jitter_ratio: f64,
    /// Honor retry-after from error.
    pub respect_retry_after: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter_ratio: 0.1,
            respect_retry_after: true,
        }
    }
}

impl RetryConfig {
    /// Create a config that disables retries (single attempt).
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            ..Default::default()
        }
    }

    /// Set the maximum number of attempts.
    pub fn with_max_attempts(mut self, attempts: i32) -> Self {
        self.max_attempts = attempts;
        self
    }

    /// Set the initial backoff delay.
    pub fn with_initial_backoff(mut self, backoff: Duration) -> Self {
        self.initial_backoff = backoff;
        self
    }

    /// Set the maximum backoff delay.
    pub fn with_max_backoff(mut self, max: Duration) -> Self {
        self.max_backoff = max;
        self
    }

    /// Set the backoff multiplier.
    pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }

    /// Set the jitter ratio (0.0 to 1.0).
    pub fn with_jitter_ratio(mut self, ratio: f64) -> Self {
        self.jitter_ratio = ratio.clamp(0.0, 1.0);
        self
    }

    /// Set whether to respect retry-after from errors.
    pub fn with_respect_retry_after(mut self, respect: bool) -> Self {
        self.respect_retry_after = respect;
        self
    }
}

/// Retry executor with telemetry integration.
#[derive(Debug)]
pub struct RetryExecutor {
    config: RetryConfig,
    telemetry: Option<Arc<dyn RequestTelemetry>>,
}

impl RetryExecutor {
    /// Create a new retry executor with the given configuration.
    pub fn new(config: RetryConfig) -> Self {
        Self {
            config,
            telemetry: None,
        }
    }

    /// Add telemetry to the executor.
    pub fn with_telemetry(mut self, telemetry: Arc<dyn RequestTelemetry>) -> Self {
        self.telemetry = Some(telemetry);
        self
    }

    /// Execute an operation with retries.
    ///
    /// The operation is retried according to the configuration when it returns
    /// a retryable error (as determined by `HyperError::is_retryable()`).
    pub async fn execute<F, Fut, T>(&self, mut operation: F) -> Result<T, HyperError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, HyperError>>,
    {
        let mut attempt = 1;

        loop {
            let start = std::time::Instant::now();

            match operation().await {
                Ok(result) => {
                    if let Some(ref telemetry) = self.telemetry {
                        telemetry.on_request(
                            attempt,
                            Some(http::StatusCode::OK),
                            None,
                            start.elapsed(),
                        );
                    }
                    return Ok(result);
                }
                Err(error) => {
                    let duration = start.elapsed();

                    if let Some(ref telemetry) = self.telemetry {
                        telemetry.on_request(attempt, None, Some(&error), duration);
                    }

                    if !error.is_retryable() || attempt >= self.config.max_attempts {
                        if let Some(ref telemetry) = self.telemetry {
                            telemetry.on_exhausted(attempt, &error);
                        }
                        return Err(error);
                    }

                    let delay = self.calculate_delay(attempt, &error);

                    if let Some(ref telemetry) = self.telemetry {
                        telemetry.on_retry(attempt, delay);
                    }

                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
            }
        }
    }

    fn calculate_delay(&self, attempt: i32, error: &HyperError) -> Duration {
        // Honor retry-after if available
        if self.config.respect_retry_after {
            if let Some(delay) = error.retry_delay() {
                return delay.min(self.config.max_backoff);
            }
        }

        // Exponential backoff
        let base = self.config.initial_backoff.as_secs_f64()
            * self.config.backoff_multiplier.powi(attempt - 1);
        let base = base.min(self.config.max_backoff.as_secs_f64());

        // Apply jitter using a simple pseudo-random approach
        let jitter = base * self.config.jitter_ratio * simple_random();
        Duration::from_secs_f64(base + jitter)
    }
}

/// Simple pseudo-random number generator for jitter.
/// Returns a value between 0.0 and 1.0.
fn simple_random() -> f64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    // Use a combination of time and counter for basic randomness
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    let count = COUNTER.fetch_add(1, Ordering::Relaxed);

    // Simple hash-like mixing
    let mixed = now.wrapping_mul(0x517cc1b727220a95).wrapping_add(count);
    let mixed = mixed ^ (mixed >> 33);
    let mixed = mixed.wrapping_mul(0xc4ceb9fe1a85ec53);
    let mixed = mixed ^ (mixed >> 33);

    // Convert to 0.0-1.0 range
    (mixed as f64) / (u64::MAX as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicI32;

    #[tokio::test]
    async fn test_retry_success_first_attempt() {
        let executor = RetryExecutor::new(RetryConfig::default());
        let attempts = AtomicI32::new(0);

        let result = executor
            .execute(|| {
                attempts.fetch_add(1, Ordering::SeqCst);
                async { Ok::<_, HyperError>(42) }
            })
            .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let config = RetryConfig::default()
            .with_max_attempts(5)
            .with_initial_backoff(Duration::from_millis(1));

        let executor = RetryExecutor::new(config);
        let attempts = AtomicI32::new(0);

        let result = executor
            .execute(|| {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                async move {
                    if attempt < 3 {
                        Err(HyperError::NetworkError("connection failed".to_string()))
                    } else {
                        Ok(42)
                    }
                }
            })
            .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let config = RetryConfig::default()
            .with_max_attempts(3)
            .with_initial_backoff(Duration::from_millis(1));

        let executor = RetryExecutor::new(config);
        let attempts = AtomicI32::new(0);

        let result: Result<i32, HyperError> = executor
            .execute(|| {
                attempts.fetch_add(1, Ordering::SeqCst);
                async { Err(HyperError::NetworkError("always fails".to_string())) }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error() {
        let config = RetryConfig::default()
            .with_max_attempts(5)
            .with_initial_backoff(Duration::from_millis(1));

        let executor = RetryExecutor::new(config);
        let attempts = AtomicI32::new(0);

        let result: Result<i32, HyperError> = executor
            .execute(|| {
                attempts.fetch_add(1, Ordering::SeqCst);
                async {
                    // Auth errors are not retryable
                    Err(HyperError::AuthenticationFailed("invalid key".to_string()))
                }
            })
            .await;

        assert!(result.is_err());
        // Should not retry auth errors
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_no_retry_config() {
        let executor = RetryExecutor::new(RetryConfig::no_retry());
        let attempts = AtomicI32::new(0);

        let result: Result<i32, HyperError> = executor
            .execute(|| {
                attempts.fetch_add(1, Ordering::SeqCst);
                async { Err(HyperError::NetworkError("fail".to_string())) }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_respects_retry_after() {
        let config = RetryConfig::default()
            .with_max_attempts(2)
            .with_initial_backoff(Duration::from_secs(10)) // Long backoff
            .with_respect_retry_after(true);

        let executor = RetryExecutor::new(config);
        let attempts = AtomicI32::new(0);
        let start = std::time::Instant::now();

        let result: Result<i32, HyperError> = executor
            .execute(|| {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                async move {
                    if attempt == 1 {
                        // Short retry-after should be used instead of long backoff
                        Err(HyperError::Retryable {
                            message: "rate limited".to_string(),
                            delay: Some(Duration::from_millis(10)),
                        })
                    } else {
                        Ok(42)
                    }
                }
            })
            .await;

        let elapsed = start.elapsed();
        assert_eq!(result.unwrap(), 42);
        // Should use short delay from retry-after, not 10 second backoff
        assert!(elapsed < Duration::from_secs(1));
    }

    #[derive(Debug)]
    struct TestTelemetry {
        requests: Mutex<Vec<(i32, Option<http::StatusCode>, bool)>>,
        retries: Mutex<Vec<(i32, Duration)>>,
        exhausted: Mutex<Option<(i32, String)>>,
    }

    impl TestTelemetry {
        fn new() -> Self {
            Self {
                requests: Mutex::new(Vec::new()),
                retries: Mutex::new(Vec::new()),
                exhausted: Mutex::new(None),
            }
        }
    }

    impl RequestTelemetry for TestTelemetry {
        fn on_request(
            &self,
            attempt: i32,
            status: Option<http::StatusCode>,
            error: Option<&HyperError>,
            _duration: Duration,
        ) {
            self.requests
                .lock()
                .unwrap()
                .push((attempt, status, error.is_some()));
        }

        fn on_retry(&self, attempt: i32, delay: Duration) {
            self.retries.lock().unwrap().push((attempt, delay));
        }

        fn on_exhausted(&self, total_attempts: i32, final_error: &HyperError) {
            *self.exhausted.lock().unwrap() = Some((total_attempts, final_error.to_string()));
        }
    }

    #[tokio::test]
    async fn test_retry_telemetry() {
        let config = RetryConfig::default()
            .with_max_attempts(3)
            .with_initial_backoff(Duration::from_millis(1));

        let telemetry = Arc::new(TestTelemetry::new());
        let executor = RetryExecutor::new(config).with_telemetry(telemetry.clone());

        let attempts = AtomicI32::new(0);
        let _: Result<i32, HyperError> = executor
            .execute(|| {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                async move {
                    if attempt < 3 {
                        Err(HyperError::NetworkError("fail".to_string()))
                    } else {
                        Ok(42)
                    }
                }
            })
            .await;

        let requests = telemetry.requests.lock().unwrap();
        assert_eq!(requests.len(), 3);
        // First two have errors
        assert!(requests[0].2);
        assert!(requests[1].2);
        // Third is success
        assert!(!requests[2].2);
        assert_eq!(requests[2].1, Some(http::StatusCode::OK));

        let retries = telemetry.retries.lock().unwrap();
        assert_eq!(retries.len(), 2); // Two retries before success

        // No exhausted call since we succeeded
        assert!(telemetry.exhausted.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn test_retry_telemetry_exhausted() {
        let config = RetryConfig::default()
            .with_max_attempts(2)
            .with_initial_backoff(Duration::from_millis(1));

        let telemetry = Arc::new(TestTelemetry::new());
        let executor = RetryExecutor::new(config).with_telemetry(telemetry.clone());

        let _: Result<i32, HyperError> = executor
            .execute(|| async { Err(HyperError::NetworkError("fail".to_string())) })
            .await;

        let exhausted = telemetry.exhausted.lock().unwrap();
        assert!(exhausted.is_some());
        let (attempts, msg) = exhausted.as_ref().unwrap();
        assert_eq!(*attempts, 2);
        assert!(msg.contains("fail"));
    }

    #[test]
    fn test_config_builder() {
        let config = RetryConfig::default()
            .with_max_attempts(5)
            .with_initial_backoff(Duration::from_millis(200))
            .with_max_backoff(Duration::from_secs(60))
            .with_backoff_multiplier(3.0)
            .with_jitter_ratio(0.2)
            .with_respect_retry_after(false);

        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.initial_backoff, Duration::from_millis(200));
        assert_eq!(config.max_backoff, Duration::from_secs(60));
        assert_eq!(config.backoff_multiplier, 3.0);
        assert_eq!(config.jitter_ratio, 0.2);
        assert!(!config.respect_retry_after);
    }

    #[test]
    fn test_jitter_ratio_clamped() {
        let config = RetryConfig::default().with_jitter_ratio(2.0);
        assert_eq!(config.jitter_ratio, 1.0);

        let config = RetryConfig::default().with_jitter_ratio(-0.5);
        assert_eq!(config.jitter_ratio, 0.0);
    }

    #[test]
    fn test_simple_random_produces_valid_range() {
        for _ in 0..100 {
            let r = simple_random();
            assert!(r >= 0.0 && r <= 1.0);
        }
    }
}
