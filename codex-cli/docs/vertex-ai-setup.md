# Vertex AI Setup Guide

This guide explains how to configure Codex CLI to use Google Vertex AI models.

## Prerequisites

1. A Google Cloud Project with Vertex AI API enabled
2. Google Cloud SDK (gcloud) installed
3. Appropriate IAM permissions for Vertex AI

## Authentication Setup

Vertex AI uses Google Cloud authentication. You have several options:

### Option 1: User Credentials (Recommended for Development)

```bash
# Login with your Google account
gcloud auth login

# Set application default credentials
gcloud auth application-default login

# Set your project ID
export GOOGLE_CLOUD_PROJECT="your-project-id"
```

### Option 2: Service Account (Recommended for Production)

```bash
# Set the path to your service account key file
export GOOGLE_APPLICATION_CREDENTIALS="/path/to/service-account-key.json"

# Or set the project ID if not in the service account file
export GOOGLE_CLOUD_PROJECT="your-project-id"
```

**Security Note**: Service account key files contain sensitive credentials. Please:
- Store key files with restricted permissions (e.g., `chmod 600 key.json`)
- Never commit key files to version control
- Use a secure secret management system in production
- Consider using Workload Identity or other keyless authentication methods when possible
- Rotate keys regularly and revoke unused keys

### Option 3: Google Cloud Compute Resources

If running on Google Cloud (Compute Engine, Cloud Run, etc.), authentication happens automatically through the metadata service.

## Configuration

1. Set the provider to `vertex` in your config file (`~/.codex/config.json`):

```json
{
  "provider": "vertex",
  "model": "gemini-2.0-flash"
}
```

2. Configure the location (optional, defaults to `us-central1`):

```bash
export VERTEX_LOCATION="us-central1"
```

## Available Models

Vertex AI provides access to Google's Gemini models (as of May 2025):

### Latest Generation (Recommended)
- `gemini-2.0-flash` - More capable model for complex tasks (alias points to `gemini-2.0-flash-001`)
- `gemini-2.0-flash-lite` - Efficient model for speed/cost optimization (alias points to `gemini-2.0-flash-lite-001`)

### Previous Generation (Auto-updated aliases)
- `gemini-2.0-flash` - Auto-updated alias (currently points to `gemini-2.0-flash-002`)
- `gemini-2.0-flash-lite` - Auto-updated alias (currently points to `gemini-2.0-flash-lite-002`)

### Preview Models
- Google also offers Gemini 2.5 Pro and Flash models in preview for cutting-edge capabilities

The provider automatically maps common model names:
- `gpt-4` → `gemini-2.0-flash`
- `gpt-3.5-turbo` → `gemini-2.0-flash-lite`

## Usage

Once configured, use Codex CLI normally:

```bash
codex --provider vertex "explain this code"
```

Or with a specific model:

```bash
codex --provider vertex --model gemini-2.0-flash-lite "write a function to..."
```

## Troubleshooting

### Authentication Errors

If you see authentication errors:

1. Verify your credentials:
   ```bash
   gcloud auth application-default print-access-token
   ```

2. Check your project ID is set:
   ```bash
   echo $GOOGLE_CLOUD_PROJECT
   ```

3. Ensure Vertex AI API is enabled:
   ```bash
   gcloud services enable aiplatform.googleapis.com
   ```

### Project ID Not Found

Set the project ID explicitly:

```bash
export VERTEX_PROJECT_ID="your-project-id"
# or
export GOOGLE_CLOUD_PROJECT="your-project-id"
```

### Region/Location Issues

Vertex AI is not available in all regions. Use a supported region:

```bash
export VERTEX_LOCATION="us-central1"  # or another supported region
```

## Advanced Configuration

### Custom Model Mappings

You can override the default model mappings in your config:

```json
{
  "provider": "vertex",
  "providers": {
    "vertex": {
      "customConfig": {
        "modelMapping": {
          "my-custom-model": "gemini-2.0-flash",
          "gpt-4": "gemini-2.0-flash"  // Override default mapping
        }
      }
    }
  }
}
```

### Using with Async Code

For code that needs async initialization:

```typescript
import { createOpenAIClientAsync } from "@openai/codex";

const client = await createOpenAIClientAsync({ provider: "vertex" });
```