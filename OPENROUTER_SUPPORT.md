# OpenRouter Support for Codex

This document explains how to use OpenRouter models with Codex. OpenRouter provides access to a wide range of AI models from different providers, including Anthropic Claude, Meta Llama, and more.

## Setup

To use OpenRouter with Codex, you need to:

1. Get an API key from [OpenRouter](https://openrouter.ai/keys)
2. Set up your environment or configuration

### Environment Setup

You can set up OpenRouter in one of these ways:

#### Option 1: Environment Variables

```bash
# Set your OpenRouter API key
export OPENROUTER_API_KEY="your-openrouter-api-key"

# Optional: Set a custom OpenRouter base URL (if needed)
export OPENROUTER_BASE_URL="https://openrouter.ai/api/v1"
```

#### Option 2: Configuration File

Add OpenRouter settings to your Codex configuration file at `~/.codex/config.json`:

```json
{
  "model": "anthropic/claude-3-opus",
  "useOpenRouter": true
}
```

Or if you prefer YAML, in `~/.codex/config.yaml`:

```yaml
model: anthropic/claude-3-opus
useOpenRouter: true
```

## Usage

### Command Line

To use OpenRouter models from the command line:

```bash
# Basic usage with OpenRouter enabled
codex --use-openrouter "Write a function to calculate Fibonacci numbers"

# Specify a specific OpenRouter model
codex --use-openrouter --model "anthropic/claude-3-opus" "Write a function to calculate Fibonacci numbers"

# Other popular models
codex --use-openrouter --model "meta-llama/llama-3-70b-instruct" "Explain quantum computing"
codex --use-openrouter --model "anthropic/claude-3-sonnet" "Create a React component"
```

### Available Models

OpenRouter provides access to many models. Here are some popular ones:

- `anthropic/claude-3-opus` - Anthropic's most powerful model
- `anthropic/claude-3-sonnet` - Balanced performance and cost
- `anthropic/claude-3-haiku` - Fast and efficient
- `meta-llama/llama-3-70b-instruct` - Meta's largest Llama 3 model
- `meta-llama/llama-3-8b-instruct` - Smaller, faster Llama 3 model
- `google/gemini-pro` - Google's Gemini Pro model

You can see the full list of available models in the model selection overlay (press `m` in the interactive mode) or by visiting the [OpenRouter models page](https://openrouter.ai/models).

## Visual Indicators

When using OpenRouter:

1. The terminal header will show "(via OpenRouter)" next to the model name
2. In the model selection overlay, OpenRouter models are marked with a 🔄 icon

## Troubleshooting

If you encounter issues:

1. **API Key Issues**: Make sure your OpenRouter API key is correctly set
2. **Model Not Found**: Verify the model name is correct (format is usually `provider/model-name`)
3. **Connection Issues**: Check your internet connection and firewall settings

## Examples

### Example 1: Basic Usage

```bash
export OPENROUTER_API_KEY="your-api-key"
codex --use-openrouter "Write a Python script that downloads images from a URL"
```

### Example 2: Specific Model

```bash
codex --use-openrouter --model "anthropic/claude-3-opus" "Create a React component for a shopping cart"
```

### Example 3: Configuration File + Command Line

With this in `~/.codex/config.json`:
```json
{
  "useOpenRouter": true
}
```

You can simply run:
```bash
codex --model "meta-llama/llama-3-70b-instruct" "Explain how to implement a binary search tree"
```

## Notes

- OpenRouter charges may apply based on your usage and the models you select
- Model availability may change over time as OpenRouter updates its offerings
- Performance may vary between different model providers
