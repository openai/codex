# Apps tool discovery

Searches Apps MCP tool metadata with BM25 and exposes matching tools for the next model call.

When `search_tool_bm25` is available, Apps MCP tools (`mcp__codex_apps__...`) are hidden until you search for them.

Follow this workflow:

1. Call `search_tool_bm25` with:
   - `query` (required): focused terms that describe the capability you need.
   - `limit` (optional): maximum number of tools to return (default `8`).
2. Use the returned `tools` list to decide which Apps tools are relevant.
3. Matching tools are added to `active_selected_tools`. Only tools in `active_selected_tools` are available for the remainder of the current turn.
4. Repeated searches in the same turn are additive: new matches are unioned into `active_selected_tools`.
5. `active_selected_tools` resets at the start of the next turn.

Notes:
- Core tools remain available without searching.
- If you are unsure, start with `limit` between 5 and 10 to see a broader set of tools.
- `query` is matched against Apps tool metadata fields:
  - `name`
  - `tool_name`
  - `server_name`
  - `title`
  - `description`
  - `connector_name`
  - `connector_id`
  - input schema property keys (`input_keys`)
- Use `search_tool_bm25` when the user asks to work with an Apps-backed external system (for example Slack, Google Drive, Jira, Notion) and the exact tool name is not already known.
- If the needed App tool is already explicit in the prompt (for example an `apps://...` mention) or already present in the current `tools` list, you can call that tool directly.
- Do not use `search_tool_bm25` for non-Apps/local tasks (filesystem, repo search, or shell-only workflows).
