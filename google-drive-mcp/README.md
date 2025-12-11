# @openai/codex-google-drive-mcp

This package provides an MCP server for browsing files in Google Drive using the official `@modelcontextprotocol/sdk` TypeScript library.

It exposes a tool for:

- Listing recent files in Google Drive, with optional name and MIME type filters.

## Usage

The server communicates over stdio and is suitable for use with any MCP-compatible client.

```bash
npx @openai/codex-google-drive-mcp
```

To inspect it with the MCP inspector:

```bash
npx @modelcontextprotocol/inspector @openai/codex-google-drive-mcp
```

## Authentication (no gcloud required)

You can authenticate either with Google Application Default Credentials (service account or user ADC) or with a built-in OAuth flow that does not require `gcloud`.

### Option 1: Application Default Credentials

Before running the server, configure credentials using one of:

- Set `GOOGLE_APPLICATION_CREDENTIALS` to point at a service account JSON key file.
- Use `gcloud auth application-default login` to configure user credentials.

The server requests read-only access to Google Drive.

### Option 2: Built-in OAuth flow (recommended for user accounts)

1. Create an OAuth 2.0 Client ID in Google Cloud Console of type "Desktop app".
2. Either:
   - Set these environment variables:
     - `GOOGLE_OAUTH_CLIENT_ID`
     - `GOOGLE_OAUTH_CLIENT_SECRET`
   - Or run the guided setup, which will prompt for the values and store them under `~/.codex`:

     ```bash
     npx @openai/codex-google-drive-mcp --setup-auth
     ```

3. Run the interactive login flow (either as part of `--setup-auth`, or separately with env vars set):

   ```bash
   npx @openai/codex-google-drive-mcp --authorize
   ```

   This starts a local HTTP listener, opens (or prints) a Google consent URL, and saves tokens to:

   - `~/.codex/google-drive-mcp-oauth.json`

4. After that, run the MCP server normally (without `--authorize`):

   ```bash
   npx @openai/codex-google-drive-mcp
   ```

The server automatically uses stored OAuth tokens when available; otherwise it falls back to Application Default Credentials.
