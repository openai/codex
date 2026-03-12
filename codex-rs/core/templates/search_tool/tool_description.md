# Apps (Connectors) tool discovery

Searches over apps/connectors tool metadata with BM25 and exposes matching tools for the next model call.

Tools of the apps/connectors ({{app_names}}) are hidden until you search for them with this tool (`tool_search`). If the task can be fulfilled by the apps/connectors mentioned here, always prefer to do tool search before calling `list_mcp_resources` or `list_mcp_resource_templates`.