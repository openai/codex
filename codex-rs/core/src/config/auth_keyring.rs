use super::Config;
use super::ConfigTomlLoadResult;
use super::ManagedFeatures;
use super::resolve_bootstrap_auth_route_config;
use codex_config::types::AuthKeyringBackendKind;
use codex_features::Feature;
use codex_features::FeatureConfigSource;
use codex_features::FeatureOverrides;
use codex_features::Features;
use codex_login::AuthConfig;
use std::path::Path;

impl Config {
    pub fn auth_keyring_backend_kind(&self) -> AuthKeyringBackendKind {
        auth_keyring_backend_kind_from_secret_auth_storage(
            self.features.enabled(Feature::SecretAuthStorage),
        )
    }

    pub fn auth_config(&self) -> AuthConfig {
        AuthConfig {
            codex_home: self.codex_home.to_path_buf(),
            auth_credentials_store_mode: self.cli_auth_credentials_store_mode,
            keyring_backend_kind: self.auth_keyring_backend_kind(),
            forced_login_method: self.forced_login_method,
            chatgpt_base_url: Some(self.chatgpt_base_url.clone()),
            forced_chatgpt_workspace_id: self.forced_chatgpt_workspace_id.clone(),
            managed_auth_policy: self.config_layer_stack.requirements().managed_auth_policy(),
            auth_route_config: self.auth_route_config(),
        }
    }
}

/// Builds authentication settings from the locally resolved bootstrap config.
///
/// Use this before fetching cloud requirements, when a full [`Config`] is not
/// yet available. Preserves the configured credential store, keyring backend,
/// ChatGPT base URL, auth routing, and managed login/workspace restrictions.
pub fn bootstrap_auth_config(
    codex_home: &Path,
    bootstrap_config: &ConfigTomlLoadResult,
) -> std::io::Result<AuthConfig> {
    let config = &bootstrap_config.config_toml;
    // Empty legacy workspace settings mean unrestricted, not an empty allowlist.
    let forced_chatgpt_workspace_id = config
        .forced_chatgpt_workspace_id
        .clone()
        .map(|workspaces| {
            workspaces
                .into_vec()
                .into_iter()
                .map(|workspace| workspace.trim().to_string())
                .filter(|workspace| !workspace.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|workspaces| !workspaces.is_empty());
    let auth_config = AuthConfig {
        codex_home: codex_home.to_path_buf(),
        auth_credentials_store_mode: config.cli_auth_credentials_store.unwrap_or_default(),
        keyring_backend_kind: resolve_bootstrap_auth_keyring_backend_kind(bootstrap_config)?,
        forced_login_method: config.forced_login_method,
        chatgpt_base_url: config.chatgpt_base_url.clone(),
        forced_chatgpt_workspace_id,
        managed_auth_policy: bootstrap_config
            .config_layer_stack
            .requirements()
            .managed_auth_policy(),
        auth_route_config: resolve_bootstrap_auth_route_config(
            config,
            bootstrap_config
                .config_layer_stack
                .requirements()
                .feature_requirements
                .as_ref(),
        )?,
    };
    auth_config.validate()?;
    Ok(auth_config)
}

/// Resolve the auth keyring backend from a partially loaded bootstrap config.
///
/// This is intended for startup paths that must read auth before managed cloud
/// requirements can be loaded and before a full [`Config`] exists.
pub fn resolve_bootstrap_auth_keyring_backend_kind(
    bootstrap_config: &ConfigTomlLoadResult,
) -> std::io::Result<AuthKeyringBackendKind> {
    let config_toml = &bootstrap_config.config_toml;
    let features = Features::from_sources(
        FeatureConfigSource {
            features: config_toml.features.as_ref(),
            experimental_use_unified_exec_tool: config_toml.experimental_use_unified_exec_tool,
        },
        FeatureConfigSource::default(),
        FeatureOverrides::default(),
    );
    let managed_features = ManagedFeatures::from_configured(
        features,
        bootstrap_config
            .config_layer_stack
            .requirements()
            .feature_requirements
            .clone(),
    )?;
    Ok(auth_keyring_backend_kind_from_secret_auth_storage(
        managed_features.enabled(Feature::SecretAuthStorage),
    ))
}

fn auth_keyring_backend_kind_from_secret_auth_storage(
    secret_auth_storage_enabled: bool,
) -> AuthKeyringBackendKind {
    if secret_auth_storage_enabled {
        AuthKeyringBackendKind::Secrets
    } else {
        AuthKeyringBackendKind::Direct
    }
}

#[cfg(test)]
#[path = "auth_keyring_tests.rs"]
mod tests;
