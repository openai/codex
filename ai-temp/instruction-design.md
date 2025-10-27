# Instruction Handling Strategy

## Goals
- Let each sub-agent define its own `AGENTS.md` guidance under `~/.codex/agents/<agent_id>/AGENTS.md`.
- Provide flexible working-directory scopes so instructions can apply to isolated sandboxes, shared repos, or arbitrary service folders.
- Control whether sub-agent instructions replace or extend repo-level documents.

## Working Directory Modes
- The orchestrator assigns a working directory per agent by setting `ConfigOverrides::cwd` before constructing `Config`.
- Supported modes:
  1. **Isolated sandbox** – run the agent in a dedicated staging directory (e.g., `/tmp/...`) for experimentation without touching the main workspace.
  2. **Shared workspace** – reuse the primary agent’s current working directory so collaborators operate on the same files.
  3. **Custom path** – point at a specific project directory (frontend/backend split, microservice repos, etc.).
- `AgentContext` records the chosen path so downstream code (project-doc discovery, logging) operates with consistent scope.

## Instruction Inheritance (current vs. future)
- **Today:** loading an agent replaces inherited docs; only the agent’s own `AGENTS.md` is applied.
- **Future idea:** introduce an opt-in `inherit_repo_instructions` flag so agents can append repo-level documents after their own guidance. This flag is not implemented yet; the section remains here as a backlog note.

## UI Exposure
- The TUI status helpers (`codex-rs/tui/src/status/helpers.rs`) will detect the updated `Config` and display whichever instruction set is active.
- Future enhancements may include surfacing the current inheritance mode or working directory in status overlays so users know the context the sub-agent is operating within.
