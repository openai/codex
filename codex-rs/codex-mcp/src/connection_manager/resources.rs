use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use rmcp::model::ListResourcesResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::Resource;
use rmcp::model::ResourceTemplate;
use tokio::task::JoinSet;
use tracing::warn;

use super::McpConnectionSet;
use crate::rmcp_client::ManagedClient;

impl McpConnectionSet {
    /// Returns resources from servers selected by `include_server`.
    pub async fn list_all_resources(
        &self,
        include_server: impl Fn(&str) -> bool,
    ) -> HashMap<String, Vec<Resource>> {
        let mut join_set = JoinSet::new();
        for (server_name, view) in self
            .servers
            .iter()
            .filter(|(server_name, _)| include_server(server_name))
        {
            let server_name = server_name.clone();
            let Ok(managed_client) = view.connection.client().await else {
                continue;
            };
            let timeout = view.tool_timeout;
            let client = managed_client.client;
            join_set.spawn(async move {
                let mut resources = Vec::new();
                let mut cursor: Option<String> = None;
                loop {
                    let params = cursor.as_ref().map(|next| {
                        PaginatedRequestParams::default().with_cursor(Some(next.clone()))
                    });
                    let response = match client.list_resources(params, timeout).await {
                        Ok(result) => result,
                        Err(error) => return (server_name, Err(error)),
                    };
                    resources.extend(response.resources);
                    match response.next_cursor {
                        Some(next) if cursor.as_ref() == Some(&next) => {
                            return (
                                server_name,
                                Err(anyhow!("resources/list returned duplicate cursor")),
                            );
                        }
                        Some(next) => cursor = Some(next),
                        None => return (server_name, Ok(resources)),
                    }
                }
            });
        }

        let mut resources = HashMap::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok((server_name, Ok(server_resources))) => {
                    resources.insert(server_name, server_resources);
                }
                Ok((server_name, Err(error))) => {
                    warn!("Failed to list resources for MCP server '{server_name}': {error:#}");
                }
                Err(error) => {
                    warn!("Task panic when listing resources for MCP server: {error:#}");
                }
            }
        }
        resources
    }

    /// Returns resource templates from servers selected by `include_server`.
    pub async fn list_all_resource_templates(
        &self,
        include_server: impl Fn(&str) -> bool,
    ) -> HashMap<String, Vec<ResourceTemplate>> {
        let mut join_set = JoinSet::new();
        for (server_name, view) in self
            .servers
            .iter()
            .filter(|(server_name, _)| include_server(server_name))
        {
            let server_name = server_name.clone();
            let Ok(managed_client) = view.connection.client().await else {
                continue;
            };
            let timeout = view.tool_timeout;
            let client = managed_client.client;
            join_set.spawn(async move {
                let mut templates = Vec::new();
                let mut cursor: Option<String> = None;
                loop {
                    let params = cursor.as_ref().map(|next| {
                        PaginatedRequestParams::default().with_cursor(Some(next.clone()))
                    });
                    let response = match client.list_resource_templates(params, timeout).await {
                        Ok(result) => result,
                        Err(error) => return (server_name, Err(error)),
                    };
                    templates.extend(response.resource_templates);
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
                        None => return (server_name, Ok(templates)),
                    }
                }
            });
        }

        let mut templates = HashMap::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok((server_name, Ok(server_templates))) => {
                    templates.insert(server_name, server_templates);
                }
                Ok((server_name, Err(error))) => {
                    warn!(
                        "Failed to list resource templates for MCP server '{server_name}': {error:#}"
                    );
                }
                Err(error) => {
                    warn!("Task panic when listing resource templates for MCP server: {error:#}");
                }
            }
        }
        templates
    }

    pub async fn list_resources(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> Result<ListResourcesResult> {
        let (managed, timeout) = self.client_by_name(server).await?;
        managed
            .client
            .list_resources(params, timeout)
            .await
            .with_context(|| format!("resources/list failed for `{server}`"))
    }

    pub async fn read_resource(
        &self,
        server: &str,
        params: ReadResourceRequestParams,
    ) -> Result<ReadResourceResult> {
        let (managed, timeout) = self.client_by_name(server).await?;
        let uri = params.uri.clone();
        managed
            .client
            .read_resource(params, timeout)
            .await
            .with_context(|| format!("resources/read failed for `{server}` ({uri})"))
    }

    async fn client_by_name(&self, name: &str) -> Result<(ManagedClient, Option<Duration>)> {
        let view = self
            .servers
            .get(name)
            .ok_or_else(|| anyhow!("unknown MCP server '{name}'"))?;
        let client = view
            .connection
            .client()
            .await
            .context("failed to get client")?;
        Ok((client, view.tool_timeout))
    }
}
