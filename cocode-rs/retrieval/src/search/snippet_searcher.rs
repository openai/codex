//! Snippet search integration for HybridSearcher.
//!
//! Provides `SnippetSearcher` that converts symbol search results to
//! `SearchResult` for RRF fusion with BM25 and vector search.

use std::sync::Arc;

use crate::error::Result;
use crate::storage::SnippetStorage;
use crate::storage::SqliteStore;
use crate::storage::StoredSnippet;
use crate::storage::SymbolQuery;
use crate::types::CodeChunk;
use crate::types::ScoreType;
use crate::types::SearchResult;

/// Snippet searcher for symbol-based code search.
///
/// Wraps `SnippetStorage` and converts results to `SearchResult`
/// for integration with `HybridSearcher`.
pub struct SnippetSearcher {
    storage: SnippetStorage,
    workspace: String,
}

impl SnippetSearcher {
    /// Create a new snippet searcher.
    pub fn new(db: Arc<SqliteStore>, workspace: &str) -> Self {
        Self {
            storage: SnippetStorage::new(db),
            workspace: workspace.to_string(),
        }
    }

    /// Search snippets using the query string.
    ///
    /// Parses the query for symbol-specific syntax (`type:`, `name:`)
    /// and returns results as `SearchResult` with rank-based scores.
    pub async fn search(&self, query: &str, limit: i32) -> Result<Vec<SearchResult>> {
        let parsed = SymbolQuery::parse(query);

        // If empty query, return empty results
        if parsed.is_empty() {
            return Ok(Vec::new());
        }

        let snippets = self
            .storage
            .search_fts(&self.workspace, &parsed, limit)
            .await?;
        Ok(self.snippets_to_results(snippets))
    }

    /// Search snippets by name pattern only.
    pub async fn search_by_name(&self, name: &str, limit: i32) -> Result<Vec<SearchResult>> {
        let snippets = self
            .storage
            .search_by_name(&self.workspace, name, limit)
            .await?;
        Ok(self.snippets_to_results(snippets))
    }

    /// Get raw snippets for a file (for symbol outline).
    pub async fn get_file_symbols(&self, filepath: &str) -> Result<Vec<StoredSnippet>> {
        let query = SymbolQuery::for_file(filepath);
        self.storage.search_fts(&self.workspace, &query, 1000).await
    }

    /// Convert stored snippets to search results.
    fn snippets_to_results(&self, snippets: Vec<StoredSnippet>) -> Vec<SearchResult> {
        snippets
            .into_iter()
            .enumerate()
            .map(|(i, s)| self.snippet_to_result(s, i))
            .collect()
    }

    /// Convert a single snippet to SearchResult.
    fn snippet_to_result(&self, snippet: StoredSnippet, rank: usize) -> SearchResult {
        // Use signature or name as content
        let content = snippet
            .signature
            .clone()
            .unwrap_or_else(|| snippet.name.clone());

        // Generate unique chunk ID
        let chunk_id = format!(
            "snippet:{}:{}:{}",
            snippet.workspace, snippet.filepath, snippet.start_line
        );

        // Detect language from filepath
        let language = detect_language_from_path(&snippet.filepath);

        SearchResult {
            chunk: CodeChunk {
                id: chunk_id,
                source_id: snippet.workspace.clone(),
                filepath: snippet.filepath,
                language,
                content,
                start_line: snippet.start_line,
                end_line: snippet.end_line,
                embedding: None,
                modified_time: None,
                workspace: snippet.workspace,
                content_hash: snippet.content_hash,
                indexed_at: 0,
                parent_symbol: None, // TODO: Extract from TagExtractor
                is_overview: false,  // Snippets are not overview chunks
            },
            // Rank-based score (1.0, 0.5, 0.33, ...)
            score: 1.0 / (rank as f32 + 1.0),
            score_type: ScoreType::Snippet,
            is_stale: None, // Not hydrated yet
        }
    }

    /// Check if a query should use snippet search.
    ///
    /// Returns true if the query contains symbol-specific syntax.
    pub fn should_use_snippet_search(query: &str) -> bool {
        let parsed = SymbolQuery::parse(query);
        parsed.is_symbol_query()
    }
}

/// Detect programming language from file extension.
fn detect_language_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|ext| match ext {
            "rs" => "rust",
            "go" => "go",
            "py" => "python",
            "java" => "java",
            "ts" | "tsx" => "typescript",
            "js" | "jsx" => "javascript",
            _ => ext,
        })
        .unwrap_or("text")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_test_snippet(id: i64, name: &str, syntax_type: &str) -> StoredSnippet {
        StoredSnippet {
            id,
            workspace: "test".to_string(),
            filepath: "src/main.rs".to_string(),
            name: name.to_string(),
            syntax_type: syntax_type.to_string(),
            start_line: (id * 10) as i32,
            end_line: (id * 10 + 5) as i32,
            signature: Some(format!("fn {}()", name)),
            docs: None,
            content_hash: "abc123".to_string(),
        }
    }

    #[test]
    fn test_snippet_to_result() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Arc::new(SqliteStore::open(&db_path).unwrap());
        let searcher = SnippetSearcher::new(store, "test");

        let snippet = make_test_snippet(1, "parse_config", "function");
        let result = searcher.snippet_to_result(snippet, 0);

        assert_eq!(result.chunk.filepath, "src/main.rs");
        assert_eq!(result.chunk.content, "fn parse_config()");
        assert_eq!(result.chunk.start_line, 10);
        assert_eq!(result.chunk.language, "rust");
        assert_eq!(result.score, 1.0);
        assert_eq!(result.score_type, ScoreType::Snippet);
    }

    #[test]
    fn test_rank_based_scores() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Arc::new(SqliteStore::open(&db_path).unwrap());
        let searcher = SnippetSearcher::new(store, "test");

        let snippets = vec![
            make_test_snippet(1, "first", "function"),
            make_test_snippet(2, "second", "function"),
            make_test_snippet(3, "third", "function"),
        ];

        let results = searcher.snippets_to_results(snippets);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].score, 1.0); // rank 0 -> 1.0
        assert_eq!(results[1].score, 0.5); // rank 1 -> 0.5
        assert!((results[2].score - 0.333).abs() < 0.01); // rank 2 -> ~0.33
    }

    #[test]
    fn test_should_use_snippet_search() {
        assert!(SnippetSearcher::should_use_snippet_search("type:function"));
        assert!(SnippetSearcher::should_use_snippet_search("name:parse"));
        assert!(SnippetSearcher::should_use_snippet_search(
            "type:class name:User"
        ));
        assert!(!SnippetSearcher::should_use_snippet_search("parse error"));
        assert!(!SnippetSearcher::should_use_snippet_search("getUserName"));
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language_from_path("src/main.rs"), "rust");
        assert_eq!(detect_language_from_path("pkg/server.go"), "go");
        assert_eq!(detect_language_from_path("app.py"), "python");
        assert_eq!(detect_language_from_path("App.tsx"), "typescript");
        assert_eq!(detect_language_from_path("unknown.xyz"), "xyz");
        assert_eq!(detect_language_from_path("no_extension"), "text");
    }
}
