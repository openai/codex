# Helios Rebrand Summary

## What Was Done

### 1. Project Reorganization
- Moved `heliosHarness/clones/helios-cli/` → `repos/helios/`
- Renamed `codex-rs/` → `helios-rs/`
- Created `heliosHarness/ref/codex-upstream/` as reference

### 2. Crate Rebranding (codex-* → helios-*)
| Old Name | New Name |
|----------|----------|
| codex-api | helios-api |
| codex-backend-openapi-models | helios-backend-openapi-models |
| codex-client | helios-client |
| codex-experimental-api-macros | helios-experimental-api-macros |

### 3. Directory Rebranding
- `.codex/` → `.helios/`
- `codex-cli/` → `helios-cli-bin/`

### 4. Source Code Updates
- Replaced `codex-` → `helios-` in all Cargo.toml files
- Replaced `codex::` → `helios::` in all .rs files
- Replaced `CODEX` → `HELIOS` in all .rs files

### 5. Transport Layer Added
Created `helios-rs/core/src/transport/` with:
- `mod.rs` - Transport types and config
- `selector.rs` - Auto-select optimal transport
- `pool.rs` - Connection pooling
- `http2.rs` - HTTP/2 transport
- `websocket.rs` - WebSocket transport
- `unix_socket.rs` - Unix domain socket transport
- `grpc.rs` - gRPC transport

## Current Structure

```
repos/
├── helios/                          ← Main Helios project (codex fork)
│   ├── helios-rs/                   ← 51 Rust crates
│   │   ├── helios-api/
│   │   ├── helios-client/
│   │   ├── core/src/transport/      ← NEW: Transport layer
│   │   └── ...
│   ├── helios-cli-bin/
│   └── .helios/
│
├── heliosHarness/
│   ├── ref/codex-upstream/          ← Reference (fresh clone)
│   └── packages/helios_native/      ← Rust native extension
│
├── thegent/                         ← Python project (separate)
├── cliproxy++/                      ← LLM proxy (Go)
└── ...
```

## Transport Types

| Transport | Use Case | Latency |
|-----------|----------|---------|
| Unix Socket | Local IPC | 10-50µs |
| WebSocket | Streaming | 50-200µs |
| HTTP/2 | Default | 100-500µs |
| gRPC | Typed API | 50-200µs |

## Next Steps

1. Complete transport implementation in each module
2. Add transport commands to CLI
3. Update all tests
4. Update documentation

## Build

```bash
cd repos/helios/helios-rs
cargo build --release
```
