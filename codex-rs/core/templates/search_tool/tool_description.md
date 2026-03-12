# Apps (Connectors) tool discovery

Searches over apps/connectors tool metadata with BM25 and exposes matching tools for the next model call.

The following connectors are installed but their tools may not be loaded upfront:
({{app_names}})
When the request needs one of these connectors and you don't already have the required tools from it, search for the connector tools with this tool (`tool_search`) to load them. For the connectors mentioned above, always prefer `tool_search`over `list_mcp_resources` or `list_mcp_resource_templates` for tool discovery.