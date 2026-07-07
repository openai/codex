use codex_app_server_protocol::ThreadItem;
use std::collections::VecDeque;

const MAX_MCP_RESOURCE_ORIGINS: usize = 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct McpResourceOrigin {
    pub(crate) server: String,
    pub(crate) tool: String,
    pub(crate) connector_id: String,
    pub(crate) link_id: Option<String>,
    pub(crate) resource_uri: String,
}

#[derive(Default)]
pub(crate) struct McpResourceOriginIndex(VecDeque<(String, McpResourceOrigin)>);

impl McpResourceOrigin {
    pub(crate) fn find(items: &[ThreadItem], call_id: &str) -> Option<Self> {
        items
            .iter()
            .find(|item| matches!(item, ThreadItem::McpToolCall { id, .. } if id == call_id))
            .and_then(|item| Self::from_item(item).map(|(_, origin)| origin))
    }

    fn from_item(item: &ThreadItem) -> Option<(&str, Self)> {
        let ThreadItem::McpToolCall {
            id,
            server,
            tool,
            app_context,
            ..
        } = item
        else {
            return None;
        };
        let app_context = app_context.as_ref()?;
        Some((
            id,
            Self {
                server: server.clone(),
                tool: tool.clone(),
                connector_id: app_context.connector_id.clone(),
                link_id: app_context.link_id.clone(),
                resource_uri: app_context.resource_uri.clone()?,
            },
        ))
    }
}

impl McpResourceOriginIndex {
    pub(crate) fn get(&self, call_id: &str) -> Option<McpResourceOrigin> {
        self.0.iter().find_map(|(existing_call_id, origin)| {
            (existing_call_id == call_id).then(|| origin.clone())
        })
    }

    pub(crate) fn seed<'a>(&mut self, items: impl IntoIterator<Item = &'a ThreadItem>) {
        for (call_id, origin) in items.into_iter().filter_map(McpResourceOrigin::from_item) {
            self.insert(call_id.to_string(), origin);
        }
    }

    pub(crate) fn insert(&mut self, call_id: String, origin: McpResourceOrigin) {
        self.0
            .retain(|(existing_call_id, _)| existing_call_id != &call_id);
        self.0.push_back((call_id, origin));
        if self.0.len() > MAX_MCP_RESOURCE_ORIGINS {
            self.0.pop_front();
        }
    }
}

#[cfg(test)]
#[path = "mcp_resource_origin_tests.rs"]
mod tests;
