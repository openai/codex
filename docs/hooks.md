# Codex Event Hooks

Codex supports configurable event hooks that allow you to execute custom scripts when specific events occur during agent interactions. This enables you to integrate Codex with external tools, notification systems, or custom automation workflows.

## Configuration

Event hooks are configured in your `~/.codex/config.toml` file under the `[hooks]` section. Each hook type accepts an array of command strings that will be executed when the event occurs.

```toml
[hooks]
agent_turn_complete = ["notify-send 'Codex' 'Agent finished'"]
user_input_required = ["play /System/Library/Sounds/Glass.aiff"]
session_started = ["echo 'Session started' >> ~/.codex/session.log"]
session_ended = ["echo 'Session ended' >> ~/.codex/session.log"]
tool_execution_started = ["echo 'Tool started' >> ~/.codex/tools.log"]
tool_execution_completed = ["echo 'Tool completed' >> ~/.codex/tools.log"]
```

## Available Hook Events

### `agent_turn_complete`
Triggered when the agent finishes processing a user request and provides a complete response.

**JSON Payload Example:**
```json
{
  "type": "agent-turn-complete",
  "turn-id": "12345",
  "input-messages": ["Please rename the function foo to bar"],
  "last-assistant-message": "Function renamed successfully and tests pass"
}
```

### `user_input_required`
Triggered when the agent needs user approval or input (e.g., command execution approval).

**JSON Payload Example:**
```json
{
  "type": "user-input-required",
  "turn-id": "67890",
  "reason": "approval",
  "message": "Approve file deletion?"
}
```

### `session_started`
Triggered when a new Codex session begins.

**JSON Payload Example:**
```json
{
  "type": "session-started",
  "session-id": "abc123",
  "cwd": "/path/to/project"
}
```

### `session_ended`
Triggered when a Codex session ends.

**JSON Payload Example:**
```json
{
  "type": "session-ended",
  "session-id": "abc123"
}
```

### `tool_execution_started`
Triggered when the agent begins executing a tool (e.g., bash command).

**JSON Payload Example:**
```json
{
  "type": "tool-execution-started",
  "turn-id": "turn123",
  "tool-name": "bash",
  "tool-args": {
    "command": "ls -la",
    "cwd": "/path/to/project"
  }
}
```

### `tool_execution_completed`
Triggered when the agent finishes executing a tool.

**JSON Payload Example:**
```json
{
  "type": "tool-execution-completed",
  "turn-id": "turn123",
  "tool-name": "bash",
  "success": true,
  "error-message": null
}
```

## Hook Command Format

Each hook command is specified as a string that will be parsed using simple whitespace splitting. The JSON payload for the event is automatically appended as the last argument to your command.

### Examples

**Simple notification:**
```toml
agent_turn_complete = ["notify-send 'Codex' 'Task complete'"]
```

**Custom script with arguments:**
```toml
agent_turn_complete = ["/path/to/my-script.sh --codex-event"]
```

**Multiple hooks for the same event:**
```toml
agent_turn_complete = [
    "notify-send 'Codex' 'Task complete'",
    "/path/to/log-completion.sh",
    "osascript -e 'display notification \"Task complete\" with title \"Codex\"'"
]
```

## Script Examples

### Basic Notification Script

```bash
#!/bin/bash
# save as ~/.codex/hooks/notify.sh

EVENT_JSON="$1"
EVENT_TYPE=$(echo "$EVENT_JSON" | jq -r '.type')

case "$EVENT_TYPE" in
    "agent-turn-complete")
        notify-send "Codex" "Agent finished processing your request"
        ;;
    "user-input-required")
        notify-send "Codex" "Your input is required" --urgency=critical
        ;;
    "tool-execution-started")
        TOOL_NAME=$(echo "$EVENT_JSON" | jq -r '.["tool-name"]')
        notify-send "Codex" "Executing $TOOL_NAME"
        ;;
esac
```

### Session Logging Script

```bash
#!/bin/bash
# save as ~/.codex/hooks/session-log.sh

EVENT_JSON="$1"
EVENT_TYPE=$(echo "$EVENT_JSON" | jq -r '.type')
TIMESTAMP=$(date '+%Y-%m-%d %H:%M:%S')
LOG_FILE="$HOME/.codex/session.log"

echo "[$TIMESTAMP] $EVENT_TYPE: $EVENT_JSON" >> "$LOG_FILE"
```

### Integration with Task Management

```python
#!/usr/bin/env python3
# save as ~/.codex/hooks/task-tracker.py

import json
import sys
import requests
from datetime import datetime

def main():
    if len(sys.argv) < 2:
        return
    
    event_data = json.loads(sys.argv[1])
    event_type = event_data.get('type')
    
    if event_type == 'agent-turn-complete':
        # Update task management system
        task_data = {
            'title': f"Codex task completed",
            'description': event_data.get('last-assistant-message', ''),
            'completed_at': datetime.now().isoformat(),
            'turn_id': event_data.get('turn-id')
        }
        
        # Post to your task management API
        # requests.post('https://api.your-task-system.com/tasks', json=task_data)
        print(f"Task completed: {event_data.get('turn-id')}")

if __name__ == '__main__':
    main()
```

## Legacy Compatibility

The existing `notify` configuration is still supported for backward compatibility:

```toml
# This will receive all events (same as before)
notify = ["notify-send", "Codex"]

# New event-specific hooks can be used alongside legacy notify
[hooks]
agent_turn_complete = ["/path/to/completion-specific-hook.sh"]
```

## Error Handling

- Hook commands are executed as fire-and-forget processes
- Hook failures do not interrupt the main Codex workflow
- Failed hook executions are logged but do not affect agent functionality
- Invalid JSON serialization errors are logged

## Security Considerations

- Hook commands are executed with the same permissions as the Codex process
- Be careful when using hooks with untrusted input
- Consider using absolute paths for hook scripts
- Validate and sanitize any data extracted from the JSON payload in your scripts