//! Result deduplication for search results.
//!
//! Merges overlapping code chunks from the same file, keeping the highest score.
//! Reference: Continue `core/autocomplete/context/ranking/index.ts:70-131`

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::types::SearchResult;

/// Deduplicate search results by handling overlapping chunks from the same file.
///
/// # Algorithm
/// 1. Group results by filepath
/// 2. Sort each group by start_line
/// 3. For overlapping ranges (where prev.end_line >= next.start_line):
///    - Keep the chunk with larger line coverage (more complete code)
///    - If same coverage, keep the one with higher score
///    - Extend the kept chunk's range to cover both
/// 4. Re-sort by score descending
///
/// # Design Decision
/// We don't attempt to merge content from overlapping chunks because:
/// - Line-based content merging is error-prone and may corrupt code structure
/// - Keeping complete chunks ensures syntactic integrity
/// - The chunk with larger coverage typically contains more context
pub fn deduplicate_results(results: Vec<SearchResult>) -> Vec<SearchResult> {
    if results.is_empty() {
        return results;
    }

    // 1. Group by filepath
    let mut groups: HashMap<String, Vec<SearchResult>> = HashMap::new();
    for r in results {
        groups.entry(r.chunk.filepath.clone()).or_default().push(r);
    }

    // 2. Sort and merge each group
    let mut merged = Vec::new();
    for (_, mut group) in groups {
        if group.is_empty() {
            continue;
        }

        // Sort by start_line
        group.sort_by_key(|r| r.chunk.start_line);

        let mut current = group.remove(0);
        for next in group {
            if current.chunk.end_line >= next.chunk.start_line {
                // Overlapping chunks detected
                let current_lines = current.chunk.end_line - current.chunk.start_line + 1;
                let next_lines = next.chunk.end_line - next.chunk.start_line + 1;

                // Calculate merged range
                let merged_start = current.chunk.start_line.min(next.chunk.start_line);
                let merged_end = current.chunk.end_line.max(next.chunk.end_line);

                // Decide which chunk's content to keep:
                // - Prefer larger coverage (more complete code)
                // - If equal coverage, prefer higher score
                let keep_next = if next_lines > current_lines {
                    true
                } else if next_lines == current_lines {
                    next.score > current.score
                } else {
                    false
                };

                // Get the max score before potentially moving next
                let max_score = current.score.max(next.score);

                if keep_next {
                    // Use next's content but extend line range to cover both
                    current = next;
                }

                // Extend range to cover both chunks
                current.chunk.start_line = merged_start;
                current.chunk.end_line = merged_end;
                current.score = max_score;
            } else {
                merged.push(current);
                current = next;
            }
        }
        merged.push(current);
    }

    // 3. Re-sort by score descending
    merged.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));

    merged
}
/// Deduplicate results with a configurable overlap threshold.
///
/// Two chunks are considered overlapping if they share at least `min_overlap_lines` lines.
pub fn deduplicate_with_threshold(
    results: Vec<SearchResult>,
    min_overlap_lines: i32,
) -> Vec<SearchResult> {
    if results.is_empty() || min_overlap_lines < 1 {
        return results;
    }

    // Group by filepath
    let mut groups: HashMap<String, Vec<SearchResult>> = HashMap::new();
    for r in results {
        groups.entry(r.chunk.filepath.clone()).or_default().push(r);
    }

    let mut merged = Vec::new();
    for (_, mut group) in groups {
        if group.is_empty() {
            continue;
        }

        group.sort_by_key(|r| r.chunk.start_line);

        let mut current = group.remove(0);
        for next in group {
            // Calculate overlap
            let overlap_start = current.chunk.start_line.max(next.chunk.start_line);
            let overlap_end = current.chunk.end_line.min(next.chunk.end_line);
            let overlap_lines = (overlap_end - overlap_start + 1).max(0);

            if overlap_lines >= min_overlap_lines {
                // Merge
                current.chunk.end_line = current.chunk.end_line.max(next.chunk.end_line);
                current.score = current.score.max(next.score);
            } else {
                merged.push(current);
                current = next;
            }
        }
        merged.push(current);
    }

    merged.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));

    merged
}

/// Limit the number of chunks per file to ensure result diversity.
///
/// This prevents a single highly-relevant file from dominating all search results,
/// which improves context diversity for LLM consumption.
///
/// Reference: Tabby's `services/code.rs` (max 2 chunks per file)
///
/// # Arguments
/// * `results` - Search results (should already be sorted by score descending)
/// * `max_per_file` - Maximum number of chunks allowed per file
///
/// # Returns
/// Filtered results with at most `max_per_file` chunks per file,
/// maintaining the original score order.
pub fn limit_chunks_per_file(results: Vec<SearchResult>, max_per_file: usize) -> Vec<SearchResult> {
    if max_per_file == 0 {
        return Vec::new();
    }

    let mut counts: HashMap<String, usize> = HashMap::new();
    results
        .into_iter()
        .filter(|r| {
            let count = counts.entry(r.chunk.filepath.clone()).or_insert(0);
            if *count < max_per_file {
                *count += 1;
                true
            } else {
                false
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CodeChunk;
    use crate::types::ScoreType;

    fn make_result(filepath: &str, start: i32, end: i32, score: f32) -> SearchResult {
        SearchResult {
            chunk: CodeChunk {
                id: format!("{}:{}-{}", filepath, start, end),
                source_id: "test".to_string(),
                filepath: filepath.to_string(),
                language: "rust".to_string(),
                content: format!("content from line {} to {}", start, end),
                start_line: start,
                end_line: end,
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
    fn test_empty_results() {
        let results = deduplicate_results(vec![]);
        assert!(results.is_empty());
    }

    #[test]
    fn test_no_overlap() {
        let results = vec![
            make_result("a.rs", 1, 10, 0.9),
            make_result("a.rs", 20, 30, 0.8),
            make_result("b.rs", 1, 10, 0.7),
        ];
        let deduped = deduplicate_results(results);
        assert_eq!(deduped.len(), 3);
    }

    #[test]
    fn test_overlapping_same_file() {
        let results = vec![
            make_result("a.rs", 1, 10, 0.5),
            make_result("a.rs", 5, 15, 0.9),  // overlaps with first
            make_result("a.rs", 12, 20, 0.7), // overlaps with merged
        ];
        let deduped = deduplicate_results(results);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].chunk.start_line, 1);
        assert_eq!(deduped[0].chunk.end_line, 20);
        assert_eq!(deduped[0].score, 0.9); // max score
    }

    #[test]
    fn test_different_files_not_merged() {
        let results = vec![
            make_result("a.rs", 1, 10, 0.9),
            make_result("b.rs", 1, 10, 0.8), // same range, different file
        ];
        let deduped = deduplicate_results(results);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn test_sorted_by_score() {
        let results = vec![
            make_result("a.rs", 1, 10, 0.5),
            make_result("b.rs", 1, 10, 0.9),
            make_result("c.rs", 1, 10, 0.7),
        ];
        let deduped = deduplicate_results(results);
        assert_eq!(deduped.len(), 3);
        assert!(deduped[0].score >= deduped[1].score);
        assert!(deduped[1].score >= deduped[2].score);
    }

    #[test]
    fn test_overlap_threshold() {
        let results = vec![
            make_result("a.rs", 1, 10, 0.9),
            make_result("a.rs", 9, 20, 0.8), // 2 lines overlap (9-10)
        ];

        // With threshold 1, should merge
        let deduped = deduplicate_with_threshold(results.clone(), 1);
        assert_eq!(deduped.len(), 1);

        // With threshold 5, should not merge
        let deduped = deduplicate_with_threshold(results, 5);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn test_limit_chunks_per_file_empty() {
        let results = limit_chunks_per_file(vec![], 2);
        assert!(results.is_empty());
    }

    #[test]
    fn test_limit_chunks_per_file_under_limit() {
        // 2 chunks from same file, limit is 3 -> all kept
        let results = vec![
            make_result("a.rs", 1, 10, 0.9),
            make_result("a.rs", 20, 30, 0.8),
        ];
        let limited = limit_chunks_per_file(results, 3);
        assert_eq!(limited.len(), 2);
    }

    #[test]
    fn test_limit_chunks_per_file_at_limit() {
        // 3 chunks from same file, limit is 2 -> only first 2 kept
        let results = vec![
            make_result("a.rs", 1, 10, 0.9),
            make_result("a.rs", 20, 30, 0.8),
            make_result("a.rs", 40, 50, 0.7),
        ];
        let limited = limit_chunks_per_file(results, 2);
        assert_eq!(limited.len(), 2);
        assert_eq!(limited[0].score, 0.9); // highest score kept
        assert_eq!(limited[1].score, 0.8); // second highest kept
    }

    #[test]
    fn test_limit_chunks_per_file_multiple_files() {
        // 3 chunks from a.rs, 2 from b.rs, limit 2 -> 2 from each
        let results = vec![
            make_result("a.rs", 1, 10, 0.95),
            make_result("b.rs", 1, 10, 0.9),
            make_result("a.rs", 20, 30, 0.85),
            make_result("b.rs", 20, 30, 0.8),
            make_result("a.rs", 40, 50, 0.75), // should be filtered out
        ];
        let limited = limit_chunks_per_file(results, 2);
        assert_eq!(limited.len(), 4);

        // Check files
        let a_count = limited
            .iter()
            .filter(|r| r.chunk.filepath == "a.rs")
            .count();
        let b_count = limited
            .iter()
            .filter(|r| r.chunk.filepath == "b.rs")
            .count();
        assert_eq!(a_count, 2);
        assert_eq!(b_count, 2);
    }

    #[test]
    fn test_limit_chunks_per_file_zero_limit() {
        let results = vec![make_result("a.rs", 1, 10, 0.9)];
        let limited = limit_chunks_per_file(results, 0);
        assert!(limited.is_empty());
    }

    #[test]
    fn test_limit_chunks_preserves_order() {
        // Results should maintain original order (by score)
        let results = vec![
            make_result("a.rs", 1, 10, 0.9),
            make_result("b.rs", 1, 10, 0.8),
            make_result("a.rs", 20, 30, 0.7),
            make_result("c.rs", 1, 10, 0.6),
        ];
        let limited = limit_chunks_per_file(results, 2);
        assert_eq!(limited.len(), 4);
        assert_eq!(limited[0].score, 0.9);
        assert_eq!(limited[1].score, 0.8);
        assert_eq!(limited[2].score, 0.7);
        assert_eq!(limited[3].score, 0.6);
    }

    /// Helper to create a result with specific content.
    fn make_result_with_content(
        filepath: &str,
        start: i32,
        end: i32,
        score: f32,
        content: &str,
    ) -> SearchResult {
        SearchResult {
            chunk: CodeChunk {
                id: format!("{}:{}-{}", filepath, start, end),
                source_id: "test".to_string(),
                filepath: filepath.to_string(),
                language: "rust".to_string(),
                content: content.to_string(),
                start_line: start,
                end_line: end,
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
    fn test_dedup_keeps_larger_chunk_content() {
        // When chunks overlap, we should keep the larger chunk's content
        // to preserve code integrity
        let small_chunk = make_result_with_content(
            "a.rs",
            5,
            10,
            0.9,
            "fn foo() {}", // 6 lines
        );
        let large_chunk = make_result_with_content(
            "a.rs",
            1,
            15,
            0.8,
            "// header\nfn foo() {\n  bar();\n}\nfn baz() {}", // 15 lines
        );

        let results = vec![small_chunk, large_chunk];
        let deduped = deduplicate_results(results);

        assert_eq!(deduped.len(), 1);
        // Should keep the larger chunk's content
        assert!(deduped[0].chunk.content.contains("// header"));
        assert!(deduped[0].chunk.content.contains("fn baz()"));
        // But should have max score
        assert_eq!(deduped[0].score, 0.9);
        // And merged range
        assert_eq!(deduped[0].chunk.start_line, 1);
        assert_eq!(deduped[0].chunk.end_line, 15);
    }

    #[test]
    fn test_dedup_equal_size_keeps_higher_score_content() {
        // When chunks have equal line coverage, prefer higher score content
        let high_score = make_result_with_content("a.rs", 1, 10, 0.9, "fn high_score_content() {}");
        let low_score = make_result_with_content("a.rs", 5, 14, 0.7, "fn low_score_content() {}");

        let results = vec![high_score, low_score];
        let deduped = deduplicate_results(results);

        assert_eq!(deduped.len(), 1);
        // Should keep high score content when same size
        assert!(deduped[0].chunk.content.contains("high_score_content"));
        assert_eq!(deduped[0].score, 0.9);
    }
}
