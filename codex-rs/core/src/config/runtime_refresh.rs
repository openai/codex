//! Resolves refreshable MCP inputs without rebuilding unrelated session configuration.
//! Existing sessions refresh ordinary MCP state without gaining enterprise authority.
//! Validate the retained backend after applying the refreshable requirements.

use crate::config::Config;
use crate::config::ManagedFeatures;
use crate::config::constrain_mcp_servers;
use crate::config::resolve_tool_suggest_config_from_layer_stack;
use codex_config::McpServerAuth;
use codex_config::McpServerConfig;
use codex_features::Feature;
use codex_features::FeatureConfigSource;
use codex_features::FeatureOverrides;
use codex_features::Features;
use codex_features::FeaturesToml;
use serde::Deserialize;
use std::collections::HashMap;

#[cfg(test)]
#[path = "runtime_refresh_tests.rs"]
mod tests;

#[derive(Clone, Copy)]
pub(crate) enum RuntimeConfigRefresh {
    User,
    Mcp,
    UserFiles,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct McpRefreshToml {
    features: Option<FeaturesToml>,
    plugins: codex_config::types::PluginsConfigToml,
}

impl Config {
    pub(crate) fn resolve_runtime_refresh(
        &self,
        incoming: &Self,
        scope: RuntimeConfigRefresh,
    ) -> std::io::Result<Self> {
        let layers = match scope {
            RuntimeConfigRefresh::User | RuntimeConfigRefresh::UserFiles => self
                .config_layer_stack
                .with_user_layer_from(&incoming.config_layer_stack),
            RuntimeConfigRefresh::Mcp => Self::layer_stack_preserving_session(
                &self.config_layer_stack,
                &incoming.config_layer_stack,
            )?
            .with_user_layer_from(&self.config_layer_stack),
        };
        let cfg: McpRefreshToml = layers
            .effective_config()
            .try_into()
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        let mut configured_features = Features::from_sources(
            FeatureConfigSource {
                features: cfg.features.as_ref(),
                ..Default::default()
            },
            FeatureConfigSource::default(),
            FeatureOverrides::default(),
        );
        // These operational controls accept host defaults beneath explicit layer values.
        // Enterprise opt-in always comes from trusted layers instead.
        for feature in [
            Feature::SecretAuthStorage,
            Feature::McpOAuthRefreshCoordination,
        ] {
            if matches!(scope, RuntimeConfigRefresh::UserFiles) {
                let explicitly_configured = cfg.features.as_ref().is_some_and(|features| {
                    features
                        .entries()
                        .keys()
                        .any(|key| codex_features::feature_for_key(key) == Some(feature))
                });
                if !explicitly_configured
                    && let Some(enabled) = self.runtime_feature_defaults.get(&feature)
                {
                    configured_features.set_enabled(feature, *enabled);
                }
            } else {
                // A publication wait may have allowed a newer host/user override.
                configured_features.set_enabled(feature, incoming.features.enabled(feature));
            }
        }
        let features = ManagedFeatures::from_configured(
            configured_features,
            layers.requirements().feature_requirements.clone(),
        )?;
        let is_enterprise = |server: &McpServerConfig| {
            matches!(server.auth, McpServerAuth::EmaAuth) || server.ema_registration().is_some()
        };
        // Refresh ordinary servers from the complete host-owned set. Keep the
        // initially admitted EMA configuration until a fresh session is constructed.
        let mut servers: HashMap<_, _> = incoming
            .mcp_servers
            .get()
            .iter()
            .filter(|(name, server)| {
                !is_enterprise(server)
                    && !self.mcp_servers.get().get(*name).is_some_and(is_enterprise)
            })
            .map(|(name, server)| (name.clone(), server.clone()))
            .collect();
        servers.extend(
            self.mcp_servers
                .get()
                .iter()
                .filter(|(_, server)| is_enterprise(server))
                .map(|(name, server)| (name.clone(), server.clone())),
        );
        let mut config = self.clone();
        if !matches!(scope, RuntimeConfigRefresh::UserFiles) {
            config.runtime_feature_defaults = incoming.runtime_feature_defaults.clone();
            config.mcp_optional_startup_grace = incoming.mcp_optional_startup_grace;
            config.mcp_oauth_credentials_store_mode = incoming.mcp_oauth_credentials_store_mode;
        }
        config
            .features
            .refresh_mcp_features(&features)
            .map_err(std::io::Error::other)?;
        config.mcp_servers =
            constrain_mcp_servers(servers, layers.requirements().mcp_servers.as_ref())
                .map_err(std::io::Error::other)?;
        // A missing profile also carries the host's fail-closed load result.
        let enterprise_retired = incoming.mcp_enterprise_managed_auth.is_none()
            || !incoming.features.enabled(Feature::UseXaa)
            || (self
                .mcp_servers
                .get()
                .values()
                .any(|server| is_enterprise(server) && server.enabled)
                && incoming.mcp_enterprise_managed_auth != self.mcp_enterprise_managed_auth)
            || self.mcp_servers.get().iter().any(|(name, server)| {
                is_enterprise(server)
                    && server.enabled
                    && !incoming
                        .mcp_servers
                        .get()
                        .get(name)
                        .is_some_and(|incoming| {
                            incoming.enabled
                                && incoming.auth == server.auth
                                && incoming.transport == server.transport
                                && incoming.oauth_resource == server.oauth_resource
                                && incoming.oauth == server.oauth
                                && incoming.scopes == server.scopes
                                && incoming.environment_id == server.environment_id
                                && incoming.ema_registration() == server.ema_registration()
                        })
            })
            || self.mcp_servers.get().iter().any(|(name, server)| {
                is_enterprise(server)
                    && server.enabled
                    && !config
                        .mcp_servers
                        .get()
                        .get(name)
                        .is_some_and(|server| server.enabled)
            });
        if matches!(scope, RuntimeConfigRefresh::User) {
            config.active_project = incoming.active_project.clone();
        }
        if !matches!(scope, RuntimeConfigRefresh::Mcp) {
            config.tool_suggest = resolve_tool_suggest_config_from_layer_stack(&layers);
        }
        config.config_layer_stack = layers;
        config.plugins = cfg.plugins;
        if enterprise_retired {
            config.disable_mcp_enterprise_auth();
        }
        config.validate_windows_mxc_requirement()?;
        Ok(config)
    }

    /// Disables enterprise MCP authorization while preserving unrelated MCP servers.
    pub fn disable_mcp_enterprise_auth(&mut self) {
        self.mcp_enterprise_managed_auth = None;
        let mut servers = self.mcp_servers.get().clone();
        for server in servers.values_mut() {
            if matches!(server.auth, McpServerAuth::EmaAuth) || server.ema_registration().is_some()
            {
                server.enabled = false;
                if let Some(oauth) = &mut server.oauth {
                    oauth.ema_registration = None;
                }
            }
        }
        if self.mcp_servers.set(servers).is_err() {
            tracing::warn!("failed to clear disabled enterprise registrations");
        }
    }
}
