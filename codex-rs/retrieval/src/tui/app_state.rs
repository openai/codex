//! TUI application state types.
//!
//! Contains state structures for different views in the TUI.

use super::widgets::ProgressBarState;
use super::widgets::ResultListState;
use super::widgets::SearchInputState;
use super::widgets::StatsPanelState;

/// Search state for the search view.
#[derive(Debug, Default)]
pub struct SearchState {
    /// Search input widget state.
    pub input: SearchInputState,
    /// Result list widget state.
    pub results: ResultListState,
    /// Search pipeline state.
    pub pipeline: SearchPipelineState,
    /// Current query ID.
    pub query_id: Option<String>,
    /// Search error message.
    pub error: Option<String>,
    /// Focus mode: true = input, false = results.
    pub focus_input: bool,
}

/// Index state for the index view.
#[derive(Debug, Default)]
pub struct IndexState {
    /// Progress bar state.
    pub progress: ProgressBarState,
    /// Stats panel state.
    pub stats: StatsPanelState,
    /// Is watching.
    pub watching: bool,
}

/// Search pipeline stage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SearchStage {
    /// No search in progress.
    #[default]
    Idle,
    /// Query preprocessing (tokenization, language detection).
    Preprocessing,
    /// Query rewriting (LLM-based expansion).
    QueryRewriting,
    /// BM25 full-text search.
    Bm25Search,
    /// Vector similarity search.
    VectorSearch,
    /// Symbol/snippet search.
    SnippetSearch,
    /// RRF fusion of results.
    Fusion,
    /// Reranking results.
    Reranking,
    /// Search completed.
    Complete,
    /// Search failed.
    Error,
}

impl std::fmt::Display for SearchStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchStage::Idle => write!(f, "Idle"),
            SearchStage::Preprocessing => write!(f, "Preprocess"),
            SearchStage::QueryRewriting => write!(f, "Query Rewrite"),
            SearchStage::Bm25Search => write!(f, "BM25"),
            SearchStage::VectorSearch => write!(f, "Vector"),
            SearchStage::SnippetSearch => write!(f, "Snippet"),
            SearchStage::Fusion => write!(f, "Fusion"),
            SearchStage::Reranking => write!(f, "Reranking"),
            SearchStage::Complete => write!(f, "Complete"),
            SearchStage::Error => write!(f, "Error"),
        }
    }
}

/// Search pipeline state for tracking search progress.
#[derive(Debug, Clone, Default)]
pub struct SearchPipelineState {
    /// Current stage.
    pub stage: SearchStage,
    /// Preprocessing duration.
    pub preprocess_duration_ms: Option<i64>,
    /// Query rewrite info.
    pub original_query: Option<String>,
    pub rewritten_query: Option<String>,
    pub query_expansions: Vec<String>,
    pub rewrite_duration_ms: Option<i64>,
    /// BM25 search results.
    pub bm25_count: Option<i32>,
    pub bm25_duration_ms: Option<i64>,
    /// Vector search results.
    pub vector_count: Option<i32>,
    pub vector_duration_ms: Option<i64>,
    /// Snippet search results.
    pub snippet_count: Option<i32>,
    pub snippet_duration_ms: Option<i64>,
    /// Fusion results.
    pub fusion_count: Option<i32>,
    pub fusion_duration_ms: Option<i64>,
    /// Reranking duration.
    pub rerank_duration_ms: Option<i64>,
    /// Total duration.
    pub total_duration_ms: Option<i64>,
    /// Error message if failed.
    pub error: Option<String>,
}

impl SearchPipelineState {
    /// Reset the pipeline state for a new search.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Start a new search.
    pub fn start(&mut self) {
        self.reset();
        self.stage = SearchStage::Preprocessing;
    }
}

/// RepoMap state for the repomap view.
#[derive(Debug, Default)]
pub struct RepoMapState {
    /// Token budget.
    pub max_tokens: i32,
    /// Generated content.
    pub content: Option<String>,
    /// Actual tokens used.
    pub tokens: i32,
    /// Files included.
    pub files: i32,
    /// Generation time.
    pub duration_ms: i64,
    /// Is generating.
    pub generating: bool,
    /// Scroll offset for viewing content.
    pub scroll_offset: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_stage_display() {
        assert_eq!(SearchStage::Idle.to_string(), "Idle");
        assert_eq!(SearchStage::Bm25Search.to_string(), "BM25");
        assert_eq!(SearchStage::VectorSearch.to_string(), "Vector");
        assert_eq!(SearchStage::Complete.to_string(), "Complete");
    }

    #[test]
    fn test_search_pipeline_state_reset() {
        let mut state = SearchPipelineState {
            stage: SearchStage::Complete,
            bm25_count: Some(10),
            ..Default::default()
        };
        state.reset();
        assert_eq!(state.stage, SearchStage::Idle);
        assert!(state.bm25_count.is_none());
    }

    #[test]
    fn test_search_pipeline_state_start() {
        let mut state = SearchPipelineState::default();
        state.start();
        assert_eq!(state.stage, SearchStage::Preprocessing);
    }
}
