//! Jaccard similarity ranking for search results.
//!
//! Provides symbol-level similarity calculation for better ranking of code search results.
//! Reference: Continue `core/autocomplete/context/ranking/index.ts:6-36`

use std::collections::HashSet;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::types::SearchResult;

/// Regex for splitting code into symbols (tokens).
/// Splits on whitespace and common punctuation.
static SYMBOL_SPLIT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"[\s.,/#!$%^&*;:{}=\-_`~()\[\]<>"'\\|+@?]+"#).unwrap());

/// Extract symbols (tokens) from a code snippet.
///
/// Splits the text on whitespace and punctuation, converting to lowercase.
/// Returns a set of unique symbols.
pub fn extract_symbols(text: &str) -> HashSet<String> {
    SYMBOL_SPLIT
        .split(text)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

/// Calculate Jaccard similarity between two text snippets.
///
/// Jaccard similarity = |A ∩ B| / |A ∪ B|
///
/// Returns a value between 0.0 (no overlap) and 1.0 (identical symbols).
pub fn jaccard_similarity(a: &str, b: &str) -> f32 {
    let set_a = extract_symbols(a);
    let set_b = extract_symbols(b);

    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();

    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

/// Boost results based on Jaccard similarity with the query.
///
/// Adds `similarity * boost_factor` to each result's score.
pub fn apply_jaccard_boost(results: &mut [SearchResult], query: &str, boost_factor: f32) {
    for result in results.iter_mut() {
        let similarity = jaccard_similarity(query, &result.chunk.content);
        result.score += similarity * boost_factor;
    }
}

/// Re-rank results by Jaccard similarity.
///
/// Useful for tie-breaking when scores are similar.
pub fn rerank_by_jaccard(results: &mut [SearchResult], query: &str) {
    let query_symbols = extract_symbols(query);

    // Sort by: (original_score, jaccard_similarity) descending
    results.sort_by(|a, b| {
        let sim_a = jaccard_with_set(&a.chunk.content, &query_symbols);
        let sim_b = jaccard_with_set(&b.chunk.content, &query_symbols);

        // Compare by score first, then by Jaccard similarity
        match b.score.partial_cmp(&a.score) {
            Some(std::cmp::Ordering::Equal) => sim_b
                .partial_cmp(&sim_a)
                .unwrap_or(std::cmp::Ordering::Equal),
            Some(ord) => ord,
            None => std::cmp::Ordering::Equal,
        }
    });
}

/// Calculate Jaccard similarity with a pre-computed symbol set.
fn jaccard_with_set(text: &str, query_symbols: &HashSet<String>) -> f32 {
    let text_symbols = extract_symbols(text);

    let intersection = text_symbols.intersection(query_symbols).count();
    let union = text_symbols.union(query_symbols).count();

    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CodeChunk;
    use crate::types::ScoreType;

    #[test]
    fn test_extract_symbols() {
        let symbols = extract_symbols("fn get_user_name(id: i32) -> String");
        assert!(symbols.contains("fn"));
        assert!(symbols.contains("get"));
        assert!(symbols.contains("user"));
        assert!(symbols.contains("name"));
        assert!(symbols.contains("id"));
        assert!(symbols.contains("i32"));
        assert!(symbols.contains("string"));
    }

    #[test]
    fn test_extract_symbols_code() {
        let code = "let result = calculate_sum(a, b);";
        let symbols = extract_symbols(code);
        assert!(symbols.contains("let"));
        assert!(symbols.contains("result"));
        assert!(symbols.contains("calculate"));
        assert!(symbols.contains("sum"));
        assert!(symbols.contains("a"));
        assert!(symbols.contains("b"));
    }

    #[test]
    fn test_jaccard_identical() {
        let similarity = jaccard_similarity("hello world", "hello world");
        assert!((similarity - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_no_overlap() {
        let similarity = jaccard_similarity("hello world", "foo bar");
        assert!(similarity < 0.001);
    }

    #[test]
    fn test_jaccard_partial_overlap() {
        // "hello world" -> {hello, world}
        // "hello foo" -> {hello, foo}
        // intersection = {hello}, union = {hello, world, foo}
        // similarity = 1/3 = 0.333...
        let similarity = jaccard_similarity("hello world", "hello foo");
        assert!((similarity - 1.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_jaccard_empty() {
        let similarity = jaccard_similarity("", "");
        assert!(similarity < 0.001);

        let similarity = jaccard_similarity("hello", "");
        assert!(similarity < 0.001);
    }

    #[test]
    fn test_jaccard_case_insensitive() {
        let similarity = jaccard_similarity("Hello World", "hello world");
        assert!((similarity - 1.0).abs() < 0.001);
    }

    fn make_result(content: &str, score: f32) -> SearchResult {
        SearchResult {
            chunk: CodeChunk {
                id: "test".to_string(),
                source_id: "test".to_string(),
                filepath: "test.rs".to_string(),
                language: "rust".to_string(),
                content: content.to_string(),
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
            score_type: ScoreType::Bm25,
            is_stale: None,
        }
    }

    #[test]
    fn test_apply_jaccard_boost() {
        let mut results = vec![
            make_result("fn get_user_name()", 0.5),
            make_result("fn calculate_sum()", 0.5),
        ];

        apply_jaccard_boost(&mut results, "get user", 0.1);

        // First result should have higher score (more overlap with query)
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn test_rerank_by_jaccard() {
        let mut results = vec![
            make_result("fn calculate_sum(a, b)", 0.5),
            make_result("fn get_user_name(id)", 0.5), // same score
        ];

        rerank_by_jaccard(&mut results, "get user name");

        // Second result (get_user_name) should be first after reranking
        assert!(results[0].chunk.content.contains("get_user_name"));
    }
}
