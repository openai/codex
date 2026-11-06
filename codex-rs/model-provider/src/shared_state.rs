use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::Weak;

use crate::gateway_export::GatewayExport;
use codex_keyring_store::DefaultKeyringStore;
use codex_login::AuthManager;
use codex_login::AuthRuntimeConfig;
use codex_login::GatewayAuthConfig;
use codex_login::OAuthSessionManager;
use codex_model_provider_info::CredentialExportConfig;
use codex_model_provider_info::GatewayOAuthSettings;
use codex_model_provider_info::ModelProviderAwsAuthInfo;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::config_types::ModelProviderAuthInfo;

use crate::amazon_bedrock::AwsAuthRecovery;
use crate::amazon_bedrock::AwsCredentialExport;

/// Provider-owned runtime state shared across independently configured sessions.
#[derive(Debug, Default)]
pub(crate) struct ModelProviderSharedState {
    command_credentials: Mutex<Vec<(ModelProviderAuthInfo, AuthRuntimeConfig, Weak<AuthManager>)>>,
    aws_credential_exports: Mutex<Vec<(ModelProviderAwsAuthInfo, Weak<AwsCredentialExport>)>>,
    aws_auth_recoveries: Mutex<Vec<(ModelProviderAwsAuthInfo, Weak<AwsAuthRecovery>)>>,
    gateway_exports: Mutex<Vec<GatewayExportEntry>>,
    oauth_sessions: Mutex<
        Vec<(
            GatewayAuthConfig,
            AuthRuntimeConfig,
            Weak<OAuthSessionManager>,
        )>,
    >,
}

#[derive(Debug)]
struct GatewayExportEntry {
    owner: Weak<AuthManager>,
    provider: ModelProviderInfo,
    export: Weak<GatewayExport>,
}

pub(crate) fn process_shared_state() -> &'static ModelProviderSharedState {
    static STATE: OnceLock<ModelProviderSharedState> = OnceLock::new();
    STATE.get_or_init(ModelProviderSharedState::default)
}

impl ModelProviderSharedState {
    pub(crate) fn command_credentials(
        &self,
        config: &ModelProviderAuthInfo,
        runtime: &AuthRuntimeConfig,
    ) -> Arc<AuthManager> {
        let mut owners = self
            .command_credentials
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        owners.retain(|(_, _, owner)| owner.strong_count() != 0);
        if let Some(owner) = owners
            .iter()
            .find(|(source, context, _)| source == config && context == runtime)
            .and_then(|(_, _, owner)| owner.upgrade())
        {
            return owner;
        }
        let owner = AuthManager::external_bearer_with_runtime(config.clone(), runtime.clone());
        owners.push((config.clone(), runtime.clone(), Arc::downgrade(&owner)));
        owner
    }

    pub(crate) fn gateway_oauth_session(
        &self,
        config: &GatewayOAuthSettings,
        runtime: &AuthRuntimeConfig,
    ) -> std::io::Result<Arc<OAuthSessionManager>> {
        let oauth = GatewayAuthConfig {
            authorization_url: config.authorization_url.clone(),
            device_authorization_url: config.device_authorization_url.clone(),
            token_url: config.token_url.clone(),
            client_id: config.client_id.clone(),
            resource: config.resource.clone(),
            scopes: config.scopes.clone(),
            redirect_port: config.redirect_port,
            credentials_store: config.credentials_store,
        };
        let mut managers = self
            .oauth_sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        managers.retain(|(_, _, manager)| manager.strong_count() != 0);
        if let Some(manager) = managers
            .iter()
            .find(|(cached_config, cached_runtime, _)| {
                cached_config == &oauth && cached_runtime == runtime
            })
            .and_then(|(_, _, manager)| manager.upgrade())
        {
            return Ok(manager);
        }
        let manager = Arc::new(OAuthSessionManager::new(
            oauth.clone(),
            runtime.codex_home.clone(),
            runtime.auth_route_config.http_client_factory(),
            Arc::new(DefaultKeyringStore),
        )?);
        managers.push((oauth, runtime.clone(), Arc::downgrade(&manager)));
        Ok(manager)
    }

    // The catalog preflight and endpoint construct separate providers for the same owner.
    pub(crate) fn gateway_export(
        &self,
        provider: &ModelProviderInfo,
        command: &CredentialExportConfig,
        owner: &Arc<AuthManager>,
    ) -> Arc<GatewayExport> {
        let owner_key = Arc::downgrade(owner);
        let mut entries = self
            .gateway_exports
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        entries.retain(|entry| entry.export.strong_count() != 0 && entry.owner.strong_count() != 0);
        if let Some(export) = entries
            .iter()
            .find(|entry| entry.owner.ptr_eq(&owner_key) && &entry.provider == provider)
            .and_then(|entry| entry.export.upgrade())
        {
            return export;
        }
        let export = Arc::new(GatewayExport::new(
            command.clone(),
            owner.application_network_policy(),
        ));
        entries.push(GatewayExportEntry {
            owner: owner_key,
            provider: provider.clone(),
            export: Arc::downgrade(&export),
        });
        export
    }

    pub(crate) fn aws_credential_export(
        &self,
        aws: &ModelProviderAwsAuthInfo,
    ) -> Option<Arc<AwsCredentialExport>> {
        let config = aws.credential_export.as_ref()?;
        let mut exports = self
            .aws_credential_exports
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        exports.retain(|(_, export)| export.strong_count() != 0);

        if let Some(export) = exports
            .iter()
            .find(|(cached_aws, _)| cached_aws == aws)
            .and_then(|(_, export)| export.upgrade())
        {
            return Some(export);
        }

        let export = Arc::new(AwsCredentialExport::new(config.clone()));
        exports.push((aws.clone(), Arc::downgrade(&export)));
        Some(export)
    }

    pub(crate) fn aws_auth_recovery(
        &self,
        aws: &ModelProviderAwsAuthInfo,
    ) -> Option<Arc<AwsAuthRecovery>> {
        let config = aws.auth_refresh.as_ref()?;
        let mut recoveries = self
            .aws_auth_recoveries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        recoveries.retain(|(_, recovery)| recovery.strong_count() != 0);

        if let Some(recovery) = recoveries
            .iter()
            .find(|(cached_aws, _)| cached_aws == aws)
            .and_then(|(_, recovery)| recovery.upgrade())
        {
            return Some(recovery);
        }

        let recovery = Arc::new(AwsAuthRecovery::new(config.clone()));
        recoveries.push((aws.clone(), Arc::downgrade(&recovery)));
        Some(recovery)
    }
}

#[path = "shared_state_test_support.rs"]
pub(crate) mod test_support;

#[cfg(test)]
#[path = "shared_state_tests.rs"]
mod tests;
