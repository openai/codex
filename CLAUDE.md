# CLAUDE.md - Codex CLI Development Guide

## Project Overview

**Codex CLI** is OpenAI's AI-powered coding agent that runs locally in the terminal. It provides ChatGPT-level reasoning with the ability to execute code in a controlled sandbox, manipulate files, and iterate on solutions under version control.

- **Main implementation**: Rust (in `codex-rs/`)
- **License**: Apache-2.0
- **Package managers**: Cargo (Rust), pnpm (TypeScript/SDK)

## Quick Commands

```bash
# Navigate to Rust workspace
cd codex-rs

# Format code (run after every change)
just fmt

# Fix linter issues (scope to specific crate)
just fix -p <crate-name>

# Run tests for specific crate
cargo test -p <crate-name>

# Run all tests with nextest (faster)
just test

# Install development tools
just install

# View pending snapshot changes
cargo insta pending-snapshots -p codex-tui
```

## Project Structure

```
codex/
├── codex-rs/           # Main Rust implementation (40+ crates)
│   ├── cli/            # CLI multiplexer (codex-cli)
│   ├── core/           # Core agent logic (codex-core)
│   ├── tui/            # Terminal UI (codex-tui)
│   ├── exec/           # Headless execution (codex-exec)
│   ├── linux-sandbox/  # Linux sandboxing
│   └── ...             # Many more crates
├── codex-cli/          # Legacy TypeScript CLI (superseded)
├── sdk/typescript/     # TypeScript SDK for embedding
└── docs/               # User documentation
```

Crate names are prefixed with `codex-`. For example, the `core/` folder's crate is `codex-core`.

## Rust Code Conventions

### Formatting & Style

- **Always inline format! args**: Use `format!("text {var}")` not `format!("text {}", var)`
- **Collapse if statements**: Follow [collapsible_if](https://rust-lang.github.io/rust-clippy/master/index.html#collapsible_if)
- **Method references over closures**: Use `.map(String::as_str)` not `.map(|s| s.as_str())`
- **No unsigned integers**: Even for non-negative numbers, use signed types
- **Run `just fmt` after every change** - no approval needed

### TUI Styling (Ratatui)

- Use Stylize trait helpers: `"text".red()`, `"text".dim()`, `"text".cyan()`
- Avoid `.white()` and `.black()` - use default foreground
- Prefer `"text".into()` for simple spans
- Chain styles for readability: `url.cyan().underlined()`
- See `codex-rs/tui/styles.md` for detailed conventions

### Testing

- Use `pretty_assertions::assert_eq` for clearer diffs
- Compare entire objects, not field-by-field
- Snapshot tests use `cargo-insta` (especially in TUI)
- Test helpers: `core_test_support::responses` for integration tests

## Development Workflow

### Before Finalizing Changes

1. **Format code** (always, no approval needed):
   ```bash
   just fmt
   ```

2. **Fix linter issues** (scope to changed crate):
   ```bash
   just fix -p codex-tui
   ```

3. **Run tests** for the specific crate:
   ```bash
   cargo test -p codex-tui
   ```

4. **If you changed common, core, or protocol**, run full suite:
   ```bash
   cargo test --all-features
   ```

### Snapshot Testing

When UI output changes intentionally:

```bash
# Run tests to generate new snapshots
cargo test -p codex-tui

# Check pending snapshots
cargo insta pending-snapshots -p codex-tui

# Review .snap.new files, then accept if correct
cargo insta accept -p codex-tui
```

## Important Restrictions

### NEVER Modify Sandbox Code

- Do **NOT** add or modify code related to:
  - `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR`
  - `CODEX_SANDBOX_ENV_VAR`
- These environment variables control sandbox behavior
- Tests using these often early-exit due to sandbox limitations

### Documentation Updates

When adding or changing APIs, ensure the `docs/` folder is updated if applicable.

## Key Files

- `codex-rs/Cargo.toml` - Workspace configuration
- `codex-rs/rust-toolchain.toml` - Rust 1.90.0
- `codex-rs/clippy.toml` - Custom lint rules
- `codex-rs/justfile` - Task automation commands
- `codex-rs/tui/styles.md` - TUI styling guide
- `.prettierrc.toml` - TypeScript/JS formatting (80 chars, 2 spaces)

## Integration Test Patterns

Use helpers from `core_test_support::responses`:

```rust
let mock = responses::mount_sse_once(&server, responses::sse(vec![
    responses::ev_response_created("resp-1"),
    responses::ev_function_call(call_id, "shell", &args_json),
    responses::ev_completed("resp-1"),
])).await;

codex.submit(Op::UserTurn { ... }).await?;

let request = mock.single_request();
// Assert using request.function_call_output(call_id)
```

Prefer:
- `wait_for_event` over `wait_for_event_with_timeout`
- `mount_sse_once` over `mount_sse_once_match` or `mount_sse_sequence`

## Common Tasks

### Adding a New Feature

1. Check if it affects shared crates (common, core, protocol)
2. Write tests alongside implementation
3. Update `docs/` if user-facing
4. Run `just fmt` and `just fix -p <crate>`
5. Test: `cargo test -p <crate>`

### Fixing a Bug

1. Write a failing test first
2. Fix the bug
3. Ensure test passes
4. Run `just fmt` and `just fix -p <crate>`

### Working on TUI

1. Use snapshot tests for output validation
2. Follow Stylize conventions (see `tui/styles.md`)
3. Use text wrapping helpers from `tui/src/wrapping.rs`
4. Test with `cargo test -p codex-tui`
5. Review snapshots with `cargo insta`

## External Contributions

- Open an issue first for feature proposals
- Get OpenAI team approval before starting work
- CLA signature required
- Focus on bug fixes and security improvements
