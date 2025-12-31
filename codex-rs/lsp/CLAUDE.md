# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Crate Overview

`codex-lsp` is an AI-friendly LSP client library that abstracts the Language Server Protocol to enable **symbol name-based queries** instead of exact line/column positions. Part of the codex-rs workspace.

## Important Note

**This crate does NOT follow the `*_ext.rs` extension pattern.** Direct modifications to existing files are allowed and preferred for this directory.

## Development Commands

```bash
# From codex-rs/ directory (REQUIRED - never run from lsp/)
cd codex-rs

# Build
cargo build -p codex-lsp

# Test
cargo test -p codex-lsp

# Check (fast iteration)
cargo check -p codex-lsp

# Format (no approval needed)
just fmt
```

## Architecture

```
LspServerManager              # Manages multiple server instances, health checks, auto-restart
    ↓ get_client(path)
LspClient                     # Single server connection, AI-friendly operations, caching
    ↓
JsonRpcConnection             # JSON-RPC 2.0 over stdio, request multiplexing
```

### Module Responsibilities

| File | Purpose |
|------|---------|
| `server.rs` | `LspServerManager` - server lifecycle, client caching, prewarm |
| `client.rs` | `LspClient` - LSP operations, symbol caching, file tracking |
| `config.rs` | `BUILTIN_SERVERS`, `LspServerConfig`, config loading |
| `protocol.rs` | `JsonRpcConnection`, `TimeoutConfig`, message encoding |
| `symbols.rs` | `SymbolKind`, `find_matching_symbols()`, symbol flattening |
| `client_ext.rs` | Incremental document sync using Myers diff |
| `diagnostics.rs` | `DiagnosticsStore` with debouncing |
| `lifecycle.rs` | `ServerLifecycle`, health monitoring, restart logic |
| `error.rs` | `LspErr` enum |

## Key Patterns

### Error Handling

Uses `LspErr` (thiserror-based) - follows workspace convention:
```rust
use crate::error::{LspErr, Result};
```

### AI-Friendly Symbol Resolution

Query by name+kind instead of position:
```rust
client.definition(path, "Config", Some(SymbolKind::Struct)).await?;
client.references(path, "process", Some(SymbolKind::Function), true).await?;
```

Position-based variants available with `_at_position` suffix.

### Symbol Kind Parsing

`SymbolKind::from_str_loose()` accepts: `fn`/`func`/`function`, `trait`/`interface`, `var`/`let`/`variable`, etc.

### Caching

- **Symbol cache**: 100 entries per file, LRU eviction, version-tracked invalidation
- **File tracking**: 500 max open files, 25% LRU eviction when full
- **Incremental sync**: Myers diff for files ≤1MB, falls back to full sync

### Lifecycle Management

```rust
// ServerLifecycle tracks health, restarts, crashes
lifecycle.should_restart()      // Check restart budget
lifecycle.record_crash()        // Returns true if should retry
lifecycle.record_started()      // Reset crash counter
```

Health check: tries `workspace/symbol`, falls back to `hover` on any open file.

## Built-in Language Servers

| Server | Extensions | Install |
|--------|------------|---------|
| rust-analyzer | `.rs` | `rustup component add rust-analyzer` |
| gopls | `.go` | `go install golang.org/x/tools/gopls@latest` |
| pyright | `.py`, `.pyi` | `npm install -g pyright` |
| typescript-language-server | `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs` | `npm install -g typescript-language-server typescript` |

## Configuration

Config files: `~/.codex/lsp_servers.json` (user) → `.codex/lsp_servers.json` (project overrides)

```json
{
  "servers": {
    "rust-analyzer": {
      "initialization_options": {"checkOnSave": {"command": "clippy"}},
      "max_restarts": 5
    },
    "gopls": { "disabled": true },
    "my-custom-lsp": {
      "command": "my-lsp",
      "args": ["--stdio"],
      "file_extensions": [".xyz"]
    }
  }
}
```

## Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_OPENED_FILES` | 500 | File tracking limit |
| `MAX_SYMBOL_CACHE_SIZE` | 100 | Symbol cache entries per file |
| `MAX_INCREMENTAL_CONTENT_SIZE` | 1MB | Incremental sync threshold |
| `LRU_EVICTION_PERCENT` | 25% | Cache eviction batch size |
| `HEALTH_CHECK_TIMEOUT_SECS` | 5 | Health probe timeout |

## Common Operations

### Adding New LSP Operations

1. Add method to `LspClient` in `client.rs`
2. Use `sync_file()` first to ensure file is opened
3. Use `find_matching_symbols()` for name-based lookup
4. Check capability before calling (e.g., `supports_call_hierarchy()`)

### Adding Built-in Server

1. Add entry to `BUILTIN_SERVERS` in `config.rs`
2. Include: `id`, `extensions`, `commands`, `install_hint`, `languages`

## Testing Notes

- Uses `tokio::test` for async tests
- Config tests use temp files with cleanup
- Symbol matching tests verify exact vs substring priority
