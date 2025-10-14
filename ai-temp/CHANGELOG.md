# Multi-Agent Orchestrator Change Log

> Keep this file current; it documents the evolution of the multi-agent design work. An outdated changelog breaks the orchestrator timeline.

## 2025-10-14
- Captured the baseline design artifacts (`AGENTS.md`, `config-design.md`, `instruction-design.md`, `persistence-design.md`, `error-handling.md`) compiled during the planning phase.
- Reiterated the requirement that this changelog must stay up to date as the multi-agent feature evolves.
- Scaffolded the `codex-multi-agent` crate with `AgentId`, `AgentRegistry`, and async config loading that merges global/agent/CLI overrides into an `AgentContext`.
- Wired the TUI bootstrapper to the new loader, introducing a `--agent` flag that scopes interactive runs to `~/.codex/agents/<agent_id>/`.
- Added `ai-temp/example-codex-home/` with ready-to-run config, instructions, and multiple agent directories for hands-on testing via `CODEX_HOME=...` and `--agent`.
- Authored `ai-temp/orchestration-integration.md`, outlining logic, UI/UX, and minimal-coupling hooks to let the primary agent delegate work to sub-agents in the existing codebase.
- Captured delegation decisions (single-flight execution, shared auth, primary-agent-composed prompts) inside `ai-temp/orchestration-integration.md`.
- Implemented the orchestrator (`codex-rs/multi-agent/src/orchestrator.rs`) and wired the TUI to support inline delegation via `#agent_id ...` prompts, with progress and completion surfaced through the main transcript.
- Simplified the example Codex home to `ideas_provider` (gpt-5) and `critic` (gpt-5-nano) agents for easier manual testing.
- Fixed the TUI delegation hook so user-entered `#agent_id …` messages trigger the orchestrator, and added a regression test to guard the behaviour.
- Delegated runs now stream live output (`DelegateEvent::Delta`) through the TUI, and remaining UX follow-ups are tracked in `ai-temp/ui-ux-delegation.md`.
- Added a dedicated status indicator while a delegate runs, restored the idle header on completion, and regression-tested streaming to prevent animation regressions.
- Updated the sample Codex home instructions/README, ensured the critic agent uses `gpt-5-nano`, and documented the new delegation UX in `ai-temp/ui-ux-delegation.md`.
