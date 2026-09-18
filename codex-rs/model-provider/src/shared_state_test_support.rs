//! Seeds encrypted gateway credentials for provider and agent integration tests.

#![allow(clippy::expect_used)] // Fixture setup failures should fail the calling test immediately.

use std::sync::Arc;
use std::sync::Mutex;

use codex_keyring_store::CredentialStoreError;
use codex_keyring_store::KeyringStore;
use codex_login::AuthManager;
use codex_login::GatewayAuthConfig;
use codex_login::GatewayAuthManager;
use codex_model_provider_info::ModelProviderInfo;
use codex_secrets::LocalSecretsNamespace;
use codex_secrets::SecretName;
use codex_secrets::SecretScope;
use codex_secrets::SecretsBackendKind;
use codex_secrets::SecretsManager;
use sha2::Digest;
use sha2::Sha256;

use super::process_shared_state;

#[derive(Debug)]
struct TestKeyring(Mutex<Option<String>>);

impl KeyringStore for TestKeyring {
    fn load(
        &self,
        _service: &str,
        _account: &str,
    ) -> std::result::Result<Option<String>, CredentialStoreError> {
        Ok(self
            .0
            .lock()
            .expect("gateway test token store lock")
            .clone())
    }
    fn save(
        &self,
        _service: &str,
        _account: &str,
        value: &str,
    ) -> std::result::Result<(), CredentialStoreError> {
        *self.0.lock().expect("gateway test token store lock") = Some(value.to_string());
        Ok(())
    }
    fn delete(
        &self,
        _service: &str,
        _account: &str,
    ) -> std::result::Result<bool, CredentialStoreError> {
        Ok(self
            .0
            .lock()
            .expect("gateway test token store lock")
            .take()
            .is_some())
    }
}

/// Seeds a provider's encrypted gateway store with a test-only keyring passphrase.
/// Keep the returned manager alive while requests use this fixture.
pub fn seed_gateway_auth(
    info: &ModelProviderInfo,
    primary: &AuthManager,
    token: serde_json::Value,
) -> Arc<GatewayAuthManager> {
    let config = info
        .gateway_oauth
        .clone()
        .expect("gateway test configuration");
    let runtime = primary.runtime_config();
    let oauth = GatewayAuthConfig {
        authorization_url: config.authorization_url,
        token_url: config.token_url,
        client_id: config.client_id,
        resource: config.resource,
        scopes: config.scopes,
        redirect_port: config.redirect_port,
    };
    // Repeated fixtures sharing a Codex home must use the same encryption key.
    let keyring = Arc::new(TestKeyring(Mutex::new(Some(
        "gateway-test-passphrase".to_string(),
    ))));
    let secrets = SecretsManager::new_with_keyring_store_and_namespace(
        runtime.codex_home.clone(),
        SecretsBackendKind::Local,
        keyring.clone(),
        LocalSecretsNamespace::GatewayOAuth,
    );
    // Match the credential identity used by GatewayAuthManager's encrypted store.
    let mut digest = Sha256::new();
    digest.update(runtime.codex_home.to_string_lossy().as_bytes());
    digest.update([0]);
    for value in [
        oauth.authorization_url.as_str(),
        oauth.token_url.as_str(),
        oauth.client_id.as_str(),
        oauth.resource.as_deref().unwrap_or_default(),
    ] {
        digest.update(value.as_bytes());
        digest.update([0]);
    }
    for scope in &oauth.scopes {
        digest.update(scope.as_bytes());
        digest.update([0]);
    }
    let name = SecretName::new(&format!("PROVIDER_OAUTH_{:X}", digest.finalize()))
        .expect("gateway test credential name");
    secrets
        .set(&SecretScope::Global, &name, &token.to_string())
        .expect("seed encrypted gateway credentials");
    let manager = Arc::new(
        GatewayAuthManager::new(
            oauth.clone(),
            runtime.codex_home.clone(),
            runtime.auth_route_config.http_client_factory(),
            keyring,
        )
        .expect("gateway test HTTP client"),
    );
    process_shared_state()
        .gateway_managers
        .lock()
        .expect("gateway test manager registry lock")
        .push((oauth, runtime, Arc::downgrade(&manager)));
    manager
}
