# Error Handling Plan

## Existing Patterns To Reuse
- **`anyhow::Result` for CLI flows** – `codex-rs/cli/src/main.rs:242` returns `anyhow::Result<()>`, bubbling rich context to the top level.
- **Explicit `bail!` for user mistakes** – `codex-rs/cli/src/mcp_cmd.rs:234` uses `anyhow::bail!` when arguments are invalid; we can mirror that when an agent directory is missing required files.
- **Structured core errors** – `codex-rs/core/src/error.rs` defines the `CodexErr` enum backed by `thiserror`. New orchestration-specific failures should either map onto existing variants (e.g., `UnsupportedOperation`) or wrap into `CodexErr::Fatal`.
- **I/O fallbacks** – modules like `codex-rs/core/src/rollout/recorder.rs:106` return `std::io::Result`, letting callers decide whether to disable persistence. Follow the same convention for filesystem interactions inside `AgentRegistry`.
- **Runtime logging** – the TUI logs with `tracing::error!` (`codex-rs/tui/src/lib.rs:294`). Whenever we catch and suppress an error, emit a tracing event so users can inspect `codex-tui.log`.

## Planned Error Classes
1. **Agent discovery errors**: missing directory, unreadable contents, invalid slug. These trigger `anyhow::bail!` during registry enumeration so the CLI shows an actionable message.
2. **Instruction violations**: absent or empty `AGENTS.md` when required. Registry will surface a validation error; orchestrator records a warning in the main history (without launching the agent).
3. **Config parsing errors**: malformed `config.toml` under the agent. Propagate the `toml::de::Error` via `anyhow` while annotating with the agent id/path.
4. **Working directory issues**: nonexistent or non-writable target paths. Return `std::io::Error` with `ErrorKind::NotFound`/`PermissionDenied` so callers can decide whether to fall back or abort.
5. **Session/log persistence errors**: mirror rollout recorder behaviour—log a warning and continue without persistence when write failures occur.

## Surfacing Strategy
- CLI/TUI: display concise, user-facing messages for validation failures (e.g., “`#rust_test_writer` is missing AGENTS.md – fix ~/.codex/agents/rust_test_writer/AGENTS.md`”).
- Logs: use `tracing::error!` and `warn!` to include full context (agent id, path, underlying error).
- Main history: when a sub-agent fails to launch, append a summary item noting the failure class.

## Recovery Paths
- If agent setup fails, the orchestrator:
  - Logs the error.
  - Writes a single history entry describing the failure.
  - Returns control to the primary agent without launching the sub-agent.
- Follow-up invocations will re-run validation so fixes take effect immediately.

## TODO
- Implement shared helpers inside the new multi-agent crate to construct consistent error messages.
- Add tests covering invalid agent directories and ensure the CLI/TUI renders the expected guidance.
