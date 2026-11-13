//! Read MCP tool catalogs from the connected app server’s global inventory.

use super::McpServerName;
use super::McpToolName;
use super::StatuslineProvider;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::McpServerConnectionStatus;
use codex_app_server_protocol::McpServerStatus;
use codex_app_server_protocol::McpServerStatusDetail;
use codex_app_server_protocol::RequestId;
use codex_plugin::PluginId;
use codex_plugin::PluginIdError;
use codex_protocol::ThreadId;
use codex_protocol::mcp::Tool;
use color_eyre::Result;
use color_eyre::eyre::bail;
use color_eyre::eyre::eyre;
use futures::Stream;
use futures::stream;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, PartialEq)]
pub(super) struct StatuslineServer {
    pub(super) name: McpServerName,
    pub(super) runtime_status: Option<McpServerConnectionStatus>,
    pub(super) plugin_id: Option<PluginId>,
    pub(super) tools: Result<HashMap<String, Tool>, String>,
}

impl TryFrom<McpServerStatus> for StatuslineServer {
    type Error = PluginIdError;

    fn try_from(row: McpServerStatus) -> Result<Self, Self::Error> {
        let tools = match row.tools_error {
            Some(error) => Err(error),
            None if row.runtime_status == Some(McpServerConnectionStatus::Failed) => {
                Err(format!("MCP server {} is not connected", row.name))
            }
            None => Ok(row.tools),
        };
        Ok(Self {
            name: McpServerName(row.name),
            runtime_status: row.runtime_status,
            plugin_id: row.plugin_id.as_deref().map(PluginId::parse).transpose()?,
            tools,
        })
    }
}

impl StatuslineServer {
    fn catalog(&self) -> Result<&HashMap<String, Tool>> {
        self.tools.as_ref().map_err(|error| eyre!("{error}"))
    }
    pub(super) fn providers(&self) -> Result<Vec<StatuslineProvider>> {
        Ok(self
            .catalog()?
            .values()
            .filter_map(|tool| {
                StatuslineProvider::from_tool(&self.name, self.plugin_id.as_ref(), tool)
            })
            .collect())
    }
    pub(super) fn provider(&self, name: &McpToolName) -> Result<Option<StatuslineProvider>> {
        Ok(self.catalog()?.get(&name.0).and_then(|tool| {
            StatuslineProvider::from_tool(&self.name, self.plugin_id.as_ref(), tool)
        }))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InventoryPage {
    data: Vec<Value>,
    next_cursor: Option<String>,
}

async fn read_page(
    client: &AppServerRequestHandle,
    params: ListMcpServerStatusParams,
) -> Result<InventoryPage> {
    Ok(client
        .request_typed(ClientRequest::McpServerStatusList {
            request_id: RequestId::String(Uuid::new_v4().to_string()),
            params,
        })
        .await?)
}

/// Request the whole inventory. Some hosts paginate by offset into a newly rebuilt catalog.
pub(super) fn global_pages(
    client: &AppServerRequestHandle,
) -> impl Stream<Item = Result<Vec<StatuslineServer>>> + '_ {
    stream::try_unfold(Some((None, HashSet::new())), move |next| async move {
        let Some((cursor, mut seen)) = next else {
            return Ok(None);
        };
        if let Some(cursor) = &cursor
            && !seen.insert(String::clone(cursor))
        {
            bail!("MCP inventory repeated a cursor");
        }
        let page = read_page(
            client,
            ListMcpServerStatusParams {
                thread_id: None,
                server_name: None,
                environment_id: None,
                detail: Some(McpServerStatusDetail::ToolsAndAuthOnly),
                cursor,
                limit: None,
            },
        )
        .await?;
        let mut servers = Vec::new();
        for value in page.data {
            if let Some(row) = local_server(value)? {
                servers.push(row);
            }
        }
        Ok(Some((
            servers,
            page.next_cursor.map(|next| (Some(next), seen)),
        )))
    })
}

fn local_server(value: Value) -> Result<Option<StatuslineServer>> {
    // Stdio servers are eligible. The app server sets `httpOrigin` to null for stdio
    // and for invalid or unsupported HTTP URLs. Skip entries from older hosts that
    // omit the field because their transport is unknown.
    if value.get("httpOrigin") != Some(&Value::Null) {
        return Ok(None);
    }
    let status = serde_json::from_value::<McpServerStatus>(value)?;
    Ok(StatuslineServer::try_from(status).ok())
}

pub(super) async fn server(
    client: &AppServerRequestHandle,
    name: &McpServerName,
    thread: Option<ThreadId>,
) -> Result<Option<StatuslineServer>> {
    let mut page = read_page(
        client,
        ListMcpServerStatusParams {
            thread_id: thread.map(|id| id.to_string()),
            server_name: Some(name.0.clone()),
            environment_id: None,
            detail: Some(McpServerStatusDetail::ToolsAndAuthOnly),
            cursor: None,
            limit: None,
        },
    )
    .await?;
    if page.next_cursor.is_some() || page.data.len() > 1 {
        bail!("targeted MCP inventory returned more than one server");
    }
    let Some(value) = page.data.pop() else {
        return Ok(None);
    };
    if value["name"] != name.as_str() {
        bail!("targeted MCP inventory did not identify the requested server");
    }
    local_server(value)
}

#[cfg(test)]
#[path = "inventory_tests.rs"]
mod tests;
