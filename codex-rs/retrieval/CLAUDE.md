# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Crate Overview

**codex-retrieval** - Code retrieval system providing intelligent code search for codex-rs. Combines BM25 full-text search, vector semantic search, and AST-aware symbol extraction.

**IMPORTANT:** This crate is part of the codex-rs workspace. Read `../CLAUDE.md` (or `codex/CLAUDE.md`) for workspace-wide conventions before making changes.

## Important Note

**This crate does NOT follow the `*_ext.rs` extension pattern.** Direct modifications to existing files are allowed and preferred for this directory.

## Build and Test Commands

```bash
# From codex-rs/ directory (required)
cargo build -p codex-retrieval                    # Standard build
cargo build -p codex-retrieval --features local   # With local embeddings + reranking
cargo test -p codex-retrieval                     # Run tests
cargo check -p codex-retrieval                    # Quick check

# Run CLI/TUI for testing
cargo run -p codex-retrieval --bin retrieval -- --help
cargo run -p codex-retrieval --bin retrieval -- /path/to/project              # TUI mode (default)
cargo run -p codex-retrieval --bin retrieval -- /path/to/project --no-tui build
cargo run -p codex-retrieval --bin retrieval -- /path/to/project --no-tui search "query"
```

## Feature Flags

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| `local-embeddings` | Local embeddings via fastembed (ONNX) | fastembed |
| `neural-reranker` | Local neural reranking via fastembed | fastembed |
| `local` | All local features | fastembed |

## Architecture

```
src/
├── service.rs          # RetrievalService - main entry point, cached instances
├── config.rs           # RetrievalConfig (from ~/.codex/retrieval.toml)
├── error.rs            # RetrievalErr - structured errors with context
├── traits.rs           # Core traits: Indexer, Searcher, EmbeddingProvider, ChunkStore
├── types.rs            # Core types: CodeChunk, SearchResult, SourceFileId
│
├── indexing/           # Index management
│   ├── manager.rs      # IndexManager - orchestrates rebuild/update
│   ├── walker.rs       # File walker with gitignore support
│   ├── watcher.rs      # FileWatcher for incremental updates
│   └── change_detector.rs  # SHA256 content hash change detection
│
├── chunking/           # Code splitting
│   ├── splitter.rs     # AST-aware chunking (tree-sitter) + fallback
│   └── collapser.rs    # Token budget collapsing
│
├── embeddings/         # Embedding providers
│   ├── fastembed.rs    # Local ONNX (nomic-embed-text, bge-*, MiniLM-*)
│   ├── openai.rs       # OpenAI API (text-embedding-3-small/large)
│   └── queue.rs        # Batched embedding queue
│
├── search/             # Search engines
│   ├── bm25.rs         # BM25 full-text (k1=0.8, b=0.5)
│   ├── hybrid.rs       # HybridSearcher - combines BM25 + vector + snippet
│   ├── fusion.rs       # Reciprocal Rank Fusion (RRF)
│   └── recent.rs       # RecentFilesCache for recency boost
│
├── storage/            # Persistence
│   ├── sqlite.rs       # SqliteStore - metadata, FTS5
│   ├── lancedb.rs      # LanceDbStore - vector storage
│   └── snippets.rs     # Symbol/snippet storage
│
├── query/              # Query processing
│   ├── rewriter.rs     # LLM-based query rewriting
│   ├── preprocessor.rs # Tokenization, stemming
│   └── llm_provider.rs # OpenAI/Ollama for query rewrite
│
├── reranker/           # Result reranking
│   ├── rule_based.rs   # Heuristic reranking
│   ├── local.rs        # fastembed reranker (bge-reranker, jina-reranker)
│   └── remote.rs       # Cohere/Voyage AI API
│
├── repomap/            # PageRank context generation
│   ├── graph.rs        # Dependency graph from AST
│   ├── pagerank.rs     # PageRank algorithm
│   └── renderer.rs     # Token-budgeted output
│
└── tags/               # Symbol extraction
    ├── extractor.rs    # tree-sitter-tags based
    └── languages.rs    # Language configs (Go, Rust, Python, Java)
```

## Error Handling

Uses `RetrievalErr` (not `anyhow`). Key variants:
- `NotEnabled` - retrieval not configured
- `NotReady` - index building (retryable)
- `SqliteLockedTimeout` - concurrent access (retryable)
- `EmbeddingFailed`, `SearchFailed` - operation failures

Check `is_retryable()` and `suggested_retry_delay_ms()` for transient errors.

## Key Patterns

### Trait Bounds
All async traits use `Send + Sync`:
```rust
#[async_trait]
pub trait EmbeddingProvider: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &str;
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
}
```

### Integer Types
Use `i32`/`i64` (never unsigned) per workspace convention:
```rust
pub start_line: i32,   // ✅
pub limit: i32,        // ✅
```

### Configuration Defaults
Use `#[serde(default)]` for optional fields:
```rust
#[serde(default)]
pub watch_enabled: bool,

#[serde(default = "default_batch_size")]
pub batch_size: i32,
```

### Service Caching
`RetrievalService` instances are cached per workdir with LRU eviction:
```rust
static INSTANCES: Lazy<BlockingLruCache<PathBuf, Arc<RetrievalService>>> = ...;
```

### Service API (Facade Pattern)
`RetrievalService` is the single entry point for all retrieval operations:

```rust
// Search API
service.search_with_limit(&query, Some(limit)).await?;
service.search_bm25(&query, limit).await?;
service.search_vector(&query, limit).await?;

// Operations API
service.build_index(mode, cancel_token).await?;  // Returns Receiver<IndexProgress>
service.get_index_status().await?;               // Returns IndexStats
service.start_watch(cancel_token).await?;        // Returns Receiver<WatchEvent>
service.generate_repomap(request).await?;        // Returns RepoMapResult
```

CLI and TUI both use this service API - no direct access to `IndexManager`, `SqliteStore`, or `FileWatcher`.

## Supported Languages (AST)

| Language | Symbol Extraction | Chunking |
|----------|-------------------|----------|
| Go | ✅ | ✅ |
| Rust | ✅ | ✅ |
| Python | ✅ | ✅ |
| Java | ✅ | ✅ |

TypeScript, JavaScript, C++ are NOT yet supported for AST features.

## Configuration

Config file locations (in priority order):
1. `{workdir}/.codex/retrieval.toml`
2. `~/.codex/retrieval.toml`

Key sections: `indexing`, `chunking`, `search`, `embedding`, `query_rewrite`, `extended_reranker`, `repo_map`

## Testing

```bash
# Unit tests
cargo test -p codex-retrieval

# Integration tests
cargo test -p codex-retrieval --test cli_test
cargo test -p codex-retrieval --test indexing_test
cargo test -p codex-retrieval --test vector_search_test

# With local features
cargo test -p codex-retrieval --features local
```

Test helpers use `tempfile::TempDir` for isolated test environments.
