use std::collections::BTreeMap;

use anyhow::Context;
use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::Signer as _;
use serde::Deserialize;
use serde::Serialize;
use tracing::debug;

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct AgentAssertionEnvelope {
    pub(crate) agent_runtime_id: String,
    pub(crate) task_id: String,
    pub(crate) timestamp: String,
    pub(crate) signature: String,
}

impl AgentIdentityManager {
    pub(crate) async fn authorization_header_for_task(
        &self,
        agent_task: &RegisteredAgentTask,
    ) -> Result<Option<String>> {
        if !self.feature_enabled {
            return Ok(None);
        }

        let Some(stored_identity) = self.ensure_registered_identity().await? else {
            return Ok(None);
        };
        anyhow::ensure!(
            stored_identity.agent_runtime_id == agent_task.agent_runtime_id,
            "agent task runtime {} does not match stored agent identity {}",
            agent_task.agent_runtime_id,
            stored_identity.agent_runtime_id
        );

        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let envelope = AgentAssertionEnvelope {
            agent_runtime_id: agent_task.agent_runtime_id.clone(),
            task_id: agent_task.task_id.clone(),
            timestamp: timestamp.clone(),
            signature: sign_agent_assertion_payload(&stored_identity, agent_task, &timestamp)?,
        };
        let serialized_assertion = serialize_agent_assertion(&envelope)?;
        debug!(
            agent_runtime_id = %envelope.agent_runtime_id,
            task_id = %envelope.task_id,
            "attaching agent assertion authorization to downstream request"
        );
        Ok(Some(format!("AgentAssertion {serialized_assertion}")))
    }
}

fn sign_agent_assertion_payload(
    stored_identity: &StoredAgentIdentity,
    agent_task: &RegisteredAgentTask,
    timestamp: &str,
) -> Result<String> {
    let signing_key = stored_identity.signing_key()?;
    let payload = format!(
        "{}:{}:{timestamp}",
        agent_task.agent_runtime_id, agent_task.task_id
    );
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use codex_keyring_store::tests::MockKeyringStore;
    use ed25519_dalek::Signature;
    use ed25519_dalek::Verifier as _;
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn authorization_header_for_task_skips_when_feature_is_disabled() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let keyring_store = Arc::new(MockKeyringStore::default());
        let secrets_manager = SecretsManager::new_with_keyring_store(
            tempdir.path().to_path_buf(),
            SecretsBackendKind::Local,
            keyring_store,
        );
        let auth_manager =
            AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ false,
            "https://chatgpt.com/backend-api/".to_string(),
            SessionSource::Cli,
            secrets_manager,
        );
        let agent_task = RegisteredAgentTask {
            agent_runtime_id: "agent-123".to_string(),
            task_id: "task-123".to_string(),
            registered_at: "2026-03-23T12:00:00Z".to_string(),
        };

        assert_eq!(
            manager
                .authorization_header_for_task(&agent_task)
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn authorization_header_for_task_serializes_signed_agent_assertion() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let keyring_store = Arc::new(MockKeyringStore::default());
        let secrets_manager = SecretsManager::new_with_keyring_store(
            tempdir.path().to_path_buf(),
            SecretsBackendKind::Local,
            keyring_store,
        );
        let auth_manager =
            AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
        let manager = AgentIdentityManager::new_for_tests(
            auth_manager,
            /*feature_enabled*/ true,
            "https://chatgpt.com/backend-api/".to_string(),
            SessionSource::Cli,
            secrets_manager,
        );
        let stored_identity = manager
            .seed_generated_identity_for_tests("agent-123")
            .await
            .expect("seed test identity");
        let agent_task = RegisteredAgentTask {
            agent_runtime_id: "agent-123".to_string(),
            task_id: "task-123".to_string(),
            registered_at: "2026-03-23T12:00:00Z".to_string(),
        };

        let header = manager
            .authorization_header_for_task(&agent_task)
            .await
            .expect("build agent assertion")
            .expect("header should exist");
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
        let signing_key = stored_identity.signing_key().expect("signing key");
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
}
