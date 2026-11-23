//! Expand local gateway enrollment into the existing policy and inference requirements.
//! The highest-priority enrollment supplies both destinations and their shared OAuth settings.

use super::layer::ComposableRequirementsLayer;
use super::stack::RequirementsCompositionError;
use crate::ConfigServer;
use crate::ConfigServerOAuth;
use codex_model_provider_info::GatewayOAuthConfig;
use codex_model_provider_info::GatewayOAuthSettings;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::WireApi;
use codex_model_provider_info::is_azure_provider_name;
use codex_model_provider_info::uses_builtin_auth;
use codex_utils_redacted_string::RedactedString;
use serde::Deserialize;
use toml::Value;

/// Local enrollment for managed configuration and model requests.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GatewayRequirementsToml {
    name: Option<String>,
    base_url: RedactedString,
    auth: ConfigServerOAuth,
}

impl GatewayRequirementsToml {
    fn into_requirements(self) -> Result<toml::Table, String> {
        let auth = self.auth;
        let mut server = ConfigServer {
            base_url: self.base_url.into_inner(),
            oauth: Some(Box::new(auth.clone())),
        };
        let root = server
            .validated_base_url()
            .map_err(|message| message.replace("config_server", "gateway"))?;
        let root = root.trim_end_matches('/');
        server.base_url = root.to_owned();

        let name = self.name.as_deref().unwrap_or("Gateway").trim();
        // These names select built-in authentication or enable Azure remote compaction.
        if name.is_empty() || uses_builtin_auth(name) || is_azure_provider_name(name) {
            return Err("gateway.name must be a non-empty, non-reserved provider name".into());
        }

        let provider = ModelProviderInfo {
            name: name.to_owned(),
            base_url: Some(format!("{root}/v1")),
            model_catalog_url: Some(format!("{root}/v1/models").into()),
            wire_api: WireApi::Responses,
            requires_openai_auth: false,
            gateway_oauth: Some(GatewayOAuthConfig::OAuth(GatewayOAuthSettings {
                authorization_url: auth.authorization_url,
                token_url: auth.token_url,
                device_authorization_url: auth.device_authorization_url,
                client_id: auth.client_id,
                resource: auth.resource,
                scopes: auth.scopes,
                redirect_port: auth.redirect_port,
                credentials_store: auth.credentials_store,
                delivery: Default::default(),
            })),
            ..Default::default()
        };

        let server = Value::try_from(server).map_err(|err| err.to_string())?;
        let provider = Value::try_from(provider).map_err(|err| err.to_string())?;
        Ok(toml::toml! {
            model_provider = "gateway"
            config_server = (server)
            [model_providers]
            gateway = (provider)
        })
    }
}

pub(super) fn expand(
    layers: &mut [ComposableRequirementsLayer],
) -> Result<(), RequirementsCompositionError> {
    let mut enrollment = None;
    for (index, layer) in layers.iter_mut().enumerate() {
        let Some(gateway) = layer.domain_fields.gateway.take() else {
            continue;
        };
        let requirements =
            gateway
                .into_requirements()
                .map_err(|message| RequirementsCompositionError::Parse {
                    layer_source: layer.source.clone(),
                    message,
                })?;
        enrollment = Some((index, requirements));
    }

    let Some((index, requirements)) = enrollment else {
        return Ok(());
    };
    for layer in layers.iter() {
        let config = &layer.regular_toml;
        let conflict = if config.get("config_server").is_some() {
            "config_server"
        } else if config
            .get("model_provider")
            .is_some_and(|id| id.as_str() != Some("gateway"))
        {
            "model_provider"
        } else if config
            .get("model_providers")
            .and_then(|providers| providers.get("gateway"))
            .is_some()
        {
            "model_providers.gateway"
        } else {
            continue;
        };
        return Err(RequirementsCompositionError::Conflict {
            field: conflict.to_string(),
            existing_source: layers[index].source.clone(),
            incoming_source: layer.source.clone(),
            message: format!("[gateway] supplies {conflict}; remove the separate requirement"),
        });
    }
    crate::merge::merge_toml_values(&mut layers[index].regular_toml, &Value::Table(requirements));
    Ok(())
}
