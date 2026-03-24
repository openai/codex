use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use crypto_box::SecretKey as Curve25519SecretKey;
use ed25519_dalek::Signer as _;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest as _;
use sha2::Sha512;
use tracing::info;

use super::*;

const AGENT_TASK_REGISTRATION_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RegisteredAgentTask {
    pub(crate) agent_runtime_id: String,
    pub(crate) task_id: String,
    pub(crate) registered_at: String,
}

#[derive(Debug, Serialize)]
struct RegisterTaskRequest {
    signature: String,
    timestamp: String,
}

#[derive(Debug, Deserialize)]
struct RegisterTaskResponse {
    encrypted_task_id: String,
}

impl AgentIdentityManager {
    pub(crate) async fn register_task(&self) -> Result<Option<RegisteredAgentTask>> {
        if !self.feature_enabled {
            return Ok(None);
        }

        let Some(binding) = self.current_binding().await else {
            return Ok(None);
        };
        let Some(stored_identity) = self.ensure_registered_identity().await? else {
            return Ok(None);
        };

        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let request_body = RegisterTaskRequest {
            signature: sign_task_registration_payload(&stored_identity, &timestamp)?,
            timestamp,
        };

        let client = create_client();
        let url =
            agent_task_registration_url(&self.chatgpt_base_url, &stored_identity.agent_runtime_id);
        let response = client
            .post(&url)
            .bearer_auth(&binding.access_token)
            .header("chatgpt-account-id", &binding.chatgpt_account_id)
            .json(&request_body)
            .timeout(AGENT_TASK_REGISTRATION_TIMEOUT)
            .send()
            .await
            .with_context(|| format!("failed to send agent task registration request to {url}"))?;

        if response.status().is_success() {
            let response_body = response
                .json::<RegisterTaskResponse>()
                .await
                .with_context(|| format!("failed to parse agent task response from {url}"))?;
            let registered_task = RegisteredAgentTask {
                agent_runtime_id: stored_identity.agent_runtime_id.clone(),
                task_id: decrypt_task_id_response(&stored_identity, &response_body.encrypted_task_id)?,
                registered_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            };
            info!(
                agent_runtime_id = %registered_task.agent_runtime_id,
                task_id = %registered_task.task_id,
                "registered agent task"
            );
            return Ok(Some(registered_task));
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("agent task registration failed with status {status} from {url}: {body}")
    }
}

fn sign_task_registration_payload(
    stored_identity: &StoredAgentIdentity,
    timestamp: &str,
) -> Result<String> {
    let signing_key = stored_identity.signing_key()?;
    let payload = format!("{}:{timestamp}", stored_identity.agent_runtime_id);
    Ok(BASE64_STANDARD.encode(signing_key.sign(payload.as_bytes()).to_bytes()))
}

fn decrypt_task_id_response(
    stored_identity: &StoredAgentIdentity,
    encrypted_task_id: &str,
) -> Result<String> {
    let signing_key = stored_identity.signing_key()?;
    let ciphertext = BASE64_STANDARD
        .decode(encrypted_task_id)
        .context("encrypted task id is not valid base64")?;
    let plaintext = curve25519_secret_key_from_signing_key(&signing_key)
        .unseal(&ciphertext)
        .map_err(|_| anyhow::anyhow!("failed to decrypt encrypted task id"))?;
    String::from_utf8(plaintext).context("decrypted task id is not valid UTF-8")
}

fn curve25519_secret_key_from_signing_key(signing_key: &SigningKey) -> Curve25519SecretKey {
    let digest = Sha512::digest(signing_key.to_bytes());
    let mut secret_key = [0u8; 32];
    secret_key.copy_from_slice(&digest[..32]);
    secret_key[0] &= 248;
    secret_key[31] &= 127;
    secret_key[31] |= 64;
    Curve25519SecretKey::from(secret_key)
}

fn agent_task_registration_url(chatgpt_base_url: &str, agent_runtime_id: &str) -> String {
    let trimmed = chatgpt_base_url.trim_end_matches('/');
    let path = format!("/v1/agent/{agent_runtime_id}/task/register");
    if let Some(root) = trimmed.strip_suffix("/backend-api") {
        return format!("{root}{path}");
    }
    format!("{trimmed}{path}")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use codex_app_server_protocol::AuthMode as ApiAuthMode;
    use codex_keyring_store::tests::MockKeyringStore;
    use codex_login::AuthCredentialsStoreMode;
    use codex_login::AuthDotJson;
    use codex_login::save_auth;
    use codex_login::token_data::IdTokenInfo;
    use codex_login::token_data::TokenData;
    use pretty_assertions::assert_eq;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    use super::*;

    #[tokio::test]
    async fn register_task_skips_when_feature_is_disabled() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let keyring_store = Arc::new(MockKeyringStore::default());
        let secrets_manager = SecretsManager::new_with_keyring_store(
            tempdir.path().to_path_buf(),
            SecretsBackendKind::Local,
            keyring_store,
        );
        let auth_manager =
            AuthManager::from_auth_for_testing(make_chatgpt_auth("account-123", Some("user-123")));
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ false,
            "https://chatgpt.com/backend-api/".to_string(),
            SessionSource::Cli,
            secrets_manager,
        );

        assert_eq!(manager.register_task().await.unwrap(), None);
    }

    #[tokio::test]
    async fn register_task_skips_for_api_key_auth() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let keyring_store = Arc::new(MockKeyringStore::default());
        let secrets_manager = SecretsManager::new_with_keyring_store(
            tempdir.path().to_path_buf(),
            SecretsBackendKind::Local,
            keyring_store,
        );
        let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test-key"));
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ true,
            "https://chatgpt.com/backend-api/".to_string(),
            SessionSource::Cli,
            secrets_manager,
        );

        assert_eq!(manager.register_task().await.unwrap(), None);
    }

    #[tokio::test]
    async fn register_task_registers_and_decrypts_plaintext_task_id() {
        let server = MockServer::start().await;
        let tempdir = tempfile::tempdir().expect("tempdir");
        let keyring_store = Arc::new(MockKeyringStore::default());
        let secrets_manager = SecretsManager::new_with_keyring_store(
            tempdir.path().to_path_buf(),
            SecretsBackendKind::Local,
            keyring_store,
        );
        let auth_manager =
            AuthManager::from_auth_for_testing(make_chatgpt_auth("account-123", Some("user-123")));
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ true,
            format!("{}/backend-api/", server.uri()),
            SessionSource::Cli,
            secrets_manager.clone(),
        );
        let stored_identity =
            seed_stored_identity(&manager, &secrets_manager, "agent-123", "account-123");
        let encrypted_task_id =
            encrypt_task_id_for_identity(&stored_identity, "task_123").expect("task ciphertext");

        Mock::given(method("POST"))
            .and(path("/v1/agent/agent-123/task/register"))
            .and(header("authorization", "Bearer access-token-account-123"))
            .and(header("chatgpt-account-id", "account-123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "encrypted_task_id": encrypted_task_id,
            })))
            .expect(1)
            .mount(&server)
            .await;

        let task = manager
            .register_task()
            .await
            .unwrap()
            .expect("task should be registered");

        assert_eq!(
            task,
            RegisteredAgentTask {
                agent_runtime_id: "agent-123".to_string(),
                task_id: "task_123".to_string(),
                registered_at: task.registered_at.clone(),
            }
        );
    }

    #[tokio::test]
    async fn register_task_uses_canonical_registration_url() {
        let server = MockServer::start().await;
        let tempdir = tempfile::tempdir().expect("tempdir");
        let keyring_store = Arc::new(MockKeyringStore::default());
        let secrets_manager = SecretsManager::new_with_keyring_store(
            tempdir.path().to_path_buf(),
            SecretsBackendKind::Local,
            keyring_store,
        );
        let auth_manager =
            AuthManager::from_auth_for_testing(make_chatgpt_auth("account-123", Some("user-123")));
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ true,
            format!("{}/backend-api/", server.uri()),
            SessionSource::Cli,
            secrets_manager.clone(),
        );
        let stored_identity =
            seed_stored_identity(&manager, &secrets_manager, "agent-fallback", "account-123");
        let encrypted_task_id = encrypt_task_id_for_identity(&stored_identity, "task_fallback")
            .expect("task ciphertext");

        Mock::given(method("POST"))
            .and(path("/v1/agent/agent-fallback/task/register"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "encrypted_task_id": encrypted_task_id,
            })))
            .expect(1)
            .mount(&server)
            .await;

        let task = manager
            .register_task()
            .await
            .unwrap()
            .expect("task should be registered");

        assert_eq!(task.agent_runtime_id, "agent-fallback");
        assert_eq!(task.task_id, "task_fallback");
    }

    fn seed_stored_identity(
        manager: &AgentIdentityManager,
        secrets_manager: &SecretsManager,
        agent_runtime_id: &str,
        account_id: &str,
    ) -> StoredAgentIdentity {
        let key_material = generate_agent_key_material().expect("key material");
        let binding =
            AgentIdentityBinding::from_auth(&make_chatgpt_auth(account_id, Some("user-123")), None)
                .expect("binding");
        let stored_identity = StoredAgentIdentity {
            binding_id: binding.binding_id.clone(),
            chatgpt_account_id: account_id.to_string(),
            chatgpt_user_id: Some("user-123".to_string()),
            agent_runtime_id: agent_runtime_id.to_string(),
            private_key_pkcs8_base64: key_material.private_key_pkcs8_base64,
            public_key_ssh: key_material.public_key_ssh,
            registered_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            abom: manager.abom.clone(),
        };
        manager
            .store_identity(&binding, &stored_identity)
            .expect("store identity");
        let scope = secret_scope(&binding).expect("scope");
        let name = agent_identity_secret_name().expect("secret name");
        let persisted = secrets_manager
            .get(&scope, &name)
            .expect("read secret")
            .expect("persisted secret");
        let parsed: StoredAgentIdentity = serde_json::from_str(&persisted).expect("json");
        assert_eq!(parsed, stored_identity);
        stored_identity
    }

    fn encrypt_task_id_for_identity(
        stored_identity: &StoredAgentIdentity,
        task_id: &str,
    ) -> Result<String> {
        let mut rng = crypto_box::aead::OsRng;
        let public_key =
            curve25519_secret_key_from_signing_key(&stored_identity.signing_key()?).public_key();
        let ciphertext = public_key
            .seal(&mut rng, task_id.as_bytes())
            .map_err(|_| anyhow::anyhow!("failed to encrypt test task id"))?;
        Ok(BASE64_STANDARD.encode(ciphertext))
    }

    fn make_chatgpt_auth(account_id: &str, user_id: Option<&str>) -> CodexAuth {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let auth_json = AuthDotJson {
            auth_mode: Some(ApiAuthMode::Chatgpt),
            openai_api_key: None,
            tokens: Some(TokenData {
                id_token: IdTokenInfo {
                    email: None,
                    chatgpt_plan_type: None,
                    chatgpt_user_id: user_id.map(ToOwned::to_owned),
                    chatgpt_account_id: Some(account_id.to_string()),
                    raw_jwt: fake_id_token(account_id, user_id),
                },
                access_token: format!("access-token-{account_id}"),
                refresh_token: "refresh-token".to_string(),
                account_id: Some(account_id.to_string()),
            }),
            last_refresh: Some(Utc::now()),
        };
        save_auth(tempdir.path(), &auth_json, AuthCredentialsStoreMode::File).expect("save auth");
        CodexAuth::from_auth_storage(tempdir.path(), AuthCredentialsStoreMode::File)
            .expect("load auth")
            .expect("auth")
    }

    fn fake_id_token(account_id: &str, user_id: Option<&str>) -> String {
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
        let payload = serde_json::json!({
            "https://api.openai.com/auth": {
                "chatgpt_user_id": user_id,
                "chatgpt_account_id": account_id,
            }
        });
        let payload = URL_SAFE_NO_PAD.encode(payload.to_string());
        format!("{header}.{payload}.signature")
    }
}
