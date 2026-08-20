use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

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

use super::McpConnectionSet;
use super::McpServerConnection;
use crate::pagination::collect_paginated;
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
                let resources = collect_paginated("resources/list", timeout, |params| {
                    let client = Arc::clone(&client);
                    async move {
                        let response = client.list_resources(params, timeout).await?;
                        Ok((response.resources, response.next_cursor))
                    }
                })
                .await;
                (server_name, resources)
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
                let templates = collect_paginated("resources/templates/list", timeout, |params| {
                    let client = Arc::clone(&client);
                    async move {
                        let response = client.list_resource_templates(params, timeout).await?;
                        Ok((response.resource_templates, response.next_cursor))
                    }
                })
                .await;
                (server_name, templates)
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

    pub async fn list_resource_templates(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> Result<ListResourceTemplatesResult> {
        let (managed, timeout) = self.client_by_name(server).await?;
        managed
            .client
            .list_resource_templates(params, timeout)
            .await
            .with_context(|| format!("resources/templates/list failed for `{server}`"))
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

    pub(crate) async fn client_by_name(
        &self,
        name: &str,
    ) -> Result<(ManagedClient, Option<Duration>)> {
        let (client, timeout, _connection) = self.client_with_connection_by_name(name).await?;
        Ok((client, timeout))
    }

    pub(crate) async fn client_with_connection_by_name(
        &self,
        name: &str,
    ) -> Result<(ManagedClient, Option<Duration>, Arc<McpServerConnection>)> {
        let view = self
            .servers
            .get(name)
            .ok_or_else(|| anyhow!("unknown MCP server '{name}'"))?;
        let connection = Arc::clone(&view.connection);
        let client = connection.client().await.context("failed to get client")?;
        Ok((client, view.tool_timeout, connection))
    }
}
