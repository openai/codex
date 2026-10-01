//! Post-login GovCloud checks use current authentication and freshly loaded configuration.
//! Official endpoint hostnames take precedence over credential and configured AWS regions.
//! Results are advisory; configuration and region resolution failures remain RPC errors.

use super::AccountRequestProcessor;
use crate::error_code::internal_error;
use codex_app_server_protocol::BedrockCheckGovCloudRequirementsResponse;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_config::NetworkDomainPermissionToml;
use codex_model_provider::is_amazon_bedrock_gov_cloud_region;
use codex_model_provider::resolve_amazon_bedrock_region;
use codex_protocol::config_types::ForcedLoginMethod;

impl AccountRequestProcessor {
    pub(crate) async fn bedrock_check_gov_cloud_requirements(
        &self,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let config = self
            .config_manager
            .load_latest_config(/*fallback_cwd*/ None)
            .await
            .map_err(|err| internal_error(format!("failed to load configuration: {err}")))?;
        let provider = &config.model_provider;
        if !provider.is_amazon_bedrock() {
            return Ok(Some(
                BedrockCheckGovCloudRequirementsResponse {
                    is_gov_cloud: false,
                    should_warn: false,
                }
                .into(),
            ));
        }

        let endpoint_domain = provider
            .base_url
            .as_deref()
            .map(|base_url| {
                url::Url::parse(base_url)
                    .ok()
                    .and_then(|url| {
                        url.host_str()
                            .map(|host| host.trim_end_matches('.').to_string())
                    })
                    .ok_or_else(|| internal_error("Amazon Bedrock endpoint has no valid hostname"))
            })
            .transpose()?;
        let endpoint_region = endpoint_domain.as_deref().and_then(|domain| {
            let (service, rest) = domain.split_once('.')?;
            let region = match service {
                "bedrock-mantle" => rest.strip_suffix(".api.aws"),
                "bedrock-runtime" | "bedrock-runtime-fips" => rest.strip_suffix(".amazonaws.com"),
                _ => None,
            }?;
            // Require a single region label, not a substring in a proxy hostname.
            (!region.is_empty() && !region.contains('.')).then_some(region)
        });
        let region = match endpoint_region {
            Some(region) => region.to_string(),
            None => {
                resolve_amazon_bedrock_region(provider.clone(), Some(self.auth_manager.clone()))
                    .await
                    .map_err(|err| {
                        internal_error(format!("failed to resolve Amazon Bedrock region: {err}"))
                    })?
            }
        };
        let is_gov_cloud = is_amazon_bedrock_gov_cloud_region(&region);
        let mut should_warn = false;
        if is_gov_cloud {
            let domain = if let Some(domain) = endpoint_domain {
                domain
            } else if provider.is_amazon_bedrock_runtime() {
                format!("bedrock-runtime.{region}.amazonaws.com")
            } else {
                format!("bedrock-mantle.{region}.api.aws")
            };
            let requirements = config.config_layer_stack.requirements_toml();
            let api_only = requirements
                .allowed_login_methods
                .as_ref()
                .is_some_and(|methods| {
                    !methods.is_empty()
                        && methods
                            .iter()
                            .all(|method| *method == ForcedLoginMethod::Api)
                });
            let network_allowed = requirements
                .application
                .as_ref()
                .and_then(|application| application.network.as_ref())
                .is_some_and(|network| {
                    network.enabled
                        && network.domains.get(&domain) == Some(&NetworkDomainPermissionToml::Allow)
                });
            should_warn = !api_only || !network_allowed;
        }

        Ok(Some(
            BedrockCheckGovCloudRequirementsResponse {
                is_gov_cloud,
                should_warn,
            }
            .into(),
        ))
    }
}
