use std::sync::Arc;

use crate::config_manager::ConfigManager;
use crate::config_manager_service::ConfigManagerError;
use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::outgoing_message::ConnectionRequestId;
use crate::outgoing_message::OutgoingMessageSender;
use crate::transport::RemoteControlHandle;
use codex_analytics::AnalyticsEventsClient;
use codex_app_server_protocol::AppListUpdatedNotification;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::ConfigRequirements;
use codex_app_server_protocol::ConfigRequirementsReadResponse;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWriteErrorCode;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::ConfiguredHookHandler;
use codex_app_server_protocol::ConfiguredHookMatcherGroup;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetParams;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ManagedHooksRequirements;
use codex_app_server_protocol::ModelProviderCapabilitiesReadResponse;
use codex_app_server_protocol::NetworkDomainPermission;
use codex_app_server_protocol::NetworkRequirements;
use codex_app_server_protocol::NetworkUnixSocketPermission;
use codex_app_server_protocol::SandboxMode;
use codex_app_server_protocol::ServerNotification;
use codex_chatgpt::connectors;
use codex_config::ConfigRequirementsToml;
use codex_config::HookEventsToml;
use codex_config::HookHandlerConfig as CoreHookHandlerConfig;
use codex_config::ManagedHooksRequirementsToml;
use codex_config::MatcherGroup as CoreMatcherGroup;
use codex_config::ResidencyRequirement as CoreResidencyRequirement;
use codex_config::SandboxModeRequirement as CoreSandboxModeRequirement;
use codex_core::ThreadManager;
use codex_features::Feature;
use codex_features::canonical_feature_for_key;
use codex_features::feature_for_key;
use codex_login::AuthManager;
use codex_model_provider::create_model_provider;
use codex_plugin::PluginId;
use codex_protocol::config_types::WebSearchMode;
use serde_json::json;
use std::path::PathBuf;

const SUPPORTED_EXPERIMENTAL_FEATURE_ENABLEMENT: &[&str] = &[
    "apps",
    "memories",
    "mentions_v2",
    "plugins",
    "remote_control",
    "tool_search",
    "tool_suggest",
    "tool_call_mcp_elicitation",
];

#[derive(Clone)]
pub(crate) struct ConfigRequestProcessor {
    outgoing: Arc<OutgoingMessageSender>,
    config_manager: ConfigManager,
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
    analytics_events_client: AnalyticsEventsClient,
    remote_control_handle: Option<RemoteControlHandle>,
}

impl ConfigRequestProcessor {
    pub(crate) fn new(
        outgoing: Arc<OutgoingMessageSender>,
        config_manager: ConfigManager,
        auth_manager: Arc<AuthManager>,
        thread_manager: Arc<ThreadManager>,
        analytics_events_client: AnalyticsEventsClient,
        remote_control_handle: Option<RemoteControlHandle>,
    ) -> Self {
        Self {
            outgoing,
            config_manager,
            auth_manager,
            thread_manager,
            analytics_events_client,
            remote_control_handle,
        }
    }

    pub(crate) async fn read(
        &self,
        params: ConfigReadParams,
    ) -> Result<ConfigReadResponse, JSONRPCErrorError> {
        let fallback_cwd = params.cwd.as_ref().map(PathBuf::from);
        let mut response = self.config_manager.read(params).await.map_err(map_error)?;
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
            .config_manager
            .read_requirements()
            .await
            .map_err(map_error)?
            .map(map_requirements_toml_to_api);

        Ok(ConfigRequirementsReadResponse { requirements })
    }

    pub(crate) async fn value_write(
        &self,
        params: ConfigValueWriteParams,
    ) -> Result<ClientResponsePayload, JSONRPCErrorError> {
        self.handle_config_mutation_result(self.write_value(params).await)
            .await
            .map(ClientResponsePayload::ConfigValueWrite)
    }

    pub(crate) async fn batch_write(
        &self,
        params: ConfigBatchWriteParams,
    ) -> Result<ClientResponsePayload, JSONRPCErrorError> {
        self.handle_config_mutation_result(self.batch_write_inner(params).await)
            .await
            .map(ClientResponsePayload::ConfigBatchWrite)
    }

    pub(crate) async fn experimental_feature_enablement_set(
        &self,
        request_id: ConnectionRequestId,
        params: ExperimentalFeatureEnablementSetParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let should_refresh_apps_list = params.enablement.get("apps").copied() == Some(true);
        let response = self
            .handle_config_mutation_result(self.set_experimental_feature_enablement(params).await)
            .await?;
        self.outgoing
            .send_response_as(
                request_id,
                ClientResponsePayload::ExperimentalFeatureEnablementSet(response),
            )
            .await;
        if should_refresh_apps_list {
            self.refresh_apps_list_after_experimental_feature_enablement_set()
                .await;
        }
        Ok(None)
    }

    pub(crate) async fn model_provider_capabilities_read(
        &self,
    ) -> Result<ModelProviderCapabilitiesReadResponse, JSONRPCErrorError> {
        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        let provider = create_model_provider(config.model_provider, /*auth_manager*/ None);
        let capabilities = provider.capabilities();
        Ok(ModelProviderCapabilitiesReadResponse {
            namespace_tools: capabilities.namespace_tools,
            image_generation: capabilities.image_generation,
            web_search: capabilities.web_search,
        })
    }

    pub(crate) async fn handle_config_mutation(&self) {
        self.thread_manager.plugins_manager().clear_cache();
        self.thread_manager.skills_manager().clear_cache();
        let Some(remote_control_handle) = &self.remote_control_handle else {
            return;
        };

        match self.load_latest_config(/*fallback_cwd*/ None).await {
            Ok(config) => {
                remote_control_handle.set_enabled(config.features.enabled(Feature::RemoteControl));
            }
            Err(error) => {
                tracing::warn!(
                    "failed to load config for remote control enablement refresh after config mutation: {}",
                    error.message
                );
            }
        }
    }

    async fn handle_config_mutation_result<T>(
        &self,
        result: std::result::Result<T, JSONRPCErrorError>,
    ) -> Result<T, JSONRPCErrorError> {
        let response = result?;
        self.handle_config_mutation().await;
        Ok(response)
    }

    async fn refresh_apps_list_after_experimental_feature_enablement_set(&self) {
        let config = match self.load_latest_config(/*fallback_cwd*/ None).await {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(
                    "failed to load config for apps list refresh after experimental feature enablement: {}",
                    error.message
                );
                return;
            }
        };
        let auth = self.auth_manager.auth().await;
        if !config.features.apps_enabled_for_auth(
            auth.as_ref()
                .is_some_and(codex_login::CodexAuth::uses_codex_backend),
        ) {
            return;
        }

        let outgoing = Arc::clone(&self.outgoing);
        let environment_manager = self.thread_manager.environment_manager();
        tokio::spawn(async move {
            let (all_connectors_result, accessible_connectors_result) = tokio::join!(
                connectors::list_all_connectors_with_options(&config, /*force_refetch*/ true),
                connectors::list_accessible_connectors_from_mcp_tools_with_environment_manager(
                    &config,
                    /*force_refetch*/ true,
                    &environment_manager,
                ),
            );
            let all_connectors = match all_connectors_result {
                Ok(connectors) => connectors,
                Err(err) => {
                    tracing::warn!(
                        "failed to force-refresh directory apps after experimental feature enablement: {err:#}"
                    );
                    return;
                }
            };
            let accessible_connectors = match accessible_connectors_result {
                Ok(status) => status.connectors,
                Err(err) => {
                    tracing::warn!(
                        "failed to force-refresh accessible apps after experimental feature enablement: {err:#}"
                    );
                    return;
                }
            };

            let data = connectors::with_app_enabled_state(
                connectors::merge_connectors_with_accessible(
                    all_connectors,
                    accessible_connectors,
                    /*all_connectors_loaded*/ true,
                ),
                &config,
            );
            outgoing
                .send_server_notification(ServerNotification::AppListUpdated(
                    AppListUpdatedNotification { data },
                ))
                .await;
        });
    }

    async fn load_latest_config(
        &self,
        fallback_cwd: Option<PathBuf>,
    ) -> Result<codex_core::config::Config, JSONRPCErrorError> {
        self.config_manager
            .load_latest_config(fallback_cwd)
            .await
            .map_err(|err| {
                internal_error(format!(
                    "failed to resolve feature override precedence: {err}"
                ))
            })
    }

    async fn write_value(
        &self,
        params: ConfigValueWriteParams,
    ) -> Result<ConfigWriteResponse, JSONRPCErrorError> {
        let pending_changes = codex_core_plugins::toggles::collect_plugin_enabled_candidates(
            [(&params.key_path, &params.value)].into_iter(),
        );
        let response = self
            .config_manager
            .write_value(params)
            .await
            .map_err(map_error)?;
        self.emit_plugin_toggle_events(pending_changes).await;
        Ok(response)
    }

    async fn batch_write_inner(
        &self,
        params: ConfigBatchWriteParams,
    ) -> Result<ConfigWriteResponse, JSONRPCErrorError> {
        let reload_user_config = params.reload_user_config;
        let pending_changes = codex_core_plugins::toggles::collect_plugin_enabled_candidates(
            params
                .edits
                .iter()
                .map(|edit| (&edit.key_path, &edit.value)),
        );
        let response = self
            .config_manager
            .batch_write(params)
            .await
            .map_err(map_error)?;
        self.emit_plugin_toggle_events(pending_changes).await;
        if reload_user_config {
            self.reload_user_config().await;
        }
        Ok(response)
    }

    async fn set_experimental_feature_enablement(
        &self,
        params: ExperimentalFeatureEnablementSetParams,
    ) -> Result<ExperimentalFeatureEnablementSetResponse, JSONRPCErrorError> {
        let ExperimentalFeatureEnablementSetParams { enablement } = params;
        for key in enablement.keys() {
            if canonical_feature_for_key(key).is_some() {
                if SUPPORTED_EXPERIMENTAL_FEATURE_ENABLEMENT.contains(&key.as_str()) {
                    continue;
                }

                return Err(invalid_request(format!(
                    "unsupported feature enablement `{key}`: currently supported features are {}",
                    SUPPORTED_EXPERIMENTAL_FEATURE_ENABLEMENT.join(", ")
                )));
            }

            let message = if let Some(feature) = feature_for_key(key) {
                format!(
                    "invalid feature enablement `{key}`: use canonical feature key `{}`",
                    feature.key()
                )
            } else {
                format!("invalid feature enablement `{key}`")
            };
            return Err(invalid_request(message));
        }

        if enablement.is_empty() {
            return Ok(ExperimentalFeatureEnablementSetResponse { enablement });
        }

        self.config_manager
            .extend_runtime_feature_enablement(
                enablement
                    .iter()
                    .map(|(name, enabled)| (name.clone(), *enabled)),
            )
            .map_err(|_| internal_error("failed to update feature enablement"))?;

        self.load_latest_config(/*fallback_cwd*/ None).await?;
        self.reload_user_config().await;

        Ok(ExperimentalFeatureEnablementSetResponse { enablement })
    }

    async fn reload_user_config(&self) {
        let next_config = match self.load_latest_config(/*fallback_cwd*/ None).await {
            Ok(config) => config,
            Err(err) => {
                tracing::warn!(
                    "failed to rebuild user config for runtime refresh: {}",
                    err.message
                );
                return;
            }
        };
        let thread_ids = self.thread_manager.list_thread_ids().await;
        for thread_id in thread_ids {
            let Ok(thread) = self.thread_manager.get_thread(thread_id).await else {
                continue;
            };
            thread.refresh_runtime_config(next_config.clone()).await;
        }
    }

    async fn emit_plugin_toggle_events(
        &self,
        pending_changes: std::collections::BTreeMap<String, bool>,
    ) {
        for (plugin_id, enabled) in pending_changes {
            let Ok(plugin_id) = PluginId::parse(&plugin_id) else {
                continue;
            };
            let metadata = codex_core_plugins::loader::installed_plugin_telemetry_metadata(
                self.config_manager.codex_home(),
                &plugin_id,
            )
            .await;
            if enabled {
                self.analytics_events_client.track_plugin_enabled(metadata);
            } else {
                self.analytics_events_client.track_plugin_disabled(metadata);
            }
        }
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
        allow_managed_hooks_only: requirements.allow_managed_hooks_only,
        feature_requirements: requirements
            .feature_requirements
            .map(|requirements| requirements.entries),
        hooks: requirements.hooks.map(map_hooks_requirements_to_api),
        enforce_residency: requirements
            .enforce_residency
            .map(map_residency_requirement_to_api),
        network: requirements.network.map(map_network_requirements_to_api),
    }
}

fn map_hooks_requirements_to_api(hooks: ManagedHooksRequirementsToml) -> ManagedHooksRequirements {
    let ManagedHooksRequirementsToml {
        managed_dir,
        windows_managed_dir,
        hooks,
    } = hooks;
    let HookEventsToml {
        pre_tool_use,
        permission_request,
        post_tool_use,
        pre_compact,
        post_compact,
        session_start,
        user_prompt_submit,
        stop,
    } = hooks;

    ManagedHooksRequirements {
        managed_dir,
        windows_managed_dir,
        pre_tool_use: map_hook_matcher_groups_to_api(pre_tool_use),
        permission_request: map_hook_matcher_groups_to_api(permission_request),
        post_tool_use: map_hook_matcher_groups_to_api(post_tool_use),
        pre_compact: map_hook_matcher_groups_to_api(pre_compact),
        post_compact: map_hook_matcher_groups_to_api(post_compact),
        session_start: map_hook_matcher_groups_to_api(session_start),
        user_prompt_submit: map_hook_matcher_groups_to_api(user_prompt_submit),
        stop: map_hook_matcher_groups_to_api(stop),
    }
}

fn map_hook_matcher_groups_to_api(
    groups: Vec<CoreMatcherGroup>,
) -> Vec<ConfiguredHookMatcherGroup> {
    groups
        .into_iter()
        .map(map_hook_matcher_group_to_api)
        .collect()
}

fn map_hook_matcher_group_to_api(group: CoreMatcherGroup) -> ConfiguredHookMatcherGroup {
    ConfiguredHookMatcherGroup {
        matcher: group.matcher,
        hooks: group
            .hooks
            .into_iter()
            .map(map_hook_handler_to_api)
            .collect(),
    }
}

fn map_hook_handler_to_api(handler: CoreHookHandlerConfig) -> ConfiguredHookHandler {
    match handler {
        CoreHookHandlerConfig::Command {
            command,
            command_windows,
            timeout_sec,
            r#async,
            status_message,
        } => ConfiguredHookHandler::Command {
            command,
            command_windows,
            timeout_sec,
            r#async,
            status_message,
        },
        CoreHookHandlerConfig::Prompt {} => ConfiguredHookHandler::Prompt {},
        CoreHookHandlerConfig::Agent {} => ConfiguredHookHandler::Agent {},
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
    network: codex_config::NetworkRequirementsToml,
) -> NetworkRequirements {
    let allowed_domains = network
        .domains
        .as_ref()
        .and_then(codex_config::NetworkDomainPermissionsToml::allowed_domains);
    let denied_domains = network
        .domains
        .as_ref()
        .and_then(codex_config::NetworkDomainPermissionsToml::denied_domains);
    let allow_unix_sockets = network
        .unix_sockets
        .as_ref()
        .map(codex_config::NetworkUnixSocketPermissionsToml::allow_unix_sockets)
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
    permission: codex_config::NetworkDomainPermissionToml,
) -> NetworkDomainPermission {
    match permission {
        codex_config::NetworkDomainPermissionToml::Allow => NetworkDomainPermission::Allow,
        codex_config::NetworkDomainPermissionToml::Deny => NetworkDomainPermission::Deny,
    }
}

fn map_network_unix_socket_permission_to_api(
    permission: codex_config::NetworkUnixSocketPermissionToml,
) -> NetworkUnixSocketPermission {
    match permission {
        codex_config::NetworkUnixSocketPermissionToml::Allow => NetworkUnixSocketPermission::Allow,
        codex_config::NetworkUnixSocketPermissionToml::None => NetworkUnixSocketPermission::None,
    }
}

fn map_error(err: ConfigManagerError) -> JSONRPCErrorError {
    if let Some(code) = err.write_error_code() {
        return config_write_error(code, err.to_string());
    }

    internal_error(err.to_string())
}

fn config_write_error(code: ConfigWriteErrorCode, message: impl Into<String>) -> JSONRPCErrorError {
    let mut error = invalid_request(message);
    error.data = Some(json!({
        "config_write_error_code": code,
    }));
    error
}

#[cfg(test)]
mod tests {
    use super::map_requirements_toml_to_api;
    use codex_config::ConfigRequirementsToml;
    use pretty_assertions::assert_eq;

    #[test]
    fn requirements_api_includes_allow_managed_hooks_only() {
        let mapped = map_requirements_toml_to_api(ConfigRequirementsToml {
            allow_managed_hooks_only: Some(true),
            ..ConfigRequirementsToml::default()
        });

        assert_eq!(mapped.allow_managed_hooks_only, Some(true));
        assert_eq!(mapped.hooks, None);
    }
}
