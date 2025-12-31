//! Repo map module for PageRank-based context generation.
//!
//! Provides intelligent codebase context generation for LLMs using:
//! - Tree-sitter tag extraction (definitions and references)
//! - PageRank-based file/symbol importance ranking
//! - Token-budgeted output generation
//! - 3-level caching (SQLite, in-memory LRU, TTL)
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

use sha2::Digest;
use sha2::Sha256;

use crate::config::RefreshMode;
use crate::config::RepoMapConfig;
use crate::error::Result;
use crate::event_emitter;
use crate::events::RankedFileSummary;
use crate::events::RetrievalEvent;
use crate::storage::SqliteStore;
use crate::tags::extractor::CodeTag;

pub use budget::TokenBudgeter;
pub use cache::RepoMapCache;
pub use graph::DependencyGraph;
pub use graph::extract_terms;
pub use pagerank::PageRanker;
pub use renderer::TreeRenderer;

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
}

/// Main repo map service.
///
/// Coordinates tag extraction, graph building, PageRank ranking,
/// token budgeting, and tree rendering.
pub struct RepoMapService {
    config: RepoMapConfig,
    cache: RepoMapCache,
    graph: DependencyGraph,
    ranker: PageRanker,
    budgeter: TokenBudgeter,
    renderer: TreeRenderer,
    workspace_root: PathBuf,
    /// Last generated result (for Manual refresh mode)
    last_result: Option<RepoMapResult>,
    /// Last generation time in milliseconds (for Auto refresh mode)
    last_generation_time_ms: i64,
}

impl RepoMapService {
    /// Create a new repo map service.
    pub fn new(
        config: RepoMapConfig,
        db: Arc<SqliteStore>,
        workspace_root: PathBuf,
    ) -> Result<Self> {
        let cache = RepoMapCache::new(db, config.cache_ttl_secs);
        let graph = DependencyGraph::new();
        let ranker = PageRanker::new(
            config.damping_factor,
            config.max_iterations,
            config.tolerance,
        );
        let budgeter = TokenBudgeter::new()?;
        let renderer = TreeRenderer::new();

        Ok(Self {
            config,
            cache,
            graph,
            ranker,
            budgeter,
            renderer,
            workspace_root,
            last_result: None,
            last_generation_time_ms: 0,
        })
    }

    /// Compute a deterministic hash for the request to use as cache key.
    ///
    /// Uses SHA256 instead of DefaultHasher to ensure hash consistency across
    /// process restarts (DefaultHasher may vary between runs).
    fn compute_request_hash(&self, request: &RepoMapRequest, include_mentions: bool) -> String {
        let mut hasher = Sha256::new();

        // Hash chat files (sorted for determinism)
        let mut chat_files: Vec<_> = request
            .chat_files
            .iter()
            .filter_map(|p| p.to_str())
            .collect();
        chat_files.sort();
        for f in chat_files {
            hasher.update(f.as_bytes());
            hasher.update(b"\0"); // separator
        }

        // Hash other files (sorted for determinism)
        let mut other_files: Vec<_> = request
            .other_files
            .iter()
            .filter_map(|p| p.to_str())
            .collect();
        other_files.sort();
        for f in other_files {
            hasher.update(f.as_bytes());
            hasher.update(b"\0");
        }

        // Hash max tokens
        hasher.update(request.max_tokens.to_le_bytes());

        // For Files mode, exclude mentions from hash
        if include_mentions {
            let mut fnames: Vec<_> = request.mentioned_fnames.iter().collect();
            fnames.sort();
            for f in fnames {
                hasher.update(f.as_bytes());
                hasher.update(b"\0");
            }

            let mut idents: Vec<_> = request.mentioned_idents.iter().collect();
            idents.sort();
            for i in idents {
                hasher.update(i.as_bytes());
                hasher.update(b"\0");
            }
        }

        // Return first 16 hex chars of SHA256 (64 bits)
        let result = hasher.finalize();
        hex::encode(&result[..8])
    }

    /// Generate a repo map for the given request.
    ///
    /// # Arguments
    /// * `request` - The repo map request
    /// * `force_refresh` - Force regeneration even if cached result exists
    pub async fn generate(
        &mut self,
        request: &RepoMapRequest,
        force_refresh: bool,
    ) -> Result<RepoMapResult> {
        // Generate request_id for event tracking
        let request_id = format!("repomap-{}", chrono::Utc::now().timestamp_millis());

        // Emit RepoMapStarted event
        event_emitter::emit(RetrievalEvent::RepoMapStarted {
            request_id: request_id.clone(),
            max_tokens: request.max_tokens,
            chat_files: request.chat_files.len() as i32,
            other_files: request.other_files.len() as i32,
        });

        // Check refresh mode for early return
        if !force_refresh {
            match self.config.refresh_mode {
                RefreshMode::Manual => {
                    // Return cached result if available
                    if let Some(ref cached) = self.last_result {
                        event_emitter::emit(RetrievalEvent::RepoMapCacheHit {
                            request_id: request_id.clone(),
                            cache_key: "manual_last_result".to_string(),
                        });
                        return Ok(cached.clone());
                    }
                }
                RefreshMode::Auto => {
                    // Use L3 cache if last generation took > 1 second
                    if self.last_generation_time_ms > 1000 {
                        let hash = self.compute_request_hash(request, true);
                        if let Some((content, tokens, files_included)) = self.cache.get_map(&hash) {
                            event_emitter::emit(RetrievalEvent::RepoMapCacheHit {
                                request_id: request_id.clone(),
                                cache_key: hash,
                            });
                            return Ok(RepoMapResult {
                                content,
                                tokens,
                                files_included,
                                generation_time_ms: 0, // Cached result
                            });
                        }
                    }
                }
                RefreshMode::Files => {
                    // Use L3 cache based on file set only (exclude mentions from hash)
                    let hash = self.compute_request_hash(request, false);
                    if let Some((content, tokens, files_included)) = self.cache.get_map(&hash) {
                        event_emitter::emit(RetrievalEvent::RepoMapCacheHit {
                            request_id: request_id.clone(),
                            cache_key: hash,
                        });
                        return Ok(RepoMapResult {
                            content,
                            tokens,
                            files_included,
                            generation_time_ms: 0, // Cached result
                        });
                    }
                }
                RefreshMode::Always => {
                    // Never use cache, always regenerate
                }
            }
        }

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
        let file_tags = self.extract_tags_for_files(&all_files).await?;

        // Build dependency graph
        self.graph.clear();
        for (filepath, tags) in &file_tags {
            self.graph.add_file_tags(filepath, tags);
        }

        // Extract query terms from mentioned identifiers for fuzzy matching
        let query_terms: HashSet<String> = request
            .mentioned_idents
            .iter()
            .flat_map(|ident| extract_terms(ident))
            .collect();

        // Build weighted edges with personalization
        self.graph.build_edges(
            &chat_file_set,
            &request.mentioned_idents,
            &query_terms,
            self.config.chat_file_weight,
            self.config.mentioned_ident_weight,
            self.config.private_symbol_weight,
            self.config.naming_style_weight,
            self.config.term_match_weight,
        );

        // Run PageRank
        let personalization = self.graph.build_personalization(&chat_file_set);
        let file_ranks = self.ranker.rank(self.graph.graph(), &personalization)?;

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
        let file_def_counts = self.graph.compute_file_definition_counts();

        // Distribute ranks to symbols
        let ranked_symbols = self.ranker.distribute_to_definitions(
            &file_ranks,
            self.graph.definitions(),
            &file_def_counts,
        );

        // Determine token budget
        let max_tokens = if request.chat_files.is_empty() {
            (request.max_tokens as f32 * self.config.map_mul_no_files) as i32
        } else {
            request.max_tokens
        };

        // Find optimal tag count via binary search
        let optimal_count =
            self.budgeter
                .find_optimal_count(&ranked_symbols, &self.renderer, max_tokens);

        // Render the tree
        let (rendered_content, rendered_files) = self.renderer.render(
            &ranked_symbols,
            &chat_file_set,
            optimal_count,
            &self.workspace_root,
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
        };

        // Save for caching (used by Manual and Auto refresh modes)
        self.last_result = Some(result.clone());
        self.last_generation_time_ms = generation_time_ms;

        // Store in L3 cache for Auto and Files modes
        match self.config.refresh_mode {
            RefreshMode::Auto => {
                let hash = self.compute_request_hash(request, true);
                self.cache.put_map(
                    &hash,
                    result.content.clone(),
                    result.tokens,
                    result.files_included,
                );
            }
            RefreshMode::Files => {
                let hash = self.compute_request_hash(request, false);
                self.cache.put_map(
                    &hash,
                    result.content.clone(),
                    result.tokens,
                    result.files_included,
                );
            }
            RefreshMode::Manual | RefreshMode::Always => {
                // Manual uses last_result, Always never caches
            }
        }

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
        &mut self,
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

        self.graph.clear();
        for (filepath, tags) in &file_tags {
            self.graph.add_file_tags(filepath, tags);
        }

        // Extract query terms for fuzzy matching
        let query_terms: HashSet<String> = mentioned_idents
            .iter()
            .flat_map(|ident| extract_terms(ident))
            .collect();

        self.graph.build_edges(
            &chat_file_set,
            mentioned_idents,
            &query_terms,
            self.config.chat_file_weight,
            self.config.mentioned_ident_weight,
            self.config.private_symbol_weight,
            self.config.naming_style_weight,
            self.config.term_match_weight,
        );

        let personalization = self.graph.build_personalization(&chat_file_set);
        let file_ranks = self.ranker.rank(self.graph.graph(), &personalization)?;

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

            // Extract tags from file
            let mut extractor = crate::tags::extractor::TagExtractor::new();
            match extractor.extract_file(file) {
                Ok(tags) => {
                    // Cache the tags
                    self.cache.put_tags(&filepath, &tags).await?;
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
