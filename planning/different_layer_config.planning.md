1. currently codex-cli only respect `~/.codex/config.toml`, I want it to also support `./.codex/config.toml`.
It will kinda merge these 2 config, if conflicted then  `./.codex/config.toml` is the priority.


2. currently codex-cli only respect `~/.codex/prompts/`, I want it to also support `./.codex/prompts/`.
It will kinda merge these folders, if there're conflicted files then  `./.codex/prompts/` are the priority.

---

• Here’s a concrete implementation plan to add project-local overrides for both config and prompts, without changing existing global behavior.

Overview

- Add per-project overlays on top of the user’s global settings.
- Keep precedence: CLI overrides > managed (admin) layers > project-local ./.codex > user ~/.codex > defaults.
- Merge prompts from both locations; dedupe by name with project-local winning.

Config: Local Overlay

- Loader change (core)
    - File: codex-rs/core/src/config_loader/mod.rs
    - Extend the base layer to include a project-local file at ./.codex/config.toml (relative to current working directory).
    - Implementation shape:
        - Compute project_config_path = current_dir/.codex/config.toml via std::env::current_dir() and PathBuf::join.
        - Read using existing read_config_from_path.
        - Merge onto base (global): let mut base = user_config.unwrap_or_else(default_empty_table); if let Some(project) = project_config { merge_toml_values(&mut base, &project); }
        - Keep everything else unchanged so managed layers still apply on top of the combined base.
- Testability (without global cwd races)
    - Add an optional field to the private LoaderOverrides struct: project_config_path: Option<PathBuf>.
    - Update load_config_layers_internal to prefer overrides.project_config_path when present; otherwise fall back to current_dir/.codex/config.toml.
    - Update destructuring in the cfg-sensitive sections so it compiles on all platforms.
- Precedence confirmation
    - Resulting precedence remains:
        - CLI --config overlay
        - managed preferences (macOS profiles), then managed_config.toml
        - project-local ./.codex/config.toml
        - user ~/.codex/config.toml
        - defaults
- No change to CODEX_HOME
    - CODEX_HOME continues to point to the global state directory (history, logs, credentials, global config).
    - All write-backs (e.g., MCP server mutations) should continue to use codex_home.

Prompts: Local Overlay

- Discovery change (core)
    - File: codex-rs/core/src/custom_prompts.rs
    - Add helper project_prompts_dir() -> Option<PathBuf> returning current_dir/.codex/prompts (use std::env::current_dir(); return None if resolution fails).
    - Add a new function to discover and merge prompts from both directories:
        - Proposed: discover_default_prompts() -> Vec<CustomPrompt>:
            - Gather from default_prompts_dir() (global) and project_prompts_dir() (local).
            - For each directory, use existing discover_prompts_in.
            - Dedupe by name (file stem), preferring project-local entries on conflicts.
            - Sort final list by name.
- Use the new function
    - File: codex-rs/core/src/codex.rs
    - In Op::ListCustomPrompts handler, replace:
        - Current: call default_prompts_dir() and then discover_prompts_in.
        - New: call discover_default_prompts() to return the merged list.

Docs

- Config docs
    - File: docs/config.md
    - Update “Config sources”:
        - Add ./.codex/config.toml as a project-level overlay.
        - Revise precedence section:
            1. CLI key=value overrides and command-specific flags
            2. Managed preferences (macOS), then managed_config
            3. Project-local ./.codex/config.toml
            4. Global $CODEX_HOME/config.toml (default ~/.codex/config.toml)
            5. Built-in defaults
    - Add a short note clarifying that CODEX_HOME still controls state, logs, history.
- Prompts docs
    - File: docs/prompts.md
    - Update “Where prompts live”:
        - Prompts are loaded from both $CODEX_HOME/prompts/ and ./.codex/prompts/.
        - If the same prompt name exists in both, the project-local prompt wins.
        - Final prompt list is deduplicated and sorted by name.
    - Keep existing refresh behavior note.

Tests

- Config loader tests (core)
    - File: codex-rs/core/src/config_loader/mod.rs (existing test module)
    - New tests:
        - “project overlay overrides user”:
            - Write base config.toml to a temp $CODEX_HOME with foo = 1.
            - Write project config (using LoaderOverrides.project_config_path) with foo = 2 and [nested] merges.
            - Assert merged top-level and nested keys reflect project precedence.
        - “managed still overrides project” (non‑macOS):
            - Base foo = 1, project foo = 2, managed foo = 3.
            - Assert foo = 3 after load via load_config_as_toml_with_overrides.
- Custom prompts tests (core)
    - File: codex-rs/core/src/custom_prompts.rs (existing test module)
    - New tests:
        - “merges global and local prompts”:
            - Create temp dirs simulating $CODEX_HOME/prompts and project ./.codex/prompts.
            - Place a.md in global and b.md in project; assert both present and sorted.
        - “local prompt wins on conflict”:
            - Create review.md in both; local content differs.
            - Assert merged list contains one review with local content.
- Optional end-to-end assertion (core)
    - File: codex-rs/core/src/codex.rs
    - Add/adjust a unit or integration test for Op::ListCustomPrompts to confirm merged list is returned.

Compatibility and Edge Cases

- If ./.codex/config.toml is missing, behavior remains unchanged.
- If ./.codex/prompts/ is missing, behavior remains unchanged.
- Duplicates by name across folders resolve to project-local.
- Malformed project config produces a parse error (same as global), logged with path; command exits with an error as today.
- Managed layers continue to take priority over both user and project configs.
- No changes to write behavior: writebacks still target $CODEX_HOME.

Implementation Steps

- Config
    - Add project_config_path: Option<PathBuf> to LoaderOverrides.
    - Update load_config_layers_internal to read+merge project config on top of user config.
    - Keep managed layers application as-is.
- Prompts
    - Add project_prompts_dir() and discover_default_prompts() in custom_prompts.
    - Switch Op::ListCustomPrompts to use the merged discovery function.
- Tests
    - Add tests described above.
- Docs
    - Update docs/config.md and docs/prompts.md accordingly.

Validation Plan

- Format and lint in codex-rs:
    - Run just fmt.
    - Run just fix -p codex-core (and -p codex-tui only if touched).
- Targeted tests:
    - cargo test -p codex-core.
    - If we modify code used by TUI snapshots, run cargo test -p codex-tui and review any snapshot diffs.
- Optional full suite:
    - With your confirmation, run cargo test --all-features if core/common/protocol were changed.

Want me to implement this now and run the targeted tests?