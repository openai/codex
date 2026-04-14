use std::collections::HashMap;

use codex_mcp::ToolInfo;

#[derive(Debug, Default)]
pub(crate) struct McpToolCatalog {
    has_servers: bool,
    tools: HashMap<String, ToolInfo>,
}

#[derive(Debug)]
pub(crate) struct McpToolCatalogUpdate {
    pub(crate) has_servers: bool,
    pub(crate) tools: HashMap<String, ToolInfo>,
}

#[derive(Debug)]
pub(crate) struct McpToolCatalogSnapshot {
    pub(crate) has_servers: bool,
    pub(crate) tools: HashMap<String, ToolInfo>,
}

impl McpToolCatalog {
    pub(crate) fn merge(&mut self, update: McpToolCatalogUpdate) -> McpToolCatalogSnapshot {
        let McpToolCatalogUpdate { has_servers, tools } = update;
        self.has_servers |= has_servers;
        for (name, tool) in tools {
            self.tools.entry(name).or_insert(tool);
        }
        self.snapshot()
    }

    pub(crate) fn snapshot(&self) -> McpToolCatalogSnapshot {
        McpToolCatalogSnapshot {
            has_servers: self.has_servers,
            tools: self.tools.clone(),
        }
    }

    pub(crate) fn resolve(&self, name: &str, namespace: Option<&str>) -> Option<ToolInfo> {
        let qualified_name = qualified_mcp_tool_name(name, namespace);
        self.tools.get(&qualified_name).cloned()
    }
}

fn qualified_mcp_tool_name(name: &str, namespace: Option<&str>) -> String {
    match namespace {
        Some(namespace) if name.starts_with(namespace) => name.to_string(),
        Some(namespace) => format!("{namespace}{name}"),
        None => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rmcp::model::Tool;
    use serde_json::Map as JsonObject;

    use super::*;

    fn tool_info(server_name: &str, callable_name: &str) -> ToolInfo {
        ToolInfo {
            server_name: server_name.to_string(),
            callable_name: callable_name.to_string(),
            callable_namespace: format!("mcp__{server_name}__"),
            server_instructions: None,
            tool: Tool {
                name: callable_name.to_string().into(),
                title: None,
                description: Some(format!("Test tool: {callable_name}").into()),
                input_schema: Arc::new(JsonObject::default()),
                output_schema: None,
                annotations: None,
                execution: None,
                icons: None,
                meta: None,
            },
            connector_id: None,
            connector_name: None,
            plugin_display_names: Vec::new(),
            connector_description: None,
        }
    }

    #[test]
    fn merge_preserves_first_seen_tool_definition() {
        let mut catalog = McpToolCatalog::default();
        catalog.merge(McpToolCatalogUpdate {
            has_servers: true,
            tools: HashMap::from([(
                "mcp__observability__get_dashboard".to_string(),
                tool_info("observability", "get_dashboard"),
            )]),
        });
        catalog.merge(McpToolCatalogUpdate {
            has_servers: true,
            tools: HashMap::from([(
                "mcp__observability__get_dashboard".to_string(),
                tool_info("other", "replacement"),
            )]),
        });

        let resolved = catalog
            .resolve("mcp__observability__get_dashboard", None)
            .expect("tool should resolve");
        assert_eq!(resolved.server_name, "observability");
        assert_eq!(resolved.tool.name.as_ref(), "get_dashboard");
    }

    #[test]
    fn resolve_matches_namespaced_tool_calls() {
        let mut catalog = McpToolCatalog::default();
        catalog.merge(McpToolCatalogUpdate {
            has_servers: true,
            tools: HashMap::from([(
                "mcp__observability__get_dashboard".to_string(),
                tool_info("observability", "get_dashboard"),
            )]),
        });

        let resolved = catalog
            .resolve("get_dashboard", Some("mcp__observability__"))
            .expect("tool should resolve");
        assert_eq!(resolved.server_name, "observability");
    }

    #[test]
    fn merge_preserves_advertised_server_presence() {
        let mut catalog = McpToolCatalog::default();
        let snapshot = catalog.merge(McpToolCatalogUpdate {
            has_servers: true,
            tools: HashMap::new(),
        });
        assert!(snapshot.has_servers);

        let snapshot = catalog.merge(McpToolCatalogUpdate {
            has_servers: false,
            tools: HashMap::new(),
        });
        assert!(snapshot.has_servers);
        assert!(snapshot.tools.is_empty());
    }
}
