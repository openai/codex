//! Adds negotiated policy metadata using the same publication as the MCP call.

use super::McpConnectionSet;
use crate::MCP_NETWORK_POLICY_META_CAPABILITY;
use crate::rmcp_client::ManagedClient;
use anyhow::Context;
use codex_network_proxy::NetworkMode;
use codex_protocol::models::PermissionProfile;
use rmcp::model::ServerCapabilities;
use serde::Serialize;
use serde_json::Value;
use std::sync::OnceLock;

/// Effective network restrictions delivered with an MCP call, without proxy endpoints or credentials.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPolicyMetadata {
    pub enabled: bool,
    pub mode: NetworkMode,
    pub allowed_domains: Vec<String>,
    pub denied_domains: Vec<String>,
    pub hard_deny_allowlist_misses: bool,
}

/// Produces trusted network policy metadata from an MCP publication's frozen policy sources.
/// Implementations must use the supplied call authority and capture mutable grants once per call.
pub trait NetworkPolicyMetadataProvider: Send + Sync {
    fn metadata_for_call(
        &self,
        permission_profile: &PermissionProfile,
        environment_id: &str,
    ) -> anyhow::Result<NetworkPolicyMetadata>;
}

impl McpConnectionSet {
    pub(crate) fn network_policy_meta_for_call(
        &self,
        client: &ManagedClient,
        permission_profile: Option<&PermissionProfile>,
        environment_id: &str,
        mut meta: Option<Value>,
    ) -> impl Fn(&ServerCapabilities) -> anyhow::Result<Option<Value>> + Send + Sync {
        if let Some(Value::Object(meta)) = meta.as_mut() {
            meta.remove(MCP_NETWORK_POLICY_META_CAPABILITY);
        }
        // A retry can negotiate policy support for the first time. Capture mutable grants
        // only once, when an attempt needs them, and keep that snapshot through recovery.
        let policy = OnceLock::new();
        move |capabilities| {
            let mut meta = meta.clone();
            if client.client_supports_network_policy_meta_capability
                && capabilities
                    .experimental
                    .as_ref()
                    .is_some_and(|experimental| {
                        experimental.contains_key(MCP_NETWORK_POLICY_META_CAPABILITY)
                    })
            {
                let policy = policy.get_or_init(|| -> anyhow::Result<Value> {
                    let provider = self
                        .network_policy_metadata
                        .as_ref()
                        .context("MCP network policy metadata source is unavailable")?;
                    let permission_profile = permission_profile
                        .context("MCP server has no captured permission authority")?;
                    serde_json::to_value(
                        provider.metadata_for_call(permission_profile, environment_id)?,
                    )
                    .map_err(Into::into)
                });
                let policy = policy
                    .as_ref()
                    .map_err(|error| anyhow::anyhow!("{error:#}"))?;
                meta.get_or_insert_with(|| Value::Object(Default::default()))
                    .as_object_mut()
                    .context("MCP request metadata must be an object")?
                    .insert(
                        MCP_NETWORK_POLICY_META_CAPABILITY.to_string(),
                        policy.clone(),
                    );
            }
            Ok(meta)
        }
    }
}
