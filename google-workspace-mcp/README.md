# @openai/codex-google-workspace-mcp

This package provides an MCP server (named `codex-google-workspace-mcp`) for listing and reading Google Docs, listing and reading Google Sheets, and listing Google Drive files using the official `@modelcontextprotocol/sdk` TypeScript library.

It exposes tools for:

- Listing recent Google Docs documents in the configured account's Drive.
- Fetching the plain-text content of a specific Google Docs document.
- Listing recent Google Sheets spreadsheets in the configured account's Drive.
- Fetching values from a specific Google Sheets range (A1 notation).
- Listing recent Google Drive files (any MIME type).

## Usage

The server communicates over stdio and is suitable for use with any MCP-compatible client.

```bash
npx @openai/codex-google-workspace-mcp
```

To inspect it with the MCP inspector:

```bash
npx @modelcontextprotocol/inspector @openai/codex-google-workspace-mcp
```

## Authentication (no gcloud required)

You can authenticate either with Google Application Default Credentials (service account or user ADC) or with a built-in OAuth flow that does not require `gcloud`.

### Option 1: Application Default Credentials

Before running the server, configure credentials using one of:

- Set `GOOGLE_APPLICATION_CREDENTIALS` to point at a service account JSON key file.
- Use `gcloud auth application-default login` to configure user credentials.

Scopes / profiles

- Default profile: **full** (`documents`, `drive`, `spreadsheets`) — read/write access.
- Alternate profile: **read** (`documents.readonly`, `drive.readonly`, `spreadsheets.readonly`).
- Custom scopes: use `--scopes documents.readonly,drive.metadata.readonly,spreadsheets.readonly` (shorthand is expanded to full Google scope URLs). Cannot combine with `--profile`.
  Profiles and custom scope sets each get their own token file: `~/.codex/google-workspace-mcp-oauth-<profile>.json` or `~/.codex/google-workspace-mcp-oauth-custom-<slug>.json`.

### Option 2: Built-in OAuth flow (recommended for user accounts)

1. Create an OAuth 2.0 Client ID in Google Cloud Console of type "Desktop app".
2. Either:
   - Set these environment variables:
     - `GOOGLE_OAUTH_CLIENT_ID`
     - `GOOGLE_OAUTH_CLIENT_SECRET`
       (running `--setup-auth` will pick these up automatically and store them)
   - Or run the guided setup, which will prompt for the values and store them under `~/.codex`:

     ```bash
     npx @openai/codex-google-workspace-mcp --setup-auth           # defaults to profile=full
     npx @openai/codex-google-workspace-mcp --setup-auth --profile read
     npx @openai/codex-google-workspace-mcp --setup-auth --scopes documents.readonly,drive.metadata.readonly
     ```

   This starts a local HTTP listener, opens (or prints) a Google consent URL, and saves tokens to:
   - `~/.codex/google-workspace-mcp-oauth-<profile>.json` (profile defaults to `full`, or `...-custom-<slug>.json` when using `--scopes`)

3. After that, run the MCP server normally:

   ```bash
   npx @openai/codex-google-workspace-mcp
   ```

The server automatically uses stored OAuth tokens when available; otherwise it falls back to Application Default Credentials. If the token file for the selected profile or scopes is missing, the server will automatically walk through the setup-auth flow to create it.
