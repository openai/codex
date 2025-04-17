# Ollama Integration for Codex CLI

This document explains how to use the Ollama integration in Codex CLI, which allows you to use locally hosted open-source models with Codex.

## Overview

Codex CLI now supports using models from [Ollama](https://ollama.ai/), a tool for running open-source large language models locally. This integration allows you to:

- Use locally hosted models like Gemma, Llama, and others
- Work offline without requiring an internet connection
- Maintain privacy by keeping your data local
- Experiment with different open-source models

## Prerequisites

1. Install [Ollama](https://ollama.ai/) on your system
2. Pull the models you want to use (e.g., `ollama pull gemma3:1b`)
3. Make sure Ollama is running in the background

## Configuration

You can configure the Ollama integration using environment variables or the Codex configuration file:

### Environment Variables

- `OLLAMA_API_URL`: The URL of the Ollama API (default: `http://localhost:11434`)
- `OLLAMA_ENABLED`: Whether to enable Ollama integration (default: `true`)

### Configuration File

You can also configure Ollama in your `~/.codex/config.json` or `~/.codex/config.yaml` file:

```json
{
  "model": "o4-mini",
  "ollama": {
    "enabled": true,
    "apiUrl": "http://localhost:11434"
  }
}
```

Or in YAML:

```yaml
model: o4-mini
ollama:
  enabled: true
  apiUrl: http://localhost:11434
```

## Using Ollama Models

### Command Line

To use an Ollama model from the command line, specify the model with the `--model` flag, prefixed with `ollama:`:

```bash
codex --model ollama:gemma3:1b "Create a simple web server in Node.js"
```

### Interactive Mode

1. Start Codex CLI in interactive mode:
   ```bash
   codex
   ```

2. Press `Ctrl+M` to open the model selection overlay

3. Select an Ollama model from the list (they are marked with a 🚀 icon)

## Available Models

The available Ollama models depend on what you have pulled locally. To see a list of available models:

```bash
ollama list
```

Codex CLI will automatically detect these models and make them available in the model selection overlay.

## Recommended Models

The following Ollama models are recommended for use with Codex CLI:

- `ollama:gemma3:1b` - Google's Gemma 3 (1B parameters)
- `ollama:llama3:8b` - Meta's Llama 3 (8B parameters)

These models provide a good balance of performance and resource usage for most coding tasks.

## Limitations

- Function calling capabilities may be limited compared to OpenAI models
- Some complex coding tasks may require larger models
- Performance depends on your local hardware

## Troubleshooting

### Ollama Models Not Showing Up

If Ollama models are not showing up in the model selection overlay:

1. Make sure Ollama is running:
   ```bash
   ollama list
   ```

2. Check that the Ollama API is accessible:
   ```bash
   curl http://localhost:11434/api/tags
   ```

3. Verify that Ollama integration is enabled in your Codex configuration

### Testing the Integration

You can test the Ollama integration using the included test script:

```bash
node test-ollama.js
```

This script will check if Ollama is running, list available models, and verify that the integration is working correctly.

## Contributing

If you encounter any issues or have suggestions for improving the Ollama integration, please open an issue or submit a pull request on the [Codex GitHub repository](https://github.com/openai/codex).
