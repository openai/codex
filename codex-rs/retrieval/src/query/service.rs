//! Query rewrite service.
//!
//! Orchestrates query rewriting using cache, LLM, and rule-based strategies.
//! Provides a unified interface for query transformation with fallback behavior.

use std::sync::Arc;
use std::time::Instant;

use crate::config::QueryRewriteConfig;
use crate::error::Result;
use crate::query::RewriteSource;
use crate::query::RewrittenQuery;
use crate::query::cache::RewriteCache;
use crate::query::llm_provider::CompletionRequest;
use crate::query::llm_provider::LlmProvider;
use crate::query::llm_provider::LlmRewriteResponse;
use crate::query::llm_provider::NoopProvider;
use crate::query::llm_provider::OpenAiProvider;
use crate::query::llm_provider::QUERY_REWRITE_SYSTEM_PROMPT;
use crate::query::ollama_provider::OllamaLlmProvider;
use crate::query::rewriter::SimpleRewriter;
use crate::storage::SqliteStore;

/// Query rewrite service.
///
/// Combines cache, LLM, and rule-based strategies for query transformation.
/// The fallback order is: Cache -> LLM -> Rule-based.
pub struct QueryRewriteService {
    config: QueryRewriteConfig,
    cache: Option<RewriteCache>,
    llm_provider: Arc<dyn LlmProvider>,
    rule_rewriter: SimpleRewriter,
}

impl QueryRewriteService {
    /// Create a new query rewrite service.
    pub async fn new(config: QueryRewriteConfig, db: Option<Arc<SqliteStore>>) -> Result<Self> {
        // Initialize cache if enabled and DB is available
        let cache = if config.cache.enabled {
            if let Some(db) = db {
                Some(RewriteCache::new(db, config.cache.clone()).await?)
            } else {
                None
            }
        } else {
            None
        };

        // Initialize LLM provider based on config
        let llm_provider: Arc<dyn LlmProvider> = if config.enabled {
            match config.llm.provider.as_str() {
                "openai" => Arc::new(OpenAiProvider::new(config.llm.clone())),
                "ollama" => Arc::new(OllamaLlmProvider::new(config.llm.clone())),
                "noop" | "none" | "disabled" => Arc::new(NoopProvider::new()),
                other => {
                    tracing::warn!(
                        provider = %other,
                        "Unknown LLM provider, falling back to noop"
                    );
                    Arc::new(NoopProvider::new())
                }
            }
        } else {
            Arc::new(NoopProvider::new())
        };

        // Initialize rule rewriter with config
        let mut rule_rewriter = SimpleRewriter::new()
            .with_expansion(config.features.expansion)
            .with_case_variants(config.features.case_variants)
            .with_translation(config.features.translation);

        // Add custom synonyms from config
        if !config.rules.synonyms.is_empty() {
            rule_rewriter = rule_rewriter.with_custom_synonyms(config.rules.synonyms.clone());
        }

        Ok(Self {
            config,
            cache,
            llm_provider,
            rule_rewriter,
        })
    }

    /// Create a service with a custom LLM provider.
    pub async fn with_provider(
        config: QueryRewriteConfig,
        provider: Arc<dyn LlmProvider>,
        db: Option<Arc<SqliteStore>>,
    ) -> Result<Self> {
        let cache = if config.cache.enabled {
            if let Some(db) = db {
                Some(RewriteCache::new(db, config.cache.clone()).await?)
            } else {
                None
            }
        } else {
            None
        };

        let mut rule_rewriter = SimpleRewriter::new()
            .with_expansion(config.features.expansion)
            .with_case_variants(config.features.case_variants)
            .with_translation(config.features.translation);

        if !config.rules.synonyms.is_empty() {
            rule_rewriter = rule_rewriter.with_custom_synonyms(config.rules.synonyms.clone());
        }

        Ok(Self {
            config,
            cache,
            llm_provider: provider,
            rule_rewriter,
        })
    }

    /// Create a minimal service for testing.
    pub fn minimal() -> Self {
        Self {
            config: QueryRewriteConfig::default(),
            cache: None,
            llm_provider: Arc::new(NoopProvider::new()),
            rule_rewriter: SimpleRewriter::new().with_expansion(true),
        }
    }

    /// Rewrite a query.
    ///
    /// Attempts strategies in order: Cache -> LLM -> Rule-based.
    pub async fn rewrite(&self, query: &str) -> Result<RewrittenQuery> {
        if !self.config.enabled {
            return Ok(RewrittenQuery::unchanged(query));
        }

        let start = Instant::now();

        // Try cache first
        if let Some(ref cache) = self.cache {
            if let Ok(Some(cached)) = cache.get(query).await {
                tracing::debug!(
                    query = %query,
                    "Query rewrite cache hit"
                );
                return Ok(cached.with_source(RewriteSource::Cache));
            }
        }

        // Try LLM if available, otherwise fall back to rules
        let result = if self.llm_provider.is_available() && self.config.features.translation {
            match self.rewrite_with_llm(query).await {
                Ok(result) => {
                    let latency_ms = start.elapsed().as_millis() as i64;
                    result.with_latency_ms(latency_ms)
                }
                Err(e) => {
                    tracing::warn!(
                        query = %query,
                        error = %e,
                        "LLM rewrite failed, falling back to rule-based"
                    );
                    // Mark as Fallback - these results should NOT be cached
                    // to allow LLM retry on next request
                    let latency_ms = start.elapsed().as_millis() as i64;
                    self.rewrite_with_rules(query)
                        .await?
                        .with_latency_ms(latency_ms)
                        .with_source(RewriteSource::Fallback)
                }
            }
        } else {
            let latency_ms = start.elapsed().as_millis() as i64;
            self.rewrite_with_rules(query)
                .await?
                .with_latency_ms(latency_ms)
        };

        // Cache the result, but skip Fallback results to allow LLM retry
        if let Some(ref cache) = self.cache {
            if result.source != RewriteSource::Fallback {
                let _ = cache.put(query, &result).await;
            } else {
                tracing::debug!(
                    query = %query,
                    "Skipping cache for fallback result to allow LLM retry"
                );
            }
        }

        Ok(result)
    }

    /// Rewrite using LLM.
    async fn rewrite_with_llm(&self, query: &str) -> Result<RewrittenQuery> {
        let request = CompletionRequest {
            system: QUERY_REWRITE_SYSTEM_PROMPT.to_string(),
            user: query.to_string(),
            max_tokens: self.config.llm.max_tokens,
            temperature: self.config.llm.temperature,
        };

        let start = Instant::now();
        let response = self.llm_provider.complete(&request).await?;
        let latency_ms = start.elapsed().as_millis() as i64;

        // Parse the LLM response
        let parsed = LlmRewriteResponse::parse(&response.content)?;
        Ok(parsed.to_rewritten_query(query, latency_ms))
    }

    /// Rewrite using rule-based strategy.
    async fn rewrite_with_rules(&self, query: &str) -> Result<RewrittenQuery> {
        use crate::query::QueryRewriter;
        self.rule_rewriter.rewrite(query).await
    }

    /// Get cache statistics.
    pub async fn cache_stats(&self) -> Option<crate::query::CacheStats> {
        if let Some(ref cache) = self.cache {
            cache.stats().await.ok()
        } else {
            None
        }
    }

    /// Check if the service is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Check if LLM is available.
    pub fn is_llm_available(&self) -> bool {
        self.llm_provider.is_available()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_minimal_service() {
        let service = QueryRewriteService::minimal();

        let result = service.rewrite("test function").await.unwrap();
        assert_eq!(result.original, "test function");
        // Should use rule-based since LLM is not available
        assert_eq!(result.source, RewriteSource::Rule);
    }

    #[tokio::test]
    async fn test_disabled_service() {
        let mut config = QueryRewriteConfig::default();
        config.enabled = false;

        let service = QueryRewriteService::new(config, None).await.unwrap();
        let result = service.rewrite("test").await.unwrap();

        assert_eq!(result.original, "test");
        assert_eq!(result.rewritten, "test");
    }

    #[tokio::test]
    async fn test_with_expansion() {
        let mut config = QueryRewriteConfig::default();
        config.features.expansion = true;
        config.features.case_variants = true;

        let service = QueryRewriteService::new(config, None).await.unwrap();
        let result = service.rewrite("find function handler").await.unwrap();

        // Should have expansions for "function"
        assert!(result.has_expansion("fn") || result.has_expansion("method"));
    }

    #[tokio::test]
    async fn test_custom_synonyms() {
        let mut config = QueryRewriteConfig::default();
        config.features.expansion = true;
        config.rules.synonyms.insert(
            "widget".to_string(),
            vec!["component".to_string(), "element".to_string()],
        );

        let service = QueryRewriteService::new(config, None).await.unwrap();
        let result = service.rewrite("find widget").await.unwrap();

        assert!(result.has_expansion("component") || result.has_expansion("element"));
    }

    #[tokio::test]
    async fn test_with_cache() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Arc::new(SqliteStore::open(&db_path).unwrap());

        let config = QueryRewriteConfig::default();
        let service = QueryRewriteService::new(config, Some(db)).await.unwrap();

        // First call
        let result1 = service.rewrite("test query").await.unwrap();
        assert_eq!(result1.source, RewriteSource::Rule);

        // Second call should hit cache
        let result2 = service.rewrite("test query").await.unwrap();
        // Note: cache source is set when retrieved
        assert_eq!(result2.source, RewriteSource::Cache);

        // Check cache stats
        let stats = service.cache_stats().await.unwrap();
        assert_eq!(stats.total_entries, 1);
        assert!(stats.total_hits >= 1);
    }
}
