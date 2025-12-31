# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Overview

**Codex** - OpenAI's coding agent CLI. Rust workspace with 42+ crates in `codex-rs/`.

```
codex/
├── codex-rs/      → Main Rust workspace (ALL development here)
├── docs/          → User documentation
└── AGENTS.md      → Rust/codex-rs specific rules (READ THIS)
```

**IMPORTANT:** Read `AGENTS.md` for detailed Rust conventions. This file covers high-level architecture only.

## Critical Rules

### Working Directory

**ALWAYS run cargo/just commands from `codex-rs/` directory:**

```bash
cd codex-rs && cargo build   # ✅ Correct
cargo build                  # ❌ Wrong
```

### Error Handling

**ALWAYS use the correct error type:**
- Core/business logic (core/, cli/, exec/, tui/, app-server/) → `CodexErr`
- Utilities/MCP/tests (mcp-*/, utils/, tests/) → `anyhow::Result`

Convert: `.map_err(|e| CodexErr::Fatal(e.to_string()))?`

### Pre-Commit Requirements

**ALWAYS run before any commit:**

```bash
just fmt                  # Format (auto, no approval)
cargo build              # ⭐ REQUIRED - catches downstream issues
```

**If changed core/protocol, ALSO run (ask user first):**

```bash
cargo test --all-features
```

### Extension Pattern (Upstream Sync)

**⚠️ CRITICAL: This repo syncs upstream regularly. Minimize conflicts by preferring extension files.**

**PREFER `*_ext.rs` for new features to minimize modifications to existing files:**

```bash
# ❌ Avoid: Large modifications to existing file
core/src/tools/spec.rs           # Adding 200+ lines → merge conflicts

# ✅ Prefer: Extension pattern
core/src/tools/spec_ext.rs       # Define/register/test new tool (200+ lines)
core/src/tools/spec.rs           # Minimal integration call (1-2 lines)
```

**Pattern:**
1. New functionality → `module_ext.rs` (bulk of code)
2. Original file → minimal import/integration (1-2 lines)
3. Tests and registration → in `module_ext.rs`

**When to use:**
- Adding new tools, handlers, features
- Original file would need 20+ lines of changes
- Code can be isolated and called from original

**When NOT to use:**
- Complete standalone features (e.g., `core/src/adapters/`)
- Refactoring existing logic
- Small fixes (< 10 lines)

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

### Core Crates (42+ total)

```
codex-rs/
├─ core/           → Business logic, conversation, tools (CodexErr)
├─ protocol/       → Message types, shared structs (anyhow)
├─ cli/            → Binary entry, arg parsing (CodexErr)
├─ tui/            → Ratatui interface (CodexErr, see tui/styles.md)
├─ exec/           → Headless mode (CodexErr)
├─ app-server/     → HTTP server for IDE (CodexErr)
├─ mcp-server/     → MCP server (anyhow)
├─ utils/          → git, cache, pty, tokenizer (anyhow)
└─ common/         → Config, model presets (anyhow)
```

### Key Files for Navigation

```
core/src/error.rs                  → CodexErr definition
core/src/codex_conversation.rs     → Main conversation flow
core/src/tools/spec.rs             → Tool registration
core/src/config/mod.rs             → Config schema
protocol/src/protocol.rs           → SQ/EQ message types
tui/styles.md                      → TUI styling guide
core/src/adapters/mod.rs           → ProviderAdapter trait, AdapterContext
core/src/adapters/registry.rs      → Global adapter registry
core/src/adapters/http.rs          → HTTP transport with streaming
core/src/adapters/gpt_openapi/     → Built-in adapters (GptAdapter, GeminiAdapter)
core/src/client_ext.rs             → Adapter integration entry point
```

## Development Workflow

### Standard Iteration

```bash
cd codex-rs

# 1. Make changes
# 2. Format (auto)
just fmt

# 3. Quick check
cargo check -p <crate>

# 4. Test
cargo test -p <crate>

# 5. Fix lints (ask user first)
just fix -p <crate>

# 6. Pre-commit (REQUIRED)
cargo build
```

### Common Commands

```bash
just codex                 # Launch TUI
just exec "prompt"         # Headless execution
just tui                   # TUI explicitly
just fmt                   # Format (no approval needed)
just clippy                # Lint check
just fix -p <crate>        # Fix lints (ask user)
just mcp-server-run        # MCP server mode

cargo test -p <crate>                     # Test specific crate
cargo test --all-features                 # Full suite (ask user)
cargo insta pending-snapshots -p codex-tui # Check TUI snapshots
```

## Adding New Tools

**Implementation steps:**

1. `protocol/src/config_types.rs` → Config struct with `#[derive(Default)]` + `#[serde(default)]`
2. `core/src/tools/my_tool.rs` → Handler (must be `Send + Sync`)
3. `core/src/tools/spec.rs` → Register in `build_specs()`
4. `core/src/config/mod.rs` → Add field to `Config`
5. Tests using `anyhow`

**If tool needs user notifications (optional):**

1. `protocol/src/protocol.rs` → Add `EventMsg` variant
2. **Update ALL matches** in: `mcp-server/src/codex_tool_runner.rs`, `exec/src/event_processor_with_human_output.rs`, `tui/src/chatwidget.rs`
3. **Run `cargo build`** to catch missing arms

**Batch error discovery (IMPORTANT):**

```bash
# Adding Config field breaks ALL test inits
cargo check 2>&1 | tee errors.txt     # Find all at once
rg "Config \{" core/src --type rust   # Locate all inits
# Fix simultaneously (saves 70% time)
```

## Multi-Provider Adapters

### Architecture

```
Config (adapter = "gpt_openapi")
    ↓
ModelClient::stream() [client.rs:159]
    ↓ if provider.ext.adapter.is_some()
client_ext::stream_with_adapter()
    ↓
AdapterHttpClient::stream_with_adapter() [http.rs]
    ↓
adapter.transform_request() → HTTP → adapter.transform_response_chunk()
```

### Key Files

| File | Purpose |
|------|---------|
| `adapters/mod.rs` | `ProviderAdapter` trait, `AdapterContext`, `RequestContext` |
| `adapters/registry.rs` | Global lazy registry, `get_adapter()`, `register_adapter()` |
| `adapters/http.rs` | HTTP layer, SSE streaming, endpoint routing |
| `adapters/gpt_openapi/gpt_adapter.rs` | OpenAI Responses API adapter |
| `adapters/gpt_openapi/gemini_adapter.rs` | Google Gemini Chat API adapter |
| `client_ext.rs` | Parameter resolution, adapter entry point |

### Built-in Adapters

| Adapter | Name | Wire API | Features |
|---------|------|----------|----------|
| GptAdapter | `"gpt_openapi"` | `responses` | SSE streaming, `previous_response_id` |
| GeminiAdapter | `"gemini_openapi"` | `chat` | Thinking budgets, non-streaming only |

### Configuration

```toml
[model_providers.my_provider]
adapter = "gpt_openapi"           # Selects adapter
wire_api = "responses"            # Must match adapter requirement
base_url = "https://..."
model_name = "gpt-4"              # REQUIRED when using adapter

[model_providers.my_provider.adapter_config]
custom_key = "value"              # Adapter-specific settings

[model_providers.my_provider.model_parameters]
temperature = 0.7                 # Sampling parameters
```

### Adding New Adapters

1. Create `adapters/gpt_openapi/my_adapter.rs`:
   - Implement `ProviderAdapter` trait (must be `Send + Sync`)
   - Key methods: `name()`, `transform_request()`, `transform_response_chunk()`
   - Validate wire_api in `validate_provider()`

2. Export in `adapters/gpt_openapi/mod.rs`

3. Register in `adapters/registry.rs`:
   ```rust
   registry.register(Arc::new(MyAdapter::new()));
   ```

4. **Tests: Keep minimal** - only test:
   - Request transformation (1-2 cases)
   - Response parsing (1 success, 1 error case)
   - Config validation (1 valid, 1 invalid)

### ProviderAdapter Trait (Key Methods)

```rust
trait ProviderAdapter: Send + Sync + Debug {
    fn name(&self) -> &str;                    // Unique ID for registry
    fn validate_provider(&self, ...) -> Result<()>;  // Check wire_api compatibility
    fn transform_request(&self, prompt, context, provider) -> Result<JsonValue>;
    fn transform_response_chunk(&self, chunk, ctx, provider) -> Result<Vec<ResponseEvent>>;
    fn supports_previous_response_id(&self) -> bool;  // Conversation continuity
}
```

### Error Types

- Config mismatch (wrong wire_api) → `CodexErr::Fatal`
- `context_length_exceeded` → `CodexErr::ContextWindowExceeded`
- `insufficient_quota` → `CodexErr::QuotaExceeded`
- `previous_response_not_found` → `CodexErr::PreviousResponseNotFound`

## Testing Patterns

### Integration Tests (core)

Use `core_test_support::responses`:

```rust
let mock = responses::mount_sse_once(&server, responses::sse(vec![
    responses::ev_response_created("resp-1"),
    responses::ev_function_call(call_id, "shell", &args),
])).await;

codex.submit(Op::UserTurn { ... }).await?;

let request = mock.single_request();
assert_eq!(request.function_call_output(call_id), expected);
```

**Helpers:** `single_request()`, `requests()`, `body_json()`, `input()`, `function_call_output(id)`

### Snapshot Tests (TUI)

```bash
cargo test -p codex-tui
cargo insta pending-snapshots -p codex-tui
cargo insta accept -p codex-tui  # Careful!
```

## TUI Development

**From tui/styles.md:**

**NEVER:**
- Use `.white()` (breaks theme)
- Use `Span::styled` when `.dim()`, `.bold()`, `.cyan()` work

**ALWAYS:**
- Use Stylize helpers: `"text".dim()`, `"text".red()`, `url.cyan().underlined()`
- Use `textwrap::wrap` for plain strings
- Use `word_wrap_lines` (from `tui/src/wrapping.rs`) for ratatui `Line`
- Use `pretty_assertions::assert_eq` in tests

## Common Pitfalls

```rust
// ❌ Avoid
cargo build                    // Not in codex-rs/
let x: u32 = 42;              // Unsigned int
format!("{}", var)            // Not inlined
data.unwrap()                 // In non-test code
"text".white()                // In TUI
cargo check -p only           // Pre-commit
// Add 200 lines to spec.rs   // Merge conflicts
/// Long field docs with example configs  // Verbose

// ✅ Prefer
cd codex-rs && cargo build
let x: i32 = 42;
format!("{var}")
data.expect("reason") or ?
"text".dim()
cargo build before commit
// Use spec_ext.rs (1-2 lines in spec.rs)  // Minimal conflicts
/// Brief field description (1-2 lines max)  // Concise
```

## Quality Check Levels

1. **Iteration:** `cargo check -p <crate>` - fast feedback
2. **Pre-commit:** `cargo build` - **MANDATORY** (catches all 42+ crates)
3. **Core changes:** `cargo test --all-features` - ask user first

## Documentation

**User docs:** `docs/` (getting-started.md, config.md, sandbox.md)
**Dev docs:** `AGENTS.md` (Rust rules), `tui/styles.md` (TUI styling)
**Install:** `npm i -g @openai/codex` or `brew install --cask codex`

## Git Workflow

**ONLY commit when user explicitly requests.**

When committing:
1. Check `git status`, `git diff`, `git log` (for style)
2. Run `cargo build` first
3. Follow repo commit message conventions

## Quick Reference

```bash
# Essential (from codex-rs/)
just fmt && cargo check -p <crate>  # Iteration
cargo build                         # ⭐ Pre-commit REQUIRED
just codex                          # Run TUI
just exec "prompt"                  # Run headless

# Avoid
.unwrap()              # Use ? or .expect()
u32/u64                # Use i32/i64
.white()               # Use .dim(), .cyan(), etc.
# Large file edits     # Prefer *_ext.rs pattern

# Extension pattern (minimize upstream conflicts)
# Large new feature → spec_ext.rs (bulk code) + spec.rs (1-2 line import)
# Small fix (< 10L)  → Direct edit OK
```

See `AGENTS.md` for complete Rust/testing conventions.
