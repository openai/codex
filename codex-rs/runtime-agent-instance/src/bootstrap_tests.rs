//! Registration wire compatibility for each supported actor entry point.

use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn registration_serializes_the_supported_actor_authorizations() {
    let installation_id = "00000000-0000-4000-8000-000000000001";
    for (authorization, expected) in [
        (
            AuthenticatedActorAuthorization::Oauth {
                credential: "secret".into(),
            },
            json!({"type": "oauth", "credential": "secret"}),
        ),
        (
            AuthenticatedActorAuthorization::PersonalAccessToken {
                credential: "secret".into(),
            },
            json!({"type": "personal_access_token", "credential": "secret"}),
        ),
        (
            AuthenticatedActorAuthorization::PlatformApiKey {
                credential: "secret".into(),
            },
            json!({"type": "platform_api_key", "credential": "secret"}),
        ),
    ] {
        let request = RuntimeRegistrationRequest {
            actor_auth: authorization,
            agent_host_auth: AgentHostAuthorization::InstallationId {
                installation_id: installation_id.parse().unwrap(),
            },
            public_key: "ssh-ed25519 public-key".into(),
            metadata: RuntimeRegistrationMetadata {
                harness: Some("codex".into()),
                harness_version: Some("0.0.0".into()),
                role: RuntimeRole::OrchestratorAndExecutor,
            },
        };
        assert!(!format!("{request:?}").contains("secret"));
        assert_eq!(
            serde_json::to_value(request).unwrap(),
            json!({
                "actor_auth": expected,
                "agent_host_auth": {"type": "installation_id", "installation_id": installation_id},
                "public_key": "ssh-ed25519 public-key",
                "metadata": {"harness": "codex", "harness_version": "0.0.0", "role": "orchestrator_and_executor"},
            })
        );
    }
}

#[test]
fn registration_decodes_composite_actor_contexts() {
    for (actor, expected) in [
        (
            json!({"kind": "chatgpt", "user_id": "user", "account_id": "account"}),
            ActorInfo::Chatgpt {
                user_id: "user".into(),
                account_id: "account".into(),
            },
        ),
        (
            json!({"kind": "platform", "user_id": "user", "organization_id": "org", "project_id": "project"}),
            ActorInfo::Platform {
                user_id: "user".into(),
                organization_id: "org".into(),
                project_id: Some("project".into()),
            },
        ),
    ] {
        let response: RuntimeRegistrationResponse = serde_json::from_value(json!({
            "runtime_agent_instance_id": "runtime", "agent_host_id": "host", "actor": actor,
        }))
        .unwrap();
        assert_eq!(
            response,
            RuntimeRegistrationResponse {
                runtime_agent_instance_id: "runtime".into(),
                agent_host_id: "host".into(),
                actor: expected,
            }
        );
    }
    assert!(
        serde_json::from_value::<RuntimeRegistrationResponse>(
            json!({"runtime_agent_instance_id": "runtime"})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<ActorInfo>(json!({"kind": "platform", "user_id": "user"}))
            .is_err()
    );
}
