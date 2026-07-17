use pretty_assertions::assert_eq;

use super::RemoteNetworkProxyConfig;
use super::RemoteNetworkProxyLaunchConfig;
use crate::MitmHookConfig;
use crate::NetworkMode;
use crate::NetworkProxyAuditMetadata;
use crate::NetworkProxyConfig;
use crate::NetworkProxyState;

#[test]
fn round_trip_preserves_supported_effective_settings() {
    let mut config = NetworkProxyConfig {
        enabled: true,
        enable_socks5: false,
        enable_socks5_udp: false,
        allow_upstream_proxy: false,
        dangerously_allow_all_unix_sockets: true,
        mode: NetworkMode::Limited,
        allow_local_binding: true,
        ..NetworkProxyConfig::default()
    };
    config.set_allowed_domains(vec!["example.com".into()]);
    config.set_denied_domains(vec!["blocked.example.com".into()]);
    config.set_allow_unix_sockets(vec!["/var/run/example.sock".into()]);

    let remote =
        RemoteNetworkProxyConfig::from_effective_config(&config).expect("supported remote config");
    let round_trip = remote.into_network_proxy_config();

    assert_eq!(round_trip, config);
}

#[test]
fn rejects_unsupported_configuration() {
    let cases = [
        (
            "MITM",
            NetworkProxyConfig {
                enabled: true,
                mitm: true,
                ..NetworkProxyConfig::default()
            },
        ),
        (
            "credential broker",
            NetworkProxyConfig {
                enabled: true,
                credential_broker: true,
                ..NetworkProxyConfig::default()
            },
        ),
        (
            "plaintext credential injection",
            NetworkProxyConfig {
                enabled: true,
                dangerously_allow_plaintext_credential_injection: true,
                ..NetworkProxyConfig::default()
            },
        ),
        (
            "MITM hooks",
            NetworkProxyConfig {
                enabled: true,
                mitm_hooks: vec![MitmHookConfig::default()],
                ..NetworkProxyConfig::default()
            },
        ),
    ];

    for (feature, config) in cases {
        assert!(
            RemoteNetworkProxyConfig::from_effective_config(&config).is_err(),
            "{feature} must not cross the remote executor boundary"
        );
    }
}

#[test]
fn accepts_unsupported_configuration_when_proxy_is_disabled() {
    let config = NetworkProxyConfig {
        mitm: true,
        credential_broker: true,
        dangerously_allow_plaintext_credential_injection: true,
        mitm_hooks: vec![MitmHookConfig::default()],
        ..NetworkProxyConfig::default()
    };

    let remote = RemoteNetworkProxyConfig::from_effective_config(&config)
        .expect("disabled proxy configuration does not cross the executor boundary");

    assert!(!remote.enabled);
}

#[test]
fn launch_config_materializes_audit_and_execution_attribution() {
    let proxy = RemoteNetworkProxyConfig::from_effective_config(&NetworkProxyConfig {
        enabled: true,
        ..NetworkProxyConfig::default()
    })
    .expect("supported remote config");
    let audit_metadata = NetworkProxyAuditMetadata {
        conversation_id: Some("conversation-1".to_string()),
        user_account_id: Some("account-1".to_string()),
        originator: Some("codex_cli_rs".to_string()),
        model: Some("model-1".to_string()),
        ..NetworkProxyAuditMetadata::default()
    };
    let state = NetworkProxyState::from_remote_launch_config(RemoteNetworkProxyLaunchConfig {
        proxy,
        audit_metadata: audit_metadata.clone(),
        environment_id: Some("remote".to_string()),
        execution_id: Some("execution-1".to_string()),
    })
    .expect("remote launch state");

    assert_eq!(state.audit_metadata(), &audit_metadata);
    assert_eq!(state.environment_id(), Some("remote"));
    assert_eq!(state.execution_id().as_deref(), Some("execution-1"));
}
