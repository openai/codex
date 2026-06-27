use crate::ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION;
use crate::EnvironmentRegistryConnectResponse;
use crate::EnvironmentRegistryRegistrationResponse;
use crate::EnvironmentRegistryTransportPolicy;
use crate::NoiseChannelIdentity;
use pretty_assertions::assert_eq;

use super::TransportPolicyCell;
use super::TransportPolicyState;

#[test]
fn registration_response_without_transport_policy_defaults_off() {
    let response: EnvironmentRegistryRegistrationResponse =
        serde_json::from_value(serde_json::json!({
            "environment_id": "environment-1",
            "url": "wss://rendezvous.test",
            "security_profile": "noise_hybrid_ik_v1",
            "executor_registration_id": "registration-1",
        }))
        .expect("legacy registration response should deserialize");

    assert_eq!(
        response,
        EnvironmentRegistryRegistrationResponse {
            environment_id: "environment-1".to_string(),
            url: "wss://rendezvous.test".to_string(),
            security_profile: "noise_hybrid_ik_v1".to_string(),
            executor_registration_id: "registration-1".to_string(),
            transport_policy: EnvironmentRegistryTransportPolicy {
                version: 0,
                assignment_epoch: "legacy".to_string(),
                outbound_tcp_nodelay: false,
                rendezvous_accepted_tcp_nodelay: false,
            },
        }
    );
    assert!(!response.transport_policy.effective_outbound_tcp_nodelay());
}

#[test]
fn transport_policy_applies_outbound_nodelay_only_for_supported_version() {
    let supported: EnvironmentRegistryTransportPolicy = serde_json::from_value(serde_json::json!({
        "version": ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION,
        "assignment_epoch": "experiment-1",
        "outbound_tcp_nodelay": true,
    }))
    .expect("supported policy should deserialize");
    let inactive: EnvironmentRegistryTransportPolicy = serde_json::from_value(serde_json::json!({
        "version": ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION,
        "assignment_epoch": "off",
        "outbound_tcp_nodelay": true,
        "rendezvous_accepted_tcp_nodelay": true,
    }))
    .expect("inactive policy should deserialize fail-closed");
    let unsupported: EnvironmentRegistryTransportPolicy =
        serde_json::from_value(serde_json::json!({
            "version": ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION + 1,
            "assignment_epoch": "future",
            "outbound_tcp_nodelay": true,
            "rendezvous_accepted_tcp_nodelay": true,
        }))
        .expect("unsupported policy should still deserialize");
    let missing_version: EnvironmentRegistryTransportPolicy =
        serde_json::from_value(serde_json::json!({
            "outbound_tcp_nodelay": true,
        }))
        .expect("policy without a version should deserialize fail-closed");

    assert_eq!(
        [
            supported.effective_outbound_tcp_nodelay(),
            inactive.effective_outbound_tcp_nodelay(),
            unsupported.effective_outbound_tcp_nodelay(),
            missing_version.effective_outbound_tcp_nodelay(),
        ],
        [true, false, false, false]
    );
    assert!(!inactive.effective_rendezvous_accepted_tcp_nodelay());
}

#[test]
fn transport_policy_cell_uses_only_supported_effective_settings() {
    let policy = |outbound_tcp_nodelay, rendezvous_accepted_tcp_nodelay| {
        EnvironmentRegistryTransportPolicy {
            version: ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION,
            assignment_epoch: "test".to_string(),
            outbound_tcp_nodelay,
            rendezvous_accepted_tcp_nodelay,
        }
    };
    let unsupported = EnvironmentRegistryTransportPolicy {
        version: ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION + 1,
        assignment_epoch: "future".to_string(),
        outbound_tcp_nodelay: true,
        rendezvous_accepted_tcp_nodelay: true,
    };

    assert_eq!(
        [
            policy(false, false).effective_cell(),
            policy(true, false).effective_cell(),
            policy(false, true).effective_cell(),
            policy(true, true).effective_cell(),
            unsupported.effective_cell(),
        ],
        [
            TransportPolicyCell::C00,
            TransportPolicyCell::C10,
            TransportPolicyCell::C01,
            TransportPolicyCell::C11,
            TransportPolicyCell::C00,
        ]
    );
}

#[test]
fn transport_policy_state_distinguishes_c00_assignment_provenance() {
    let legacy = EnvironmentRegistryTransportPolicy::default();
    let inactive = EnvironmentRegistryTransportPolicy {
        version: ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION,
        assignment_epoch: "off".to_string(),
        outbound_tcp_nodelay: true,
        rendezvous_accepted_tcp_nodelay: true,
    };
    let active = EnvironmentRegistryTransportPolicy {
        assignment_epoch: "experiment-1".to_string(),
        outbound_tcp_nodelay: false,
        rendezvous_accepted_tcp_nodelay: false,
        ..inactive
    };
    let unknown = EnvironmentRegistryTransportPolicy {
        version: ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION + 1,
        assignment_epoch: "future".to_string(),
        ..inactive
    };

    assert_eq!(
        [
            legacy.effective_cell(),
            inactive.effective_cell(),
            active.effective_cell(),
            unknown.effective_cell(),
        ],
        [TransportPolicyCell::C00; 4]
    );
    assert_eq!(
        [
            legacy.effective_state(),
            inactive.effective_state(),
            active.effective_state(),
            unknown.effective_state(),
        ],
        [
            TransportPolicyState::Legacy,
            TransportPolicyState::Inactive,
            TransportPolicyState::Active,
            TransportPolicyState::Unknown,
        ]
    );
}

#[test]
fn telemetry_assignment_epoch_rejects_unsafe_or_oversized_values() {
    let policy = |assignment_epoch: String| EnvironmentRegistryTransportPolicy {
        assignment_epoch,
        ..EnvironmentRegistryTransportPolicy::default()
    };

    assert_eq!(
        policy("experiment-1".to_string()).telemetry_assignment_epoch(),
        "experiment-1"
    );
    assert_eq!(
        policy(String::new()).telemetry_assignment_epoch(),
        "invalid"
    );
    assert_eq!(
        policy("bad\nepoch".to_string()).telemetry_assignment_epoch(),
        "invalid"
    );
    assert_eq!(
        policy("x".repeat(129)).telemetry_assignment_epoch(),
        "invalid"
    );
}

#[test]
fn connect_response_debug_redacts_authorizations() {
    let response = EnvironmentRegistryConnectResponse {
        environment_id: "environment-1".to_string(),
        url: "wss://rendezvous.test?sig=secret-url-authorization".to_string(),
        security_profile: "noise_hybrid_ik_v1".to_string(),
        executor_registration_id: "registration-1".to_string(),
        executor_public_key: NoiseChannelIdentity::generate()
            .expect("identity")
            .public_key(),
        harness_key_authorization: "secret-harness-authorization".to_string(),
        transport_policy: Default::default(),
    };

    let debug = format!("{response:?}");

    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("secret-url-authorization"));
    assert!(!debug.contains("secret-harness-authorization"));
}
