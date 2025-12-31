//! Index manager for batch indexing operations.
//!
//! Coordinates file walking, change detection, and tweakcc updates.
//! Supports optional embedding generation with caching.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use tokio::sync::mpsc;

/// Rebuild mode for indexing operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RebuildMode {
    /// Incremental: only process changed files (default)
    #[default]
    Incremental,
    /// Clean: delete all index data, then rebuild from scratch
    Clean,
}

use crate::chunking::CodeChunkerService;
use crate::chunking::OverviewConfig;
use crate::chunking::generate_overview_chunks;
use crate::config::RetrievalConfig;
use crate::embeddings::EmbeddingCache;
use crate::error::Result;
use crate::error::RetrievalErr;
use crate::event_emitter;
use crate::events::FileProcessStatus;
use crate::events::IndexPhaseInfo;
use crate::events::IndexStatsSummary;
use crate::events::RebuildModeInfo;
use crate::events::RetrievalEvent;
use crate::indexing::IndexLockGuard;
use crate::indexing::change_detector::ChangeDetector;
use crate::indexing::change_detector::ChangeStatus;
use crate::indexing::change_detector::get_mtime;
use crate::indexing::change_detector::hash_file;
use crate::indexing::progress::IndexProgress;
use crate::indexing::walker::FileWalker;
use crate::search::Bm25Searcher;
use crate::storage::SnippetStorage;
use crate::storage::SqliteStore;
use crate::storage::lancedb::LanceDbStore;
use crate::tags::SupportedLanguage;
use crate::tags::TagExtractor;
use crate::tags::get_parent_context;
use crate::traits::EmbeddingProvider;
use crate::types::CodeChunk;
use crate::types::compute_chunk_hash;
use crate::types::detect_language;

/// Index manager for coordinating indexing operations.
///
/// Supports two modes:
/// - Basic: Only indexes metadata (catalog, snippets) - use `new()`
/// - With Embeddings: Also computes embeddings and stores to LanceDB - use `with_embeddings()`
#[allow(dead_code)]
pub struct IndexManager {
    config: RetrievalConfig,
    db: Arc<SqliteStore>,
    change_detector: ChangeDetector,
    snippet_storage: SnippetStorage,
    chunker: CodeChunkerService,
    // Optional embedding components (None for basic mode)
    lancedb: Option<Arc<LanceDbStore>>,
    cache: Option<EmbeddingCache>,
    provider: Option<Arc<dyn EmbeddingProvider>>,
    /// Custom BM25 searcher with tunable k1/b parameters.
    /// When set, chunks are indexed for BM25 search during indexing.
    bm25_searcher: Option<Arc<Bm25Searcher>>,
}

impl IndexManager {
    /// Create a new index manager (basic mode, no embeddings).
    pub fn new(config: RetrievalConfig, db: Arc<SqliteStore>) -> Self {
        let change_detector = ChangeDetector::new(db.clone());
        let snippet_storage = SnippetStorage::new(db.clone());
        let chunker = Self::create_chunker(&config);

        Self {
            config,
            db,
            change_detector,
            snippet_storage,
            chunker,
            lancedb: None,
            cache: None,
            provider: None,
            bm25_searcher: None,
        }
    }

    /// Create an index manager with embedding support.
    ///
    /// This mode will:
    /// - Compute embeddings for code chunks using the provider
    /// - Cache embeddings to avoid recomputing unchanged chunks
    /// - Store chunks with embeddings to LanceDB for vector search
    /// - Index chunks for BM25 search with tunable k1/b parameters
    ///
    /// # Arguments
    /// * `config` - Retrieval configuration
    /// * `db` - SQLite store for metadata
    /// * `lancedb` - LanceDB store for vector storage
    /// * `provider` - Embedding provider (e.g., OpenAI)
    /// * `cache_path` - Path to embedding cache SQLite file
    /// * `artifact_id` - Embedding model identifier for cache isolation
    pub fn with_embeddings(
        config: RetrievalConfig,
        db: Arc<SqliteStore>,
        lancedb: Arc<LanceDbStore>,
        provider: Arc<dyn EmbeddingProvider>,
        cache_path: &Path,
        artifact_id: &str,
    ) -> Result<Self> {
        let cache = EmbeddingCache::open(cache_path, artifact_id)?;
        let change_detector = ChangeDetector::new(db.clone());
        let snippet_storage = SnippetStorage::new(db.clone());
        let chunker = Self::create_chunker(&config);
        // Create BM25 searcher with config-tuned k1/b parameters
        let bm25_searcher = Arc::new(Bm25Searcher::with_config(lancedb.clone(), &config.search));

        Ok(Self {
            config,
            db,
            change_detector,
            snippet_storage,
            chunker,
            lancedb: Some(lancedb),
            cache: Some(cache),
            provider: Some(provider),
            bm25_searcher: Some(bm25_searcher),
        })
    }

    /// Check if embedding mode is enabled.
    pub fn has_embeddings(&self) -> bool {
        self.lancedb.is_some() && self.cache.is_some() && self.provider.is_some()
    }

    /// Get the BM25 searcher if available.
    ///
    /// Returns the BM25 searcher that can be used with HybridSearcher.
    pub fn bm25_searcher(&self) -> Option<Arc<Bm25Searcher>> {
        self.bm25_searcher.clone()
    }

    /// Create chunker based on config.
    fn create_chunker(config: &RetrievalConfig) -> CodeChunkerService {
        CodeChunkerService::new(
            config.chunking.max_tokens as usize,
            config.chunking.overlap_tokens as usize,
        )
    }

    /// Index a workspace directory.
    ///
    /// Returns a stream of progress updates.
    pub async fn index_workspace(
        &mut self,
        workspace: &str,
        root: &Path,
    ) -> Result<mpsc::Receiver<IndexProgress>> {
        let (tx, rx) = mpsc::channel(100);

        // Acquire lock
        let lock = IndexLockGuard::try_acquire(
            self.db.clone(),
            workspace,
            std::time::Duration::from_secs(self.config.indexing.lock_timeout_secs as u64),
        )
        .await?;

        // Clone what we need for the async task
        let workspace = workspace.to_string();
        let root = root.to_path_buf();
        let config = self.config.clone();
        let change_detector = ChangeDetector::new(self.db.clone());
        let snippet_storage = SnippetStorage::new(self.db.clone());
        let chunker = Self::create_chunker(&config);

        // Clone optional embedding components
        let lancedb = self.lancedb.clone();
        let provider = self.provider.clone();
        let bm25_searcher = self.bm25_searcher.clone();
        // Note: cache is behind Mutex, need to clone the path and artifact_id for re-creation
        let cache_path = config.data_dir.join("embeddings.db");
        let artifact_id = config
            .embedding
            .as_ref()
            .map(|e| e.model.clone())
            .unwrap_or_else(|| "default".to_string());
        let has_embeddings = self.has_embeddings();

        tokio::spawn(async move {
            let result = Self::run_indexing(
                &workspace,
                &root,
                &config,
                &change_detector,
                &snippet_storage,
                &chunker,
                &lock,
                tx.clone(),
                lancedb.as_ref(),
                provider.as_ref(),
                if has_embeddings {
                    Some((&cache_path, artifact_id.as_str()))
                } else {
                    None
                },
                bm25_searcher.as_ref(),
            )
            .await;

            if let Err(e) = result {
                // Emit failure event
                event_emitter::emit(RetrievalEvent::IndexBuildFailed {
                    workspace: workspace.clone(),
                    error: e.to_string(),
                });

                let _ = tx
                    .send(IndexProgress::failed(format!("Indexing failed: {e}")))
                    .await;
            }
        });

        Ok(rx)
    }

    /// Run the indexing process.
    #[allow(clippy::too_many_arguments)]
    async fn run_indexing(
        workspace: &str,
        root: &Path,
        config: &RetrievalConfig,
        change_detector: &ChangeDetector,
        snippet_storage: &SnippetStorage,
        chunker: &CodeChunkerService,
        lock: &IndexLockGuard,
        tx: mpsc::Sender<IndexProgress>,
        // Optional embedding components
        lancedb: Option<&Arc<LanceDbStore>>,
        provider: Option<&Arc<dyn EmbeddingProvider>>,
        cache_info: Option<(&Path, &str)>, // (cache_path, artifact_id)
        // Optional BM25 searcher for custom BM25 indexing
        bm25_searcher: Option<&Arc<Bm25Searcher>>,
    ) -> Result<()> {
        // Create cache if embedding mode is enabled
        let cache = if let Some((cache_path, artifact_id)) = cache_info {
            Some(EmbeddingCache::open(cache_path, artifact_id)?)
        } else {
            None
        };

        let index_start = Instant::now();

        // Emit index build started event
        event_emitter::emit(RetrievalEvent::IndexBuildStarted {
            workspace: workspace.to_string(),
            mode: RebuildModeInfo::Incremental, // TODO: pass mode through
            estimated_files: 0,                 // Will update after scan
        });

        // Phase 1: Walk files
        let _ = tx.send(IndexProgress::loading("Scanning files...")).await;

        event_emitter::emit(RetrievalEvent::IndexPhaseChanged {
            workspace: workspace.to_string(),
            phase: IndexPhaseInfo::Scanning,
            progress: 0.0,
            description: "Scanning files...".to_string(),
        });

        let walker = FileWalker::new(config.indexing.max_file_size_mb);
        let files = walker.walk(root)?;
        let total_files = files.len();

        let _ = tx
            .send(IndexProgress::indexing(
                0.0,
                format!("Found {total_files} files"),
            ))
            .await;

        // Phase 2: Compute hashes for all files
        let _ = tx
            .send(IndexProgress::indexing(0.05, "Computing file hashes..."))
            .await;

        event_emitter::emit(RetrievalEvent::IndexPhaseChanged {
            workspace: workspace.to_string(),
            phase: IndexPhaseInfo::Hashing,
            progress: 0.05,
            description: format!("Computing hashes for {} files...", files.len()),
        });

        let mut current_files = HashMap::new();
        for file in &files {
            if let Ok(hash) = hash_file(file) {
                let rel_path = file
                    .strip_prefix(root)
                    .unwrap_or(file)
                    .to_string_lossy()
                    .to_string();
                current_files.insert(rel_path, hash);
            }
        }

        // Phase 3: Detect changes
        let _ = tx
            .send(IndexProgress::indexing(0.1, "Detecting changes..."))
            .await;

        event_emitter::emit(RetrievalEvent::IndexPhaseChanged {
            workspace: workspace.to_string(),
            phase: IndexPhaseInfo::Detecting,
            progress: 0.1,
            description: "Detecting changes...".to_string(),
        });

        let changes = change_detector
            .detect_changes(workspace, &current_files)
            .await?;

        let added = changes
            .iter()
            .filter(|c| c.status == ChangeStatus::Added)
            .count();
        let modified = changes
            .iter()
            .filter(|c| c.status == ChangeStatus::Modified)
            .count();
        let deleted = changes
            .iter()
            .filter(|c| c.status == ChangeStatus::Deleted)
            .count();

        let _ = tx
            .send(IndexProgress::indexing(
                0.15,
                format!("Changes: {added} added, {modified} modified, {deleted} deleted"),
            ))
            .await;

        // Phase 3.5: Check chunk limit before processing
        // Estimate new chunks based on file count (rough: 1 file = ~10 chunks avg)
        let estimated_new_chunks = (added + modified) as i64 * 10;
        let current_chunks = change_detector.get_total_chunks(workspace).await?;
        let projected_total = current_chunks + estimated_new_chunks;

        if projected_total > config.indexing.max_chunks {
            return Err(RetrievalErr::ChunkLimitExceeded {
                current: current_chunks,
                limit: config.indexing.max_chunks,
                hint:
                    "Add ignore patterns to reduce indexing scope, or increase max_chunks in config"
                        .to_string(),
            });
        }

        // Phase 4: Process changes in batches
        let batch_size = config.indexing.batch_size as usize;
        let files_to_process: Vec<_> = changes
            .iter()
            .filter(|c| c.status != ChangeStatus::Deleted)
            .collect();
        let total_to_process = files_to_process.len();

        event_emitter::emit(RetrievalEvent::IndexPhaseChanged {
            workspace: workspace.to_string(),
            phase: IndexPhaseInfo::Chunking,
            progress: 0.15,
            description: format!("Processing {} files...", total_to_process),
        });

        let mut tag_extractor = TagExtractor::new();
        let mut processed = 0;
        let mut failed_files: Vec<String> = Vec::new();

        // Time-based lock refresh (every 15 seconds, lock timeout is 30 seconds)
        let mut last_refresh = Instant::now();
        const REFRESH_INTERVAL: Duration = Duration::from_secs(15);

        for batch in files_to_process.chunks(batch_size) {
            // Refresh lock based on time, not file count
            if last_refresh.elapsed() > REFRESH_INTERVAL {
                lock.refresh().await?;
                last_refresh = Instant::now();
            }

            for change in batch {
                let file_path = root.join(&change.filepath);

                // Read file content with proper error handling
                let content = match std::fs::read_to_string(&file_path) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(
                            filepath = %change.filepath,
                            error = %e,
                            "Failed to read file during indexing, removing from catalog"
                        );
                        failed_files.push(change.filepath.clone());
                        // Remove from catalog since file is not accessible
                        // This prevents orphaned entries from accumulating
                        let _ = change_detector
                            .remove_from_catalog(workspace, &change.filepath)
                            .await;
                        continue;
                    }
                };

                // Extract tags if supported language
                let extracted_tags = if let Some(lang) = SupportedLanguage::from_path(&file_path) {
                    match tag_extractor.extract(&content, lang) {
                        Ok(tags) => {
                            let hash = change.content_hash.as_deref().unwrap_or("");
                            let _ = snippet_storage
                                .store_tags(workspace, &change.filepath, &tags, hash)
                                .await;
                            Some(tags)
                        }
                        Err(_) => None,
                    }
                } else {
                    None
                };

                // Update catalog
                let mtime = get_mtime(&file_path).unwrap_or(0);
                let language = detect_language(&file_path).unwrap_or_default();
                let mut chunk_spans = chunker.chunk(&content, &language).unwrap_or_default();

                // Generate overview chunks for classes/structs with multiple methods
                if let Some(tags) = extracted_tags.as_ref() {
                    let overview_config = OverviewConfig::default();
                    let overview_spans = generate_overview_chunks(&content, tags, &overview_config);
                    // Add overview chunks with a special suffix to distinguish them
                    for span in overview_spans {
                        chunk_spans.push(span);
                    }
                }

                // Process embeddings if enabled
                if let (Some(lancedb), Some(provider), Some(cache)) =
                    (lancedb, provider, cache.as_ref())
                {
                    // 1. Delete old chunks from LanceDB
                    if let Err(e) = lancedb.delete_by_path(&change.filepath).await {
                        tracing::warn!(
                            filepath = %change.filepath,
                            error = %e,
                            "Failed to delete old chunks from LanceDB"
                        );
                    }

                    // 2. Check cache and collect chunks needing embedding
                    let mut chunks_with_emb: Vec<CodeChunk> = Vec::new();
                    let mut to_embed: Vec<(CodeChunk, String)> = Vec::new();

                    for (idx, span) in chunk_spans.iter().enumerate() {
                        let chunk_hash = compute_chunk_hash(&span.content);
                        // Extract parent context from tags if available (skip for overview chunks)
                        let parent_symbol = if span.is_overview {
                            None // Overview chunks don't need parent context
                        } else {
                            extracted_tags.as_ref().and_then(|tags| {
                                get_parent_context(&content, tags, span.start_line, span.end_line)
                            })
                        };
                        let chunk = CodeChunk {
                            id: format!("{}:{}:{}", workspace, change.filepath, idx),
                            source_id: workspace.to_string(),
                            filepath: change.filepath.clone(),
                            language: language.clone(),
                            content: span.content.clone(),
                            start_line: span.start_line,
                            end_line: span.end_line,
                            embedding: None,
                            modified_time: Some(mtime),
                            workspace: workspace.to_string(),
                            content_hash: chunk_hash.clone(),
                            indexed_at: chrono::Utc::now().timestamp(),
                            parent_symbol,
                            is_overview: span.is_overview,
                        };

                        if let Some(embedding) = cache.get(&change.filepath, &chunk_hash) {
                            // Cache hit
                            chunks_with_emb.push(CodeChunk {
                                embedding: Some(embedding),
                                ..chunk
                            });
                        } else {
                            // Cache miss - need to compute
                            to_embed.push((chunk, chunk_hash));
                        }
                    }

                    // 3. Batch compute missing embeddings
                    if !to_embed.is_empty() {
                        let texts: Vec<String> = to_embed
                            .iter()
                            .map(|(c, _)| c.embedding_content())
                            .collect();

                        match provider.embed_batch(&texts).await {
                            Ok(embeddings) => {
                                for ((chunk, hash), emb) in
                                    to_embed.into_iter().zip(embeddings.into_iter())
                                {
                                    // Store in cache
                                    if let Err(e) = cache.put(&change.filepath, &hash, &emb) {
                                        tracing::warn!(
                                            filepath = %change.filepath,
                                            error = %e,
                                            "Failed to cache embedding"
                                        );
                                    }
                                    chunks_with_emb.push(CodeChunk {
                                        embedding: Some(emb),
                                        ..chunk
                                    });
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    filepath = %change.filepath,
                                    error = %e,
                                    "Failed to compute embeddings, storing chunks without embeddings"
                                );
                                // Store chunks without embeddings
                                for (chunk, _) in to_embed {
                                    chunks_with_emb.push(chunk);
                                }
                            }
                        }
                    }

                    // 4. Store to LanceDB
                    if !chunks_with_emb.is_empty() {
                        if let Err(e) = lancedb.store_chunks(&chunks_with_emb).await {
                            tracing::error!(
                                filepath = %change.filepath,
                                error = %e,
                                "Failed to store chunks to LanceDB"
                            );
                        }

                        // 5. Index chunks with custom BM25 (if enabled)
                        if let Some(bm25) = bm25_searcher {
                            // Index all chunks for this file
                            bm25.index_chunks(&chunks_with_emb).await;
                        }
                    }
                }

                change_detector
                    .update_catalog(
                        workspace,
                        &change.filepath,
                        change.content_hash.as_deref().unwrap_or(""),
                        mtime,
                        chunk_spans.len() as i32,
                        0,
                    )
                    .await?;

                // Emit file processed event
                event_emitter::emit(RetrievalEvent::IndexFileProcessed {
                    workspace: workspace.to_string(),
                    path: change.filepath.clone(),
                    chunks: chunk_spans.len() as i32,
                    status: FileProcessStatus::Success,
                });

                processed += 1;
            }

            // Report progress
            let progress = 0.15 + (0.8 * processed as f32 / total_to_process.max(1) as f32);
            let _ = tx
                .send(IndexProgress::indexing(
                    progress,
                    format!("Indexed {processed}/{total_to_process} files"),
                ))
                .await;
        }

        // Phase 5: Handle deletions
        for change in changes.iter().filter(|c| c.status == ChangeStatus::Deleted) {
            // Delete from LanceDB (if enabled)
            if let Some(lancedb) = lancedb {
                if let Err(e) = lancedb.delete_by_path(&change.filepath).await {
                    tracing::warn!(
                        filepath = %change.filepath,
                        error = %e,
                        "Failed to delete chunks from LanceDB during file deletion"
                    );
                }
            }

            // Delete from embedding cache (if enabled)
            if let Some(cache) = cache.as_ref() {
                if let Err(e) = cache.delete_by_filepath(&change.filepath) {
                    tracing::warn!(
                        filepath = %change.filepath,
                        error = %e,
                        "Failed to delete embeddings from cache during file deletion"
                    );
                }
            }

            // Delete from BM25 index (if enabled)
            if let Some(bm25) = bm25_searcher {
                bm25.remove_chunks_by_filepath(&change.filepath).await;
            }

            // Delete from catalog and snippets
            change_detector
                .remove_from_catalog(workspace, &change.filepath)
                .await?;
            snippet_storage
                .delete_by_filepath(workspace, &change.filepath)
                .await?;
        }

        // Report failed files if any
        if !failed_files.is_empty() {
            tracing::warn!(
                count = failed_files.len(),
                "Some files could not be indexed due to read errors"
            );
        }

        // Emit finalizing phase
        event_emitter::emit(RetrievalEvent::IndexPhaseChanged {
            workspace: workspace.to_string(),
            phase: IndexPhaseInfo::Finalizing,
            progress: 0.95,
            description: "Finalizing index...".to_string(),
        });

        // Save BM25 metadata (if enabled)
        if let Some(bm25) = bm25_searcher {
            if let Err(e) = bm25.save_to_storage().await {
                tracing::warn!(error = %e, "Failed to save BM25 metadata");
            }
        }

        let status_msg = if failed_files.is_empty() {
            format!(
                "Indexed {processed} files ({added} added, {modified} modified, {deleted} deleted)"
            )
        } else {
            format!(
                "Indexed {processed} files ({added} added, {modified} modified, {deleted} deleted, {} failed)",
                failed_files.len()
            )
        };

        let _ = tx.send(IndexProgress::done(status_msg.clone())).await;

        // Emit index build completed event
        let duration_ms = index_start.elapsed().as_millis() as i64;
        event_emitter::emit(RetrievalEvent::IndexBuildCompleted {
            workspace: workspace.to_string(),
            stats: IndexStatsSummary {
                file_count: processed as i32,
                chunk_count: 0, // TODO: track total chunks
                symbol_count: 0,
                index_size_bytes: 0,
                languages: Vec::new(),
            },
            duration_ms,
        });

        Ok(())
    }

    /// Rebuild the index with the specified mode.
    ///
    /// - `Incremental`: Only process changed files (default behavior)
    /// - `Clean`: Delete all index data and rebuild from scratch
    pub async fn rebuild(
        &mut self,
        workspace: &str,
        root: &Path,
        mode: RebuildMode,
    ) -> Result<mpsc::Receiver<IndexProgress>> {
        if mode == RebuildMode::Clean {
            self.clean(workspace).await?;
        }
        self.index_workspace(workspace, root).await
    }

    /// Clean all index data for a workspace.
    ///
    /// Deletes all catalog entries, snippet data, and LanceDB chunks for the workspace.
    pub async fn clean(&mut self, workspace: &str) -> Result<()> {
        let ws = workspace.to_string();

        // Delete from LanceDB (if enabled)
        if let Some(lancedb) = &self.lancedb {
            if let Err(e) = lancedb.delete_workspace(workspace).await {
                tracing::warn!(
                    workspace = workspace,
                    error = %e,
                    "Failed to delete workspace from LanceDB"
                );
            }
        }

        // Delete from catalog
        self.db
            .query(move |conn| {
                conn.execute("DELETE FROM catalog WHERE workspace = ?", [&ws])?;
                Ok(())
            })
            .await?;

        // Delete snippets
        self.snippet_storage.delete_by_workspace(workspace).await?;

        tracing::info!(workspace = workspace, "Cleaned all index data");
        Ok(())
    }

    /// Get index statistics for a workspace.
    pub async fn get_stats(&self, workspace: &str) -> Result<IndexStats> {
        let ws = workspace.to_string();

        let (file_count, chunk_count, last_indexed) = self
            .db
            .query(move |conn| {
                let file_count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM catalog WHERE workspace = ?",
                        [&ws],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);

                let chunk_count: i64 = conn
                    .query_row(
                        "SELECT COALESCE(SUM(chunks_count), 0) FROM catalog WHERE workspace = ?",
                        [&ws],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);

                let last_indexed: Option<i64> = conn
                    .query_row(
                        "SELECT MAX(indexed_at) FROM catalog WHERE workspace = ?",
                        [&ws],
                        |row| row.get(0),
                    )
                    .ok()
                    .flatten();

                Ok((file_count, chunk_count, last_indexed))
            })
            .await?;

        Ok(IndexStats {
            file_count,
            chunk_count,
            last_indexed,
        })
    }
}

/// Index statistics for a workspace.
#[derive(Debug, Clone, Default)]
pub struct IndexStats {
    /// Number of indexed files
    pub file_count: i64,
    /// Total number of chunks
    pub chunk_count: i64,
    /// Unix timestamp of last indexing operation
    pub last_indexed: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use tempfile::TempDir;

    use async_trait::async_trait;

    /// Mock embedding provider for testing.
    #[derive(Debug)]
    struct MockEmbeddingProvider {
        call_count: AtomicUsize,
        dimension: i32,
    }

    impl MockEmbeddingProvider {
        fn new(dimension: i32) -> Self {
            Self {
                call_count: AtomicUsize::new(0),
                dimension,
            }
        }

        fn call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }

        fn reset_count(&self) {
            self.call_count.store(0, Ordering::SeqCst);
        }
    }

    #[async_trait]
    impl EmbeddingProvider for MockEmbeddingProvider {
        fn name(&self) -> &str {
            "mock"
        }

        fn dimension(&self) -> i32 {
            self.dimension
        }

        async fn embed(&self, _text: &str) -> crate::error::Result<Vec<f32>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(vec![0.1; self.dimension as usize])
        }

        async fn embed_batch(&self, texts: &[String]) -> crate::error::Result<Vec<Vec<f32>>> {
            self.call_count.fetch_add(texts.len(), Ordering::SeqCst);
            Ok(texts
                .iter()
                .map(|_| vec![0.1; self.dimension as usize])
                .collect())
        }
    }

    #[tokio::test]
    async fn test_index_manager_new() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Arc::new(SqliteStore::open(&db_path).unwrap());
        let config = RetrievalConfig::default();
        let _manager = IndexManager::new(config, store);
    }

    #[tokio::test]
    async fn test_index_manager_with_embeddings() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let lancedb_path = dir.path().join("lancedb");
        let cache_path = dir.path().join("cache.db");

        let store = Arc::new(SqliteStore::open(&db_path).unwrap());
        let lancedb = Arc::new(LanceDbStore::open(&lancedb_path).await.unwrap());
        let provider = Arc::new(MockEmbeddingProvider::new(4));

        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let manager = IndexManager::with_embeddings(
            config,
            store,
            lancedb,
            provider,
            &cache_path,
            "test-model-v1",
        )
        .unwrap();

        assert!(manager.has_embeddings());
    }

    #[tokio::test]
    async fn test_index_stores_chunks_to_lancedb() {
        let dir = TempDir::new().unwrap();
        let workspace_dir = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        // Create test file
        let test_file = workspace_dir.join("test.rs");
        std::fs::write(&test_file, "fn main() {\n    println!(\"hello\");\n}").unwrap();

        let db_path = dir.path().join("test.db");
        let lancedb_path = dir.path().join("lancedb");
        let cache_path = dir.path().join("cache.db");

        let store = Arc::new(SqliteStore::open(&db_path).unwrap());
        let lancedb = Arc::new(LanceDbStore::open(&lancedb_path).await.unwrap());
        let provider = Arc::new(MockEmbeddingProvider::new(1536));

        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let mut manager = IndexManager::with_embeddings(
            config,
            store,
            lancedb.clone(),
            provider.clone(),
            &cache_path,
            "test-model-v1",
        )
        .unwrap();

        // Index the workspace
        let mut rx = manager
            .index_workspace("test", &workspace_dir)
            .await
            .unwrap();

        // Wait for indexing to complete
        while let Some(progress) = rx.recv().await {
            if matches!(
                progress.status,
                crate::indexing::progress::IndexStatus::Done
                    | crate::indexing::progress::IndexStatus::Failed
            ) {
                break;
            }
        }

        // Verify chunks were stored in LanceDB
        let count = lancedb.count().await.unwrap();
        assert!(count > 0, "Expected chunks in LanceDB, got {count}");

        // Verify provider was called
        assert!(
            provider.call_count() > 0,
            "Expected provider to be called, but call_count is 0"
        );
    }

    #[tokio::test]
    async fn test_cache_hit_skips_api_call() {
        let dir = TempDir::new().unwrap();
        let workspace_dir = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        // Create test file
        let test_file = workspace_dir.join("test.rs");
        std::fs::write(&test_file, "fn foo() {}").unwrap();

        let db_path = dir.path().join("test.db");
        let lancedb_path = dir.path().join("lancedb");
        let cache_path = dir.path().join("cache.db");

        let store = Arc::new(SqliteStore::open(&db_path).unwrap());
        let lancedb = Arc::new(LanceDbStore::open(&lancedb_path).await.unwrap());
        let provider = Arc::new(MockEmbeddingProvider::new(1536));

        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        // First indexing
        let mut manager = IndexManager::with_embeddings(
            config.clone(),
            store.clone(),
            lancedb.clone(),
            provider.clone(),
            &cache_path,
            "test-model-v1",
        )
        .unwrap();

        let mut rx = manager
            .index_workspace("test", &workspace_dir)
            .await
            .unwrap();
        while let Some(progress) = rx.recv().await {
            if matches!(
                progress.status,
                crate::indexing::progress::IndexStatus::Done
                    | crate::indexing::progress::IndexStatus::Failed
            ) {
                break;
            }
        }

        let first_call_count = provider.call_count();
        assert!(first_call_count > 0, "First indexing should call provider");

        // Reset provider count
        provider.reset_count();

        // Modify file slightly (add whitespace at end) - but same chunk content
        // Note: This tests that unchanged chunks reuse cache
        // For this test, we keep file exactly the same to trigger cache hit
        // Touch the file to update mtime (simulates re-indexing)
        // Actually, let's create a new manager and re-index

        // Second indexing with same content
        let store2 = Arc::new(SqliteStore::open(&db_path).unwrap());
        let mut manager2 = IndexManager::with_embeddings(
            config,
            store2,
            lancedb.clone(),
            provider.clone(),
            &cache_path,
            "test-model-v1",
        )
        .unwrap();

        // Delete catalog entries to force re-processing
        manager2.clean("test").await.unwrap();

        let mut rx = manager2
            .index_workspace("test", &workspace_dir)
            .await
            .unwrap();
        while let Some(progress) = rx.recv().await {
            if matches!(
                progress.status,
                crate::indexing::progress::IndexStatus::Done
                    | crate::indexing::progress::IndexStatus::Failed
            ) {
                break;
            }
        }

        // Provider should NOT be called because cache has the embeddings
        assert_eq!(
            provider.call_count(),
            0,
            "Second indexing should use cache, but provider was called {} times",
            provider.call_count()
        );
    }

    #[tokio::test]
    async fn test_file_deletion_clears_lancedb_and_cache() {
        let dir = TempDir::new().unwrap();
        let workspace_dir = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();

        // Create test file
        let test_file = workspace_dir.join("test.rs");
        std::fs::write(&test_file, "fn main() {}").unwrap();

        let db_path = dir.path().join("test.db");
        let lancedb_path = dir.path().join("lancedb");
        let cache_path = dir.path().join("cache.db");

        let store = Arc::new(SqliteStore::open(&db_path).unwrap());
        let lancedb = Arc::new(LanceDbStore::open(&lancedb_path).await.unwrap());
        let provider = Arc::new(MockEmbeddingProvider::new(1536));

        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let mut manager = IndexManager::with_embeddings(
            config.clone(),
            store.clone(),
            lancedb.clone(),
            provider.clone(),
            &cache_path,
            "test-model-v1",
        )
        .unwrap();

        // First indexing
        let mut rx = manager
            .index_workspace("test", &workspace_dir)
            .await
            .unwrap();
        while let Some(progress) = rx.recv().await {
            if matches!(
                progress.status,
                crate::indexing::progress::IndexStatus::Done
                    | crate::indexing::progress::IndexStatus::Failed
            ) {
                break;
            }
        }

        // Verify data exists
        let count_before = lancedb.count().await.unwrap();
        assert!(count_before > 0, "Expected chunks before deletion");

        // Delete the file
        std::fs::remove_file(&test_file).unwrap();

        // Re-index to detect deletion
        let store2 = Arc::new(SqliteStore::open(&db_path).unwrap());
        let mut manager2 = IndexManager::with_embeddings(
            config,
            store2,
            lancedb.clone(),
            provider.clone(),
            &cache_path,
            "test-model-v1",
        )
        .unwrap();

        let mut rx = manager2
            .index_workspace("test", &workspace_dir)
            .await
            .unwrap();
        while let Some(progress) = rx.recv().await {
            if matches!(
                progress.status,
                crate::indexing::progress::IndexStatus::Done
                    | crate::indexing::progress::IndexStatus::Failed
            ) {
                break;
            }
        }

        // Verify LanceDB is empty
        let count_after = lancedb.count().await.unwrap();
        assert_eq!(count_after, 0, "Expected no chunks after file deletion");
    }
}
