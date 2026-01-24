//! Repo map module for PageRank-based context generation.
//!
//! Provides intelligent codebase context generation for LLMs using:
//! - Tree-sitter tag extraction (definitions and references)
//! - PageRank-based file/symbol importance ranking
//! - Token-budgeted output generation
//! - SQLite-based tag caching
//!
//! Inspired by Aider's repo map feature.

pub mod budget;
pub mod cache;
pub mod graph;
pub mod important_files;
pub mod pagerank;
pub mod renderer;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::config::RepoMapConfig;
use crate::error::Result;
use crate::event_emitter;
use crate::events::RankedFileSummary;
use crate::events::RetrievalEvent;
use crate::storage::SqliteStore;
use crate::tags::extractor::CodeTag;

// Internal imports (not re-exported)
use graph::DependencyGraph;
use graph::extract_terms;
use pagerank::PageRanker;
use renderer::TreeRenderer;

pub use budget::TokenBudgeter;
pub use cache::RepoMapCache;

// Re-export TagPipeline types from indexing (where they now live)
pub use crate::indexing::SharedTagPipeline;
pub use crate::indexing::TagEventProcessor;
pub use crate::indexing::TagPipeline;
pub use crate::indexing::TagPipelineState;
pub use crate::indexing::TagReadiness;
pub use crate::indexing::TagStats;
pub use crate::indexing::TagStrictModeConfig;
pub use crate::indexing::TagWorkerPool;

/// Ranked file with PageRank score.
#[derive(Debug, Clone)]
pub struct RankedFile {
    /// File path (relative to workspace root)
    pub filepath: String,
    /// PageRank score
    pub rank: f64,
    /// Ranked symbols in this file
    pub symbols: Vec<RankedSymbol>,
}

/// Ranked symbol with PageRank score.
#[derive(Debug, Clone)]
pub struct RankedSymbol {
    /// The code tag (reused from existing extractor)
    pub tag: CodeTag,
    /// PageRank score for this symbol
    pub rank: f64,
    /// File path containing this symbol
    pub filepath: String,
}

/// Repo map generation request.
#[derive(Debug, Clone)]
pub struct RepoMapRequest {
    /// Files currently in chat context (get 50x weight boost)
    pub chat_files: Vec<PathBuf>,
    /// Other repository files to include
    pub other_files: Vec<PathBuf>,
    /// File names mentioned by user
    pub mentioned_fnames: HashSet<String>,
    /// Identifiers mentioned by user (10x weight boost)
    pub mentioned_idents: HashSet<String>,
    /// Maximum tokens for output
    pub max_tokens: i32,
}

impl Default for RepoMapRequest {
    fn default() -> Self {
        Self {
            chat_files: Vec::new(),
            other_files: Vec::new(),
            mentioned_fnames: HashSet::new(),
            mentioned_idents: HashSet::new(),
            max_tokens: 1024,
        }
    }
}

/// Repo map generation result.
#[derive(Debug, Clone)]
pub struct RepoMapResult {
    /// Rendered tree output
    pub content: String,
    /// Actual token count
    pub tokens: i32,
    /// Number of files included
    pub files_included: i32,
    /// Generation time in milliseconds
    pub generation_time_ms: i64,
    /// Active filter configuration (if any).
    ///
    /// Allows LLM callers to understand what files/directories are
    /// included or excluded from the index used to build this map.
    pub filter: Option<crate::indexing::FilterSummary>,
}

/// RepoMap generator for PageRank-based context generation.
///
/// Coordinates tag extraction, graph building, PageRank ranking,
/// token budgeting, and tree rendering.
///
/// **Lifecycle**: Per-request. Create via `new_with_shared()`, call
/// `generate()`, then discard. Not a long-lived service.
pub struct RepoMapGenerator {
    config: RepoMapConfig,
    cache: RepoMapCache,
    budgeter: Arc<TokenBudgeter>,
    workspace_root: PathBuf,
}

impl RepoMapGenerator {
    /// Create with shared components (recommended for production).
    ///
    /// Uses pre-initialized SqliteStore and TokenBudgeter to avoid
    /// repeated initialization overhead.
    pub fn new_with_shared(
        config: RepoMapConfig,
        db: Arc<SqliteStore>,
        budgeter: Arc<TokenBudgeter>,
        workspace_root: PathBuf,
    ) -> Result<Self> {
        let cache = RepoMapCache::new(db);

        Ok(Self {
            config,
            cache,
            budgeter,
            workspace_root,
        })
    }

    /// Create a new repo map generator (convenience constructor).
    ///
    /// Uses the global TokenBudgeter singleton.
    pub fn new(
        config: RepoMapConfig,
        db: Arc<SqliteStore>,
        workspace_root: PathBuf,
    ) -> Result<Self> {
        Self::new_with_shared(config, db, TokenBudgeter::shared(), workspace_root)
    }

    /// Generate a repo map for the given request.
    ///
    /// # Arguments
    /// * `request` - The repo map request
    pub async fn generate(&self, request: &RepoMapRequest) -> Result<RepoMapResult> {
        // Generate request_id for event tracking
        let request_id = format!("repomap-{}", chrono::Utc::now().timestamp_millis());

        tracing::debug!(
            request_id = %request_id,
            max_tokens = request.max_tokens,
            chat_files = request.chat_files.len(),
            other_files = request.other_files.len(),
            mentioned_idents = request.mentioned_idents.len(),
            "RepoMap generation started"
        );

        // Emit RepoMapStarted event
        event_emitter::emit(RetrievalEvent::RepoMapStarted {
            request_id: request_id.clone(),
            max_tokens: request.max_tokens,
            chat_files: request.chat_files.len() as i32,
            other_files: request.other_files.len() as i32,
        });

        let start = Instant::now();

        // Collect all files
        let mut all_files: Vec<PathBuf> = request.chat_files.clone();
        all_files.extend(request.other_files.clone());

        // Build file sets for personalization
        let chat_file_set: HashSet<String> = request
            .chat_files
            .iter()
            .filter_map(|p| p.to_str().map(String::from))
            .collect();

        // Extract tags for all files (using cache where possible)
        let tag_start = Instant::now();
        let file_tags = self.extract_tags_for_files(&all_files).await?;
        let total_tags: usize = file_tags.values().map(|t| t.len()).sum();
        tracing::debug!(
            files = file_tags.len(),
            total_tags = total_tags,
            duration_ms = tag_start.elapsed().as_millis() as i64,
            "Tags extracted"
        );

        // Build dependency graph (created per-request)
        let graph_start = Instant::now();
        let mut graph = DependencyGraph::new();
        for (filepath, tags) in &file_tags {
            graph.add_file_tags(filepath, tags);
        }

        // Extract query terms from mentioned identifiers for fuzzy matching
        let query_terms: HashSet<String> = request
            .mentioned_idents
            .iter()
            .flat_map(|ident| extract_terms(ident))
            .collect();

        // Build weighted edges with personalization
        graph.build_edges(
            &chat_file_set,
            &request.mentioned_idents,
            &query_terms,
            self.config.chat_file_weight,
            self.config.mentioned_ident_weight,
            self.config.private_symbol_weight,
            self.config.naming_style_weight,
            self.config.term_match_weight,
        );
        tracing::trace!(
            edges = graph.edge_count(),
            duration_ms = graph_start.elapsed().as_millis() as i64,
            "Graph built"
        );

        // Run PageRank (created per-request)
        let ranker = PageRanker::new(
            self.config.damping_factor,
            self.config.max_iterations,
            self.config.tolerance,
        );
        let personalization = graph.build_personalization(&chat_file_set);
        let file_ranks = ranker.rank(graph.graph(), &personalization)?;

        // Emit PageRankComputed event with top 10 files
        let mut sorted_ranks: Vec<_> = file_ranks.iter().collect();
        sorted_ranks.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top_files: Vec<RankedFileSummary> = sorted_ranks
            .iter()
            .take(10)
            .map(|(filepath, rank)| {
                let symbol_count = file_tags
                    .get(*filepath)
                    .map(|tags| tags.iter().filter(|t| t.is_definition).count() as i32)
                    .unwrap_or(0);
                RankedFileSummary {
                    filepath: (*filepath).clone(),
                    rank: **rank,
                    symbol_count,
                }
            })
            .collect();
        event_emitter::emit(RetrievalEvent::PageRankComputed {
            request_id: request_id.clone(),
            iterations: self.config.max_iterations,
            top_files,
        });

        // Compute file definition counts for proper rank distribution
        let file_def_counts = graph.compute_file_definition_counts();

        // Distribute ranks to symbols
        let ranked_symbols =
            ranker.distribute_to_definitions(&file_ranks, graph.definitions(), &file_def_counts);

        // Determine token budget
        let max_tokens = if request.chat_files.is_empty() {
            (request.max_tokens as f32 * self.config.map_mul_no_files) as i32
        } else {
            request.max_tokens
        };

        // Find optimal tag count via binary search (renderer created per-request)
        let renderer = TreeRenderer::new();
        let budget_start = Instant::now();
        let optimal_count =
            self.budgeter
                .find_optimal_count(&ranked_symbols, &renderer, max_tokens);
        tracing::trace!(
            max_tokens = max_tokens,
            optimal_count = optimal_count,
            duration_ms = budget_start.elapsed().as_millis() as i64,
            "Budget calculated"
        );

        // Render the tree
        let render_start = Instant::now();
        let (rendered_content, rendered_files) = renderer.render(
            &ranked_symbols,
            &chat_file_set,
            optimal_count,
            &self.workspace_root,
        );
        tracing::trace!(
            rendered_files = rendered_files.len(),
            content_len = rendered_content.len(),
            duration_ms = render_start.elapsed().as_millis() as i64,
            "Tree rendered"
        );

        // Prepend important files that aren't already rendered
        let other_file_paths: Vec<String> = request
            .other_files
            .iter()
            .filter_map(|p| p.to_str().map(String::from))
            .collect();
        let important = important_files::filter_important_files(&other_file_paths);

        let mut content = String::new();
        for f in &important {
            // Only add if not already in rendered files (use set instead of string search)
            if !rendered_files.contains(f) {
                content.push_str(f);
                content.push('\n');
            }
        }
        if !content.is_empty() && !rendered_content.is_empty() {
            content.push('\n');
        }
        content.push_str(&rendered_content);

        // Count tokens in final output
        let tokens = self.budgeter.count_tokens(&content);

        let generation_time_ms = start.elapsed().as_millis() as i64;

        let result = RepoMapResult {
            content,
            tokens,
            files_included: file_tags.len() as i32,
            generation_time_ms,
            filter: None, // Will be set by caller
        };

        // Emit RepoMapGenerated event
        event_emitter::emit(RetrievalEvent::RepoMapGenerated {
            request_id,
            tokens: result.tokens,
            files: result.files_included,
            duration_ms: result.generation_time_ms,
        });

        Ok(result)
    }

    /// Get ranked files without rendering (for search integration).
    pub async fn get_ranked_files(
        &self,
        chat_files: &[PathBuf],
        other_files: &[PathBuf],
        mentioned_idents: &HashSet<String>,
    ) -> Result<Vec<RankedFile>> {
        let mut all_files: Vec<PathBuf> = chat_files.to_vec();
        all_files.extend(other_files.to_vec());

        let chat_file_set: HashSet<String> = chat_files
            .iter()
            .filter_map(|p| p.to_str().map(String::from))
            .collect();

        let file_tags = self.extract_tags_for_files(&all_files).await?;

        // Build dependency graph (created per-request)
        let mut graph = DependencyGraph::new();
        for (filepath, tags) in &file_tags {
            graph.add_file_tags(filepath, tags);
        }

        // Extract query terms for fuzzy matching
        let query_terms: HashSet<String> = mentioned_idents
            .iter()
            .flat_map(|ident| extract_terms(ident))
            .collect();

        graph.build_edges(
            &chat_file_set,
            mentioned_idents,
            &query_terms,
            self.config.chat_file_weight,
            self.config.mentioned_ident_weight,
            self.config.private_symbol_weight,
            self.config.naming_style_weight,
            self.config.term_match_weight,
        );

        // Run PageRank (created per-request)
        let ranker = PageRanker::new(
            self.config.damping_factor,
            self.config.max_iterations,
            self.config.tolerance,
        );
        let personalization = graph.build_personalization(&chat_file_set);
        let file_ranks = ranker.rank(graph.graph(), &personalization)?;

        // Group by file
        let mut ranked_files: Vec<RankedFile> = file_ranks
            .iter()
            .map(|(filepath, rank)| {
                let symbols = file_tags
                    .get(filepath)
                    .map(|tags| {
                        tags.iter()
                            .filter(|t| t.is_definition)
                            .map(|tag| RankedSymbol {
                                tag: tag.clone(),
                                rank: *rank,
                                filepath: filepath.clone(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                RankedFile {
                    filepath: filepath.clone(),
                    rank: *rank,
                    symbols,
                }
            })
            .collect();

        // Sort by rank descending
        ranked_files.sort_by(|a, b| {
            b.rank
                .partial_cmp(&a.rank)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(ranked_files)
    }

    /// Extract mentions from a user message.
    ///
    /// Returns (mentioned_fnames, mentioned_idents).
    pub fn extract_mentions(message: &str) -> (HashSet<String>, HashSet<String>) {
        let mut fnames = HashSet::new();
        let mut idents = HashSet::new();

        // Split on non-word characters
        for word in
            message.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '.' && c != '/')
        {
            let word = word.trim();
            if word.is_empty() {
                continue;
            }

            // Check if it looks like a file path
            if word.contains('/') || word.contains('.') {
                if let Some(fname) = word.split('/').last() {
                    if fname.contains('.') {
                        fnames.insert(fname.to_string());
                    }
                }
            }

            // Check if it's a valid identifier
            if word.len() >= 3
                && word
                    .chars()
                    .next()
                    .map(|c| c.is_alphabetic() || c == '_')
                    .unwrap_or(false)
            {
                idents.insert(word.to_string());
            }
        }

        (fnames, idents)
    }

    /// Extract tags for files (using cache).
    async fn extract_tags_for_files(
        &self,
        files: &[PathBuf],
    ) -> Result<std::collections::HashMap<String, Vec<CodeTag>>> {
        let mut result = std::collections::HashMap::new();

        for file in files {
            let filepath = file.to_string_lossy().to_string();

            // Try cache first
            if let Some(cached_tags) = self.cache.get_tags(&filepath).await? {
                result.insert(filepath, cached_tags);
                continue;
            }

            // Record mtime BEFORE extraction (for optimistic lock)
            let mtime_before = RepoMapCache::file_mtime(&filepath);

            // Extract tags from file
            let mut extractor = crate::tags::extractor::TagExtractor::new();
            match extractor.extract_file(file) {
                Ok(tags) => {
                    // Cache the tags with optimistic lock validation
                    let written = self.cache.put_tags(&filepath, &tags, mtime_before).await?;
                    if !written {
                        tracing::debug!(
                            file = %filepath,
                            "Cache write skipped: newer version exists in DB"
                        );
                    }
                    result.insert(filepath, tags);
                }
                Err(e) => {
                    tracing::debug!(file = %filepath, error = %e, "Failed to extract tags");
                    // Continue with other files
                }
            }
        }

        Ok(result)
    }
}
