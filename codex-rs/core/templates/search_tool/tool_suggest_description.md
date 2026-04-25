# Tool suggestion discovery

Suggests a missing connector in an installed plugin, or in narrower cases a not installed but discoverable plugin, when the user clearly wants a capability that is not currently available in the active `tools` list.

Use this ONLY when:
- You've already tried to find a matching available tool for the user's request but couldn't find a good match. This includes `tool_search` (if available) and other means.
- For connectors/apps that are not installed but needed for an installed plugin, suggest to install them if the task requirements match precisely.
- For plugins that are not installed but discoverable, only suggest discoverable and installable plugins when the user's intent very explicitly and unambiguously matches that plugin itself. Do not suggest a plugin just because one of its connectors or capabilities seems relevant.
- The `suggest_reason` must be a specific one-line user-facing reason for the current request. Do not use placeholders like `placeholder`.

There are two types of allowed suggestions:
1. Suggest a plugin needed in the context that explicitly and unambiguously fits the user intent but is not installed, tool_type = "plugin", action_type = "install"
2. Suggest a connector needed in the context but not installed (even when its plugin is installed), tool_type = "connector", action_type = "install"

Tool suggestions should only use the discoverable tools listed here. DO NOT explore or recommend tools that are not on this list.

Discoverable tools:
{{discoverable_tools}}

Workflow:

1. Ensure all possible means have been exhausted to find an existing available tool but none of them matches the request intent. If tool search is available, tool search should happen before tool suggestion.
2. If no available or searchable tool is found, match the user's intent against the discoverable tools list above. Decide if any of the discoverable tools match the user intent explicitly and unambiguously. Suggest a tool only when it qualifies all the conditions.
3. If one tool clearly fits, call `tool_suggest` with:
   - `tool_type`: `connector` or `plugin`
   - `action_type`: `install` or `enable`
   - `tool_id`: exact id from the discoverable tools list above
   - `suggest_reason`: concise one-line user-facing reason this tool can help with the current request, must not be empty and must not be a placeholder
4. After the suggestion flow completes:
   - if the user finished the install or enable flow, continue by searching again or using the newly available tool
   - if the user did not finish, continue without that tool, and don't suggest that tool again unless the user explicitly asks you to.
