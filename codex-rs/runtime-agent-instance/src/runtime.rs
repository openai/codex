//! A pending registration keeps its key and retry ID; a registered runtime signs v1 assertions.

use crate::AgentHost;
use crate::AuthenticatedActor;
use crate::RuntimeRegistrationRequest;
use crate::RuntimeRegistrationResponse;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_agent_identity::encode_ssh_ed25519_public_key;
use codex_utils_redacted_string::RedactedString;
use ed25519_dalek::Signer;
use ed25519_dalek::SigningKey;
use rand::TryRngCore;
use rand::rngs::OsRng;
use serde_json::json;
use std::fmt;
use std::io;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use uuid::Uuid;
use zeroize::Zeroizing;

/// One initialization, including retries. Neither this key nor the resulting runtime is persisted.
pub struct RuntimeRegistration {
    pub(crate) initialization_id: Uuid,
    pub(crate) request: RuntimeRegistrationRequest,
    pub(crate) signing_key: Arc<SigningKey>,
}

impl RuntimeRegistration {
    pub fn new(
        actor: crate::AuthenticatedActorAuthorization,
        host: crate::AgentHostAuthorization,
        metadata: crate::RuntimeRegistrationMetadata,
    ) -> io::Result<Self> {
        let mut seed = Zeroizing::new([0u8; 32]);
        OsRng
            .try_fill_bytes(seed.as_mut())
            .map_err(|_| io::Error::other("failed to generate runtime signing key"))?;
        let signing_key = SigningKey::from_bytes(&seed);
        Ok(Self {
            initialization_id: Uuid::new_v4(),
            request: RuntimeRegistrationRequest {
                actor_auth: actor,
                agent_host_auth: host,
                public_key: encode_ssh_ed25519_public_key(&signing_key.verifying_key()),
                metadata,
            },
            signing_key: Arc::new(signing_key),
        })
    }

    pub(crate) fn finish(&self, info: RuntimeRegistrationResponse) -> RuntimeAgentInstance {
        RuntimeAgentInstance {
            id: info.runtime_agent_instance_id,
            actor: AuthenticatedActor { info: info.actor },
            host: AgentHost {
                id: info.agent_host_id,
                authorization: self.request.agent_host_auth.clone(),
            },
            signing_key: self.signing_key.clone(),
        }
    }
}

/// A server-registered identity and its memory-only signer, shared for this runtime's lifetime.
pub struct RuntimeAgentInstance {
    id: String,
    pub(crate) actor: AuthenticatedActor,
    host: AgentHost,
    signing_key: Arc<SigningKey>,
}

impl RuntimeAgentInstance {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn actor(&self) -> &AuthenticatedActor {
        &self.actor
    }

    pub fn host(&self) -> &AgentHost {
        &self.host
    }

    /// Make a fresh assertion for each request attempt or connection handshake.
    pub fn assertion(&self) -> io::Result<RedactedString> {
        let issued_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| io::Error::other("system clock is before the Unix epoch"))?
            .as_secs();
        let protected =
            URL_SAFE_NO_PAD.encode(br#"{"alg":"EdDSA","typ":"openai-agent-assertion+jws"}"#);
        let payload = URL_SAFE_NO_PAD.encode(
            json!({
                "version": 1,
                "runtime_agent_instance": self.id,
                "iat": issued_at,
                "id": Uuid::new_v4(),
            })
            .to_string(),
        );
        let signing_input = format!("{protected}.{payload}");
        let signature =
            URL_SAFE_NO_PAD.encode(self.signing_key.sign(signing_input.as_bytes()).to_bytes());
        // The outer version routes the assertion; it is not part of the JWS signing input.
        let assertion = format!("1.{signing_input}.{signature}");
        if assertion.len() > 8192 {
            return Err(io::Error::other("runtime assertion exceeds the size limit"));
        }
        Ok(assertion.into())
    }
}

impl fmt::Debug for RuntimeAgentInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeAgentInstance")
            .field("runtime_agent_instance_id", &self.id)
            .finish_non_exhaustive()
    }
}
