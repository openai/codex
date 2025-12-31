//! Codex Retrieval System
//!
//! Code retrieval system providing intelligent code search capabilities for codex-rs.
//!
//! ## Features
//!
//! | Feature | Description | Config Key | Default |
//! |---------|-------------|------------|---------|
//! | **BM25 Full-text Search** | Keyword matching, high performance | `code_search` | Off |
//! | **Vector Semantic Search** | Embedding similarity | `vector_search` | Off |
//! | **Query Rewrite** | CN/EN bilingual translation | `query_rewrite` | Off |
//! | **AST Tag Extraction** | Go/Rust/Python/Java symbol extraction | - | On |
//! | **Incremental Update** | Content hash (SHA256) change detection | - | On |
//!
//! ## Quick Start
//!
//! ```toml
//! [features]
//! code_search = true
//!
//! [retrieval]
//! enabled = true
//! data_dir = "~/.codex/retrieval"
//! ```

// Core modules
pub mod config;
pub mod error;
pub mod event_emitter;
pub mod events;
pub mod metrics;
pub mod service;
pub mod traits;
pub mod types;

// Subsystems
pub mod chunking;
pub mod embeddings;
pub mod health;
pub mod indexing;
pub mod query;
pub mod repomap;
pub mod reranker;
pub mod search;
pub mod storage;
pub mod tags;
pub mod tui;

// Re-exports
pub use config::RerankerConfig;
pub use config::RetrievalConfig;
pub use error::Result;
pub use error::RetrievalErr;
pub use metrics::CodeMetrics;
pub use metrics::is_valid_file;
pub use reranker::Reranker;
pub use reranker::RuleBasedReranker;
pub use reranker::RuleBasedRerankerConfig;
pub use search::HybridSearcher;
pub use search::RecentFilesCache;
pub use search::SnippetSearcher;
pub use search::has_symbol_syntax;
pub use service::RetrievalFeatures;
pub use service::RetrievalService;
pub use storage::SnippetStorage;
pub use storage::SqliteStore;
pub use storage::StoredSnippet;
pub use storage::SymbolQuery;
pub use types::ChunkRef;
pub use types::CodeChunk;
pub use types::HydratedChunk;
pub use types::ScoreType;
pub use types::SearchQuery;
pub use types::SearchResult;
pub use types::SourceFileId;
pub use types::calculate_n_final;
pub use types::compute_chunk_hash;
pub use types::detect_language;
pub use types::wrap_content_for_embedding;

// Indexing exports
pub use indexing::FileWatcher;
pub use indexing::IndexManager;
pub use indexing::IndexProgress;
pub use indexing::IndexStats;
pub use indexing::RebuildMode;
pub use indexing::WatchEvent;
pub use indexing::WatchEventKind;

// Repo map exports
pub use repomap::RankedFile;
pub use repomap::RankedSymbol;
pub use repomap::RepoMapRequest;
pub use repomap::RepoMapResult;
pub use repomap::RepoMapService;

// Event system exports
pub use event_emitter::EventEmitter;
pub use event_emitter::ScopedEventCollector;
pub use event_emitter::emit;
pub use event_emitter::subscribe;
pub use events::EventConsumer;
pub use events::FileChangeKind;
pub use events::JsonLinesConsumer;
pub use events::LoggingConsumer;
pub use events::RetrievalEvent;
pub use events::SearchMode;
pub use events::SearchResultSummary;
