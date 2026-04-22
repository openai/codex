use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::SecondsFormat;
use chrono::Utc;
use codex_protocol::account::PlanType as AccountPlanType;
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
use serde::de::DeserializeOwned;
use sha2::Digest as _;
use sha2::Sha512;

const AGENT_TASK_REGISTRATION_TIMEOUT: Duration = Duration::from_secs(30);
const AGENT_REGISTRATION_TIMEOUT: Duration = Duration::from_secs(15);
const AGENT_IDENTITY_BISCUIT_TIMEOUT: Duration = Duration::from_secs(15);

/// Stored key material for a registered agent identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentIdentityKey<'a> {
    pub agent_runtime_id: &'a str,
    pub private_key_pkcs8_base64: &'a str,
}

/// Task binding to use when constructing a task-scoped AgentAssertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentTaskAuthorizationTarget<'a> {
    pub agent_runtime_id: &'a str,
    pub task_id: &'a str,
}

/// Runtime identity that owns one or more registered agent tasks.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRuntimeId(String);

impl AgentRuntimeId {
    pub fn new(agent_runtime_id: impl Into<String>) -> Self {
        Self(agent_runtime_id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<String> for AgentRuntimeId {
    fn from(agent_runtime_id: String) -> Self {
        Self::new(agent_runtime_id)
    }
}

impl From<&str> for AgentRuntimeId {
    fn from(agent_runtime_id: &str) -> Self {
        Self::new(agent_runtime_id)
    }
}

/// Task identifier granted to an agent runtime for a scoped objective.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentTaskId(String);

impl AgentTaskId {
    pub fn new(task_id: impl Into<String>) -> Self {
        Self(task_id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<String> for AgentTaskId {
    fn from(task_id: String) -> Self {
        Self::new(task_id)
    }
}

impl From<&str> for AgentTaskId {
    fn from(task_id: &str) -> Self {
        Self::new(task_id)
    }
}

/// Caller-owned stable reference that HAI can resolve to an opaque task id.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AgentTaskExternalRef(String);

impl AgentTaskExternalRef {
    pub fn new(external_ref: impl Into<String>) -> Self {
        Self(external_ref.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<String> for AgentTaskExternalRef {
    fn from(external_ref: String) -> Self {
        Self::new(external_ref)
    }
}

impl From<&str> for AgentTaskExternalRef {
    fn from(external_ref: &str) -> Self {
        Self::new(external_ref)
    }
}

/// Purpose of a registered task binding.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskKind {
    Thread,
    Background,
}

/// Registered task binding used to authorize work for an agent runtime.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegisteredAgentTask {
    pub agent_runtime_id: AgentRuntimeId,
    pub task_id: AgentTaskId,
    pub kind: AgentTaskKind,
}

impl RegisteredAgentTask {
    pub fn new(
        agent_runtime_id: impl Into<AgentRuntimeId>,
        task_id: impl Into<AgentTaskId>,
        kind: AgentTaskKind,
    ) -> Self {
        Self {
            agent_runtime_id: agent_runtime_id.into(),
            task_id: task_id.into(),
            kind,
        }
    }

    pub fn authorization_target(&self) -> AgentTaskAuthorizationTarget<'_> {
        AgentTaskAuthorizationTarget {
            agent_runtime_id: self.agent_runtime_id.as_str(),
            task_id: self.task_id.as_str(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentBillOfMaterials {
    pub agent_version: String,
    pub agent_harness_id: String,
    pub running_location: String,
}

pub struct GeneratedAgentKeyMaterial {
    pub private_key_pkcs8_base64: String,
    pub public_key_ssh: String,
}

/// Claims carried by an Agent Identity JWT.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct AgentIdentityJwtClaims {
    pub agent_runtime_id: String,
    pub agent_private_key: String,
    pub account_id: String,
    pub chatgpt_user_id: String,
    pub email: String,
    pub plan_type: AccountPlanType,
    pub chatgpt_account_is_fedramp: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct AgentAssertionEnvelope {
    agent_runtime_id: String,
    task_id: String,
    timestamp: String,
    signature: String,
}

#[derive(Serialize)]
struct RegisterTaskRequest<'a> {
    timestamp: String,
    signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    external_task_ref: Option<&'a str>,
}

#[derive(Deserialize)]
struct RegisterTaskResponse {
    #[serde(default)]
    task_id: Option<String>,
    #[serde(default, rename = "taskId")]
    task_id_camel: Option<String>,
    #[serde(default)]
    encrypted_task_id: Option<String>,
    #[serde(default, rename = "encryptedTaskId")]
    encrypted_task_id_camel: Option<String>,
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

pub fn authorization_header_for_agent_task(
    key: AgentIdentityKey<'_>,
    target: AgentTaskAuthorizationTarget<'_>,
) -> Result<String> {
    anyhow::ensure!(
        key.agent_runtime_id == target.agent_runtime_id,
        "agent task runtime {} does not match stored agent identity {}",
        target.agent_runtime_id,
        key.agent_runtime_id
    );

    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let envelope = AgentAssertionEnvelope {
        agent_runtime_id: target.agent_runtime_id.to_string(),
        task_id: target.task_id.to_string(),
        timestamp: timestamp.clone(),
        signature: sign_agent_assertion_payload(key, target.task_id, &timestamp)?,
    };
    let serialized_assertion = serialize_agent_assertion(&envelope)?;
    Ok(format!("AgentAssertion {serialized_assertion}"))
}

pub fn authorization_header_for_registered_task(
    key: AgentIdentityKey<'_>,
    task: &RegisteredAgentTask,
) -> Result<String> {
    authorization_header_for_agent_task(key, task.authorization_target())
}

pub fn decode_agent_identity_jwt(jwt: &str) -> Result<AgentIdentityJwtClaims> {
    decode_jwt_payload(jwt).context("failed to decode agent identity JWT")
}

pub fn sign_task_registration_payload(
    key: AgentIdentityKey<'_>,
    timestamp: &str,
) -> Result<String> {
    let signing_key = signing_key_from_private_key_pkcs8_base64(key.private_key_pkcs8_base64)?;
    let payload = format!("{}:{timestamp}", key.agent_runtime_id);
    Ok(BASE64_STANDARD.encode(signing_key.sign(payload.as_bytes()).to_bytes()))
}

pub async fn register_agent_task(
    client: &reqwest::Client,
    chatgpt_base_url: &str,
    key: AgentIdentityKey<'_>,
) -> Result<String> {
    register_agent_task_with_external_ref(client, chatgpt_base_url, key, /*external_ref*/ None)
        .await
}

pub async fn register_agent_task_with_external_ref(
    client: &reqwest::Client,
    chatgpt_base_url: &str,
    key: AgentIdentityKey<'_>,
    external_ref: Option<&AgentTaskExternalRef>,
) -> Result<String> {
    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let request = RegisterTaskRequest {
        signature: sign_task_registration_payload(key, &timestamp)?,
        timestamp,
        external_task_ref: external_ref.map(AgentTaskExternalRef::as_str),
    };

    let response = client
        .post(agent_task_registration_url(
            chatgpt_base_url,
            key.agent_runtime_id,
        ))
        .timeout(AGENT_TASK_REGISTRATION_TIMEOUT)
        .json(&request)
        .send()
        .await
        .context("failed to register agent task")?
        .error_for_status()
        .context("failed to register agent task")?
        .json()
        .await
        .context("failed to decode agent task registration response")?;

    task_id_from_register_task_response(key, response)
}

pub async fn register_agent_identity(
    client: &reqwest::Client,
    chatgpt_base_url: &str,
    access_token: &str,
    key_material: &GeneratedAgentKeyMaterial,
    abom: AgentBillOfMaterials,
) -> Result<AgentRuntimeId> {
    let url = agent_registration_url(chatgpt_base_url);
    let human_biscuit =
        mint_agent_identity_biscuit(client, chatgpt_base_url, access_token, "POST", &url).await?;
    let request = RegisterAgentRequest {
        abom,
        agent_public_key: key_material.public_key_ssh.clone(),
        capabilities: Vec::new(),
    };

    let response = client
        .post(&url)
        .header("X-OpenAI-Authorization", human_biscuit)
        .json(&request)
        .timeout(AGENT_REGISTRATION_TIMEOUT)
        .send()
        .await
        .with_context(|| format!("failed to send agent identity registration request to {url}"))?
        .error_for_status()
        .with_context(|| format!("agent identity registration failed for {url}"))?
        .json::<RegisterAgentResponse>()
        .await
        .with_context(|| format!("failed to parse agent identity response from {url}"))?;

    Ok(AgentRuntimeId::new(response.agent_runtime_id))
}

async fn mint_agent_identity_biscuit(
    client: &reqwest::Client,
    chatgpt_base_url: &str,
    access_token: &str,
    target_method: &str,
    target_url: &str,
) -> Result<String> {
    let url = agent_identity_biscuit_url(chatgpt_base_url);
    let request_id = agent_identity_request_id()?;
    let response = client
        .get(&url)
        .bearer_auth(access_token)
        .header("X-Request-Id", request_id)
        .header("X-Original-Method", target_method)
        .header("X-Original-Url", target_url)
        .timeout(AGENT_IDENTITY_BISCUIT_TIMEOUT)
        .send()
        .await
        .with_context(|| format!("failed to send agent identity biscuit request to {url}"))?
        .error_for_status()
        .with_context(|| format!("agent identity biscuit minting failed for {url}"))?;

    response
        .headers()
        .get("x-openai-authorization")
        .context("agent identity biscuit response did not include x-openai-authorization")?
        .to_str()
        .context("agent identity biscuit response header was not valid UTF-8")
        .map(str::to_string)
}

fn task_id_from_register_task_response(
    key: AgentIdentityKey<'_>,
    response: RegisterTaskResponse,
) -> Result<String> {
    if let Some(task_id) = response.task_id.or(response.task_id_camel) {
        return Ok(task_id);
    }
    let encrypted_task_id = response
        .encrypted_task_id
        .or(response.encrypted_task_id_camel)
        .context("agent task registration response omitted task id")?;
    decrypt_task_id_response(key, &encrypted_task_id)
}

pub fn decrypt_task_id_response(
    key: AgentIdentityKey<'_>,
    encrypted_task_id: &str,
) -> Result<String> {
    let signing_key = signing_key_from_private_key_pkcs8_base64(key.private_key_pkcs8_base64)?;
    let ciphertext = BASE64_STANDARD
        .decode(encrypted_task_id)
        .context("encrypted task id is not valid base64")?;
    let plaintext = curve25519_secret_key_from_signing_key(&signing_key)
        .unseal(&ciphertext)
        .map_err(|_| anyhow::anyhow!("failed to decrypt encrypted task id"))?;
    String::from_utf8(plaintext).context("decrypted task id is not valid UTF-8")
}

pub fn generate_agent_key_material() -> Result<GeneratedAgentKeyMaterial> {
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

pub fn public_key_ssh_from_private_key_pkcs8_base64(
    private_key_pkcs8_base64: &str,
) -> Result<String> {
    let signing_key = signing_key_from_private_key_pkcs8_base64(private_key_pkcs8_base64)?;
    Ok(encode_ssh_ed25519_public_key(&signing_key.verifying_key()))
}

pub fn verifying_key_from_private_key_pkcs8_base64(
    private_key_pkcs8_base64: &str,
) -> Result<VerifyingKey> {
    let signing_key = signing_key_from_private_key_pkcs8_base64(private_key_pkcs8_base64)?;
    Ok(signing_key.verifying_key())
}

pub fn curve25519_secret_key_from_private_key_pkcs8_base64(
    private_key_pkcs8_base64: &str,
) -> Result<Curve25519SecretKey> {
    let signing_key = signing_key_from_private_key_pkcs8_base64(private_key_pkcs8_base64)?;
    Ok(curve25519_secret_key_from_signing_key(&signing_key))
}

pub fn agent_registration_url(chatgpt_base_url: &str) -> String {
    let trimmed = chatgpt_base_url.trim_end_matches('/');
    format!("{trimmed}/v1/agent/register")
}

pub fn agent_task_registration_url(chatgpt_base_url: &str, agent_runtime_id: &str) -> String {
    let trimmed = chatgpt_base_url.trim_end_matches('/');
    format!("{trimmed}/v1/agent/{agent_runtime_id}/task/register")
}

pub fn agent_identity_biscuit_url(chatgpt_base_url: &str) -> String {
    let trimmed = chatgpt_base_url.trim_end_matches('/');
    format!("{trimmed}/authenticate_app_v2")
}

pub fn agent_identity_request_id() -> Result<String> {
    let mut request_id_bytes = [0u8; 16];
    OsRng
        .try_fill_bytes(&mut request_id_bytes)
        .context("failed to generate agent identity request id")?;
    Ok(format!(
        "codex-agent-identity-{}",
        URL_SAFE_NO_PAD.encode(request_id_bytes)
    ))
}

pub fn normalize_chatgpt_base_url(chatgpt_base_url: &str) -> String {
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
    if let Some(stripped) = base_url.strip_suffix("/codex") {
        base_url = stripped.to_string();
    }
    if (base_url.starts_with("https://chatgpt.com")
        || base_url.starts_with("https://chat.openai.com"))
        && !base_url.contains("/backend-api")
    {
        base_url = format!("{base_url}/backend-api");
    }
    base_url
}

pub fn build_abom(session_source: SessionSource) -> AgentBillOfMaterials {
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

pub fn encode_ssh_ed25519_public_key(verifying_key: &VerifyingKey) -> String {
    let mut blob = Vec::with_capacity(4 + 11 + 4 + 32);
    append_ssh_string(&mut blob, b"ssh-ed25519");
    append_ssh_string(&mut blob, verifying_key.as_bytes());
    format!("ssh-ed25519 {}", BASE64_STANDARD.encode(blob))
}

fn sign_agent_assertion_payload(
    key: AgentIdentityKey<'_>,
    task_id: &str,
    timestamp: &str,
) -> Result<String> {
    let signing_key = signing_key_from_private_key_pkcs8_base64(key.private_key_pkcs8_base64)?;
    let payload = format!("{}:{task_id}:{timestamp}", key.agent_runtime_id);
    Ok(BASE64_STANDARD.encode(signing_key.sign(payload.as_bytes()).to_bytes()))
}

fn serialize_agent_assertion(envelope: &AgentAssertionEnvelope) -> Result<String> {
    let payload = serde_json::to_vec(&BTreeMap::from([
        ("agent_runtime_id", envelope.agent_runtime_id.as_str()),
        ("signature", envelope.signature.as_str()),
        ("task_id", envelope.task_id.as_str()),
        ("timestamp", envelope.timestamp.as_str()),
    ]))
    .context("failed to serialize agent assertion envelope")?;
    Ok(URL_SAFE_NO_PAD.encode(payload))
}

fn decode_jwt_payload<T: DeserializeOwned>(jwt: &str) -> Result<T> {
    let mut parts = jwt.split('.');
    let (_header_b64, payload_b64, _sig_b64) = match (parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(s)) if !h.is_empty() && !p.is_empty() && !s.is_empty() => (h, p, s),
        _ => anyhow::bail!("invalid JWT format"),
    };

    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .context("JWT payload is not valid base64url")?;
    serde_json::from_slice(&payload_bytes).context("JWT payload is not valid JSON")
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

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use ed25519_dalek::Signature;
    use ed25519_dalek::Verifier as _;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn registered_agent_task_builds_authorization_target() {
        let task = RegisteredAgentTask::new(
            "agent-runtime-123",
            "task-thread-456",
            AgentTaskKind::Thread,
        );

        assert_eq!(
            task.authorization_target(),
            AgentTaskAuthorizationTarget {
                agent_runtime_id: "agent-runtime-123",
                task_id: "task-thread-456",
            }
        );
    }

    #[test]
    fn register_task_request_omits_external_ref_by_default() {
        let request = RegisterTaskRequest {
            timestamp: "2026-04-23T00:00:00Z".to_string(),
            signature: "signature".to_string(),
            external_task_ref: None,
        };

        let serialized = serde_json::to_value(request).expect("serialize request");

        assert_eq!(
            serialized,
            serde_json::json!({
                "timestamp": "2026-04-23T00:00:00Z",
                "signature": "signature",
            })
        );
    }

    #[test]
    fn register_task_request_includes_external_ref_when_provided() {
        let external_ref = AgentTaskExternalRef::new("thread-123");
        let request = RegisterTaskRequest {
            timestamp: "2026-04-23T00:00:00Z".to_string(),
            signature: "signature".to_string(),
            external_task_ref: Some(external_ref.as_str()),
        };

        let serialized = serde_json::to_value(request).expect("serialize request");

        assert_eq!(
            serialized,
            serde_json::json!({
                "timestamp": "2026-04-23T00:00:00Z",
                "signature": "signature",
                "external_task_ref": "thread-123",
            })
        );
    }

    #[test]
    fn authorization_header_for_agent_task_serializes_signed_agent_assertion() {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let private_key = signing_key
            .to_pkcs8_der()
            .expect("encode test key material");
        let key = AgentIdentityKey {
            agent_runtime_id: "agent-123",
            private_key_pkcs8_base64: &BASE64_STANDARD.encode(private_key.as_bytes()),
        };
        let target = AgentTaskAuthorizationTarget {
            agent_runtime_id: "agent-123",
            task_id: "task-123",
        };

        let header =
            authorization_header_for_agent_task(key, target).expect("build agent assertion header");
        let token = header
            .strip_prefix("AgentAssertion ")
            .expect("agent assertion scheme");
        let payload = URL_SAFE_NO_PAD
            .decode(token)
            .expect("valid base64url payload");
        let envelope: AgentAssertionEnvelope =
            serde_json::from_slice(&payload).expect("valid assertion envelope");

        assert_eq!(
            envelope,
            AgentAssertionEnvelope {
                agent_runtime_id: "agent-123".to_string(),
                task_id: "task-123".to_string(),
                timestamp: envelope.timestamp.clone(),
                signature: envelope.signature.clone(),
            }
        );
        let signature_bytes = BASE64_STANDARD
            .decode(&envelope.signature)
            .expect("valid base64 signature");
        let signature = Signature::from_slice(&signature_bytes).expect("valid signature bytes");
        signing_key
            .verifying_key()
            .verify(
                format!(
                    "{}:{}:{}",
                    envelope.agent_runtime_id, envelope.task_id, envelope.timestamp
                )
                .as_bytes(),
                &signature,
            )
            .expect("signature should verify");
    }

    #[test]
    fn authorization_header_for_agent_task_rejects_mismatched_runtime() {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let private_key = signing_key
            .to_pkcs8_der()
            .expect("encode test key material");
        let private_key_pkcs8_base64 = BASE64_STANDARD.encode(private_key.as_bytes());
        let key = AgentIdentityKey {
            agent_runtime_id: "agent-123",
            private_key_pkcs8_base64: &private_key_pkcs8_base64,
        };
        let target = AgentTaskAuthorizationTarget {
            agent_runtime_id: "agent-456",
            task_id: "task-123",
        };

        let error = authorization_header_for_agent_task(key, target)
            .expect_err("runtime mismatch should fail");

        assert_eq!(
            error.to_string(),
            "agent task runtime agent-456 does not match stored agent identity agent-123"
        );
    }

    #[test]
    fn authorization_header_for_registered_task_uses_existing_wire_shape() {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let private_key = signing_key
            .to_pkcs8_der()
            .expect("encode test key material");
        let private_key_pkcs8_base64 = BASE64_STANDARD.encode(private_key.as_bytes());
        let key = AgentIdentityKey {
            agent_runtime_id: "agent-123",
            private_key_pkcs8_base64: &private_key_pkcs8_base64,
        };
        let task = RegisteredAgentTask::new("agent-123", "task-123", AgentTaskKind::Background);

        let header = authorization_header_for_registered_task(key, &task)
            .expect("build registered task assertion header");
        let token = header
            .strip_prefix("AgentAssertion ")
            .expect("agent assertion scheme");
        let payload = URL_SAFE_NO_PAD
            .decode(token)
            .expect("valid base64url payload");
        let envelope: AgentAssertionEnvelope =
            serde_json::from_slice(&payload).expect("valid assertion envelope");

        assert_eq!(
            envelope,
            AgentAssertionEnvelope {
                agent_runtime_id: "agent-123".to_string(),
                task_id: "task-123".to_string(),
                timestamp: envelope.timestamp.clone(),
                signature: envelope.signature.clone(),
            }
        );
    }

    #[test]
    fn decode_agent_identity_jwt_reads_claims() {
        let jwt = jwt_with_payload(serde_json::json!({
            "agent_runtime_id": "agent-runtime-id",
            "agent_private_key": "private-key",
            "account_id": "account-id",
            "chatgpt_user_id": "user-id",
            "email": "user@example.com",
            "plan_type": "pro",
            "chatgpt_account_is_fedramp": false,
        }));

        let claims = decode_agent_identity_jwt(&jwt).expect("JWT should decode");

        assert_eq!(
            claims,
            AgentIdentityJwtClaims {
                agent_runtime_id: "agent-runtime-id".to_string(),
                agent_private_key: "private-key".to_string(),
                account_id: "account-id".to_string(),
                chatgpt_user_id: "user-id".to_string(),
                email: "user@example.com".to_string(),
                plan_type: AccountPlanType::Pro,
                chatgpt_account_is_fedramp: false,
            }
        );
    }

    #[test]
    fn normalize_chatgpt_base_url_strips_codex_before_backend_api() {
        assert_eq!(
            normalize_chatgpt_base_url("https://chatgpt.com/codex"),
            "https://chatgpt.com/backend-api"
        );
    }

    fn jwt_with_payload(payload: serde_json::Value) -> String {
        let encode = |bytes: &[u8]| URL_SAFE_NO_PAD.encode(bytes);
        let header_b64 = encode(br#"{"alg":"none","typ":"JWT"}"#);
        let payload_b64 = encode(&serde_json::to_vec(&payload).expect("payload should serialize"));
        let signature_b64 = encode(b"sig");
        format!("{header_b64}.{payload_b64}.{signature_b64}")
    }
}
