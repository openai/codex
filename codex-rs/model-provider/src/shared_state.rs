use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::Weak;

use codex_keyring_store::DefaultKeyringStore;
use codex_login::AuthRuntimeConfig;
use codex_login::GatewayAuthConfig;
use codex_login::GatewayAuthManager;
use codex_model_provider_info::GatewayOAuthConfig;
use codex_model_provider_info::ModelProviderAwsAuthInfo;

use crate::amazon_bedrock::AwsAuthRecovery;
use crate::amazon_bedrock::AwsCredentialExport;

/// Provider-owned runtime state shared across independently configured sessions.
#[derive(Debug, Default)]
pub(crate) struct ModelProviderSharedState {
    aws_credential_exports: Mutex<Vec<(ModelProviderAwsAuthInfo, Weak<AwsCredentialExport>)>>,
    aws_auth_recoveries: Mutex<Vec<(ModelProviderAwsAuthInfo, Weak<AwsAuthRecovery>)>>,
    gateway_managers: Mutex<
        Vec<(
            GatewayAuthConfig,
            AuthRuntimeConfig,
            Weak<GatewayAuthManager>,
        )>,
    >,
}

pub(crate) fn process_shared_state() -> &'static ModelProviderSharedState {
    static STATE: OnceLock<ModelProviderSharedState> = OnceLock::new();
    STATE.get_or_init(ModelProviderSharedState::default)
}

impl ModelProviderSharedState {
    pub(crate) fn gateway_auth(
        &self,
        config: &GatewayOAuthConfig,
        runtime: &AuthRuntimeConfig,
    ) -> std::io::Result<Arc<GatewayAuthManager>> {
        let oauth = GatewayAuthConfig {
            authorization_url: config.authorization_url.clone(),
            token_url: config.token_url.clone(),
            client_id: config.client_id.clone(),
            resource: config.resource.clone(),
            scopes: config.scopes.clone(),
            redirect_port: config.redirect_port,
        };
        let mut managers = self
            .gateway_managers
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
        let manager = Arc::new(GatewayAuthManager::new(
            oauth.clone(),
            runtime.codex_home.clone(),
            runtime.auth_route_config.http_client_factory(),
            Arc::new(DefaultKeyringStore),
        )?);
        managers.push((oauth, runtime.clone(), Arc::downgrade(&manager)));
        Ok(manager)
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

#[cfg(test)]
#[path = "shared_state_tests.rs"]
mod tests;

#[path = "shared_state_test_support.rs"]
pub(crate) mod test_support;
