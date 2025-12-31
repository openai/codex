# Codex LSP

AI-friendly LSP client library for codex-rs with symbol name resolution.

## Overview

The `codex-lsp` crate provides a high-level abstraction over raw LSP (Language Server Protocol), enabling **symbol name-based queries** instead of exact line/column positions. This makes it ideal for AI-powered code analysis and navigation.

**Key Features:**
- Query symbols by name (e.g., `definition("Config")`) instead of position
- Automatic LSP server lifecycle management
- Symbol caching with LRU eviction
- Incremental document sync using Myers diff algorithm
- Health monitoring with auto-restart

## Supported Languages

| Language | LSP Server | Extensions | Install Command |
|----------|------------|------------|-----------------|
| Rust | rust-analyzer | `.rs` | `rustup component add rust-analyzer` |
| Go | gopls | `.go` | `go install golang.org/x/tools/gopls@latest` |
| Python | pyright | `.py`, `.pyi` | `npm install -g pyright` |
| TypeScript/JavaScript | typescript-language-server | `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs` | `npm install -g typescript-language-server typescript` |

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      LspServerManager                            │
│  - Manages multiple LSP server instances                         │
│  - Health checks, auto-restart                                   │
│  - Server lifecycle (start/stop/restart)                         │
└────────────────────────┬────────────────────────────────────────┘
                         │ get_client(path)
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                         LspClient                                │
│  - Single server connection                                      │
│  - AI-friendly operations (definition, references, hover...)     │
│  - Symbol caching (LRU, 100 entries)                             │
│  - File tracking (max 500 files)                                 │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    JsonRpcConnection                             │
│  - JSON-RPC 2.0 over stdio                                       │
│  - Request/response multiplexing                                 │
│  - Notification handling                                         │
└─────────────────────────────────────────────────────────────────┘
```

### Module Structure

| Module | Purpose |
|--------|---------|
| `server.rs` | LspServerManager - manages multiple LSP server instances |
| `client.rs` | LspClient - single server operations and caching |
| `config.rs` | Configuration types and built-in server definitions |
| `protocol.rs` | JSON-RPC 2.0 connection handling |
| `symbols.rs` | Symbol types and matching algorithms |
| `diagnostics.rs` | Diagnostic storage with debouncing |
| `lifecycle.rs` | Server health tracking and restart logic |
| `client_ext.rs` | Incremental document sync (Myers diff) |

## Features

### LSP Operations

| Operation | AI-Friendly | Position-based | Description |
|-----------|-------------|----------------|-------------|
| Definition | `definition(path, name, kind)` | `definition_at_position()` | Find where symbol is defined |
| Implementation | `implementation(path, name, kind)` | `implementation_at_position()` | Find trait/interface implementations |
| References | `references(path, name, kind, include_decl)` | `references_at_position()` | Find all symbol references |
| Hover | `hover(path, name, kind)` | `hover_at_position()` | Get type/documentation info |
| Type Definition | `type_definition(path, name, kind)` | `type_definition_at_position()` | Find type definition |
| Declaration | `declaration(path, name, kind)` | `declaration_at_position()` | Find symbol declaration |
| Symbols | `document_symbols(path)` | - | Get all symbols in file |
| Workspace | `workspace_symbol(query)` | - | Search entire workspace |
| Call Hierarchy | `prepare_call_hierarchy()` → `incoming_calls()` / `outgoing_calls()` | - | Analyze call chains |

### Symbol Kind Matching

The library parses symbol kinds loosely for AI-friendly input:

| Input | Matches |
|-------|---------|
| `function`, `func`, `fn` | Function |
| `method` | Method |
| `class` | Class |
| `struct` | Struct |
| `interface`, `trait` | Interface |
| `enum` | Enum |
| `variable`, `var`, `let` | Variable |
| `constant`, `const` | Constant |
| `property`, `prop` | Property |
| `field` | Field |
| `module`, `mod`, `namespace` | Module |
| `type` | Type |

## Configuration

### Config File Locations

Priority order (later overrides earlier):
1. User-level: `~/.codex/lsp_servers.json`
2. Project-level: `.codex/lsp_servers.json`

### Configuration Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `disabled` | bool | `false` | Disable this server |
| `command` | string | - | Server command (required for custom servers) |
| `args` | string[] | `[]` | Command-line arguments |
| `file_extensions` | string[] | `[]` | File extensions this server handles |
| `languages` | string[] | `[]` | Language identifiers |
| `env` | object | `{}` | Environment variables |
| `initialization_options` | object | `null` | LSP initialization options |
| `settings` | object | `null` | Workspace settings |
| `workspace_folder` | string | - | Explicit workspace folder path |
| `max_restarts` | int | `3` | Max restart attempts before giving up |
| `restart_on_crash` | bool | `true` | Auto-restart on crash |
| `startup_timeout_ms` | int | `10000` | Initialization timeout (ms) |
| `shutdown_timeout_ms` | int | `5000` | Shutdown timeout (ms) |
| `request_timeout_ms` | int | `30000` | Request timeout (ms) |
| `health_check_interval_ms` | int | `30000` | Health check interval (ms) |
| `notification_buffer_size` | int | `100` | Notification channel buffer size |

### Example Configuration

```json
{
  "servers": {
    "rust-analyzer": {
      "initialization_options": {
        "checkOnSave": { "command": "clippy" }
      },
      "max_restarts": 5,
      "startup_timeout_ms": 15000
    },
    "gopls": {
      "disabled": true
    },
    "typescript": {
      "command": "typescript-language-server",
      "args": ["--stdio"],
      "file_extensions": [".ts", ".tsx", ".js", ".jsx"],
      "languages": ["typescript", "javascript"]
    }
  }
}
```

## Algorithm Details

### Symbol Matching Algorithm

The `find_matching_symbols()` function uses an optimized matching strategy:

```
1. Filter by kind (if provided) - O(1) check
2. Exact name match (case-insensitive) - prioritized
3. Substring match (only if no exact match) - lazy allocation
4. Sort by exact_match (true values first)
```

**Optimization**: Uses `OnceCell` to defer lowercase string allocation until needed for substring matching, avoiding allocations for exact matches.

### File Management

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_OPENED_FILES` | 500 | Max files tracked per server |
| `LRU_EVICTION_PERCENT` | 25% | Fraction of files to evict when full |

### Symbol Caching

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_SYMBOL_CACHE_SIZE` | 100 | Max symbols cached per file |
| Cache invalidation | On file change | Version-tracked |

### Incremental Document Sync

- Uses **Myers diff algorithm** (via `similar` crate)
- Line-based comparison for files ≤ 1 MB
- Falls back to full sync for larger files
- Computes minimal `TextDocumentContentChangeEvent` vector

### Health Monitoring

| Constant | Value | Purpose |
|----------|-------|---------|
| `HEALTH_CHECK_MIN_INTERVAL_SECS` | 30 | Rate-limit health checks |
| `HEALTH_CHECK_TIMEOUT_SECS` | 5 | Health check timeout |

**Health Check Method**: Dual fallback
1. Try `workspace/symbol` request
2. If fails, try `hover` on any open file

## API Usage

### Basic Usage

```rust
use codex_lsp::{LspServersConfig, LspServerManager, DiagnosticsStore, SymbolKind};
use std::sync::Arc;
use std::path::Path;

// Create manager with default config
let diagnostics = Arc::new(DiagnosticsStore::new());
let config = LspServersConfig::default();
let manager = LspServerManager::new(config, diagnostics);

// Or load config from standard locations
let manager = LspServerManager::with_auto_config(Some(project_root), diagnostics);

// Get client for a file
let client = manager.get_client(Path::new("src/lib.rs")).await?;

// Pre-warm servers for known extensions
let warmed = manager.prewarm(&[".rs", ".go"], project_root).await;
```

### AI-Friendly Operations

```rust
// Find definition by symbol name
let locations = client.definition(
    Path::new("src/lib.rs"),
    "Config",
    Some(SymbolKind::Struct)
).await?;

// Find all references
let refs = client.references(
    Path::new("src/lib.rs"),
    "Config",
    Some(SymbolKind::Struct),
    true  // include_declaration
).await?;

// Get hover documentation
let hover_text = client.hover(
    Path::new("src/lib.rs"),
    "Config",
    Some(SymbolKind::Struct)
).await?;

// Analyze call hierarchy
let items = client.prepare_call_hierarchy(
    Path::new("src/main.rs"),
    "main",
    Some(SymbolKind::Function)
).await?;
let callers = client.incoming_calls(items[0].clone()).await?;
let callees = client.outgoing_calls(items[0].clone()).await?;
```

### Shutdown

```rust
// Clean shutdown of all servers
manager.shutdown_all().await;
```

## Error Handling

| Error | When Triggered |
|-------|----------------|
| `ServerNotFound` | LSP binary not in PATH |
| `ServerStartFailed` | Process spawn failed |
| `InitializationTimeout` | Init took > startup_timeout_ms |
| `ServerFailed` | Crashed after max restart attempts |
| `ServerRestarting` | Restart in progress, retry needed |
| `HealthCheckFailed` | Health check failed |
| `NoServerForExtension` | No server for file extension |
| `OperationNotSupported` | Server doesn't support operation |
| `SymbolNotFound` | Symbol not in document |
| `RequestTimeout` | Request exceeded timeout |

## Server Health States

```rust
pub enum ServerHealth {
    Healthy,   // Server running and responding
    Starting,  // Initialization in progress
    Crashed,   // Server crashed, restart pending
    Failed,    // Failed after max restart attempts
    Stopping,  // Graceful shutdown
}
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime (process, IO, sync) |
| `lsp-types` | LSP protocol type definitions |
| `serde` / `serde_json` | JSON serialization |
| `thiserror` | Error handling |
| `tracing` | Structured logging |
| `which` | Binary detection |
| `similar` | Myers diff algorithm |
| `dirs` | Home directory detection |

## License

See the project root LICENSE file.
