//! Wire types for one-call runtime registration. Credentials are inputs, not identity claims.

use codex_utils_redacted_string::RedactedString;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthenticatedActorAuthorization {
    Oauth {
        credential: RedactedString,
    },
    PersonalAccessToken {
        credential: RedactedString,
    },
    PlatformApiKey {
        credential: RedactedString,
        #[serde(flatten)]
        selection: PlatformResourceSelection,
    },
}

/// API resource selectors to authenticate, not claims about the resulting actor.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize)]
pub struct PlatformResourceSelection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentHostAuthorization {
    // An installation ID is tracking metadata, not proof of device ownership.
    InstallationId { installation_id: Uuid },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRole {
    Orchestrator,
    Executor,
    OrchestratorAndExecutor,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeRegistrationMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harness_version: Option<String>,
    pub role: RuntimeRole,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeRegistrationRequest {
    pub actor_auth: AuthenticatedActorAuthorization,
    pub agent_host_auth: AgentHostAuthorization,
    pub public_key: String,
    pub metadata: RuntimeRegistrationMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct RuntimeRegistrationResponse {
    pub runtime_agent_instance_id: String,
    pub agent_host_id: String,
    pub actor: ActorInfo,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActorInfo {
    Chatgpt {
        user_id: String,
        account_id: String,
    },
    Platform {
        user_id: String,
        organization_id: String,
        project_id: Option<String>,
    },
}

#[cfg(test)]
#[path = "bootstrap_tests.rs"]
mod tests;
