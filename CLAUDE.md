# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Overview

**Cocode** - Multi-provider LLM SDK and utilities. Main development in `cocode-rs/`.

```
codex/
├── cocode-rs/     → Main Rust workspace (ALL development here)
├── codex-rs/      → Legacy workspace (reference only)
├── sdk2/          → Python Agent SDK (see sdk2/CLAUDE.md)
├── docs/          → User documentation
└── AGENTS.md      → Rust conventions (READ THIS)
```

**IMPORTANT:** Read `AGENTS.md` for detailed Rust conventions. This file covers architecture and crate-specific guidance.

**Note:** `codex-rs/` (59+ crates) is kept as reference for implementing similar features in `cocode-rs/`. Do not actively develop in `codex-rs/`.

## Critical Rules

### Working Directory

**Run cargo commands from `codex/` directory using `--manifest-path`:**

```bash
cargo build --manifest-path cocode-rs/Cargo.toml   # Correct
cargo check -p hyper-sdk --manifest-path cocode-rs/Cargo.toml  # Correct
cd cocode-rs && cargo build                         # Avoid (stay in codex/)
```

### Error Handling

**Use `cocode-error` for cocode-rs crates:**

| Crate Category        | Error Type |
|-----------------------|------------|
| common/, core/        | `cocode-error` + snafu (NO custom constructors) |
| provider-sdks/, utils/ | `anyhow::Result` (avoids reverse dependency on common) |

**Required pattern:**

```rust
use cocode_error::{ErrorExt, Location, StatusCode, stack_trace_debug};
use snafu::Snafu;

#[stack_trace_debug]  // Must be BEFORE #[derive(Snafu)]
#[derive(Snafu)]
#[snafu(visibility(pub(crate)), module)]  // Hide snafu types
pub enum MyError {
    #[snafu(display("IO error: {message}"))]
    Io {
        message: String,
        #[snafu(source)]
        source: std::io::Error,
        #[snafu(implicit)]
        location: Location,
    },

    #[snafu(display("Internal error: {message}"))]
    Internal {
        message: String,
        #[snafu(implicit)]
        location: Location,
    },
}

impl ErrorExt for MyError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Io { .. } => StatusCode::IoError,
            Self::Internal { .. } => StatusCode::Internal,
        }
    }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
```

**Key rules:**
- `#[stack_trace_debug]` BEFORE `#[derive(Snafu)]`
- `#[snafu(visibility(pub(crate)), module)]` to hide snafu types
- All variants must have `#[snafu(implicit)] location: Location`
- Do NOT write custom constructor functions — use snafu context selectors directly
- Implement `ErrorExt` with appropriate `StatusCode`

**Snafu usage patterns (use context selectors from `my_error` module):**

```rust
use crate::error::my_error::*;
use snafu::ResultExt;

// 1. .context() — variant has #[snafu(source)], wraps the original error
fs::read(path).context(IoSnafu { message: "read config" })?;

// 2. .map_err() — variant has NO source (e.g., PoisonError is !Send, no useful info)
lock.read().map_err(|e| InternalSnafu { message: format!("lock poisoned: {e}") }.build())?;

// 3. .fail() — generate error from condition, no source error
return NotFoundSnafu { kind: NotFoundKind::Model, name }.fail();

// 4. ensure! — conditional fail
snafu::ensure!(valid, NotFoundSnafu { kind: NotFoundKind::Provider, name: "x" });
```

**StatusCode Categories (5-digit: XX_YYY):**

| Range | Category | Examples |
|-------|----------|----------|
| 00_xxx | Success | Ok |
| 01_xxx | Common | Unknown, Internal, Cancelled |
| 02_xxx | Input | InvalidArguments, ParseError |
| 03_xxx | IO | IoError, FileNotFound |
| 04_xxx | Network | NetworkError, ServiceUnavailable |
| 05_xxx | Auth | AuthenticationFailed, PermissionDenied |
| 10_xxx | Config | InvalidConfig |
| 11_xxx | Provider | ProviderNotFound, ModelNotFound, StreamError |
| 12_xxx | Resource | RateLimited, Timeout |

**Reference:** See `cocode-rs/common/error/README.md` for full StatusCode list.

### Pre-Commit Requirements

**ALWAYS run before any commit:**

```bash
cargo fmt --manifest-path cocode-rs/Cargo.toml    # Format (auto, no approval)
cargo build --manifest-path cocode-rs/Cargo.toml  # REQUIRED - catches downstream issues
```

**If changed provider-sdks or core crates, ALSO run (ask user first):**

```bash
cargo test --manifest-path cocode-rs/Cargo.toml --all-features
```

## cocode-rs Crate Guide

### Common Crates (4 total)

| Crate | Purpose | Key Types |
|-------|---------|-----------|
| `protocol` | Foundational types for LLM interactions | `Capability`, `ModelInfo`, `ProviderType`, `LoopConfig`, `LoopEvent`, `ToolConfig` |
| `config` | Layered config: JSON files + env vars + runtime overrides | `ConfigManager`, `JsonConfig`, `EnvLoader` |
| `error` | Unified error handling with stack traces | `StatusCode`, `ErrorExt`, `Location` |
| `otel` | OpenTelemetry metrics and tracing | `OtelConfig`, `init_tracing()` |

**protocol** is the most important - defines all shared types used across crates:
- **Model types:** `Capability`, `ModelInfo`, `ReasoningEffort`, `TruncationMode`
- **Provider types:** `ProviderType`, `ProviderInfo`, `WireApi`
- **Feature types:** `Feature`, `Stage` (GA/Beta/Preview/Internal)
- **Config types:** `LoopConfig`, `ThinkingLevel`, `ToolConfig`, `PlanModeConfig`
- **Event types:** `LoopEvent`, `TokenUsage`, `AgentProgress`

### Provider SDKs (6 total)

| Crate | Purpose | API |
|-------|---------|-----|
| `hyper-sdk` | **Main SDK**: Multi-provider client, streaming, tool calling | All providers |
| `anthropic` | Anthropic Claude API SDK | Messages API |
| `openai` | OpenAI SDK | Responses API |
| `google-genai` | Google Gemini SDK | GenerateContent API |
| `volcengine-ark` | Volcengine Ark SDK | Chat API |
| `z-ai` | ZhipuAI/Z.AI SDK | Chat API |

**hyper-sdk** is the main development focus - it provides:
- Unified client for all providers
- Streaming response handling
- Tool calling abstractions
- Provider-specific request/response transformations

### Utils (14 total)

| Crate | Purpose |
|-------|---------|
| `file-ignore` | .gitignore-aware file filtering |
| `file-search` | Fuzzy file search (ripgrep-based) |
| `apply-patch` | Unified diff/patch application |
| `git` | Git operations wrapper |
| `keyring-store` | Secure credential storage |
| `pty` | Pseudo-terminal handling |
| `diff` | Diff generation and formatting |
| `checksum` | File checksum utilities |
| `normalize-path` | Cross-platform path normalization |
| `project-root` | Project root detection |
| `relative-path` | Relative path utilities |
| `temp-file` | Temporary file management |
| `try-to-own` | Ownership conversion utilities |
| `winnow-utils` | Parser utilities |

### Crate Structure

```
cocode-rs/
├─ common/
│  ├─ protocol/       → Foundational types (Model, Provider, Config, Events)
│  ├─ config/         → Layered config system
│  ├─ error/          → Unified error handling
│  └─ otel/           → OpenTelemetry integration
├─ provider-sdks/
│  ├─ hyper-sdk/      → Central multi-provider SDK (main development)
│  ├─ anthropic/      → Anthropic API SDK
│  ├─ openai/         → OpenAI API SDK
│  ├─ google-genai/   → Google GenAI SDK
│  ├─ volcengine-ark/ → Volcengine Ark SDK
│  └─ z-ai/           → Z.AI SDK
└─ utils/             → 14 utility crates
```

## Protocol Types Quick Reference

**Model Configuration:**
```rust
// Reasoning effort (ordered: None < Minimal < Low < Medium < High < XHigh)
pub enum ReasoningEffort { None, Minimal, Low, Medium, High, XHigh }

// Unified thinking level (model-level, not app-level config)
pub struct ThinkingLevel {
    pub effort: ReasoningEffort,
    pub budget_tokens: Option<i32>,
    pub max_output_tokens: Option<i32>,
    pub interleaved: bool,
}

// Content truncation strategy
pub enum TruncationMode { Auto, Disabled }

// Model capabilities
pub struct Capability { pub name: String, pub enabled: bool }
```

**Provider Configuration:**
```rust
// Supported providers
pub enum ProviderType { Anthropic, OpenAI, Google, Volcengine, ZhipuAI, Custom }

// Wire protocol
pub enum WireApi { Anthropic, OpenAI, Google, Custom }
```

**Loop Configuration:**
```rust
pub struct LoopConfig {
    pub model: String,
    pub provider: ProviderType,
    pub thinking: Option<ThinkingLevel>,
    pub tools: ToolConfig,
    pub plan_mode: Option<PlanModeConfig>,
}
```

**Events:**
```rust
pub enum LoopEvent {
    Started { session_id: String },
    MessageReceived { content: String },
    ToolCall { name: String, input: Value },
    TokenUsage { input: i32, output: i32 },
    Completed { reason: String },
    Error { code: StatusCode, message: String },
}
```

## Architecture Patterns

### Retrieval Pattern (from codex-rs reference)
- Dual storage: LanceDB vectors + SQLite metadata
- BM25 + vector hybrid search
- AST-aware code chunking

### Concurrency
- SQLite-based locks with timeout
- Async-safe with `tokio::task::spawn_blocking` for blocking ops

### Error Recovery
- Graceful degradation when APIs fail
- Checkpoint recovery for long operations
- Retry with exponential backoff for transient errors

## Code Conventions (from AGENTS.md)

**ALWAYS:**
- Use `i32`/`i64` (NEVER `u32`/`u64`)
- Inline format args: `format!("{var}")` not `format!("{}", var)`
- Add `Send + Sync` bounds to traits used with `Arc<dyn Trait>`
- Compare entire objects in tests (not field-by-field)
- Add `#[serde(default)]` for optional config fields
- Add `#[derive(Default)]` for structs used with `..Default::default()`

**NEVER:**
- Use `.unwrap()` in non-test code (use `?` or `.expect("reason")`)
- Use `.white()` in TUI code (breaks theme)
- Modify `CODEX_SANDBOX_*` environment variables
- Commit without user explicitly requesting

**Comments:**
- Keep concise - describe purpose, not implementation details
- Field docs: 1-2 lines max, no example configs/commands
- Code comments: state intent only when non-obvious

## Development Workflow

### Standard Iteration (from codex/ directory)

```bash
# 1. Make changes

# 2. Format (auto)
cargo fmt --manifest-path cocode-rs/Cargo.toml

# 3. Quick check
cargo check -p <crate> --manifest-path cocode-rs/Cargo.toml

# 4. Test
cargo test -p <crate> --manifest-path cocode-rs/Cargo.toml

# 5. Fix lints (ask user first)
cargo clippy -p <crate> --manifest-path cocode-rs/Cargo.toml --fix

# 6. Pre-commit (REQUIRED)
cargo build --manifest-path cocode-rs/Cargo.toml
```

### Common Commands

```bash
# Build and check (from codex/)
cargo build --manifest-path cocode-rs/Cargo.toml
cargo check -p hyper-sdk --manifest-path cocode-rs/Cargo.toml
cargo test -p hyper-sdk --manifest-path cocode-rs/Cargo.toml
cargo fmt --manifest-path cocode-rs/Cargo.toml
cargo clippy --manifest-path cocode-rs/Cargo.toml

# Test specific crate
cargo test -p protocol --manifest-path cocode-rs/Cargo.toml
cargo test -p anthropic-sdk --manifest-path cocode-rs/Cargo.toml

# Full test suite (ask user first)
cargo test --manifest-path cocode-rs/Cargo.toml --all-features
```

### Adding New Provider Support

1. Create provider module in `provider-sdks/hyper-sdk/src/providers/`
2. Implement request/response transformations
3. Add configuration types in `common/protocol/src/`
4. Register provider in hyper-sdk client
5. Add tests

## codex-rs Reference (Legacy)

Use codex-rs as reference when implementing similar features in cocode-rs:

| Feature | Reference Path |
|---------|----------------|
| Multi-provider adapters | `codex-rs/core/src/adapters/` |
| Streaming handling | `codex-rs/codex-api/` |
| Code search/retrieval | `codex-rs/retrieval/` |
| Tool implementations | `codex-rs/core/src/tools/` |
| TUI components | `codex-rs/tui/` |

## Quality Check Levels

1. **Iteration:** `cargo check -p <crate> --manifest-path cocode-rs/Cargo.toml` - fast feedback
2. **Pre-commit:** `cargo build --manifest-path cocode-rs/Cargo.toml` - **MANDATORY**
3. **Core changes:** `cargo test --manifest-path cocode-rs/Cargo.toml --all-features` - ask user first

## Quick Reference

```bash
# Essential (from codex/ directory)
cargo fmt --manifest-path cocode-rs/Cargo.toml                    # Format
cargo check -p hyper-sdk --manifest-path cocode-rs/Cargo.toml     # Quick check
cargo build --manifest-path cocode-rs/Cargo.toml                  # Pre-commit REQUIRED
cargo test -p hyper-sdk --manifest-path cocode-rs/Cargo.toml      # Test

# Avoid
.unwrap()              # Use ? or .expect()
u32/u64                # Use i32/i64
cd cocode-rs/          # Stay in codex/ directory
```

## Documentation

| Type | Location |
|------|----------|
| User docs | `docs/` (getting-started.md, config.md, sandbox.md) |
| Dev conventions | `AGENTS.md` |
| SDK docs | `cocode-rs/provider-sdks/hyper-sdk/docs/` |
| Error codes | `cocode-rs/common/error/README.md` |

## Git Workflow

**ONLY commit when user explicitly requests.**

When committing:
1. Check `git status`, `git diff`, `git log` (for style)
2. Run `cargo build --manifest-path cocode-rs/Cargo.toml` first
3. Follow repo commit message conventions

See `AGENTS_cocode.md` for complete Rust/testing conventions.
