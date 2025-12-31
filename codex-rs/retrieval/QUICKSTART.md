# Quick Start Guide

Step-by-step guide to set up and use the Codex Retrieval System with local models.

## Prerequisites

- **Rust toolchain**: `rustup` with stable Rust
- **Ollama** (optional): For local LLM query rewriting

## Step 1: Install Ollama

### macOS

```bash
# Install via Homebrew
brew install ollama

# Start Ollama server
ollama serve
```

### Linux

```bash
# Install via curl
curl -fsSL https://ollama.com/install.sh | sh

# Start Ollama server
ollama serve
```

Verify installation:
```bash
ollama --version
```

## Step 2: Pull Required Models

### For Query Rewriting (Ollama)

```bash
# Minimal model (~400MB) - recommended for most cases
ollama pull qwen3:0.6b

# Or better quality (~1.6GB)
ollama pull gemma2:2b
```

### For Embeddings & Reranking (fastembed)

fastembed models are automatically downloaded on first use. No manual action required.

Models will be cached in `~/.cache/fastembed/` by default.

## Step 3: Create Configuration

Create configuration file at `~/.codex/retrieval.toml`:

```bash
mkdir -p ~/.codex
```

### Minimal Local Configuration

```toml
# ~/.codex/retrieval.toml
# Minimal local config - no external API dependencies

[retrieval]
enabled = true
data_dir = "~/.codex/retrieval"

# Embedding: Local fastembed (auto-downloads on first use)
[retrieval.embedding]
provider = "fastembed"
model = "nomic-embed-text-v1.5"   # 768 dims, ~274MB
dimension = 768
batch_size = 100

# Query Rewriting: Local Ollama
[retrieval.query_rewrite]
enabled = true

[retrieval.query_rewrite.llm]
provider = "ollama"
model = "qwen3:0.6b"
base_url = "http://localhost:11434/v1"
temperature = 0.3
max_tokens = 500
timeout_secs = 30

# Reranking: Local fastembed
[retrieval.extended_reranker]
backend = "local"

[retrieval.extended_reranker.local]
model = "jina-reranker-v1-turbo"  # ~137MB, fastest
batch_size = 32

# Repo Map: PageRank-based context generation
[retrieval.repo_map]
enabled = true
map_tokens = 1024
damping_factor = 0.85
```

### Remote API Configuration (OpenAI)

```toml
# ~/.codex/retrieval.toml
# Remote config - requires API keys

[retrieval]
enabled = true
data_dir = "~/.codex/retrieval"

# Embedding: OpenAI API
[retrieval.embedding]
provider = "openai"
model = "text-embedding-3-small"
dimension = 1536
batch_size = 100

# Query Rewriting: OpenAI API
[retrieval.query_rewrite]
enabled = true

[retrieval.query_rewrite.llm]
provider = "openai"
model = "gpt-4o-mini"
temperature = 0.3
max_tokens = 500

# Reranking: Cohere API
[retrieval.extended_reranker]
backend = "remote"

[retrieval.extended_reranker.remote]
provider = "cohere"
model = "rerank-english-v3.0"
api_key_env = "COHERE_API_KEY"
```

Set environment variables:
```bash
export OPENAI_API_KEY="sk-..."
export COHERE_API_KEY="..."
```

## Step 4: Build with Local Features

```bash
cd codex-rs

# Build with all local features
cargo build -p codex-retrieval --features local

# Or build specific features
cargo build -p codex-retrieval --features local-embeddings
cargo build -p codex-retrieval --features neural-reranker
```

## Step 5: Verify Setup

```bash
# Check Ollama is running
curl http://localhost:11434/api/tags

# Expected output: {"models":[{"name":"qwen3:0.6b",...}]}
```

---

## Configuration Parameters Reference

### Indexing (`[retrieval.indexing]`)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `max_file_size_mb` | 5 | Maximum file size to index (MB) |
| `batch_size` | 100 | Files to process per batch |
| `commit_interval` | 100 | Commit to DB every N operations |
| `lock_timeout_secs` | 30 | Lock timeout for concurrent access (seconds) |
| `watch_enabled` | false | Enable file watching |
| `watch_debounce_ms` | 500 | Debounce interval for file changes |
| `max_chunks` | 500000 | Maximum total chunks |

### Chunking (`[retrieval.chunking]`)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `max_tokens` | 512 | Maximum tokens per chunk |
| `overlap_tokens` | 50 | Token overlap between chunks |

Industry best practice: 256-512 tokens for code search.

### Search (`[retrieval.search]`)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `n_final` | 20 | Final number of results |
| `n_retrieve` | 50 | Initial candidates to retrieve |
| `bm25_threshold` | -2.5 | BM25 score threshold (negative, lower = stricter) |
| `rerank_threshold` | 0.3 | Reranking threshold |
| `max_result_tokens` | 8000 | Maximum tokens for all results combined |
| `max_chunks_per_file` | 2 | Max chunks per file (ensures result diversity) |
| `max_token_length` | 64 | Maximum token length for filtering |
| `truncate_strategy` | "tail" | Token truncation: `tail` or `smart` |

**Text Processing**:

| Parameter | Default | Description |
|-----------|---------|-------------|
| `enable_stemming` | true | Enable word stemming |
| `enable_ngrams` | false | Enable n-gram generation |
| `ngram_size` | 3 | N-gram size (when enabled) |

**BM25 Parameters** (code-optimized):

| Parameter | Default | Standard | Notes |
|-----------|---------|----------|-------|
| `bm25_k1` | 0.8 | 1.2 | Term frequency saturation. Lower = better for repeated keywords |
| `bm25_b` | 0.5 | 0.75 | Length normalization. Lower = less penalty for long functions |

**Fusion Weights** (RRF):

| Parameter | Default | Description |
|-----------|---------|-------------|
| `bm25_weight` | 0.6 | Full-text search weight |
| `vector_weight` | 0.3 | Semantic search weight |
| `snippet_weight` | 0.1 | Snippet match weight |
| `recent_weight` | 0.2 | Recently edited files weight |
| `rrf_k` | 60.0 | RRF constant |
| `path_weight_multiplier` | 10.0 | Boost for path matches |

### Embedding (`[retrieval.embedding]`)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `provider` | - | `"fastembed"` or `"openai"` |
| `model` | - | Model name (see tables below) |
| `dimension` | 1536 | Embedding dimension |
| `batch_size` | 100 | Batch size for embedding requests |
| `base_url` | - | Custom API endpoint (optional) |

**Vector Quantization** (`[retrieval.embedding.quantization]`):

Reduces storage for large indexes (>100k chunks).

| Parameter | Default | Description |
|-----------|---------|-------------|
| `method` | `"none"` | `"none"`, `"scalar"` (4x compression), `"product"` (4-8x) |
| `num_sub_vectors` | 16 | PQ subquantizers (must divide dimension evenly) |
| `num_bits` | 8 | Bits per code for Product Quantization |

**fastembed Models**:

| Model | Dimension | Size | Notes |
|-------|-----------|------|-------|
| `nomic-embed-text-v1.5` | 768 | ~274MB | Recommended default |
| `bge-small-en-v1.5` | 384 | ~134MB | Smallest |
| `all-MiniLM-L6-v2` | 384 | ~90MB | Classic, fast |
| `bge-base-en-v1.5` | 768 | ~420MB | Good balance |
| `mxbai-embed-large-v1` | 1024 | ~670MB | High quality |

### Query Rewrite (`[retrieval.query_rewrite]`)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `enabled` | true | Enable query rewriting |

**LLM Provider** (`[retrieval.query_rewrite.llm]`):

| Parameter | Default | Description |
|-----------|---------|-------------|
| `provider` | `"openai"` | `"ollama"` or `"openai"` |
| `model` | `"gpt-4o-mini"` | Model name |
| `base_url` | - | Custom endpoint (for Ollama) |
| `temperature` | 0.3 | Generation temperature |
| `max_tokens` | 500 | Max tokens for response |
| `timeout_secs` | 10 | Request timeout |
| `max_retries` | 2 | Retry attempts |

**Cache** (`[retrieval.query_rewrite.cache]`):

| Parameter | Default | Description |
|-----------|---------|-------------|
| `enabled` | true | Enable query rewrite caching |
| `ttl_secs` | 86400 | Cache TTL (24 hours) |
| `max_entries` | 10000 | Maximum cache entries |

**Features** (`[retrieval.query_rewrite.features]`):

| Parameter | Default | Description |
|-----------|---------|-------------|
| `translation` | true | Translate non-English to English |
| `intent_classification` | true | Classify query intent |
| `expansion` | true | Expand with synonyms/related terms |
| `case_variants` | true | Generate camelCase/snake_case variants |

**Rules** (`[retrieval.query_rewrite.rules]`):

| Parameter | Default | Description |
|-----------|---------|-------------|
| `synonyms` | {} | Custom synonym mappings (term → [synonyms]) |

Example synonyms:
```toml
[retrieval.query_rewrite.rules.synonyms]
function = ["fn", "func", "method", "def"]
class = ["struct", "type", "interface"]
```

**Ollama Models** (for local deployment):

| Model | Size | Speed | Quality |
|-------|------|-------|---------|
| `qwen3:0.6b` | ~400MB | Fast | Good |
| `qwen2.5:1.5b` | ~1GB | Medium | Better |
| `gemma2:2b` | ~1.6GB | Medium | Good |
| `phi3:mini` | ~2.3GB | Slow | Best |

### Repo Map (`[retrieval.repo_map]`)

PageRank-based context generation for LLMs (inspired by Aider).

| Parameter | Default | Description |
|-----------|---------|-------------|
| `enabled` | false | Enable repo map generation |
| `map_tokens` | 1024 | Maximum tokens for output |
| `map_mul_no_files` | 8.0 | Token multiplier when no chat files |
| `cache_ttl_secs` | 3600 | Cache TTL (1 hour) |

**PageRank Parameters**:

| Parameter | Default | Description |
|-----------|---------|-------------|
| `damping_factor` | 0.85 | PageRank damping (0-1) |
| `max_iterations` | 100 | Max PageRank iterations |
| `tolerance` | 1e-6 | Convergence tolerance |

**Weight Multipliers**:

| Parameter | Default | Description |
|-----------|---------|-------------|
| `chat_file_weight` | 50.0 | Boost for chat-referenced files |
| `mentioned_ident_weight` | 10.0 | Boost for mentioned identifiers |
| `private_symbol_weight` | 0.1 | Penalty for private symbols |
| `naming_style_weight` | 10.0 | Boost for well-named identifiers |
| `term_match_weight` | 5.0 | Boost for term matches |

**Refresh Modes**:

| Mode | Description |
|------|-------------|
| `auto` | Cache if computation > 1 second (default) |
| `files` | Cache based on file set only |
| `always` | Never use cache |
| `manual` | Only regenerate on explicit request |

### Rule-Based Reranker (`[retrieval.reranker]`)

Legacy rule-based reranker (no ML model required).

| Parameter | Default | Description |
|-----------|---------|-------------|
| `enabled` | true | Enable rule-based reranking |
| `exact_match_boost` | 2.0 | Boost for exact query term matches |
| `path_relevance_boost` | 1.5 | Boost for query terms in file path |
| `recency_boost` | 1.2 | Boost for recently modified files |
| `recency_days_threshold` | 7 | Days threshold for recency boost |

### Extended Reranker (`[retrieval.extended_reranker]`)

Supports rule-based, local neural, remote API, and chained rerankers.

| Parameter | Default | Description |
|-----------|---------|-------------|
| `backend` | `"rule_based"` | `"rule_based"`, `"local"`, `"remote"`, or `"chain"` |

**Local Neural Reranker** (`[retrieval.extended_reranker.local]`):

Uses fastembed-rs with ONNX Runtime. Models auto-download on first use.

| Parameter | Default | Description |
|-----------|---------|-------------|
| `model` | `"bge-reranker-base"` | Reranker model name |
| `batch_size` | 32 | Batch size |
| `cache_dir` | - | Model cache directory |
| `show_download_progress` | false | Show model download progress |

**Local Reranker Models**:

| Model | Size | Speed | Quality |
|-------|------|-------|---------|
| `jina-reranker-v1-turbo` | ~137MB | Fastest | Good |
| `bge-reranker-base` | ~278MB | Medium | Better |
| `bge-reranker-v2-m3` | ~580MB | Slow | Best |

**Remote API Reranker** (`[retrieval.extended_reranker.remote]`):

Supports Cohere, Voyage AI, and custom API endpoints.

| Parameter | Default | Description |
|-----------|---------|-------------|
| `provider` | - | `"cohere"`, `"voyage_ai"`, or `"custom"` |
| `model` | - | Model name (e.g., `"rerank-english-v3.0"`) |
| `api_key_env` | `"COHERE_API_KEY"` | Environment variable for API key |
| `base_url` | - | Custom API base URL (optional) |
| `timeout_secs` | 10 | Request timeout |
| `max_retries` | 2 | Maximum retry attempts |
| `top_n` | - | Return top-N results (optional) |

**Chained Reranker** (`[retrieval.extended_reranker.chain]`):

Chain multiple rerankers sequentially (e.g., rule-based → neural).

```toml
[[retrieval.extended_reranker.chain]]
backend = "rule_based"

[[retrieval.extended_reranker.chain]]
backend = "local"
[retrieval.extended_reranker.chain.local]
model = "jina-reranker-v1-turbo"
```

---

## Troubleshooting

### Ollama Connection Errors

```
Cannot connect to Ollama at 'http://localhost:11434/v1'
```

**Fix**: Start Ollama server:
```bash
ollama serve
```

### Model Not Found

```
Model 'qwen3:0.6b' not found
```

**Fix**: Pull the model:
```bash
ollama pull qwen3:0.6b
```

### fastembed Model Download Fails

**Fix**: Check internet connection and retry. Models are downloaded to `~/.cache/fastembed/`.

Clear cache and retry:
```bash
rm -rf ~/.cache/fastembed
```

### Memory Issues

For low-memory systems:
- Use smaller embedding model: `bge-small-en-v1.5` (384 dims)
- Use smaller LLM: `qwen3:0.6b`
- Use faster reranker: `jina-reranker-v1-turbo`

### Dimension Mismatch

If you change embedding models, you must re-index:
```bash
rm -rf ~/.codex/retrieval
# Re-run indexing
```

---

## Complete Example: Local-Only Setup

```bash
# 1. Install Ollama (macOS)
brew install ollama
ollama serve &

# 2. Pull LLM model
ollama pull qwen3:0.6b

# 3. Create config
mkdir -p ~/.codex
cat > ~/.codex/retrieval.toml << 'EOF'
[retrieval]
enabled = true
data_dir = "~/.codex/retrieval"

[retrieval.embedding]
provider = "fastembed"
model = "nomic-embed-text-v1.5"
dimension = 768

[retrieval.query_rewrite]
enabled = true

[retrieval.query_rewrite.llm]
provider = "ollama"
model = "qwen3:0.6b"
base_url = "http://localhost:11434/v1"

[retrieval.extended_reranker]
backend = "local"

[retrieval.extended_reranker.local]
model = "jina-reranker-v1-turbo"

[retrieval.repo_map]
enabled = true
map_tokens = 1024
EOF

# 4. Build
cd codex-rs
cargo build -p codex-retrieval --features local

# 5. Verify
curl http://localhost:11434/api/tags
echo "Setup complete!"
```
