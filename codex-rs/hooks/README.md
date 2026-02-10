# Hooks

Hooks are arbitrary programs which run at various deterministic, pre-defined points in the Codex lifecycle.

Hooks implementation is in progress (as of 2025-02-10).

## TODO

- Allow hooks to return errors which halt execution of subsequent hooks.
- Add a /hooks slash command to list and debug hooks.
- Implement the following hooks:
  - SessionStart
  - SessionEnd
  - BeforeAgent
  - BeforeTool
- Add Hooks to config.toml

## Done

- Hooks for:
  - AfterAgent
  - AfterTool
