# Codex with Ollama Setup Guide

## Configuration Complete!

Your Codex is now configured to work with Ollama. Here's what has been set up:

### 1. Configuration Files Created

- **~/.config/codex/config.toml** - Main configuration file for Codex with Ollama provider settings
- **~/.codex/config.json** - User-specific settings with model and provider preferences

### 2. Environment Setup

- Added `export OLLAMA_API_KEY="dummy"` to your `~/.zshrc` file
- Ollama is configured to use the Chat Completions API at `http://localhost:11434/v1`

### 3. Available Models

You have many models available in Ollama, including:
- qwen2.5-coder:32b-128k (currently configured)
- mistral-small3.2:latest
- deepseek-coder-v2:236b
- llama3.3:70b-instruct-fp16
- And many more...

### 4. Running Codex with Ollama

Due to terminal compatibility issues with the current build, you have several options:

#### Option 1: Use the wrapper script (Recommended for testing)
```bash
./run-codex.sh "your prompt here" --provider ollama --model "qwen2.5-coder:32b-128k"
```

#### Option 2: Direct API usage in your own scripts
```javascript
import OpenAI from 'openai';

const client = new OpenAI({
  baseURL: 'http://localhost:11434/v1',
  apiKey: 'dummy',
});

// Use the client for chat completions
```

#### Option 3: Wait for terminal fix
The current issue is related to raw mode support in the terminal. This will be fixed in future versions.

### 5. MCP Servers (Tools)

The configuration file includes commented examples for various MCP servers that provide tools:
- filesystem - File system operations
- github - GitHub integration
- postgres/sqlite - Database operations
- google-maps - Maps integration
- slack - Slack messaging
- memory - Persistent memory
- browser/puppeteer - Web automation
- And more...

To enable any MCP server, uncomment the relevant section in `~/.config/codex/config.toml`.

### 6. Verified Working

- ✅ Ollama is running and accessible
- ✅ API connection tested successfully
- ✅ Models are available and responding
- ✅ Configuration files are in place
- ✅ Environment variables are set

### Next Steps

1. To use different models, update the `model` field in config files or use the `--model` flag
2. To enable MCP tools, uncomment and configure the desired servers in the config.toml file
3. For production use, consider running Codex in a proper terminal environment that supports raw mode

### Troubleshooting

If you encounter issues:
1. Ensure Ollama is running: `ollama serve`
2. Check available models: `ollama list`
3. Verify the API is accessible: `curl http://localhost:11434/api/tags`
4. Check logs for any errors