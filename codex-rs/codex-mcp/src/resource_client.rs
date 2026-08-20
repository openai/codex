use std::sync::Arc;
use std::sync::Weak;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_protocol::mcp::Resource;
use codex_protocol::mcp::ResourceContent;
use codex_rmcp_client::CancellableEventStreamRequest;
use rmcp::model::GetMeta;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ServerResult;
use rmcp::service::ServiceError;
use serde::Deserialize;
use serde_json::Map;
use serde_json::Value;
use serde_json::json;
use tokio::runtime::Handle;
use tokio::sync::watch;

use crate::McpRuntime;
use crate::connection_manager::McpConnectionSet;
use crate::connection_manager::McpServerConnection;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;

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

/// An event advertised by an MCP server.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpEventDefinition {
    pub name: String,
    pub description: String,
    pub delivery: Vec<String>,
    pub input_schema: Value,
    pub payload_schema: Value,
}

/// Events returned from one stable MCP connection generation.
pub struct McpEventCatalogSnapshot {
    pub cache_key: McpResourceClientCacheKey,
    pub events: Vec<McpEventDefinition>,
}

/// One unmodified lifecycle notification from an MCP event subscription.
#[derive(Clone, Debug, PartialEq)]
pub struct McpEventNotification {
    pub method: String,
    pub params: Option<Value>,
}

/// Owns an MCP event subscription and cancels its request when dropped.
pub struct McpEventStream {
    request: Option<CancellableEventStreamRequest>,
    runtime_handle: Handle,
    connection: Option<Arc<McpServerConnection>>,
    hosted_event_server_removals: watch::Receiver<()>,
}

impl McpEventStream {
    /// Receives the next raw lifecycle notification for this subscription.
    pub async fn recv(&mut self) -> Result<Option<McpEventNotification>> {
        let Some(request) = self.request.as_mut() else {
            return Ok(None);
        };

        tokio::select! {
            biased;

            Ok(()) = self.hosted_event_server_removals.changed() => {
                self.cancel();
                Err(anyhow!("hosted MCP event server was removed"))
            }
            Some(notification) = request.notifications.recv() => {
                let metadata = notification.get_meta().0.0.clone();
                let mut params = notification.params;
                if !metadata.is_empty() {
                    params.get_or_insert_with(|| json!({}))["_meta"] = Value::Object(metadata);
                }
                Ok(Some(McpEventNotification {
                    method: notification.method,
                    params,
                }))
            }
            response = &mut request.handle.rx => {
                self.request = None;
                self.connection = None;

                match response {
                    Ok(Ok(_))
                    | Ok(Err(ServiceError::Cancelled { .. }))
                    | Ok(Err(ServiceError::TransportClosed))
                    | Err(_) => Ok(None),
                    Ok(Err(error)) => Err(error.into()),
                }
            }
        }
    }

    fn cancel(&mut self) {
        if let Some(CancellableEventStreamRequest {
            handle,
            notifications,
        }) = self.request.take()
        {
            drop(notifications);
            let connection = self.connection.take();
            self.runtime_handle.spawn(async move {
                let _ = tokio::time::timeout(
                    Duration::from_secs(30),
                    handle.cancel(Some("event subscription closed".to_string())),
                )
                .await;
                drop(connection);
            });
        }
    }
}

impl Drop for McpEventStream {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpEventListResult {
    events: Vec<McpEventDefinition>,
}

/// Access to MCP resources and event subscriptions through the latest runtime.
#[derive(Clone)]
pub struct McpResourceClient {
    runtime: Arc<McpRuntime>,
}

/// Opaque identity for the connection set currently used by an MCP resource client.
#[derive(Clone)]
pub struct McpResourceClientCacheKey(Weak<McpConnectionSet>);

impl PartialEq for McpResourceClientCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.ptr_eq(&other.0)
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
        Self { runtime }
    }

    /// Returns the identity of the connection set used by this client.
    pub fn cache_key(&self) -> McpResourceClientCacheKey {
        McpResourceClientCacheKey(Arc::downgrade(&self.runtime.latest_connections()))
    }

    /// Returns whether this client can address the named server.
    ///
    /// This does not wait for server startup.
    pub async fn has_server(&self, server: &str) -> bool {
        self.runtime.latest_connections().contains_server(server)
    }

    /// Lists one resource page from the named server.
    pub async fn list_resources(
        &self,
        server: &str,
        cursor: Option<String>,
    ) -> Result<McpResourcePage> {
        let params =
            cursor.map(|cursor| PaginatedRequestParams::default().with_cursor(Some(cursor)));
        let result = self
            .runtime
            .latest_connections()
            .list_resources(server, params)
            .await?;
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
        let result = self
            .runtime
            .latest_connections()
            .read_resource(server, params)
            .await?;
        let contents = result
            .contents
            .into_iter()
            .map(resource_content_from_rmcp)
            .collect::<Result<Vec<_>>>()?;
        Ok(McpResourceReadResult { contents })
    }

    /// Lists the events advertised by the hosted Plugin Runtime.
    pub async fn list_events(&self) -> Result<McpEventCatalogSnapshot> {
        let (connections, _) = self
            .runtime
            .latest_connections_for_event_server(CODEX_APPS_MCP_SERVER_NAME)?;
        let cache_key = McpResourceClientCacheKey(Arc::downgrade(&connections));
        let (managed, request_timeout) = connections
            .client_by_name(CODEX_APPS_MCP_SERVER_NAME)
            .await?;
        let result = managed
            .client
            .send_custom_request_with_timeout("events/list", /*params*/ None, request_timeout)
            .await
            .context("events/list failed for hosted Plugin Runtime")?;
        let ServerResult::CustomResult(result) = result else {
            return Err(anyhow!("events/list returned an unexpected MCP result"));
        };
        let result = result
            .result_as::<McpEventListResult>()
            .context("events/list returned invalid event definitions")?;

        Ok(McpEventCatalogSnapshot {
            cache_key,
            events: result.events,
        })
    }

    /// Opens an MCP event subscription with the supplied event arguments.
    pub async fn open_event_stream(
        &self,
        event_name: &str,
        arguments: &Value,
        request_meta: Option<&Map<String, Value>>,
    ) -> Result<McpEventStream> {
        let mut params = json!({
            "name": event_name,
            "arguments": arguments,
        });
        if let Some(request_meta) = request_meta {
            params["_meta"] = Value::Object(request_meta.clone());
        }

        let (connections, hosted_event_server_removals) = self
            .runtime
            .latest_connections_for_event_server(CODEX_APPS_MCP_SERVER_NAME)?;
        let (managed, _, connection) = connections
            .client_with_connection_by_name(CODEX_APPS_MCP_SERVER_NAME)
            .await?;
        let request = managed
            .client
            .send_event_stream_request(Some(params))
            .await
            .context("events/stream failed for hosted Plugin Runtime")?;

        Ok(McpEventStream {
            request: Some(request),
            runtime_handle: Handle::current(),
            connection: Some(connection),
            hosted_event_server_removals,
        })
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
