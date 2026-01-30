# Configuration

For basic configuration instructions, see [this documentation](https://developers.openai.com/codex/config-basic).

For advanced configuration instructions, see [this documentation](https://developers.openai.com/codex/config-advanced).

For a full configuration reference, see [this documentation](https://developers.openai.com/codex/config-reference).

## Connecting to MCP servers

Codex can connect to MCP servers configured in `~/.codex/config.toml`. See the configuration reference for the latest MCP server options:

- https://developers.openai.com/codex/config-reference

## Apps (Connectors)

Use `$` in the composer to insert a ChatGPT connector; the popover lists accessible
apps. The `/apps` command lists available and installed apps. Connected apps appear first
and are labeled as connected; others are marked as can be installed.

## Notify

Codex can run a notification hook when the agent finishes a turn. See the configuration reference for the latest notification settings:

- https://developers.openai.com/codex/config-reference

## Session sharing (enterprise)

To enable `/share` for enterprise or self-hosted storage, configure:

```
session_object_storage_url = "https://your-object-store.example.com/codex-sessions/"
```

You can also use Azure Blob Storage with a SAS URL, either as a standard HTTPS URL or the shorthand `az://` form:

```
session_object_storage_url = "https://<account>.blob.core.windows.net/<container>/codex-sessions?<sas>"
```

```
session_object_storage_url = "az://<account>/<container>/codex-sessions?<sas>"
```

For Azure, the SAS token must allow read and write access to blob objects under the prefix. Listing is not required.

If you omit the SAS token, Codex will try Azure CLI authentication (`az login`) and request storage scope tokens for the blob API.

For HTTP/HTTPS URLs, the endpoint should support `HEAD`/`PUT` for individual objects. Codex will upload the session rollout (`.jsonl`) under this prefix when sharing.

## JSON Schema

The generated JSON Schema for `config.toml` lives at `codex-rs/core/config.schema.json`.

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).
