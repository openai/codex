# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Overview

**Cocode** - Multi-provider LLM SDK and utilities. Main development in `cocode-rs/` (20 crates).

```
codex/
├── cocode-rs/     → Main Rust workspace (ALL development here) - 20 crates
├── codex-rs/      → Legacy workspace (reference only) - 59+ crates
├── sdk2/          → Python Agent SDK (see sdk2/CLAUDE.md)
├── docs/          → User documentation
└── AGENTS.md      → Rust conventions (READ THIS)
```

**IMPORTANT:** Read `AGENTS.md` for detailed Rust conventions. This file covers high-level architecture only.

**Note:** `codex-rs/` is kept as reference for implementing similar features in `cocode-rs/`. Do not actively develop in `codex-rs/`.

## Critical Rules

### Working Directory

**Run cargo commands from `codex/` directory using `--manifest-path`:**

```bash
cargo build --manifest-path cocode-rs/Cargo.toml   # ✅ Correct
cargo check -p hyper-sdk --manifest-path cocode-rs/Cargo.toml  # ✅ Correct
cd cocode-rs && cargo build                         # ❌ Avoid (stay in codex/)
```

### Error Handling

**Use `cocode-error` for cocode-rs core crates:**

| Crate Category         | Error Type |
|------------------------|------------|
| common/                | `cocode-error` + custom error enum |
| provider-sdks/, utils/ | `anyhow::Result` (避免反向依赖 common) |

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
        #[snafu(implicit)]
        location: Location,
    },
}

// Library-agnostic public constructors
impl MyError {
    #[track_caller]
    pub fn io(message: impl Into<String>) -> Self {
        Self::Io {
            message: message.into(),
            location: caller_location(),
        }
    }
}

impl ErrorExt for MyError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Io { .. } => StatusCode::IoError,
        }
    }
    fn as_any(&self) -> &dyn std::any::Any { self }
}
```

**Key rules:**
- `#[stack_trace_debug]` BEFORE `#[derive(Snafu)]`
- `#[snafu(visibility(pub(crate)), module)]` to hide snafu types
- All variants must have `#[snafu(implicit)] location: Location`
- Provide clean public constructors with `#[track_caller]`
- Implement `ErrorExt` with appropriate `StatusCode`

**Reference:** See `cocode-rs/common/error/README.md` for StatusCode categories.

**For codex-rs reference (legacy):**
- Core/business logic (core/, cli/, exec/, tui/, app-server/) → `CodexErr`
- Utilities/MCP/tests (mcp-*/, utils/, tests/) → `anyhow::Result`

### Pre-Commit Requirements

**ALWAYS run before any commit:**

```bash
cargo fmt --manifest-path cocode-rs/Cargo.toml    # Format (auto, no approval)
cargo build --manifest-path cocode-rs/Cargo.toml  # ⭐ REQUIRED - catches downstream issues
```

**If changed provider-sdks or core crates, ALSO run (ask user first):**

```bash
cargo test --manifest-path cocode-rs/Cargo.toml --all-features
```

### Key Development Files (cocode-rs)

**Main development focus - hyper-sdk:**

| File | Purpose |
|------|---------|
| `provider-sdks/hyper-sdk/src/client.rs` | Multi-provider HTTP client |
| `provider-sdks/hyper-sdk/src/lib.rs` | SDK public API |
| `provider-sdks/hyper-sdk/src/providers/` | Provider implementations |
| `provider-sdks/hyper-sdk/src/config/` | Configuration types |

**Strategy:**
1. Keep provider implementations modular
2. Use trait-based abstractions for provider differences
3. Implement streaming support consistently across providers

### Code Conventions (from AGENTS.md)

**ALWAYS:**
- Use `i32`/`i64` (NEVER `u32`/`u64`)
- Inline format args: `format!("{var}")`
- Add `Send + Sync` bounds to traits used with `Arc<dyn Trait>`
- Compare entire objects in tests (not field-by-field)
- Add `#[serde(default)]` for optional config fields
- Add `#[derive(Default)]` for structs used with `..Default::default()`

**NEVER:**
- Use `.unwrap()` in non-test code
- Use `.white()` in TUI code (breaks theme)
- Modify `CODEX_SANDBOX_*` environment variables
- Commit without user explicitly requesting

**Comments:**
- Keep concise - describe purpose, not implementation details
- Field docs: 1-2 lines max, no example configs/commands
- Code comments: state intent only when non-obvious

## Architecture Quick Reference

### cocode-rs Crates (20 total)

```
cocode-rs/
├─ provider-sdks/
│  ├─ hyper-sdk/      → Central multi-provider SDK (main development)
│  ├─ anthropic/      → Anthropic API SDK
│  ├─ openai/         → OpenAI API SDK
│  ├─ google-genai/   → Google GenAI SDK
│  ├─ volcengine-ark/ → Volcengine Ark SDK
│  └─ z-ai/           → Z.AI (Zhipu) SDK
├─ file-ignore/       → .gitignore-aware file filtering
├─ file-search/       → Fuzzy file search (ripgrep-based)
└─ utils/             → 14 utility crates
```

### hyper-sdk Structure (Main Focus)

| Module | Purpose |
|--------|---------|
| `src/client.rs` | Multi-provider HTTP client with streaming |
| `src/lib.rs` | Public API exports |
| `src/config/` | Provider configuration types |
| `src/providers/` | Provider-specific implementations |
| `docs/` | SDK documentation |

### Key Files for Navigation

```
# hyper-sdk (main development)
provider-sdks/hyper-sdk/src/lib.rs       → SDK public API
provider-sdks/hyper-sdk/src/client.rs    → Multi-provider client
provider-sdks/hyper-sdk/src/config/      → Configuration types
provider-sdks/hyper-sdk/src/providers/   → Provider implementations
provider-sdks/hyper-sdk/docs/            → Documentation

# Individual Provider SDKs
provider-sdks/anthropic/src/lib.rs       → Anthropic SDK
provider-sdks/openai/src/lib.rs          → OpenAI SDK
provider-sdks/google-genai/src/lib.rs    → Google GenAI SDK
provider-sdks/volcengine-ark/src/lib.rs  → Volcengine Ark SDK
provider-sdks/z-ai/src/lib.rs            → Z.AI SDK

# Utilities
file-ignore/src/lib.rs                   → Gitignore filtering
file-search/src/lib.rs                   → Fuzzy file search
```

### codex-rs Reference (Legacy - 59+ crates)

For reference when implementing similar features in cocode-rs:

```
codex-rs/
├─ core/           → Business logic, conversation, tools
├─ protocol/       → Message types, shared structs
├─ cli/            → Binary entry, arg parsing
├─ tui/            → Ratatui interface
├─ exec/           → Headless mode
├─ codex-api/      → Multi-provider LLM API (reference for hyper-sdk)
├─ retrieval/      → Code search (BM25 + vector + AST)
└─ provider-sdks/  → Provider SDK implementations (reference)
```

## Development Workflow

### Standard Iteration (from codex/ directory)

```bash
# 1. Make changes

# 2. Format (auto)
cargo fmt --manifest-path cocode-rs/Cargo.toml

# 3. Quick check
cargo check -p hyper-sdk --manifest-path cocode-rs/Cargo.toml

# 4. Test
cargo test -p hyper-sdk --manifest-path cocode-rs/Cargo.toml

# 5. Fix lints (ask user first)
cargo clippy -p hyper-sdk --manifest-path cocode-rs/Cargo.toml --fix

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

# Test specific provider SDK
cargo test -p anthropic-sdk --manifest-path cocode-rs/Cargo.toml
cargo test -p openai-sdk --manifest-path cocode-rs/Cargo.toml

# Full test suite (ask user first)
cargo test --manifest-path cocode-rs/Cargo.toml --all-features
```

## Adding New Provider Support (hyper-sdk)

**Implementation steps:**

1. Create provider module in `provider-sdks/hyper-sdk/src/providers/`
2. Implement provider-specific request/response transformations
3. Add configuration types in `src/config/`
4. Register provider in the client
5. Add tests

**Key traits to implement:**
- Request transformation (messages → provider format)
- Response streaming (SSE parsing)
- Error mapping

## Testing Patterns

### Unit Tests

```bash
# Test specific crate
cargo test -p hyper-sdk --manifest-path cocode-rs/Cargo.toml

# Test with specific feature
cargo test -p hyper-sdk --manifest-path cocode-rs/Cargo.toml --features anthropic
```

### Integration Tests

Place integration tests in `provider-sdks/hyper-sdk/tests/` directory.

## Common Pitfalls

```rust
// ❌ Avoid
cd cocode-rs && cargo build        // Stay in codex/ directory
let x: u32 = 42;                   // Unsigned int
format!("{}", var)                 // Not inlined
data.unwrap()                      // In non-test code
cargo check -p only                // Pre-commit needs full build

// ✅ Prefer
cargo build --manifest-path cocode-rs/Cargo.toml  // From codex/
let x: i32 = 42;
format!("{var}")
data.expect("reason") or ?
cargo build before commit
```

## Quality Check Levels

1. **Iteration:** `cargo check -p <crate> --manifest-path cocode-rs/Cargo.toml` - fast feedback
2. **Pre-commit:** `cargo build --manifest-path cocode-rs/Cargo.toml` - **MANDATORY**
3. **Core changes:** `cargo test --manifest-path cocode-rs/Cargo.toml --all-features` - ask user first

## Documentation

**User docs:** `docs/` (getting-started.md, config.md, sandbox.md)
**Dev docs:** `AGENTS.md` (Rust conventions)
**SDK docs:** `cocode-rs/provider-sdks/hyper-sdk/docs/`

## Git Workflow

**ONLY commit when user explicitly requests.**

When committing:
1. Check `git status`, `git diff`, `git log` (for style)
2. Run `cargo build --manifest-path cocode-rs/Cargo.toml` first
3. Follow repo commit message conventions

## Quick Reference

```bash
# Essential (from codex/ directory)
cargo fmt --manifest-path cocode-rs/Cargo.toml                    # Format
cargo check -p hyper-sdk --manifest-path cocode-rs/Cargo.toml     # Quick check
cargo build --manifest-path cocode-rs/Cargo.toml                  # ⭐ Pre-commit REQUIRED
cargo test -p hyper-sdk --manifest-path cocode-rs/Cargo.toml      # Test

# Avoid
.unwrap()              # Use ? or .expect()
u32/u64                # Use i32/i64
cd cocode-rs/          # Stay in codex/ directory
```

## codex-rs Reference

When implementing features in cocode-rs, refer to codex-rs for:
- Multi-provider adapter patterns (`codex-rs/core/src/adapters/`)
- Streaming response handling (`codex-rs/codex-api/`)
- Provider SDK implementations (`codex-rs/provider-sdks/`)

See `AGENTS.md` for complete Rust/testing conventions.
