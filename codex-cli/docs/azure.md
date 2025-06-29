# Azure Provider Configuration

> This guide explains how to configure the Codex CLI to use the Azure OpenAI provider instead of the default OpenAI provider.

## Prerequisites

- An active Azure OpenAI resource.
- Your Azure OpenAI API key.
- Your Azure OpenAI resource endpoint (base URL).

## 1. Set Azure Environment Variables

You must export both the Azure API key and the base URL in your shell:

```bash
export AZURE_OPENAI_API_KEY="<your-azure-openai-key>"
export AZURE_OPENAI_BASE_URL="https://<your-resource>.openai.azure.com"
```

> **Note:** Set `AZURE_OPENAI_BASE_URL` to the root of your Azure OpenAI resource **without** the `/openai` path. The CLI will automatically append `/openai` when making API requests.

## 2. Configure Codex CLI to Use Azure

### Option A: Use the CLI flag

Invoke the CLI with the `--provider azure` flag:

```bash
codex --provider azure "<your prompt here>"
```

### Option B: Set provider in user config

Add `provider: "azure"` to your `~/.codex/config.json` or `~/.codex/config.yaml`:

**JSON (`~/.codex/config.json`):**

```jsonc
{
  "provider": "azure",
}
```

**YAML (`~/.codex/config.yaml`):**

```yaml
provider: azure
```

### Model / Deployment Name

When using the Azure provider, the `--model` (or `-m`) flag should match your Azure deployment name, not the underlying model ID. For example, if you deployed `gpt-3.5-turbo` under the deployment name `o3`, you would run:

```bash
codex --provider azure --model o3 "<your prompt here>"
```

## 3. How It Works

By default, Codex CLI runs the OpenAI auth/login flow (auth.openai.com) on startup, regardless of the provider you later choose. This step validates or fetches an OpenAI API key unless you bypass it.

Once the CLI has an OpenAI key in hand, it reads your `provider` setting and uses either the OpenAI client or the Azure client accordingly. The Azure client will pick up your `AZURE_OPENAI_API_KEY` and `AZURE_BASE_URL` from the environment.

## 4. Optional: Skip the OpenAI Login Flow

If you want to bypass the initial OpenAI login/validation step entirely for Azure, you can patch the CLI source so that the auth flow only runs when `provider === "openai"`. This requires editing `src/cli.tsx` in the codebase. You may file a feature request if you’d like this behavior upstream.

## TL;DR

- **Export both** `AZURE_OPENAI_API_KEY` **and** `AZURE_OPENAI_BASE_URL`.
- **Set** `provider` **to** `azure` (via `--provider azure` or user config).
- Codex CLI always runs the OpenAI auth flow first—only then will it switch to using your Azure settings.

## Native Responses API support

When using Azure, the CLI will call the Azure OpenAI Responses API directly (the `/responses` endpoint) to generate completions. This uses the Azure-specific Responses API as documented here: https://learn.microsoft.com/en-us/azure/ai-services/openai/how-to/responses?tabs=python-key.

You can configure the API version by setting the `AZURE_OPENAI_API_VERSION` environment variable (default: `2025-04-01-preview`).

## Rate Limit Considerations

Azure OpenAI enforces rate limits per resource and deployment. When you exceed these limits, you may encounter HTTP 429 errors. The Codex CLI automatically retries requests using exponential backoff and honors any suggested retry time provided in the error message (e.g., "please try again in X seconds"). You can customize the base retry wait time by setting the `OPENAI_RATE_LIMIT_RETRY_WAIT_MS` environment variable. If you continue to see rate limit errors, consider reducing your request rate or upgrading your Azure resource tier.
