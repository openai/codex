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
│  Utils: utility crates                                            │
└─────────────────────────────────────────────────────────────────┘
```

## Crate Guide

### Common

| Crate | Purpose |
|-------|---------|
| `protocol` | Foundational types: Model, Provider, Config, Events |
| `config` | Layered config: JSON + env + runtime |
| `error` | Unified errors with stack traces |
| `otel` | OpenTelemetry tracing |

### Provider SDKs

| Crate | Purpose |
|-------|---------|
| `hyper-sdk` | **Main SDK**: Multi-provider client, streaming, tools |
| `anthropic` | Anthropic Claude API |
| `openai` | OpenAI Responses API |
| `google-genai` | Google Gemini API |
| `volcengine-ark` | Volcengine Ark API |
| `z-ai` | ZhipuAI/Z.AI API |

### Core

| Crate | Purpose |
|-------|---------|
| `api` | Provider-agnostic LLM API client |
| `message` | Conversation message types and turn history |
| `tools` | Tool schemas, invocation, and result handling |
| `context` | Context window assembly and truncation |
| `prompt` | System prompt templating and composition |
| `loop` | Main agent turn loop: prompt → LLM → tools → repeat |
| `executor` | Top-level task driver with permissions and hooks |
| `subagent` | Spawn isolated agent instances for parallel tasks |
| `system-reminder` | Inject system reminders mid-conversation |

### App

| Crate | Purpose |
|-------|---------|
| `cli` | CLI entry point (`cocode` binary) |
| `tui` | Terminal UI (ratatui-based) |
| `session` | Session persistence and management |

### Exec

| Crate | Purpose |
|-------|---------|
| `shell` | Safe shell command spawning and output capture |
| `sandbox` | macOS Seatbelt / Linux landlock sandboxing |
| `arg0` | Dispatch `cocode` subcommands by argv[0] |

### Features

| Crate | Purpose |
|-------|---------|
| `skill` | `/command` skill definitions and execution |
| `hooks` | Pre/post event hooks for tool calls |
| `plugin` | Dynamic plugin discovery and loading |
| `plan-mode` | Plan-then-execute workflow orchestration |

### MCP

| Crate | Purpose |
|-------|---------|
| `mcp-types` | Model Context Protocol message types |
| `rmcp-client` | MCP client over stdio/SSE transport |

### Standalone

| Crate | Purpose | CLAUDE.md |
|-------|---------|-----------|
| `retrieval` | Code search: BM25 + vector + AST | [retrieval/CLAUDE.md](cocode-rs/retrieval/CLAUDE.md) |
| `lsp` | AI-friendly LSP client | [lsp/CLAUDE.md](cocode-rs/lsp/CLAUDE.md) |

### Utils

| Crate | Purpose |
|-------|---------|
| `file-ignore` | .gitignore-aware filtering |
| `file-search` | Fuzzy file search |
| `file-encoding` | File encoding detection and line-ending preservation |
| `apply-patch` | Unified diff/patch |
| `git` | Git operations wrapper |
| `shell-parser` | Shell command parsing |
| `keyring-store` | Secure credential storage |
| `pty` | Pseudo-terminal handling |
| `image` | Image processing |
| `cache` | In-memory and disk cache helpers |
| `common` | Cross-crate utility functions |
| `absolute-path` | Path absolutization |
| `cargo-bin` | Cargo binary helpers |
| `json-to-toml` | JSON to TOML conversion |
| `readiness` | Health/readiness probes for services |
| `string` | String manipulation helpers |
| `async-utils` | Async runtime utilities and combinators |
| `stdio-to-uds` | Bridge stdio streams to Unix domain sockets |

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
