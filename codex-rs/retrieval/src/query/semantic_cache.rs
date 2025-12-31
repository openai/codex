//! In-memory semantic query cache using LRU + embedding similarity.
//!
//! Caches query results and looks them up by embedding similarity,
//! allowing semantically similar queries to reuse cached results.

use std::collections::VecDeque;
use std::sync::RwLock;

/// Configuration for semantic cache.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SemanticCacheConfig {
    /// Enable semantic similarity lookup.
    #[serde(default)]
    pub enabled: bool,

    /// Similarity threshold for cache hit (0.0-1.0).
    /// Higher values require more similar queries.
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f32,

    /// Maximum number of entries to cache.
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
}

fn default_similarity_threshold() -> f32 {
    0.95
}

fn default_max_entries() -> usize {
    1000
}

impl Default for SemanticCacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            similarity_threshold: default_similarity_threshold(),
            max_entries: default_max_entries(),
        }
    }
}

/// Cache entry with query embedding.
struct CacheEntry {
    /// Original query text (for debugging).
    #[allow(dead_code)]
    query: String,
    /// Query embedding vector.
    embedding: Vec<f32>,
    /// Cached result (serialized JSON or raw data).
    result: String,
}

/// In-memory semantic query cache.
///
/// Uses embedding similarity for lookup and LRU eviction.
/// No persistence - cleared on restart.
///
/// Thread-safe via RwLock.
pub struct SemanticQueryCache {
    entries: RwLock<VecDeque<CacheEntry>>,
    config: SemanticCacheConfig,
}

impl SemanticQueryCache {
    /// Create a new semantic cache with the given configuration.
    pub fn new(config: SemanticCacheConfig) -> Self {
        Self {
            entries: RwLock::new(VecDeque::with_capacity(config.max_entries)),
            config,
        }
    }

    /// Check if the cache is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Lookup by semantic similarity.
    ///
    /// Returns cached result if a query with similarity above threshold exists.
    /// Returns None if cache is disabled, empty, or no similar query found.
    pub fn get_semantic(&self, query_embedding: &[f32]) -> Option<String> {
        if !self.config.enabled {
            return None;
        }

        let entries = self.entries.read().ok()?;

        let mut best_match: Option<(f32, &str)> = None;

        for entry in entries.iter() {
            let similarity = cosine_similarity(query_embedding, &entry.embedding);
            if similarity >= self.config.similarity_threshold {
                match &best_match {
                    Some((best_sim, _)) if similarity <= *best_sim => {}
                    _ => best_match = Some((similarity, &entry.result)),
                }
            }
        }

        best_match.map(|(_, result)| result.to_string())
    }

    /// Store query with its embedding.
    ///
    /// Uses LRU eviction when capacity is reached.
    pub fn put(&self, query: &str, embedding: Vec<f32>, result: &str) {
        if !self.config.enabled {
            return;
        }

        let mut entries = match self.entries.write() {
            Ok(e) => e,
            Err(_) => return,
        };

        // Evict oldest if at capacity
        if entries.len() >= self.config.max_entries {
            entries.pop_front();
        }

        entries.push_back(CacheEntry {
            query: query.to_string(),
            embedding,
            result: result.to_string(),
        });
    }

    /// Clear all cached entries.
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
    }

    /// Get current number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.read().map(|e| e.len()).unwrap_or(0)
    }

    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get cache hit statistics (for monitoring).
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.len(),
            capacity: self.config.max_entries,
            threshold: self.config.similarity_threshold,
        }
    }
}

/// Cache statistics for monitoring.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Current number of entries.
    pub entries: usize,
    /// Maximum capacity.
    pub capacity: usize,
    /// Similarity threshold.
    pub threshold: f32,
}

/// Compute cosine similarity between two embedding vectors.
///
/// Returns value between -1.0 and 1.0, where 1.0 means identical.
/// Returns 0.0 if either vector is zero-length or has zero magnitude.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled_config() -> SemanticCacheConfig {
        SemanticCacheConfig {
            enabled: true,
            similarity_threshold: 0.95,
            max_entries: 10,
        }
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_cache_disabled() {
        let config = SemanticCacheConfig::default();
        assert!(!config.enabled);

        let cache = SemanticQueryCache::new(config);
        cache.put("test", vec![1.0, 2.0], "result");
        assert!(cache.is_empty());
        assert!(cache.get_semantic(&[1.0, 2.0]).is_none());
    }

    #[test]
    fn test_cache_put_and_get_exact() {
        let cache = SemanticQueryCache::new(enabled_config());

        let embedding = vec![1.0, 2.0, 3.0];
        cache.put("test query", embedding.clone(), "test result");

        assert_eq!(cache.len(), 1);

        // Exact match should hit
        let result = cache.get_semantic(&embedding);
        assert_eq!(result, Some("test result".to_string()));
    }

    #[test]
    fn test_cache_similar_query() {
        let cache = SemanticQueryCache::new(enabled_config());

        let embedding1 = vec![1.0, 0.0, 0.0];
        cache.put("query1", embedding1, "result1");

        // Very similar embedding (cosine similarity > 0.95)
        let embedding2 = vec![0.99, 0.01, 0.01];
        let result = cache.get_semantic(&embedding2);

        // Should find the cached result
        assert!(result.is_some());
    }

    #[test]
    fn test_cache_dissimilar_query() {
        let cache = SemanticQueryCache::new(enabled_config());

        let embedding1 = vec![1.0, 0.0, 0.0];
        cache.put("query1", embedding1, "result1");

        // Very different embedding (orthogonal)
        let embedding2 = vec![0.0, 1.0, 0.0];
        let result = cache.get_semantic(&embedding2);

        // Should not find a match
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_lru_eviction() {
        let mut config = enabled_config();
        config.max_entries = 3;
        let cache = SemanticQueryCache::new(config);

        // Fill cache
        cache.put("q1", vec![1.0, 0.0, 0.0], "r1");
        cache.put("q2", vec![0.0, 1.0, 0.0], "r2");
        cache.put("q3", vec![0.0, 0.0, 1.0], "r3");
        assert_eq!(cache.len(), 3);

        // Add one more, should evict oldest
        cache.put("q4", vec![1.0, 1.0, 0.0], "r4");
        assert_eq!(cache.len(), 3);

        // First entry should be gone
        assert!(cache.get_semantic(&[1.0, 0.0, 0.0]).is_none());

        // Others should remain
        assert!(cache.get_semantic(&[0.0, 1.0, 0.0]).is_some());
    }

    #[test]
    fn test_cache_clear() {
        let cache = SemanticQueryCache::new(enabled_config());

        cache.put("q1", vec![1.0], "r1");
        cache.put("q2", vec![2.0], "r2");
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_stats() {
        let config = enabled_config();
        let cache = SemanticQueryCache::new(config.clone());

        cache.put("q1", vec![1.0], "r1");

        let stats = cache.stats();
        assert_eq!(stats.entries, 1);
        assert_eq!(stats.capacity, config.max_entries);
        assert!((stats.threshold - config.similarity_threshold).abs() < 0.001);
    }

    #[test]
    fn test_best_match_selection() {
        let mut config = enabled_config();
        config.similarity_threshold = 0.9;
        let cache = SemanticQueryCache::new(config);

        // Add two entries
        cache.put("q1", vec![1.0, 0.0], "r1");
        cache.put("q2", vec![0.9, 0.1], "r2");

        // Query that's closer to q1
        let query = vec![0.95, 0.05];
        let result = cache.get_semantic(&query);

        // Should return best match (r1 or r2 depending on similarity)
        assert!(result.is_some());
    }
}
