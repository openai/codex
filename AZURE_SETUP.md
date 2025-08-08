# Azure OpenAI Setup Guide for Codex

This guide explains how to configure Codex to work with Azure OpenAI services, including o3 and gpt-5 models using the Responses API.

## Prerequisites

1. An Azure OpenAI resource with appropriate model deployments
2. Your Azure OpenAI API key
3. Your Azure OpenAI resource endpoint URL

## Configuration

### For Rust Implementation (codex-rs)

Edit `~/.codex/config.toml` and add:

```toml
# Set the default model provider to Azure
model_provider = "azure"
model = "gpt-5"  # or "o3", or your deployment name

# Azure OpenAI provider configuration
[model_providers.azure]
name = "Azure OpenAI"
base_url = "https://YOUR_RESOURCE_NAME.openai.azure.com/openai/v1"
wire_api = "responses"
query_params = { api-version = "preview" }
env_http_headers = { "api-key" = "AZURE_OPENAI_API_KEY" }

# Optional: Configure retry behavior for Azure rate limits
request_max_retries = 6
stream_max_retries = 12
stream_idle_timeout_ms = 300000

# Optional: Create a profile for easy switching
[profiles.azure-o3]
model_provider = "azure"
model = "o3"  # Use your exact deployment name
approval_policy = "on-request"
```

### For TypeScript Implementation (codex-cli)

The TypeScript implementation has been updated to properly support Azure OpenAI with the Responses API.

Set environment variables:

```bash
export AZURE_OPENAI_API_KEY="your-api-key"
export AZURE_BASE_URL="https://YOUR_RESOURCE_NAME.openai.azure.com/openai/v1"
export AZURE_OPENAI_API_VERSION="preview"  # or "2025-04-01-preview"
```

Or update `~/.codex/config.json`:

```json
{
  "provider": "azure",
  "model": "gpt-5",
  "providers": {
    "azure": {
      "name": "AzureOpenAI",
      "baseURL": "https://YOUR_RESOURCE_NAME.openai.azure.com/openai/v1",
      "envKey": "AZURE_OPENAI_API_KEY"
    }
  }
}
```

## Usage

### With Rust implementation:

```bash
# Using default configuration
codex

# Using a specific profile
codex --profile azure-o3

# Override model on the fly
codex --config model="o3"

```

### With TypeScript implementation:

```bash
# Set provider to azure
codex --provider azure --model gpt-5
```

## Important Notes

1. **Model Names**: Use the exact deployment name from your Azure OpenAI resource
2. **API Version**: Use `"preview"` for the latest Responses API features
3. **Authentication**: Azure uses the `api-key` header instead of Bearer token
4. **Endpoint Format**: Must include `/openai/v1` for the Responses API
5. **Rate Limits**: Azure has different rate limiting behavior; the configuration includes retry settings

## Troubleshooting

### 401 Unauthorized Error

- Verify your API key is correctly set in the environment variable
- Ensure the API key has access to the specified deployment
- Check that the endpoint URL matches your Azure resource

### 404 Not Found Error

- Verify the model deployment name matches exactly
- Ensure the endpoint includes `/openai/v1` for Responses API
- Check that the deployment is in the same region as your resource

### Rate Limiting

- Increase retry counts in the configuration
- Consider using background mode for long-running tasks with o3
- Monitor your Azure quota and usage

## Supported Models

For the latest list of supported models and regions, refer to:

- [Azure OpenAI Models](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/concepts/models)
- [Responses API Documentation](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/how-to/responses)
