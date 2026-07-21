use std::sync::Arc;
use std::sync::Weak;

use anyhow::Context;
use anyhow::Result;
use codex_protocol::mcp::Resource;
use codex_protocol::mcp::ResourceContent;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ReadResourceRequestParams;

use crate::McpRuntime;
use crate::binding_clients::McpBindingClientIdentity;
use crate::binding_clients::McpBindingClients;
use crate::connection_manager::McpConnectionManager;

/// One page of resources returned by an MCP server.
#[derive(Clone, Debug, PartialEq)]
pub struct McpResourcePage {
    /// Resources advertised on this page.
    pub resources: Vec<Resource>,
    /// Opaque cursor to supply when requesting the next page.
    pub next_cursor: Option<String>,
}

/// Contents returned after reading one MCP resource.
#[derive(Clone, Debug, PartialEq)]
pub struct McpResourceReadResult {
    /// Text or blob content returned for the requested resource.
    pub contents: Vec<ResourceContent>,
}

/// Access to MCP resources through either the latest runtime or one exact step.
#[derive(Clone)]
pub struct McpResourceClient {
    source: McpResourceSource,
}

/// Opaque identity for the connection set currently used by an MCP resource client.
#[derive(Clone)]
pub struct McpResourceClientCacheKey(McpResourceClientCacheKeyInner);

#[derive(Clone)]
enum McpResourceClientCacheKeyInner {
    Latest(Weak<McpConnectionManager>),
    Exact(McpBindingClientIdentity),
}

#[derive(Clone)]
enum McpResourceSource {
    Latest(Arc<McpRuntime>),
    Exact(Arc<McpBindingClients>),
}

impl PartialEq for McpResourceClientCacheKey {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (
                McpResourceClientCacheKeyInner::Latest(left),
                McpResourceClientCacheKeyInner::Latest(right),
            ) => left.ptr_eq(right),
            (
                McpResourceClientCacheKeyInner::Exact(left),
                McpResourceClientCacheKeyInner::Exact(right),
            ) => left == right,
            (
                McpResourceClientCacheKeyInner::Latest(_),
                McpResourceClientCacheKeyInner::Exact(_),
            )
            | (
                McpResourceClientCacheKeyInner::Exact(_),
                McpResourceClientCacheKeyInner::Latest(_),
            ) => false,
        }
    }
}

impl Eq for McpResourceClientCacheKey {}

impl std::fmt::Debug for McpResourceClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpResourceClient")
            .finish_non_exhaustive()
    }
}

impl McpResourceClient {
    /// Creates a resource client that follows the thread's latest published runtime.
    pub fn new(runtime: Arc<McpRuntime>) -> Self {
        Self {
            source: McpResourceSource::Latest(runtime),
        }
    }

    pub(crate) fn for_binding(clients: Arc<McpBindingClients>) -> Self {
        Self {
            source: McpResourceSource::Exact(clients),
        }
    }

    /// Returns the identity of the connection set used by this client.
    pub fn cache_key(&self) -> McpResourceClientCacheKey {
        let key = match &self.source {
            McpResourceSource::Latest(runtime) => {
                McpResourceClientCacheKeyInner::Latest(Arc::downgrade(&runtime.snapshot()))
            }
            McpResourceSource::Exact(clients) => {
                McpResourceClientCacheKeyInner::Exact(clients.identity())
            }
        };
        McpResourceClientCacheKey(key)
    }

    /// Returns whether this client can address the named server.
    ///
    /// This does not wait for server startup.
    pub async fn has_server(&self, server: &str) -> bool {
        match &self.source {
            McpResourceSource::Latest(runtime) => runtime.snapshot().contains_server(server),
            McpResourceSource::Exact(clients) => clients.contains_server(server),
        }
    }

    /// Lists one resource page from the named server.
    pub async fn list_resources(
        &self,
        server: &str,
        cursor: Option<String>,
    ) -> Result<McpResourcePage> {
        let params =
            cursor.map(|cursor| PaginatedRequestParams::default().with_cursor(Some(cursor)));
        let result = match &self.source {
            McpResourceSource::Latest(runtime) => {
                runtime.snapshot().list_resources(server, params).await
            }
            McpResourceSource::Exact(clients) => clients.list_resources(server, params).await,
        }?;
        let resources = result
            .resources
            .into_iter()
            .map(resource_from_rmcp)
            .collect::<Result<Vec<_>>>()?;
        Ok(McpResourcePage {
            resources,
            next_cursor: result.next_cursor,
        })
    }

    /// Reads one resource from the named server.
    pub async fn read_resource(&self, server: &str, uri: &str) -> Result<McpResourceReadResult> {
        let params = ReadResourceRequestParams::new(uri.to_string());
        let result = match &self.source {
            McpResourceSource::Latest(runtime) => {
                runtime.snapshot().read_resource(server, params).await
            }
            McpResourceSource::Exact(clients) => clients.read_resource(server, params).await,
        }?;
        let contents = result
            .contents
            .into_iter()
            .map(resource_content_from_rmcp)
            .collect::<Result<Vec<_>>>()?;
        Ok(McpResourceReadResult { contents })
    }
}

fn resource_from_rmcp(resource: rmcp::model::Resource) -> Result<Resource> {
    let value = serde_json::to_value(resource).context("failed to serialize MCP resource")?;
    Resource::from_mcp_value(value).context("failed to convert MCP resource")
}

fn resource_content_from_rmcp(content: rmcp::model::ResourceContents) -> Result<ResourceContent> {
    let value =
        serde_json::to_value(content).context("failed to serialize MCP resource content")?;
    serde_json::from_value(value).context("failed to convert MCP resource content")
}
