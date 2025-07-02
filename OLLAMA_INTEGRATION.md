# Ollama Integration for Codex

This document describes the complete Ollama integration in Codex, including function calling support and JSON repair functionality.

## Overview

Codex now fully supports Ollama as a provider with complete function calling (tools) support. The integration includes robust handling of Ollama's sometimes non-standard JSON output format.

## Key Features

### 1. Function Calling Support
- Ollama models can execute shell commands, apply patches, and use other Codex tools
- Automatic detection of Ollama based on baseURL (port 11434)
- Special handling for Ollama's JSON-in-content response format

### 2. JSON Repair System
- Dynamic JSON repair for malformed output
- Handles common issues:
  - Invalid escape sequences (`\(` → `\\(`)
  - Trailing commas
  - Missing quotes around property names
  - Single quotes instead of double quotes
  - Incomplete JSON

### 3. Multi-line JSON Parsing
- Brace-counting parser handles JSON split across multiple lines
- Consistent parsing between streaming response handler and UI layer

## Configuration

Use Ollama with Codex:
```bash
codex --provider ollama --model qwen2.5-coder:32b
```

Or with a custom base URL:
```bash
codex --provider openai --base-url http://localhost:11434/v1 --model qwen2.5-coder:32b
```

## Implementation Details

### Response Processing (`responses.ts`)
- Detects Ollama by checking if baseURL contains '11434' or 'ollama'
- Implements special streaming logic to buffer and process complete responses
- Uses brace-counting parser for multi-line JSON
- Applies JSON repair when parsing fails

### UI Filtering (`terminal-chat-response-item.tsx`)
- Filters out JSON function calls from display
- Uses same brace-counting logic as response processor
- Ensures users never see raw JSON output

### Tool Argument Parsing (`parsers.ts`)
- `parseToolCallArguments()` - repairs command arguments
- `parseToolCallOutput()` - repairs command output
- Graceful fallback on repair failure

### System Prompts (`agent-loop.ts`)
- Ollama-specific instructions for function calling format
- Guides model to output only JSON for tool use

## Error Handling

### ENOTDIR Fix
- Validates `workdir` parameter is actually a directory
- Falls back to current working directory if invalid
- Prevents spawn errors from file paths used as directories

### Malformed JSON Recovery
- Three-level fallback system:
  1. Try standard JSON.parse()
  2. Attempt repair + parse
  3. Manual name extraction for function calls

## Testing

To verify Ollama integration:

1. Start Ollama:
```bash
ollama serve
```

2. Pull a coding model:
```bash
ollama pull qwen2.5-coder:32b
```

3. Run Codex:
```bash
codex --provider ollama --model qwen2.5-coder:32b
```

4. Test commands:
- "List all files in the current directory"
- "Search for functions containing 'parse'"
- "Create a new file called test.py with a hello world function"

## Troubleshooting

### JSON Still Visible
- Ensure you're using the latest build
- Check that Ollama is detected (baseURL should contain '11434')
- Enable debug mode: `CODEX_DEBUG=1 codex ...`

### Function Calls Not Working
- Verify model supports function calling
- Check Ollama version is up to date
- Try a different model (qwen2.5-coder recommended)

### Errors During Execution
- Check debug logs for JSON repair attempts
- Verify commands don't use file paths as workdir
- Ensure proper escaping in shell commands

## Future Improvements

- Extended JSON repair for more edge cases
- Model-specific prompt optimizations
- Performance tuning for large responses