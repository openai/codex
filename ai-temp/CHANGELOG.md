# Multi-Agent Orchestrator Change Log

> Keep this file current; it documents the evolution of the multi-agent design work. An outdated changelog breaks the orchestrator timeline.

## 2025-10-16
- Added batched delegate execution: the core handler now accepts `batch` payloads, launches every child run concurrently (respecting the orchestrator’s concurrency cap), and returns per-agent summaries in a single response.
- Updated the TUI delegate tree to indent siblings (two spaces per depth) and keep the status banner aligned while multiple children stream at once; adjusted snapshot/unit coverage accordingly.
- Refreshed documentation and sample instructions (`ai-temp/parallel-delegation.md`, `ai-temp/tool-implementation-patterns.md`, example Codex home agents) to describe the batched call flow and new presentation.
- Removed the inline `#agent` autocomplete experiment and reverted documentation to focus on the delegate picker and slash command flow.
- Added child delegate directories (`creative_ideas`, `conservative_ideas`) to the example Codex home and updated instructions/README to describe the fixed delegation chain (main → ideas_provider → critic).
- Auto-enabled the delegate tool whenever `[multi_agent].agents` is non-empty so sub-agents inherit delegation without toggling `include_delegate_tool`.
- Updated `AgentOrchestrator` to spawn sub-agent conversations via `ConversationManager::with_delegate`, enabling delegates to invoke their own delegates.
- Switched delegate execution tracking to a stack; the TUI now shows nested runs with indented history lines.
- Added focused unit tests covering the new config flag behaviour and UI indentation to prevent regressions.

## 2025-10-14
- Captured the baseline design artifacts (`AGENTS.md`, `config-design.md`, `instruction-design.md`, `persistence-design.md`, `error-handling.md`) compiled during the planning phase.
- Reiterated the requirement that this changelog must stay up to date as the multi-agent feature evolves.
- Scaffolded the `codex-multi-agent` crate with `AgentId`, `AgentRegistry`, and async config loading that merges global/agent/CLI overrides into an `AgentContext`.
- Wired the TUI bootstrapper to the new loader, introducing a `--agent` flag that scopes interactive runs to `~/.codex/agents/<agent_id>/`.
- Added `ai-temp/example-codex-home/` with ready-to-run config, instructions, and multiple agent directories for hands-on testing via `CODEX_HOME=...` and `--agent`.
- Authored `ai-temp/orchestration-integration.md`, outlining logic, UI/UX, and minimal-coupling hooks to let the primary agent delegate work to sub-agents in the existing codebase.
- Captured delegation decisions (single-flight execution, shared auth, primary-agent-composed prompts) inside `ai-temp/orchestration-integration.md`.
- Simplified the example Codex home to `ideas_provider` (gpt-5) and `critic` (gpt-5-nano) agents for easier manual testing.
- Delegated runs now stream live output (`DelegateEvent::Delta`) through the TUI, and remaining UX follow-ups are tracked in `ai-temp/ui-ux-delegation.md`.
- Added a dedicated status indicator while a delegate runs, restored the idle header on completion, and regression-tested streaming to prevent animation regressions.
- Updated the sample Codex home instructions/README, ensured the critic agent uses `gpt-5-nano`, and documented the new delegation UX in `ai-temp/ui-ux-delegation.md`.
- Documented plan-tool implementation patterns and how they inform future delegation tools (`ai-temp/tool-implementation-patterns.md`).
- Observed that the coordinator remains silent between delegate runs (e.g., between `#ideas_provider` completion and the `#critic` request) because tool composition happens model-side without emitting UI events; leaving this behaviour in place for now.
