# Model Providers in Codex CLI

This document describes the supported model providers in Codex CLI and how they're integrated into the system.

## Supported Providers

Codex CLI supports the following model providers:

1. **OpenAI** - Default provider with models like GPT-4o, GPT-4.1, etc.
2. **Anthropic/Claude** - Claude family of models including Claude 3 Opus, Sonnet, and Haiku
3. **Google/Gemini** - Gemini 1.5 Pro, Gemini 1.5 Flash, and other Gemini models

## Model Configuration

Each model provider has specific configuration requirements and capabilities. The system automatically selects the appropriate provider based on the model name, or you can explicitly specify a provider.

### Model Information Structure

All models share a common information structure:

```typescript
type ModelInfo = {
  /** The human-readable label for this model */
  label: string;
  /** The max context window size for this model */
  maxContextLength: number;
};
```

## Provider-Specific Prompt Adaptations

Codex CLI adapts the system prompts for each provider to optimize for their specific capabilities and requirements:

### OpenAI Models

OpenAI models like GPT-4o and GPT-4.1 work well with the standard prompt format but include additional reminders about:

- Waiting for confirmation after each tool use
- Following the exact XML format for tool usage
- Thinking step-by-step before using tools

### Claude/Anthropic Models

Claude models receive specific adaptations:

- More explicit formatting of the TOOL USE section
- Additional emphasis on waiting for user confirmation
- Clear boundaries between sections with explicit markers

### Gemini Models

Gemini models receive adaptations focused on:

- Simplified, step-by-step instructions for tool usage
- Explicit numbered guidelines
- Additional clarification on the order of operations

## Tool Support

All providers support the full set of Codex CLI tools, including:

- Standard tools: `execute_command`, `read_file`, `write_to_file`, `replace_in_file`, `search_files`, `list_files`
- Enhanced tools: `list_code_definition_names`, `browser_action`
- Special purpose tools: `ask_followup_question`, `attempt_completion`
- MCP integration: `use_mcp_tool`, `access_mcp_resource`

## Provider Detection

Codex CLI automatically detects the appropriate provider based on the model name:

- Models containing "claude" are assigned to the Anthropic provider
- Models containing "gemini" are assigned to the Gemini provider
- All other models default to the OpenAI provider

You can also explicitly specify a provider in the configuration.

## Example Usage

```bash
# Using an OpenAI model
codex --model gpt-4o

# Using a Claude model
codex --model claude-3-opus

# Using a Gemini model
codex --model gemini-1.5-pro
```

## Model Selection GUI

When using the Codex CLI, you can also select models from a dropdown menu that organizes models by provider.

## Custom Model Configuration

If you need to use a model that's not pre-configured, you can add it to your configuration:

```json
{
  "models": {
    "custom-model": {
      "label": "My Custom Model",
      "maxContextLength": 32000,
      "provider": "openai" // or "anthropic" or "gemini"
    }
  }
}
```

## Testing Model Integration

The integration of each model provider is tested to ensure:

1. Proper provider detection based on model name
2. Appropriate prompt adaptations for each provider
3. Correct handling of model capabilities and limitations
4. Tool functionality across all providers
