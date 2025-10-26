## Overview
`test_codex_exec` provides a lightweight builder for invoking the `codex-exec` binary in integration tests with isolated temp directories.

## Detailed Behavior
- `TestCodexExecBuilder` owns temporary home and working directories created at construction time.
- `cmd()` returns an `assert_cmd::Command` preconfigured with `CODEX_HOME` and a dummy API key, pointing `current_dir` at the temp workspace.
- `cmd_with_server(server)` extends the command to target a mock OpenAI base URL, aligning HTTP calls with a `wiremock::MockServer`.
- Accessors expose the filesystem paths so tests can inspect artifacts after execution.
- `test_codex_exec()` constructs and returns a new builder.

## Broader Context
- Used in CLI-focused tests that need to run the compiled `codex-exec` binary end to end, often in tandem with the response mocks from `responses.rs`.

## Technical Debt
- Builder always injects a dummy API key; tests needing to simulate missing credentials must override the environment manually.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
