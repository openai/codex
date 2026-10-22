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
use crate::RemoteCredentialMasking;
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
    for (executor_os, socket, credential_masking, configured_provider) in [
        (Platform::Linux, "/var/run/example.sock", false, false),
        (Platform::Linux, "/var/run/example.sock", true, false),
        (Platform::Linux, "/var/run/example.sock", true, true),
        (Platform::Macos, "/var/run/example.sock", false, false),
        (Platform::Macos, "/var/run/example.sock", true, true),
        (Platform::Windows, r"C:\example.sock", false, false),
        (Platform::Unknown, r"C:\example.sock", false, false),
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
        // Domain permissions serialize as a sorted map.
        config.set_denied_domains(vec!["blocked.example.com".into()]);
        config.set_allowed_domains(vec!["example.com".into()]);
        config.set_allow_unix_sockets(vec![socket.into()]);

        let mut remote = RemoteNetworkProxyConfig::from_effective_config(&config)
            .expect("supported remote config");
        config.set_credential_broker_enabled(credential_masking);
        if configured_provider {
            config.credential_providers.insert(
                "vendor".to_string(),
                crate::CredentialProviderConfig {
                    env: vec!["VENDOR_TOKEN".to_string()],
                    patterns: vec!["vendor_[a-z]{8}".to_string()],
                    url_prefix_from_env: Some("VENDOR_BASE_URL".to_string()),
                    auth: vec![crate::CredentialAuthMethod::Bearer],
                    ..Default::default()
                },
            );
        }
        if credential_masking {
            remote.enable_credential_masking(config.credential_providers.clone());
        }
        let wire = serde_json::to_value(&remote).expect("serialize remote config");
        assert_eq!(
            wire.get("credentialMasking"),
            credential_masking.then_some(&serde_json::Value::Bool(true))
        );
        assert_eq!(
            wire.get("credentialProviders"),
            configured_provider
                .then(|| serde_json::to_value(&config.credential_providers).unwrap())
                .as_ref()
        );
        assert_eq!(
            serde_json::from_value::<RemoteNetworkProxyConfig>(wire.clone())
                .expect("deserialize remote config"),
            remote
        );
        let mut disabled_wire = wire;
        disabled_wire
            .as_object_mut()
            .unwrap()
            .remove("credentialMasking");
        let mut expected_wire = disabled_wire.clone();
        expected_wire
            .as_object_mut()
            .unwrap()
            .remove("credentialProviders");
        // Ignore provider rules when the peer omits or explicitly disables masking.
        let mut explicitly_disabled_wire = disabled_wire.clone();
        explicitly_disabled_wire["credentialMasking"] = false.into();
        for wire in [disabled_wire, explicitly_disabled_wire] {
            let disabled: RemoteNetworkProxyConfig =
                serde_json::from_value(wire).expect("disabled masking ignores provider rules");
            assert!(!disabled.credential_masking_enabled());
            assert_eq!(serde_json::to_value(disabled).unwrap(), expected_wire);
        }
        let diagnostic = format!("{remote:?}");
        assert!(!diagnostic.contains("vendor_[a-z]{8}"));
        assert!(!diagnostic.contains("VENDOR_BASE_URL"));
        let round_trip = remote.clone().into_network_proxy_config();

        assert_eq!(round_trip, config);

        let state = Arc::new(
            NetworkProxyState::from_remote_launch_config(
                RemoteNetworkProxyLaunchConfig::new(remote),
                executor_os,
                &crate::RemoteCredentialBrokerCache::default(),
                &std::collections::HashMap::new(),
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
                .remote_launch_config(
                    DefaultFalse,
                    if credential_masking {
                        RemoteCredentialMasking::WithConfiguredProviders
                    } else {
                        RemoteCredentialMasking::Disabled
                    },
                )
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
        &crate::RemoteCredentialBrokerCache::default(),
        &std::collections::HashMap::new(),
    )
    .expect("remote launch state");

    assert_eq!(state.audit_metadata(), &audit_metadata);
    assert_eq!(state.environment_id(), Some("remote"));
    assert_eq!(state.execution_id().as_deref(), Some("execution-1"));
}

#[tokio::test]
async fn remote_credential_mappings_follow_the_environment_and_broker_config() {
    const TOKEN: &str = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
    let brokers = crate::RemoteCredentialBrokerCache::default();
    let launch = |environment_id: &str,
                  conversation_id: &str,
                  mode,
                  allow_local_binding,
                  env: &std::collections::HashMap<String, String>| {
        let mut proxy = RemoteNetworkProxyConfig::from_effective_config(&NetworkProxyConfig {
            enabled: true,
            allow_local_binding: Some(allow_local_binding),
            mode,
            ..NetworkProxyConfig::default()
        })
        .unwrap();
        proxy.enable_credential_masking(Default::default());
        NetworkProxyState::from_remote_launch_config(
            RemoteNetworkProxyLaunchConfig::new(proxy)
                .for_execution(environment_id.to_string(), "execution".to_string())
                .with_audit_metadata(NetworkProxyAuditMetadata {
                    conversation_id: Some(conversation_id.to_string()),
                    ..NetworkProxyAuditMetadata::default()
                }),
            Platform::Linux,
            &brokers,
            env,
        )
        .unwrap()
    };
    let mut env = std::collections::HashMap::from([
        ("GH_ENTERPRISE_TOKEN".to_string(), TOKEN.to_string()),
        (
            "GH_HOST".to_string(),
            "github.enterprise.example".to_string(),
        ),
    ]);
    let first = launch(
        "remote",
        "thread",
        NetworkMode::Full,
        /*allow_local_binding*/ false,
        &env,
    );
    first.virtualize_child_credentials(&mut env);
    let placeholder = env["GH_ENTERPRISE_TOKEN"].clone();
    assert_ne!(placeholder, TOKEN);

    let mut next_env = std::collections::HashMap::from([
        ("GH_ENTERPRISE_TOKEN".to_string(), TOKEN.to_string()),
        ("GH_HOST".to_string(), String::new()),
    ]);
    let next = launch(
        "remote",
        "thread",
        NetworkMode::Full,
        /*allow_local_binding*/ false,
        &env,
    );
    let listener = next.clone();
    next.bind_credential_broker_environment(&brokers, &next_env)
        .await
        .expect("bind startup credential context");
    next.virtualize_child_credentials(&mut next_env);
    let mut headers = rama_http::HeaderMap::from_iter([(
        rama_http::header::AUTHORIZATION,
        rama_http::HeaderValue::from_str(&format!("Bearer {placeholder}")).unwrap(),
    )]);
    let mut listener_headers = headers.clone();
    listener.inject_request_credentials("github.enterprise.example", &mut listener_headers);
    assert_eq!(listener_headers, headers);
    first.inject_request_credentials("github.enterprise.example", &mut headers);
    assert_eq!(
        headers[rama_http::header::AUTHORIZATION].to_str().unwrap(),
        format!("Bearer {TOKEN}")
    );
    drop(first);

    for (environment_id, conversation_id, allow_local_binding, expected) in [
        ("remote", "thread", false, TOKEN),
        ("other", "thread", false, placeholder.as_str()),
        ("remote", "other", false, placeholder.as_str()),
        ("remote", "thread", true, placeholder.as_str()),
    ] {
        // A different network mode must not discard mappings for the same scope.
        let state = launch(
            environment_id,
            conversation_id,
            NetworkMode::Limited,
            allow_local_binding,
            &env,
        );
        let mut headers = rama_http::HeaderMap::new();
        headers.insert(
            rama_http::header::AUTHORIZATION,
            rama_http::HeaderValue::from_str(&format!("Bearer {placeholder}")).unwrap(),
        );
        state.inject_request_credentials("github.enterprise.example", &mut headers);
        assert_eq!(
            headers[rama_http::header::AUTHORIZATION].to_str().unwrap(),
            format!("Bearer {expected}")
        );
    }
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
                .remote_launch_config(policy, RemoteCredentialMasking::Disabled)
                .await
                .ok()
                .map(|launch| launch.proxy.allow_local_binding),
            expected,
        );
    }
    Ok(())
}
