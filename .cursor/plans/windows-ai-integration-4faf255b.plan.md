<!-- 4faf255b-c10d-4457-8fc6-18e7e40c9992 9b8e716c-feac-48ce-a96a-1d0d1eaea37b -->
# Zero-Warning Release Prep

## Step 1 – Clear CLI Compilation Errors

- Fix the three `Option<ReasoningEffort>` mismatches in `codex-rs/cli/src/agent_create_cmd.rs`, `delegate_cmd.rs`, `parallel_delegate_cmd.rs`
- Ensure new handling keeps behaviour unchanged (prefer `ok_or_else` / `unwrap_or_default` where appropriate)

## Step 2 – Restore Windows AI Feature Gating

- Update `codex-rs/core/Cargo.toml` and `codex-rs/core/src/windows_ai_integration.rs` to require the `windows-ai` feature again
- Verify gating matches other crates and removes stray imports

## Step 3 – Add MCP CUDA Feature Definition

- Introduce a `cuda` feature in `codex-rs/mcp-server/Cargo.toml` with correct dependency wiring
- Guard CUDA-specific code in `codex-rs/mcp-server/src/codex_tools/mod.rs`

## Step 4 – Implement TUI GPU Stats Overlay

- Add GPU stats view components under `codex-rs/tui/src/` (likely `app.rs`, `ui/metrics.rs` or new module)
- Reuse Windows AI / CUDA runtime APIs, honour feature flags, add snapshot tests if output changes

## Step 5 – Lint, Format, and Smoke Tests

- Run `just fmt`
- Run `cargo clippy --workspace --all-features -- -D warnings`
- Run targeted tests: `cargo test -p codex-cli -p codex-tui`
- Summarise status / residual issues if any