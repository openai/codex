# Non-interactive mode

For information about non-interactive mode, see [this documentation](https://developers.openai.com/codex/noninteractive).

## Exporting Rollouts for Training/RL

You can export the full session rollout as a structured JSON file using the `--output-rollout` flag. This format is designed for reinforcement learning and training workflows.

```bash
codex exec --output-rollout ./rollout.json "Your prompt here"
```

### Output Format

The exported JSON is structured for RL training with:

| Field | Description |
|-------|-------------|
| `session_id` | Unique session identifier |
| `model` | The model used for this session |
| `cwd` | Working directory |
| `prompt` | The original user prompt |
| `system_instructions` | System/agent instructions (from AGENTS.md, etc.) |
| `trajectory` | Array of interaction steps |
| `final_response` | The final assistant response |
| `success` | Whether the session completed successfully |

### Trajectory Steps

The `trajectory` array contains typed steps:

- **`user_message`**: User input with content and timestamp
- **`assistant_message`**: Agent response with content and timestamp
- **`reasoning`**: Agent's internal reasoning/thinking
- **`tool_call`**: Tool/function invocation with name, arguments, and call_id
- **`tool_output`**: Tool result with call_id and output
- **`shell_command`**: Shell command execution with command array

### Example

```json
{
  "session_id": "019b8520-1139-7c73-98ba-cf0dc7361f29",
  "model": "gpt-4o",
  "cwd": "/Users/user/project",
  "prompt": "Say hello world",
  "system_instructions": "You are a helpful assistant...",
  "trajectory": [
    {
      "type": "user_message",
      "content": "Say hello world",
      "timestamp": "2025-01-03T12:00:00.000Z"
    },
    {
      "type": "reasoning",
      "content": "The user wants me to greet them...",
      "timestamp": "2025-01-03T12:00:01.000Z"
    },
    {
      "type": "assistant_message",
      "content": "Hello world!",
      "timestamp": "2025-01-03T12:00:02.000Z"
    }
  ],
  "final_response": "Hello world!",
  "success": true,
  "source_path": "/path/to/rollout.jsonl",
  "raw_item_count": 10
}
```
