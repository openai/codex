//! Local neural reranker using fastembed-rs (ONNX Runtime).
//!
//! Provides high-quality semantic reranking using Cross-Encoder models.
//! Models are downloaded on first use and cached locally.
//!
//! ## Supported Models
//!
//! - `bge-reranker-base`: BAAI BGE reranker (default, 278M params)
//! - `jina-reranker-v2`: Jina AI multilingual reranker
//!
//! ## Example
//!
//! ```toml
//! [retrieval.extended_reranker]
//! backend = "local"
//!
//! [retrieval.extended_reranker.local]
//! model = "bge-reranker-base"
//! batch_size = 32
//! ```

use std::cmp::Ordering;

use async_trait::async_trait;
use fastembed::RerankInitOptions;
use fastembed::RerankResult;
use fastembed::RerankerModel;
use fastembed::TextRerank;

use crate::config::LocalRerankerConfig;
use crate::error::Result;
use crate::error::RetrievalErr;
use crate::types::SearchResult;

use super::Reranker;
use super::RerankerCapabilities;

/// Local neural reranker using fastembed-rs.
#[derive(Debug)]
pub struct LocalReranker {
    model: TextRerank,
    model_name: String,
    batch_size: i32,
}

impl LocalReranker {
    /// Create a new local reranker from config.
    pub fn new(config: &LocalRerankerConfig) -> Result<Self> {
        let model_enum = Self::parse_model_name(&config.model);

        let mut options = RerankInitOptions::new(model_enum)
            .with_show_download_progress(config.show_download_progress);

        if let Some(ref cache_dir) = config.cache_dir {
            options = options.with_cache_dir(cache_dir.clone());
        }

        let model = TextRerank::try_new(options).map_err(|e| RetrievalErr::RerankerError {
            provider: "local".to_string(),
            cause: format!("Failed to initialize model '{}': {}", config.model, e),
        })?;

        Ok(Self {
            model,
            model_name: config.model.clone(),
            batch_size: config.batch_size,
        })
    }

    /// Parse model name string to fastembed enum.
    fn parse_model_name(name: &str) -> RerankerModel {
        match name.to_lowercase().as_str() {
            "bge-reranker-base" | "bgererankerbase" => RerankerModel::BGERerankerBase,
            "bge-reranker-v2-m3" | "bgererankerv2m3" => RerankerModel::BGERerankerV2M3,
            "jina-reranker-v2" | "jinarerankerv2basemultilingual" => {
                RerankerModel::JINARerankerV2BaseMultiligual
            }
            "jina-reranker-v1-turbo" | "jinarerankerv1turboen" => {
                RerankerModel::JINARerankerV1TurboEn
            }
            // Default to BGE base
            _ => RerankerModel::BGERerankerBase,
        }
    }

    /// Perform synchronous reranking (fastembed is sync internally).
    fn rerank_internal(&self, query: &str, results: &mut [SearchResult]) -> Result<()> {
        if results.is_empty() {
            return Ok(());
        }

        // Extract document contents
        let documents: Vec<&str> = results.iter().map(|r| r.chunk.content.as_str()).collect();

        // Call fastembed rerank (returns sorted results)
        let ranked: Vec<RerankResult> =
            self.model
                .rerank(query, documents, true, None)
                .map_err(|e| RetrievalErr::RerankerError {
                    provider: "local".to_string(),
                    cause: format!("Reranking failed: {}", e),
                })?;

        // Update scores from reranking results
        for item in &ranked {
            if item.index < results.len() {
                results[item.index].score = item.score as f32;
            }
        }

        // Re-sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));

        Ok(())
    }
}

#[async_trait]
impl Reranker for LocalReranker {
    fn name(&self) -> &str {
        &self.model_name
    }

    fn capabilities(&self) -> RerankerCapabilities {
        RerankerCapabilities {
            requires_network: false, // Models are cached locally after first download
            supports_batch: true,
            max_batch_size: Some(self.batch_size),
            is_async: false, // fastembed is synchronous internally
        }
    }

    async fn rerank(&self, query: &str, results: &mut [SearchResult]) -> Result<()> {
        // fastembed is CPU-bound, run in blocking task to not block async runtime
        // Since we can't easily move &mut results across threads, we do it in place
        // This is acceptable because reranking is typically fast (~10-100ms)
        self.rerank_internal(query, results)
    }
}

#[cfg(test)]
mod tests {
    // Tests require the neural-reranker feature and model download
    // Run with: cargo test -p codex-retrieval --features neural-reranker
}
