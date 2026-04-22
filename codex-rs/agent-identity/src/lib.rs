use std::collections::BTreeMap;

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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct AgentAssertionEnvelope {
    agent_runtime_id: String,
    task_id: String,
    timestamp: String,
    signature: String,
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

pub fn sign_task_registration_payload(
    key: AgentIdentityKey<'_>,
    timestamp: &str,
) -> Result<String> {
    let signing_key = signing_key_from_private_key_pkcs8_base64(key.private_key_pkcs8_base64)?;
    let payload = format!("{}:{timestamp}", key.agent_runtime_id);
    Ok(BASE64_STANDARD.encode(signing_key.sign(payload.as_bytes()).to_bytes()))
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
    fn normalize_chatgpt_base_url_strips_codex_before_backend_api() {
        assert_eq!(
            normalize_chatgpt_base_url("https://chatgpt.com/codex"),
            "https://chatgpt.com/backend-api"
        );
    }
}
