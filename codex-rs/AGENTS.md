Codex Fork Guidance (fork branch)

This file guides Codex (the coding assistant) when working in this fork. It applies to the Rust workspace under `codex-rs/`. Our `main` mirrors `openai/codex` and should remain pristine.

Branch Model
- `main`: fast‑forward mirror of `upstream/main` (no direct commits).
- `fix-mcp-session-id-response`: long‑lived fork branch with our small changes. Do all local work here and tag releases from here.
- `pr/compat-mode`: PR staging branch (temporary). Keep fork-only files (like AGENTS.md) OFF this branch.
- Topic work: create short‑lived branches off `fix-mcp-session-id-response`; rebase onto `main` regularly, then merge back into it.

Build & Test
- Format: run `just fmt` (or `cargo fmt` fallback) after any Rust edits.
- Lints: ask before running `just fix -p <crate>` to apply clippy fixes. Prefer `-p` to scope work.
- Tests:
  - Per‑crate: run `cargo test -p <crate>` for crates you changed (e.g., `codex-mcp-server`, `codex-tui`).
  - Snapshots (tui): use `cargo test -p codex-tui` then `cargo insta` as needed.
  - Full suite (only when core/common/protocol changed): ask before running `cargo test --all-features`.

Local Build (for daily use)
- Build full workspace release binaries: `cargo build --workspace --release`.
- Run the primary CLI: `cargo run --bin codex -- <args>`.
- MCP server (with compatibility mode): `cargo run -p codex-mcp-server -- --compatibility-mode`.

Release From Our Fork
- We reuse upstream’s release workflow `.github/workflows/rust-release.yml` which triggers on tags `rust-vX.Y.Z`.
- Steps (on `fix-mcp-session-id-response`):
  1) Bump `version` in `codex-rs/Cargo.toml` (align with upstream; use your own patch/prerelease if needed, e.g., `0.31.1` or `0.31.0-alpha.1`).
  2) Commit: `Release <version>`.
  3) Tag: `git tag -a rust-v<version> -m "Release <version>" && git push origin rust-v<version>`.
- CI artifacts: the workflow validates the tag matches `Cargo.toml`, cross‑builds for Linux/macOS/Windows (x86_64 + arm64), produces `.zst` and `.tar.gz` (and `.zip` on Windows), and attaches them to the GitHub Release.

What’s Special In This Fork (MCP compatibility)
- Immediate ack for MCP tools: `codex` tool can return an immediate response with a session ID when `mcp.compatibility_mode = true`.
- Continue a session: `codex-reply` tool sends further prompts for the same session.
- Polling tool: `codex-get-response` lets clients retrieve the final/failed result for a session (default timeout 600s).
- Enable compatibility mode:
  - CLI flag: `codex-mcp-server --compatibility-mode`.
  - Or config: in CODEX_HOME `config.toml`, under `[mcp]`, set `compatibility_mode = true`.
- Implementation notes for Codex:
  - Session storage keyed by raw `Uuid` in `mcp-server/src/session_storage.rs`.
  - Tools defined in `mcp-server/src/codex_tool_config.rs`.
  - Flow in `mcp-server/src/message_processor.rs` and `src/codex_tool_runner.rs`.

TUI Conventions (quick ref)
- Prefer Stylize helpers (`.dim()`, `.cyan()`, `.bold()`, etc.).
- Use `"text".into()` and `vec![…].into()` where obvious for spans/lines.
- Wrap text with `textwrap::wrap` or helpers in `tui/src/wrapping.rs`.

Sandbox Notes
- Do not modify any logic related to `CODEX_SANDBOX` or `CODEX_SANDBOX_NETWORK_DISABLED` environment variables.
- Some tests are skipped under sandbox; this is expected.

Daily Update Flow
- Sync mirror: `git checkout main && git fetch upstream && git merge --ff-only upstream/main && git push origin main`.
- Refresh custom: `git checkout custom && git rebase main` (enable `git config rerere.enabled true`).

Contact / Context
- This branch incorporates: MCP compatibility mode, `codex-get-response`, increased default get‑response timeout (600s), and session history handling.
- If a change is broadly useful, consider upstreaming to reduce fork drift.

PR Guidance
- Purpose: add a default-off compatibility mode for clients that do not support async notifications.
- Flag/config: `--compatibility-mode` or `[mcp] compatibility_mode = true`.
- Scope in PR: immediate ack for `codex`, immediate ack for `codex-reply`, gated `codex-get-response` tool.
- Capabilities: expose `codex-get-response` only when compatibility mode is enabled.
- Avoid naming specific third-party products in PR text; describe as "compatibility mode for non-notification clients."
- Do not include fork-only files (e.g., `codex-rs/AGENTS.md`) on PR branches.