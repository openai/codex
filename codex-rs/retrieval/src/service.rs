//! High-level retrieval service.
//!
//! Provides a unified API for code retrieval with feature flags.
//!
//! ## Configuration
//!
//! Retrieval has its own independent configuration system:
//! - Project-level: `.codex/retrieval.toml`
//! - Global: `~/.codex/retrieval.toml`
//!
//! ## Usage
//!
//! ```ignore
//! use codex_retrieval::RetrievalService;
//!
//! // Get service for current working directory (loads config automatically)
//! let service = RetrievalService::for_workdir(&cwd).await?;
//! let results = service.search("function definition").await?;
//! ```

use std::num::NonZeroUsize;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_utils_cache::BlockingLruCache;
use once_cell::sync::Lazy;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::chunking::CodeChunkerService;
use crate::chunking::supported_languages_info;
use crate::config::RetrievalConfig;
use crate::error::Result;
use crate::error::RetrievalErr;
use crate::event_emitter;
use crate::events::RetrievalEvent;
use crate::events::SearchMode;
use crate::events::SearchResultSummary;
use crate::indexing::FileWatcher;
use crate::indexing::IndexManager;
use crate::indexing::IndexProgress;
use crate::indexing::IndexStats;
use crate::indexing::RebuildMode;
use crate::indexing::WatchEvent;
use crate::query::rewriter::QueryRewriter;
use crate::query::rewriter::RewrittenQuery;
use crate::query::rewriter::SimpleRewriter;
use crate::repomap::RepoMapRequest;
use crate::repomap::RepoMapResult;
use crate::repomap::RepoMapService;
use crate::search::HybridSearcher;
use crate::search::RecentFilesCache;
use crate::storage::SqliteStore;
use crate::storage::lancedb::LanceDbStore;
use crate::traits::EmbeddingProvider;
use crate::types::CodeChunk;
use crate::types::ScoreType;
use crate::types::SearchResult;

/// Maximum number of cached RetrievalService instances.
/// Prevents unbounded memory growth in long-running processes.
const MAX_CACHED_SERVICES: usize = 16;

/// Generate a unique query ID using timestamp and counter.
fn generate_query_id() -> String {
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() % 1_000_000)
        .unwrap_or(0);
    format!("q-{ts:06}-{count}")
}

/// Global service instance cache by workdir with LRU eviction.
static INSTANCES: Lazy<BlockingLruCache<PathBuf, Arc<RetrievalService>>> = Lazy::new(|| {
    BlockingLruCache::new(NonZeroUsize::new(MAX_CACHED_SERVICES).expect("capacity > 0"))
});

/// Feature flags for retrieval system.
#[derive(Debug, Clone, Copy, Default)]
pub struct RetrievalFeatures {
    /// Enable BM25 full-text search (basic code search).
    pub code_search: bool,
    /// Enable vector similarity search.
    pub vector_search: bool,
    /// Enable query rewriting (CN/EN translation, expansion).
    pub query_rewrite: bool,
}

impl RetrievalFeatures {
    /// Create with all features disabled.
    pub fn none() -> Self {
        Self::default()
    }

    /// Create with code search enabled.
    pub fn with_code_search() -> Self {
        Self {
            code_search: true,
            ..Default::default()
        }
    }

    /// Enable all features.
    pub fn all() -> Self {
        Self {
            code_search: true,
            vector_search: true,
            query_rewrite: true,
        }
    }

    /// Check if any search feature is enabled.
    pub fn has_search(&self) -> bool {
        self.code_search || self.vector_search
    }
}

/// Default capacity for recent files cache.
const DEFAULT_RECENT_FILES_CAPACITY: usize = 50;

/// High-level retrieval service.
///
/// Integrates search, query rewriting, and embedding providers.
pub struct RetrievalService {
    config: RetrievalConfig,
    features: RetrievalFeatures,
    searcher: HybridSearcher,
    rewriter: Arc<dyn QueryRewriter>,
    /// LRU cache for recently accessed files (temporal relevance signal).
    recent_files: RwLock<RecentFilesCache>,
    /// Workspace root for hydrating content from files.
    workspace_root: Option<PathBuf>,
}

impl RetrievalService {
    /// Get or create a RetrievalService for the given working directory.
    ///
    /// Loads configuration from retrieval.toml files:
    /// 1. `{workdir}/.codex/retrieval.toml` (project-level)
    /// 2. `~/.codex/retrieval.toml` (global)
    ///
    /// Returns `NotEnabled` error if retrieval is not configured/enabled.
    /// Instances are cached by canonicalized workdir path.
    pub async fn for_workdir(workdir: &Path) -> Result<Arc<Self>> {
        // Canonicalize path for cache key
        let canonical = workdir
            .canonicalize()
            .unwrap_or_else(|_| workdir.to_path_buf());

        // Try to get from cache (LRU cache with bounded capacity)
        if let Some(service) = INSTANCES.get(&canonical) {
            return Ok(service);
        }

        // Load config
        let config = RetrievalConfig::load(workdir)?;

        // Check if enabled
        if !config.enabled {
            return Err(RetrievalErr::NotEnabled);
        }

        // Create new service with default features
        let features = RetrievalFeatures {
            code_search: true,
            query_rewrite: true,
            ..Default::default()
        };

        let service = Arc::new(Self::with_workspace(config, features, canonical.clone()).await?);

        // Cache the instance (LRU eviction handles memory bounds)
        INSTANCES.insert(canonical, Arc::clone(&service));

        tracing::info!(
            workdir = ?workdir,
            languages = %supported_languages_info(),
            "RetrievalService initialized"
        );
        Ok(service)
    }

    /// Check if retrieval is configured (without initializing).
    pub fn is_configured(workdir: &Path) -> bool {
        RetrievalConfig::load(workdir)
            .map(|c| c.enabled)
            .unwrap_or(false)
    }

    /// Create a new retrieval service with BM25-only search.
    pub async fn new(config: RetrievalConfig, features: RetrievalFeatures) -> Result<Self> {
        Self::with_workspace(config, features, None).await
    }

    /// Create a new retrieval service with workspace root for content hydration.
    ///
    /// When workspace_root is set, search results return fresh file content
    /// instead of stale indexed content.
    pub async fn with_workspace(
        config: RetrievalConfig,
        features: RetrievalFeatures,
        workspace_root: impl Into<Option<PathBuf>>,
    ) -> Result<Self> {
        let workspace_root = workspace_root.into();
        let store = Arc::new(LanceDbStore::open(&config.data_dir).await?);
        let max_chunks_per_file = config.search.max_chunks_per_file as usize;
        let mut searcher = HybridSearcher::new(store)
            .with_max_chunks_per_file(max_chunks_per_file)
            .with_reranker_config(&config.reranker);

        // Enable hydration if workspace_root is set
        if let Some(ref root) = workspace_root {
            searcher = searcher.with_workspace_root(root.clone());
        }

        let rewriter: Arc<dyn QueryRewriter> = Arc::new(SimpleRewriter::new());
        let recent_files = RwLock::new(RecentFilesCache::new(DEFAULT_RECENT_FILES_CAPACITY));

        Ok(Self {
            config,
            features,
            searcher,
            rewriter,
            recent_files,
            workspace_root,
        })
    }

    /// Create with an embedding provider for vector search.
    pub async fn with_embeddings(
        config: RetrievalConfig,
        features: RetrievalFeatures,
        provider: Arc<dyn EmbeddingProvider>,
    ) -> Result<Self> {
        Self::with_embeddings_and_workspace(config, features, provider, None).await
    }

    /// Create with an embedding provider and workspace root.
    pub async fn with_embeddings_and_workspace(
        config: RetrievalConfig,
        features: RetrievalFeatures,
        provider: Arc<dyn EmbeddingProvider>,
        workspace_root: impl Into<Option<PathBuf>>,
    ) -> Result<Self> {
        let workspace_root = workspace_root.into();
        let store = Arc::new(LanceDbStore::open(&config.data_dir).await?);
        let max_chunks_per_file = config.search.max_chunks_per_file as usize;
        let mut searcher = HybridSearcher::with_embeddings(store, provider)
            .with_max_chunks_per_file(max_chunks_per_file)
            .with_reranker_config(&config.reranker);

        // Enable hydration if workspace_root is set
        if let Some(ref root) = workspace_root {
            searcher = searcher.with_workspace_root(root.clone());
        }

        let rewriter: Arc<dyn QueryRewriter> = Arc::new(SimpleRewriter::new().with_expansion(true));
        let recent_files = RwLock::new(RecentFilesCache::new(DEFAULT_RECENT_FILES_CAPACITY));

        Ok(Self {
            config,
            features,
            searcher,
            rewriter,
            recent_files,
            workspace_root,
        })
    }

    /// Set a custom query rewriter.
    pub fn with_rewriter(mut self, rewriter: Arc<dyn QueryRewriter>) -> Self {
        self.rewriter = rewriter;
        self
    }

    /// Search for code matching the query.
    ///
    /// Applies query rewriting if enabled, then performs hybrid search.
    ///
    /// # Arguments
    /// * `query` - Search query string
    /// * `limit` - Maximum number of results (if None, uses config.search.n_final)
    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        self.search_with_limit(query, None).await
    }

    /// Search with explicit limit parameter.
    pub async fn search_with_limit(
        &self,
        query: &str,
        limit: Option<i32>,
    ) -> Result<Vec<SearchResult>> {
        let start_time = std::time::Instant::now();
        let query_id = generate_query_id();
        let limit = limit.unwrap_or(self.config.search.n_final);

        if !self.features.has_search() {
            return Ok(Vec::new());
        }

        // Emit search started event
        event_emitter::emit(RetrievalEvent::SearchStarted {
            query_id: query_id.clone(),
            query: query.to_string(),
            mode: SearchMode::Hybrid,
            limit,
        });

        // Apply query rewriting if enabled
        let rewrite_start = std::time::Instant::now();
        let effective_query = if self.features.query_rewrite {
            let rewritten = self.rewriter.rewrite(query).await?;
            tracing::debug!(
                original = %query,
                rewritten = %rewritten.rewritten,
                translated = rewritten.was_translated,
                "Query rewritten"
            );

            // Emit query rewritten event
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
        let recent_results = self.get_recent_search_results(limit as usize).await;

        // Perform search with hydration and recent files boost
        let results = self
            .searcher
            .search_hydrated_with_recent(&effective_query, limit, &recent_results)
            .await;

        // Emit search completed or error event
        let duration_ms = start_time.elapsed().as_millis() as i64;
        match &results {
            Ok(results) => {
                event_emitter::emit(RetrievalEvent::SearchCompleted {
                    query_id,
                    results: results
                        .iter()
                        .map(|r| SearchResultSummary::from(r.clone()))
                        .collect(),
                    total_duration_ms: duration_ms,
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

        results
    }

    /// Get SearchResults from recently accessed files for RRF fusion.
    ///
    /// Reads and chunks files on demand to ensure fresh content.
    async fn get_recent_search_results(&self, limit: usize) -> Vec<SearchResult> {
        let paths = self.recent_files.read().await.get_recent_paths(limit);

        if paths.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        for (rank, path) in paths.iter().enumerate() {
            match self.chunk_file(path).await {
                Ok(chunks) => {
                    for chunk in chunks {
                        results.push(SearchResult {
                            chunk,
                            // Score based on recency rank (most recent = highest)
                            score: 1.0 / (rank as f32 + 1.0),
                            score_type: ScoreType::Recent,
                            is_stale: None,
                        });
                    }
                }
                Err(e) => {
                    tracing::debug!(path = ?path, error = %e, "Failed to chunk recent file");
                }
            }
        }

        results
    }

    /// Search using BM25 full-text search only.
    ///
    /// Unlike `search()`, this bypasses vector search and RRF fusion.
    pub async fn search_bm25(&self, query: &str, limit: i32) -> Result<Vec<SearchResult>> {
        let start_time = std::time::Instant::now();
        let query_id = generate_query_id();

        if !self.features.code_search {
            return Ok(Vec::new());
        }

        // Emit search started event
        event_emitter::emit(RetrievalEvent::SearchStarted {
            query_id: query_id.clone(),
            query: query.to_string(),
            mode: SearchMode::Bm25,
            limit,
        });

        // Apply query rewriting if enabled
        let effective_query = if self.features.query_rewrite {
            self.rewriter.rewrite(query).await?.effective_query()
        } else {
            query.to_string()
        };

        let results = self.searcher.search_bm25(&effective_query, limit).await;

        // Emit completion event
        let duration_ms = start_time.elapsed().as_millis() as i64;
        match &results {
            Ok(results) => {
                event_emitter::emit(RetrievalEvent::SearchCompleted {
                    query_id,
                    results: results
                        .iter()
                        .map(|r| SearchResultSummary::from(r.clone()))
                        .collect(),
                    total_duration_ms: duration_ms,
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

        results
    }

    /// Search using vector similarity only.
    ///
    /// Returns empty results if embeddings are not configured.
    pub async fn search_vector(&self, query: &str, limit: i32) -> Result<Vec<SearchResult>> {
        let start_time = std::time::Instant::now();
        let query_id = generate_query_id();

        if !self.has_vector_search() {
            return Ok(Vec::new());
        }

        // Emit search started event
        event_emitter::emit(RetrievalEvent::SearchStarted {
            query_id: query_id.clone(),
            query: query.to_string(),
            mode: SearchMode::Vector,
            limit,
        });

        // Apply query rewriting if enabled
        let effective_query = if self.features.query_rewrite {
            self.rewriter.rewrite(query).await?.effective_query()
        } else {
            query.to_string()
        };

        let results = self
            .searcher
            .search_vector_only(&effective_query, limit)
            .await;

        // Emit completion event
        let duration_ms = start_time.elapsed().as_millis() as i64;
        match &results {
            Ok(results) => {
                event_emitter::emit(RetrievalEvent::SearchCompleted {
                    query_id,
                    results: results
                        .iter()
                        .map(|r| SearchResultSummary::from(r.clone()))
                        .collect(),
                    total_duration_ms: duration_ms,
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

        results
    }

    /// Rewrite a query without searching.
    ///
    /// Returns None if query rewriting is disabled.
    pub async fn rewrite_query(&self, query: &str) -> Option<Result<RewrittenQuery>> {
        if self.features.query_rewrite {
            Some(self.rewriter.rewrite(query).await)
        } else {
            None
        }
    }

    /// Get current features.
    pub fn features(&self) -> &RetrievalFeatures {
        &self.features
    }

    /// Get configuration.
    pub fn config(&self) -> &RetrievalConfig {
        &self.config
    }

    /// Get workspace root (if set).
    pub fn workspace_root(&self) -> Option<&Path> {
        self.workspace_root.as_deref()
    }

    /// Check if vector search is available.
    pub fn has_vector_search(&self) -> bool {
        self.features.vector_search && self.searcher.has_vector_search()
    }

    // ========== Recent Files API ==========

    /// Notify that a file has been accessed or edited.
    ///
    /// This updates the LRU cache for temporal relevance in search results.
    /// Recently accessed files will be boosted in search ranking.
    ///
    /// Note: Only the path is stored. Content is read fresh on demand during
    /// search to avoid consistency issues with stale cached content.
    pub async fn notify_file_accessed(&self, path: impl AsRef<Path>) {
        self.recent_files.write().await.notify_file_accessed(path);
    }

    /// Remove a file from the recent files cache.
    ///
    /// Call this when a file is closed or deleted.
    pub async fn remove_recent_file(&self, path: impl AsRef<Path>) {
        self.recent_files.write().await.remove(path);
    }

    /// Get paths of recently accessed files.
    ///
    /// Returns up to `limit` file paths, ordered by most recently accessed first.
    pub async fn get_recent_paths(&self, limit: usize) -> Vec<PathBuf> {
        self.recent_files.read().await.get_recent_paths(limit)
    }

    /// Get chunks from recently accessed files.
    ///
    /// Reads files from disk and chunks them on demand to ensure fresh content.
    /// Used internally for RRF fusion with temporal relevance signal.
    pub async fn get_recent_chunks(&self, limit: usize) -> Vec<CodeChunk> {
        let paths = self.recent_files.read().await.get_recent_paths(limit);
        let mut all_chunks = Vec::new();

        for path in paths {
            match self.chunk_file(&path).await {
                Ok(chunks) => all_chunks.extend(chunks),
                Err(e) => {
                    tracing::debug!(path = ?path, error = %e, "Failed to chunk recent file");
                }
            }
        }

        all_chunks
    }

    /// Clear all recent files from the cache.
    pub async fn clear_recent_files(&self) {
        self.recent_files.write().await.clear();
    }

    /// Check if a file is in the recent files cache.
    pub async fn is_recent_file(&self, path: impl AsRef<Path>) -> bool {
        self.recent_files.read().await.contains(path)
    }

    /// Get the number of files in the recent files cache.
    pub async fn recent_files_count(&self) -> usize {
        self.recent_files.read().await.len()
    }

    // ========== Operations API ==========
    // These methods provide a unified interface for indexing, watching, and repomap
    // operations. They are used by both CLI and TUI to avoid code duplication.

    /// Build or rebuild the index for the workspace.
    ///
    /// # Arguments
    /// * `mode` - `RebuildMode::Incremental` (default) or `RebuildMode::Clean`
    /// * `cancel` - Cancellation token to abort the operation
    ///
    /// # Returns
    /// A channel receiver that yields `IndexProgress` updates.
    ///
    /// # Example
    /// ```ignore
    /// let cancel = CancellationToken::new();
    /// let mut rx = service.build_index(RebuildMode::Incremental, cancel).await?;
    /// while let Some(progress) = rx.recv().await {
    ///     println!("{}: {}", progress.status, progress.description);
    /// }
    /// ```
    pub async fn build_index(
        &self,
        mode: RebuildMode,
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<IndexProgress>> {
        let workdir = self
            .workspace_root
            .as_ref()
            .ok_or_else(|| RetrievalErr::NotEnabled)?;

        let workspace = workdir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("default")
            .to_string();

        // Ensure data directory exists
        std::fs::create_dir_all(&self.config.data_dir)?;

        let db_path = self.config.data_dir.join("retrieval.db");
        let store = Arc::new(SqliteStore::open(&db_path)?);

        let mut manager = IndexManager::new(self.config.clone(), store);
        let workdir = workdir.clone();

        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Index build cancelled by user");
                    let _ = tx.send(IndexProgress::failed("Cancelled by user")).await;
                }
                result = manager.rebuild(&workspace, &workdir, mode) => {
                    match result {
                        Ok(mut progress_rx) => {
                            while let Some(progress) = progress_rx.recv().await {
                                if tx.send(progress).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Index rebuild failed: {}", e);
                            let _ = tx.send(IndexProgress::failed(e.to_string())).await;
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Get index status and statistics.
    ///
    /// Returns information about the index including file count, chunk count,
    /// and last indexing time.
    pub async fn get_index_status(&self) -> Result<IndexStats> {
        let workdir = self
            .workspace_root
            .as_ref()
            .ok_or_else(|| RetrievalErr::NotEnabled)?;

        let workspace = workdir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("default")
            .to_string();

        let db_path = self.config.data_dir.join("retrieval.db");
        if !db_path.exists() {
            return Ok(IndexStats::default());
        }

        let store = Arc::new(SqliteStore::open(&db_path)?);
        let manager = IndexManager::new(self.config.clone(), store);
        manager.get_stats(&workspace).await
    }

    /// Start file watcher for incremental index updates.
    ///
    /// # Arguments
    /// * `cancel` - Cancellation token to stop watching
    ///
    /// # Returns
    /// A channel receiver that yields `WatchEvent` updates.
    pub async fn start_watch(
        &self,
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<WatchEvent>> {
        let workdir = self
            .workspace_root
            .as_ref()
            .ok_or_else(|| RetrievalErr::NotEnabled)?;

        let workspace = workdir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("default")
            .to_string();

        let debounce_ms = self.config.indexing.watch_debounce_ms.max(0) as u64;
        let watcher = FileWatcher::new(workdir, debounce_ms)?;

        // Emit watch started event
        event_emitter::emit(RetrievalEvent::WatchStarted {
            workspace: workspace.clone(),
            paths: vec![workdir.display().to_string()],
        });

        let (tx, rx) = mpsc::channel(100);
        let config = self.config.clone();
        let workdir = workdir.clone();

        tokio::spawn(async move {
            let db_path = config.data_dir.join("retrieval.db");
            let store = match SqliteStore::open(&db_path) {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    tracing::error!("Failed to open database for watcher: {}", e);
                    event_emitter::emit(RetrievalEvent::WatchStopped {
                        workspace: workspace.clone(),
                    });
                    return;
                }
            };

            let mut manager = IndexManager::new(config.clone(), store);

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        tracing::info!("File watcher cancelled");
                        break;
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                        if let Some(events) = watcher.recv_timeout(std::time::Duration::from_millis(100)) {
                            for event in &events {
                                let watch_event = WatchEvent {
                                    path: event.path.clone(),
                                    kind: event.kind.clone(),
                                };
                                let _ = tx.send(watch_event).await;
                            }

                            if !events.is_empty() {
                                // Trigger incremental rebuild
                                if let Err(e) = manager
                                    .rebuild(&workspace, &workdir, RebuildMode::Incremental)
                                    .await
                                {
                                    tracing::error!("Incremental rebuild failed: {}", e);
                                }
                            }
                        }
                    }
                }
            }

            event_emitter::emit(RetrievalEvent::WatchStopped { workspace });
        });

        Ok(rx)
    }

    /// Generate a repository map.
    ///
    /// Creates a condensed representation of the codebase structure
    /// using PageRank to prioritize important files and symbols.
    ///
    /// # Arguments
    /// * `request` - Request parameters including max_tokens and chat_files
    pub async fn generate_repomap(&self, request: RepoMapRequest) -> Result<RepoMapResult> {
        let workdir = self
            .workspace_root
            .as_ref()
            .ok_or_else(|| RetrievalErr::NotEnabled)?;

        let workdir_name = workdir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let db_path = self.config.data_dir.join("retrieval.db");
        if !db_path.exists() {
            return Err(RetrievalErr::NotReady {
                workspace: workdir_name.to_string(),
                reason: "Index not found - please build the index first".to_string(),
            });
        }

        let store = Arc::new(SqliteStore::open(&db_path)?);
        let repo_map_config = self.config.repo_map.clone().unwrap_or_default();
        let mut repomap_service = RepoMapService::new(repo_map_config, store, workdir.clone())?;

        repomap_service.generate(&request, false).await
    }

    /// Internal: read and chunk a file.
    ///
    /// Returns empty vec if file is not readable or not a supported language.
    async fn chunk_file(&self, path: &Path) -> Result<Vec<CodeChunk>> {
        // Read file content
        let content = match tokio::fs::read_to_string(path).await {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!(path = ?path, error = %e, "Failed to read file for chunking");
                return Ok(Vec::new());
            }
        };

        // Get file extension for language detection
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt")
            .to_string();

        // Get filepath string
        let filepath_str = path.to_string_lossy().to_string();

        // Chunking config
        let max_tokens = self.config.chunking.max_tokens as usize;
        let overlap_tokens = self.config.chunking.overlap_tokens as usize;

        // Clone extension for use in closure (original is used later for CodeChunk.language)
        let ext_for_chunk = extension.clone();

        // Run CPU-intensive chunking in a blocking thread pool
        let spans = match tokio::task::spawn_blocking(move || {
            let chunker = CodeChunkerService::new(max_tokens, overlap_tokens);
            chunker.chunk(&content, &ext_for_chunk)
        })
        .await
        {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                tracing::debug!(path = ?path, error = %e, "Failed to chunk file");
                return Ok(Vec::new());
            }
            Err(e) => {
                tracing::warn!(path = ?path, error = %e, "Chunking task panicked");
                return Ok(Vec::new());
            }
        };

        // Convert spans to CodeChunks
        let workspace = "recent"; // Mark as from recent files
        let chunks: Vec<CodeChunk> = spans
            .into_iter()
            .enumerate()
            .map(|(i, span)| CodeChunk {
                id: format!("{}:{}:{}", workspace, filepath_str, i),
                source_id: workspace.to_string(),
                filepath: filepath_str.clone(),
                language: extension.to_string(),
                content: span.content,
                start_line: span.start_line,
                end_line: span.end_line,
                embedding: None,
                modified_time: None,
                workspace: workspace.to_string(),
                content_hash: String::new(),
                indexed_at: 0,
                parent_symbol: None, // TODO: Extract from TagExtractor
                is_overview: span.is_overview,
            })
            .collect();

        Ok(chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_service_creation() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::with_code_search();
        let service = RetrievalService::new(config, features).await.unwrap();

        assert!(service.features().code_search);
        assert!(!service.features().vector_search);
    }

    #[tokio::test]
    async fn test_search_disabled_returns_empty() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::none();
        let service = RetrievalService::new(config, features).await.unwrap();

        let results = service.search("test query").await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_rewrite_query_disabled_returns_none() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::with_code_search();
        let service = RetrievalService::new(config, features).await.unwrap();

        assert!(service.rewrite_query("test").await.is_none());
    }

    #[tokio::test]
    async fn test_rewrite_query_enabled() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures {
            code_search: true,
            query_rewrite: true,
            ..Default::default()
        };
        let service = RetrievalService::new(config, features).await.unwrap();

        let result = service.rewrite_query("test function").await;
        assert!(result.is_some());
        let rewritten = result.unwrap().unwrap();
        assert_eq!(rewritten.original, "test function");
    }

    #[test]
    fn test_features_none() {
        let features = RetrievalFeatures::none();
        assert!(!features.code_search);
        assert!(!features.vector_search);
        assert!(!features.query_rewrite);
        assert!(!features.has_search());
    }

    #[test]
    fn test_features_all() {
        let features = RetrievalFeatures::all();
        assert!(features.code_search);
        assert!(features.vector_search);
        assert!(features.query_rewrite);
        assert!(features.has_search());
    }

    // ========== Recent Files Tests ==========

    #[tokio::test]
    async fn test_recent_files_empty_on_creation() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::with_code_search();
        let service = RetrievalService::new(config, features).await.unwrap();

        assert_eq!(service.recent_files_count().await, 0);
    }

    #[tokio::test]
    async fn test_notify_file_accessed() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::with_code_search();
        let service = RetrievalService::new(config, features).await.unwrap();

        let path = Path::new("src/main.rs");
        service.notify_file_accessed(path).await;

        assert!(service.is_recent_file(path).await);
        assert_eq!(service.recent_files_count().await, 1);
    }

    #[tokio::test]
    async fn test_get_recent_paths() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::with_code_search();
        let service = RetrievalService::new(config, features).await.unwrap();

        service.notify_file_accessed("a.rs").await;
        service.notify_file_accessed("b.rs").await;
        service.notify_file_accessed("c.rs").await;

        let paths = service.get_recent_paths(10).await;
        assert_eq!(paths.len(), 3);
        // Most recent first
        assert_eq!(paths[0], PathBuf::from("c.rs"));
        assert_eq!(paths[1], PathBuf::from("b.rs"));
        assert_eq!(paths[2], PathBuf::from("a.rs"));
    }

    #[tokio::test]
    async fn test_get_recent_chunks() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::with_code_search();
        let service = RetrievalService::new(config, features).await.unwrap();

        // Create a temporary file
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}").unwrap();

        service.notify_file_accessed(&file_path).await;

        let chunks = service.get_recent_chunks(100).await;
        assert!(!chunks.is_empty());
        assert!(chunks[0].content.contains("fn main()"));
    }

    #[tokio::test]
    async fn test_remove_recent_file() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::with_code_search();
        let service = RetrievalService::new(config, features).await.unwrap();

        let path = Path::new("src/main.rs");
        service.notify_file_accessed(path).await;
        assert!(service.is_recent_file(path).await);

        service.remove_recent_file(path).await;
        assert!(!service.is_recent_file(path).await);
    }

    #[tokio::test]
    async fn test_clear_recent_files() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::with_code_search();
        let service = RetrievalService::new(config, features).await.unwrap();

        service.notify_file_accessed("a.rs").await;
        service.notify_file_accessed("b.rs").await;
        assert_eq!(service.recent_files_count().await, 2);

        service.clear_recent_files().await;
        assert_eq!(service.recent_files_count().await, 0);
    }

    #[tokio::test]
    async fn test_get_recent_chunks_nonexistent_file() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::with_code_search();
        let service = RetrievalService::new(config, features).await.unwrap();

        // Notify with non-existent file
        let path = Path::new("/nonexistent/file.rs");
        service.notify_file_accessed(path).await;

        // File is tracked
        assert!(service.is_recent_file(path).await);

        // But get_recent_chunks returns empty (file doesn't exist)
        let chunks = service.get_recent_chunks(100).await;
        assert!(chunks.is_empty());
    }
}
