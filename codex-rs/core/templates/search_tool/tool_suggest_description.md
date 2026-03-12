# Tool suggestion discovery

Suggests a discoverable connector or plugin when the user clearly wants a capability that is not currently available in the active `tools` list.

Use this ONLY when:
- There's no available tool to handle the user's request
- And tool_search fails to find a good match
- AND the user's request strongly matches one of the discoverable tools listed below.

Tool suggestions should only use the discoverable tools listed here. DO NOT explore or recommend tools that are not on this list.

Discoverable tools:
{{discoverable_tools}}

Workflow:

1. Match the user's request against the discoverable tools list above.
2. If one tool clearly fits, call `tool_suggest` with:
   - `tool_type`: `connector` or `plugin`
   - `action_type`: `install` or `enable`
   - `tool_id`: exact id from the discoverable tools list above
   - `suggest_reason`: concise one-line user-facing reason this tool can help with the current request
3. After the suggestion flow completes:
   - if the user finished the install or enable flow, continue by searching again or using the newly available tool
   - if the user did not finish, continue without that tool, and don't suggest that tool again unless the user explicitly asks you to.
