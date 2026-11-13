//! Resolve globally advertised statusline tools and read them on the displayed thread.

mod descriptor;
pub(crate) mod discovery;
mod inventory;
#[cfg(test)]
mod test_support;

use crate::config_update::read_effective_config_with_layers;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::McpResourceReadParams;
use codex_app_server_protocol::McpResourceReadResponse;
use codex_app_server_protocol::McpServerConnectionStatus;
use codex_app_server_protocol::RequestId;
use codex_protocol::ThreadId;
use codex_utils_path_uri::PathUri;
use color_eyre::Result;
use color_eyre::eyre::OptionExt as _;
use color_eyre::eyre::bail;
pub(crate) use descriptor::StatuslineProvider;
use serde_json::Value;
use std::collections::HashMap;
use tokio::time::Duration;
use uuid::Uuid;

const LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
pub(crate) enum ProviderLookupError {
    #[error("statusline provider lookup timed out after 5 seconds")]
    TimedOut,
    #[error(transparent)]
    Request(#[from] color_eyre::Report),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct McpServerName(String);

impl McpServerName {
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for McpServerName {
    fn from(name: &str) -> Self {
        Self(name.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct McpToolName(String);

impl From<&str> for McpToolName {
    fn from(name: &str) -> Self {
        Self(name.to_owned())
    }
}

/// The saved identity is independent of the latest display title and resource URI.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ProviderId {
    pub(crate) server: McpServerName,
    pub(crate) tool: McpToolName,
}

#[cfg_attr(not(test), expect(dead_code, reason = "staged provider choice"))]
pub(crate) enum ProviderSelection<'a> {
    Saved(&'a ProviderId),
    /// A provider advertised during the current picker refresh.
    Discovered(&'a StatuslineProvider),
}

pub(crate) struct ResolvedStatuslineProvider {
    client: AppServerRequestHandle,
    thread: ThreadId,
    provider: StatuslineProvider,
}

#[cfg_attr(not(test), expect(dead_code, reason = "staged resource reader"))]
impl ResolvedStatuslineProvider {
    pub(crate) async fn lookup(
        client: &AppServerRequestHandle,
        thread: ThreadId,
        cwd: &PathUri,
        selection: ProviderSelection<'_>,
    ) -> Result<Option<Self>, ProviderLookupError> {
        tokio::select! {
            biased;
            () = tokio::time::sleep(LOOKUP_TIMEOUT) => Err(ProviderLookupError::TimedOut),
            result = Self::resolve(client, thread, cwd, selection) => Ok(result?),
        }
    }

    async fn resolve(
        client: &AppServerRequestHandle,
        thread: ThreadId,
        cwd: &PathUri,
        selection: ProviderSelection<'_>,
    ) -> Result<Option<Self>> {
        let config =
            read_effective_config_with_layers(client, cwd.inferred_native_path_string()).await?;
        let layers = config
            .layers
            .ok_or_eyre("config/read omitted the requested layers")?;
        let project_sets = |section: &str, key: &str| {
            let definitions: Vec<_> = layers
                .iter()
                .filter(|layer| layer["disabledReason"].is_null())
                .filter_map(|layer| {
                    Some((
                        layer["name"]["type"] == "project",
                        layer["config"][section].get(key)?,
                    ))
                })
                .collect();
            has_project_field(&definitions)
        };
        let (id, advertisement) = match selection {
            ProviderSelection::Saved(id) => (id, None),
            ProviderSelection::Discovered(provider) => (&provider.id, Some(provider)),
        };
        if project_sets("mcp_servers", id.server.as_str()) {
            return Ok(None);
        }
        let owner = if let Some(provider) = advertisement {
            provider.plugin_id.clone()
        } else {
            let Some(global) = inventory::server(client, &id.server, /*thread*/ None).await? else {
                return Ok(None);
            };
            if global.provider(&id.tool)?.is_none() {
                return Ok(None);
            }
            global.plugin_id
        };
        if let Some(plugin) = &owner
            && (project_sets("plugins", &plugin.as_key())
                || project_sets("marketplaces", &plugin.marketplace_name))
        {
            return Ok(None);
        }
        let Some(live) = inventory::server(client, &id.server, Some(thread)).await? else {
            return Ok(None);
        };
        if live.plugin_id != owner {
            return Ok(None);
        }
        let provider = live.provider(&id.tool)?;
        if live.runtime_status != Some(McpServerConnectionStatus::Connected) {
            bail!("MCP server {} is not connected", id.server.as_str());
        }
        Ok(provider.map(|provider| Self {
            client: client.clone(),
            thread,
            provider,
        }))
    }

    pub(crate) fn provider(&self) -> &StatuslineProvider {
        &self.provider
    }

    /// The host routes by name. A reload can replace that name between lookup and read.
    /// A thread/start override can also replace a configured command before lookup. Both
    /// registrations report the same name and no plugin ID. Same-owner plugin replacements
    /// are indistinguishable for the same reason.
    pub(crate) async fn read(&self, snapshot: Value) -> Result<McpResourceReadResponse> {
        let provider = &self.provider;
        Ok(self
            .client
            .request_typed(ClientRequest::McpResourceRead {
                request_id: RequestId::String(Uuid::new_v4().to_string()),
                params: McpResourceReadParams {
                    thread_id: Some(self.thread.to_string()),
                    server: provider.id.server.0.clone(),
                    environment_id: None,
                    uri: provider.uri.clone(),
                    connector_id: None,
                    origin_call_id: None,
                    target: None,
                    meta: Some(serde_json::Map::from_iter([(
                        "com.openai/codex-statusline-state".into(),
                        snapshot,
                    )])),
                },
            })
            .await?)
    }
}

/// Config layers arrive highest first. A project field only matters if a higher layer
/// leaves it in the effective registration. Arrays and scalars are replaced as a whole.
fn has_project_field(definitions: &[(bool, &Value)]) -> bool {
    let Some((&(project, first), rest)) = definitions.split_first() else {
        return false;
    };
    if !first.is_object() || rest.iter().all(|(origin, _)| *origin == project) {
        return project;
    }
    let mut fields: HashMap<&str, Vec<_>> = HashMap::new();
    for &(project, value) in definitions {
        let Some(table) = value.as_object() else {
            break;
        };
        for (key, value) in table {
            fields.entry(key).or_default().push((project, value));
        }
    }
    if fields.is_empty() {
        project
    } else {
        fields.values().any(|values| has_project_field(values))
    }
}

#[cfg(test)]
#[path = "statusline_provider_tests.rs"]
mod tests;
