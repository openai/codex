//! Reranking module for search results.
//!
//! Provides post-retrieval reranking to improve search relevance.
//!
//! ## Supported Backends
//!
//! - **RuleBased**: Fast, no ML model required. Boosts based on exact match, path, recency.
//! - **Local**: Neural reranking using fastembed-rs (ONNX Runtime). Requires `neural-reranker` feature.
//! - **Remote**: API-based reranking (Cohere, Voyage AI, etc.). Requires network access.
//! - **Chain**: Combine multiple rerankers sequentially.

pub mod rule_based;

#[cfg(feature = "neural-reranker")]
pub mod local;

pub mod registry;
pub mod remote;

pub use rule_based::RuleBasedReranker;
pub use rule_based::RuleBasedRerankerConfig;

#[cfg(feature = "neural-reranker")]
pub use local::LocalReranker;

pub use registry::ChainReranker;
pub use registry::create_reranker;
pub use remote::RemoteReranker;

use crate::error::Result;
use crate::types::SearchResult;
use async_trait::async_trait;

/// Reranker capabilities descriptor.
///
/// Describes the runtime characteristics of a reranker implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RerankerCapabilities {
    /// Whether this reranker requires network access
    pub requires_network: bool,
    /// Whether this reranker supports batch processing
    pub supports_batch: bool,
    /// Maximum batch size (if applicable)
    pub max_batch_size: Option<i32>,
    /// Whether the rerank operation is truly async (vs sync wrapped in async)
    pub is_async: bool,
}

impl Default for RerankerCapabilities {
    fn default() -> Self {
        Self {
            requires_network: false,
            supports_batch: false,
            max_batch_size: None,
            is_async: false,
        }
    }
}

/// Async reranker trait for post-retrieval score adjustment.
///
/// Implementations can use rules, models, or hybrid approaches
/// to reorder search results for better relevance.
///
/// All rerankers implement this async trait for consistency,
/// even if the underlying operation is synchronous.
#[async_trait]
pub trait Reranker: Send + Sync + std::fmt::Debug {
    /// Returns the name of this reranker (for logging/debugging).
    fn name(&self) -> &str;

    /// Returns the capabilities of this reranker.
    fn capabilities(&self) -> RerankerCapabilities;

    /// Rerank search results based on query context.
    ///
    /// Modifies scores in place and re-sorts the results by score descending.
    ///
    /// # Arguments
    /// * `query` - The search query
    /// * `results` - Mutable slice of search results to rerank
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(RetrievalErr::RerankerError)` on failure
    async fn rerank(&self, query: &str, results: &mut [SearchResult]) -> Result<()>;

    /// Synchronous rerank for contexts without async runtime.
    ///
    /// Default implementation uses tokio's blocking mechanism.
    fn rerank_sync(&self, query: &str, results: &mut [SearchResult]) -> Result<()>
    where
        Self: Sized,
    {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.rerank(query, results))
        })
    }
}
