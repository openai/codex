# Codex Retrieval System

Code retrieval system providing intelligent code search for codex-rs.

## Overview

The retrieval module provides:
- **Hybrid Search**: Combines BM25 full-text and vector semantic search
- **Local-First**: Supports local models via fastembed (ONNX) and Ollama
- **AST-Aware**: Tree-sitter based code chunking and symbol extraction
- **Incremental Indexing**: Content-hash based change detection
- **Repo Map**: PageRank-based context generation (inspired by Aider)

## Architecture

```
retrieval/src/
├── service.rs          # Main RetrievalService entry point
├── config.rs           # RetrievalConfig
├── indexing/           # File walking, change detection, IndexManager
├── chunking/           # Code splitting, AST-aware chunking
├── embeddings/         # Embedding providers (fastembed, OpenAI)
├── search/             # BM25, vector search, hybrid fusion
├── storage/            # SQLite (metadata), LanceDB (vectors)
├── tags/               # tree-sitter-tags symbol extraction
├── query/              # Query preprocessing, LLM rewriting
├── reranker/           # Rule-based and neural reranking
└── repomap/            # PageRank-based context generation
```

## Supported Languages (AST)

| Language | Symbol Extraction | Chunking |
|----------|-------------------|----------|
| Go | ✅ | ✅ |
| Rust | ✅ | ✅ |
| Python | ✅ | ✅ |
| Java | ✅ | ✅ |

*TypeScript, JavaScript, C++ are NOT yet supported.*

## Search Algorithms

### BM25 Full-Text Search
- Code-optimized parameters: `k1=0.8`, `b=0.5`
- Lower k1 (vs 1.2): better for repeated keywords in code
- Lower b (vs 0.75): less length normalization for functions

### Vector Semantic Search
- Embedding-based similarity search
- Supports local (fastembed) and remote (OpenAI) providers

### Hybrid Fusion (RRF)
Reciprocal Rank Fusion combines multiple search signals:

| Signal | Default Weight | Description |
|--------|----------------|-------------|
| `bm25_weight` | 0.6 | Full-text relevance |
| `vector_weight` | 0.3 | Semantic similarity |
| `snippet_weight` | 0.1 | Code snippet matches |
| `recent_weight` | 0.2 | Recently edited files |

RRF constant: `rrf_k=60.0`

### Repo Map (PageRank)

Token-budgeted context generation using PageRank algorithm:
- Builds dependency graph from AST symbol references
- Ranks files/symbols by importance using PageRank
- Generates compact code context within token budget
- Inspired by [Aider's repo map](https://aider.chat/docs/repomap.html)

Key parameters:
| Parameter | Default | Description |
|-----------|---------|-------------|
| `map_tokens` | 1024 | Max tokens for output |
| `damping_factor` | 0.85 | PageRank damping |
| `chat_file_weight` | 50.0 | Boost for referenced files |
| `mentioned_ident_weight` | 10.0 | Boost for mentioned identifiers |

## Providers

### Embedding Providers

| Provider | Type | Models | Feature Flag |
|----------|------|--------|--------------|
| **fastembed** | Local (ONNX) | nomic-embed-text, bge-*, MiniLM-* | `local-embeddings` |
| **OpenAI** | Remote API | text-embedding-3-small/large | - |

### LLM Providers (Query Rewriting)

| Provider | Type | Models | Notes |
|----------|------|--------|-------|
| **Ollama** | Local | qwen3:0.6b, gemma2:2b, phi3 | Requires `ollama serve` |
| **OpenAI** | Remote API | gpt-4o-mini | Default |

### Reranking Providers

| Provider | Type | Models | Feature Flag |
|----------|------|--------|--------------|
| **fastembed** | Local (ONNX) | bge-reranker-*, jina-reranker-* | `neural-reranker` |
| **Cohere** | Remote API | rerank-english-v3.0 | - |
| **Voyage AI** | Remote API | rerank-2 | - |

## Feature Flags

```toml
[features]
default = []
neural-reranker = ["fastembed"]    # Local neural reranking
local-embeddings = ["fastembed"]   # Local embeddings
local = ["local-embeddings", "neural-reranker"]  # All local features
```

Build with local features:
```bash
cargo build -p codex-retrieval --features local
```

## Configuration

Configuration file: `~/.codex/retrieval.toml` or `{project}/.codex/retrieval.toml`

See [QUICKSTART.md](QUICKSTART.md) for complete configuration examples.

### Key Configuration Sections

| Section | Description |
|---------|-------------|
| `retrieval.indexing` | File size limits, batch size, watch settings |
| `retrieval.chunking` | Token limits, overlap settings |
| `retrieval.search` | BM25 params, weights, thresholds |
| `retrieval.embedding` | Provider, model, dimension |
| `retrieval.query_rewrite` | LLM provider, model settings |
| `retrieval.extended_reranker` | Backend, model configuration |
| `retrieval.repo_map` | PageRank context generation settings |

## Local Model Summary

Minimal local deployment (~811MB total):

| Component | Model | Size | Runtime |
|-----------|-------|------|---------|
| Embedding | nomic-embed-text-v1.5 | 274MB | fastembed/ONNX |
| Query Rewrite | qwen3:0.6b | 400MB | Ollama |
| Reranking | jina-reranker-v1-turbo | 137MB | fastembed/ONNX |

## Links

- [Quick Start Guide](QUICKSTART.md) - Step-by-step setup and usage
