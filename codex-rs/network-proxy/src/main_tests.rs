use clap::Parser;
use codex_network_proxy::NetworkMitmCaConfig;
use codex_network_proxy::StaticNetworkProxyConfig;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn enables_mitm_for_limited_mode_and_hooks() {
    for (network, expected_mitm, expected_ca) in [
        (json!({"enabled": true, "mode": "full"}), false, None),
        (
            json!({"enabled": true, "mode": "full", "mitm": true}),
            true,
            None,
        ),
        (json!({"enabled": true, "mode": "limited"}), true, None),
        (
            json!({
                "enabled": true,
                "mitm_hooks": [{"host": "api.example.com"}]
            }),
            true,
            None,
        ),
        (
            json!({
                "enabled": true,
                "mode": "limited",
                "mitm_ca": {
                    "certificate_file": "/run/proxy/ca.pem",
                    "private_key_file": "/run/proxy/key.pem"
                }
            }),
            true,
            Some(NetworkMitmCaConfig {
                certificate_file: "/run/proxy/ca.pem".to_string(),
                private_key_file: "/run/proxy/key.pem".to_string(),
            }),
        ),
    ] {
        let bytes = serde_json::to_vec(&json!({"network": network, "constraints": {}})).unwrap();
        for parse in [
            StaticNetworkProxyConfig::from_slice,
            StaticNetworkProxyConfig::from_slice_strict,
        ] {
            let config = parse(&bytes).unwrap();
            assert_eq!(
                (config.network.mitm, config.network.mitm_ca),
                (expected_mitm, expected_ca.clone())
            );
        }
    }
}

#[test]
fn accepts_dynamic_domain_socket_and_hook_matcher_keys() {
    let bytes = serde_json::to_vec(&json!({
        "network": {
            "enabled": true,
            "domains": {"api.example.com": "allow"},
            "unix_sockets": {"/tmp/example.sock": "allow"},
            "mitm_hooks": [{
                "host": "api.example.com",
                "match": {
                    "query": {"scope": ["read"]},
                    "headers": {"authorization": ["Bearer token"]}
                }
            }]
        },
        "constraints": {}
    }))
    .unwrap();

    let config = StaticNetworkProxyConfig::from_slice_strict(&bytes).unwrap();

    assert_eq!(
        config.network.allowed_domains(),
        Some(vec!["api.example.com".to_string()])
    );
    assert_eq!(
        config.network.allow_unix_sockets(),
        vec!["/tmp/example.sock".to_string()]
    );
}

#[test]
fn strict_flag_controls_config_parsing() {
    let base = ["codex-network-proxy", "--config", "policy.json"];
    let lenient = super::Args::try_parse_from(base).unwrap();
    let strict = super::Args::try_parse_from(base.into_iter().chain(["--strict"])).unwrap();
    for bytes in [
        br#"{"network":{"enabled":true}}"#.as_slice(),
        br#"{"network":{"enabled":true},"metadata":{"future_setting":true},"constraints":{}}"#,
    ] {
        assert!(lenient.parse_config(bytes).is_ok());
        assert!(strict.parse_config(bytes).is_err());
    }
    assert!(
        strict
            .parse_config(br#"{"network":{"enabled":true},"constraints":{}}"#)
            .is_ok()
    );
}
