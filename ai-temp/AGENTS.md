# Multi-Agent Orchestrator Notes

## Feature Goal
- Allow the primary Codex CLI agent to delegate work to named sub-agents that live inside `~/.codex/agents/<agent_id>/`. Here `<agent_id>` is a human-friendly slug like `rust_test_writer` that doubles as the directory name.
- Each sub-agent should behave like an isolated Codex installation with its own `AGENTS.md`, `config.toml`, `log/`, `sessions/`, and related state directories.
- The orchestrator must load, run, and switch between agents without duplicating the existing configuration, logging, history, and persistence logic.

## Existing Implementation Survey

- `codex-rs/core/src/config.rs` owns the `Config` struct, the `find_codex_home` helper, and `Config::load_from_base_config_with_overrides`, which lets us inject a custom `codex_home` path when constructing a configuration. `Config::log_dir` and related helpers derive paths by appending to `codex_home`, so moving to a per-agent directory is automatically supported.
- `codex-rs/core/src/config_loader/mod.rs` implements layered config loading (`config.toml`, managed overrides, CLI overrides). It already accepts an arbitrary base directory, so we can reuse it for sub-agent trees by pointing it at `~/.codex/agents/<agent_id>`.
- `codex-rs/common/src/config_override.rs` parses `-c key=value` overrides. Those overrides can continue to target agent-specific settings as long as we resolve them against the sub-agent config before the run starts.
- The `multi_agent.agents = ["…"]` list in each `config.toml` now controls delegate availability. When the list is non-empty the delegate tool auto-enables; when empty it stays hidden, eliminating the need for manual `include_delegate_tool` flags.

### Project instructions (`AGENTS.md`)
- `Config::load_instructions` in `codex-rs/core/src/config.rs` reads `AGENTS.md` at the root of `codex_home`. That gives us a place to put per-agent doctrine without touching repo-level instructions.
- Repository and cwd instructions are merged by `codex-rs/core/src/project_doc.rs`, which walks the filesystem to collect `AGENTS.md` files. This logic happens after `Config` is loaded, so sub-agent instructions will cascade naturally once the agent-specific `Config` sets its own cwd and codex_home.
- The TUI status widget (`codex-rs/tui/src/status/helpers.rs`) already summarises discovered instructions. It will display sub-agent docs correctly as long as the orchestrator updates the `Config` before rendering.
### Session persistence and logging
- Each agent writes rollouts, streaming history, and logs under its own `codex_home`. See `ai-temp/persistence-design.md` for the isolation rules and orchestrator responsibilities.

### Auth and CLI entry points
- Authentication helpers in `codex-rs/core/src/auth.rs` read and write `auth.json` beneath `codex_home`. For the first iteration, all agents share the primary `~/.codex/auth.json`; isolation hooks can be added later if needed.
- CLI bootstrapping happens in `codex-rs/cli/src/main.rs`, which constructs `Config` via the shared loader and then launches the TUI or other subcommands. The orchestrator will need to hook here (or inside the TUI) to select an agent before the config load so that downstream crates operate against the correct directory tree.
- Documentation for the current configuration surface is in `docs/config.md`, ensuring any new flags or environment variables we introduce are documented alongside existing options.

## Design Principles
- Treat each sub-agent as an isolated `Config` + state bundle so existing code paths stay unchanged.
- Keep the orchestration layer thin: it should select the right `codex_home`, prepare overrides, and then call into unmodified core/TUI code wherever possible.
- Prefer additive interfaces (e.g., `AgentRegistry::resolve_path(id) -> PathBuf`) over invasive changes to core modules, respecting the repository's instruction to avoid Java-level over-abstraction.
- Make it easy to fall back to single-agent behaviour by defaulting to the legacy `~/.codex` layout when no sub-agent is selected.

## Proposed Architecture
- Directory layout:
  - `~/.codex/agents/<agent_id>/AGENTS.md` – sub-agent guidance consumed by `Config::load_instructions`. `<agent_id>` should be a meaningful, filesystem-safe identifier (e.g., `rust_test_writer`).
  - `~/.codex/agents/<agent_id>/config.toml` – optional overrides layered on top of the global config loader.
  - `~/.codex/agents/<agent_id>/log/` and `~/.codex/agents/<agent_id>/sessions/` – reused by the TUI and rollout recorder with no code changes.
  - Optional extras such as `history.jsonl`, `auth.json`, or MCP metadata can mirror the top-level structure when isolation is desired.
- Orchestration flow:
  - Extend the CLI (likely in `codex-rs/cli/src/main.rs`) to accept an `--agent <id>` flag or read the selection from a config profile. The orchestrator resolves `~/.codex/agents/<id>` (creating it if missing) before loading `Config`.
  - Introduce a lightweight helper (e.g., `codex-rs/core/src/agent_registry.rs`) that maps agent identifiers to directories, validates presence of `AGENTS.md`/`config.toml`, and exposes the resolved `codex_home`.
  - When the main agent needs to talk to a sub-agent, construct a new `Config` by calling `Config::load_from_base_config_with_overrides` with the agent's path. All downstream components (sessions, logs, instructions) receive the correct context automatically.
- Maintain a controller component in the CLI or core layer that mediates conversations: the primary agent keeps the user-facing session, delegates tasks via API calls to sub-agent Codex instances, and reconciles their responses.
- Decoupling strategy:
  - Keep orchestrator logic in a new module/crate rather than embedding it directly into `codex-rs/core/src/codex.rs`, so only the orchestration entry points depend on it.
  - Use trait-based boundaries sparingly: a simple `AgentContext` struct carrying the agent id, codex_home, and resolved `Config` may be enough, keeping future changes localised.

## Agent Invocation UX
- Default behaviour: the main agent chooses when to invoke sub-agents, treating them like native tools (similar to the plan tool or apply-patch flow).
- Explicit requests: users can opt to summon particular agents by tagging them in prompts, e.g. `#rust_test_writer`.
- Multiple tags (`#agent_one #agent_two`) allow coordinated runs when orchestration logic supports it.

## Error Handling
- Validation, logging, and recovery patterns are documented in `ai-temp/error-handling.md`. Highlights:
  - Agent discovery failures turn into actionable CLI/TUI errors via `anyhow::bail!`.
  - Detailed context is emitted through `tracing` logs while the main history records only summary entries.
  - Persistence and working-directory issues follow the same `std::io::Result` semantics used by rollout recording.

## Roadmap
1. Implement an `AgentRegistry` that enumerates `~/.codex/agents`, validates directory shape, and resolves paths.
2. Add CLI plumbing to choose an agent (flag, config entry, or interactive prompt) before constructing `Config`.
3. Ensure core services (auth, logging, history, rollout) honour the selected agent by threading the alternate `codex_home`.
4. Prototype orchestration logic that spins up a secondary Codex instance using the sub-agent context and mediates message flow.
5. Expand tests and documentation to cover multi-agent behaviour, including snapshots for the new directory layout and user guidance in `docs/`.
