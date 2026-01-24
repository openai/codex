//! High-level retrieval facade with builder pattern.
//!
//! Provides a unified API for code retrieval following the Facade pattern.
//! Use `FacadeBuilder` for custom configuration or `for_workdir()` for standard usage.
//!
//! ## Usage
//!
//! ```ignore
//! use codex_retrieval::{RetrievalFacade, FacadeBuilder, RetrievalFeatures};
//!
//! // Standard: Get facade for current working directory (cached)
//! let facade = RetrievalFacade::for_workdir(&cwd).await?;
//!
//! // Custom: Builder pattern
//! let facade = FacadeBuilder::new(config)
//!     .features(RetrievalFeatures::all())
//!     .workspace("/path/to/project")
//!     .build()
//!     .await?;
//!
//! // Primary API
//! let results = facade.search("function definition").await?;
//! let repomap = facade.generate_repomap(request).await?;
//!
//! // Advanced API - use SearchRequest for more control
//! use codex_retrieval::SearchRequest;
//! let search_svc = facade.search_service();
//! search_svc.execute(SearchRequest::new("query").bm25().limit(10)).await?;
//! ```

use std::num::NonZeroUsize;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_utils_cache::BlockingLruCache;
use once_cell::sync::Lazy;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::chunking::supported_languages_info;
use crate::config::RetrievalConfig;
use crate::context::RetrievalContext;
use crate::context::RetrievalFeatures;
use crate::error::Result;
use crate::error::RetrievalErr;
use crate::indexing::FilterSummary;
use crate::indexing::IndexProgress;
use crate::indexing::RebuildMode;
use crate::repomap::RepoMapGenerator;
use crate::repomap::RepoMapRequest;
use crate::repomap::RepoMapResult;
use crate::repomap::TokenBudgeter;
use crate::services::IndexService;
use crate::services::RecentFilesService;
use crate::services::SearchService;
use crate::storage::SqliteStore;
use crate::traits::EmbeddingProvider;
use crate::types::SearchOutput;

/// Maximum number of cached facade instances.
const MAX_CACHED_INSTANCES: usize = 16;

/// Global facade instance cache by workdir with LRU eviction.
static INSTANCES: Lazy<BlockingLruCache<PathBuf, Arc<RetrievalFacade>>> = Lazy::new(|| {
    BlockingLruCache::new(NonZeroUsize::new(MAX_CACHED_INSTANCES).expect("capacity > 0"))
});

// ============================================================================
// Builder
// ============================================================================

/// Builder for constructing RetrievalFacade instances.
///
/// Provides a fluent API for configuring facade construction with optional
/// features, workspace root, and embedding provider.
///
/// ## Example
///
/// ```ignore
/// let facade = FacadeBuilder::new(config)
///     .features(RetrievalFeatures::all())
///     .workspace("/path/to/project")
///     .embedding_provider(my_embedder)
///     .build()
///     .await?;
/// ```
#[derive(Debug)]
pub struct FacadeBuilder {
    config: RetrievalConfig,
    features: RetrievalFeatures,
    workspace_root: Option<PathBuf>,
    embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
}

impl FacadeBuilder {
    /// Create a new builder with the given configuration.
    pub fn new(config: RetrievalConfig) -> Self {
        Self {
            config,
            features: RetrievalFeatures::default(),
            workspace_root: None,
            embedding_provider: None,
        }
    }

    /// Set feature flags.
    pub fn features(mut self, features: RetrievalFeatures) -> Self {
        self.features = features;
        self
    }

    /// Set workspace root directory.
    ///
    /// Required for file watching and indexing operations.
    /// If not set, defaults to `config.data_dir`.
    pub fn workspace(mut self, path: impl Into<PathBuf>) -> Self {
        self.workspace_root = Some(path.into());
        self
    }

    /// Set embedding provider for vector search.
    ///
    /// When set, enables semantic vector search in addition to BM25.
    pub fn embedding_provider(mut self, provider: Arc<dyn EmbeddingProvider>) -> Self {
        self.embedding_provider = Some(provider);
        self
    }

    /// Build the facade.
    pub async fn build(self) -> Result<RetrievalFacade> {
        let workspace_root = self
            .workspace_root
            .unwrap_or_else(|| self.config.data_dir.clone());

        // Create shared context
        let ctx =
            Arc::new(RetrievalContext::new(self.config, self.features, workspace_root).await?);

        // Create services
        let recent = Arc::new(RecentFilesService::new(Arc::clone(&ctx)));
        let index = Arc::new(IndexService::new(Arc::clone(&ctx)));
        let search = Arc::new(
            SearchService::new(
                Arc::clone(&ctx),
                Arc::clone(&recent),
                Arc::clone(&index),
                self.embedding_provider,
            )
            .await?,
        );

        Ok(RetrievalFacade {
            ctx,
            search,
            index,
            recent,
        })
    }
}

// ============================================================================
// Facade
// ============================================================================

/// High-level retrieval facade.
///
/// Follows the Facade design pattern to provide a simplified API for code
/// retrieval operations. The facade hides the complexity of the underlying
/// services while providing direct access when needed.
///
/// ## API Design
///
/// - **Primary API**: Direct methods like `search()`, `build_index()`, `generate_repomap()`
/// - **Service Access**: Methods like `search_service()` for advanced use cases
///
/// ## Thread Safety
///
/// All services are wrapped in `Arc` and are safe to use from multiple tasks.
pub struct RetrievalFacade {
    /// Shared context with config, db, etc.
    ctx: Arc<RetrievalContext>,
    /// Search service.
    search: Arc<SearchService>,
    /// Index management service.
    index: Arc<IndexService>,
    /// Recent files service.
    recent: Arc<RecentFilesService>,
}

impl std::fmt::Debug for RetrievalFacade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetrievalFacade")
            .field("workspace_root", &self.ctx.workspace_root())
            .field("features", self.ctx.features())
            .finish()
    }
}

impl RetrievalFacade {
    // ========================================================================
    // Factory Methods
    // ========================================================================

    /// Get or create a cached facade for the given working directory.
    ///
    /// Loads configuration from retrieval.toml files:
    /// 1. `{workdir}/.codex/retrieval.toml` (project-level)
    /// 2. `~/.codex/retrieval.toml` (global)
    ///
    /// Returns `NotEnabled` error if retrieval is not configured/enabled.
    /// Instances are cached by canonicalized workdir path (LRU, max 16).
    pub async fn for_workdir(workdir: &Path) -> Result<Arc<Self>> {
        // Canonicalize path for cache key
        let canonical = workdir
            .canonicalize()
            .unwrap_or_else(|_| workdir.to_path_buf());

        // Try to get from cache
        if let Some(facade) = INSTANCES.get(&canonical) {
            return Ok(facade);
        }

        // Load config
        let config = RetrievalConfig::load(workdir)?;

        // Check if enabled
        if !config.enabled {
            return Err(RetrievalErr::NotEnabled);
        }

        // Create new facade with standard features
        let facade = Arc::new(
            FacadeBuilder::new(config)
                .features(RetrievalFeatures::STANDARD)
                .workspace(canonical.clone())
                .build()
                .await?,
        );

        // Cache the instance
        INSTANCES.insert(canonical, Arc::clone(&facade));

        tracing::info!(
            workdir = ?workdir,
            languages = %supported_languages_info(),
            "RetrievalFacade initialized"
        );
        Ok(facade)
    }

    /// Check if retrieval is configured (without initializing).
    pub fn is_configured(workdir: &Path) -> bool {
        RetrievalConfig::load(workdir)
            .map(|c| c.enabled)
            .unwrap_or(false)
    }

    // ========================================================================
    // Primary API - Simple facade methods for common operations
    // ========================================================================

    // --- Search Operations ---

    /// Search for code matching the query.
    ///
    /// Uses hybrid search (BM25 + vector + recency) with query rewriting.
    /// Returns `NotReady` error if the index is not initialized.
    ///
    /// # Example
    /// ```ignore
    /// let results = facade.search("authentication handler").await?;
    /// for result in results.results {
    ///     println!("{}:{}", result.chunk.filepath, result.chunk.start_line);
    /// }
    /// ```
    pub async fn search(&self, query: &str) -> Result<SearchOutput> {
        self.search.execute(query).await
    }

    // --- Index Operations ---

    /// Start the unified indexing pipeline.
    ///
    /// Must be called before `trigger_session_start()`.
    pub async fn start_pipeline(&self) -> Result<()> {
        self.index.start_pipeline().await
    }

    /// Stop the unified pipeline.
    pub async fn stop_pipeline(&self) {
        self.index.stop_pipeline().await
    }

    /// Build or rebuild the index.
    ///
    /// # Arguments
    /// * `mode` - `RebuildMode::Incremental` or `RebuildMode::Clean`
    /// * `cancel` - Cancellation token to abort the operation
    pub async fn build_index(
        &self,
        mode: RebuildMode,
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<IndexProgress>> {
        self.index.build_index(mode, cancel).await
    }

    // --- RepoMap Operations ---

    /// Generate a repository map.
    ///
    /// Creates a condensed view of the codebase structure using PageRank.
    pub async fn generate_repomap(&self, request: RepoMapRequest) -> Result<RepoMapResult> {
        let repo_map_config = self.ctx.config().repo_map.clone().unwrap_or_default();

        // Use shared components from context
        let generator = RepoMapGenerator::new_with_shared(
            repo_map_config,
            self.ctx.db(),
            self.ctx.budgeter(),
            self.ctx.workspace_root().to_path_buf(),
        )?;

        let mut result = generator.generate(&request).await?;
        // Add filter info so LLM knows what files are indexed
        result.filter = self.ctx.filter_summary();
        Ok(result)
    }

    // ========================================================================
    // Service Accessors - For advanced use cases
    // ========================================================================

    /// Get the search service for advanced operations.
    ///
    /// Use this when you need `SearchRequest` for explicit mode/limit control,
    /// or access to `warmup()`, `rewrite_query()`, `has_vector_search()`.
    pub fn search_service(&self) -> Arc<SearchService> {
        Arc::clone(&self.search)
    }

    /// Get the index service for advanced operations.
    ///
    /// Use this when you need detailed coordinator access or custom
    /// pipeline control, such as `trigger_session_start()` or `get_status()`.
    pub fn index_service(&self) -> Arc<IndexService> {
        Arc::clone(&self.index)
    }

    /// Get the recent files service for advanced operations.
    ///
    /// Use this for file access notifications and recent file queries.
    pub fn recent_service(&self) -> Arc<RecentFilesService> {
        Arc::clone(&self.recent)
    }

    // ========================================================================
    // Shared State Accessors
    // ========================================================================

    /// Get configuration.
    pub fn config(&self) -> &RetrievalConfig {
        self.ctx.config()
    }

    /// Get feature flags.
    pub fn features(&self) -> &RetrievalFeatures {
        self.ctx.features()
    }

    /// Get workspace root path.
    pub fn workspace_root(&self) -> &Path {
        self.ctx.workspace_root()
    }

    /// Get shared SQLite store.
    pub fn db(&self) -> Arc<SqliteStore> {
        self.ctx.db()
    }

    /// Get shared token budgeter.
    pub fn budgeter(&self) -> Arc<TokenBudgeter> {
        self.ctx.budgeter()
    }

    /// Get filter summary for event emission.
    pub fn filter_summary(&self) -> Option<FilterSummary> {
        self.ctx.filter_summary()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_builder_basic() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::with_code_search();
        let facade = FacadeBuilder::new(config)
            .features(features)
            .build()
            .await
            .unwrap();

        assert!(facade.features().code_search);
        assert!(!facade.features().vector_search);
    }

    #[tokio::test]
    async fn test_builder_with_workspace() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::with_code_search();
        let facade = FacadeBuilder::new(config)
            .features(features)
            .workspace(dir.path().to_path_buf())
            .build()
            .await
            .unwrap();

        assert_eq!(facade.workspace_root(), dir.path());
    }

    #[tokio::test]
    async fn test_facade_search_disabled_returns_empty() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::none();
        let facade = FacadeBuilder::new(config)
            .features(features)
            .build()
            .await
            .unwrap();

        let results = facade.search("test query").await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_service_accessors_return_arc() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::with_code_search();
        let facade = FacadeBuilder::new(config)
            .features(features)
            .build()
            .await
            .unwrap();

        // Verify all service accessors work and return Arc
        let _search: Arc<SearchService> = facade.search_service();
        let _index: Arc<IndexService> = facade.index_service();
        let _recent: Arc<RecentFilesService> = facade.recent_service();
    }

    #[test]
    fn test_is_configured_false() {
        let dir = TempDir::new().unwrap();
        // Create a config file with enabled = false
        let config_dir = dir.path().join(".codex");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("retrieval.toml"), "enabled = false").unwrap();
        // Should return false when explicitly disabled
        assert!(!RetrievalFacade::is_configured(dir.path()));
    }
}
