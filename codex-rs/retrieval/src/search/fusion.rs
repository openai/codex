//! Reciprocal Rank Fusion (RRF) for combining search results.
//!
//! RRF is a simple but effective method for combining ranked lists.
//! Score = Σ weight / (rank + k), where k is typically 60.
//!
//! Also includes recency decay for boosting recently modified files.

use std::collections::HashMap;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use super::constants::DEFAULT_RECENCY_HALF_LIFE_DAYS;
use super::constants::DEFAULT_RRF_K;
use super::constants::LN_2;
use super::constants::SECONDS_PER_DAY;
use crate::types::CodeChunk;
use crate::types::ScoreType;
use crate::types::SearchResult;

/// RRF fusion configuration.
#[derive(Debug, Clone)]
pub struct RrfConfig {
    /// RRF constant (typically 60).
    pub k: f32,
    /// Weight for BM25 results.
    pub bm25_weight: f32,
    /// Weight for vector results.
    pub vector_weight: f32,
    /// Weight for snippet/tag matches.
    pub snippet_weight: f32,
    /// Weight for recently edited files as a retrieval source.
    pub recent_weight: f32,
    /// Weight for recency boost (0.0 = disabled).
    /// This is different from recent_weight - this applies time-based decay to all results.
    pub recency_boost_weight: f32,
    /// Recency decay half-life in days.
    pub recency_half_life_days: f32,
}

impl Default for RrfConfig {
    fn default() -> Self {
        Self {
            k: DEFAULT_RRF_K,
            bm25_weight: 0.5,
            vector_weight: 0.3,
            snippet_weight: 0.0,
            recent_weight: 0.2, // 20% for recently edited files (matches SearchConfig)
            recency_boost_weight: 0.0, // Disabled by default
            recency_half_life_days: DEFAULT_RECENCY_HALF_LIFE_DAYS,
        }
    }
}

impl RrfConfig {
    /// Create a new RRF config with custom weights.
    pub fn new(bm25_weight: f32, vector_weight: f32, snippet_weight: f32) -> Self {
        Self {
            k: DEFAULT_RRF_K,
            bm25_weight,
            vector_weight,
            snippet_weight,
            recent_weight: 0.0,
            recency_boost_weight: 0.0,
            recency_half_life_days: DEFAULT_RECENCY_HALF_LIFE_DAYS,
        }
    }

    /// Create a new RRF config with all four source weights.
    pub fn with_all_weights(
        bm25_weight: f32,
        vector_weight: f32,
        snippet_weight: f32,
        recent_weight: f32,
    ) -> Self {
        Self {
            k: DEFAULT_RRF_K,
            bm25_weight,
            vector_weight,
            snippet_weight,
            recent_weight,
            recency_boost_weight: 0.0,
            recency_half_life_days: DEFAULT_RECENCY_HALF_LIFE_DAYS,
        }
    }

    /// Set the weight for recently edited files source.
    pub fn with_recent_weight(mut self, weight: f32) -> Self {
        self.recent_weight = weight;
        self
    }

    /// Enable recency boost with default half-life (7 days).
    /// This applies time-based decay to all results.
    pub fn with_recency_boost(mut self, weight: f32) -> Self {
        self.recency_boost_weight = weight;
        self
    }

    /// Enable recency boost with custom half-life.
    pub fn with_recency_boost_config(mut self, weight: f32, half_life_days: f32) -> Self {
        self.recency_boost_weight = weight;
        self.recency_half_life_days = half_life_days;
        self
    }

    /// Adjust weights for identifier-heavy queries.
    ///
    /// When the query looks like an identifier (function name, variable, etc.),
    /// boost snippet weight for exact symbol matching.
    pub fn for_identifier_query(mut self) -> Self {
        self.bm25_weight = 0.4;
        self.vector_weight = 0.2;
        self.snippet_weight = 0.3;
        self.recent_weight = 0.1;
        self
    }

    /// Adjust weights for symbol-specific queries.
    ///
    /// When the query contains `type:` or `name:` syntax, heavily boost
    /// snippet weight for symbol matching.
    pub fn for_symbol_query(mut self) -> Self {
        self.bm25_weight = 0.2;
        self.vector_weight = 0.1;
        self.snippet_weight = 0.6;
        self.recent_weight = 0.1;
        self
    }
}

/// Check if query contains symbol search syntax.
///
/// Returns true if the query contains `type:`, `name:`, `file:`, or `path:` prefixes.
pub fn has_symbol_syntax(query: &str) -> bool {
    query.contains("type:")
        || query.contains("name:")
        || query.contains("file:")
        || query.contains("path:")
}

/// Calculate recency score based on file modification time.
///
/// Returns a value between 0.0 and 1.0, where:
/// - 1.0 = modified today
/// - 0.5 = modified `half_life_days` ago
/// - Decays exponentially for older files
///
/// Returns 0.0 if `modified_time` is None or in the future.
pub fn recency_score(modified_time: Option<i64>, half_life_days: f32) -> f32 {
    let Some(mtime) = modified_time else {
        return 0.0;
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    if mtime > now {
        return 0.0; // Future timestamp
    }

    let age_seconds = (now - mtime) as f32;
    let age_days = age_seconds / SECONDS_PER_DAY;

    // Exponential decay: score = exp(-ln(2) * age / half_life)
    let decay_rate = LN_2 / half_life_days;
    (-decay_rate * age_days).exp()
}

/// Apply recency boost to search results.
///
/// Adds `recency_score * recency_boost_weight` to each result's score.
pub fn apply_recency_boost(results: &mut [SearchResult], config: &RrfConfig) {
    if config.recency_boost_weight <= 0.0 {
        return;
    }

    for result in results.iter_mut() {
        let boost = recency_score(result.chunk.modified_time, config.recency_half_life_days);
        result.score += boost * config.recency_boost_weight;
    }
}

/// Compute RRF score for a result at a given rank.
fn rrf_score(rank: i32, weight: f32, k: f32) -> f32 {
    weight / (rank as f32 + k)
}

/// Fuse multiple ranked lists using RRF.
///
/// # Arguments
/// * `bm25_results` - Results from BM25 full-text search
/// * `vector_results` - Results from vector similarity search
/// * `snippet_results` - Results from exact symbol matching
/// * `config` - RRF configuration
/// * `limit` - Maximum results to return
///
/// # Returns
/// Fused and re-ranked results
pub fn fuse_results(
    bm25_results: &[SearchResult],
    vector_results: &[SearchResult],
    snippet_results: &[SearchResult],
    config: &RrfConfig,
    limit: i32,
) -> Vec<SearchResult> {
    // Accumulate scores by chunk ID
    let mut scores: HashMap<String, (f32, CodeChunk)> = HashMap::new();

    // Process BM25 results
    for (rank, result) in bm25_results.iter().enumerate() {
        let score = rrf_score(rank as i32, config.bm25_weight, config.k);
        scores
            .entry(result.chunk.id.clone())
            .and_modify(|(s, _)| *s += score)
            .or_insert((score, result.chunk.clone()));
    }

    // Process vector results
    for (rank, result) in vector_results.iter().enumerate() {
        let score = rrf_score(rank as i32, config.vector_weight, config.k);
        scores
            .entry(result.chunk.id.clone())
            .and_modify(|(s, _)| *s += score)
            .or_insert((score, result.chunk.clone()));
    }

    // Process snippet results
    for (rank, result) in snippet_results.iter().enumerate() {
        let score = rrf_score(rank as i32, config.snippet_weight, config.k);
        scores
            .entry(result.chunk.id.clone())
            .and_modify(|(s, _)| *s += score)
            .or_insert((score, result.chunk.clone()));
    }

    // Sort by fused score (descending)
    let mut results: Vec<_> = scores
        .into_iter()
        .map(|(_, (score, chunk))| SearchResult {
            chunk,
            score,
            score_type: ScoreType::Hybrid,
            is_stale: None,
        })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit as usize);
    results
}

/// Fuse only BM25 and vector results (simpler variant).
pub fn fuse_bm25_vector(
    bm25_results: &[SearchResult],
    vector_results: &[SearchResult],
    config: &RrfConfig,
    limit: i32,
) -> Vec<SearchResult> {
    fuse_results(bm25_results, vector_results, &[], config, limit)
}

/// Fuse all four sources: BM25, vector, snippet, and recent.
///
/// This is the full multi-source retrieval function.
pub fn fuse_all_results(
    bm25_results: &[SearchResult],
    vector_results: &[SearchResult],
    snippet_results: &[SearchResult],
    recent_results: &[SearchResult],
    config: &RrfConfig,
    limit: i32,
) -> Vec<SearchResult> {
    // Accumulate scores by chunk ID
    let mut scores: HashMap<String, (f32, CodeChunk)> = HashMap::new();

    // Process BM25 results
    for (rank, result) in bm25_results.iter().enumerate() {
        let score = rrf_score(rank as i32, config.bm25_weight, config.k);
        scores
            .entry(result.chunk.id.clone())
            .and_modify(|(s, _)| *s += score)
            .or_insert((score, result.chunk.clone()));
    }

    // Process vector results
    for (rank, result) in vector_results.iter().enumerate() {
        let score = rrf_score(rank as i32, config.vector_weight, config.k);
        scores
            .entry(result.chunk.id.clone())
            .and_modify(|(s, _)| *s += score)
            .or_insert((score, result.chunk.clone()));
    }

    // Process snippet results
    for (rank, result) in snippet_results.iter().enumerate() {
        let score = rrf_score(rank as i32, config.snippet_weight, config.k);
        scores
            .entry(result.chunk.id.clone())
            .and_modify(|(s, _)| *s += score)
            .or_insert((score, result.chunk.clone()));
    }

    // Process recent results (from recently edited files)
    for (rank, result) in recent_results.iter().enumerate() {
        let score = rrf_score(rank as i32, config.recent_weight, config.k);
        scores
            .entry(result.chunk.id.clone())
            .and_modify(|(s, _)| *s += score)
            .or_insert((score, result.chunk.clone()));
    }

    // Sort by fused score (descending)
    let mut results: Vec<_> = scores
        .into_iter()
        .map(|(_, (score, chunk))| SearchResult {
            chunk,
            score,
            score_type: ScoreType::Hybrid,
            is_stale: None,
        })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit as usize);
    results
}

/// Detect if a query looks like an identifier.
///
/// Returns true if the query matches patterns like:
/// - Single word with underscores: `get_user_name`
/// - CamelCase: `getUserName`, `GetUserName`
/// - Single word without spaces
pub fn is_identifier_query(query: &str) -> bool {
    let trimmed = query.trim();

    // Empty or has spaces -> not an identifier
    if trimmed.is_empty() || trimmed.contains(' ') {
        return false;
    }

    // Contains underscore -> likely snake_case identifier
    if trimmed.contains('_') {
        return true;
    }

    // Check for camelCase or PascalCase
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.is_empty() {
        return false;
    }

    // First char should be a letter
    if !chars[0].is_alphabetic() {
        return false;
    }

    // Check for mixed case (camelCase/PascalCase)
    let has_upper = chars.iter().any(|c| c.is_uppercase());
    let has_lower = chars.iter().any(|c| c.is_lowercase());

    // If has both upper and lower, it's likely camelCase/PascalCase
    if has_upper && has_lower {
        return true;
    }

    // Single word all lowercase or all uppercase is still an identifier
    chars.iter().all(|c| c.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(id: &str, score: f32, score_type: ScoreType) -> SearchResult {
        SearchResult {
            chunk: CodeChunk {
                id: id.to_string(),
                source_id: "test".to_string(),
                filepath: "test.rs".to_string(),
                language: "rust".to_string(),
                content: "test content".to_string(),
                start_line: 1,
                end_line: 1,
                embedding: None,
                modified_time: None,
                workspace: "test".to_string(),
                content_hash: String::new(),
                indexed_at: 0,
                parent_symbol: None,
                is_overview: false,
            },
            score,
            score_type,
            is_stale: None,
        }
    }

    fn make_result_with_mtime(id: &str, score: f32, mtime: Option<i64>) -> SearchResult {
        SearchResult {
            chunk: CodeChunk {
                id: id.to_string(),
                source_id: "test".to_string(),
                filepath: "test.rs".to_string(),
                language: "rust".to_string(),
                content: "test content".to_string(),
                start_line: 1,
                end_line: 1,
                embedding: None,
                modified_time: mtime,
                workspace: "test".to_string(),
                content_hash: String::new(),
                indexed_at: 0,
                parent_symbol: None,
                is_overview: false,
            },
            score,
            score_type: ScoreType::Bm25,
            is_stale: None,
        }
    }

    #[test]
    fn test_rrf_score() {
        // At rank 0 with k=60, score = weight / 60
        assert!((rrf_score(0, 1.0, 60.0) - 1.0 / 60.0).abs() < 0.001);
        // At rank 1 with k=60, score = weight / 61
        assert!((rrf_score(1, 1.0, 60.0) - 1.0 / 61.0).abs() < 0.001);
    }

    #[test]
    fn test_fuse_results() {
        let bm25 = vec![
            make_result("a", 1.0, ScoreType::Bm25),
            make_result("b", 0.8, ScoreType::Bm25),
        ];
        let vector = vec![
            make_result("b", 0.9, ScoreType::Vector),
            make_result("c", 0.7, ScoreType::Vector),
        ];

        let config = RrfConfig::default();
        let fused = fuse_results(&bm25, &vector, &[], &config, 10);

        // "b" should be ranked higher because it appears in both lists
        assert_eq!(fused.len(), 3);
        assert_eq!(fused[0].chunk.id, "b");
    }

    #[test]
    fn test_is_identifier_query() {
        // Snake case
        assert!(is_identifier_query("get_user_name"));
        assert!(is_identifier_query("MAX_SIZE"));

        // CamelCase / PascalCase
        assert!(is_identifier_query("getUserName"));
        assert!(is_identifier_query("GetUserName"));
        assert!(is_identifier_query("XMLParser"));

        // Simple identifiers
        assert!(is_identifier_query("main"));
        assert!(is_identifier_query("foo"));

        // Not identifiers
        assert!(!is_identifier_query("get user name"));
        assert!(!is_identifier_query("how to parse json"));
        assert!(!is_identifier_query(""));
        assert!(!is_identifier_query("123abc"));
    }

    #[test]
    fn test_config_for_identifier() {
        let config = RrfConfig::default().for_identifier_query();
        assert_eq!(config.snippet_weight, 0.3);
        assert!(config.snippet_weight > RrfConfig::default().snippet_weight);
    }

    #[test]
    fn test_has_symbol_syntax() {
        // type: prefix
        assert!(has_symbol_syntax("type:function"));
        assert!(has_symbol_syntax("type:class name:User"));

        // name: prefix
        assert!(has_symbol_syntax("name:parse"));
        assert!(has_symbol_syntax("find name:getUserName"));

        // file: prefix
        assert!(has_symbol_syntax("file:src/main.rs"));
        assert!(has_symbol_syntax("type:function file:lib.rs"));

        // path: prefix (alias for file:)
        assert!(has_symbol_syntax("path:src/main.rs"));
        assert!(has_symbol_syntax("path:*.rs type:struct"));

        // Not symbol syntax
        assert!(!has_symbol_syntax("parse error"));
        assert!(!has_symbol_syntax("getUserName"));
        assert!(!has_symbol_syntax("how to fix bug"));
    }

    #[test]
    fn test_config_for_symbol_query() {
        let config = RrfConfig::default().for_symbol_query();
        assert_eq!(config.snippet_weight, 0.6);
        assert_eq!(config.bm25_weight, 0.2);
        assert_eq!(config.vector_weight, 0.1);
    }

    #[test]
    fn test_recency_score_none() {
        assert!(recency_score(None, 7.0) < 0.001);
    }

    #[test]
    fn test_recency_score_now() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let score = recency_score(Some(now), 7.0);
        assert!((score - 1.0).abs() < 0.01); // Should be very close to 1.0
    }

    #[test]
    fn test_recency_score_half_life() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let seven_days_ago = now - (7 * 86400);
        let score = recency_score(Some(seven_days_ago), 7.0);
        assert!((score - 0.5).abs() < 0.01); // Should be ~0.5 after one half-life
    }

    #[test]
    fn test_recency_score_future() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let future = now + 86400;
        assert!(recency_score(Some(future), 7.0) < 0.001);
    }

    #[test]
    fn test_apply_recency_boost() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mut results = vec![
            make_result_with_mtime("old", 0.5, Some(now - 30 * 86400)), // 30 days old
            make_result_with_mtime("new", 0.5, Some(now)),              // just now
        ];

        let config = RrfConfig::default().with_recency_boost(0.1);
        apply_recency_boost(&mut results, &config);

        // New file should have higher score
        assert!(results[1].score > results[0].score);
    }

    #[test]
    fn test_recency_boost_disabled() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mut results = vec![make_result_with_mtime("test", 0.5, Some(now))];

        let config = RrfConfig::default(); // recency_boost_weight = 0.0
        let original_score = results[0].score;
        apply_recency_boost(&mut results, &config);

        assert!((results[0].score - original_score).abs() < 0.001);
    }

    #[test]
    fn test_config_with_recency_boost() {
        let config = RrfConfig::default().with_recency_boost(0.15);
        assert!((config.recency_boost_weight - 0.15).abs() < 0.001);
        assert!((config.recency_half_life_days - 7.0).abs() < 0.001);

        let config = RrfConfig::default().with_recency_boost_config(0.2, 14.0);
        assert!((config.recency_boost_weight - 0.2).abs() < 0.001);
        assert!((config.recency_half_life_days - 14.0).abs() < 0.001);
    }

    #[test]
    fn test_fuse_all_results() {
        let bm25 = vec![
            make_result("a", 1.0, ScoreType::Bm25),
            make_result("b", 0.8, ScoreType::Bm25),
        ];
        let vector = vec![
            make_result("b", 0.9, ScoreType::Vector),
            make_result("c", 0.7, ScoreType::Vector),
        ];
        let recent = vec![
            make_result("d", 0.95, ScoreType::Hybrid),
            make_result("a", 0.85, ScoreType::Hybrid),
        ];

        let config = RrfConfig::default().with_recent_weight(0.2);
        let fused = fuse_all_results(&bm25, &vector, &[], &recent, &config, 10);

        // All unique items should be present
        assert_eq!(fused.len(), 4);
        // "b" should be ranked high (appears in bm25 and vector)
        // "a" should also be high (appears in bm25 and recent)
        let ids: Vec<_> = fused.iter().map(|r| r.chunk.id.as_str()).collect();
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"b"));
        assert!(ids.contains(&"c"));
        assert!(ids.contains(&"d"));
    }

    // ========== Additional edge case tests ==========

    #[test]
    fn test_fuse_empty_inputs() {
        let config = RrfConfig::default();

        // All empty
        let fused = fuse_results(&[], &[], &[], &config, 10);
        assert!(fused.is_empty());

        // Only BM25
        let bm25 = vec![make_result("a", 1.0, ScoreType::Bm25)];
        let fused = fuse_results(&bm25, &[], &[], &config, 10);
        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].chunk.id, "a");

        // Only vector
        let vector = vec![make_result("b", 0.9, ScoreType::Vector)];
        let fused = fuse_results(&[], &vector, &[], &config, 10);
        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].chunk.id, "b");
    }

    #[test]
    fn test_fuse_limit_zero() {
        let bm25 = vec![make_result("a", 1.0, ScoreType::Bm25)];
        let config = RrfConfig::default();

        let fused = fuse_results(&bm25, &[], &[], &config, 0);
        assert!(fused.is_empty());
    }

    #[test]
    fn test_fuse_limit_smaller_than_results() {
        let bm25 = vec![
            make_result("a", 1.0, ScoreType::Bm25),
            make_result("b", 0.8, ScoreType::Bm25),
            make_result("c", 0.6, ScoreType::Bm25),
        ];
        let config = RrfConfig::default();

        let fused = fuse_results(&bm25, &[], &[], &config, 2);
        assert_eq!(fused.len(), 2);
    }

    #[test]
    fn test_rrf_score_ordering() {
        // Verify that RRF score decreases with rank
        let config = RrfConfig::default();
        let score_rank0 = rrf_score(0, config.bm25_weight, config.k);
        let score_rank1 = rrf_score(1, config.bm25_weight, config.k);
        let score_rank10 = rrf_score(10, config.bm25_weight, config.k);

        assert!(score_rank0 > score_rank1);
        assert!(score_rank1 > score_rank10);
    }

    #[test]
    fn test_fuse_duplicate_item_accumulates_score() {
        // Item appearing in multiple sources should have accumulated score
        let bm25 = vec![make_result("dup", 1.0, ScoreType::Bm25)];
        let vector = vec![make_result("dup", 0.9, ScoreType::Vector)];
        let snippet = vec![make_result("dup", 0.8, ScoreType::Hybrid)];

        let config = RrfConfig::new(0.5, 0.3, 0.2);
        let fused = fuse_results(&bm25, &vector, &snippet, &config, 10);

        assert_eq!(fused.len(), 1);

        // Score should be sum of RRF contributions from all three sources
        // rank 0 in all three: 0.5/60 + 0.3/60 + 0.2/60 = 1.0/60
        let expected_score = (0.5 + 0.3 + 0.2) / 60.0;
        assert!(
            (fused[0].score - expected_score).abs() < 0.001,
            "Expected score ~{:.4}, got {:.4}",
            expected_score,
            fused[0].score
        );
    }

    #[test]
    fn test_weight_configuration_affects_ranking() {
        // Item A appears in BM25 only, Item B appears in vector only
        let bm25 = vec![make_result("a", 1.0, ScoreType::Bm25)];
        let vector = vec![make_result("b", 0.9, ScoreType::Vector)];

        // High BM25 weight -> A should rank first
        let config_bm25 = RrfConfig::new(0.8, 0.2, 0.0);
        let fused = fuse_results(&bm25, &vector, &[], &config_bm25, 10);
        assert_eq!(fused[0].chunk.id, "a");

        // High vector weight -> B should rank first
        let config_vector = RrfConfig::new(0.2, 0.8, 0.0);
        let fused = fuse_results(&bm25, &vector, &[], &config_vector, 10);
        assert_eq!(fused[0].chunk.id, "b");
    }
}
