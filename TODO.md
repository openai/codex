# TODO

- [x] Extend `codex-rs/core/src/custom_prompts.rs` to merge prompts from both `$CODEX_HOME/prompts` and `<repo-root>/.codex/commands`, locating the repo root once per startup.
- [x] Only include immediate `.md` files under `.codex/commands`; derive the slash name from the filename stem (e.g. `test.md` → `/test`).
- [x] Track prompt names to detect duplicates; drop later duplicates and emit warnings via CLI stdout and a startup event so the TUI can surface them.
- [x] Ensure project prompts flow through `ListCustomPrompts` so the slash popup lists them automatically.
- [x] During command submission, support positional placeholders: `/test arg1 arg2` replaces `${1}`, `${2}`, etc., leaving unmatched placeholders untouched.
- [x] Document `.codex/commands` usage and `${n}` substitution in `docs/prompts.md`; add unit tests (and TUI snapshot if needed) covering discovery, duplicate warnings, and substitution.

## Context
- `.codex/commands` lives at the project root (closest git root to `Config::cwd`). We only read direct children; nested folders are ignored even if present.
- Slash names must be unique across project + `$CODEX_HOME` prompts. On duplicates, prefer the first source discovered and emit warnings to both CLI stdout and TUI (via an event) so users know why the project copy was skipped.
- Variable substitution is positional: `/cmd foo bar` exposes `${1}` = "foo", `${2}` = "bar", etc. Unused placeholders remain literal. No templating beyond simple string replacement.
- Discovery runs only at startup, no file watching needed. Cache results alongside mod-times if helpful but not required yet.
- Remember to keep built-in commands filtered out and reuse existing tests/helpers whenever possible.
- When updating docs, clarify that project prompts complement (not replace) `$CODEX_HOME/prompts` and remind users to restart Codex to pick up new files.

Instructions: mark each checkbox as soon as its item is finished, not just at the end of the run.
