# Request plugin/connector install

Use this tool only to ask the user to install one known plugin or connector from the list below. The list contains known candidates that are not currently installed.

Use this ONLY when all of the following are true:
- The user explicitly asks to use a specific plugin or connector that is not already available in the current context or active `tools` list.
- `tool_search` is not available, or it has already been called and did not find or make the requested tool callable.
- The plugin or connector is one of the known installable plugins or connectors listed below. Only ask to install plugins or connectors from this list.

Do not use this tool for adjacent capabilities, broad recommendations, or tools that merely seem useful. Only use when the user explicitly asks to use that exact listed plugin or connector.

Known plugins/connectors available to install:
{{discoverable_tools}}

Workflow:

1. Check the current context and active `tools` list first. If current active tools aren't relevant and `tool_search` is available, only call this tool after `tool_search` has already been tried and found no relevant tool.
2. Match the user's explicit request against the known plugin/connector list above. Only proceed when one listed plugin or connector exactly fits.
3. If we found both connectors and plugins to install, use plugins first, only use connectors if the corresponding plugin is installed but the connector is not.
4. If one plugin or connector clearly fits, call `request_plugin_install` with:
   - `tool_type`: `connector` or `plugin`
   - `action_type`: `install`
   - `tool_id`: exact id from the known plugin/connector list above
   - `suggest_reason`: concise one-line user-facing reason this plugin or connector can help with the current request
5. After the request flow completes:
   - if the user finished the install flow, continue by searching again or using the newly available plugin or connector
   - if the user did not finish, continue without that plugin or connector, and don't request it again unless the user explicitly asks for it.

IMPORTANT: DO NOT call this tool in parallel with other tools.
