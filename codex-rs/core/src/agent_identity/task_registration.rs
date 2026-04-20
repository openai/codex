use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_protocol::protocol::SessionAgentTask;
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

        let Some((auth, binding)) = self.current_auth_binding().await else {
            return Ok(None);
        };

        self.register_task_for_binding(auth, binding).await
    }

    async fn register_task_for_binding(
        &self,
        auth: CodexAuth,
        binding: AgentIdentityBinding,
    ) -> Result<Option<RegisteredAgentTask>> {
        let stored_identity = self
            .ensure_registered_identity_for_binding(&auth, &binding)
            .await?;

        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let request_body = RegisterTaskRequest {
            signature: sign_task_registration_payload(&stored_identity, &timestamp)?,
            timestamp,
        };

        let client = create_client();
        let url =
            agent_task_registration_url(&self.chatgpt_base_url, &stored_identity.agent_runtime_id);
        let human_biscuit = self.mint_human_biscuit(&binding, "POST", &url).await?;
        let response = client
            .post(&url)
            .header("X-OpenAI-Authorization", human_biscuit)
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
                task_id: decrypt_task_id_response(
                    &stored_identity,
                    &response_body.encrypted_task_id,
                )?,
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

impl RegisteredAgentTask {
    pub(crate) fn to_session_agent_task(&self) -> SessionAgentTask {
        SessionAgentTask {
            agent_runtime_id: self.agent_runtime_id.clone(),
            task_id: self.task_id.clone(),
            registered_at: self.registered_at.clone(),
        }
    }

    pub(crate) fn from_session_agent_task(task: SessionAgentTask) -> Self {
        Self {
            agent_runtime_id: task.agent_runtime_id,
            task_id: task.task_id,
            registered_at: task.registered_at,
        }
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
    format!("{trimmed}/v1/agent/{agent_runtime_id}/task/register")
}

#[cfg(test)]
mod tests {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use codex_app_server_protocol::AuthMode as ApiAuthMode;
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
        let auth_manager =
            AuthManager::from_auth_for_testing(make_chatgpt_auth("account-123", Some("user-123")));
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ false,
            "https://chatgpt.com/backend-api/".to_string(),
            SessionSource::Cli,
        );

        assert_eq!(manager.register_task().await.unwrap(), None);
    }

    #[tokio::test]
    async fn register_task_skips_for_api_key_auth() {
        let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test-key"));
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ true,
            "https://chatgpt.com/backend-api/".to_string(),
            SessionSource::Cli,
        );

        assert_eq!(manager.register_task().await.unwrap(), None);
    }

    #[tokio::test]
    async fn register_task_registers_and_decrypts_plaintext_task_id() {
        let server = MockServer::start().await;
        let chatgpt_base_url = server.uri();
        mount_human_biscuit(&server, &chatgpt_base_url, "agent-123").await;
        let auth = make_chatgpt_auth("account-123", Some("user-123"));
        let auth_manager = AuthManager::from_auth_for_testing(auth.clone());
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ true,
            chatgpt_base_url,
            SessionSource::Cli,
        );
        let stored_identity = seed_stored_identity(&manager, &auth, "agent-123", "account-123");
        let encrypted_task_id =
            encrypt_task_id_for_identity(&stored_identity, "task_123").expect("task ciphertext");

        Mock::given(method("POST"))
            .and(path("/v1/agent/agent-123/task/register"))
            .and(header("x-openai-authorization", "human-biscuit"))
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
    async fn register_task_uses_chatgpt_base_url() {
        let server = MockServer::start().await;
        let chatgpt_base_url = format!("{}/backend-api", server.uri());
        mount_human_biscuit(&server, &chatgpt_base_url, "agent-fallback").await;
        let auth = make_chatgpt_auth("account-123", Some("user-123"));
        let auth_manager = AuthManager::from_auth_for_testing(auth.clone());
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ true,
            chatgpt_base_url,
            SessionSource::Cli,
        );
        let stored_identity =
            seed_stored_identity(&manager, &auth, "agent-fallback", "account-123");
        let encrypted_task_id = encrypt_task_id_for_identity(&stored_identity, "task_fallback")
            .expect("task ciphertext");

        Mock::given(method("POST"))
            .and(path("/backend-api/v1/agent/agent-fallback/task/register"))
            .and(header("x-openai-authorization", "human-biscuit"))
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

    #[tokio::test]
    async fn register_task_for_binding_keeps_one_auth_snapshot() {
        let server = MockServer::start().await;
        let chatgpt_base_url = server.uri();
        mount_human_biscuit(&server, &chatgpt_base_url, "agent-123").await;
        let binding_auth = make_chatgpt_auth("account-123", Some("user-123"));
        let auth_manager =
            AuthManager::from_auth_for_testing(make_chatgpt_auth("account-456", Some("user-456")));
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ true,
            chatgpt_base_url,
            SessionSource::Cli,
        );
        let stored_identity =
            seed_stored_identity(&manager, &binding_auth, "agent-123", "account-123");
        let encrypted_task_id =
            encrypt_task_id_for_identity(&stored_identity, "task_123").expect("task ciphertext");
        let binding =
            AgentIdentityBinding::from_auth(&binding_auth, /*forced_workspace_id*/ None)
                .expect("binding");

        Mock::given(method("POST"))
            .and(path("/v1/agent/agent-123/task/register"))
            .and(header("x-openai-authorization", "human-biscuit"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "encrypted_task_id": encrypted_task_id,
            })))
            .expect(1)
            .mount(&server)
            .await;

        let task = manager
            .register_task_for_binding(binding_auth, binding)
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
    async fn task_matches_current_identity_rejects_stale_registered_identity() {
        let auth_manager =
            AuthManager::from_auth_for_testing(make_chatgpt_auth("account-456", Some("user-456")));
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ true,
            "https://chatgpt.com/backend-api/".to_string(),
            SessionSource::Cli,
        );
        let task = RegisteredAgentTask {
            agent_runtime_id: "agent-123".to_string(),
            task_id: "task_123".to_string(),
            registered_at: "2026-03-23T12:00:00Z".to_string(),
        };

        assert!(!manager.task_matches_current_identity(&task).await);
    }

    async fn mount_human_biscuit(
        server: &MockServer,
        chatgpt_base_url: &str,
        agent_runtime_id: &str,
    ) {
        let biscuit_url = agent_identity_biscuit_url(chatgpt_base_url);
        let biscuit_path = reqwest::Url::parse(&biscuit_url)
            .expect("biscuit URL parses")
            .path()
            .to_string();
        let target_url = agent_task_registration_url(chatgpt_base_url, agent_runtime_id);
        Mock::given(method("GET"))
            .and(path(biscuit_path))
            .and(header("authorization", "Bearer access-token-account-123"))
            .and(header("x-original-method", "POST"))
            .and(header("x-original-url", target_url))
            .respond_with(
                ResponseTemplate::new(200).insert_header("x-openai-authorization", "human-biscuit"),
            )
            .expect(1)
            .mount(server)
            .await;
    }

    fn seed_stored_identity(
        manager: &AgentIdentityManager,
        auth: &CodexAuth,
        agent_runtime_id: &str,
        account_id: &str,
    ) -> StoredAgentIdentity {
        let key_material = generate_agent_key_material().expect("key material");
        let binding =
            AgentIdentityBinding::from_auth(auth, /*forced_workspace_id*/ None).expect("binding");
        let stored_identity = StoredAgentIdentity {
            binding_id: binding.binding_id,
            chatgpt_account_id: account_id.to_string(),
            chatgpt_user_id: Some("user-123".to_string()),
            agent_runtime_id: agent_runtime_id.to_string(),
            private_key_pkcs8_base64: key_material.private_key_pkcs8_base64,
            public_key_ssh: key_material.public_key_ssh,
            registered_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            abom: manager.abom.clone(),
        };
        manager
            .store_identity(auth, &stored_identity)
            .expect("store identity");
        let persisted = auth
            .get_agent_identity(account_id)
            .expect("persisted identity");
        assert_eq!(persisted.agent_runtime_id, agent_runtime_id);
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
                    chatgpt_account_is_fedramp: false,
                    raw_jwt: fake_id_token(account_id, user_id),
                },
                access_token: format!("access-token-{account_id}"),
                refresh_token: "refresh-token".to_string(),
                account_id: Some(account_id.to_string()),
            }),
            last_refresh: Some(Utc::now()),
            agent_identity: None,
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
