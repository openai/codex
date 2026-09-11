//! Enterprise MCP configuration and registration provenance.
//!
//! Only host, user, or managed configuration selects the IdP. Plugin declarations
//! and project settings may not redirect the enterprise credential source.

use std::io;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

use codex_features::Feature;

use crate::ConfigLayerSource;
use crate::ConfigLayerStack;
use crate::McpServerConfig;
use crate::McpServerTransportConfig;
use crate::types::PluginMcpServerEmaAuthConfig;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct McpServerIdpOAuthConfig {
    /// Issuer used for enterprise OAuth discovery and identity validation.
    pub issuer: String,
    /// Public OAuth client registered with that enterprise IdP.
    pub client_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct McpEnterpriseManagedAuthConfig {
    /// Shared enterprise authorization, independent of Codex account credentials.
    pub idp: McpServerIdpOAuthConfig,
}

/// Immutable authorization selected after trusted policy and catalog resolution.
/// This cannot be deserialized from a server or plugin declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpEmaRegistration {
    idp: McpServerIdpOAuthConfig,
    server_url: String,
    resource: Option<String>,
    client_id: Option<String>,
    authorization_server_issuer: Option<String>,
    scopes: Vec<String>,
}

impl McpEmaRegistration {
    pub fn idp(&self) -> &McpServerIdpOAuthConfig {
        &self.idp
    }

    pub fn server_url(&self) -> &str {
        &self.server_url
    }

    pub fn resource(&self) -> Option<&str> {
        self.resource.as_deref()
    }

    pub fn client_id(&self) -> Option<&str> {
        self.client_id.as_deref()
    }

    pub fn authorization_server_issuer(&self) -> Option<&str> {
        self.authorization_server_issuer.as_deref()
    }

    pub fn scopes(&self) -> &[String] {
        &self.scopes
    }
}

impl McpEnterpriseManagedAuthConfig {
    /// Resolves the trusted profile and checks provenance at a config load boundary.
    /// Plugin endpoints are checked later, when their declarations are materialized.
    pub fn resolve(
        stack: &ConfigLayerStack,
        fallback: Option<&Self>,
        servers: &std::collections::HashMap<String, McpServerConfig>,
        xaa_enabled: bool,
    ) -> io::Result<Option<Self>> {
        validate_ema_auth_sources(stack, servers)?;
        validate_xaa_opt_in_source(stack, xaa_enabled)?;
        Self::from_config_layers(stack, fallback)
    }

    /// Selects one complete trusted registration rather than merging issuer/client pairs.
    fn from_config_layers(
        stack: &ConfigLayerStack,
        fallback: Option<&Self>,
    ) -> io::Result<Option<Self>> {
        if stack.layers_high_to_low().next().is_none() {
            return Ok(fallback.cloned());
        }
        let sections = || {
            stack.layers_high_to_low().filter_map(|layer| {
                layer
                    .config
                    .get("mcp_enterprise_managed_auth")
                    .map(|section| (&layer.name, section))
            })
        };
        let selected = sections()
            .find(|(source, _)| {
                matches!(
                    source,
                    ConfigLayerSource::Mdm { .. }
                        | ConfigLayerSource::System { .. }
                        | ConfigLayerSource::EnterpriseManaged { .. }
                        | ConfigLayerSource::LegacyManagedConfigTomlFromFile { .. }
                        | ConfigLayerSource::LegacyManagedConfigTomlFromMdm
                )
            })
            .or_else(|| {
                sections().find(|(source, _)| !matches!(source, ConfigLayerSource::Project { .. }))
            });
        selected
            .map(|(_, section)| {
                section.clone().try_into().map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "enterprise IdP must be complete in one trusted config layer: {error}"
                        ),
                    )
                })
            })
            .transpose()
    }
}

impl McpServerConfig {
    pub fn oauth_idp(&self) -> Option<&McpServerIdpOAuthConfig> {
        self.ema_registration().map(McpEmaRegistration::idp)
    }

    pub fn ema_registration(&self) -> Option<&McpEmaRegistration> {
        self.oauth.as_ref()?.ema_registration.as_ref()
    }

    /// EMA never falls back to an unrelated bearer or executor-owned credential.
    pub fn validate_ema_auth_transport(&self) -> Result<(), &'static str> {
        if !self.is_local_environment() {
            return Err("ema_auth requires a host-owned MCP connection");
        }
        let McpServerTransportConfig::StreamableHttp {
            bearer_token_env_var,
            http_headers,
            env_http_headers,
            http_headers_helper,
            ..
        } = &self.transport
        else {
            return Err("ema_auth requires streamable HTTP");
        };
        if bearer_token_env_var.is_some()
            || http_headers
                .as_ref()
                .is_some_and(|headers| !headers.is_empty())
            || env_http_headers
                .as_ref()
                .is_some_and(|headers| !headers.is_empty())
            || http_headers_helper.is_some()
        {
            return Err("ema_auth cannot be combined with alternate HTTP credentials");
        }
        Ok(())
    }
}

fn plugin_ema_registration(
    config: &toml::Value,
    plugin_name: &str,
    server_name: &str,
) -> Option<PluginMcpServerEmaAuthConfig> {
    config
        .get("plugins")?
        .get(plugin_name)?
        .get("mcp_servers")?
        .get(server_name)?
        .get("ema_auth")?
        .clone()
        .try_into()
        .ok()
}

fn has_trusted_atomic_registration(
    stack: &ConfigLayerStack,
    non_project: &toml::Value,
    matches: impl Fn(&toml::Value) -> bool,
) -> bool {
    matches(non_project)
        && stack
            .layers_high_to_low()
            .filter(|layer| !matches!(layer.name, ConfigLayerSource::Project { .. }))
            .any(|layer| matches(&layer.config))
}

fn reenabled_over_non_project_denial(
    effective_enabled: bool,
    non_project: Option<&toml::Value>,
) -> bool {
    effective_enabled
        && non_project
            .and_then(|server| server.get("enabled"))
            .and_then(toml::Value::as_bool)
            == Some(false)
}

/// Projects cannot add, replace, or downgrade a non-project enterprise registration.
fn validate_ema_auth_sources(
    stack: &ConfigLayerStack,
    servers: &std::collections::HashMap<String, McpServerConfig>,
) -> io::Result<()> {
    if stack.layers_high_to_low().next().is_none() {
        return Ok(());
    }
    let mut non_project = toml::Value::Table(Default::default());
    for layer in stack
        .layers_low_to_high()
        .filter(|layer| !matches!(layer.name, ConfigLayerSource::Project { .. }))
    {
        crate::merge_toml_values(&mut non_project, &layer.config);
    }

    for (name, effective) in servers {
        let non_project_server = non_project
            .get("mcp_servers")
            .and_then(|servers| servers.get(name));
        let inherited_ema = non_project_server
            .and_then(|server| server.get("auth"))
            .and_then(toml::Value::as_str)
            == Some("ema_auth");
        if !matches!(effective.auth, crate::McpServerAuth::EmaAuth) && !inherited_ema {
            continue;
        }
        if reenabled_over_non_project_denial(effective.enabled, non_project_server) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("project configuration cannot re-enable enterprise MCP server `{name}`"),
            ));
        }
        let matches_effective_authorization = |server: &McpServerConfig| {
            matches!(server.auth, crate::McpServerAuth::EmaAuth)
                && server.auth == effective.auth
                && server.transport == effective.transport
                && server.scopes == effective.scopes
                && server.oauth == effective.oauth
                && server.oauth_resource == effective.oauth_resource
        };
        if !has_trusted_atomic_registration(stack, &non_project, |config| {
            config
                .get("mcp_servers")
                .and_then(|servers| servers.get(name))
                .and_then(|server| server.clone().try_into::<McpServerConfig>().ok())
                .is_some_and(|server| matches_effective_authorization(&server))
        }) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "enterprise MCP server `{name}` must define its transport, authorization, scopes, and resource in one non-project config layer"
                ),
            ));
        }
    }

    let effective_config = stack.effective_config();
    let Some(plugins) = effective_config
        .get("plugins")
        .and_then(toml::Value::as_table)
    else {
        return Ok(());
    };
    for (plugin_name, plugin) in plugins {
        let Some(plugin_servers) = plugin.get("mcp_servers").and_then(toml::Value::as_table) else {
            continue;
        };
        for (server_name, server) in plugin_servers {
            let Some(ema_auth) = server.get("ema_auth") else {
                continue;
            };
            let non_project_server = non_project
                .get("plugins")
                .and_then(|plugins| plugins.get(plugin_name))
                .and_then(|plugin| plugin.get("mcp_servers"))
                .and_then(|servers| servers.get(server_name));
            if reenabled_over_non_project_denial(
                server.get("enabled").and_then(toml::Value::as_bool) != Some(false),
                non_project_server,
            ) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "project configuration cannot re-enable enterprise MCP plugin `{plugin_name}` server `{server_name}`"
                    ),
                ));
            }
            let effective = ema_auth
                .clone()
                .try_into::<PluginMcpServerEmaAuthConfig>()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
            if !has_trusted_atomic_registration(stack, &non_project, |config| {
                plugin_ema_registration(config, plugin_name, server_name)
                    .is_some_and(|registration| registration == effective)
            }) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "enterprise MCP registration for plugin `{plugin_name}` server `{server_name}` must be defined in one non-project config layer"
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Enabling XAA requires an explicit non-project setting or managed requirement.
fn validate_xaa_opt_in_source(stack: &ConfigLayerStack, xaa_enabled: bool) -> io::Result<()> {
    if !xaa_enabled || stack.layers_high_to_low().next().is_none() {
        return Ok(());
    }
    let required = stack
        .requirements()
        .feature_requirements
        .as_ref()
        .and_then(|requirements| requirements.value.entries.get(Feature::UseXaa.key()))
        .copied();
    let configured = stack
        .layers_high_to_low()
        .filter(|layer| !matches!(layer.name, ConfigLayerSource::Project { .. }))
        .find_map(|layer| {
            layer
                .config
                .get("features")?
                .get(Feature::UseXaa.key())?
                .as_bool()
        });
    if required.or(configured) == Some(true) {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "`[features].use_xaa = true` must be selected in a non-project config layer",
    ))
}

#[cfg(test)]
#[path = "mcp_ema_tests.rs"]
mod tests;
