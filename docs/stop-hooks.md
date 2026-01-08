# Stop Hooks

Codex can run **Stop hooks** when the model is about to finish a turn. A Stop hook can approve the stop or block it and provide a follow‑up prompt that will be injected into the same session.

This is a **generic plugin mechanism**: any external script can participate.

## Hook Discovery

Codex searches upward from the current working directory for:

- `.codex/hooks/hooks.json` (preferred)
- `.codex/hooks.json` (fallback)

The first file found is used.

## Hook File Format

Use a Claude‑style hook configuration. Only `Stop` hooks are currently supported.

```json
{
  "hooks": {
    "Stop": [
      {
        "type": "command",
        "command": "./scripts/stop-hook.sh",
        "args": ["--verbose"],
        "timeout": 30
      }
    ]
  }
}
```

Notes:
- `timeout` is in seconds. You can also use `timeout_ms` for milliseconds.
- If no timeout is set, Codex defaults to 30 seconds.
- `type` is optional; if present it must be `command`.
- You may also use a top‑level `Stop` array instead of `hooks.Stop`.

## Config.toml (Global or Project)

You can configure Stop hooks in `config.toml` (global `~/.codex/config.toml` or project
`./.codex/config.toml`) under `[stop_hooks]`.

```toml
[stop_hooks]
include_project_hooks = true

[stop_hooks.sources.primary]
command = "/abs/path/stop-hook.sh"
timeout = 30
order = 10

[stop_hooks.sources.extra_checks]
file = "/abs/path/hooks.json"
order = 20
enabled = true
```

Notes:
- Each source is a named table under `stop_hooks.sources`.
- `command` sources run a single command hook (same fields as hooks.json).
- `file` sources load a `hooks.json` file and extract `Stop` hooks.
- `order` sorts sources (ascending) before execution; ties sort by name.
- `enabled = false` skips a source.
- `include_project_hooks = true` keeps the `.codex/hooks/hooks.json` discovery behavior.

## TUI Visibility

Stop hook activity can be surfaced in the TUI via the `[tui]` config section:

```toml
[tui]
stop_hook_visibility = "status"
```

Values:
- `off`: no UI output
- `status` (default): show a status-line update while hooks run
- `summary`: status-line updates + a final summary line
- `verbose`: status-line updates + per-hook history lines

## Hook Input (stdin JSON)

The Stop hook receives a JSON object on stdin:

```json
{
  "hook_event_name": "stop",
  "cwd": "/path/to/repo",
  "conversation_id": "...",
  "turn_id": "...",
  "rollout_path": "/path/to/rollout.jsonl",
  "input_messages": ["<user messages for this turn>"],
  "last_agent_message": "<assistant final message>"
}
```

Codex also sets environment variables for convenience:

- `CODEX_HOOK_EVENT=stop`
- `CODEX_CWD`
- `CODEX_CONVERSATION_ID`
- `CODEX_TURN_ID`
- `CODEX_ROLLOUT_PATH` (if available)

## Hook Output (stdout JSON)

Return JSON on stdout. If missing or invalid, the hook is ignored.

```json
{
  "decision": "block",
  "reason": "Repeat the original prompt here",
  "systemMessage": "Optional system message injected into context"
}
```

- `decision`: `approve` or `block`
- `reason`: required when `block` (used as the next user prompt)
- `systemMessage`: optional system message inserted before re‑prompt

If any Stop hook returns `block`, Codex injects `reason` and continues the same session.

## Security

Stop hooks run local commands with your user permissions. Only enable scripts you trust.
