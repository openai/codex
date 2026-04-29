use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::SecondsFormat;
use chrono::Utc;
use codex_protocol::auth::PlanType as AuthPlanType;
use codex_protocol::protocol::SessionSource;
use crypto_box::SecretKey as Curve25519SecretKey;
use ed25519_dalek::Signer as _;
use ed25519_dalek::SigningKey;
use ed25519_dalek::VerifyingKey;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::pkcs8::EncodePrivateKey;
use jsonwebtoken::Algorithm;
use jsonwebtoken::DecodingKey;
use jsonwebtoken::Validation;
use jsonwebtoken::decode;
use jsonwebtoken::decode_header;
use jsonwebtoken::jwk::JwkSet;
use rand::TryRngCore;
use rand::rngs::OsRng;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::Digest as _;
use sha2::Sha512;

const AGENT_TASK_REGISTRATION_TIMEOUT: Duration = Duration::from_secs(30);
const AGENT_IDENTITY_JWKS_TIMEOUT: Duration = Duration::from_secs(10);
const AGENT_IDENTITY_JWT_AUDIENCE: &str = "codex-app-server";
const AGENT_IDENTITY_JWT_ISSUER: &str = "https://chatgpt.com/codex-backend/agent-identity";

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

/// Claims carried by an Agent Identity JWT.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct AgentIdentityJwtClaims {
    pub iss: String,
    pub aud: String,
    pub iat: usize,
    pub exp: usize,
    pub agent_runtime_id: String,
    pub agent_private_key: String,
    pub account_id: String,
    pub chatgpt_user_id: String,
    pub email: String,
    pub plan_type: AuthPlanType,
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
struct RegisterTaskRequest {
    timestamp: String,
    signature: String,
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

pub async fn fetch_agent_identity_jwks(
    client: &reqwest::Client,
    chatgpt_base_url: &str,
) -> Result<JwkSet> {
    let response = client
        .get(agent_identity_jwks_url(chatgpt_base_url))
        .timeout(AGENT_IDENTITY_JWKS_TIMEOUT)
        .send()
        .await
        .context("failed to request agent identity JWKS")?
        .error_for_status()
        .context("agent identity JWKS endpoint returned an error")?;

    response
        .json()
        .await
        .context("failed to decode agent identity JWKS")
}

pub fn decode_agent_identity_jwt(
    jwt: &str,
    jwks: Option<&JwkSet>,
) -> Result<AgentIdentityJwtClaims> {
    let Some(jwks) = jwks else {
        return decode_agent_identity_jwt_payload(jwt);
    };

    let header = decode_header(jwt).context("failed to decode agent identity JWT header")?;
    let kid = header
        .kid
        .context("agent identity JWT header does not include a kid")?;
    let jwk = jwks
        .find(&kid)
        .with_context(|| format!("agent identity JWT kid {kid} is not trusted"))?;
    let decoding_key = DecodingKey::from_jwk(jwk).context("failed to build JWT decoding key")?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[AGENT_IDENTITY_JWT_AUDIENCE]);
    validation.set_issuer(&[AGENT_IDENTITY_JWT_ISSUER]);
    validation.required_spec_claims.insert("iss".to_string());
    validation.required_spec_claims.insert("aud".to_string());
    decode::<AgentIdentityJwtClaims>(jwt, &decoding_key, &validation)
        .map(|data| data.claims)
        .context("failed to verify agent identity JWT")
}

fn decode_agent_identity_jwt_payload<T: DeserializeOwned>(jwt: &str) -> Result<T> {
    let mut parts = jwt.split('.');
    let (_header_b64, payload_b64, _sig_b64) = match (parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(s)) if !h.is_empty() && !p.is_empty() && !s.is_empty() => (h, p, s),
        _ => anyhow::bail!("invalid agent identity JWT format"),
    };
    anyhow::ensure!(parts.next().is_none(), "invalid agent identity JWT format");

    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .context("agent identity JWT payload is not valid base64url")?;
    serde_json::from_slice(&payload_bytes).context("agent identity JWT payload is not valid JSON")
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
    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let request = RegisterTaskRequest {
        signature: sign_task_registration_payload(key, &timestamp)?,
        timestamp,
    };
    let url = agent_task_registration_url(chatgpt_base_url, key.agent_runtime_id);

    let response = client
        .post(url)
        .timeout(AGENT_TASK_REGISTRATION_TIMEOUT)
        .json(&request)
        .send()
        .await
        .context("failed to register agent task")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let body = if body.len() > 512 {
            format!("{}...", body.chars().take(512).collect::<String>())
        } else {
            body
        };
        anyhow::bail!("failed to register agent task with status {status}: {body}");
    }

    let response = response
        .json()
        .await
        .context("failed to decode agent task registration response")?;

    task_id_from_register_task_response(key, response)
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

pub fn agent_identity_jwks_url(chatgpt_base_url: &str) -> String {
    let trimmed = chatgpt_base_url.trim_end_matches('/');
    if trimmed.contains("/backend-api") {
        format!("{trimmed}/wham/agent-identities/jwks")
    } else {
        format!("{trimmed}/agent-identities/jwks")
    }
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

pub fn build_abom(session_source: SessionSource) -> AgentBillOfMaterials {
    AgentBillOfMaterials {
        agent_version: env!("CARGO_PKG_VERSION").to_string(),
        agent_harness_id: match &session_source {
            SessionSource::VSCode => "codex-app".to_string(),
            SessionSource::Cli
            | SessionSource::Exec
            | SessionSource::Mcp
            | SessionSource::Custom(_)
            | SessionSource::Internal(_)
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
    use jsonwebtoken::EncodingKey;
    use jsonwebtoken::Header;
    use pretty_assertions::assert_eq;

    use codex_protocol::auth::KnownPlan;

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
    fn decode_agent_identity_jwt_reads_claims() {
        let jwt = jwt_with_payload(serde_json::json!({
            "iss": AGENT_IDENTITY_JWT_ISSUER,
            "aud": AGENT_IDENTITY_JWT_AUDIENCE,
            "iat": 1_700_000_000usize,
            "exp": 4_000_000_000usize,
            "agent_runtime_id": "agent-runtime-id",
            "agent_private_key": "private-key",
            "account_id": "account-id",
            "chatgpt_user_id": "user-id",
            "email": "user@example.com",
            "plan_type": "pro",
            "chatgpt_account_is_fedramp": false,
        }));

        let claims = decode_agent_identity_jwt(&jwt, /*jwks*/ None).expect("JWT should decode");

        assert_eq!(
            claims,
            AgentIdentityJwtClaims {
                iss: AGENT_IDENTITY_JWT_ISSUER.to_string(),
                aud: AGENT_IDENTITY_JWT_AUDIENCE.to_string(),
                iat: 1_700_000_000,
                exp: 4_000_000_000,
                agent_runtime_id: "agent-runtime-id".to_string(),
                agent_private_key: "private-key".to_string(),
                account_id: "account-id".to_string(),
                chatgpt_user_id: "user-id".to_string(),
                email: "user@example.com".to_string(),
                plan_type: AuthPlanType::Known(KnownPlan::Pro),
                chatgpt_account_is_fedramp: false,
            }
        );
    }

    #[test]
    fn decode_agent_identity_jwt_maps_raw_plan_aliases() {
        let jwt = jwt_with_payload(serde_json::json!({
            "iss": AGENT_IDENTITY_JWT_ISSUER,
            "aud": AGENT_IDENTITY_JWT_AUDIENCE,
            "iat": 1_700_000_000usize,
            "exp": 4_000_000_000usize,
            "agent_runtime_id": "agent-runtime-id",
            "agent_private_key": "private-key",
            "account_id": "account-id",
            "chatgpt_user_id": "user-id",
            "email": "user@example.com",
            "plan_type": "hc",
            "chatgpt_account_is_fedramp": false,
        }));

        let claims = decode_agent_identity_jwt(&jwt, /*jwks*/ None).expect("JWT should decode");

        assert_eq!(claims.plan_type, AuthPlanType::Known(KnownPlan::Enterprise));
    }

    #[test]
    fn decode_agent_identity_jwt_verifies_when_jwks_is_present() {
        let jwks = test_jwks("test-key");
        let claims = AgentIdentityJwtClaims {
            iss: AGENT_IDENTITY_JWT_ISSUER.to_string(),
            aud: AGENT_IDENTITY_JWT_AUDIENCE.to_string(),
            iat: 1_700_000_000,
            exp: 4_000_000_000,
            agent_runtime_id: "agent-runtime-id".to_string(),
            agent_private_key: "private-key".to_string(),
            account_id: "account-id".to_string(),
            chatgpt_user_id: "user-id".to_string(),
            email: "user@example.com".to_string(),
            plan_type: AuthPlanType::Known(KnownPlan::Pro),
            chatgpt_account_is_fedramp: false,
        };
        let jwt = jsonwebtoken::encode(
            &test_jwt_header("test-key"),
            &serde_json::json!({
                "iss": claims.iss,
                "aud": claims.aud,
                "iat": claims.iat,
                "exp": claims.exp,
                "agent_runtime_id": claims.agent_runtime_id,
                "agent_private_key": claims.agent_private_key,
                "account_id": claims.account_id,
                "chatgpt_user_id": claims.chatgpt_user_id,
                "email": claims.email,
                "plan_type": "pro",
                "chatgpt_account_is_fedramp": claims.chatgpt_account_is_fedramp,
            }),
            &test_rsa_encoding_key(),
        )
        .expect("JWT should encode");

        let expected_claims = AgentIdentityJwtClaims {
            iss: AGENT_IDENTITY_JWT_ISSUER.to_string(),
            aud: AGENT_IDENTITY_JWT_AUDIENCE.to_string(),
            iat: 1_700_000_000,
            exp: 4_000_000_000,
            agent_runtime_id: "agent-runtime-id".to_string(),
            agent_private_key: "private-key".to_string(),
            account_id: "account-id".to_string(),
            chatgpt_user_id: "user-id".to_string(),
            email: "user@example.com".to_string(),
            plan_type: AuthPlanType::Known(KnownPlan::Pro),
            chatgpt_account_is_fedramp: false,
        };
        assert_eq!(
            decode_agent_identity_jwt(&jwt, Some(&jwks)).expect("JWT should verify"),
            expected_claims
        );
    }

    #[test]
    fn decode_agent_identity_jwt_rejects_untrusted_kid() {
        let jwks = test_jwks("other-key");

        let jwt = jsonwebtoken::encode(
            &test_jwt_header("test-key"),
            &serde_json::json!({
                "iss": AGENT_IDENTITY_JWT_ISSUER,
                "aud": AGENT_IDENTITY_JWT_AUDIENCE,
                "iat": 1_700_000_000,
                "exp": 4_000_000_000usize,
                "agent_runtime_id": "agent-runtime-id",
                "agent_private_key": "private-key",
                "account_id": "account-id",
                "chatgpt_user_id": "user-id",
                "email": "user@example.com",
                "plan_type": "pro",
                "chatgpt_account_is_fedramp": false,
            }),
            &test_rsa_encoding_key(),
        )
        .expect("JWT should encode");

        decode_agent_identity_jwt(&jwt, Some(&jwks)).expect_err("JWT should not verify");
    }

    #[test]
    fn decode_agent_identity_jwt_requires_issuer_and_audience() {
        let jwks = test_jwks("test-key");
        let jwt = jsonwebtoken::encode(
            &test_jwt_header("test-key"),
            &serde_json::json!({
                "iat": 1_700_000_000,
                "exp": 4_000_000_000usize,
                "agent_runtime_id": "agent-runtime-id",
                "agent_private_key": "private-key",
                "account_id": "account-id",
                "chatgpt_user_id": "user-id",
                "email": "user@example.com",
                "plan_type": "pro",
                "chatgpt_account_is_fedramp": false,
            }),
            &test_rsa_encoding_key(),
        )
        .expect("JWT should encode");

        decode_agent_identity_jwt(&jwt, Some(&jwks)).expect_err("JWT should not verify");
    }

    fn test_jwt_header(kid: &str) -> Header {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(kid.to_string());
        header
    }

    fn test_rsa_encoding_key() -> EncodingKey {
        EncodingKey::from_rsa_pem(
            br#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDWpAXYypOsYAwO
bvBduMk/mxaoYDze0AZSzaSzLuIlcsl2EKDgC3AabhIWXh/qTGEJLOU3VB1e5mO9
FPbBlmIZSL3FQTbyt/hYutPFKfCou5PLmScw/TzILS3/RhT8UY9kxxZvXiEbTki9
mvxRuZFpVqDFJHwfitIjKZGhXDCYVKurPTrxetYZJg0h8sQBLKjkZ0BqqaTUkAsg
0eBgZAlXEzG3By8PGhUqYLt6W1Q3KYw0FmGy/gTyzH1g0ukGgSJvOd8SkNT8MbOs
zl5kKxDNqpuEE6UZ3jbuJ+5382d31w+rOAJRzbf7QVdI9+luCSwJcDACYPQ4WNBa
uCpV0ovpAgMBAAECggEAVu84LwZdqYN9XpswX8VoPYrjMm9IODapWQBRpQFoNyK2
1ksF3bjEPvA2Azk8U/l7k+vLKw22l6lY3EyRZPcz5GnB8xLm3ogE3mtNOp4yCyVu
RxhQ91aaN7mU17/a4BdorLi2LYVCg3zBmYociD1Q2AluNGsCmwPu+K7tfR2J0Sg8
NjqiTbDG1XDpR/icwgC9t6vh8lZpCHDhF4tbQfLLVLeA/OdcuzXDyMCXbmdVIdBQ
rm4aIFmr2e1/2ctTbCg85S6AGFTH+pSLjrwTzyvf+F6NW5uNjLQAQLFj+EznBDxj
Xdx90cySrjsKK6PVWQF4RiTvkSW8eWL7R6B2FZbGwQKBgQDuVQRj72hWloR7mbEL
aUEEv3pIXTMXWEsoMBNczos/1L1RnAN1AI44TurznasPZAWvQj+kVbLDR+TAeZrL
iA8HIWswQUI18hFmgKzSkwIXGtubcKVrgsKeS4lMDKCM/Ef6WAYdeq6ronoY5lCN
YrJFmGp81W5zcV7lyiycgbSiGwKBgQDmjWYf6pZjrK7Z+OJ3X1AZfi2vss15SCvL
3fPgzIDbViztpGyQhc3DQZIsBNIu0xZp/veGce9TEeTds2ro9NfdJFeou8+fC7Pq
sOsM3amGFFi+ZW/9BWyjZEM88bgWWAjqLHbpfHDxjAf5CSxddqxgHlbP0Ytyb1Vg
gmPDn9YKSwKBgQDbTi3hC35WFuDHn0/zcSHcDZmnFuOZeqyFyV83yfMGhGrEuqvP
sPgtRikajJ3IZsB4WZyYSidZXEFY/0z6NjOl2xF38MTNQPbT/FmK1q1Yt2UWrlv5
BvSwlk87RG9D7C0LZo4R+D7cPoDdgqjiwMvMEIkEX5zn641oI1ZTmWKuuwKBgQCD
KF+3unnRvHRAVoFnTZbA2fJdqMeRvogD04GhGlYX8V9f1hFY6nXTJaNlXVzA/J8c
r8ra9kgjJuPfZ+ljG58OFFW2DRohLcQtuHYPfK6rMzoFHqnl9EcIcMp7ijuionR3
29HOJFgQYgxLFXfit9d6WugiE+BTupiEbckZif13HwKBgE/lAlkVHP6YahOO2Ljc
J1bwkqKZTB5dHolX9A58e/xXnfZ5P8f3Z83+Izap3FwqQulk7b1WO1MQcHuVg2NN
5da0D4h2rYOXnbYIg0BVu4spQbaM6ewsp66b8+MzLOBvj8SzWdt1Oyw0q/MRyQAR
8U4M2TSWCKUY/A6sT4W8+mT9
-----END PRIVATE KEY-----"#,
        )
        .expect("test RSA key should parse")
    }

    fn test_jwks(kid: &str) -> jsonwebtoken::jwk::JwkSet {
        serde_json::from_value(serde_json::json!({
            "keys": [{
                "kty": "RSA",
                "kid": kid,
                "use": "sig",
                "alg": "RS256",
                "n": "1qQF2MqTrGAMDm7wXbjJP5sWqGA83tAGUs2ksy7iJXLJdhCg4AtwGm4SFl4f6kxhCSzlN1QdXuZjvRT2wZZiGUi9xUE28rf4WLrTxSnwqLuTy5knMP08yC0t_0YU_FGPZMcWb14hG05IvZr8UbmRaVagxSR8H4rSIymRoVwwmFSrqz068XrWGSYNIfLEASyo5GdAaqmk1JALINHgYGQJVxMxtwcvDxoVKmC7eltUNymMNBZhsv4E8sx9YNLpBoEibznfEpDU_DGzrM5eZCsQzaqbhBOlGd427ifud_Nnd9cPqzgCUc23-0FXSPfpbgksCXAwAmD0OFjQWrgqVdKL6Q",
                "e": "AQAB",
            }]
        }))
        .expect("test JWKS should parse")
    }

    #[test]
    fn agent_identity_jwks_url_uses_backend_api_base_url() {
        assert_eq!(
            agent_identity_jwks_url("https://chatgpt.com/backend-api"),
            "https://chatgpt.com/backend-api/wham/agent-identities/jwks"
        );
        assert_eq!(
            agent_identity_jwks_url("https://chatgpt.com/backend-api/"),
            "https://chatgpt.com/backend-api/wham/agent-identities/jwks"
        );
    }

    #[test]
    fn agent_identity_jwks_url_uses_codex_api_base_url() {
        assert_eq!(
            agent_identity_jwks_url("http://localhost:8080/api/codex"),
            "http://localhost:8080/api/codex/agent-identities/jwks"
        );
        assert_eq!(
            agent_identity_jwks_url("http://localhost:8080/api/codex/"),
            "http://localhost:8080/api/codex/agent-identities/jwks"
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
