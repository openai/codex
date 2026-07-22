use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::Resource;
use rmcp::model::ResourceTemplate;
use tokio::task::JoinSet;
use tracing::warn;

use crate::rmcp_client::ManagedClient;

/// The ready clients captured for one model step.
pub(crate) struct McpBindingClients {
    clients: HashMap<String, Arc<ManagedClient>>,
}

impl McpBindingClients {
    pub(crate) fn new(clients: HashMap<String, Arc<ManagedClient>>) -> Self {
        Self { clients }
    }

    pub(crate) fn client(&self, server: &str) -> Option<Arc<ManagedClient>> {
        self.clients.get(server).cloned()
    }

    pub(crate) async fn list_resources(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> Result<ListResourcesResult> {
        let managed = self
            .client(server)
            .ok_or_else(|| anyhow!("MCP server '{server}' was not ready for this step"))?;
        managed
            .client
            .list_resources(params, managed.tool_timeout)
            .await
            .with_context(|| format!("resources/list failed for `{server}`"))
    }

    pub(crate) async fn list_resource_templates(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> Result<ListResourceTemplatesResult> {
        let managed = self
            .client(server)
            .ok_or_else(|| anyhow!("MCP server '{server}' was not ready for this step"))?;
        managed
            .client
            .list_resource_templates(params, managed.tool_timeout)
            .await
            .with_context(|| format!("resources/templates/list failed for `{server}`"))
    }

    pub(crate) async fn read_resource(
        &self,
        server: &str,
        params: ReadResourceRequestParams,
    ) -> Result<ReadResourceResult> {
        let managed = self
            .client(server)
            .ok_or_else(|| anyhow!("MCP server '{server}' was not ready for this step"))?;
        let uri = params.uri.clone();
        managed
            .client
            .read_resource(params, managed.tool_timeout)
            .await
            .with_context(|| format!("resources/read failed for `{server}` ({uri})"))
    }

    pub(crate) async fn list_all_resources(
        &self,
        include_server: impl Fn(&str) -> bool,
    ) -> HashMap<String, Vec<Resource>> {
        let mut join_set = JoinSet::new();
        for (server_name, managed) in self
            .clients
            .iter()
            .filter(|(server_name, _)| include_server(server_name))
        {
            let server_name = server_name.clone();
            let client = Arc::clone(&managed.client);
            let timeout = managed.tool_timeout;
            join_set.spawn(async move {
                let mut collected = Vec::new();
                let mut cursor: Option<String> = None;
                loop {
                    let params = cursor.as_ref().map(|next| {
                        PaginatedRequestParams::default().with_cursor(Some(next.clone()))
                    });
                    let response = match client.list_resources(params, timeout).await {
                        Ok(result) => result,
                        Err(error) => return (server_name, Err(error)),
                    };
                    collected.extend(response.resources);
                    match response.next_cursor {
                        Some(next) if cursor.as_ref() == Some(&next) => {
                            return (
                                server_name,
                                Err(anyhow!("resources/list returned duplicate cursor")),
                            );
                        }
                        Some(next) => cursor = Some(next),
                        None => return (server_name, Ok(collected)),
                    }
                }
            });
        }
        collect_resource_results(&mut join_set, "resources").await
    }

    pub(crate) async fn list_all_resource_templates(
        &self,
        include_server: impl Fn(&str) -> bool,
    ) -> HashMap<String, Vec<ResourceTemplate>> {
        let mut join_set = JoinSet::new();
        for (server_name, managed) in self
            .clients
            .iter()
            .filter(|(server_name, _)| include_server(server_name))
        {
            let server_name = server_name.clone();
            let client = Arc::clone(&managed.client);
            let timeout = managed.tool_timeout;
            join_set.spawn(async move {
                let mut collected = Vec::new();
                let mut cursor: Option<String> = None;
                loop {
                    let params = cursor.as_ref().map(|next| {
                        PaginatedRequestParams::default().with_cursor(Some(next.clone()))
                    });
                    let response = match client.list_resource_templates(params, timeout).await {
                        Ok(result) => result,
                        Err(error) => return (server_name, Err(error)),
                    };
                    collected.extend(response.resource_templates);
                    match response.next_cursor {
                        Some(next) if cursor.as_ref() == Some(&next) => {
                            return (
                                server_name,
                                Err(anyhow!(
                                    "resources/templates/list returned duplicate cursor"
                                )),
                            );
                        }
                        Some(next) => cursor = Some(next),
                        None => return (server_name, Ok(collected)),
                    }
                }
            });
        }
        collect_resource_results(&mut join_set, "resource templates").await
    }
}

async fn collect_resource_results<T: Send + 'static>(
    join_set: &mut JoinSet<(String, Result<Vec<T>>)>,
    kind: &str,
) -> HashMap<String, Vec<T>> {
    let mut resources = HashMap::new();
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok((server, Ok(server_resources))) => {
                resources.insert(server, server_resources);
            }
            Ok((server, Err(error))) => {
                warn!("Failed to list {kind} for MCP server '{server}': {error:#}");
            }
            Err(error) => {
                warn!("Task panic when listing {kind} for MCP server: {error:#}");
            }
        }
    }
    resources
}
