use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::ConfigRequirements;
use codex_app_server_protocol::ConfigRequirementsReadResponse;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWriteErrorCode;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::NetworkRequirements;
use codex_app_server_protocol::SandboxMode;
use codex_core::config::ConfigService;
use codex_core::config::ConfigServiceError;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::ConfigRequirementsToml;
use codex_core::config_loader::LoaderOverrides;
use codex_core::config_loader::ResidencyRequirement as CoreResidencyRequirement;
use codex_core::config_loader::SandboxModeRequirement as CoreSandboxModeRequirement;
use codex_protocol::config_types::WebSearchMode;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use toml::Value as TomlValue;

#[derive(Clone)]
pub(crate) struct ConfigApi {
    codex_home: PathBuf,
    cli_overrides: Vec<(String, TomlValue)>,
    loader_overrides: LoaderOverrides,
    cloud_requirements: Arc<RwLock<CloudRequirementsLoader>>,
}

impl ConfigApi {
    pub(crate) fn new(
        codex_home: PathBuf,
        cli_overrides: Vec<(String, TomlValue)>,
        loader_overrides: LoaderOverrides,
        cloud_requirements: Arc<RwLock<CloudRequirementsLoader>>,
    ) -> Self {
        Self {
            codex_home,
            cli_overrides,
            loader_overrides,
            cloud_requirements,
        }
    }

    fn config_service(&self) -> ConfigService {
        let cloud_requirements = self
            .cloud_requirements
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        ConfigService::new(
            self.codex_home.clone(),
            self.cli_overrides.clone(),
            self.loader_overrides.clone(),
            cloud_requirements,
        )
    }

    pub(crate) async fn read(
        &self,
        params: ConfigReadParams,
    ) -> Result<ConfigReadResponse, JSONRPCErrorError> {
        self.config_service().read(params).await.map_err(map_error)
    }

    pub(crate) async fn config_requirements_read(
        &self,
    ) -> Result<ConfigRequirementsReadResponse, JSONRPCErrorError> {
        let requirements = self
            .config_service()
            .read_requirements()
            .await
            .map_err(map_error)?
            .map(map_requirements_toml_to_api);

        Ok(ConfigRequirementsReadResponse { requirements })
    }

    pub(crate) async fn write_value(
        &self,
        params: ConfigValueWriteParams,
    ) -> Result<ConfigWriteResponse, JSONRPCErrorError> {
        self.config_service()
            .write_value(params)
            .await
            .map_err(map_error)
    }

    pub(crate) async fn batch_write(
        &self,
        params: ConfigBatchWriteParams,
    ) -> Result<ConfigWriteResponse, JSONRPCErrorError> {
        self.config_service()
            .batch_write(params)
            .await
            .map_err(map_error)
    }
}

fn map_requirements_toml_to_api(requirements: ConfigRequirementsToml) -> ConfigRequirements {
    ConfigRequirements {
        allowed_approval_policies: requirements.allowed_approval_policies.map(|policies| {
            policies
                .into_iter()
                .map(codex_app_server_protocol::AskForApproval::from)
                .collect()
        }),
        allowed_sandbox_modes: requirements.allowed_sandbox_modes.map(|modes| {
            modes
                .into_iter()
                .filter_map(map_sandbox_mode_requirement_to_api)
                .collect()
        }),
        allowed_web_search_modes: requirements.allowed_web_search_modes.map(|modes| {
            let mut normalized = modes
                .into_iter()
                .map(Into::into)
                .collect::<Vec<WebSearchMode>>();
            if !normalized.contains(&WebSearchMode::Disabled) {
                normalized.push(WebSearchMode::Disabled);
            }
            normalized
        }),
        enforce_residency: requirements
            .enforce_residency
            .map(map_residency_requirement_to_api),
        network: requirements.network.map(map_network_requirements_to_api),
    }
}

fn map_sandbox_mode_requirement_to_api(mode: CoreSandboxModeRequirement) -> Option<SandboxMode> {
    match mode {
        CoreSandboxModeRequirement::ReadOnly => Some(SandboxMode::ReadOnly),
        CoreSandboxModeRequirement::WorkspaceWrite => Some(SandboxMode::WorkspaceWrite),
        CoreSandboxModeRequirement::DangerFullAccess => Some(SandboxMode::DangerFullAccess),
        CoreSandboxModeRequirement::ExternalSandbox => None,
    }
}

fn map_residency_requirement_to_api(
    residency: CoreResidencyRequirement,
) -> codex_app_server_protocol::ResidencyRequirement {
    match residency {
        CoreResidencyRequirement::Us => codex_app_server_protocol::ResidencyRequirement::Us,
    }
}

fn map_network_requirements_to_api(
    network: codex_core::config_loader::NetworkRequirementsToml,
) -> NetworkRequirements {
    NetworkRequirements {
        enabled: network.enabled,
        http_port: network.http_port,
        socks_port: network.socks_port,
        allow_upstream_proxy: network.allow_upstream_proxy,
        dangerously_allow_non_loopback_proxy: network.dangerously_allow_non_loopback_proxy,
        dangerously_allow_non_loopback_admin: network.dangerously_allow_non_loopback_admin,
        allowed_domains: network.allowed_domains,
        denied_domains: network.denied_domains,
        allow_unix_sockets: network.allow_unix_sockets,
        allow_local_binding: network.allow_local_binding,
    }
}

fn map_error(err: ConfigServiceError) -> JSONRPCErrorError {
    if let Some(code) = err.write_error_code() {
        return config_write_error(code, err.to_string());
    }

    JSONRPCErrorError {
        code: INTERNAL_ERROR_CODE,
        message: err.to_string(),
        data: None,
    }
}

fn config_write_error(code: ConfigWriteErrorCode, message: impl Into<String>) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: INVALID_REQUEST_ERROR_CODE,
        message: message.into(),
        data: Some(json!({
            "config_write_error_code": code,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::config_loader::NetworkRequirementsToml as CoreNetworkRequirementsToml;
    use codex_protocol::protocol::AskForApproval as CoreAskForApproval;
    use pretty_assertions::assert_eq;

    #[test]
    fn map_requirements_toml_to_api_converts_core_enums() {
        let requirements = ConfigRequirementsToml {
            allowed_approval_policies: Some(vec![
                CoreAskForApproval::Never,
                CoreAskForApproval::OnRequest,
            ]),
            allowed_sandbox_modes: Some(vec![
                CoreSandboxModeRequirement::ReadOnly,
                CoreSandboxModeRequirement::ExternalSandbox,
            ]),
            allowed_web_search_modes: Some(vec![
                codex_core::config_loader::WebSearchModeRequirement::Cached,
            ]),
            mcp_servers: None,
            rules: None,
            enforce_residency: Some(CoreResidencyRequirement::Us),
            network: Some(CoreNetworkRequirementsToml {
                enabled: Some(true),
                http_port: Some(8080),
                socks_port: Some(1080),
                allow_upstream_proxy: Some(false),
                dangerously_allow_non_loopback_proxy: Some(false),
                dangerously_allow_non_loopback_admin: Some(false),
                allowed_domains: Some(vec!["api.openai.com".to_string()]),
                denied_domains: Some(vec!["example.com".to_string()]),
                allow_unix_sockets: Some(vec!["/tmp/proxy.sock".to_string()]),
                allow_local_binding: Some(true),
            }),
        };

        let mapped = map_requirements_toml_to_api(requirements);

        assert_eq!(
            mapped.allowed_approval_policies,
            Some(vec![
                codex_app_server_protocol::AskForApproval::Never,
                codex_app_server_protocol::AskForApproval::OnRequest,
            ])
        );
        assert_eq!(
            mapped.allowed_sandbox_modes,
            Some(vec![SandboxMode::ReadOnly]),
        );
        assert_eq!(
            mapped.allowed_web_search_modes,
            Some(vec![WebSearchMode::Cached, WebSearchMode::Disabled]),
        );
        assert_eq!(
            mapped.enforce_residency,
            Some(codex_app_server_protocol::ResidencyRequirement::Us),
        );
        assert_eq!(
            mapped.network,
            Some(NetworkRequirements {
                enabled: Some(true),
                http_port: Some(8080),
                socks_port: Some(1080),
                allow_upstream_proxy: Some(false),
                dangerously_allow_non_loopback_proxy: Some(false),
                dangerously_allow_non_loopback_admin: Some(false),
                allowed_domains: Some(vec!["api.openai.com".to_string()]),
                denied_domains: Some(vec!["example.com".to_string()]),
                allow_unix_sockets: Some(vec!["/tmp/proxy.sock".to_string()]),
                allow_local_binding: Some(true),
            }),
        );
    }

    #[test]
    fn map_requirements_toml_to_api_normalizes_allowed_web_search_modes() {
        let requirements = ConfigRequirementsToml {
            allowed_approval_policies: None,
            allowed_sandbox_modes: None,
            allowed_web_search_modes: Some(Vec::new()),
            mcp_servers: None,
            rules: None,
            enforce_residency: None,
            network: None,
        };

        let mapped = map_requirements_toml_to_api(requirements);

        assert_eq!(
            mapped.allowed_web_search_modes,
            Some(vec![WebSearchMode::Disabled])
        );
    }
}
