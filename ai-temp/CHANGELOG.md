# Multi-Agent Orchestrator Change Log

> Keep this file current; it documents the evolution of the multi-agent design work. An outdated changelog breaks the orchestrator timeline.

## 2025-10-14
- Captured the baseline design artifacts (`AGENTS.md`, `config-design.md`, `instruction-design.md`, `persistence-design.md`, `error-handling.md`) compiled during the planning phase.
- Reiterated the requirement that this changelog must stay up to date as the multi-agent feature evolves.
- Scaffolded the `codex-multi-agent` crate with `AgentId`, `AgentRegistry`, and async config loading that merges global/agent/CLI overrides into an `AgentContext`.
- Wired the TUI bootstrapper to the new loader, introducing a `--agent` flag that scopes interactive runs to `~/.codex/agents/<agent_id>/`.
- Added `ai-temp/example-codex-home/` with ready-to-run config, instructions, and multiple agent directories for hands-on testing via `CODEX_HOME=...` and `--agent`.
