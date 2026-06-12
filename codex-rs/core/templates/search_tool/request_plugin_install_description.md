# Request plugin/connector install

Use this tool only to ask the user to install one or more known plugins/connectors from the list below. The list contains known candidates that are not currently installed.

Use this ONLY when all of the following are true:
- The user explicitly asks to use a specific plugin or connector that is not already available in the current context or active `tools` list.
- `tool_search` is not available, or it has already been called and did not find or make the requested tool callable.
- Every requested plugin or connector is one of the known installable plugins or connectors listed below. Only ask to install plugins or connectors from this list.

Do not use this tool for adjacent capabilities, broad recommendations, or tools that merely seem useful. Only use when the user explicitly asks to use exact listed plugins or connectors.

Known plugins/connectors available to install:
{{discoverable_tools}}

Workflow:

1. Check the current context and active `tools` list first. If current active tools aren't relevant and `tool_search` is available, only call this tool after `tool_search` has already been tried and found no relevant tool.
2. Match the user's explicit request against the known plugin/connector list above. Only proceed for exact listed plugins/connectors.
3. If we found both connectors and plugins to install, use plugins first, only use connectors if the corresponding plugin is installed but the connector is not.
4. Do not invent ids or copy display metadata into this tool call. Every flat or categorized entry must contain only the exact `tool_type` and `tool_id` from the known plugin/connector list above; Codex resolves picker labels and metadata from that known list.
5. If one or more plugins or connectors clearly fit, call `request_plugin_install` once with:
   - `action_type`: `install`
   - `entries`: a flat list of candidates, each with `tool_type` and `tool_id`
   - use a one-item `entries` list when one plugin or connector clearly fits
6. If multiple exact install candidates are alternatives within named categories, call `request_plugin_install` once with:
   - `action_type`: `install`
   - `categories`: a list of categories, each with `title` and `entries`
   - each categorized entry must include only `tool_type` and `tool_id`
7. After the request flow completes:
   - if the user finished the install flow, continue by searching again or using the newly available plugin or connector
   - if the user did not finish, continue without that plugin or connector, and don't request it again unless the user explicitly asks for it.

IMPORTANT: DO NOT call this tool in parallel with other tools.
