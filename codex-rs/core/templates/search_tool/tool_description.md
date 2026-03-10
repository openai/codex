# Apps tool discovery

Searches over apps metadata with BM25 and exposes matching enabled tools for the next model call, or discoverable apps when requested.

MCP tools of the apps ({{app_names}}) are hidden until you search for them with this tool (`search_tool_bm25`).
Known discoverable tools (connectors, plugins) via this tool: {{discoverable_connector_names}}.

Follow this workflow:

1. Call `search_tool_bm25` with:
   - `query` (required): focused terms that describe the capability you need.
   - `limit` (optional): maximum number of tools to return (default `8`).
   - `mode` (optional): `enabled` or `discoverable`. Default is `enabled`.
2. Always search `mode: "enabled"` first.
3. If `enabled` finds the right tool, use the returned `tools` list to decide which Apps tools are relevant.
4. Matching enabled tools are added to available `tools` for the remainder of the current session/thread.
5. If `enabled` does not find the right tool and the user strongly wants a specific app, search again with `mode: "discoverable"`.
6. `discoverable` results prefer:
   - connectors that are not installed
   - connectors that are installed but disabled
7. If `discoverable` finds the right app, call `tool_suggest` with the returned `connector_id`, `tool_type`, and `suggestion_type`, include a concise one-liner, user-facing `suggest_reason` for the `tool_suggest` tool.

Notes:
- Core tools remain available without searching.
- If you are unsure, start with `limit` between 5 and 10 to see a broader set of tools.
- In `enabled` mode, `query` is matched against Apps tool metadata fields:
  - `name`
  - `tool_name`
  - `server_name`
  - `title`
  - `description`
  - `connector_name`
  - input schema property keys (`input_keys`)
- In `discoverable` mode, `query` is matched against various kinds of tool metadata such as connector id, name, description, labels, categories, and plugin display names.
- `discoverable` can also surface connectors that are already installed but currently disabled, even if they are not listed above.
- If the needed app is already explicit in the prompt (for example `[$app-name](app://{connector_id})`) or already present in the current `tools` list, you can call that tool directly.
- Do not call app MCP tools for apps returned only by `discoverable` mode until the user installs or enables them.
- Do not use `search_tool_bm25` for non-apps/local tasks (filesystem, repo search, or shell-only workflows) or anything not related to the specific tool names mentioned above.
