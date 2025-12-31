//! Reranker registry and factory.
//!
//! Provides factory functions to create rerankers from configuration,
//! and the ChainReranker for combining multiple rerankers.

use std::sync::Arc;

use async_trait::async_trait;

use crate::config::ChainedRerankerConfig;
use crate::config::ExtendedRerankerConfig;
use crate::config::RerankerBackend;
use crate::config::RerankerConfig;
use crate::error::Result;
use crate::error::RetrievalErr;
use crate::types::SearchResult;

use super::Reranker;
use super::RerankerCapabilities;
use super::remote::RemoteReranker;
use super::rule_based::RuleBasedReranker;

#[cfg(feature = "neural-reranker")]
use super::local::LocalReranker;

/// Create a reranker from extended configuration.
///
/// Returns an `Arc<dyn Reranker>` that can be shared across threads.
///
/// # Arguments
/// * `config` - Extended reranker configuration
///
/// # Returns
/// * `Ok(Arc<dyn Reranker>)` - The created reranker
/// * `Err` - If the reranker could not be created
pub fn create_reranker(config: &ExtendedRerankerConfig) -> Result<Arc<dyn Reranker>> {
    match config.backend {
        RerankerBackend::RuleBased => {
            let reranker = RuleBasedReranker::with_config(config.rule_based.clone().into());
            Ok(Arc::new(reranker))
        }
        RerankerBackend::Local => {
            #[cfg(feature = "neural-reranker")]
            {
                let local_config =
                    config
                        .local
                        .as_ref()
                        .ok_or_else(|| RetrievalErr::ConfigError {
                            field: "extended_reranker.local".to_string(),
                            cause: "Local reranker config required when backend = 'local'"
                                .to_string(),
                        })?;
                let reranker = LocalReranker::new(local_config)?;
                Ok(Arc::new(reranker))
            }
            #[cfg(not(feature = "neural-reranker"))]
            {
                Err(RetrievalErr::FeatureNotEnabled(
                    "neural-reranker feature required for local reranking. \
                     Compile with --features neural-reranker"
                        .to_string(),
                ))
            }
        }
        RerankerBackend::Remote => {
            let remote_config =
                config
                    .remote
                    .as_ref()
                    .ok_or_else(|| RetrievalErr::ConfigError {
                        field: "extended_reranker.remote".to_string(),
                        cause: "Remote reranker config required when backend = 'remote'"
                            .to_string(),
                    })?;
            let reranker = RemoteReranker::new(remote_config)?;
            Ok(Arc::new(reranker))
        }
        RerankerBackend::Chain => {
            if config.chain.is_empty() {
                return Err(RetrievalErr::ConfigError {
                    field: "extended_reranker.chain".to_string(),
                    cause: "Chain reranker requires at least one stage".to_string(),
                });
            }

            let mut rerankers: Vec<Arc<dyn Reranker>> = Vec::new();
            for stage in &config.chain {
                let reranker = create_chained_reranker(stage)?;
                rerankers.push(reranker);
            }

            Ok(Arc::new(ChainReranker::new(rerankers)))
        }
    }
}

/// Create a reranker from a chained config entry.
fn create_chained_reranker(config: &ChainedRerankerConfig) -> Result<Arc<dyn Reranker>> {
    match config.backend {
        RerankerBackend::RuleBased => {
            let rule_config = config.rule_based.as_ref().cloned().unwrap_or_default();
            let reranker = RuleBasedReranker::with_config(rule_config.into());
            Ok(Arc::new(reranker))
        }
        RerankerBackend::Local => {
            #[cfg(feature = "neural-reranker")]
            {
                let local_config =
                    config
                        .local
                        .as_ref()
                        .ok_or_else(|| RetrievalErr::ConfigError {
                            field: "chain[].local".to_string(),
                            cause: "Local config required for chain stage with backend = 'local'"
                                .to_string(),
                        })?;
                let reranker = LocalReranker::new(local_config)?;
                Ok(Arc::new(reranker))
            }
            #[cfg(not(feature = "neural-reranker"))]
            {
                Err(RetrievalErr::FeatureNotEnabled(
                    "neural-reranker feature required for local reranking".to_string(),
                ))
            }
        }
        RerankerBackend::Remote => {
            let remote_config =
                config
                    .remote
                    .as_ref()
                    .ok_or_else(|| RetrievalErr::ConfigError {
                        field: "chain[].remote".to_string(),
                        cause: "Remote config required for chain stage with backend = 'remote'"
                            .to_string(),
                    })?;
            let reranker = RemoteReranker::new(remote_config)?;
            Ok(Arc::new(reranker))
        }
        RerankerBackend::Chain => Err(RetrievalErr::ConfigError {
            field: "chain[].backend".to_string(),
            cause: "Nested chain rerankers are not supported".to_string(),
        }),
    }
}

/// Create a simple rule-based reranker from legacy config.
///
/// This is a convenience function for backward compatibility.
pub fn create_rule_based_reranker(config: &RerankerConfig) -> Arc<dyn Reranker> {
    Arc::new(RuleBasedReranker::with_config(config.clone().into()))
}

// ============================================================================
// Chain Reranker
// ============================================================================

/// Chain reranker that combines multiple rerankers sequentially.
///
/// Results are passed through each reranker in order, with each
/// stage receiving the scores from the previous stage.
///
/// ## Example Use Cases
///
/// - Rule-based filtering followed by neural reranking
/// - Fast local model followed by high-quality API reranking for top results
#[derive(Debug)]
pub struct ChainReranker {
    rerankers: Vec<Arc<dyn Reranker>>,
}

impl ChainReranker {
    /// Create a new chain reranker with the given stages.
    pub fn new(rerankers: Vec<Arc<dyn Reranker>>) -> Self {
        Self { rerankers }
    }

    /// Get the number of stages in the chain.
    pub fn len(&self) -> usize {
        self.rerankers.len()
    }

    /// Check if the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.rerankers.is_empty()
    }
}

#[async_trait]
impl Reranker for ChainReranker {
    fn name(&self) -> &str {
        "chain"
    }

    fn capabilities(&self) -> RerankerCapabilities {
        // Aggregate capabilities from all stages
        let requires_network = self
            .rerankers
            .iter()
            .any(|r| r.capabilities().requires_network);
        let is_async = self.rerankers.iter().any(|r| r.capabilities().is_async);
        let min_batch_size = self
            .rerankers
            .iter()
            .filter_map(|r| r.capabilities().max_batch_size)
            .min();

        RerankerCapabilities {
            requires_network,
            supports_batch: true,
            max_batch_size: min_batch_size,
            is_async,
        }
    }

    async fn rerank(&self, query: &str, results: &mut [SearchResult]) -> Result<()> {
        for reranker in &self.rerankers {
            reranker.rerank(query, results).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RerankerConfig;

    #[test]
    fn test_create_rule_based_reranker() {
        let config = RerankerConfig::default();
        let reranker = create_rule_based_reranker(&config);
        assert_eq!(reranker.name(), "rule_based");
    }

    #[test]
    fn test_chain_reranker_capabilities() {
        let rule_config = RerankerConfig::default();
        let rule_reranker = create_rule_based_reranker(&rule_config);

        let chain = ChainReranker::new(vec![rule_reranker]);

        assert!(!chain.capabilities().requires_network);
        assert!(!chain.capabilities().is_async);
    }
}
