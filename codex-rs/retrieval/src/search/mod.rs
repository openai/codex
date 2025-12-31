//! Search module.
//!
//! Provides BM25, vector, hybrid search, and recently edited files retrieval.

pub mod bm25;
pub mod bm25_index;
pub mod code_tokenizer;
pub mod constants;
pub mod dedup;
pub mod fusion;
pub mod hybrid;
pub mod ranking;
pub mod recent;
pub mod snippet_searcher;

pub use bm25::Bm25Searcher;
pub use dedup::deduplicate_results;
pub use dedup::deduplicate_with_threshold;
pub use dedup::limit_chunks_per_file;
pub use fusion::RrfConfig;
pub use fusion::apply_recency_boost;
pub use fusion::fuse_all_results;
pub use fusion::fuse_bm25_vector;
pub use fusion::fuse_results;
pub use fusion::has_symbol_syntax;
pub use fusion::is_identifier_query;
pub use fusion::recency_score;
pub use hybrid::HybridSearcher;
pub use ranking::apply_jaccard_boost;
pub use ranking::extract_symbols;
pub use ranking::jaccard_similarity;
pub use ranking::rerank_by_jaccard;
pub use recent::RecentFilesCache;
pub use snippet_searcher::SnippetSearcher;

pub use bm25_index::Bm25Config;
pub use bm25_index::Bm25Index;
pub use bm25_index::Bm25Metadata;
pub use bm25_index::SparseEmbedding;
pub use code_tokenizer::CodeTokenizer;
