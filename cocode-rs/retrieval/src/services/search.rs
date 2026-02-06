//! Search service.
//!
//! Provides code search functionality with BM25, vector similarity,
//! and hybrid search modes. Integrates with recent files for temporal
//! relevance boosting.
//!
//! ## Usage
//!
//! ```ignore
//! use cocode_retrieval::SearchRequest;
//!
//! // Simple search (hybrid mode, default limit)
//! let results = service.execute("authentication handler").await?;
//!
//! // With options
//! let results = service.execute(
//!     SearchRequest::new("auth handler")
//!         .bm25()
//!         .limit(20)
//! ).await?;
//! ```

use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::context::RetrievalContext;
use crate::error::Result;
use crate::error::RetrievalErr;
use crate::event_emitter;
use crate::events::RetrievalEvent;
use crate::events::SearchMode;
use crate::events::SearchResultSummary;
use crate::query::rewriter::QueryRewriter;
use crate::query::rewriter::RewrittenQuery;
use crate::query::rewriter::SimpleRewriter;
use crate::search::HybridSearcher;
use crate::services::IndexService;
use crate::services::RecentFilesService;
use crate::traits::EmbeddingProvider;
use crate::types::SearchOutput;

// ============================================================================
// SearchRequest
// ============================================================================

/// Unified search request with builder pattern.
///
/// Provides a type-safe way to construct search queries with various options.
/// All search operations go through this single type.
///
/// ## Example
///
/// ```ignore
/// // Simple query (hybrid mode)
/// let req = SearchRequest::new("auth handler");
///
/// // With mode and limit
/// let req = SearchRequest::new("auth handler")
///     .bm25()
///     .limit(20);
///
/// // Using mode() directly
/// let req = SearchRequest::new("query")
///     .mode(SearchMode::Vector)
///     .limit(10);
/// ```
#[derive(Debug, Clone, Default)]
pub struct SearchRequest {
    /// Query text.
    pub query: String,
    /// Search mode (hybrid, bm25, vector, snippet).
    pub mode: SearchMode,
    /// Maximum results (None = use config default).
    pub limit: Option<i32>,
}

impl SearchRequest {
    /// Create a new search request with hybrid mode (default).
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            mode: SearchMode::Hybrid,
            limit: None,
        }
    }

    /// Set search mode.
    pub fn mode(mut self, mode: SearchMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set result limit.
    pub fn limit(mut self, limit: i32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set BM25 mode (keyword search only).
    pub fn bm25(self) -> Self {
        self.mode(SearchMode::Bm25)
    }

    /// Set vector mode (semantic similarity only).
    pub fn vector(self) -> Self {
        self.mode(SearchMode::Vector)
    }

    /// Set hybrid mode (BM25 + vector + recency).
    pub fn hybrid(self) -> Self {
        self.mode(SearchMode::Hybrid)
    }

    /// Set snippet mode (for code completion context).
    pub fn snippet(self) -> Self {
        self.mode(SearchMode::Snippet)
    }
}

// Enable: service.execute("query").await?
impl From<&str> for SearchRequest {
    fn from(query: &str) -> Self {
        Self::new(query)
    }
}

impl From<String> for SearchRequest {
    fn from(query: String) -> Self {
        Self::new(query)
    }
}

/// Generate a unique query ID using timestamp and counter.
fn generate_query_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() % 1_000_000)
        .unwrap_or(0);
    format!("q-{ts:06}-{count}")
}

/// Service for code search operations.
///
/// Provides hybrid search (BM25 + vector), BM25-only, and vector-only
/// search modes. Integrates with `RecentFilesService` for temporal
/// relevance boosting.
pub struct SearchService {
    /// Shared context with config, features, etc.
    ctx: Arc<RetrievalContext>,
    /// Hybrid searcher instance.
    searcher: HybridSearcher,
    /// Query rewriter (translation, expansion).
    rewriter: Arc<dyn QueryRewriter>,
    /// Recent files service for temporal relevance.
    recent_files: Arc<RecentFilesService>,
    /// Index service for readiness checks.
    index: Arc<IndexService>,
}

impl std::fmt::Debug for SearchService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchService")
            .field("features", self.ctx.features())
            .finish()
    }
}

impl SearchService {
    /// Create a new search service.
    ///
    /// # Arguments
    /// * `ctx` - Shared retrieval context
    /// * `recent_files` - Recent files service for temporal boost
    /// * `index` - Index service for readiness checks
    /// * `embedding_provider` - Optional embedding provider for vector search
    pub async fn new(
        ctx: Arc<RetrievalContext>,
        recent_files: Arc<RecentFilesService>,
        index: Arc<IndexService>,
        embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
    ) -> Result<Self> {
        let max_chunks_per_file = ctx.config().search.max_chunks_per_file as usize;

        // Create searcher with or without embeddings
        let mut searcher = if let Some(provider) = embedding_provider {
            HybridSearcher::with_embeddings(ctx.vector_store(), provider)
        } else {
            HybridSearcher::new(ctx.vector_store())
        };

        searcher = searcher
            .with_max_chunks_per_file(max_chunks_per_file)
            .with_reranker_config(&ctx.config().reranker);

        // Enable hydration with workspace root
        searcher = searcher.with_workspace_root(ctx.workspace_root().to_path_buf());

        // Use simple rewriter by default (query rewriting enabled based on features)
        let rewriter: Arc<dyn QueryRewriter> = if ctx.features().query_rewrite {
            Arc::new(SimpleRewriter::new().with_expansion(true))
        } else {
            Arc::new(SimpleRewriter::new())
        };

        Ok(Self {
            ctx,
            searcher,
            rewriter,
            recent_files,
            index,
        })
    }

    /// Create with a custom query rewriter.
    pub fn with_rewriter(mut self, rewriter: Arc<dyn QueryRewriter>) -> Self {
        self.rewriter = rewriter;
        self
    }

    // ========== Primary Search API ==========

    /// Execute a search with the given request.
    ///
    /// This is the primary search method. All search operations go through this.
    ///
    /// # Example
    /// ```ignore
    /// // Simple query (hybrid mode, default limit)
    /// let results = service.execute("auth handler").await?;
    ///
    /// // With options
    /// let results = service.execute(
    ///     SearchRequest::new("auth handler")
    ///         .bm25()
    ///         .limit(20)
    /// ).await?;
    /// ```
    pub async fn execute(&self, request: impl Into<SearchRequest>) -> Result<SearchOutput> {
        let req = request.into();
        let limit = req.limit.unwrap_or(self.ctx.config().search.n_final);

        match req.mode {
            SearchMode::Hybrid | SearchMode::Snippet => {
                self.execute_hybrid(&req.query, limit).await
            }
            SearchMode::Bm25 => self.execute_bm25(&req.query, limit).await,
            SearchMode::Vector => self.execute_vector(&req.query, limit).await,
        }
    }

    // ========== Internal Search Implementations ==========

    /// Execute hybrid search (BM25 + vector + recency).
    async fn execute_hybrid(&self, query: &str, limit: i32) -> Result<SearchOutput> {
        let start_time = std::time::Instant::now();
        let query_id = generate_query_id();

        if !self.ctx.features().has_search() {
            return Ok(self.empty_output());
        }

        // Check index readiness
        if !self.index.is_search_ready().await {
            let workspace = self.ctx.workspace_name().to_string();
            return Err(RetrievalErr::NotReady {
                workspace,
                reason: "Index not ready".to_string(),
            });
        }

        self.emit_search_started(&query_id, query, SearchMode::Hybrid, limit);

        // Apply query rewriting with full event emission
        let rewrite_start = std::time::Instant::now();
        let effective_query = if self.ctx.features().query_rewrite {
            let rewritten = self.rewriter.rewrite(query).await?;
            tracing::debug!(
                original = %query,
                rewritten = %rewritten.rewritten,
                translated = rewritten.was_translated,
                "Query rewritten"
            );

            event_emitter::emit(RetrievalEvent::QueryRewritten {
                query_id: query_id.clone(),
                original: query.to_string(),
                rewritten: rewritten.rewritten.clone(),
                expansions: rewritten
                    .expansions
                    .iter()
                    .map(|x| x.text.clone())
                    .collect(),
                translated: rewritten.was_translated,
                duration_ms: rewrite_start.elapsed().as_millis() as i64,
            });

            rewritten.effective_query()
        } else {
            query.to_string()
        };

        // Get recently accessed files for temporal relevance boost
        let recent_results = self
            .recent_files
            .get_recent_search_results(limit as usize)
            .await;

        // Perform search with hydration and recent files boost
        let results = self
            .searcher
            .search_hydrated_with_recent(&effective_query, limit, &recent_results)
            .await;

        self.emit_and_wrap_results(results, query_id, start_time)
    }

    /// Execute BM25 full-text search only.
    async fn execute_bm25(&self, query: &str, limit: i32) -> Result<SearchOutput> {
        let start_time = std::time::Instant::now();
        let query_id = generate_query_id();

        if !self.ctx.features().code_search {
            return Ok(self.empty_output());
        }

        self.emit_search_started(&query_id, query, SearchMode::Bm25, limit);

        let effective_query = self.rewrite_query_simple(query).await?;
        let results = self.searcher.search_bm25(&effective_query, limit).await;

        self.emit_and_wrap_results(results, query_id, start_time)
    }

    /// Execute vector similarity search only.
    async fn execute_vector(&self, query: &str, limit: i32) -> Result<SearchOutput> {
        let start_time = std::time::Instant::now();
        let query_id = generate_query_id();

        if !self.has_vector_search() {
            return Ok(self.empty_output());
        }

        self.emit_search_started(&query_id, query, SearchMode::Vector, limit);

        let effective_query = self.rewrite_query_simple(query).await?;
        let results = self
            .searcher
            .search_vector_only(&effective_query, limit)
            .await;

        self.emit_and_wrap_results(results, query_id, start_time)
    }

    // ========== Helper Methods ==========

    /// Create an empty search output.
    fn empty_output(&self) -> SearchOutput {
        SearchOutput {
            results: Vec::new(),
            filter: self.ctx.filter_summary(),
        }
    }

    /// Emit SearchStarted event.
    fn emit_search_started(&self, query_id: &str, query: &str, mode: SearchMode, limit: i32) {
        event_emitter::emit(RetrievalEvent::SearchStarted {
            query_id: query_id.to_string(),
            query: query.to_string(),
            mode,
            limit,
        });
    }

    /// Apply query rewriting without event emission (for non-hybrid modes).
    async fn rewrite_query_simple(&self, query: &str) -> Result<String> {
        if self.ctx.features().query_rewrite {
            Ok(self.rewriter.rewrite(query).await?.effective_query())
        } else {
            Ok(query.to_string())
        }
    }

    /// Emit completion/error event and wrap results.
    fn emit_and_wrap_results(
        &self,
        results: Result<Vec<crate::types::SearchResult>>,
        query_id: String,
        start_time: std::time::Instant,
    ) -> Result<SearchOutput> {
        let duration_ms = start_time.elapsed().as_millis() as i64;
        let filter = self.ctx.filter_summary();

        match &results {
            Ok(results) => {
                event_emitter::emit(RetrievalEvent::SearchCompleted {
                    query_id,
                    results: results
                        .iter()
                        .map(|r| SearchResultSummary::from(r.clone()))
                        .collect(),
                    total_duration_ms: duration_ms,
                    filter: filter.clone(),
                });
            }
            Err(e) => {
                event_emitter::emit(RetrievalEvent::SearchError {
                    query_id,
                    error: e.to_string(),
                    retryable: e.is_retryable(),
                });
            }
        }

        results.map(|r| SearchOutput { results: r, filter })
    }

    // ========== Query Processing ==========

    /// Rewrite a query without searching.
    ///
    /// Returns None if query rewriting is disabled.
    pub async fn rewrite_query(&self, query: &str) -> Option<Result<RewrittenQuery>> {
        if self.ctx.features().query_rewrite {
            Some(self.rewriter.rewrite(query).await)
        } else {
            None
        }
    }

    /// Pre-warm the BM25 index for faster first search.
    pub async fn warmup(&self) -> Result<()> {
        if let Some(bm25) = self.searcher.bm25_searcher() {
            // Ensure workspace root is set so BM25 can read content from files
            bm25.set_workspace_root(self.ctx.workspace_root().to_path_buf());
            bm25.warmup().await?;
            tracing::info!("BM25 index pre-warmed");
        }
        Ok(())
    }

    // ========== Configuration ==========

    /// Check if vector search is available.
    pub fn has_vector_search(&self) -> bool {
        self.ctx.features().vector_search && self.searcher.has_vector_search()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RetrievalConfig;
    use crate::context::RetrievalFeatures;
    use tempfile::TempDir;

    async fn create_test_services(
        dir: &TempDir,
    ) -> (
        Arc<RetrievalContext>,
        Arc<RecentFilesService>,
        Arc<IndexService>,
    ) {
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::MINIMAL;
        let ctx = Arc::new(
            RetrievalContext::new(config, features, dir.path().to_path_buf())
                .await
                .unwrap(),
        );

        let recent = Arc::new(RecentFilesService::new(Arc::clone(&ctx)));
        let index = Arc::new(IndexService::new(Arc::clone(&ctx)));

        (ctx, recent, index)
    }

    #[tokio::test]
    async fn test_search_service_creation() {
        let dir = TempDir::new().unwrap();
        let (ctx, recent, index) = create_test_services(&dir).await;

        let _service = SearchService::new(ctx, recent, index, None).await.unwrap();
    }

    #[tokio::test]
    async fn test_search_disabled_returns_empty() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::NONE;
        let ctx = Arc::new(
            RetrievalContext::new(config, features, dir.path().to_path_buf())
                .await
                .unwrap(),
        );

        let recent = Arc::new(RecentFilesService::new(Arc::clone(&ctx)));
        let index = Arc::new(IndexService::new(Arc::clone(&ctx)));

        let service = SearchService::new(ctx, recent, index, None).await.unwrap();
        // Test using new execute() API
        let results = service.execute("test query").await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_rewrite_query_disabled_returns_none() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        // code_search enabled but query_rewrite disabled
        let features = RetrievalFeatures::MINIMAL;
        let ctx = Arc::new(
            RetrievalContext::new(config, features, dir.path().to_path_buf())
                .await
                .unwrap(),
        );

        let recent = Arc::new(RecentFilesService::new(Arc::clone(&ctx)));
        let index = Arc::new(IndexService::new(Arc::clone(&ctx)));

        let service = SearchService::new(ctx, recent, index, None).await.unwrap();
        assert!(service.rewrite_query("test").await.is_none());
    }

    #[tokio::test]
    async fn test_has_vector_search() {
        let dir = TempDir::new().unwrap();
        let (ctx, recent, index) = create_test_services(&dir).await;

        let service = SearchService::new(ctx, recent, index, None).await.unwrap();
        // Vector search disabled by default (no embeddings configured)
        assert!(!service.has_vector_search());
    }

    // ========== SearchRequest Tests ==========

    #[test]
    fn test_search_request_builder() {
        let req = SearchRequest::new("test query");
        assert_eq!(req.query, "test query");
        assert!(matches!(req.mode, SearchMode::Hybrid));
        assert!(req.limit.is_none());

        let req = SearchRequest::new("test").bm25().limit(20);
        assert!(matches!(req.mode, SearchMode::Bm25));
        assert_eq!(req.limit, Some(20));

        let req = SearchRequest::new("test").vector().limit(10);
        assert!(matches!(req.mode, SearchMode::Vector));
        assert_eq!(req.limit, Some(10));
    }

    #[test]
    fn test_search_request_from_str() {
        let req: SearchRequest = "test query".into();
        assert_eq!(req.query, "test query");
        assert!(matches!(req.mode, SearchMode::Hybrid));
    }

    #[test]
    fn test_search_request_from_string() {
        let req: SearchRequest = String::from("test query").into();
        assert_eq!(req.query, "test query");
        assert!(matches!(req.mode, SearchMode::Hybrid));
    }
}
