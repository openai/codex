//! Loads embedded application policy and builds serving authentication from effective requirements.
//! Workspace policy uses stored auth even when the caller uses an environment API key.
//! Caller credential storage remains intact; initialization receives the effective URL and residency.

use crate::config_manager::ConfigManager;
use codex_core::config::Config;
use codex_login::AuthManager;
use std::io::Error as IoError;
use std::io::Result as IoResult;
use std::sync::Arc;

/// Policy shared by transports created before and owned by the embedded app-server.
#[derive(Clone, Default)]
pub struct EmbeddedNetworkPolicy {
    pub(crate) effective: codex_http_client::NetworkPolicyController,
    pub(crate) local: codex_http_client::NetworkPolicyController,
}

impl EmbeddedNetworkPolicy {
    /// Loads the local rules for cloud bootstrap; all other traffic starts unavailable.
    pub async fn load(overrides: &codex_config::LoaderOverrides) -> Self {
        let policy = Self::default();
        let local = async {
            codex_config::loader::load_local_application_requirements(
                codex_exec_server::LOCAL_FS.as_ref(),
                overrides,
            )
            .await?
            .compose(Default::default())
        }
        .await;
        if let Ok(application) = local {
            policy.local.publish(
                policy.local.policy().revision(),
                crate::config_manager::application_network::destination_policy(
                    application.as_ref(),
                ),
            );
        }
        policy
    }

    /// Attaches the live application policy before an environment can start connecting.
    pub fn bind(
        &self,
        factory: codex_http_client::HttpClientFactory,
    ) -> codex_http_client::HttpClientFactory {
        factory.with_network_policy(self.effective.policy())
    }

    /// Publishes successfully loaded requirements before caller-owned clients are created.
    pub fn activate(&self, config: &mut Config) {
        let application = config
            .config_layer_stack
            .requirements()
            .application
            .as_ref()
            .map(|requirements| &requirements.value);
        self.effective.publish(
            self.effective.policy().revision(),
            crate::config_manager::application_network::destination_policy(application),
        );
        self.bind_config(config);
    }

    /// Keeps a rebuilt caller config on the live policy owned by the running app-server.
    pub fn bind_config(&self, config: &mut Config) {
        config.application_network_policy = self.effective.policy();
    }

    /// Limits cloud discovery and each authentication request to their exact local-policy endpoints.
    pub fn bind_bootstrap_auth(
        &self,
        mut auth: codex_login::AuthConfig,
    ) -> codex_login::AuthConfig {
        let factory = auth
            .auth_route_config
            .http_client_factory()
            .clone()
            .with_network_policy(self.local.policy());
        let endpoint = codex_backend_client::Client::new(
            auth.chatgpt_base_url
                .as_deref()
                .unwrap_or("https://chatgpt.com/backend-api/"),
            factory.clone(),
        )
        .config_bundle_url();
        auth.auth_route_config = codex_login::AuthRouteConfig::from_http_client_factory(
            factory.clone().with_network_policy(
                self.local
                    .policy()
                    .restrict_to_endpoints(endpoint.parse().into_iter().collect()),
            ),
        )
        .with_local_bootstrap_factory(factory);
        auth
    }
}

pub(super) async fn configure(
    config_manager: &ConfigManager,
    config: &mut Arc<Config>,
    enable_codex_api_key_env: bool,
) -> IoResult<Arc<AuthManager>> {
    let bootstrap_config = config_manager
        .load_startup_config(Some(config.cwd.to_path_buf()))
        .await?;
    let caller_auth = config.auth_config();
    let bootstrap_auth = codex_login::AuthConfig {
        chatgpt_base_url: Some(bootstrap_config.chatgpt_base_url.clone()),
        forced_login_method: bootstrap_config.forced_login_method,
        forced_chatgpt_workspace_id: bootstrap_config.forced_chatgpt_workspace_id.clone(),
        managed_auth_policy: bootstrap_config
            .config_layer_stack
            .requirements()
            .managed_auth_policy(),
        auth_route_config: bootstrap_config.auth_route_config(),
        ..caller_auth.clone()
    };
    bootstrap_auth.validate()?;
    let bootstrap_auth_manager = AuthManager::shared_from_auth_config(
        bootstrap_auth,
        /*enable_codex_api_key_env*/ false,
    )
    .await
    .map_err(IoError::other)?;
    config_manager.replace_cloud_config_bundle_loader(
        bootstrap_auth_manager,
        bootstrap_config.chatgpt_base_url.clone(),
        bootstrap_config.http_client_factory(),
    );
    let policy_config = config_manager
        .load_startup_config(Some(config.cwd.to_path_buf()))
        .await?;
    let auth_config = codex_login::AuthConfig {
        chatgpt_base_url: Some(policy_config.chatgpt_base_url.clone()),
        forced_login_method: policy_config.forced_login_method,
        forced_chatgpt_workspace_id: policy_config.forced_chatgpt_workspace_id.clone(),
        managed_auth_policy: policy_config
            .config_layer_stack
            .requirements()
            .managed_auth_policy(),
        auth_route_config: policy_config.auth_route_config(),
        ..caller_auth
    };
    auth_config.validate()?;
    let policy_auth_manager = AuthManager::shared_from_auth_config(
        auth_config.clone(),
        /*enable_codex_api_key_env*/ false,
    )
    .await
    .map_err(IoError::other)?;
    let auth_manager = if enable_codex_api_key_env {
        AuthManager::shared_from_auth_config(auth_config, /*enable_codex_api_key_env*/ true)
            .await
            .map_err(IoError::other)?
    } else {
        policy_auth_manager.clone()
    };
    let config = Arc::make_mut(config);
    config.chatgpt_base_url = policy_config.chatgpt_base_url;
    config.enforce_residency = policy_config.enforce_residency;
    config.application_network_policy = policy_config.application_network_policy;
    config.application_auth_route_config = policy_config.application_auth_route_config;
    config.forced_login_method = policy_config.forced_login_method;
    config.forced_chatgpt_workspace_id = policy_config.forced_chatgpt_workspace_id;
    config_manager.replace_cloud_config_bundle_loader(
        policy_auth_manager,
        config.chatgpt_base_url.clone(),
        config.http_client_factory(),
    );
    codex_login::default_client::set_default_client_residency_requirement(
        config.enforce_residency.value(),
    );
    Ok(auth_manager)
}

#[cfg(test)]
#[path = "in_process_bootstrap_tests.rs"]
mod tests;
