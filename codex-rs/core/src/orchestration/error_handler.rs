//! Error handling and retry logic for orchestration.

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;
use tokio::time::sleep;
use tracing::debug;
use tracing::info;
use tracing::warn;

/// Retry policy for failed operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retries
    pub max_retries: usize,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryPolicy {
    /// Calculate backoff duration for a given retry attempt.
    pub fn backoff_duration(&self, attempt: usize) -> Duration {
        let backoff =
            self.initial_backoff.as_secs_f64() * self.backoff_multiplier.powi(attempt as i32);
        let backoff_secs = backoff.min(self.max_backoff.as_secs_f64());
        Duration::from_secs_f64(backoff_secs)
    }
}

/// Fallback strategy when operations fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FallbackStrategy {
    /// Retry with exponential backoff
    RetryWithBackoff,
    /// Switch to sequential execution
    FallbackToSequential,
    /// Skip the failed operation and continue
    SkipAndContinue,
    /// Fail immediately
    FailImmediately,
}

/// Resolution for an agent error.
#[derive(Debug, Clone)]
pub enum ErrorResolution {
    /// Retry the operation
    Retry { after: Duration },
    /// Skip this agent and continue
    Skip,
    /// Switch to sequential execution
    SwitchToSequential,
    /// Fail the entire orchestration
    Fail,
}

/// Agent-specific error types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentError {
    /// Operation timed out
    Timeout,
    /// API rate limit exceeded
    ApiRateLimit,
    /// File not found
    FileNotFound,
    /// Permission denied
    PermissionDenied,
    /// Network error
    NetworkError,
    /// Unknown error
    Unknown,
}

impl AgentError {
    /// Determine if the error is retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            AgentError::Timeout | AgentError::ApiRateLimit | AgentError::NetworkError
        )
    }

    /// Get the default fallback strategy for this error.
    pub fn default_fallback(&self) -> FallbackStrategy {
        match self {
            AgentError::Timeout => FallbackStrategy::RetryWithBackoff,
            AgentError::ApiRateLimit => FallbackStrategy::FallbackToSequential,
            AgentError::FileNotFound => FallbackStrategy::SkipAndContinue,
            AgentError::PermissionDenied => FallbackStrategy::FailImmediately,
            AgentError::NetworkError => FallbackStrategy::RetryWithBackoff,
            AgentError::Unknown => FallbackStrategy::RetryWithBackoff,
        }
    }
}

/// Error handler for orchestration failures.
pub struct ErrorHandler {
    /// Retry policy
    retry_policy: RetryPolicy,
    /// Default fallback strategy
    _default_fallback: FallbackStrategy, // Prefixed with _ to suppress unused warning
}

impl ErrorHandler {
    /// Create a new error handler with default settings.
    pub fn new() -> Self {
        Self {
            retry_policy: RetryPolicy::default(),
            _default_fallback: FallbackStrategy::RetryWithBackoff,
        }
    }

    /// Create an error handler with custom settings.
    pub fn with_policy(retry_policy: RetryPolicy, _default_fallback: FallbackStrategy) -> Self {
        Self {
            retry_policy,
            _default_fallback,
        }
    }

    /// Handle an agent error and determine the resolution.
    pub async fn handle_agent_error(
        &self,
        error: AgentError,
        agent_name: &str,
        attempt: usize,
    ) -> ErrorResolution {
        info!(
            "🔧 Handling error for agent '{}': {:?} (attempt {}/{})",
            agent_name,
            error,
            attempt + 1,
            self.retry_policy.max_retries
        );

        // Check if we've exceeded max retries
        if attempt >= self.retry_policy.max_retries {
            warn!(
                "⚠️  Agent '{}' exceeded max retries ({})",
                agent_name, self.retry_policy.max_retries
            );
            return ErrorResolution::Fail;
        }

        // Get the fallback strategy for this error
        let strategy = error.default_fallback();

        match strategy {
            FallbackStrategy::RetryWithBackoff => {
                let backoff = self.retry_policy.backoff_duration(attempt);
                info!("🔄 Retrying agent '{}' after {:?}", agent_name, backoff);
                ErrorResolution::Retry { after: backoff }
            }
            FallbackStrategy::FallbackToSequential => {
                info!(
                    "🔀 Switching to sequential execution for agent '{}'",
                    agent_name
                );
                ErrorResolution::SwitchToSequential
            }
            FallbackStrategy::SkipAndContinue => {
                info!("⏭️  Skipping agent '{}' and continuing", agent_name);
                ErrorResolution::Skip
            }
            FallbackStrategy::FailImmediately => {
                warn!("❌ Failing immediately for agent '{}'", agent_name);
                ErrorResolution::Fail
            }
        }
    }

    /// Retry an async operation with the configured policy.
    pub async fn retry<F, T, E>(&self, mut operation: F) -> Result<T, E>
    where
        F: FnMut() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, E>> + Send>>,
        E: std::fmt::Debug,
    {
        let mut attempt = 0;

        loop {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    if attempt >= self.retry_policy.max_retries {
                        debug!("Max retries exceeded, failing");
                        return Err(err);
                    }

                    let backoff = self.retry_policy.backoff_duration(attempt);
                    debug!(
                        "Operation failed (attempt {}/{}): {:?}, retrying after {:?}",
                        attempt + 1,
                        self.retry_policy.max_retries,
                        err,
                        backoff
                    );

                    sleep(backoff).await;
                    attempt += 1;
                }
            }
        }
    }
}

impl Default for ErrorHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_policy_backoff() {
        let policy = RetryPolicy::default();

        let backoff0 = policy.backoff_duration(0);
        let backoff1 = policy.backoff_duration(1);
        let backoff2 = policy.backoff_duration(2);

        assert_eq!(backoff0, Duration::from_secs(1));
        assert_eq!(backoff1, Duration::from_secs(2));
        assert_eq!(backoff2, Duration::from_secs(4));
    }

    #[test]
    fn test_retry_policy_max_backoff() {
        let policy = RetryPolicy::default();

        // Should cap at max_backoff (30s)
        let backoff10 = policy.backoff_duration(10);
        assert_eq!(backoff10, Duration::from_secs(30));
    }

    #[test]
    fn test_agent_error_is_retryable() {
        assert!(AgentError::Timeout.is_retryable());
        assert!(AgentError::ApiRateLimit.is_retryable());
        assert!(AgentError::NetworkError.is_retryable());
        assert!(!AgentError::FileNotFound.is_retryable());
        assert!(!AgentError::PermissionDenied.is_retryable());
    }

    #[test]
    fn test_agent_error_default_fallback() {
        assert_eq!(
            AgentError::Timeout.default_fallback(),
            FallbackStrategy::RetryWithBackoff
        );
        assert_eq!(
            AgentError::ApiRateLimit.default_fallback(),
            FallbackStrategy::FallbackToSequential
        );
        assert_eq!(
            AgentError::FileNotFound.default_fallback(),
            FallbackStrategy::SkipAndContinue
        );
        assert_eq!(
            AgentError::PermissionDenied.default_fallback(),
            FallbackStrategy::FailImmediately
        );
    }

    #[tokio::test]
    async fn test_error_handler_retry() {
        let handler = ErrorHandler::new();

        let resolution = handler
            .handle_agent_error(AgentError::Timeout, "test-agent", 0)
            .await;

        match resolution {
            ErrorResolution::Retry { after } => {
                assert_eq!(after, Duration::from_secs(1));
            }
            _ => panic!("Expected Retry resolution"),
        }
    }

    #[tokio::test]
    async fn test_error_handler_max_retries() {
        let handler = ErrorHandler::new();

        let resolution = handler
            .handle_agent_error(AgentError::Timeout, "test-agent", 3)
            .await;

        match resolution {
            ErrorResolution::Fail => {}
            _ => panic!("Expected Fail resolution after max retries"),
        }
    }
}
