//! Seeds gateway credentials in the configured store for provider and agent integration tests.

#![allow(clippy::expect_used)] // Fixture setup failures should fail the calling test immediately.

use std::sync::Arc;
use std::sync::Mutex;

use codex_keyring_store::CredentialStoreError;
use codex_keyring_store::KeyringStore;
use codex_login::AuthManager;
use codex_login::GatewayAuthConfig;
use codex_login::GatewayAuthManager;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::OAuthCredentialsStore;
use codex_secrets::LocalSecretsNamespace;
use codex_secrets::SecretName;
use codex_secrets::SecretScope;
use codex_secrets::SecretsBackendKind;
use codex_secrets::SecretsManager;

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

/// Seeds a provider's configured gateway store, using a test passphrase for the keyring backend.
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
        device_authorization_url: None,
        token_url: config.token_url,
        client_id: config.client_id,
        resource: config.resource,
        scopes: config.scopes,
        redirect_port: config.redirect_port,
        credentials_store: config.credentials_store,
    };
    // Repeated fixtures sharing a Codex home must use the same encryption key.
    let keyring = Arc::new(TestKeyring(Mutex::new(Some(
        "gateway-test-passphrase".to_string(),
    ))));
    let manager = Arc::new(
        GatewayAuthManager::new(
            oauth.clone(),
            runtime.codex_home.clone(),
            runtime.auth_route_config.http_client_factory(),
            keyring.clone(),
        )
        .expect("gateway test HTTP client"),
    );
    let credential_id = manager.credential_id();
    match oauth.credentials_store {
        OAuthCredentialsStore::Keyring => {
            let secrets = SecretsManager::new_with_keyring_store_and_namespace(
                runtime.codex_home.clone(),
                SecretsBackendKind::Local,
                keyring,
                LocalSecretsNamespace::GatewayOAuth,
            );
            let digest = credential_id
                .strip_prefix("provider-oauth|")
                .expect("gateway credential ID");
            let name = SecretName::new(&format!("PROVIDER_OAUTH_{}", digest.to_ascii_uppercase()))
                .expect("gateway test credential name");
            secrets
                .set(&SecretScope::Global, &name, &token.to_string())
                .expect("seed encrypted gateway credentials");
        }
        OAuthCredentialsStore::File => {
            let dir = runtime.codex_home.join("secrets");
            std::fs::create_dir_all(&dir).expect("create test credentials directory");
            let file = dir.join("gateway_oauth.json");
            let mut tokens: std::collections::BTreeMap<String, serde_json::Value> =
                match std::fs::read(&file) {
                    Ok(bytes) => serde_json::from_slice(&bytes).expect("parse test gateway tokens"),
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Default::default(),
                    Err(err) => panic!("read test gateway file: {err}"),
                };
            tokens.insert(credential_id, token);
            std::fs::write(
                file,
                serde_json::to_vec(&tokens).expect("serialize test gateway tokens"),
            )
            .expect("seed test gateway file");
        }
    }
    process_shared_state()
        .gateway_managers
        .lock()
        .expect("gateway test manager registry lock")
        .push((oauth, runtime, Arc::downgrade(&manager)));
    manager
}
