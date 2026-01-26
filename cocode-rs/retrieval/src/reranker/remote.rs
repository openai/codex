//! Remote API-based reranker.
//!
//! Supports Cohere, Voyage AI, and custom API endpoints.
//!
//! ## Supported Providers
//!
//! - **Cohere**: `rerank-english-v3.0`, `rerank-multilingual-v3.0`
//! - **Voyage AI**: `rerank-2`, `rerank-lite-1`
//! - **Custom**: Any OpenAI-compatible rerank API
//!
//! ## Example
//!
//! ```toml
//! [retrieval.extended_reranker]
//! backend = "remote"
//!
//! [retrieval.extended_reranker.remote]
//! provider = "cohere"
//! model = "rerank-english-v3.0"
//! api_key_env = "COHERE_API_KEY"
//! ```

use std::cmp::Ordering;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::time::Duration;
use std::time::Instant;

use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;

use crate::config::RemoteRerankerConfig;
use crate::config::RerankerProvider;
use crate::error::Result;
use crate::error::RetrievalErr;
use crate::types::SearchResult;

use super::Reranker;
use super::RerankerCapabilities;

/// Circuit breaker state for remote API protection.
///
/// Opens after `failure_threshold` consecutive failures, preventing further
/// API calls until `reset_timeout_secs` has elapsed. This protects against
/// quota exhaustion and improves responsiveness when the API is unavailable.
#[derive(Debug)]
struct CircuitBreaker {
    /// Number of consecutive failures
    failures: AtomicI32,
    /// Timestamp (seconds since UNIX epoch) of last failure
    last_failure_secs: AtomicU64,
    /// Threshold before circuit opens
    failure_threshold: i32,
    /// Seconds to wait before attempting recovery
    reset_timeout_secs: i32,
    /// Start time for elapsed calculation
    start_instant: Instant,
}

impl CircuitBreaker {
    fn new(failure_threshold: i32, reset_timeout_secs: i32) -> Self {
        Self {
            failures: AtomicI32::new(0),
            last_failure_secs: AtomicU64::new(0),
            failure_threshold,
            reset_timeout_secs,
            start_instant: Instant::now(),
        }
    }

    /// Check if the circuit is open (blocking requests).
    fn is_open(&self) -> bool {
        let failures = self.failures.load(AtomicOrdering::SeqCst);
        if failures < self.failure_threshold {
            return false;
        }

        // Check if enough time has passed to attempt recovery
        let last_failure = self.last_failure_secs.load(AtomicOrdering::SeqCst);
        let now_secs = self.start_instant.elapsed().as_secs();
        let elapsed = now_secs.saturating_sub(last_failure);

        elapsed < self.reset_timeout_secs as u64
    }

    /// Record a successful request (resets failure count).
    fn record_success(&self) {
        self.failures.store(0, AtomicOrdering::SeqCst);
    }

    /// Record a failed request.
    fn record_failure(&self) {
        self.failures.fetch_add(1, AtomicOrdering::SeqCst);
        let now_secs = self.start_instant.elapsed().as_secs();
        self.last_failure_secs
            .store(now_secs, AtomicOrdering::SeqCst);
    }

    /// Get current failure count for logging.
    fn failure_count(&self) -> i32 {
        self.failures.load(AtomicOrdering::SeqCst)
    }
}

/// Remote API-based reranker.
#[derive(Debug)]
pub struct RemoteReranker {
    provider: RerankerProvider,
    model: String,
    api_key_env: String,
    base_url: Option<String>,
    #[allow(dead_code)] // Reserved for retry configuration
    timeout_secs: i32,
    #[allow(dead_code)] // Stored for potential future use
    max_retries: i32,
    top_n: Option<i32>,
    client: reqwest::Client,
    /// Circuit breaker for API protection
    circuit_breaker: CircuitBreaker,
}

/// Default circuit breaker failure threshold
const DEFAULT_FAILURE_THRESHOLD: i32 = 5;
/// Default circuit breaker reset timeout in seconds
const DEFAULT_RESET_TIMEOUT_SECS: i32 = 60;

impl RemoteReranker {
    /// Create a new remote reranker from config.
    pub fn new(config: &RemoteRerankerConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs as u64))
            .build()
            .map_err(|e| RetrievalErr::RerankerError {
                provider: format!("{:?}", config.provider),
                cause: format!("Failed to create HTTP client: {}", e),
            })?;

        // Circuit breaker: open after 5 failures, reset after 60 seconds
        let circuit_breaker =
            CircuitBreaker::new(DEFAULT_FAILURE_THRESHOLD, DEFAULT_RESET_TIMEOUT_SECS);

        Ok(Self {
            provider: config.provider.clone(),
            model: config.model.clone(),
            api_key_env: config.api_key_env.clone(),
            base_url: config.base_url.clone(),
            timeout_secs: config.timeout_secs,
            max_retries: config.max_retries,
            top_n: config.top_n,
            client,
            circuit_breaker,
        })
    }

    /// Get API key from environment variable.
    fn get_api_key(&self) -> Result<String> {
        std::env::var(&self.api_key_env).map_err(|_| RetrievalErr::ConfigError {
            field: "api_key_env".to_string(),
            cause: format!(
                "Environment variable '{}' not set. Required for {:?} reranker.",
                self.api_key_env, self.provider
            ),
        })
    }

    /// Get the API endpoint URL for the provider.
    fn get_endpoint(&self) -> &str {
        if let Some(ref url) = self.base_url {
            return url;
        }

        match self.provider {
            RerankerProvider::Cohere => "https://api.cohere.ai/v1/rerank",
            RerankerProvider::VoyageAi => "https://api.voyageai.com/v1/rerank",
            RerankerProvider::Custom => "",
        }
    }

    /// Rerank using Cohere API.
    async fn rerank_cohere(&self, query: &str, documents: &[&str]) -> Result<Vec<(usize, f32)>> {
        let api_key = self.get_api_key()?;
        let url = self.get_endpoint();

        let request = CohereRerankRequest {
            model: &self.model,
            query,
            documents,
            top_n: self.top_n.unwrap_or(documents.len() as i32),
            return_documents: false,
        };

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| RetrievalErr::RerankerError {
                provider: "cohere".to_string(),
                cause: format!("HTTP request failed: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(RetrievalErr::RerankerApiError {
                provider: "cohere".to_string(),
                status,
                body,
            });
        }

        let result: CohereRerankResponse =
            response
                .json()
                .await
                .map_err(|e| RetrievalErr::RerankerError {
                    provider: "cohere".to_string(),
                    cause: format!("Failed to parse response: {}", e),
                })?;

        Ok(result
            .results
            .into_iter()
            .map(|r| (r.index as usize, r.relevance_score as f32))
            .collect())
    }

    /// Rerank using Voyage AI API.
    async fn rerank_voyage(&self, query: &str, documents: &[&str]) -> Result<Vec<(usize, f32)>> {
        let api_key = self.get_api_key()?;
        let url = self.get_endpoint();

        let request = VoyageRerankRequest {
            model: &self.model,
            query,
            documents,
            top_k: self.top_n.unwrap_or(documents.len() as i32),
        };

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| RetrievalErr::RerankerError {
                provider: "voyage_ai".to_string(),
                cause: format!("HTTP request failed: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(RetrievalErr::RerankerApiError {
                provider: "voyage_ai".to_string(),
                status,
                body,
            });
        }

        let result: VoyageRerankResponse =
            response
                .json()
                .await
                .map_err(|e| RetrievalErr::RerankerError {
                    provider: "voyage_ai".to_string(),
                    cause: format!("Failed to parse response: {}", e),
                })?;

        Ok(result
            .data
            .into_iter()
            .map(|r| (r.index as usize, r.relevance_score as f32))
            .collect())
    }
}

#[async_trait]
impl Reranker for RemoteReranker {
    fn name(&self) -> &str {
        &self.model
    }

    fn capabilities(&self) -> RerankerCapabilities {
        RerankerCapabilities {
            requires_network: true,
            supports_batch: true,
            max_batch_size: Some(100), // Most APIs support up to 100 documents
            is_async: true,
        }
    }

    async fn rerank(&self, query: &str, results: &mut [SearchResult]) -> Result<()> {
        if results.is_empty() {
            return Ok(());
        }

        // Check circuit breaker before making API call
        if self.circuit_breaker.is_open() {
            tracing::warn!(
                provider = ?self.provider,
                failures = self.circuit_breaker.failure_count(),
                "Remote reranker circuit breaker is open, skipping rerank"
            );
            // Return without reranking - results keep their original scores
            return Ok(());
        }

        // Extract document contents
        let documents: Vec<&str> = results.iter().map(|r| r.chunk.content.as_str()).collect();

        // Call appropriate provider API with circuit breaker tracking
        let scores_result = match self.provider {
            RerankerProvider::Cohere => self.rerank_cohere(query, &documents).await,
            RerankerProvider::VoyageAi => self.rerank_voyage(query, &documents).await,
            RerankerProvider::Custom => {
                // Default to Cohere-compatible API for custom endpoints
                self.rerank_cohere(query, &documents).await
            }
        };

        // Handle result with circuit breaker
        let scores = match scores_result {
            Ok(s) => {
                self.circuit_breaker.record_success();
                s
            }
            Err(e) => {
                self.circuit_breaker.record_failure();
                let failures = self.circuit_breaker.failure_count();
                tracing::warn!(
                    provider = ?self.provider,
                    error = %e,
                    failures = failures,
                    threshold = DEFAULT_FAILURE_THRESHOLD,
                    "Remote reranker API call failed"
                );
                // Return without reranking on error
                return Err(e);
            }
        };

        // Update scores
        for (idx, score) in scores {
            if idx < results.len() {
                results[idx].score = score;
            }
        }

        // Re-sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));

        Ok(())
    }
}

// ============================================================================
// Cohere API Types
// ============================================================================

#[derive(Serialize)]
struct CohereRerankRequest<'a> {
    model: &'a str,
    query: &'a str,
    documents: &'a [&'a str],
    top_n: i32,
    return_documents: bool,
}

#[derive(Deserialize)]
struct CohereRerankResponse {
    results: Vec<CohereRerankResult>,
}

#[derive(Deserialize)]
struct CohereRerankResult {
    index: i32,
    relevance_score: f64,
}

// ============================================================================
// Voyage AI API Types
// ============================================================================

#[derive(Serialize)]
struct VoyageRerankRequest<'a> {
    model: &'a str,
    query: &'a str,
    documents: &'a [&'a str],
    top_k: i32,
}

#[derive(Deserialize)]
struct VoyageRerankResponse {
    data: Vec<VoyageRerankResult>,
}

#[derive(Deserialize)]
struct VoyageRerankResult {
    index: i32,
    relevance_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RerankerProvider;

    #[test]
    fn test_get_endpoint() {
        let config = RemoteRerankerConfig {
            provider: RerankerProvider::Cohere,
            model: "rerank-english-v3.0".to_string(),
            api_key_env: "COHERE_API_KEY".to_string(),
            base_url: None,
            timeout_secs: 10,
            max_retries: 2,
            top_n: None,
        };

        let reranker = RemoteReranker::new(&config).unwrap();
        assert_eq!(reranker.get_endpoint(), "https://api.cohere.ai/v1/rerank");
    }

    #[test]
    fn test_custom_base_url() {
        let config = RemoteRerankerConfig {
            provider: RerankerProvider::Custom,
            model: "custom-model".to_string(),
            api_key_env: "CUSTOM_API_KEY".to_string(),
            base_url: Some("https://custom.api.com/rerank".to_string()),
            timeout_secs: 10,
            max_retries: 2,
            top_n: None,
        };

        let reranker = RemoteReranker::new(&config).unwrap();
        assert_eq!(reranker.get_endpoint(), "https://custom.api.com/rerank");
    }
}
