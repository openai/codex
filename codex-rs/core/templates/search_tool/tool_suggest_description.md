# Tool suggestion discovery

Use this tool only to ask the user to install one known plugin or connector from the list below. The list contains known candidates that are not currently installed.

Use this ONLY when all of the following are true:
- The user explicitly wants a specific plugin or connector that is not already available in the current context or active `tools` list.
- `tool_search` is not available, or it has already been called and did not find or make the requested tool callable.
- The tool is one of the known installable plugins or connectors listed below. Only ask to install tools from this list.

Do not use tool suggestion for adjacent capabilities, broad recommendations, or tools that merely seem useful. The user's intent must clearly match one listed tool.

Known plugins/connectors available to install:
{{discoverable_tools}}

Workflow:

1. Check the current context and active `tools` list first. If `tool_search` is available, call `tool_search` before calling `tool_suggest`. Do not use tool suggestion if the needed tool is already available, found through `tool_search`, or callable after discovery.
2. Match the user's explicit request against the known plugin/connector list above. Only proceed when one listed plugin or connector exactly fits.
3. If we found both connectors and plugins to suggest, use plugins first, only use connectors if the corresponding plugin is installed but the connector is not.
4. If one tool clearly fits, call `tool_suggest` with:
   - `tool_type`: `connector` or `plugin`
   - `action_type`: `install`
   - `tool_id`: exact id from the known plugin/connector list above
   - `suggest_reason`: concise one-line user-facing reason this tool can help with the current request
5. After the suggestion flow completes:
   - if the user finished the install flow, continue by searching again or using the newly available tool
   - if the user did not finish, continue without that tool, and don't suggest that tool again unless the user explicitly asks for it.

IMPORTANT: DO NOT call this tool in parallel with other tools.
