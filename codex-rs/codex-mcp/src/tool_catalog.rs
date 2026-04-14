use std::collections::HashMap;

use codex_protocol::mcp::AdvertisedMcpToolCatalog;
use codex_protocol::mcp::AdvertisedMcpToolInfo;
use tracing::warn;

use crate::mcp_connection_manager::ToolInfo;

#[derive(Debug, Default)]
pub struct McpToolCatalog {
    has_servers: bool,
    tools: HashMap<String, ToolInfo>,
}

#[derive(Debug)]
pub struct McpToolCatalogUpdate {
    pub has_servers: bool,
    pub tools: HashMap<String, ToolInfo>,
}

#[derive(Debug)]
pub struct McpToolCatalogSnapshot {
    pub has_servers: bool,
    pub tools: HashMap<String, ToolInfo>,
}

#[derive(Debug)]
pub struct McpToolCatalogMerge {
    pub snapshot: McpToolCatalogSnapshot,
    pub changed: bool,
}

impl McpToolCatalog {
    pub fn merge(&mut self, update: McpToolCatalogUpdate) -> McpToolCatalogMerge {
        let McpToolCatalogUpdate { has_servers, tools } = update;
        let mut changed = false;
        if has_servers && !self.has_servers {
            self.has_servers = true;
            changed = true;
        }
        for (name, tool) in tools {
            if let std::collections::hash_map::Entry::Vacant(entry) = self.tools.entry(name) {
                entry.insert(tool);
                changed = true;
            }
        }
        McpToolCatalogMerge {
            snapshot: self.snapshot(),
            changed,
        }
    }

    pub fn snapshot(&self) -> McpToolCatalogSnapshot {
        McpToolCatalogSnapshot {
            has_servers: self.has_servers,
            tools: self.tools.clone(),
        }
    }

    pub fn resolve(&self, name: &str, namespace: Option<&str>) -> Option<ToolInfo> {
        let qualified_name = qualified_mcp_tool_name(name, namespace);
        self.tools.get(&qualified_name).cloned()
    }
}

impl McpToolCatalogUpdate {
    pub fn from_advertised(catalog: AdvertisedMcpToolCatalog) -> Self {
        let tools = catalog
            .tools
            .into_iter()
            .filter_map(
                |(name, tool)| match advertised_tool_info_into_tool_info(tool) {
                    Ok(tool) => Some((name, tool)),
                    Err(err) => {
                        warn!("failed to hydrate advertised MCP tool {name} from rollout: {err}");
                        None
                    }
                },
            )
            .collect();
        Self {
            has_servers: catalog.has_servers,
            tools,
        }
    }
}

impl McpToolCatalogSnapshot {
    pub fn to_advertised(&self) -> Option<AdvertisedMcpToolCatalog> {
        if !self.has_servers && self.tools.is_empty() {
            return None;
        }

        let tools = self
            .tools
            .iter()
            .filter_map(
                |(name, tool)| match advertised_tool_info_from_tool_info(tool) {
                    Ok(tool) => Some((name.clone(), tool)),
                    Err(err) => {
                        warn!("failed to persist advertised MCP tool {name}: {err}");
                        None
                    }
                },
            )
            .collect();

        Some(AdvertisedMcpToolCatalog {
            has_servers: self.has_servers,
            tools,
        })
    }
}

fn advertised_tool_info_from_tool_info(
    tool: &ToolInfo,
) -> serde_json::Result<AdvertisedMcpToolInfo> {
    Ok(AdvertisedMcpToolInfo {
        server_name: tool.server_name.clone(),
        callable_name: tool.callable_name.clone(),
        callable_namespace: tool.callable_namespace.clone(),
        server_instructions: tool.server_instructions.clone(),
        tool: serde_json::to_value(&tool.tool)?,
        connector_id: tool.connector_id.clone(),
        connector_name: tool.connector_name.clone(),
        plugin_display_names: tool.plugin_display_names.clone(),
        connector_description: tool.connector_description.clone(),
    })
}

fn advertised_tool_info_into_tool_info(
    tool: AdvertisedMcpToolInfo,
) -> serde_json::Result<ToolInfo> {
    Ok(ToolInfo {
        server_name: tool.server_name,
        callable_name: tool.callable_name,
        callable_namespace: tool.callable_namespace,
        server_instructions: tool.server_instructions,
        tool: serde_json::from_value(tool.tool)?,
        connector_id: tool.connector_id,
        connector_name: tool.connector_name,
        plugin_display_names: tool.plugin_display_names,
        connector_description: tool.connector_description,
    })
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
            .resolve("mcp__observability__get_dashboard", /*namespace*/ None)
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
        let merged = catalog.merge(McpToolCatalogUpdate {
            has_servers: true,
            tools: HashMap::new(),
        });
        assert!(merged.changed);
        assert!(merged.snapshot.has_servers);

        let merged = catalog.merge(McpToolCatalogUpdate {
            has_servers: false,
            tools: HashMap::new(),
        });
        assert!(!merged.changed);
        assert!(merged.snapshot.has_servers);
        assert!(merged.snapshot.tools.is_empty());
    }
}
