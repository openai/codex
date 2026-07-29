# Local documentation MCP setup and diagnostics

Use this route only when the user explicitly asks to configure or troubleshoot the official OpenAI documentation MCP server in a supported **local Codex client**. A missing documentation tool during an ordinary documentation request is not a setup request: answer with the root skill's official-domain web fallback without installation, sandbox escalation, configuration changes, or restart.

## Verify the supported local setup

1. Search and fetch current official Codex MCP setup documentation when those tools are callable. Otherwise, search and fetch the relevant official OpenAI documentation directly.
2. Confirm the documented endpoint is `https://developers.openai.com/mcp` and verify the supported command or configuration against that current documentation before recommending it.
3. When the current documentation supports it, the local Codex CLI setup is:

   ```sh
   codex mcp add openaiDeveloperDocs --url https://developers.openai.com/mcp
   ```

   The equivalent documented configuration is:

   ```toml
   [mcp_servers.openaiDeveloperDocs]
   url = "https://developers.openai.com/mcp"
   ```

4. Check the supported local client's MCP listing or configuration, its enabled state, relevant workspace/admin policy, and any documented authentication requirement. Verify success from the actual command result, configuration, or a callable documentation-tool search/fetch; never claim installation or access without evidence.
5. Recommend a local-client restart or new local session only when current official documentation or observed client behavior requires it. Clearly identify which local client must refresh.

A skill dependency declaration, configured server, or local-client setup does not make a tool callable in an already running session. In particular, editing a hosted container's local configuration cannot install a tool into the host or model's current tool inventory. Never claim a local command installed the server into a current hosted session.

Only perform a local installation or configuration change when the user explicitly authorizes that change. Never request sandbox escalation, edit hosted configuration, install a dependency, or ask the user to restart a hosted session merely to answer an ordinary documentation question.
