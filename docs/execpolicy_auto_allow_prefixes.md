STATUS: ready-for-implementation
## Execpolicy Auto-Allow Prefixes

### Goals
- Allow users to pre-approve exec command prefixes via config so repeated approvals are avoided.
- Support both global config and per-project overrides.
- Ensure prefix matches allow trailing argument changes (prefix match, not exact match).
- Limit scope to exec approvals only (no effect on apply_patch or non-exec tools).

### Non-Goals
- No changes to existing execpolicy file format or rule parsing.
- No auto-generation of rules for apply_patch or other tool classes.
- No wildcard or regex matching beyond execpolicy prefix semantics.

### Config Schema
- New config key (available in any config layer): `execpolicy.auto_allow_prefixes = [<string>, ...]`
- Each string is a shell-style prefix that is tokenized using shlex rules into argv.
- The resulting argv prefix is applied as an execpolicy allow-prefix rule.
- Empty/whitespace-only strings are ignored and do not produce rules.
- If tokenization fails for a string, log a warning and skip that entry (do not fail startup).

### Precedence Rules
- Config layers are merged in precedence order (lowest to highest): system config, user config, project layers (`./config.toml`, `.codex/config.toml` from repo root to cwd), session flags, then legacy managed config.
- `execpolicy.auto_allow_prefixes` is an array; higher-precedence layers replace lower-precedence values (no implicit concatenation).
- Within project layers, the closest config to the cwd wins for the key.

### Persistence Behavior
- Prefixes are applied to the in-memory execpolicy at session start.
- No execpolicy rule files are written for these prefixes (no persistence beyond the current session).

### Examples (config.toml)
```toml
[execpolicy]
auto_allow_prefixes = [
  "PGPASSWORD=example_password psql -h 127.0.0.1 -p 5432 -U example_user -d example_db -c",
  "git status",
]
```

```toml
# .codex/config.toml (project override example)
[execpolicy]
auto_allow_prefixes = [
  "PGPASSWORD=project_password psql -h 127.0.0.1 -p 5432 -U project_user -d project_db -c",
]
```

### Acceptance Criteria
- A configured prefix auto-allows exec commands whose argv starts with that prefix, even when trailing args change.
- Exec approvals are skipped for matching prefixes; apply_patch behavior is unchanged.
- Project config layers can override the global `execpolicy.auto_allow_prefixes` value.
- Prefixes are loaded from config at session start without writing to execpolicy rule files.
- Invalid prefix strings do not prevent startup; they are logged and ignored.
