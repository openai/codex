# MCP tool discovery

When `search_tool_bm25` is available, MCP tools (`mcp__...`) are hidden until you search for them.

Follow this workflow:

1. Call `search_tool_bm25` with:
   - `query` (required): focused terms that describe the capability you need.
   - `limit` (optional): maximum number of tools to return (default `8`).
2. Use the returned `tools` list to decide which MCP tools are relevant.
3. Matching tools are added to `active_selected_tools`. Only tools in `active_selected_tools` are available for the remainder of the current turn.
4. Repeated searches in the same turn are additive: new matches are unioned into `active_selected_tools`.
5. `active_selected_tools` resets at the start of the next turn.

Notes:
- Core tools remain available without searching.
- If you are unsure, start with `limit` between 5 and 10 to see a broader set of tools.
- `query` is matched against MCP tool metadata fields:
  - `name`
  - `tool_name`
  - `server_name`
  - `title`
  - `description`
  - `connector_name`
  - `connector_id`
  - input schema property keys (`input_keys`)
- When the user asks to search/lookup/query any external system (logs, tickets, metrics, Slack, etc.), you must call `search_tool_bm25` first before running any shell command or repo search.
- Only use shell commands if (a) MCP tools for that system are not available or not sufficient, and (b) the user explicitly wants a local file/CLI search.
- If unsure which system/tool applies, ask a clarifying question after checking MCP tools.
