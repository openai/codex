use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::SecondsFormat;
use chrono::Utc;
use codex_protocol::protocol::SessionSource;
use crypto_box::SecretKey as Curve25519SecretKey;
use ed25519_dalek::Signer as _;
use ed25519_dalek::SigningKey;
use ed25519_dalek::VerifyingKey;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::pkcs8::EncodePrivateKey;
use rand::TryRngCore;
use rand::rngs::OsRng;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest as _;
use sha2::Sha512;
use tokio::sync::Mutex;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::AgentIdentityAuthRecord;
use crate::AuthManager;
use crate::CodexAuth;
use crate::default_client::create_client;

const AGENT_REGISTRATION_TIMEOUT: Duration = Duration::from_secs(15);
const AGENT_TASK_REGISTRATION_TIMEOUT: Duration = Duration::from_secs(15);
const AGENT_IDENTITY_BISCUIT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub(crate) struct BackgroundAgentTaskManager {
    auth_manager: Arc<AuthManager>,
    chatgpt_base_url: String,
    auth_mode: BackgroundAgentTaskAuthMode,
    abom: AgentBillOfMaterials,
    ensure_lock: Arc<Mutex<()>>,
}

impl std::fmt::Debug for BackgroundAgentTaskManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackgroundAgentTaskManager")
            .field("chatgpt_base_url", &self.chatgpt_base_url)
            .field("auth_mode", &self.auth_mode)
            .field("abom", &self.abom)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BackgroundAgentTaskAuthMode {
    Enabled,
    #[default]
    Disabled,
}

impl BackgroundAgentTaskAuthMode {
    pub fn from_feature_enabled(enabled: bool) -> Self {
        if enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }

    fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct StoredAgentIdentity {
    binding_id: String,
    chatgpt_account_id: String,
    chatgpt_user_id: Option<String>,
    agent_runtime_id: String,
    private_key_pkcs8_base64: String,
    public_key_ssh: String,
    registered_at: String,
    background_task_id: Option<String>,
    abom: AgentBillOfMaterials,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct AgentBillOfMaterials {
    agent_version: String,
    agent_harness_id: String,
    running_location: String,
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

#[derive(Debug, Serialize)]
struct RegisterTaskRequest {
    signature: String,
    timestamp: String,
}

#[derive(Debug, Deserialize)]
struct RegisterTaskResponse {
    encrypted_task_id: String,
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

impl BackgroundAgentTaskManager {
    #[cfg(test)]
    pub(crate) fn new(
        auth_manager: Arc<AuthManager>,
        chatgpt_base_url: String,
        session_source: SessionSource,
    ) -> Self {
        Self::new_with_auth_mode(
            auth_manager,
            chatgpt_base_url,
            session_source,
            BackgroundAgentTaskAuthMode::Disabled,
        )
    }

    pub(crate) fn new_with_auth_mode(
        auth_manager: Arc<AuthManager>,
        chatgpt_base_url: String,
        session_source: SessionSource,
        auth_mode: BackgroundAgentTaskAuthMode,
    ) -> Self {
        Self {
            auth_manager,
            chatgpt_base_url: normalize_chatgpt_base_url(&chatgpt_base_url),
            auth_mode,
            abom: build_abom(session_source),
            ensure_lock: Arc::new(Mutex::new(())),
        }
    }

    pub(crate) async fn authorization_header_value_for_auth(
        &self,
        auth: &CodexAuth,
    ) -> Result<Option<String>> {
        if !self.auth_mode.is_enabled() {
            debug!("skipping background agent task auth because agent identity is disabled");
            return Ok(None);
        }

        if !supports_background_agent_task_auth(&self.chatgpt_base_url) {
            debug!(
                chatgpt_base_url = %self.chatgpt_base_url,
                "skipping background agent task auth for unsupported backend host"
            );
            return Ok(None);
        }

        let Some(binding) =
            AgentIdentityBinding::from_auth(auth, self.auth_manager.forced_chatgpt_workspace_id())
        else {
            debug!("skipping background agent task auth because ChatGPT auth is unavailable");
            return Ok(None);
        };

        let _guard = self.ensure_lock.lock().await;
        let mut stored_identity = self
            .ensure_registered_identity_for_binding(auth, &binding)
            .await?;
        let background_task_id = match stored_identity.background_task_id.clone() {
            Some(background_task_id) => background_task_id,
            _ => {
                let background_task_id = self
                    .register_background_task_for_identity(&binding, &stored_identity)
                    .await?;
                stored_identity.background_task_id = Some(background_task_id.clone());
                self.store_identity(auth, &stored_identity)?;
                background_task_id
            }
        };

        Ok(Some(authorization_header_for_task(
            &stored_identity,
            &background_task_id,
        )?))
    }

    pub(crate) async fn authorization_header_value_or_bearer(
        &self,
        auth: &CodexAuth,
    ) -> Option<String> {
        match self.authorization_header_value_for_auth(auth).await {
            Ok(Some(authorization_header_value)) => Some(authorization_header_value),
            Ok(None) => auth
                .get_token()
                .ok()
                .filter(|token| !token.is_empty())
                .map(|token| format!("Bearer {token}")),
            Err(error) => {
                warn!(
                    error = %error,
                    "falling back to bearer authorization because background agent task auth failed"
                );
                auth.get_token()
                    .ok()
                    .filter(|token| !token.is_empty())
                    .map(|token| format!("Bearer {token}"))
            }
        }
    }

    async fn ensure_registered_identity_for_binding(
        &self,
        auth: &CodexAuth,
        binding: &AgentIdentityBinding,
    ) -> Result<StoredAgentIdentity> {
        if let Some(stored_identity) = self.load_stored_identity(auth, binding)? {
            return Ok(stored_identity);
        }

        let stored_identity = self.register_agent_identity(binding).await?;
        self.store_identity(auth, &stored_identity)?;
        Ok(stored_identity)
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
                background_task_id: None,
                abom: self.abom.clone(),
            };
            info!(
                agent_runtime_id = %stored_identity.agent_runtime_id,
                binding_id = %binding.binding_id,
                "registered background agent identity"
            );
            return Ok(stored_identity);
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("agent identity registration failed with status {status} from {url}: {body}")
    }

    async fn register_background_task_for_identity(
        &self,
        binding: &AgentIdentityBinding,
        stored_identity: &StoredAgentIdentity,
    ) -> Result<String> {
        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let request_body = RegisterTaskRequest {
            signature: sign_task_registration_payload(stored_identity, &timestamp)?,
            timestamp,
        };

        let client = create_client();
        let url =
            agent_task_registration_url(&self.chatgpt_base_url, &stored_identity.agent_runtime_id);
        let human_biscuit = self.mint_human_biscuit(binding, "POST", &url).await?;
        let response = client
            .post(&url)
            .header("X-OpenAI-Authorization", human_biscuit)
            .json(&request_body)
            .timeout(AGENT_TASK_REGISTRATION_TIMEOUT)
            .send()
            .await
            .with_context(|| format!("failed to send background agent task request to {url}"))?;

        if response.status().is_success() {
            let response_body = response
                .json::<RegisterTaskResponse>()
                .await
                .with_context(|| format!("failed to parse background task response from {url}"))?;
            let background_task_id =
                decrypt_task_id_response(stored_identity, &response_body.encrypted_task_id)?;
            info!(
                agent_runtime_id = %stored_identity.agent_runtime_id,
                task_id = %background_task_id,
                "registered background agent task"
            );
            return Ok(background_task_id);
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "background agent task registration failed with status {status} from {url}: {body}"
        )
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
                "minted human biscuit for background agent task"
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
}

pub fn cached_background_agent_task_authorization_header_value(
    auth: &CodexAuth,
    auth_mode: BackgroundAgentTaskAuthMode,
) -> Result<Option<String>> {
    if !auth_mode.is_enabled() {
        return Ok(None);
    }

    let Some(binding) = AgentIdentityBinding::from_auth(auth, /*forced_workspace_id*/ None) else {
        return Ok(None);
    };
    let Some(record) = auth.get_agent_identity(&binding.chatgpt_account_id) else {
        return Ok(None);
    };
    let stored_identity =
        StoredAgentIdentity::from_auth_record(&binding, record, build_abom(SessionSource::Cli))?;
    if !stored_identity.matches_binding(&binding) {
        return Ok(None);
    }
    stored_identity.validate_key_material()?;
    let Some(background_task_id) = stored_identity.background_task_id.as_ref() else {
        return Ok(None);
    };
    authorization_header_for_task(&stored_identity, background_task_id).map(Some)
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
            agent_runtime_id: record.agent_runtime_id.clone(),
            private_key_pkcs8_base64: record.agent_private_key,
            public_key_ssh: encode_ssh_ed25519_public_key(&signing_key.verifying_key()),
            registered_at: record.registered_at,
            background_task_id: record.background_task_id,
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
            background_task_id: self.background_task_id.clone(),
        }
    }

    fn matches_binding(&self, binding: &AgentIdentityBinding) -> bool {
        binding.matches_parts(
            &self.binding_id,
            &self.chatgpt_account_id,
            self.chatgpt_user_id.as_deref(),
        )
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

    fn signing_key(&self) -> Result<SigningKey> {
        signing_key_from_private_key_pkcs8_base64(&self.private_key_pkcs8_base64)
    }
}

impl AgentIdentityBinding {
    fn matches_parts(
        &self,
        binding_id: &str,
        chatgpt_account_id: &str,
        chatgpt_user_id: Option<&str>,
    ) -> bool {
        binding_id == self.binding_id
            && chatgpt_account_id == self.chatgpt_account_id
            && match self.chatgpt_user_id.as_deref() {
                Some(expected_user_id) => chatgpt_user_id == Some(expected_user_id),
                None => true,
            }
    }

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

fn authorization_header_for_task(
    stored_identity: &StoredAgentIdentity,
    background_task_id: &str,
) -> Result<String> {
    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let signature = sign_agent_assertion_payload(stored_identity, background_task_id, &timestamp)?;
    let payload = serde_json::to_vec(&BTreeMap::from([
        (
            "agent_runtime_id",
            stored_identity.agent_runtime_id.as_str(),
        ),
        ("signature", signature.as_str()),
        ("task_id", background_task_id),
        ("timestamp", timestamp.as_str()),
    ]))
    .context("failed to serialize agent assertion envelope")?;
    Ok(format!(
        "AgentAssertion {}",
        URL_SAFE_NO_PAD.encode(payload)
    ))
}

fn sign_agent_assertion_payload(
    stored_identity: &StoredAgentIdentity,
    background_task_id: &str,
    timestamp: &str,
) -> Result<String> {
    let signing_key = stored_identity.signing_key()?;
    let payload = format!(
        "{}:{background_task_id}:{timestamp}",
        stored_identity.agent_runtime_id
    );
    Ok(BASE64_STANDARD.encode(signing_key.sign(payload.as_bytes()).to_bytes()))
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

fn signing_key_from_private_key_pkcs8_base64(private_key_pkcs8_base64: &str) -> Result<SigningKey> {
    let private_key = BASE64_STANDARD
        .decode(private_key_pkcs8_base64)
        .context("stored agent identity private key is not valid base64")?;
    SigningKey::from_pkcs8_der(&private_key)
        .context("stored agent identity private key is not valid PKCS#8")
}

fn agent_registration_url(chatgpt_base_url: &str) -> String {
    let trimmed = chatgpt_base_url.trim_end_matches('/');
    format!("{trimmed}/v1/agent/register")
}

fn agent_task_registration_url(chatgpt_base_url: &str, agent_runtime_id: &str) -> String {
    let trimmed = chatgpt_base_url.trim_end_matches('/');
    format!("{trimmed}/v1/agent/{agent_runtime_id}/task/register")
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

fn normalize_chatgpt_base_url(chatgpt_base_url: &str) -> String {
    let mut base_url = chatgpt_base_url.trim_end_matches('/').to_string();
    for suffix in [
        "/wham/remote/control/server/enroll",
        "/wham/remote/control/server",
    ] {
        if let Some(stripped) = base_url.strip_suffix(suffix) {
            base_url = stripped.to_string();
            break;
        }
    }
    if (base_url.starts_with("https://chatgpt.com")
        || base_url.starts_with("https://chat.openai.com"))
        && !base_url.contains("/backend-api")
    {
        base_url = format!("{base_url}/backend-api");
    }
    if let Some(stripped) = base_url.strip_suffix("/codex") {
        stripped.to_string()
    } else {
        base_url
    }
}

fn supports_background_agent_task_auth(chatgpt_base_url: &str) -> bool {
    let Ok(url) = url::Url::parse(chatgpt_base_url) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    host == "chatgpt.com"
        || host == "chat.openai.com"
        || host == "chatgpt-staging.com"
        || host.ends_with(".chatgpt.com")
        || host.ends_with(".chatgpt-staging.com")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disabled_background_agent_task_auth_returns_none_for_supported_host() {
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let auth_manager = AuthManager::from_auth_for_testing(auth.clone());
        let manager = BackgroundAgentTaskManager::new_with_auth_mode(
            auth_manager,
            "https://chatgpt.com/backend-api".to_string(),
            SessionSource::Cli,
            BackgroundAgentTaskAuthMode::Disabled,
        );

        let authorization_header_value = manager
            .authorization_header_value_for_auth(&auth)
            .await
            .expect("disabled manager should not fail");

        assert_eq!(None, authorization_header_value);
    }

    #[tokio::test]
    async fn default_background_agent_task_auth_returns_none_for_supported_host() {
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let auth_manager = AuthManager::from_auth_for_testing(auth.clone());
        let manager = BackgroundAgentTaskManager::new(
            auth_manager,
            "https://chatgpt.com/backend-api".to_string(),
            SessionSource::Cli,
        );

        let authorization_header_value = manager
            .authorization_header_value_for_auth(&auth)
            .await
            .expect("default manager should not fail");

        assert_eq!(None, authorization_header_value);
    }

    #[test]
    fn cached_background_agent_task_auth_honors_disabled_mode() {
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let key_material = generate_agent_key_material().expect("generate key material");
        auth.set_agent_identity(AgentIdentityAuthRecord {
            workspace_id: "account_id".to_string(),
            chatgpt_user_id: None,
            agent_runtime_id: "agent_123".to_string(),
            agent_private_key: key_material.private_key_pkcs8_base64,
            registered_at: "2026-04-13T12:00:00Z".to_string(),
            background_task_id: Some("task_123".to_string()),
        })
        .expect("set agent identity");

        let disabled_authorization_header_value =
            cached_background_agent_task_authorization_header_value(
                &auth,
                BackgroundAgentTaskAuthMode::Disabled,
            )
            .expect("disabled cached auth should not fail");
        let enabled_authorization_header_value =
            cached_background_agent_task_authorization_header_value(
                &auth,
                BackgroundAgentTaskAuthMode::Enabled,
            )
            .expect("enabled cached auth should not fail");

        assert_eq!(None, disabled_authorization_header_value);
        assert!(enabled_authorization_header_value.is_some());
    }
}
