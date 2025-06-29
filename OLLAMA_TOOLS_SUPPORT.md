# Ollama Tools/Function Calling - FIXED!

## Current Status

✅ **Function calling is now supported!** A fix has been implemented in Codex to handle Ollama's text-based function call format.

### How It Works

Ollama v0.9.3 returns function calls as JSON text in the content field rather than in the structured `tool_calls` format that OpenAI uses. The Codex fix automatically detects and transforms these text-based function calls into the proper format.

### Example Response from Ollama:
```json
{
  "message": {
    "role": "assistant",
    "content": "{\"name\": \"get_weather\", \"arguments\": {\"location\": \"Boston, MA\"}}"
  }
}
```

### Expected OpenAI Format:
```json
{
  "message": {
    "role": "assistant",
    "content": null,
    "tool_calls": [
      {
        "id": "call_123",
        "type": "function",
        "function": {
          "name": "get_weather",
          "arguments": "{\"location\": \"Boston, MA\"}"
        }
      }
    ]
  }
}
```

## The Fix

The fix implemented in `chat_completions.rs` does the following:

1. **Detects Ollama Provider**: Checks if the provider name is "ollama"
2. **Parses JSON Content**: When content starts with `{`, tries to parse it as JSON
3. **Transforms to Tool Call**: If the JSON contains `name` and `arguments` fields, creates a proper function call
4. **Generates Call ID**: Creates a unique call ID for tracking the function call

## How to Use

With the fix in place, you can now use Ollama with full tool support:

```bash
# Shell commands work!
codex "list files in current directory" --provider ollama --model qwen2.5-coder:32b-128k

# File operations work!
codex "create a hello.py file" --provider ollama --model qwen2.5-coder:32b-128k

# MCP tools work!
# (once configured in config.toml)
```

## Supported Models

All Ollama models that understand function calling prompts will work, including:
- qwen2.5-coder:32b-128k (recommended)
- mistral-small3.2:latest
- deepseek-coder-v2:236b
- llama3.3:70b-instruct-fp16
- And many more!

## Known Limitations

1. **Terminal Mode**: There are still issues with interactive terminal mode. Use the wrapper script or direct API calls for now.
2. **Response Format**: Ollama's function calls are detected by parsing JSON in the content field, which may occasionally fail if the model doesn't format it correctly.

## Testing Tool Support

You can test if a model supports proper function calling using the included test script:
```bash
node test-ollama-tools.js
```

## Future Updates

When Ollama adds proper function calling support, Codex should work seamlessly with tools. The configuration is already in place:

```toml
[model_providers.ollama]
name = "Ollama"
base_url = "http://localhost:11434/v1"
wire_api = "chat"
```

No changes to Codex configuration will be needed once Ollama implements the standard OpenAI function calling format.