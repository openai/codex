# Ollama Tools/Function Calling Limitation

## Current Status

As of Ollama v0.9.3, there is limited support for OpenAI-style function calling. When tools are provided in the request, Ollama models return the function call as plain text in the content field rather than in the structured `tool_calls` format that OpenAI uses.

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

## Impact on Codex

This means that when using Ollama with Codex:
- The built-in `shell` tool will not work properly
- MCP server tools will not function
- The model cannot execute commands or interact with external tools

## Workarounds

### 1. Use Ollama for Chat Only
Use Ollama models for conversation and code generation, but not for tool-based interactions:
```bash
codex "explain this code" --provider ollama --model qwen2.5-coder:32b-128k
```

### 2. Use OpenAI-Compatible Providers for Tools
For tasks requiring tools (file operations, shell commands, etc.), use providers that fully support function calling:
- OpenAI
- Azure OpenAI
- OpenRouter (with compatible models)
- Groq
- Together AI

### 3. Wait for Ollama Updates
The Ollama team is actively working on improving OpenAI compatibility. Check for updates:
```bash
ollama --version
```

### 4. Use Alternative Local Solutions
Consider using other local solutions that support function calling:
- LM Studio with function-calling capable models
- LocalAI with proper function calling support
- Text-generation-webui with OpenAI-compatible API

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