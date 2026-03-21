# Apps (Connectors) tool discovery

Searches over apps/connectors tool metadata with BM25 and exposes matching tools for the next model call.

You have access to all the tools of the following apps/connectors:
{{app_descriptions}}
Some of the tools may not have been provided to you upfront, and you should use this tool (`tool_search_tool`) to search for the required tools and load them for the apps mentioned above. For the apps mentioned above, always use `tool_search_tool` instead of `list_mcp_resources` or `list_mcp_resource_templates` for tool discovery.
