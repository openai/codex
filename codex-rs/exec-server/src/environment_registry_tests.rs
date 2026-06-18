use crate::EnvironmentRegistryConnectResponse;
use crate::NoiseChannelIdentity;

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
    };

    let debug = format!("{response:?}");

    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("secret-url-authorization"));
    assert!(!debug.contains("secret-harness-authorization"));
}
