use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::SecondsFormat;
use chrono::Utc;
use crypto_box::SecretKey as CryptoBoxSecretKey;
use ed25519_dalek::Signature;
use ed25519_dalek::Signer;
use ed25519_dalek::SigningKey;
use ed25519_dalek::VerifyingKey;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::pkcs8::EncodePrivateKey;
use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing::info;
use tracing::trace;
use tracing::warn;

use crate::AuthManager;
use crate::CodexAuth;
use crate::config::Config;
use crate::default_client::build_reqwest_client;
use crate::default_client::originator;
use crate::features::Feature;
use codex_secrets::SecretName;
use codex_secrets::SecretScope;
use codex_secrets::SecretsBackendKind;
use codex_secrets::SecretsManager;

const AGENT_IDENTITY_SECRET_NAME: &str = "AGENT_IDENTITY";

#[derive(Clone)]
pub(crate) struct AgentIdentityManager {
    auth_manager: Arc<AuthManager>,
    http_client: Client,
    secrets: SecretsManager,
}

impl AgentIdentityManager {
    pub(crate) fn new(auth_manager: Arc<AuthManager>, codex_home: PathBuf) -> Self {
        Self {
            auth_manager,
            http_client: build_reqwest_client(),
            secrets: SecretsManager::new(codex_home, SecretsBackendKind::Local),
        }
    }

    pub(crate) async fn ensure_thread_task(
        &self,
        config: &Config,
        thread_id: &str,
    ) -> Result<Option<String>> {
        if !config.features.enabled(Feature::UseAgentIdentity) {
            return Ok(None);
        }

        let Some(auth) = self.auth_manager.auth().await else {
            return Ok(None);
        };
        if !auth.is_chatgpt_auth() {
            return Ok(None);
        }

        let binding_id = binding_id_for_auth(
            self.auth_manager.forced_chatgpt_workspace_id(),
            auth.get_account_id(),
        )
        .context("agent identity requires a ChatGPT workspace/account binding")?;

        let identity = match self.ensure_agent_identity(config, &auth, &binding_id).await {
            Ok(identity) => identity,
            Err(err) => {
                warn!(binding_id = %binding_id, "failed to ensure agent identity: {err:#}");
                return Err(err);
            }
        };

        match self
            .register_task(config, &auth, &identity, thread_id)
            .await
        {
            Ok(task_id) => Ok(Some(task_id)),
            Err(err) => {
                warn!(
                    binding_id = %binding_id,
                    "agent task registration failed, deleting stored identity and retrying once: {err:#}"
                );
                self.delete_stored_identity(&binding_id)?;
                let identity = self
                    .register_agent_identity(config, &auth, &binding_id)
                    .await
                    .context("re-registering agent identity after task failure")?;
                let task_id = self
                    .register_task(config, &auth, &identity, thread_id)
                    .await
                    .context("registering agent task after agent identity retry")?;
                Ok(Some(task_id))
            }
        }
    }

    async fn ensure_agent_identity(
        &self,
        config: &Config,
        auth: &CodexAuth,
        binding_id: &str,
    ) -> Result<StoredAgentIdentity> {
        if let Some(identity) = self.load_stored_identity(binding_id)? {
            trace!(binding_id = %binding_id, "reusing stored agent identity");
            return Ok(identity);
        }

        self.register_agent_identity(config, auth, binding_id).await
    }

    async fn register_agent_identity(
        &self,
        config: &Config,
        auth: &CodexAuth,
        binding_id: &str,
    ) -> Result<StoredAgentIdentity> {
        let key_material = generate_key_material()?;
        let body = AgentRegisterRequest {
            agent_public_key: key_material.agent_public_key.clone(),
            abom: build_abom(),
            capabilities: vec!["codex_backend".to_string(), "connector_gateway".to_string()],
        };
        let response: AgentRegisterResponse = self
            .post_json(agent_register_url(&config.chatgpt_base_url), auth, &body)
            .await
            .context("registering agent identity")?;

        let identity = StoredAgentIdentity {
            binding_id: binding_id.to_string(),
            agent_runtime_id: response.agent_runtime_id,
            private_key_pkcs8_base64: key_material.private_key_pkcs8_base64,
            agent_public_key: body.agent_public_key,
            registered_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            abom: body.abom,
        };
        self.store_identity(&identity)?;
        info!(binding_id = %binding_id, "agent identity registration succeeded");
        Ok(identity)
    }

    async fn register_task(
        &self,
        config: &Config,
        auth: &CodexAuth,
        identity: &StoredAgentIdentity,
        thread_id: &str,
    ) -> Result<String> {
        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let payload = canonical_signing_payload(&identity.agent_runtime_id, &timestamp);
        let signature = sign_payload(&identity.private_key_pkcs8_base64, payload.as_bytes())?;
        let body = TaskRegisterRequest {
            timestamp,
            signature,
        };
        let response: TaskRegisterResponse = self
            .post_json(
                task_register_url(&config.chatgpt_base_url, &identity.agent_runtime_id),
                auth,
                &body,
            )
            .await
            .context("registering agent task")?;
        let task_id = decrypt_task_id(response, &identity.private_key_pkcs8_base64)?;
        info!(thread_id = %thread_id, "agent task registration succeeded");
        Ok(task_id)
    }

    async fn post_json<TReq, TResp>(
        &self,
        url: String,
        auth: &CodexAuth,
        body: &TReq,
    ) -> Result<TResp>
    where
        TReq: Serialize + ?Sized,
        TResp: DeserializeOwned,
    {
        let token = auth
            .get_token()
            .context("loading ChatGPT access token for agent identity request")?;
        let mut request = self.http_client.post(url).bearer_auth(token).json(body);
        if let Some(account_id) = auth.get_account_id() {
            request = request.header("ChatGPT-Account-ID", account_id);
        }
        let response = request
            .send()
            .await
            .context("sending agent identity request")?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("agent identity request failed with {status}: {body}");
        }
        response
            .json::<TResp>()
            .await
            .context("decoding agent identity response")
    }

    fn load_stored_identity(&self, binding_id: &str) -> Result<Option<StoredAgentIdentity>> {
        let secret_name = secret_name()?;
        let secret_scope = secret_scope(binding_id)?;
        let Some(raw) = self.secrets.get(&secret_scope, &secret_name)? else {
            return Ok(None);
        };
        match serde_json::from_str::<StoredAgentIdentity>(&raw) {
            Ok(identity)
                if identity.binding_id == binding_id
                    && identity.agent_public_key.starts_with("ssh-ed25519 ") =>
            {
                Ok(Some(identity))
            }
            Ok(identity) if identity.binding_id == binding_id => {
                warn!(
                    binding_id = %binding_id,
                    "stored agent identity uses a legacy public key format, deleting cached value"
                );
                self.delete_stored_identity(binding_id)?;
                Ok(None)
            }
            Ok(_) => {
                warn!(binding_id = %binding_id, "stored agent identity binding mismatch, deleting cached value");
                self.delete_stored_identity(binding_id)?;
                Ok(None)
            }
            Err(err) => {
                warn!(binding_id = %binding_id, "failed to parse stored agent identity, deleting cached value: {err:#}");
                self.delete_stored_identity(binding_id)?;
                Ok(None)
            }
        }
    }

    fn store_identity(&self, identity: &StoredAgentIdentity) -> Result<()> {
        let secret_name = secret_name()?;
        let secret_scope = secret_scope(&identity.binding_id)?;
        let raw = serde_json::to_string(identity).context("serializing stored agent identity")?;
        self.secrets
            .set(&secret_scope, &secret_name, &raw)
            .context("persisting stored agent identity")
    }

    fn delete_stored_identity(&self, binding_id: &str) -> Result<()> {
        let secret_name = secret_name()?;
        let secret_scope = secret_scope(binding_id)?;
        let _ = self
            .secrets
            .delete(&secret_scope, &secret_name)
            .context("deleting stored agent identity")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct StoredAgentIdentity {
    binding_id: String,
    agent_runtime_id: String,
    private_key_pkcs8_base64: String,
    #[serde(alias = "public_key_base64")]
    agent_public_key: String,
    registered_at: String,
    abom: AgentAbom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AgentAbom {
    agent_version: String,
    agent_harness_id: String,
    running_location: String,
}

#[derive(Debug, Serialize)]
struct AgentRegisterRequest {
    agent_public_key: String,
    abom: AgentAbom,
    capabilities: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AgentRegisterResponse {
    #[serde(alias = "agent_id", alias = "runtime_identity")]
    agent_runtime_id: String,
}

#[derive(Debug, Serialize)]
struct TaskRegisterRequest {
    timestamp: String,
    signature: String,
}

#[derive(Debug, Deserialize)]
struct TaskRegisterResponse {
    #[serde(default)]
    task_id: Option<String>,
    #[serde(default)]
    encrypted_task_id: Option<String>,
}

struct GeneratedKeyMaterial {
    private_key_pkcs8_base64: String,
    agent_public_key: String,
}

fn binding_id_for_auth(
    forced_chatgpt_workspace_id: Option<String>,
    account_id: Option<String>,
) -> Option<String> {
    forced_chatgpt_workspace_id.or(account_id)
}

fn normalized_agent_identity_base_url(chatgpt_base_url: &str) -> String {
    let base_url = chatgpt_base_url.trim_end_matches('/');
    if base_url.contains("/backend-api") || base_url.contains("/api/codex") {
        base_url.to_string()
    } else {
        format!("{base_url}/backend-api")
    }
}

fn agent_register_url(chatgpt_base_url: &str) -> String {
    format!(
        "{}/v1/agent/register",
        normalized_agent_identity_base_url(chatgpt_base_url)
    )
}

fn task_register_url(chatgpt_base_url: &str, agent_runtime_id: &str) -> String {
    format!(
        "{}/v1/agent/{agent_runtime_id}/task/register",
        normalized_agent_identity_base_url(chatgpt_base_url),
    )
}

fn secret_name() -> Result<SecretName> {
    SecretName::new(AGENT_IDENTITY_SECRET_NAME).context("building agent identity secret name")
}

fn secret_scope(binding_id: &str) -> Result<SecretScope> {
    SecretScope::environment(format!("agent-identity-{binding_id}"))
        .context("building agent identity secret scope")
}

fn build_abom() -> AgentAbom {
    let os_info = os_info::get();
    AgentAbom {
        agent_version: env!("CARGO_PKG_VERSION").to_string(),
        agent_harness_id: originator().value,
        running_location: format!(
            "{}-{}",
            os_info.os_type(),
            os_info.architecture().unwrap_or("unknown")
        ),
    }
}

fn generate_key_material() -> Result<GeneratedKeyMaterial> {
    let secret_key = rand::random::<[u8; 32]>();
    let signing_key = SigningKey::from_bytes(&secret_key);
    let private_key_pkcs8_base64 = BASE64_STANDARD.encode(
        signing_key
            .to_pkcs8_der()
            .context("encoding agent identity private key")?
            .as_bytes(),
    );
    let agent_public_key = format_ssh_ed25519_public_key(&signing_key.verifying_key());
    Ok(GeneratedKeyMaterial {
        private_key_pkcs8_base64,
        agent_public_key,
    })
}

fn format_ssh_ed25519_public_key(verifying_key: &VerifyingKey) -> String {
    const KEY_TYPE: &str = "ssh-ed25519";

    let mut blob = Vec::with_capacity(4 + KEY_TYPE.len() + 4 + verifying_key.as_bytes().len());
    push_ssh_string(&mut blob, KEY_TYPE.as_bytes());
    push_ssh_string(&mut blob, verifying_key.as_bytes());
    format!("{KEY_TYPE} {}", BASE64_STANDARD.encode(blob))
}

fn push_ssh_string(buf: &mut Vec<u8>, value: &[u8]) {
    buf.extend_from_slice(&(value.len() as u32).to_be_bytes());
    buf.extend_from_slice(value);
}

fn canonical_signing_payload(agent_runtime_id: &str, timestamp: &str) -> String {
    format!("{agent_runtime_id}:{timestamp}")
}

fn decode_signing_key(private_key_pkcs8_base64: &str) -> Result<SigningKey> {
    let private_key_pkcs8_der = BASE64_STANDARD
        .decode(private_key_pkcs8_base64)
        .context("decoding agent identity private key")?;
    SigningKey::from_pkcs8_der(&private_key_pkcs8_der)
        .context("decoding agent identity private key")
}

fn sign_payload(private_key_pkcs8_base64: &str, payload: &[u8]) -> Result<String> {
    let signing_key = decode_signing_key(private_key_pkcs8_base64)?;
    let signature: Signature = signing_key.sign(payload);
    Ok(URL_SAFE_NO_PAD.encode(signature.to_bytes()))
}

fn decrypt_task_id(
    response: TaskRegisterResponse,
    private_key_pkcs8_base64: &str,
) -> Result<String> {
    if let Some(task_id) = response.task_id {
        return Ok(task_id);
    }

    let encrypted_task_id = response
        .encrypted_task_id
        .context("task register response was missing both task_id and encrypted_task_id")?;

    if let Some(task_id) = encrypted_task_id.strip_prefix("plaintext:") {
        return Ok(task_id.to_string());
    }

    let decoded = BASE64_STANDARD
        .decode(&encrypted_task_id)
        .or_else(|_| URL_SAFE_NO_PAD.decode(&encrypted_task_id))
        .context("decoding encrypted task id envelope")?;
    let signing_key = decode_signing_key(private_key_pkcs8_base64)?;
    let secret_key = CryptoBoxSecretKey::from(signing_key.to_scalar());
    let decrypted = secret_key
        .unseal(&decoded)
        .map_err(|err| anyhow::anyhow!("decrypting encrypted task id envelope: {err:?}"))?;
    String::from_utf8(decrypted).context("decoding encrypted task id UTF-8 payload")
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_keyring_store::tests::MockKeyringStore;
    use codex_secrets::SecretsManager;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::sync::Arc;

    #[test]
    fn binding_id_prefers_forced_workspace() {
        let binding = binding_id_for_auth(
            Some("workspace-123".to_string()),
            Some("account-456".to_string()),
        );
        assert_eq!(binding, Some("workspace-123".to_string()));
    }

    #[test]
    fn signing_payload_is_stable() {
        assert_eq!(
            canonical_signing_payload("agent-123", "2026-03-16T12:34:56Z"),
            "agent-123:2026-03-16T12:34:56Z".to_string()
        );
    }

    #[test]
    fn decrypt_task_id_prefers_plaintext_field() {
        let task_id = decrypt_task_id(
            TaskRegisterResponse {
                task_id: Some("task-123".to_string()),
                encrypted_task_id: Some(BASE64_STANDARD.encode("ignored")),
            },
            "ignored",
        )
        .expect("task id should decode");
        assert_eq!(task_id, "task-123".to_string());
    }

    #[test]
    fn generate_key_material_uses_ssh_public_key_format() {
        let key_material = generate_key_material().expect("key material");
        assert!(key_material.agent_public_key.starts_with("ssh-ed25519 "));
    }

    #[test]
    fn decrypt_task_id_decrypts_sealed_box_response() {
        let key_material = generate_key_material().expect("key material");
        let private_key_pkcs8_der = BASE64_STANDARD
            .decode(&key_material.private_key_pkcs8_base64)
            .expect("decode private key");
        let signing_key =
            SigningKey::from_pkcs8_der(&private_key_pkcs8_der).expect("parse private key");
        let recipient_public_key =
            crypto_box::PublicKey::from(signing_key.verifying_key().to_montgomery().to_bytes());
        let encrypted_task_id = recipient_public_key
            .seal(&mut crypto_box::aead::OsRng, b"task-789")
            .expect("encrypt task id");
        let task_id = decrypt_task_id(
            TaskRegisterResponse {
                task_id: None,
                encrypted_task_id: Some(BASE64_STANDARD.encode(encrypted_task_id)),
            },
            &key_material.private_key_pkcs8_base64,
        )
        .expect("task id should decrypt");
        assert_eq!(task_id, "task-789".to_string());
    }

    #[test]
    fn stored_identity_round_trips_in_secrets_manager() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let keyring = Arc::new(MockKeyringStore::default());
        let secrets = SecretsManager::new_with_keyring_store(
            codex_home.path().to_path_buf(),
            SecretsBackendKind::Local,
            keyring,
        );
        let key_material = generate_key_material().expect("key material");
        let identity = StoredAgentIdentity {
            binding_id: "workspace-123".to_string(),
            agent_runtime_id: "agent-123".to_string(),
            private_key_pkcs8_base64: "private".to_string(),
            agent_public_key: key_material.agent_public_key,
            registered_at: "2026-03-16T12:34:56Z".to_string(),
            abom: build_abom(),
        };

        let secret_name = secret_name().expect("secret name");
        let secret_scope = secret_scope("workspace-123").expect("secret scope");
        let serialized = serde_json::to_string(&identity).expect("serialize identity");
        secrets
            .set(&secret_scope, &secret_name, &serialized)
            .expect("set identity");
        let loaded = secrets
            .get(&secret_scope, &secret_name)
            .expect("get identity")
            .expect("missing identity");
        let decoded: StoredAgentIdentity = serde_json::from_str(&loaded).expect("decode identity");
        assert_eq!(decoded, identity);
    }

    #[test]
    fn stored_identity_accepts_legacy_public_key_field_name() {
        let legacy_identity = json!({
            "binding_id": "workspace-123",
            "agent_runtime_id": "agent-123",
            "private_key_pkcs8_base64": "private",
            "public_key_base64": "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "registered_at": "2026-03-16T12:34:56Z",
            "abom": build_abom(),
        });
        let decoded: StoredAgentIdentity =
            serde_json::from_value(legacy_identity).expect("decode legacy identity");
        assert_eq!(
            decoded.agent_public_key.split_whitespace().next(),
            Some("ssh-ed25519")
        );
    }

    #[test]
    fn decrypt_task_id_accepts_plaintext_prefix_fallback() {
        let task_id = decrypt_task_id(
            TaskRegisterResponse {
                task_id: None,
                encrypted_task_id: Some("plaintext:task-456".to_string()),
            },
            "ignored",
        )
        .expect("task id should decode");
        assert_eq!(task_id, "task-456".to_string());
    }

    #[test]
    fn secret_scope_is_binding_specific() {
        let first = secret_scope("workspace-123").expect("first scope");
        let second = secret_scope("workspace-456").expect("second scope");
        assert_ne!(first, second);
    }
}
