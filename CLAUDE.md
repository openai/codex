# CLAUDE.md

Multi-provider LLM SDK and CLI. All development in `cocode-rs/`.

**Read `AGENTS.md` for Rust conventions.** This file covers architecture and crate navigation.

## Commands

Run from `cocode-rs/` directory:

```bash
just fmt          # After Rust changes (auto-approve)
just pre-commit   # REQUIRED before commit
just test         # If changed provider-sdks or core crates
just help         # All commands
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  App Layer: cli, tui, session                                   │
├─────────────────────────────────────────────────────────────────┤
│  Core Layer: loop → executor → api                              │
│              ↓         ↓                                        │
│           tools ← context ← prompt                              │
│              ↓                                                  │
│        message, system-reminder, subagent                       │
├─────────────────────────────────────────────────────────────────┤
│  Features: skill, hooks, plugin, plan-mode                      │
│  Exec: shell, sandbox, arg0                                     │
│  MCP: mcp-types, rmcp-client                                    │
│  Standalone: retrieval, lsp                                     │
├─────────────────────────────────────────────────────────────────┤
│  Provider SDKs: hyper-sdk → anthropic, openai, google-genai,    │
│                             volcengine-ark, z-ai                │
├─────────────────────────────────────────────────────────────────┤
│  Common: protocol, config, error, otel                          │
│  Utils: 17 utility crates                                       │
└─────────────────────────────────────────────────────────────────┘
```

## Crate Guide (50 crates)

### Common (4)

| Crate | Purpose |
|-------|---------|
| `protocol` | Foundational types: Model, Provider, Config, Events |
| `config` | Layered config: JSON + env + runtime |
| `error` | Unified errors with stack traces |
| `otel` | OpenTelemetry tracing |

### Provider SDKs (6)

| Crate | Purpose |
|-------|---------|
| `hyper-sdk` | **Main SDK**: Multi-provider client, streaming, tools |
| `anthropic` | Anthropic Claude API |
| `openai` | OpenAI Responses API |
| `google-genai` | Google Gemini API |
| `volcengine-ark` | Volcengine Ark API |
| `z-ai` | ZhipuAI/Z.AI API |

### Core (9)

| Crate | Purpose |
|-------|---------|
| `api` | API client abstraction |
| `message` | Message types and history |
| `tools` | Tool definitions and execution |
| `context` | Conversation context management |
| `prompt` | System prompt generation |
| `loop` | Agent loop orchestration |
| `executor` | Task execution and coordination |
| `subagent` | Sub-agent spawning and management |
| `system-reminder` | System reminder injection |

### App (3)

| Crate | Purpose |
|-------|---------|
| `cli` | CLI entry point (`cocode` binary) |
| `tui` | Terminal UI (ratatui-based) |
| `session` | Session persistence and management |

### Exec (3)

| Crate | Purpose |
|-------|---------|
| `shell` | Shell command execution |
| `sandbox` | Sandboxed execution environment |
| `arg0` | Binary dispatcher for CLI |

### Features (4)

| Crate | Purpose |
|-------|---------|
| `skill` | Slash command skills |
| `hooks` | Event hooks system |
| `plugin` | Plugin loading and management |
| `plan-mode` | Plan mode workflow |

### MCP (2)

| Crate | Purpose |
|-------|---------|
| `mcp-types` | MCP protocol types |
| `rmcp-client` | Remote MCP client |

### Standalone (2)

| Crate | Purpose | CLAUDE.md |
|-------|---------|-----------|
| `retrieval` | Code search: BM25 + vector + AST | [retrieval/CLAUDE.md](cocode-rs/retrieval/CLAUDE.md) |
| `lsp` | AI-friendly LSP client | [lsp/CLAUDE.md](cocode-rs/lsp/CLAUDE.md) |

### Utils (17)

| Crate | Purpose |
|-------|---------|
| `file-ignore` | .gitignore-aware filtering |
| `file-search` | Fuzzy file search |
| `apply-patch` | Unified diff/patch |
| `git` | Git operations wrapper |
| `shell-parser` | Shell command parsing |
| `keyring-store` | Secure credential storage |
| `pty` | Pseudo-terminal handling |
| `image` | Image processing |
| `cache` | Caching utilities |
| `common` | Shared utilities |
| `absolute-path` | Path absolutization |
| `cargo-bin` | Cargo binary helpers |
| `json-to-toml` | JSON to TOML conversion |
| `readiness` | Service readiness checks |
| `string` | String utilities |
| `async-utils` | Async helpers |
| `stdio-to-uds` | Stdio to Unix socket |

## Error Handling

| Layer | Error Type |
|-------|------------|
| common/, core/ | `cocode-error` + snafu |
| provider-sdks/, utils/ | `anyhow::Result` |

See [common/error/README.md](cocode-rs/common/error/README.md) for patterns and StatusCode list.

## Specialized Documentation

| Component | Guide |
|-----------|-------|
| TUI | [app/tui/CLAUDE.md](cocode-rs/app/tui/CLAUDE.md) |
| Retrieval | [retrieval/CLAUDE.md](cocode-rs/retrieval/CLAUDE.md) |
| LSP | [lsp/CLAUDE.md](cocode-rs/lsp/CLAUDE.md) |
| Provider SDKs | [provider-sdks/hyper-sdk/CLAUDE.md](cocode-rs/provider-sdks/hyper-sdk/CLAUDE.md) |

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| **No Prompt Caching** | Prompt caching (Anthropic's cache breakpoints feature) is not required for this project. Do not implement or plan for it. |
| **No Deprecated Code** | When refactoring or implementing features, remove obsolete code completely. Do not mark as deprecated or maintain backward compatibility - delete it outright to keep the codebase clean and avoid technical debt. |

## References

- **Code conventions**: `AGENTS.md`
- **Error codes**: `cocode-rs/common/error/README.md`
- **User docs**: `docs/` (getting-started.md, config.md, sandbox.md)
