# Multi-agent merge plan (local fork)

Goal: keep current `main` core runtime, layer in multi-agent support from PR #3655 with minimal regression risk.

Status legend: ☐ not started · ⭕ in progress · ✅ done

## Tasks

- ✅ Reset core to main for runtime files
  - Restore `codex-rs/core/src/codex.rs` from `origin/main`
  - Remove PR-only `codex-rs/core/src/codex/compact.rs` and `codex-rs/core/src/openai_tools.rs`
- ✅ Reintroduce agent protocol/events into core
  - Wire agent events through event dispatch
  - Add agent tool flag in existing tool builder (current main)
  - Pass agent registry info to tool construction
- ✅ Implement minimal agent execution path
  - Keep `core/src/agent.rs` registry loader
  - Handle agent tool calls in `codex.rs` using current Session/TurnContext APIs
  - Ensure safety/plan/turn_diff compatibility
- ✅ TUI integration
  - Ensure `/agents` and @mention handling compile with new core events
- ✅ Docs/examples
  - Verify `docs/subagents.md`, `example-agents.toml` references remain accurate
- ✅ Format & test
  - `just fmt`
  - `cargo test -p codex-core --tests --no-run`
  - `cargo test -p codex-tui` (update snapshots if needed)

## Notes

- Keep protocol agent structs already merged.
- Avoid reviving deleted legacy modules; adapt to current architecture instead.

## Follow-ups

- ✅ Doc polish: align `docs/getting-started.md`, `docs/config.md`, and `docs/subagents.md` language; keep `example-agents.toml` consistent with tool names/fields.
- ✅ Final verification: quick pass over agent registry wiring and example config after doc tweaks.
- ☐ Packaging: stage/commit, write PR summary (scope, risks, test matrix).
- ☐ Optional: full sweep `cargo test --all-features` before opening upstream PR.
