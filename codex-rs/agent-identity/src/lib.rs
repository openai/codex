use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::fmt;
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
const AGENT_REGISTRATION_TIMEOUT: Duration = Duration::from_secs(15);
const PROD_AGENT_IDENTITY_AUTHAPI_BASE_URL: &str = "https://auth.openai.com/api/accounts";
const STAGING_AGENT_IDENTITY_AUTHAPI_BASE_URL: &str = "https://auth.api.openai.org/api/accounts";
const AGENT_IDENTITY_KEY_SEED_BYTES: usize = 64;
const AGENT_IDENTITY_KEY_DERIVATION_CONTEXT: &[u8] = b"codex-agent-identity-ed25519-v1";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChatGptEnvironment {
    #[default]
    Production,
    Staging,
}

impl ChatGptEnvironment {
    pub fn from_chatgpt_base_url(chatgpt_base_url: &str) -> Result<Self> {
        match chatgpt_base_url.trim_end_matches('/') {
            "https://chatgpt.com"
            | "https://chatgpt.com/backend-api"
            | "https://chatgpt.com/codex"
            | "https://chatgpt.com/backend-api/codex"
            | "https://chat.openai.com"
            | "https://chat.openai.com/backend-api"
            | "https://chat.openai.com/codex"
            | "https://chat.openai.com/backend-api/codex" => Ok(Self::Production),
            "https://chatgpt-staging.com"
            | "https://chatgpt-staging.com/backend-api"
            | "https://chatgpt-staging.com/codex"
            | "https://chatgpt-staging.com/backend-api/codex" => Ok(Self::Staging),
            _ => anyhow::bail!(
                "Agent Identity only supports production and staging ChatGPT environments"
            ),
        }
    }

    pub fn chatgpt_base_url(self) -> &'static str {
        match self {
            Self::Production => "https://chatgpt.com/backend-api",
            Self::Staging => "https://chatgpt-staging.com/backend-api",
        }
    }

    pub fn agent_identity_authapi_base_url(self) -> &'static str {
        match self {
            Self::Production => PROD_AGENT_IDENTITY_AUTHAPI_BASE_URL,
            Self::Staging => STAGING_AGENT_IDENTITY_AUTHAPI_BASE_URL,
        }
    }
}

/// Borrowed durable signing material for a registered agent identity.
///
/// This intentionally does not include a task id. Task ids are scoped to a
/// single Codex run, while the agent runtime id and private key are the
/// reusable identity material used to register and sign that run task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentIdentityKey<'a> {
    pub agent_runtime_id: &'a str,
    pub private_key_pkcs8_base64: &'a str,
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

#[derive(Debug, Serialize)]
struct RegisterAgentRequest {
    abom: AgentBillOfMaterials,
    agent_public_key: String,
    capabilities: Vec<String>,
    ttl: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RegisterAgentResponse {
    agent_runtime_id: String,
}

/// HTTP status failure returned by Agent Identity registration endpoints.
#[derive(Debug)]
pub struct AgentIdentityRegistrationHttpError {
    operation: &'static str,
    status: reqwest::StatusCode,
    body: String,
}

impl AgentIdentityRegistrationHttpError {
    fn new(operation: &'static str, status: reqwest::StatusCode, body: String) -> Self {
        Self {
            operation,
            status,
            body,
        }
    }

    /// HTTP status returned by the registration endpoint.
    pub fn status(&self) -> reqwest::StatusCode {
        self.status
    }
}

impl fmt::Display for AgentIdentityRegistrationHttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.body.is_empty() {
            write!(f, "{} failed with status {}", self.operation, self.status)
        } else {
            write!(
                f,
                "{} failed with status {}: {}",
                self.operation, self.status, self.body
            )
        }
    }
}

impl StdError for AgentIdentityRegistrationHttpError {}

/// Returns whether an Agent Identity registration error is safe to retry.
pub fn is_retryable_registration_error(error: &anyhow::Error) -> bool {
    error.chain().any(is_retryable_registration_cause)
}

fn is_retryable_registration_cause(cause: &(dyn StdError + 'static)) -> bool {
    if let Some(error) = cause.downcast_ref::<AgentIdentityRegistrationHttpError>() {
        return is_retryable_registration_status(error.status());
    }

    if let Some(error) = cause.downcast_ref::<reqwest::Error>() {
        if let Some(status) = error.status() {
            return is_retryable_registration_status(status);
        }
        return error.is_timeout() || error.is_connect() || error.is_request();
    }

    false
}

fn is_retryable_registration_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

pub fn authorization_header_for_agent_task(
    key: AgentIdentityKey<'_>,
    task_id: &str,
) -> Result<String> {
    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let envelope = AgentAssertionEnvelope {
        agent_runtime_id: key.agent_runtime_id.to_string(),
        task_id: task_id.to_string(),
        timestamp: timestamp.clone(),
        signature: sign_agent_assertion_payload(key, task_id, &timestamp)?,
    };
    let serialized_assertion = serialize_agent_assertion(&envelope)?;
    Ok(format!("AgentAssertion {serialized_assertion}"))
}

pub async fn fetch_agent_identity_jwks(
    client: &reqwest::Client,
    agent_identity_jwt_base_url: &str,
) -> Result<JwkSet> {
    let response = client
        .get(agent_identity_jwks_url(agent_identity_jwt_base_url))
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
    agent_identity_authapi_base_url: &str,
    key: AgentIdentityKey<'_>,
) -> Result<String> {
    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let request = RegisterTaskRequest {
        signature: sign_task_registration_payload(key, &timestamp)?,
        timestamp,
    };
    let url = agent_task_registration_url(agent_identity_authapi_base_url, key.agent_runtime_id);

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
        return Err(AgentIdentityRegistrationHttpError::new(
            "agent task registration",
            status,
            body,
        )
        .into());
    }

    let response = response
        .json()
        .await
        .context("failed to decode agent task registration response")?;

    task_id_from_register_task_response(key, response)
}

pub async fn register_agent_identity(
    client: &reqwest::Client,
    agent_identity_authapi_base_url: &str,
    access_token: &str,
    is_fedramp_account: bool,
    key_material: &GeneratedAgentKeyMaterial,
    abom: AgentBillOfMaterials,
    capabilities: Vec<String>,
) -> Result<String> {
    let url = agent_registration_url(agent_identity_authapi_base_url);
    let request = RegisterAgentRequest {
        abom,
        agent_public_key: key_material.public_key_ssh.clone(),
        capabilities,
        ttl: None,
    };

    let mut request_builder = client
        .post(&url)
        .bearer_auth(access_token)
        .json(&request)
        .timeout(AGENT_REGISTRATION_TIMEOUT);
    if is_fedramp_account {
        request_builder = request_builder.header("X-OpenAI-Fedramp", "true");
    }

    let response = request_builder
        .send()
        .await
        .with_context(|| format!("failed to send agent identity registration request to {url}"))?
        .error_for_status()
        .with_context(|| format!("agent identity registration failed for {url}"))?
        .json::<RegisterAgentResponse>()
        .await
        .with_context(|| format!("failed to parse agent identity response from {url}"))?;

    Ok(response.agent_runtime_id)
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
    let mut seed_material = [0u8; AGENT_IDENTITY_KEY_SEED_BYTES];
    OsRng
        .try_fill_bytes(&mut seed_material)
        .context("failed to generate agent identity private key seed material")?;
    // Ed25519 stores a 32-byte seed, so derive it from all sampled seed material.
    let mut digest = Sha512::new();
    digest.update(AGENT_IDENTITY_KEY_DERIVATION_CONTEXT);
    digest.update(seed_material);
    let digest = digest.finalize();
    let mut secret_key_bytes = [0u8; 32];
    secret_key_bytes.copy_from_slice(&digest[..32]);
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

pub fn agent_registration_url(agent_identity_authapi_base_url: &str) -> String {
    agent_identity_authapi_url(agent_identity_authapi_base_url, "/v1/agent/register")
}

pub fn agent_task_registration_url(
    agent_identity_authapi_base_url: &str,
    agent_runtime_id: &str,
) -> String {
    agent_identity_authapi_url(
        agent_identity_authapi_base_url,
        &format!("/v1/agent/{agent_runtime_id}/task/register"),
    )
}

pub fn agent_identity_jwks_url(agent_identity_jwt_base_url: &str) -> String {
    let trimmed = agent_identity_jwt_base_url.trim_end_matches('/');
    if trimmed.contains("/backend-api") {
        format!("{trimmed}/wham/agent-identities/jwks")
    } else {
        format!("{trimmed}/agent-identities/jwks")
    }
}

fn agent_identity_authapi_url(agent_identity_authapi_base_url: &str, api_path: &str) -> String {
    let base_url = agent_identity_authapi_base_url.trim_end_matches('/');
    format!("{base_url}{api_path}")
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
    fn register_task_request_uses_single_run_task_shape() {
        let request = RegisterTaskRequest {
            timestamp: "2026-04-23T00:00:00Z".to_string(),
            signature: "signature".to_string(),
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
    fn authorization_header_for_agent_task_serializes_signed_agent_assertion() {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let private_key = signing_key
            .to_pkcs8_der()
            .expect("encode test key material");
        let key = AgentIdentityKey {
            agent_runtime_id: "agent-123",
            private_key_pkcs8_base64: &BASE64_STANDARD.encode(private_key.as_bytes()),
        };

        let header = authorization_header_for_agent_task(key, "task-123")
            .expect("build agent assertion header");
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
    fn chatgpt_environment_maps_known_urls_to_authapi() -> anyhow::Result<()> {
        assert_eq!(
            ChatGptEnvironment::from_chatgpt_base_url("https://chatgpt.com/backend-api/codex")?,
            ChatGptEnvironment::Production
        );
        assert_eq!(
            ChatGptEnvironment::Production.agent_identity_authapi_base_url(),
            "https://auth.openai.com/api/accounts"
        );
        assert_eq!(
            ChatGptEnvironment::from_chatgpt_base_url("https://chatgpt-staging.com/backend-api")?,
            ChatGptEnvironment::Staging
        );
        assert_eq!(
            ChatGptEnvironment::Staging.agent_identity_authapi_base_url(),
            "https://auth.api.openai.org/api/accounts"
        );
        Ok(())
    }

    #[test]
    fn chatgpt_environment_rejects_custom_urls() {
        assert!(ChatGptEnvironment::from_chatgpt_base_url("http://localhost:8080").is_err(),);
    }

    #[test]
    fn agent_registration_url_appends_to_authapi_base_url() {
        assert_eq!(
            agent_registration_url("https://auth.openai.com/api/accounts"),
            "https://auth.openai.com/api/accounts/v1/agent/register"
        );
        assert_eq!(
            agent_registration_url("http://localhost:8080"),
            "http://localhost:8080/v1/agent/register"
        );
        assert_eq!(
            agent_registration_url("http://localhost:8080/backend-api"),
            "http://localhost:8080/backend-api/v1/agent/register"
        );
    }

    #[test]
    fn agent_task_registration_url_appends_to_authapi_base_url() {
        assert_eq!(
            agent_task_registration_url("https://auth.openai.com/api/accounts", "agent-runtime-id"),
            "https://auth.openai.com/api/accounts/v1/agent/agent-runtime-id/task/register"
        );
        assert_eq!(
            agent_task_registration_url(
                "https://auth.openai.com/api/accounts/",
                "agent-runtime-id"
            ),
            "https://auth.openai.com/api/accounts/v1/agent/agent-runtime-id/task/register"
        );
        assert_eq!(
            agent_task_registration_url("http://localhost:8080", "agent-runtime-id"),
            "http://localhost:8080/v1/agent/agent-runtime-id/task/register"
        );
    }

    #[test]
    fn retryable_registration_error_accepts_429_and_5xx() {
        let too_many_requests = anyhow::Error::new(AgentIdentityRegistrationHttpError::new(
            "agent registration",
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            "rate limited".to_string(),
        ));
        let unavailable = anyhow::Error::new(AgentIdentityRegistrationHttpError::new(
            "agent registration",
            reqwest::StatusCode::SERVICE_UNAVAILABLE,
            "try later".to_string(),
        ));

        assert!(is_retryable_registration_error(&too_many_requests));
        assert!(is_retryable_registration_error(&unavailable));
    }

    #[test]
    fn retryable_registration_error_rejects_hard_failures() {
        let forbidden = anyhow::Error::new(AgentIdentityRegistrationHttpError::new(
            "agent registration",
            reqwest::StatusCode::FORBIDDEN,
            "not allowed".to_string(),
        ));
        let malformed = anyhow::anyhow!("failed to sign registration request");

        assert!(!is_retryable_registration_error(&forbidden));
        assert!(!is_retryable_registration_error(&malformed));
    }

    #[test]
    fn agent_identity_jwks_url_uses_agent_identity_jwt_route() {
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
    fn agent_identity_jwks_url_uses_jwt_issuer_base_url() {
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
