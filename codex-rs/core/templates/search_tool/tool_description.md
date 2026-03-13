# Apps (Connectors) tool discovery

Searches over apps/connectors tool metadata with BM25 and exposes matching tools for the next model call.

Tools of the apps ({{app_names}}) are hidden until you search for them with this tool (`tool_search`).
When the request needs one of these connectors and you don't already have the required tools from it, use this tool to load them. For the apps mentioned above, always prefer `tool_search` over `list_mcp_resources` or `list_mcp_resource_templates` for tool discovery.
