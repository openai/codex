# Apps tool discovery

Searches over apps tool metadata with BM25 and exposes matching tools for the next model call.

MCP tools of the apps ({{app_names}}) are hidden until you search for them with this tool (`search_tool_bm25`).

Follow this workflow:

1. Call `search_tool_bm25` with:
   - `query` (required): focused terms that describe the capability you need.
   - `limit` (optional): maximum number of tools to return (default `8`).
2. Use the returned `tools` list to decide which Apps tools are relevant.
3. Matching tools are added to available `tools` and available for the remainder of the current session/thread.
4. Repeated searches in the same session/thread are additive: new matches are unioned into `tools`.

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
  - input schema property keys (`input_keys`)
- If the needed app is already explicit in the prompt (for example `[$app-name](app://{connector_id})`) or already present in the current `tools` list, you can call that tool directly.
- Do not use `search_tool_bm25` for non-apps/local tasks (filesystem, repo search, or shell-only workflows) or anything not related to {{app_names}}.
