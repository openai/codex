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

## Instruction Inheritance
- Default behaviour: the agent’s `AGENTS.md` replaces inherited docs for a clean slate.
- Optional override: agent `config.toml` may set `inherit_repo_instructions = true` (name TBD) to append repository-level documents after the agent-specific instructions.
- Implementation outline:
  - During load, the config layer reads the inheritance flag.
  - If disabled, set `Config::base_instructions` to the agent file and skip repo traversal.
  - If enabled, rely on `codex_core::project_doc::read_project_docs` so instructions are merged root-to-leaf with the agent doc prepended.

## UI Exposure
- The TUI status helpers (`codex-rs/tui/src/status/helpers.rs`) will detect the updated `Config` and display whichever instruction set is active.
- Future enhancements may include surfacing the current inheritance mode or working directory in status overlays so users know the context the sub-agent is operating within.
