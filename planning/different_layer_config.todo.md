Title: Add project-local overlays for config and prompts

Goal
- Support project-local overrides in `./.codex` in addition to global `~/.codex`.
- Merge project-local and global sources with clear precedence and robust tests.

Out of Scope
- Changing `CODEX_HOME` semantics or write targets (history, logs, credentials remain under `$CODEX_HOME`).
- Changing managed/admin layer precedence.
- Any changes related to CODEX_SANDBOX_* environment variables.

Design Summary
- Config precedence (highest → lowest):
  1) CLI overrides (e.g., `--config k=v`, flags)
  2) Managed preferences (macOS), then `managed_config.toml`
  3) Project-local `./.codex/config.toml`
  4) Global `$CODEX_HOME/config.toml` (default `~/.codex/config.toml`)
  5) Built-in defaults

- Prompts sources:
  - Global: `$CODEX_HOME/prompts/` (default `~/.codex/prompts/`)
  - Project-local: `./.codex/prompts/` (relative to session cwd)
  - Merge both directories; dedupe by prompt name; project-local wins on conflict. Sort final list by name.

Implementation Plan (Detailed TODO)

1) Core config loader: add project-local overlay
- Files to modify:
  - codex-rs/core/src/config_loader/mod.rs

- TODOs
  - [x] Extend private overrides struct to accept an optional project config path
        - Add field to `LoaderOverrides`:
          - `project_config_path: Option<PathBuf>` (non-`cfg` gated, available on all platforms)
        - Ensure destructuring is updated for both macOS and non-macOS code paths.
  - [x] Resolve project-local path
        - If `overrides.project_config_path` is `Some(p)`, use that.
        - Else compute default: `std::env::current_dir()?.join(".codex").join(CONFIG_TOML_FILE)`.
          - Do not canonicalize here; rely on read attempt + merge behavior.
  - [x] Read and merge
        - Read user config from `$codex_home/config.toml` (existing behavior).
        - Read project config from computed path using `read_config_from_path`.
        - Build base as: `let mut base = user_config.unwrap_or_else(default_empty_table);`
        - If project config exists, `merge_toml_values(&mut base, &project)`. This gives project-local precedence over user.
        - Keep managed layers loading as-is. Do not change `apply_managed_layers` order.
  - [x] Logging policy
        - For missing project config, use debug-level log (similar to missing managed config). Avoid noisy info logs.
  - [x] Preserve I/O error behavior
        - Syntax errors in the project TOML should log via existing parser error path and bubble up as `InvalidData`.
  - [x] Do not alter `find_codex_home()`; only change loader stacking.

- Test plan (unit tests in config_loader)
  - [x] merges_project_overrides_user
        - Temp dir for `$CODEX_HOME`; write `$codex_home/config.toml` with `foo=1`, `[nested] value="base"`.
        - Write a temp `project.toml` with `foo=2`, `[nested] value="project", extra=true`.
        - Use `LoaderOverrides { project_config_path: Some(project_path), .. }` to load.
        - Assert `foo == 2`, `nested.value == "project"`, `nested.extra == true`.
  - [x] managed_overrides_project
        - Add `managed_config.toml` with `foo=3`.
        - Assert `foo == 3` after load, proving managed stays on top of project and user.
  - [x] returns_empty_when_all_layers_missing (adjust or add new)
        - Ensure still returns empty table when user, project, managed all missing.
  - [x] missing_project_is_ok
        - Project path provided but missing → base should reflect only user config (or empty if user missing).

2) Prompts: merge global and project-local
- Files to modify:
  - codex-rs/core/src/custom_prompts.rs
  - codex-rs/core/src/codex.rs (the `Op::ListCustomPrompts` handler)

- TODOs (helpers)
  - [x] Add a helper to compute project-local prompts directory for a given cwd:
        - `fn project_prompts_dir_for(cwd: &Path) -> PathBuf { cwd.join(".codex").join("prompts") }`
  - [x] Add a merged discovery function:
        - Signature option A (directory-based, minimal coupling):
          - `pub async fn discover_prompts_merged(global: Option<&Path>, local: Option<&Path>) -> Vec<CustomPrompt>`
        - Behavior:
          - Gather `Vec<CustomPrompt>` from `global.map(discover_prompts_in)` and `local.map(discover_prompts_in)`.
          - Build map by `name` with insertion order: first global, then local (local overwrites global on conflict).
          - Convert map values to a `Vec`, sort by `name`, return.
        - Note: keep `.md` filtering and frontmatter parsing as in `discover_prompts_in`.

- TODOs (call site)
  - [x] Update `Op::ListCustomPrompts` in `codex-rs/core/src/codex.rs`:
        - Obtain `cwd` from the active `Config` (already resolved). Use it to compute local prompts dir.
        - Obtain global prompts dir via `default_prompts_dir()` (existing helper that uses `$CODEX_HOME`).
        - Call `discover_prompts_merged(global_dir_opt.as_deref(), Some(&project_prompts_dir_for(&config.cwd))))`.
        - Remove previous single-dir call.

- Test plan (unit tests in custom_prompts)
  - [x] merges_global_and_local
        - Create temp dirs simulating global and local; place `a.md` (global) and `b.md` (local).
        - Call `discover_prompts_merged(Some(global), Some(local))`.
        - Assert names `a`, `b` returned and sorted.
  - [x] local_wins_on_conflict
        - Create `review.md` in both; local content differs.
        - Call `discover_prompts_merged(Some(global), Some(local))`.
        - Assert single `review` entry using local content.
  - [x] missing_dirs_are_ok
        - Call with `None` for one or both arguments; assert empty or single-source results accordingly.

3) Docs updates
- Files to modify:
  - docs/config.md
  - docs/prompts.md

- TODOs
  - [x] docs/config.md → add project-local config overlay
        - Update the "Config" sources list to include `./.codex/config.toml`.
        - Document precedence exactly as in Design Summary.
        - Clarify that `CODEX_HOME` continues to control state locations (history, logs, credentials) and global config.
  - [x] docs/prompts.md → load prompts from both locations
        - Update "Where prompts live" to include `./.codex/prompts/`.
        - State dedupe rule: project-local wins when names collide.
        - Note that the final list is sorted by name and loaded at session start.

4) Validation & CI
- Commands to run locally (no changes to existing tooling):
  - [x] `just fmt` (workspaces; no approval needed; only necessary if Rust files changed)
  - [x] `just fix -p codex-core` (ask before running full workspace clippy; core is sufficient here)
  - [x] `cargo test -p codex-core`
  - [x] If TUI snapshots got touched indirectly (unlikely): `cargo test -p codex-tui`
  - [x] With approval, after core/common/protocol changes: `cargo test --all-features`

5) Edge cases & behaviors
- [x] Missing `./.codex/config.toml` → no-op overlay; behavior unchanged.
- [x] Malformed project config → log parse error with path; bubble as `InvalidData` (matches existing semantics for user config).
- [x] Missing `./.codex/prompts/` → no-op; use only global prompts.
- [x] Duplicate prompt names across sources → choose project-local.
- [x] Non-UTF-8 prompt file content → ignored (existing behavior preserved by `discover_prompts_in`).
- [x] Respect session `cwd` (from `Config`) when computing project-local prompts directory.
- [x] Managed/admin layers still override both user and project-local config.

6) Code Style & Conventions (to observe during implementation)
- [x] Keep crate name prefixes `codex-` intact.
- [x] Collapse ifs where applicable (clippy: collapsible_if).
- [x] Inline `format!` args where possible (clippy: uninlined_format_args).
- [x] Prefer method references over redundant closures.
- [x] Match adjacent file-local style and avoid gratuitous refactors.

7) Open Questions (confirm before coding if needed)
- [x] OK to keep managed layers highest precedence over both project and user? (Proposed: yes, unchanged.)
- [x] Any desire to allow project-local managed overlays in the future? (Out of scope now.)
- [x] Should project-local prompts be able to shadow built-in slash commands beyond name conflicts already guarded? (Currently non-goal.)

8) Rollout
- [x] Implement behind no feature flag (low-risk, additive behavior).
- [x] Land with tests and docs in same PR.
- [ ] Mention behavior in CHANGELOG under "Added".

Acceptance Criteria
- Config loaded from both global and project paths, with project overriding global on conflicts and managed layers still on top.
- Prompts discovered from both global and project paths, deduped with project taking precedence.
- Updated docs reflect new behavior.
- Unit tests cover merges, conflicts, and missing-path scenarios.

