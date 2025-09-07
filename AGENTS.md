# Repository Guidelines

This monorepo hosts the Codex CLI and related Rust crates. Follow these concise rules to build, test, and contribute with minimal churn.

## Project Structure & Module Organization
- `codex-rs/` — Rust workspace (primary). Key crates: `core`, `cli` (binary `codex`), `tui`, `protocol`.
- `codex-cli/` — Docker and packaging helpers for distribution.
- `docs/` — User/contributor docs (Markdown).
- `scripts/` — Small maintenance helpers.

## Build, Test, and Development Commands
- Toolchain: Rust `1.89.0` with `clippy`, `rustfmt` (see `codex-rs/rust-toolchain.toml`).
- Optional dev shell: `nix develop`.
- Build (debug): `cd codex-rs && cargo build --workspace`.
- Build (release): `cargo build --workspace --release`.
- Run TUI quickly: `just tui` or `cargo run --bin codex -- tui`.
- Run CLI: `just codex --help` or `cargo run --bin codex -- --help`.
- Test: `cargo test --workspace`.
- Lint: `cargo clippy --workspace -- -D warnings`.
- Format: `cargo fmt -- --check` (fix with `cargo fmt`).
- Docs formatting (repo root): `pnpm run format` or `pnpm run format:fix`.

## Coding Style & Naming Conventions
- Rust: 4‑space indent; `rustfmt` enforced; imports sorted. Use `snake_case` (modules/functions), `PascalCase` (types/enums), `SCREAMING_SNAKE_CASE` (consts).
- Keep functions small and explicit; avoid `unwrap`/`expect` outside tests.
- New code must compile cleanly with `clippy -D warnings`.

## Testing Guidelines
- Unit tests inline (`mod tests`) and integration tests in each crate’s `tests/`.
- Name tests in `snake_case`; keep deterministic (no network by default).
- Add targeted tests where behavior changes (e.g., parsing, TUI state). Run `cargo test --workspace` before pushing.

## Commit & Pull Request Guidelines
- Prefer Conventional Commits (e.g., `feat(tui): …`, `fix(core): …`, `refactor:`). Keep PRs focused and small.
- Before review: run `cargo fmt`, `cargo clippy -D warnings`, `cargo test`.
- PRs must include: clear description, linked issues, and screenshots/asciinema for TUI changes.
- When a change completes a PRD task, update `PRD.md` to mark it completed and note the impact.

## Security & Configuration Tips
- Never commit secrets. CLI does not auto‑load project `.env`; pass env explicitly.
- Reviewer timeouts (where applicable) can be tuned via `CODEX_REVIEWER_TIMEOUT_SECS`.

