use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use async_trait::async_trait;
use codex_analytics::AnalyticsEventsClient;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::ConfigRequirements;
use codex_app_server_protocol::ConfigRequirementsReadResponse;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWriteErrorCode;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetParams;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::NetworkDomainPermission;
use codex_app_server_protocol::NetworkRequirements;
use codex_app_server_protocol::NetworkUnixSocketPermission;
use codex_app_server_protocol::SandboxMode;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::config::ConfigService;
use codex_core::config::ConfigServiceError;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::ConfigRequirementsToml;
use codex_core::config_loader::LoaderOverrides;
use codex_core::config_loader::ResidencyRequirement as CoreResidencyRequirement;
use codex_core::config_loader::SandboxModeRequirement as CoreSandboxModeRequirement;
use codex_core::plugins::PluginId;
use codex_core_plugins::loader::installed_plugin_telemetry_metadata;
use codex_core_plugins::toggles::collect_plugin_enabled_candidates;
use codex_features::canonical_feature_for_key;
use codex_features::feature_for_key;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::protocol::Op;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use toml::Value as TomlValue;
use tracing::warn;

const SUPPORTED_EXPERIMENTAL_FEATURE_ENABLEMENT: &[&str] = &[
    "apps",
    "plugins",
    "tool_search",
    "tool_suggest",
    "tool_call_mcp_elicitation",
];

#[async_trait]
pub(crate) trait UserConfigReloader: Send + Sync {
    async fn reload_user_config(&self);
}

#[async_trait]
impl UserConfigReloader for ThreadManager {
    async fn reload_user_config(&self) {
        let thread_ids = self.list_thread_ids().await;
        for thread_id in thread_ids {
            let Ok(thread) = self.get_thread(thread_id).await else {
                continue;
            };
            if let Err(err) = thread.submit(Op::ReloadUserConfig).await {
                warn!("failed to request user config reload: {err}");
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct ConfigApi {
    codex_home: PathBuf,
    cli_overrides: Arc<RwLock<Vec<(String, TomlValue)>>>,
    runtime_feature_enablement: Arc<RwLock<BTreeMap<String, bool>>>,
    loader_overrides: LoaderOverrides,
    cloud_requirements: Arc<RwLock<CloudRequirementsLoader>>,
    user_config_reloader: Arc<dyn UserConfigReloader>,
    analytics_events_client: AnalyticsEventsClient,
}

impl ConfigApi {
    pub(crate) fn new(
        codex_home: PathBuf,
        cli_overrides: Arc<RwLock<Vec<(String, TomlValue)>>>,
        runtime_feature_enablement: Arc<RwLock<BTreeMap<String, bool>>>,
        loader_overrides: LoaderOverrides,
        cloud_requirements: Arc<RwLock<CloudRequirementsLoader>>,
        user_config_reloader: Arc<dyn UserConfigReloader>,
        analytics_events_client: AnalyticsEventsClient,
    ) -> Self {
        Self {
            codex_home,
            cli_overrides,
            runtime_feature_enablement,
            loader_overrides,
            cloud_requirements,
            user_config_reloader,
            analytics_events_client,
        }
    }

    fn config_service(&self) -> ConfigService {
        ConfigService::new(
            self.codex_home.clone(),
            self.current_cli_overrides(),
            self.loader_overrides.clone(),
            self.current_cloud_requirements(),
        )
        .with_requirements_hostname(app_server_requirements_hostname())
    }

    fn current_cli_overrides(&self) -> Vec<(String, TomlValue)> {
        self.cli_overrides
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    fn current_runtime_feature_enablement(&self) -> BTreeMap<String, bool> {
        self.runtime_feature_enablement
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    fn current_cloud_requirements(&self) -> CloudRequirementsLoader {
        self.cloud_requirements
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub(crate) async fn load_latest_config(
        &self,
        fallback_cwd: Option<PathBuf>,
    ) -> Result<Config, JSONRPCErrorError> {
        let mut config = codex_core::config::ConfigBuilder::default()
            .codex_home(self.codex_home.clone())
            .cli_overrides(self.current_cli_overrides())
            .loader_overrides(self.loader_overrides.clone())
            .fallback_cwd(fallback_cwd)
            .cloud_requirements(self.current_cloud_requirements())
            .requirements_hostname(app_server_requirements_hostname())
            .build()
            .await
            .map_err(|err| JSONRPCErrorError {
                code: INTERNAL_ERROR_CODE,
                message: format!("failed to resolve feature override precedence: {err}"),
                data: None,
            })?;
        apply_runtime_feature_enablement(&mut config, &self.current_runtime_feature_enablement());
        Ok(config)
    }

    pub(crate) async fn read(
        &self,
        params: ConfigReadParams,
    ) -> Result<ConfigReadResponse, JSONRPCErrorError> {
        let fallback_cwd = params.cwd.as_ref().map(PathBuf::from);
        let mut response = self
            .config_service()
            .read(params)
            .await
            .map_err(map_error)?;
        let config = self.load_latest_config(fallback_cwd).await?;
        for feature_key in SUPPORTED_EXPERIMENTAL_FEATURE_ENABLEMENT {
            let Some(feature) = feature_for_key(feature_key) else {
                continue;
            };
            let features = response
                .config
                .additional
                .entry("features".to_string())
                .or_insert_with(|| json!({}));
            if !features.is_object() {
                *features = json!({});
            }
            if let Some(features) = features.as_object_mut() {
                features.insert(
                    (*feature_key).to_string(),
                    json!(config.features.enabled(feature)),
                );
            }
        }
        Ok(response)
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
        let pending_changes =
            collect_plugin_enabled_candidates([(&params.key_path, &params.value)].into_iter());
        let response = self
            .config_service()
            .write_value(params)
            .await
            .map_err(map_error)?;
        self.emit_plugin_toggle_events(pending_changes).await;
        Ok(response)
    }

    pub(crate) async fn batch_write(
        &self,
        params: ConfigBatchWriteParams,
    ) -> Result<ConfigWriteResponse, JSONRPCErrorError> {
        let reload_user_config = params.reload_user_config;
        let pending_changes = collect_plugin_enabled_candidates(
            params
                .edits
                .iter()
                .map(|edit| (&edit.key_path, &edit.value)),
        );
        let response = self
            .config_service()
            .batch_write(params)
            .await
            .map_err(map_error)?;
        self.emit_plugin_toggle_events(pending_changes).await;
        if reload_user_config {
            self.user_config_reloader.reload_user_config().await;
        }
        Ok(response)
    }

    pub(crate) async fn set_experimental_feature_enablement(
        &self,
        params: ExperimentalFeatureEnablementSetParams,
    ) -> Result<ExperimentalFeatureEnablementSetResponse, JSONRPCErrorError> {
        let ExperimentalFeatureEnablementSetParams { enablement } = params;
        for key in enablement.keys() {
            if canonical_feature_for_key(key).is_some() {
                if SUPPORTED_EXPERIMENTAL_FEATURE_ENABLEMENT.contains(&key.as_str()) {
                    continue;
                }

                return Err(JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!(
                        "unsupported feature enablement `{key}`: currently supported features are {}",
                        SUPPORTED_EXPERIMENTAL_FEATURE_ENABLEMENT.join(", ")
                    ),
                    data: None,
                });
            }

            let message = if let Some(feature) = feature_for_key(key) {
                format!(
                    "invalid feature enablement `{key}`: use canonical feature key `{}`",
                    feature.key()
                )
            } else {
                format!("invalid feature enablement `{key}`")
            };
            return Err(JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message,
                data: None,
            });
        }

        if enablement.is_empty() {
            return Ok(ExperimentalFeatureEnablementSetResponse { enablement });
        }

        {
            let mut runtime_feature_enablement =
                self.runtime_feature_enablement
                    .write()
                    .map_err(|_| JSONRPCErrorError {
                        code: INTERNAL_ERROR_CODE,
                        message: "failed to update feature enablement".to_string(),
                        data: None,
                    })?;
            runtime_feature_enablement.extend(
                enablement
                    .iter()
                    .map(|(name, enabled)| (name.clone(), *enabled)),
            );
        }

        self.load_latest_config(/*fallback_cwd*/ None).await?;
        self.user_config_reloader.reload_user_config().await;

        Ok(ExperimentalFeatureEnablementSetResponse { enablement })
    }

    async fn emit_plugin_toggle_events(
        &self,
        pending_changes: std::collections::BTreeMap<String, bool>,
    ) {
        for (plugin_id, enabled) in pending_changes {
            let Ok(plugin_id) = PluginId::parse(&plugin_id) else {
                continue;
            };
            let metadata =
                installed_plugin_telemetry_metadata(self.codex_home.as_path(), &plugin_id).await;
            if enabled {
                self.analytics_events_client.track_plugin_enabled(metadata);
            } else {
                self.analytics_events_client.track_plugin_disabled(metadata);
            }
        }
    }
}

pub(crate) fn protected_feature_keys(
    config_layer_stack: &codex_core::config_loader::ConfigLayerStack,
) -> BTreeSet<String> {
    let mut protected_features = config_layer_stack
        .effective_config()
        .get("features")
        .and_then(toml::Value::as_table)
        .map(|features| features.keys().cloned().collect::<BTreeSet<_>>())
        .unwrap_or_default();

    if let Some(feature_requirements) = config_layer_stack
        .requirements_toml()
        .feature_requirements
        .as_ref()
    {
        protected_features.extend(feature_requirements.entries.keys().cloned());
    }

    protected_features
}

pub(crate) fn apply_runtime_feature_enablement(
    config: &mut Config,
    runtime_feature_enablement: &BTreeMap<String, bool>,
) {
    let protected_features = protected_feature_keys(&config.config_layer_stack);
    for (name, enabled) in runtime_feature_enablement {
        if protected_features.contains(name) {
            continue;
        }
        let Some(feature) = feature_for_key(name) else {
            continue;
        };
        if let Err(err) = config.features.set_enabled(feature, *enabled) {
            warn!(
                feature = name,
                error = %err,
                "failed to apply runtime feature enablement"
            );
        }
    }
}

fn app_server_requirements_hostname() -> Option<String> {
    let hostname = gethostname::gethostname();
    let hostname = hostname.to_string_lossy().trim().to_string();
    (!hostname.is_empty()).then_some(hostname)
}

fn map_requirements_toml_to_api(requirements: ConfigRequirementsToml) -> ConfigRequirements {
    ConfigRequirements {
        allowed_approval_policies: requirements.allowed_approval_policies.map(|policies| {
            policies
                .into_iter()
                .map(codex_app_server_protocol::AskForApproval::from)
                .collect()
        }),
        allowed_approvals_reviewers: requirements.allowed_approvals_reviewers.map(|reviewers| {
            reviewers
                .into_iter()
                .map(codex_app_server_protocol::ApprovalsReviewer::from)
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
        feature_requirements: requirements
            .feature_requirements
            .map(|requirements| requirements.entries),
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
    let allowed_domains = network
        .domains
        .as_ref()
        .and_then(codex_core::config_loader::NetworkDomainPermissionsToml::allowed_domains);
    let denied_domains = network
        .domains
        .as_ref()
        .and_then(codex_core::config_loader::NetworkDomainPermissionsToml::denied_domains);
    let allow_unix_sockets = network
        .unix_sockets
        .as_ref()
        .map(codex_core::config_loader::NetworkUnixSocketPermissionsToml::allow_unix_sockets)
        .filter(|entries| !entries.is_empty());

    NetworkRequirements {
        enabled: network.enabled,
        http_port: network.http_port,
        socks_port: network.socks_port,
        allow_upstream_proxy: network.allow_upstream_proxy,
        dangerously_allow_non_loopback_proxy: network.dangerously_allow_non_loopback_proxy,
        dangerously_allow_all_unix_sockets: network.dangerously_allow_all_unix_sockets,
        domains: network.domains.map(|domains| {
            domains
                .entries
                .into_iter()
                .map(|(pattern, permission)| {
                    (pattern, map_network_domain_permission_to_api(permission))
                })
                .collect()
        }),
        managed_allowed_domains_only: network.managed_allowed_domains_only,
        allowed_domains,
        denied_domains,
        unix_sockets: network.unix_sockets.map(|unix_sockets| {
            unix_sockets
                .entries
                .into_iter()
                .map(|(path, permission)| {
                    (path, map_network_unix_socket_permission_to_api(permission))
                })
                .collect()
        }),
        allow_unix_sockets,
        allow_local_binding: network.allow_local_binding,
    }
}

fn map_network_domain_permission_to_api(
    permission: codex_core::config_loader::NetworkDomainPermissionToml,
) -> NetworkDomainPermission {
    match permission {
        codex_core::config_loader::NetworkDomainPermissionToml::Allow => {
            NetworkDomainPermission::Allow
        }
        codex_core::config_loader::NetworkDomainPermissionToml::Deny => {
            NetworkDomainPermission::Deny
        }
    }
}

fn map_network_unix_socket_permission_to_api(
    permission: codex_core::config_loader::NetworkUnixSocketPermissionToml,
) -> NetworkUnixSocketPermission {
    match permission {
        codex_core::config_loader::NetworkUnixSocketPermissionToml::Allow => {
            NetworkUnixSocketPermission::Allow
        }
        codex_core::config_loader::NetworkUnixSocketPermissionToml::None => {
            NetworkUnixSocketPermission::None
        }
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
    use codex_analytics::AnalyticsEventsClient;
    use codex_core::config_loader::NetworkDomainPermissionToml as CoreNetworkDomainPermissionToml;
    use codex_core::config_loader::NetworkDomainPermissionsToml as CoreNetworkDomainPermissionsToml;
    use codex_core::config_loader::NetworkRequirementsToml as CoreNetworkRequirementsToml;
    use codex_core::config_loader::NetworkUnixSocketPermissionToml as CoreNetworkUnixSocketPermissionToml;
    use codex_core::config_loader::NetworkUnixSocketPermissionsToml as CoreNetworkUnixSocketPermissionsToml;
    use codex_features::Feature;
    use codex_login::AuthManager;
    use codex_login::CodexAuth;
    use codex_protocol::config_types::ApprovalsReviewer as CoreApprovalsReviewer;
    use codex_protocol::protocol::AskForApproval as CoreAskForApproval;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use tempfile::TempDir;

    #[derive(Default)]
    struct RecordingUserConfigReloader {
        call_count: AtomicUsize,
    }

    #[async_trait]
    impl UserConfigReloader for RecordingUserConfigReloader {
        async fn reload_user_config(&self) {
            self.call_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn map_requirements_toml_to_api_converts_core_enums() {
        let requirements = ConfigRequirementsToml {
            allowed_approval_policies: Some(vec![
                CoreAskForApproval::Never,
                CoreAskForApproval::OnRequest,
            ]),
            allowed_approvals_reviewers: Some(vec![
                CoreApprovalsReviewer::User,
                CoreApprovalsReviewer::GuardianSubagent,
            ]),
            allowed_sandbox_modes: Some(vec![
                CoreSandboxModeRequirement::ReadOnly,
                CoreSandboxModeRequirement::ExternalSandbox,
            ]),
            remote_sandbox_config: None,
            allowed_web_search_modes: Some(vec![
                codex_core::config_loader::WebSearchModeRequirement::Cached,
            ]),
            guardian_policy_config: None,
            feature_requirements: Some(codex_core::config_loader::FeatureRequirementsToml {
                entries: std::collections::BTreeMap::from([
                    ("apps".to_string(), false),
                    ("personality".to_string(), true),
                ]),
            }),
            mcp_servers: None,
            apps: None,
            rules: None,
            enforce_residency: Some(CoreResidencyRequirement::Us),
            network: Some(CoreNetworkRequirementsToml {
                enabled: Some(true),
                http_port: Some(8080),
                socks_port: Some(1080),
                allow_upstream_proxy: Some(false),
                dangerously_allow_non_loopback_proxy: Some(false),
                dangerously_allow_all_unix_sockets: Some(true),
                domains: Some(CoreNetworkDomainPermissionsToml {
                    entries: std::collections::BTreeMap::from([
                        (
                            "api.openai.com".to_string(),
                            CoreNetworkDomainPermissionToml::Allow,
                        ),
                        (
                            "example.com".to_string(),
                            CoreNetworkDomainPermissionToml::Deny,
                        ),
                    ]),
                }),
                managed_allowed_domains_only: Some(false),
                unix_sockets: Some(CoreNetworkUnixSocketPermissionsToml {
                    entries: std::collections::BTreeMap::from([(
                        "/tmp/proxy.sock".to_string(),
                        CoreNetworkUnixSocketPermissionToml::Allow,
                    )]),
                }),
                allow_local_binding: Some(true),
            }),
            permissions: None,
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
            mapped.allowed_approvals_reviewers,
            Some(vec![
                codex_app_server_protocol::ApprovalsReviewer::User,
                codex_app_server_protocol::ApprovalsReviewer::GuardianSubagent,
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
            mapped.feature_requirements,
            Some(std::collections::BTreeMap::from([
                ("apps".to_string(), false),
                ("personality".to_string(), true),
            ])),
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
                dangerously_allow_all_unix_sockets: Some(true),
                domains: Some(std::collections::BTreeMap::from([
                    ("api.openai.com".to_string(), NetworkDomainPermission::Allow,),
                    ("example.com".to_string(), NetworkDomainPermission::Deny),
                ])),
                managed_allowed_domains_only: Some(false),
                allowed_domains: Some(vec!["api.openai.com".to_string()]),
                denied_domains: Some(vec!["example.com".to_string()]),
                unix_sockets: Some(std::collections::BTreeMap::from([(
                    "/tmp/proxy.sock".to_string(),
                    NetworkUnixSocketPermission::Allow,
                )])),
                allow_unix_sockets: Some(vec!["/tmp/proxy.sock".to_string()]),
                allow_local_binding: Some(true),
            }),
        );
    }

    #[test]
    fn map_requirements_toml_to_api_omits_unix_socket_none_entries_from_legacy_network_fields() {
        let requirements = ConfigRequirementsToml {
            allowed_approval_policies: None,
            allowed_approvals_reviewers: None,
            allowed_sandbox_modes: None,
            remote_sandbox_config: None,
            allowed_web_search_modes: None,
            guardian_policy_config: None,
            feature_requirements: None,
            mcp_servers: None,
            apps: None,
            rules: None,
            enforce_residency: None,
            network: Some(CoreNetworkRequirementsToml {
                enabled: None,
                http_port: None,
                socks_port: None,
                allow_upstream_proxy: None,
                dangerously_allow_non_loopback_proxy: None,
                dangerously_allow_all_unix_sockets: None,
                domains: None,
                managed_allowed_domains_only: None,
                unix_sockets: Some(CoreNetworkUnixSocketPermissionsToml {
                    entries: std::collections::BTreeMap::from([(
                        "/tmp/ignored.sock".to_string(),
                        CoreNetworkUnixSocketPermissionToml::None,
                    )]),
                }),
                allow_local_binding: None,
            }),
            permissions: None,
        };

        let mapped = map_requirements_toml_to_api(requirements);

        assert_eq!(
            mapped.network,
            Some(NetworkRequirements {
                enabled: None,
                http_port: None,
                socks_port: None,
                allow_upstream_proxy: None,
                dangerously_allow_non_loopback_proxy: None,
                dangerously_allow_all_unix_sockets: None,
                domains: None,
                managed_allowed_domains_only: None,
                allowed_domains: None,
                denied_domains: None,
                unix_sockets: Some(std::collections::BTreeMap::from([(
                    "/tmp/ignored.sock".to_string(),
                    NetworkUnixSocketPermission::None,
                )])),
                allow_unix_sockets: None,
                allow_local_binding: None,
            }),
        );
    }

    #[test]
    fn map_requirements_toml_to_api_normalizes_allowed_web_search_modes() {
        let requirements = ConfigRequirementsToml {
            allowed_approval_policies: None,
            allowed_approvals_reviewers: None,
            allowed_sandbox_modes: None,
            remote_sandbox_config: None,
            allowed_web_search_modes: Some(Vec::new()),
            guardian_policy_config: None,
            feature_requirements: None,
            mcp_servers: None,
            apps: None,
            rules: None,
            enforce_residency: None,
            network: None,
            permissions: None,
        };

        let mapped = map_requirements_toml_to_api(requirements);

        assert_eq!(
            mapped.allowed_web_search_modes,
            Some(vec![WebSearchMode::Disabled])
        );
    }

    #[tokio::test]
    async fn apply_runtime_feature_enablement_keeps_cli_overrides_above_config_and_runtime() {
        let codex_home = TempDir::new().expect("create temp dir");
        std::fs::write(
            codex_home.path().join("config.toml"),
            "[features]\napps = false\n",
        )
        .expect("write config");

        let mut config = codex_core::config::ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .fallback_cwd(Some(codex_home.path().to_path_buf()))
            .cli_overrides(vec![(
                "features.apps".to_string(),
                TomlValue::Boolean(true),
            )])
            .build()
            .await
            .expect("load config");

        apply_runtime_feature_enablement(
            &mut config,
            &BTreeMap::from([("apps".to_string(), false)]),
        );

        assert!(config.features.enabled(Feature::Apps));
    }

    #[tokio::test]
    async fn apply_runtime_feature_enablement_keeps_cloud_pins_above_cli_and_runtime() {
        let codex_home = TempDir::new().expect("create temp dir");

        let mut config = codex_core::config::ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .cli_overrides(vec![(
                "features.apps".to_string(),
                TomlValue::Boolean(true),
            )])
            .cloud_requirements(CloudRequirementsLoader::new(async {
                Ok(Some(ConfigRequirementsToml {
                    feature_requirements: Some(
                        codex_core::config_loader::FeatureRequirementsToml {
                            entries: BTreeMap::from([("apps".to_string(), false)]),
                        },
                    ),
                    ..Default::default()
                }))
            }))
            .build()
            .await
            .expect("load config");

        apply_runtime_feature_enablement(
            &mut config,
            &BTreeMap::from([("apps".to_string(), true)]),
        );

        assert!(!config.features.enabled(Feature::Apps));
    }

    #[tokio::test]
    async fn batch_write_reloads_user_config_when_requested() {
        let codex_home = TempDir::new().expect("create temp dir");
        let user_config_path = codex_home.path().join("config.toml");
        std::fs::write(&user_config_path, "").expect("write config");
        let reloader = Arc::new(RecordingUserConfigReloader::default());
        let analytics_config = Arc::new(
            codex_core::config::ConfigBuilder::default()
                .build()
                .await
                .expect("load analytics config"),
        );
        let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test"));
        let config_api = ConfigApi::new(
            codex_home.path().to_path_buf(),
            Arc::new(RwLock::new(Vec::new())),
            Arc::new(RwLock::new(BTreeMap::new())),
            LoaderOverrides::default(),
            Arc::new(RwLock::new(CloudRequirementsLoader::default())),
            reloader.clone(),
            AnalyticsEventsClient::new(
                auth_manager,
                analytics_config
                    .chatgpt_base_url
                    .trim_end_matches('/')
                    .to_string(),
                analytics_config.analytics_enabled,
            ),
        );

        let response = config_api
            .batch_write(ConfigBatchWriteParams {
                edits: vec![codex_app_server_protocol::ConfigEdit {
                    key_path: "model".to_string(),
                    value: json!("gpt-5"),
                    merge_strategy: codex_app_server_protocol::MergeStrategy::Replace,
                }],
                file_path: Some(user_config_path.display().to_string()),
                expected_version: None,
                reload_user_config: true,
            })
            .await
            .expect("batch write should succeed");

        assert_eq!(
            response,
            ConfigWriteResponse {
                status: codex_app_server_protocol::WriteStatus::Ok,
                version: response.version.clone(),
                file_path: codex_utils_absolute_path::AbsolutePathBuf::try_from(
                    user_config_path.clone()
                )
                .expect("absolute config path"),
                overridden_metadata: None,
            }
        );
        assert_eq!(
            std::fs::read_to_string(user_config_path).unwrap(),
            "model = \"gpt-5\"\n"
        );
        assert_eq!(reloader.call_count.load(Ordering::Relaxed), 1);
    }
}
