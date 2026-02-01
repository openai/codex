# CLAUDE.md

This document provides guidance for AI assistants working with the Codex CLI codebase.

## Project Overview

**Codex CLI** is a local coding agent from OpenAI that runs on macOS, Linux, and Windows (via WSL2). The primary implementation is in Rust (`codex-rs/`), providing a zero-dependency native executable. The project also includes a TypeScript SDK (`sdk/typescript/`) and an MCP shell tool server (`shell-tool-mcp/`).

### Installation

```bash
npm i -g @openai/codex
# or
brew install --cask codex
```

## Repository Structure

```
codex/
├── codex-rs/              # Rust implementation (main codebase)
│   ├── cli/               # CLI multitool with subcommands
│   ├── core/              # Business logic library
│   ├── tui/               # Fullscreen TUI (Ratatui)
│   ├── tui2/              # Next-gen TUI implementation
│   ├── exec/              # Headless CLI for automation
│   ├── mcp-server/        # MCP server implementation
│   └── [other crates]     # Various utility and feature crates
├── codex-cli/             # npm package wrapper (delegates to Rust binary)
├── sdk/typescript/        # TypeScript SDK (@openai/codex-sdk)
├── shell-tool-mcp/        # MCP server for sandboxed shell commands
├── docs/                  # Documentation
├── scripts/               # Build and maintenance scripts
└── .github/workflows/     # CI/CD workflows
```

### Key Rust Crates

Crate names are prefixed with `codex-`. For example, the `core/` folder's crate is named `codex-core`.

| Crate | Purpose |
|-------|---------|
| `codex-core` | Business logic and agent orchestration |
| `codex-tui` | Terminal UI built with Ratatui |
| `codex-exec` | Non-interactive execution mode |
| `codex-cli` | CLI entry point combining TUI, exec, and utilities |
| `codex-protocol` | Wire protocol definitions |
| `codex-common` | Shared types and utilities |

## Build Systems

### Rust (Cargo) - Primary Development

```bash
cd codex-rs

# Build
cargo build

# Run the TUI
cargo run --bin codex -- "your prompt"

# Run in non-interactive mode
cargo run --bin codex -- exec "your prompt"
```

### Justfile Commands

The `justfile` at repository root runs commands in `codex-rs/`:

```bash
just fmt              # Format code (run automatically after changes)
just fix -p <crate>   # Fix clippy lints for a specific crate
just clippy           # Run clippy on all crates
just test             # Run tests via cargo-nextest
just codex "prompt"   # Run codex with a prompt
```

### Bazel - CI/Release Builds

```bash
just bazel-test            # Run all Bazel tests
just bazel-remote-test     # Run with remote execution
just build-for-release     # Build release binaries
```

### pnpm - JavaScript/TypeScript

```bash
pnpm install               # Install dependencies
pnpm run format            # Check formatting
pnpm run format:fix        # Fix formatting
```

## Development Workflow

### Making Changes

1. **Read before editing**: Always read files before proposing changes.
2. **Run `just fmt`** automatically after Rust code changes (no approval needed).
3. **Run `just fix -p <crate>`** before finalizing changes to fix linter issues.
4. **Run tests** for the specific crate changed:
   ```bash
   cargo test -p codex-tui
   ```
5. If changes affect `common`, `core`, or `protocol`, run the full test suite:
   ```bash
   cargo test --all-features
   ```

### Testing Guidelines

#### Snapshot Tests (Insta)

The repo uses `insta` for snapshot testing, especially in `codex-tui`:

```bash
cargo test -p codex-tui                      # Generate snapshots
cargo insta pending-snapshots -p codex-tui   # Check pending
cargo insta accept -p codex-tui              # Accept all new snapshots
```

Install if needed: `cargo install cargo-insta`

#### Test Assertions

- Use `pretty_assertions::assert_eq` for clearer diffs.
- Prefer comparing entire objects over individual fields.
- Avoid mutating process environment in tests.

#### Spawning Binaries in Tests

Use `codex_utils_cargo_bin::cargo_bin("...")` instead of `assert_cmd::Command::cargo_bin(...)` for Bazel compatibility.

#### Integration Tests (Core)

Use helpers from `core_test_support::responses`:

```rust
let mock = responses::mount_sse_once(&server, responses::sse(vec![
    responses::ev_response_created("resp-1"),
    responses::ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
    responses::ev_completed("resp-1"),
])).await;
```

## Coding Conventions

### Rust Style

#### General

- **Inline format args**: Always use `format!("{var}")` instead of `format!("{}", var)`.
- **Collapse if statements**: Per `clippy::collapsible_if`.
- **Method references over closures**: Per `clippy::redundant_closure_for_method_calls`.
- **Edition 2024**: The workspace uses Rust 2024 edition.
- **Import granularity**: `imports_granularity = "Item"` (one import per line).

#### Clippy Lints

Key denied lints (see `Cargo.toml` for full list):
- `unwrap_used`, `expect_used` (use proper error handling)
- `uninlined_format_args`
- `redundant_closure_for_method_calls`

#### TUI Style (Ratatui)

See `codex-rs/tui/styles.md` for complete guidelines.

**Colors:**
- Default foreground for most text
- `cyan` for user input tips, selection, status indicators
- `green` for success and additions
- `red` for errors, failures, deletions
- `magenta` for Codex branding
- `dim` for secondary text
- `bold` for headers

**Avoid:**
- Custom RGB colors
- `black`, `white` as foreground (use default)
- `blue`, `yellow` (not in style guide)

**Styling Helpers:**
```rust
// Preferred
"text".dim()
"text".red()
"text".cyan().underlined()
vec!["prefix".dim(), "content".into()].into()

// Use textwrap::wrap for plain strings
// Use tui/src/wrapping.rs helpers for ratatui Lines
```

### TypeScript Style

- ESLint + Prettier for linting and formatting
- Jest for testing
- Node.js 18+ required

```bash
cd sdk/typescript
pnpm run lint
pnpm run test
pnpm run format
```

## Important Sandbox Notes

**NEVER add or modify code related to these environment variables:**
- `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR`
- `CODEX_SANDBOX_ENV_VAR`

These variables control sandbox behavior during agent operation:
- `CODEX_SANDBOX_NETWORK_DISABLED=1` is set when using the shell tool
- `CODEX_SANDBOX=seatbelt` is set when spawning processes under macOS Seatbelt

Existing code using these variables handles test skipping for sandbox limitations.

## Documentation

When making API changes, ensure documentation in `docs/` is updated.

Key documentation files:
- `docs/config.md` - Configuration options
- `docs/install.md` - Installation and building
- `docs/contributing.md` - Contribution guidelines
- `docs/exec.md` - Non-interactive mode
- `docs/sandbox.md` - Sandbox policies

## CI/CD

### Rust CI (`rust-ci.yml`)

- Runs on PRs and pushes to main
- Tests on macOS, Linux (musl/glibc), Windows
- Includes format checking, clippy, cargo-shear

### General CI (`ci.yml`)

- pnpm install and format checking
- README ASCII validation
- npm package staging

## Quick Reference

| Task | Command |
|------|---------|
| Format Rust | `just fmt` |
| Lint Rust | `just fix -p <crate>` |
| Test crate | `cargo test -p <crate>` |
| Full tests | `cargo test --all-features` or `just test` |
| Run TUI | `cargo run --bin codex -- "prompt"` |
| Run headless | `cargo run --bin codex -- exec "prompt"` |
| Format JS/TS | `pnpm run format:fix` |
| Verbose logs | Set `RUST_LOG=codex_core=debug` |
| View TUI logs | `tail -F ~/.codex/log/codex-tui.log` |
