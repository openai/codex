# Env + Tilde Expansion for config.toml

## Conversation Summary

- We inspected the codebase and found `~/.codex/config.toml` is loaded at startup via the config loader stack.
  - Loader and layer stack: `codex-rs/core/src/config_loader/mod.rs` (`load_config_layers_state`).
  - Config building: `codex-rs/core/src/config/mod.rs` (e.g., `load_config_as_toml_with_cli_overrides`).
  - Entry points that load config early: `codex-rs/exec/src/lib.rs`, `codex-rs/app-server/src/lib.rs`, CLI/TUI via config builder.
- User wants _universal_ variable expansion at ingestion time (all strings + keys), not limited to `projects` paths.
- If a variable is unset, they want a _user-visible warning_ after load, similar to other config warnings (MCP-like). The warning should not hard-fail config loading.
- We decided to add a **separate, strict `~` expansion feature** (not just shell-like behavior). Rules below.

## Goals

- Expand `$VAR` and `${VAR}` _everywhere_ in config TOML string values and table keys.
- Add strict `~` expansion rule _as a separate feature_.
- If expansion fails (unset env var), leave the string/key unexpanded and **emit a warning**.
- Surface warnings in all frontends:
  - App server: `ConfigWarningNotification` at startup.
  - TUI: history warning event on startup.
  - CLI/exec: stderr warnings.

## Non-Goals

- No `%VAR%` expansion (Windows style).
- No `~user` or mid-string `foo/~` expansion.
- No change to runtime overrides format or CLI flags.

## Proposed Expansion Rules

### Env vars

- Supported syntax: `$VAR` and `${VAR}`.
- Expansion occurs in **all string values and all table keys**.
- `$$` escapes to a literal `$`.
- If env var is unset:
  - Leave the token unexpanded.
  - Record a warning including variable name and config path/key.
- If key expansion produces a duplicate key within the same table:
  - Do not silently overwrite.
  - Keep the first entry (first-wins).
  - Emit a warning describing the collision.
  - Note: “first” is based on TOML map iteration order (typically lexicographic by key), not necessarily file order.

### Tilde

- Only expand when the string **starts** with `~/` or `~\`.
- No expansion for `~user`, `foo/~`, or `bar~baz`.
- Source:
  - Unix/macOS: `$HOME`
  - Windows: `$USERPROFILE`
- If required env var is unset: leave unexpanded + warning.

## Where to Implement

### Primary integration point

- `codex-rs/core/src/config_loader/mod.rs` during config layer loading, **before merge** and **before `ConfigToml` deserialization**.
- Introduce a pass that walks `toml::Value` and rewrites:
  - string values
  - table keys
- Keep expansion _per-layer_ so warnings can be attributed to a layer and shown with source info later.

### Warning plumbing

- Extend config loader data structures to carry expansion warnings.
  - Candidate: add `warnings: Vec<ConfigWarning>` (new type) to `ConfigLayerEntry`.
  - Or add warnings at the `ConfigLayerStack` level (aggregate list with source path).
- App server already builds warnings at startup in `codex-rs/app-server/src/lib.rs` (see `config_warnings` and `ConfigWarningNotification`). Add expansion warnings here.
- TUI already emits warnings in `codex-rs/tui/src/app.rs` (see `emit_project_config_warnings`). Add expansion warnings here.
- CLI/exec: print warnings to stderr after config load (similar to other config warnings).

### Non-breaking warning model for key collisions

- `ConfigExpansionWarning` is publicly re-exported from `codex_core::config_loader`.
- Changing its public fields would be a breaking API change for downstream consumers.
- To avoid a breaking change while still surfacing collisions:
  - Keep `ConfigExpansionWarning { var, path }` as-is.
  - Use a sentinel value in `var` for collisions (e.g., `KEY_COLLISION`).
  - Encode collision details into `path` in a structured string format that the formatter understands.
  - Centralize user-facing rendering in `format_expansion_warnings(...)` so callers do not need to interpret sentinel values.

## Suggested Warning Text

- Summary: `Config variable expansion failed; some values were left unchanged.`
- Details: list like:
  - `1. $PROJECTS in [projects."$PROJECTS/foo"] is unset`
  - `2. $HOME in "~/something" is unset`
- Include file path and (if available) TOML location. If no range info, include layer source + key path.

Additional collision example:

- `3. /path/to/config.toml: projects has duplicate key after expansion: "$ROOT/a" and "${ROOT}/a" both expand to "/abs/a" (kept first)`

## Tests to Add

- `core/src/config_loader/tests.rs` or new test module in `config_loader`:
  - Expands `$VAR` in string value.
  - Expands `$VAR` in table key (e.g., `[projects."$PROJECTS/foo"]`).
  - Expands `${VAR}`.
  - Escapes `$$`.
  - Strict `~` expansion only at start (`~/x` and `~\x`), not mid-string.
  - Unset env var emits warning and leaves token unchanged.
  - Duplicate key after expansion emits a collision warning and does not overwrite silently.
- If any warnings are surfaced to app server/tui, add lightweight tests around warning aggregation (or ensure existing tests still pass).

## Feature Checklist

- Env var expansion for all TOML string values and table keys (`$VAR`, `${VAR}`).
- `$$` escape to literal `$`.
- Strict tilde expansion only at string start (`~/` or `~\\`).
- Unset env var produces warning and leaves token unchanged.
- Warning aggregation per config layer, surfaced consistently in app-server, TUI, and CLI/exec.
- Project trust key behavior:
  - Trust writes match existing `[projects]` keys by expanded+normalized path.
  - If both symbolic and absolute keys match, trust writes prefer updating the symbolic key.
  - If a matching symbolic key exists and an absolute duplicate table contains only `trust_level`, the absolute duplicate is removed.
  - Absolute duplicate entries with additional fields are preserved.

## Test Ideas (Concrete)

- Unit tests for expansion parsing:
  - `$FOO` and `${FOO}` expand in strings.
  - `$$FOO` preserves literal `$FOO`.
  - Mixed text like `path=$FOO/sub` expands.
  - Unset `$MISSING` leaves token and emits warning.
- TOML structure tests:
  - Table key expansion: `[projects."$PROJECTS/foo"]` expands to `[projects."/abs/foo"]`.
  - Nested tables and arrays with strings expand correctly.
- Tilde tests:
  - `~/x` and `~\\x` expand using HOME/USERPROFILE.
  - `~user/x`, `foo/~` do not expand and produce no warning.
  - `~/x` when HOME/USERPROFILE missing emits warning and remains `~/x`.
- Warning plumbing:
  - App server collects expansion warnings into `ConfigWarningNotification`.
  - TUI inserts warning history cell on startup when expansion warnings exist.
  - CLI/exec emits stderr warning for expansion failures.
  - Collision warnings render clearly via `format_expansion_warnings(...)`.

## Docs

- Update `docs/config.md` (and any other relevant docs) to describe:
  - `$VAR`/`${VAR}` expansion
  - `$$` escaping
  - strict `~` expansion rules
  - warning behavior for unset vars

## Implementation Steps (Detailed)

1. **Add expansion utility**

   - New helper module in `core/src/config_loader/` (e.g., `expand.rs`) with:
     - `expand_toml(value: TomlValue) -> (TomlValue, Vec<ExpansionWarning>)`
     - Walk table keys and values recursively.
     - Use a small parser to handle `$VAR`, `${VAR}`, and `$$`.
     - Apply strict `~` expansion only at string start.
     - Track warnings with a path (e.g., `projects.$PROJECTS/foo` or TOML key path) and env var name.
   - Collision handling (non-breaking):
     - During table expansion, track expanded keys.
     - On duplicate expanded key:
       - keep the first value (first-wins),
       - emit a `ConfigExpansionWarning` with a sentinel `var` (e.g., `KEY_COLLISION`),
       - encode the expanded key and both original keys into `path` so the formatter can render a human-readable message.
     - Caveat: because TOML maps are typically iterated in sorted-key order, “first-wins” is deterministic but may not match file order.

2. **Integrate into loader**

   - In `load_config_layers_state` (or immediately after loading each layer):
     - Expand the layer’s `TomlValue`.
     - Store any warnings on the `ConfigLayerEntry` (or collect into stack).

3. **Surface warnings**

   - App server: add expansion warnings into `config_warnings` list in `codex-rs/app-server/src/lib.rs`.
   - TUI: add a new `emit_*` function to append history warnings.
   - CLI/exec: print to stderr after config load.
     - Suggested insertion point: immediately after
       `Config::load_with_cli_overrides_and_harness_overrides(...)` in
       `codex-rs/exec/src/lib.rs`.
     - Use `format_expansion_warnings(...)` and print the full multi-line message to stderr.

4. **Tests**

   - Add unit tests for expansion logic and warning emission.
   - Add collision tests that verify:
     - no silent overwrites,
     - collision warning presence,
     - collision warning rendering via `format_expansion_warnings(...)`.
   - If feasible, add an exec-level stderr capture test for expansion warnings.

5. **Docs + fmt/tests**
   - Update docs.
   - Run `just fmt` in `codex-rs`.
   - Run project-specific tests (`cargo test -p codex-core` or relevant crate), then get user approval before `cargo test --all-features` if required.

## Notes/Constraints

- Follow repo rule: do not modify any code related to `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` or `CODEX_SANDBOX_ENV_VAR`.
- Use clippy style preferences (inline format args, no wildcard matches when avoidable, etc.).
- Known trade-off (documented convention):
  - Using a sentinel in `ConfigExpansionWarning.var` for collisions is API-compatible but may surprise future code that assumes `var` is always an environment variable name.
  - Mitigation: prefer `format_expansion_warnings(...)` for user-facing output rather than inspecting `warning.var` directly.
