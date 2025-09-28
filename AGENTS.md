# Repository Guidelines

## Project Structure & Modules
- `codex-rs/`: Rust workspace. Crates are prefixed `codex-` (e.g., `codex-core`, `codex-tui`, `codex-exec`). Tests live next to each crate under `tests/`.
- `codex-cli/`: Lightweight Node.js CLI wrapper (`bin/codex.js`).
- `docs/`: User and developer docs. `scripts/`: repo maintenance.

## Build, Test, and Development
- Rust build: `cd codex-rs && cargo build -p codex-tui` (swap package as needed).
- Task helpers: `cd codex-rs && just help` → discover tasks. Common:
  - Format: `just fmt`
  - Lint/fix (scoped): `just fix -p codex-tui`
- Tests (scoped first): `cargo test -p codex-tui`
- Full suite when touching shared crates (`common`, `core`, `protocol`): `cargo test --all-features`
- Repo formatting (JS/Markdown): from repo root `pnpm run format` or `pnpm run format:fix`

## Coding Style & Naming Conventions
- Crates: prefix with `codex-` and keep names descriptive (e.g., `codex-file-search`).
- Rust: follow `rustfmt` and Clippy. When using `format!`, inline variables directly like `format!("Failed on {path}")`.
- TUI (ratatui): use Stylize helpers (e.g., `"OK".green()`, `"path".dim()`) rather than manual `Style` construction. See `codex-rs/tui/styles.md`.
- JavaScript: Prettier defaults; no custom linting.

## Testing Guidelines
- Framework: Rust `cargo test`; snapshot tests via `insta` in `codex-rs/tui`.
- Snapshot flow: `cargo test -p codex-tui` → `cargo insta pending-snapshots -p codex-tui` → review `.snap.new` → `cargo insta accept -p codex-tui` (install with `cargo install cargo-insta`).
- Scope tests to the changed crate first; run workspace tests only when touching shared code.

## Commit & Pull Requests
- Use Conventional Commits (see `cliff.toml`). Examples: `feat(tui): add model picker`, `fix(exec): handle sandbox error`.
- PRs: include a clear description, link issues, note user impact, and attach screenshots or text output for TUI changes. Ensure `just fmt`, `just fix -p <crate>`, and tests pass.

## Security & Environment
- Tests must not assume network access. Prefer mocks/feature flags.
- Avoid hardcoded absolute paths; keep scripts cross‑platform.
- Do not modify code related to `CODEX_SANDBOX_*` variables.

Questions or unsure where a change fits? Open a draft PR early for feedback.


