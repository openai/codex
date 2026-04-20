use std::collections::BTreeMap;

use anyhow::Context;
use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::SecondsFormat;
use chrono::Utc;
use ed25519_dalek::Signer as _;
use ed25519_dalek::SigningKey;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use serde::Deserialize;
use serde::Serialize;

use super::storage::AgentIdentityAuthRecord;

/// Task binding to use when constructing a task-scoped AgentAssertion.
///
/// The caller owns the task lifecycle. `AuthManager` only uses this target to
/// sign an authorization header with the stored agent identity key material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentTaskAuthorizationTarget<'a> {
    pub agent_runtime_id: &'a str,
    pub task_id: &'a str,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct AgentAssertionEnvelope {
    agent_runtime_id: String,
    task_id: String,
    timestamp: String,
    signature: String,
}

pub(super) fn authorization_header_for_agent_task(
    record: &AgentIdentityAuthRecord,
    target: AgentTaskAuthorizationTarget<'_>,
) -> Result<String> {
    anyhow::ensure!(
        record.agent_runtime_id == target.agent_runtime_id,
        "agent task runtime {} does not match stored agent identity {}",
        target.agent_runtime_id,
        record.agent_runtime_id
    );

    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let envelope = AgentAssertionEnvelope {
        agent_runtime_id: target.agent_runtime_id.to_string(),
        task_id: target.task_id.to_string(),
        timestamp: timestamp.clone(),
        signature: sign_agent_assertion_payload(record, target, &timestamp)?,
    };
    let serialized_assertion = serialize_agent_assertion(&envelope)?;
    Ok(format!("AgentAssertion {serialized_assertion}"))
}

fn sign_agent_assertion_payload(
    record: &AgentIdentityAuthRecord,
    target: AgentTaskAuthorizationTarget<'_>,
    timestamp: &str,
) -> Result<String> {
    let signing_key = signing_key_from_agent_private_key(&record.agent_private_key)?;
    let payload = format!("{}:{}:{timestamp}", target.agent_runtime_id, target.task_id);
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

fn signing_key_from_agent_private_key(agent_private_key: &str) -> Result<SigningKey> {
    let private_key = BASE64_STANDARD
        .decode(agent_private_key)
        .context("stored agent identity private key is not valid base64")?;
    SigningKey::from_pkcs8_der(&private_key)
        .context("stored agent identity private key is not valid PKCS#8")
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::Signature;
    use ed25519_dalek::Verifier as _;
    use ed25519_dalek::pkcs8::EncodePrivateKey;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn authorization_header_for_agent_task_serializes_signed_agent_assertion() {
        let record = test_agent_identity_record("agent-123");
        let target = AgentTaskAuthorizationTarget {
            agent_runtime_id: "agent-123",
            task_id: "task-123",
        };

        let header = authorization_header_for_agent_task(&record, target)
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
        signing_key_from_agent_private_key(&record.agent_private_key)
            .expect("signing key")
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
        let record = test_agent_identity_record("agent-123");
        let target = AgentTaskAuthorizationTarget {
            agent_runtime_id: "agent-456",
            task_id: "task-123",
        };

        let error = authorization_header_for_agent_task(&record, target)
            .expect_err("runtime mismatch should fail");

        assert_eq!(
            error.to_string(),
            "agent task runtime agent-456 does not match stored agent identity agent-123"
        );
    }

    fn test_agent_identity_record(agent_runtime_id: &str) -> AgentIdentityAuthRecord {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let private_key = signing_key
            .to_pkcs8_der()
            .expect("encode test key material");
        AgentIdentityAuthRecord {
            workspace_id: "account-123".to_string(),
            chatgpt_user_id: Some("user-123".to_string()),
            agent_runtime_id: agent_runtime_id.to_string(),
            agent_private_key: BASE64_STANDARD.encode(private_key.as_bytes()),
            registered_at: "2026-03-23T12:00:00Z".to_string(),
        }
    }
}
