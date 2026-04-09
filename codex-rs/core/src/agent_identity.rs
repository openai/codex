use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::SecondsFormat;
use chrono::Utc;
use codex_features::Feature;
use codex_login::AgentIdentityAuthRecord;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::default_client::create_client;
use codex_protocol::protocol::SessionSource;
use ed25519_dalek::SigningKey;
use ed25519_dalek::VerifyingKey;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::pkcs8::EncodePrivateKey;
use rand::TryRngCore;
use rand::rngs::OsRng;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::Mutex;
use tracing::debug;
use tracing::info;
use tracing::warn;

mod task_registration;

pub(crate) use task_registration::RegisteredAgentTask;

use crate::config::Config;

const AGENT_REGISTRATION_TIMEOUT: Duration = Duration::from_secs(15);
const AGENT_IDENTITY_BISCUIT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub(crate) struct AgentIdentityManager {
    auth_manager: Arc<AuthManager>,
    chatgpt_base_url: String,
    feature_enabled: bool,
    abom: AgentBillOfMaterials,
    ensure_lock: Arc<Mutex<()>>,
}

impl std::fmt::Debug for AgentIdentityManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentIdentityManager")
            .field("chatgpt_base_url", &self.chatgpt_base_url)
            .field("feature_enabled", &self.feature_enabled)
            .field("abom", &self.abom)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct StoredAgentIdentity {
    pub(crate) binding_id: String,
    pub(crate) chatgpt_account_id: String,
    pub(crate) chatgpt_user_id: Option<String>,
    pub(crate) agent_runtime_id: String,
    pub(crate) private_key_pkcs8_base64: String,
    pub(crate) public_key_ssh: String,
    pub(crate) registered_at: String,
    pub(crate) abom: AgentBillOfMaterials,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct AgentBillOfMaterials {
    pub(crate) agent_version: String,
    pub(crate) agent_harness_id: String,
    pub(crate) running_location: String,
}

#[derive(Debug, Serialize)]
struct RegisterAgentRequest {
    abom: AgentBillOfMaterials,
    agent_public_key: String,
    capabilities: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RegisterAgentResponse {
    agent_runtime_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentIdentityBinding {
    binding_id: String,
    chatgpt_account_id: String,
    chatgpt_user_id: Option<String>,
    access_token: String,
}

struct GeneratedAgentKeyMaterial {
    private_key_pkcs8_base64: String,
    public_key_ssh: String,
}

impl AgentIdentityManager {
    pub(crate) fn new(
        config: &Config,
        auth_manager: Arc<AuthManager>,
        session_source: SessionSource,
    ) -> Self {
        Self {
            auth_manager,
            chatgpt_base_url: config.chatgpt_base_url.clone(),
            feature_enabled: config.features.enabled(Feature::UseAgentIdentity),
            abom: build_abom(session_source),
            ensure_lock: Arc::new(Mutex::new(())),
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.feature_enabled
    }

    pub(crate) async fn ensure_registered_identity(&self) -> Result<Option<StoredAgentIdentity>> {
        if !self.feature_enabled {
            return Ok(None);
        }

        let Some((auth, binding)) = self.current_auth_binding().await else {
            return Ok(None);
        };

        self.ensure_registered_identity_for_binding(&auth, &binding)
            .await
            .map(Some)
    }

    async fn ensure_registered_identity_for_binding(
        &self,
        auth: &CodexAuth,
        binding: &AgentIdentityBinding,
    ) -> Result<StoredAgentIdentity> {
        let _guard = self.ensure_lock.lock().await;

        if let Some(stored_identity) = self.load_stored_identity(auth, binding)? {
            info!(
                agent_runtime_id = %stored_identity.agent_runtime_id,
                binding_id = %binding.binding_id,
                "reusing stored agent identity"
            );
            return Ok(stored_identity);
        }

        let stored_identity = self.register_agent_identity(binding).await?;
        self.store_identity(auth, &stored_identity)?;
        Ok(stored_identity)
    }

    async fn current_auth_binding(&self) -> Option<(CodexAuth, AgentIdentityBinding)> {
        let Some(auth) = self.auth_manager.auth().await else {
            debug!("skipping agent identity flow because no auth is available");
            return None;
        };

        let binding =
            AgentIdentityBinding::from_auth(&auth, self.auth_manager.forced_chatgpt_workspace_id());
        if binding.is_none() {
            debug!("skipping agent identity flow because ChatGPT auth is unavailable");
        }
        binding.map(|binding| (auth, binding))
    }

    async fn register_agent_identity(
        &self,
        binding: &AgentIdentityBinding,
    ) -> Result<StoredAgentIdentity> {
        let key_material = generate_agent_key_material()?;
        let request_body = RegisterAgentRequest {
            abom: self.abom.clone(),
            agent_public_key: key_material.public_key_ssh.clone(),
            capabilities: Vec::new(),
        };

        let url = agent_registration_url(&self.chatgpt_base_url);
        let human_biscuit = self.mint_human_biscuit(binding, "POST", &url).await?;
        let client = create_client();
        let response = client
            .post(&url)
            .header("X-OpenAI-Authorization", human_biscuit)
            .json(&request_body)
            .timeout(AGENT_REGISTRATION_TIMEOUT)
            .send()
            .await
            .with_context(|| {
                format!("failed to send agent identity registration request to {url}")
            })?;

        if response.status().is_success() {
            let response_body = response
                .json::<RegisterAgentResponse>()
                .await
                .with_context(|| format!("failed to parse agent identity response from {url}"))?;
            let stored_identity = StoredAgentIdentity {
                binding_id: binding.binding_id.clone(),
                chatgpt_account_id: binding.chatgpt_account_id.clone(),
                chatgpt_user_id: binding.chatgpt_user_id.clone(),
                agent_runtime_id: response_body.agent_runtime_id,
                private_key_pkcs8_base64: key_material.private_key_pkcs8_base64,
                public_key_ssh: key_material.public_key_ssh,
                registered_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
                abom: self.abom.clone(),
            };
            info!(
                agent_runtime_id = %stored_identity.agent_runtime_id,
                binding_id = %binding.binding_id,
                "registered agent identity"
            );
            return Ok(stored_identity);
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("agent identity registration failed with status {status} from {url}: {body}")
    }

    async fn mint_human_biscuit(
        &self,
        binding: &AgentIdentityBinding,
        target_method: &str,
        target_url: &str,
    ) -> Result<String> {
        let url = agent_identity_biscuit_url(&self.chatgpt_base_url);
        let request_id = agent_identity_request_id()?;
        let client = create_client();
        let response = client
            .get(&url)
            .bearer_auth(&binding.access_token)
            .header("X-Request-Id", request_id.clone())
            .header("X-Original-Method", target_method)
            .header("X-Original-Url", target_url)
            .timeout(AGENT_IDENTITY_BISCUIT_TIMEOUT)
            .send()
            .await
            .with_context(|| format!("failed to send agent identity biscuit request to {url}"))?;

        if response.status().is_success() {
            let human_biscuit = response
                .headers()
                .get("x-openai-authorization")
                .context("agent identity biscuit response did not include x-openai-authorization")?
                .to_str()
                .context("agent identity biscuit response header was not valid UTF-8")?
                .to_string();
            info!(
                request_id = %request_id,
                "minted human biscuit for agent identity registration"
            );
            return Ok(human_biscuit);
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "agent identity biscuit minting failed with status {status} from {url}: {body}"
        )
    }

    fn load_stored_identity(
        &self,
        auth: &CodexAuth,
        binding: &AgentIdentityBinding,
    ) -> Result<Option<StoredAgentIdentity>> {
        let Some(record) = auth.get_agent_identity(&binding.chatgpt_account_id) else {
            return Ok(None);
        };

        let stored_identity =
            match StoredAgentIdentity::from_auth_record(binding, record, self.abom.clone()) {
                Ok(stored_identity) => stored_identity,
                Err(error) => {
                    warn!(
                        binding_id = %binding.binding_id,
                        error = %error,
                        "stored agent identity is invalid; deleting cached value"
                    );
                    auth.remove_agent_identity()?;
                    return Ok(None);
                }
            };

        if !stored_identity.matches_binding(binding) {
            warn!(
                binding_id = %binding.binding_id,
                "stored agent identity binding no longer matches current auth; deleting cached value"
            );
            auth.remove_agent_identity()?;
            return Ok(None);
        }

        if let Err(error) = stored_identity.validate_key_material() {
            warn!(
                agent_runtime_id = %stored_identity.agent_runtime_id,
                binding_id = %binding.binding_id,
                error = %error,
                "stored agent identity key material is invalid; deleting cached value"
            );
            auth.remove_agent_identity()?;
            return Ok(None);
        }

        Ok(Some(stored_identity))
    }

    fn store_identity(
        &self,
        auth: &CodexAuth,
        stored_identity: &StoredAgentIdentity,
    ) -> Result<()> {
        auth.set_agent_identity(stored_identity.to_auth_record())?;
        Ok(())
    }

    #[cfg(test)]
    fn new_for_tests(
        auth_manager: Arc<AuthManager>,
        feature_enabled: bool,
        chatgpt_base_url: String,
        session_source: SessionSource,
    ) -> Self {
        Self {
            auth_manager,
            chatgpt_base_url,
            feature_enabled,
            abom: build_abom(session_source),
            ensure_lock: Arc::new(Mutex::new(())),
        }
    }
}

impl StoredAgentIdentity {
    fn from_auth_record(
        binding: &AgentIdentityBinding,
        record: AgentIdentityAuthRecord,
        abom: AgentBillOfMaterials,
    ) -> Result<Self> {
        if record.workspace_id != binding.chatgpt_account_id {
            anyhow::bail!(
                "stored agent identity workspace {:?} does not match current workspace {:?}",
                record.workspace_id,
                binding.chatgpt_account_id
            );
        }
        let signing_key = signing_key_from_private_key_pkcs8_base64(&record.agent_private_key)?;
        Ok(Self {
            binding_id: binding.binding_id.clone(),
            chatgpt_account_id: binding.chatgpt_account_id.clone(),
            chatgpt_user_id: record.chatgpt_user_id,
            agent_runtime_id: record.agent_runtime_id,
            private_key_pkcs8_base64: record.agent_private_key,
            public_key_ssh: encode_ssh_ed25519_public_key(&signing_key.verifying_key()),
            registered_at: record.registered_at,
            abom,
        })
    }

    fn to_auth_record(&self) -> AgentIdentityAuthRecord {
        AgentIdentityAuthRecord {
            workspace_id: self.chatgpt_account_id.clone(),
            chatgpt_user_id: self.chatgpt_user_id.clone(),
            agent_runtime_id: self.agent_runtime_id.clone(),
            agent_private_key: self.private_key_pkcs8_base64.clone(),
            registered_at: self.registered_at.clone(),
        }
    }

    fn matches_binding(&self, binding: &AgentIdentityBinding) -> bool {
        self.binding_id == binding.binding_id
            && self.chatgpt_account_id == binding.chatgpt_account_id
            && match binding.chatgpt_user_id.as_deref() {
                Some(chatgpt_user_id) => self.chatgpt_user_id.as_deref() == Some(chatgpt_user_id),
                None => true,
            }
    }

    fn validate_key_material(&self) -> Result<()> {
        let signing_key = self.signing_key()?;
        let derived_public_key = encode_ssh_ed25519_public_key(&signing_key.verifying_key());
        anyhow::ensure!(
            self.public_key_ssh == derived_public_key,
            "stored public key does not match the private key"
        );
        Ok(())
    }

    pub(crate) fn signing_key(&self) -> Result<SigningKey> {
        signing_key_from_private_key_pkcs8_base64(&self.private_key_pkcs8_base64)
    }
}

impl AgentIdentityBinding {
    fn from_auth(auth: &CodexAuth, forced_workspace_id: Option<String>) -> Option<Self> {
        if !auth.is_chatgpt_auth() {
            return None;
        }

        let token_data = auth.get_token_data().ok()?;
        let resolved_account_id =
            forced_workspace_id
                .filter(|value| !value.is_empty())
                .or(token_data
                    .account_id
                    .clone()
                    .filter(|value| !value.is_empty()))?;

        Some(Self {
            binding_id: format!("chatgpt-account-{resolved_account_id}"),
            chatgpt_account_id: resolved_account_id,
            chatgpt_user_id: token_data
                .id_token
                .chatgpt_user_id
                .filter(|value| !value.is_empty()),
            access_token: token_data.access_token,
        })
    }
}

fn build_abom(session_source: SessionSource) -> AgentBillOfMaterials {
    AgentBillOfMaterials {
        agent_version: env!("CARGO_PKG_VERSION").to_string(),
        agent_harness_id: match &session_source {
            SessionSource::VSCode => "codex-app".to_string(),
            SessionSource::Cli
            | SessionSource::Exec
            | SessionSource::Mcp
            | SessionSource::Custom(_)
            | SessionSource::SubAgent(_)
            | SessionSource::Unknown => "codex-cli".to_string(),
        },
        running_location: format!("{}-{}", session_source, std::env::consts::OS),
    }
}

fn generate_agent_key_material() -> Result<GeneratedAgentKeyMaterial> {
    let mut secret_key_bytes = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut secret_key_bytes)
        .context("failed to generate agent identity private key bytes")?;
    let signing_key = SigningKey::from_bytes(&secret_key_bytes);
    let private_key_pkcs8 = signing_key
        .to_pkcs8_der()
        .context("failed to encode agent identity private key as PKCS#8")?;

    Ok(GeneratedAgentKeyMaterial {
        private_key_pkcs8_base64: BASE64_STANDARD.encode(private_key_pkcs8.as_bytes()),
        public_key_ssh: encode_ssh_ed25519_public_key(&signing_key.verifying_key()),
    })
}

fn encode_ssh_ed25519_public_key(verifying_key: &VerifyingKey) -> String {
    let mut blob = Vec::with_capacity(4 + 11 + 4 + 32);
    append_ssh_string(&mut blob, b"ssh-ed25519");
    append_ssh_string(&mut blob, verifying_key.as_bytes());
    format!("ssh-ed25519 {}", BASE64_STANDARD.encode(blob))
}

fn append_ssh_string(buf: &mut Vec<u8>, value: &[u8]) {
    buf.extend_from_slice(&(value.len() as u32).to_be_bytes());
    buf.extend_from_slice(value);
}

fn agent_registration_url(chatgpt_base_url: &str) -> String {
    let trimmed = chatgpt_base_url.trim_end_matches('/');
    format!("{trimmed}/v1/agent/register")
}

fn signing_key_from_private_key_pkcs8_base64(private_key_pkcs8_base64: &str) -> Result<SigningKey> {
    let private_key = BASE64_STANDARD
        .decode(private_key_pkcs8_base64)
        .context("stored agent identity private key is not valid base64")?;
    SigningKey::from_pkcs8_der(&private_key)
        .context("stored agent identity private key is not valid PKCS#8")
}

fn agent_identity_biscuit_url(chatgpt_base_url: &str) -> String {
    let trimmed = chatgpt_base_url.trim_end_matches('/');
    format!("{trimmed}/authenticate_app_v2")
}

fn agent_identity_request_id() -> Result<String> {
    let mut request_id_bytes = [0u8; 16];
    OsRng
        .try_fill_bytes(&mut request_id_bytes)
        .context("failed to generate agent identity request id")?;
    Ok(format!(
        "codex-agent-identity-{}",
        URL_SAFE_NO_PAD.encode(request_id_bytes)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[tokio::test]
    async fn ensure_registered_identity_skips_when_feature_is_disabled() {
        let auth_manager =
            AuthManager::from_auth_for_testing(make_chatgpt_auth("account-123", Some("user-123")));
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ false,
            "https://chatgpt.com/backend-api/".to_string(),
            SessionSource::Cli,
        );

        assert_eq!(manager.ensure_registered_identity().await.unwrap(), None);
    }

    #[tokio::test]
    async fn ensure_registered_identity_skips_for_api_key_auth() {
        let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test-key"));
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ true,
            "https://chatgpt.com/backend-api/".to_string(),
            SessionSource::Cli,
        );

        assert_eq!(manager.ensure_registered_identity().await.unwrap(), None);
    }

    #[tokio::test]
    async fn ensure_registered_identity_registers_and_reuses_cached_identity() {
        let server = MockServer::start().await;
        let chatgpt_base_url = server.uri();
        mount_human_biscuit(&server, &chatgpt_base_url).await;
        Mock::given(method("POST"))
            .and(path("/v1/agent/register"))
            .and(header("x-openai-authorization", "human-biscuit"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "agent_runtime_id": "agent_123",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let auth_manager =
            AuthManager::from_auth_for_testing(make_chatgpt_auth("account-123", Some("user-123")));
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ true,
            chatgpt_base_url,
            SessionSource::Cli,
        );

        let first = manager
            .ensure_registered_identity()
            .await
            .unwrap()
            .expect("identity should be registered");
        let second = manager
            .ensure_registered_identity()
            .await
            .unwrap()
            .expect("identity should be reused");

        assert_eq!(first.agent_runtime_id, "agent_123");
        assert_eq!(first, second);
        assert_eq!(first.abom.agent_harness_id, "codex-cli");
        assert_eq!(first.chatgpt_account_id, "account-123");
        assert_eq!(first.chatgpt_user_id.as_deref(), Some("user-123"));
    }

    #[tokio::test]
    async fn ensure_registered_identity_deletes_invalid_cached_identity_and_reregisters() {
        let server = MockServer::start().await;
        let chatgpt_base_url = server.uri();
        mount_human_biscuit(&server, &chatgpt_base_url).await;
        Mock::given(method("POST"))
            .and(path("/v1/agent/register"))
            .and(header("x-openai-authorization", "human-biscuit"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "agent_runtime_id": "agent_456",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let auth = make_chatgpt_auth("account-123", Some("user-123"));
        let auth_manager = AuthManager::from_auth_for_testing(auth.clone());
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ true,
            chatgpt_base_url,
            SessionSource::Cli,
        );

        let binding =
            AgentIdentityBinding::from_auth(&auth, /*forced_workspace_id*/ None).expect("binding");
        auth.set_agent_identity(AgentIdentityAuthRecord {
            workspace_id: "account-123".to_string(),
            chatgpt_user_id: Some("user-123".to_string()),
            agent_runtime_id: "agent_invalid".to_string(),
            agent_private_key: "not-valid-base64".to_string(),
            registered_at: "2026-01-01T00:00:00Z".to_string(),
        })
        .expect("seed invalid identity");

        let stored = manager
            .ensure_registered_identity()
            .await
            .unwrap()
            .expect("identity should be registered");

        assert_eq!(stored.agent_runtime_id, "agent_456");
        let persisted = auth
            .get_agent_identity(&binding.chatgpt_account_id)
            .expect("stored identity");
        assert_eq!(persisted.agent_runtime_id, "agent_456");
    }

    #[tokio::test]
    async fn ensure_registered_identity_deletes_different_user_identity_and_reregisters() {
        let server = MockServer::start().await;
        let chatgpt_base_url = server.uri();
        mount_human_biscuit(&server, &chatgpt_base_url).await;
        Mock::given(method("POST"))
            .and(path("/v1/agent/register"))
            .and(header("x-openai-authorization", "human-biscuit"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "agent_runtime_id": "agent_new",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let auth = make_chatgpt_auth("account-123", Some("user-new"));
        let stale_key = generate_agent_key_material().expect("key material");
        auth.set_agent_identity(AgentIdentityAuthRecord {
            workspace_id: "account-123".to_string(),
            chatgpt_user_id: Some("user-old".to_string()),
            agent_runtime_id: "agent_old".to_string(),
            agent_private_key: stale_key.private_key_pkcs8_base64,
            registered_at: "2026-01-01T00:00:00Z".to_string(),
        })
        .expect("seed stale identity");

        let auth_manager = AuthManager::from_auth_for_testing(auth.clone());
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ true,
            chatgpt_base_url,
            SessionSource::Cli,
        );

        let stored = manager
            .ensure_registered_identity()
            .await
            .unwrap()
            .expect("identity should be registered");

        assert_eq!(stored.agent_runtime_id, "agent_new");
        assert_eq!(stored.chatgpt_user_id.as_deref(), Some("user-new"));
        let persisted = auth
            .get_agent_identity("account-123")
            .expect("stored identity");
        assert_eq!(persisted.agent_runtime_id, "agent_new");
        assert_eq!(persisted.chatgpt_user_id.as_deref(), Some("user-new"));
    }

    #[tokio::test]
    async fn ensure_registered_identity_uses_chatgpt_base_url() {
        let server = MockServer::start().await;
        let chatgpt_base_url = format!("{}/backend-api", server.uri());
        mount_human_biscuit(&server, &chatgpt_base_url).await;
        Mock::given(method("POST"))
            .and(path("/backend-api/v1/agent/register"))
            .and(header("x-openai-authorization", "human-biscuit"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "agent_runtime_id": "agent_canonical",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let auth_manager =
            AuthManager::from_auth_for_testing(make_chatgpt_auth("account-123", Some("user-123")));
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ true,
            chatgpt_base_url,
            SessionSource::Cli,
        );

        let stored = manager
            .ensure_registered_identity()
            .await
            .unwrap()
            .expect("identity should be registered");
        assert_eq!(stored.agent_runtime_id, "agent_canonical");
    }

    async fn mount_human_biscuit(server: &MockServer, chatgpt_base_url: &str) {
        let biscuit_url = agent_identity_biscuit_url(chatgpt_base_url);
        let biscuit_path = reqwest::Url::parse(&biscuit_url)
            .expect("biscuit URL parses")
            .path()
            .to_string();
        let target_url = agent_registration_url(chatgpt_base_url);
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

    #[test]
    fn encode_ssh_ed25519_public_key_matches_expected_wire_shape() {
        let key_material = generate_agent_key_material().expect("key material");
        let (_, encoded_blob) = key_material
            .public_key_ssh
            .split_once(' ')
            .expect("public key contains scheme");
        let decoded = BASE64_STANDARD.decode(encoded_blob).expect("base64");

        assert_eq!(&decoded[..4], 11u32.to_be_bytes().as_slice());
        assert_eq!(&decoded[4..15], b"ssh-ed25519");
        assert_eq!(&decoded[15..19], 32u32.to_be_bytes().as_slice());
        assert_eq!(decoded.len(), 51);
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
