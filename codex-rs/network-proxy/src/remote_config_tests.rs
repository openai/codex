use pretty_assertions::assert_eq;

use super::RemoteNetworkProxyConfig;
use super::RemoteNetworkProxyLaunchConfig;
use crate::LocalBindingPolicy::DefaultFalse;
use crate::LocalBindingPolicy::RequireTrue;
use crate::MitmHookConfig;
use crate::NetworkMitmCaConfig;
use crate::NetworkMode;
use crate::NetworkProxy;
use crate::NetworkProxyAuditMetadata;
use crate::NetworkProxyConfig;
use crate::NetworkProxyState;
use crate::Platform;
use std::sync::Arc;

#[test]
fn optional_socket_policy_preserves_input_and_resolves_at_remote_boundary() {
    for allow_all in [None, Some(false), Some(true)] {
        let mut config = NetworkProxyConfig {
            dangerously_allow_all_unix_sockets: allow_all,
            ..NetworkProxyConfig::default()
        };
        config.set_allow_unix_sockets(Vec::new());
        let serialized = serde_json::to_value(&config).expect("serialize config");
        assert_eq!(
            serialized.get("dangerously_allow_all_unix_sockets"),
            allow_all.map(serde_json::Value::Bool).as_ref()
        );
        assert_eq!(serialized["unix_sockets"], serde_json::json!({}));
        assert_eq!(
            serde_json::from_value::<NetworkProxyConfig>(serialized).expect("deserialize config"),
            config
        );

        let remote = RemoteNetworkProxyConfig::from_effective_config(&config)
            .expect("supported remote config");
        assert_eq!(
            remote.dangerously_allow_all_unix_sockets,
            allow_all.unwrap_or(false)
        );
        config.dangerously_allow_all_unix_sockets = Some(allow_all.unwrap_or(false));
        config.allow_local_binding = Some(false);
        assert_eq!(remote.into_network_proxy_config(), config);
    }
}

#[tokio::test]
async fn round_trip_preserves_supported_effective_settings() {
    for (executor_os, socket) in [
        (Platform::Linux, "/var/run/example.sock"),
        (Platform::Macos, "/var/run/example.sock"),
        (Platform::Windows, r"C:\example.sock"),
        (Platform::Unknown, r"C:\example.sock"),
    ] {
        let mut config = NetworkProxyConfig {
            enabled: true,
            enable_socks5: false,
            enable_socks5_udp: false,
            allow_upstream_proxy: false,
            dangerously_allow_all_unix_sockets: Some(true),
            mode: NetworkMode::Limited,
            allow_local_binding: Some(true),
            ..NetworkProxyConfig::default()
        };
        config.set_allowed_domains(vec!["example.com".into()]);
        config.set_denied_domains(vec!["blocked.example.com".into()]);
        config.set_allow_unix_sockets(vec![socket.into()]);

        let remote = RemoteNetworkProxyConfig::from_effective_config(&config)
            .expect("supported remote config");
        let round_trip = remote.clone().into_network_proxy_config();

        assert_eq!(round_trip, config);

        let state = Arc::new(
            NetworkProxyState::from_remote_launch_config(
                RemoteNetworkProxyLaunchConfig::new(remote),
                executor_os,
            )
            .unwrap(),
        );
        // Policy edits and carrier construction must retain the executor's OS even
        // when the controller itself cannot interpret the socket as a native path.
        state.add_allowed_domain("added.example.com").await.unwrap();
        let proxy = NetworkProxy::builder()
            .state(state)
            .managed_by_codex(/*managed_by_codex*/ false)
            .build()
            .await
            .unwrap();
        config.upsert_domain_permission(
            "added.example.com".into(),
            crate::NetworkDomainPermission::Allow,
            crate::normalize_host,
        );
        assert_eq!(
            proxy
                .remote_launch_config(DefaultFalse)
                .await
                .unwrap()
                .proxy
                .into_network_proxy_config(),
            config
        );
    }
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
fn rejects_external_ca_even_when_proxy_is_disabled() {
    for (enabled, mitm_enabled) in [(false, false), (false, true), (true, false), (true, true)] {
        let config = NetworkProxyConfig {
            enabled,
            mitm: mitm_enabled,
            mitm_ca: Some(NetworkMitmCaConfig {
                certificate_file: "/run/proxy/ca.pem".to_string(),
                private_key_file: "/run/proxy/key.pem".to_string(),
            }),
            ..NetworkProxyConfig::default()
        };

        assert!(
            RemoteNetworkProxyConfig::from_effective_config(&config).is_err(),
            "external CA configuration must not cross the remote executor boundary (proxy enabled={enabled}, MITM enabled={mitm_enabled})"
        );
    }
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
    let state = NetworkProxyState::from_remote_launch_config(
        RemoteNetworkProxyLaunchConfig {
            proxy,
            audit_metadata: audit_metadata.clone(),
            environment_id: Some("remote".to_string()),
            execution_id: Some("execution-1".to_string()),
            policy_decision_timeout_ms: None,
        },
        Platform::native(),
    )
    .expect("remote launch state");

    assert_eq!(state.audit_metadata(), &audit_metadata);
    assert_eq!(state.environment_id(), Some("remote"));
    assert_eq!(state.execution_id().as_deref(), Some("execution-1"));
}

#[test]
fn policy_decision_callback_timeout_round_trips() {
    let config = RemoteNetworkProxyConfig::from_effective_config(&NetworkProxyConfig::default())
        .expect("supported remote config");
    let mut launch = RemoteNetworkProxyLaunchConfig::new(config);
    let without_timeout = serde_json::to_value(&launch).expect("serialize launch config");
    assert_eq!(without_timeout.get("policyDecisionTimeoutMs"), None);
    launch.policy_decision_timeout_ms = Some(900_000);
    let with_timeout = serde_json::to_value(&launch).expect("serialize launch timeout");
    assert_eq!(with_timeout["policyDecisionTimeoutMs"], 900_000);
    assert_eq!(
        serde_json::from_value::<RemoteNetworkProxyLaunchConfig>(with_timeout)
            .expect("deserialize launch timeout"),
        launch
    );
}

#[tokio::test]
async fn local_binding_is_resolved_for_each_executor() -> anyhow::Result<()> {
    for (binding, policy, expected) in [
        (None, DefaultFalse, Some(false)),
        (None, RequireTrue, Some(true)),
        (Some(false), DefaultFalse, Some(false)),
        (Some(false), RequireTrue, None),
        (Some(true), DefaultFalse, Some(true)),
    ] {
        let config = NetworkProxyConfig {
            enabled: true,
            allow_local_binding: binding,
            ..Default::default()
        };
        let mut state = crate::runtime::network_proxy_state_for_policy(config);
        state.local_binding_policy = RequireTrue;
        let proxy = NetworkProxy::builder()
            .state(Arc::new(state))
            .managed_by_codex(false)
            .build()
            .await?;
        assert_eq!(
            proxy
                .remote_launch_config(policy)
                .await
                .ok()
                .map(|launch| launch.proxy.allow_local_binding),
            expected,
        );
    }
    Ok(())
}
