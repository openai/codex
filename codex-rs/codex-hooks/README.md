# Codex Hooks

Customizable hook system for extending codex behavior at key lifecycle points.

## Overview

The hooks system provides:
- **Event-Driven**: 12 hook events covering the full session lifecycle
- **Permission Control**: Approve, deny, or modify tool inputs before execution
- **Context Injection**: Add system messages and additional context
- **Command Hooks**: Execute shell scripts with JSON input/output
- **Callback Hooks**: Native Rust callbacks for programmatic integration
- **Pattern Matching**: Wildcard, exact, pipe-separated, and regex matchers

## Architecture

```
codex-hooks/src/
├── lib.rs           # Public API exports
├── types.rs         # HookEventType, HookType, HookCallback trait
├── error.rs         # HookError enum
├── config.rs        # JSON config schema (HooksJsonConfig)
├── loader.rs        # Config file loading (project/user priority)
├── input.rs         # HookInput, HookContext, HookEventData
├── output.rs        # HookOutput, HookOutcome, PermissionDecision
├── matcher.rs       # Pattern matching logic
├── registry.rs      # HookRegistry (global + session hooks)
├── executor.rs      # HookExecutor (orchestration, aggregation)
└── executors/
    ├── command.rs   # Shell command execution
    └── callback.rs  # Native Rust callback execution
```

## Quick Start

Create `.codex/hooks.json` in your project root:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "echo '{\"continue\": true}'",
            "timeout": 30
          }
        ]
      }
    ]
  }
}
```

## Configuration

### File Locations

| Priority | Path | Description |
|----------|------|-------------|
| 1 (highest) | `.codex/hooks.json` | Project-level configuration |
| 2 | `~/.codex/hooks.json` | User-level configuration |

### JSON Schema

```json
{
  "disableAllHooks": false,
  "shellPrefix": "/optional/wrapper.sh",
  "hooks": {
    "<EventType>": [
      {
        "matcher": "<pattern>",
        "hooks": [
          {
            "type": "command",
            "command": "your-script.sh",
            "timeout": 60,
            "statusMessage": "Running hook..."
          }
        ]
      }
    ]
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `disableAllHooks` | bool | `false` | Global kill switch |
| `shellPrefix` | string | `null` | Wraps all commands (e.g., `/init.sh cmd`) |
| `hooks` | object | `{}` | Event type → matchers mapping |
| `matcher` | string | `"*"` | Pattern to match (see Pattern Matching) |
| `type` | string | required | `"command"` (future: `"prompt"`, `"agent"`) |
| `command` | string | required | Shell command to execute |
| `timeout` | int | `60` | Timeout in seconds |
| `statusMessage` | string | `null` | Optional UI message |

## Hook Event Types

| Event | Matcher Field | Description | Use Case |
|-------|---------------|-------------|----------|
| `PreToolUse` | `tool_name` | Before tool execution | Permission override, input validation |
| `PostToolUse` | `tool_name` | After successful execution | Context injection, output validation |
| `PostToolUseFailure` | `tool_name` | After tool error | Error handling, debugging |
| `SessionStart` | `source` | Session begins | Environment setup, context injection |
| `SessionEnd` | `reason` | Session ends | Cleanup, logging |
| `Stop` | (none) | User interrupts (Ctrl+C) | Prevent interruption |
| `SubagentStart` | `agent_type` | Subagent spawns | Subagent context |
| `SubagentStop` | (none) | Subagent ends | Cleanup |
| `UserPromptSubmit` | (none) | User sends message | Prompt enhancement |
| `PermissionRequest` | `tool_name` | Before permission prompt | Auto-approve/deny |
| `PreCompact` | `trigger` | Before context compaction | Compact instructions |
| `Notification` | `notification_type` | System notifications | External forwarding |

## Pattern Matching

| Pattern | Example | Matches |
|---------|---------|---------|
| Wildcard | `"*"` or `""` | All values |
| Exact | `"Bash"` | Only "Bash" |
| Pipe-separated | `"Bash\|Write\|Edit"` | Any listed value |
| Regex | `"^Bash.*"` | Regex pattern match |

## Exit Codes

| Exit Code | Behavior | Action |
|-----------|----------|--------|
| `0` | Success | Continue, parse stdout as JSON |
| `2` | **Blocking** | Stop execution, show error |
| `1`, `3+` | Non-blocking | Log warning, continue |

**Why exit 2?** Exit code 1 is too common (false positives). Exit 2 requires explicit intent.

## Environment Variables

| Variable | Description | Availability |
|----------|-------------|--------------|
| `CLAUDE_PROJECT_DIR` | Working directory | All hooks |
| `CODEX_CODE_SHELL_PREFIX` | Fallback shell prefix | All hooks |

## Hook Input (stdin)

Hooks receive JSON via stdin:

```json
{
  "hook_event_name": "PreToolUse",
  "session_id": "session-xyz789",
  "transcript_path": "/path/to/transcript.json",
  "cwd": "/project/path",
  "permission_mode": {"mode": "ask"},
  "tool_name": "Bash",
  "tool_input": {"command": "ls -la"},
  "tool_use_id": "tool-use-abc123"
}
```

## Hook Output (stdout)

Return JSON to control execution:

```json
{
  "continue": true,
  "systemMessage": "Added context message",
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow",
    "updatedInput": {"command": "ls -la --color"}
  }
}
```

### Permission Decisions

| Decision | Effect |
|----------|--------|
| `"allow"` | Approve tool execution |
| `"deny"` | Block tool execution |
| `"ask"` | Show permission prompt |

**Aggregation:** "Deny wins" - if any hook returns deny, final decision is deny.

## Examples

### Security Check (Blocking Dangerous Commands)

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "~/.codex/hooks/block-dangerous.sh",
            "timeout": 10
          }
        ]
      }
    ]
  }
}
```

`~/.codex/hooks/block-dangerous.sh`:
```bash
#!/bin/bash
INPUT=$(cat)
COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')

if echo "$COMMAND" | grep -qE 'rm -rf /|:(){:|rm -rf \*'; then
  echo "Blocked dangerous command: $COMMAND" >&2
  exit 2
fi

echo '{"continue": true}'
```

### Context Injection (SessionStart)

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "cat ~/.codex/project-context.txt | jq -Rs '{hookSpecificOutput:{hookEventName:\"SessionStart\",additionalContext:.}}'",
            "timeout": 10
          }
        ]
      }
    ]
  }
}
```

### Auto-Approve Read-Only Tools

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Read|Glob|Grep",
        "hooks": [
          {
            "type": "command",
            "command": "echo '{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"allow\"}}'",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
```

### Shell Prefix (Custom Environment)

```json
{
  "shellPrefix": "/path/to/init-env.sh",
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "my-hook.sh"
          }
        ]
      }
    ]
  }
}
```

Effective command: `/path/to/init-env.sh my-hook.sh`

## Troubleshooting

### Hook Not Running
1. Check file location: `.codex/hooks.json` (project) or `~/.codex/hooks.json` (user)
2. Verify JSON syntax: `jq . .codex/hooks.json`
3. Check `disableAllHooks` is not `true`
4. Verify matcher pattern matches the tool name

### Exit Code 2 Not Blocking
- Ensure stderr has error message: `echo "error" >&2`
- Verify explicit `exit 2` (not just script failure)

### JSON Output Not Parsed
- Output must start with `{`
- Plain text is logged but not parsed as control output

### Permission Decision Ignored
- Check `hookSpecificOutput.hookEventName` matches event
- Verify `permissionDecision` is `"allow"`, `"deny"`, or `"ask"`

## Callback Hooks (Programmatic)

For native Rust integration:

```rust
use codex_hooks::{HookCallback, HookInput, HookOutput, HookError};
use futures::future::BoxFuture;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
struct MyCallback;

impl HookCallback for MyCallback {
    fn execute(
        &self,
        input: HookInput,
        tool_use_id: Option<String>,
        cancel: CancellationToken,
        hook_index: i32,
    ) -> BoxFuture<'static, Result<HookOutput, HookError>> {
        Box::pin(async move {
            Ok(HookOutput {
                continue_execution: true,
                system_message: Some("Custom context".into()),
                ..Default::default()
            })
        })
    }

    fn dedupe_key(&self) -> Option<String> {
        None // Callbacks are never deduplicated
    }
}
```

Register via `codex_core::hooks_ext::register_callback_hook()`.

## API Reference

### Key Types

| Type | Description |
|------|-------------|
| `HooksJsonConfig` | Root JSON configuration type |
| `HookEventType` | Enum of 12 hook events |
| `HookInput` | Input passed to hooks (JSON via stdin) |
| `HookOutput` | Output from hooks (controls execution) |
| `HookExecutionResult` | Aggregated result from all matching hooks |
| `PermissionDecision` | `Allow`, `Deny`, or `Ask` |
| `HookError` | Error types (Timeout, Blocking, NonBlocking, etc.) |

### Key Functions

| Function | Description |
|----------|-------------|
| `load_hooks_config(cwd)` | Load config from JSON files |
| `build_from_json_config(config)` | Build registry from config |
| `HookExecutor::run_hooks(...)` | Execute matching hooks |

## Links

- [Integration Layer](../core/src/hooks_ext.rs) - Core integration functions
- [CLAUDE.md](CLAUDE.md) - Development notes
