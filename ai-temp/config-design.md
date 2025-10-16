# Agent Configuration Strategy

## Objectives
- Allow each sub-agent (e.g. `rust_test_writer`) to inherit the user’s normal Codex configuration while selectively overriding settings inside `~/.codex/agents/<agent_id>/config.toml`.
- Reuse the existing configuration pipeline in `codex-rs/core` so we do not fork logic for managed layers, CLI overrides, or path derivations.
- Keep integration points narrowly scoped by introducing a dedicated crate that exposes a small API for the orchestrator and CLI entry points.

## Existing Building Blocks
- `codex-rs/core/src/config.rs:965` (`Config::load_from_base_config_with_overrides`) already accepts a `ConfigToml`, `ConfigOverrides`, and an explicit `codex_home`. Passing a sub-agent directory here causes every downstream helper (`log_dir`, history, rollout recorder, etc.) to follow that directory automatically.
- `codex-rs/core/src/config_loader/mod.rs:63` layers `config.toml`, managed overrides, and managed preferences for whichever directory we point it at. It also exposes `load_config_as_toml` for reading a `ConfigToml` directly.
- `codex-rs/common/src/config_override.rs:19` parses `-c key=value` flags into a list of overrides. These are applied after all disk-based layers, so they naturally become the last stage in the merge order.

## Proposed Loading Order
1. Resolve the base Codex home (`~/.codex`) via `codex-rs/core/src/config.rs:1290` (`find_codex_home`).
2. Load the user’s global `ConfigToml` (including managed layers) from that directory.
3. If an `agent_id` is provided, resolve `~/.codex/agents/<agent_id>` and load its `config.toml`. Merge this table on top of the global config.
4. Apply CLI overrides (`CliConfigOverrides::parse_overrides`) so one-off adjustments still work per session.
5. Instantiate the final `Config` via `Config::load_from_base_config_with_overrides`, passing the resolved agent `codex_home` when present; otherwise fall back to the global Codex home. During this step we automatically enable the delegate tool when the merged `[multi_agent].agents` list is non-empty, so sub-agents inherit delegation capabilities without extra flags.

This yields inherited behaviour by default while letting each agent override keys explicitly.

## New Crate: `codex-multi-agent`
To keep the core codebase loosely coupled, introduce a new crate under `codex-rs/multi-agent` with the following responsibilities:

- `AgentRegistry`
  - Enumerates `~/.codex/agents`, validates names, and exposes metadata for each sub-agent directory.
  - Ensures required files (currently `AGENTS.md` and optional `config.toml`) exist.

- `AgentConfigLoader`
  - Public API: `load(agent_id: Option<&str>, cli_overrides: &CliConfigOverrides) -> std::io::Result<Config>`.
  - Internally performs the loading order above:
    - Calls into `codex_core::config::load_config_as_toml` for the global layer.
    - Loads `~/.codex/agents/<agent_id>/config.toml` (if present) using the same helper.
    - Merges TOML tables using `codex_core::config::merge_toml_values`.
    - Applies CLI overrides by reusing `CliConfigOverrides::apply_on_value`.
    - Constructs `Config` with the correct `codex_home`.

- `AgentContext`
  - Struct holding `agent_id`, `codex_home`, the resolved `Config`, and helper methods (e.g., path accessors) so downstream orchestration code doesn’t manipulate raw paths.

By isolating the orchestration-specific logic in this crate, other crates only need to depend on a stable interface instead of re-implementing directory handling.

## Integration Points
- **CLI (`codex-rs/cli/src/main.rs:36`)**  
  Replace the direct call to `Config::load_with_cli_overrides` with the new loader. The CLI will pass the parsed `CliConfigOverrides` and any requested agent id (via a new `--agent` flag or profile). The returned `AgentContext` supplies the `Config` used to boot the TUI or subcommands.

- **Primary Orchestrator**  
  When the main agent delegates to a sub-agent, it asks `AgentConfigLoader` for that agent’s context. Because the returned `Config` already points at `~/.codex/agents/<agent_id>`, all existing services (rollouts, logs, history) operate in the agent’s sandbox without additional wiring.

- **Future Interfaces**  
  Other modules (e.g., a session picker or app server bridge) interact with sub-agents only through the `AgentContext` API, keeping implementation details sealed inside the new crate.

## Authentication Defaults
- Initial version: all agents share the primary `auth.json` located in `~/.codex`.
- The loader always points authentication helpers (`codex_core::auth`) at the main Codex home, regardless of the agent’s data directory.
- Future extension: agent configs may opt into isolated credentials (API keys, ChatGPT logins, provider-specific secrets). For now we defer that work until a concrete use case emerges.

## Rationale
- Leveraging `Config::load_from_base_config_with_overrides` means we honour every existing feature (profiles, managed preferences, CLI overrides) without re-creating the merge logic.
- Passing a custom `codex_home` is the safest way to ensure all path-based helpers stay in sync. It avoids ad-hoc path munging and keeps the change set small.
- A dedicated crate provides a single place to evolve agent-related behaviour (validation, migrations, metadata) without scattering knowledge of `~/.codex/agents` across the repo.

## Open Points to Finalise
- Exact CLI UX for selecting an agent (flag vs. config profile vs. interactive picker).
- Whether agent directories can fall back to the global `auth.json` or require their own credentials.
- Error reporting strategy when an agent directory exists but is misconfigured.
