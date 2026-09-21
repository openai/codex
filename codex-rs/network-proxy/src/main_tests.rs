use super::parse_standalone_config;
use codex_network_proxy::NetworkMitmCaConfig;
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
        let bytes = serde_json::to_vec(&json!({"network": network})).unwrap();
        let config = parse_standalone_config(&bytes).unwrap();
        assert_eq!(
            (config.network.mitm, config.network.mitm_ca),
            (expected_mitm, expected_ca)
        );
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
        }
    }))
    .unwrap();

    let config = parse_standalone_config(&bytes).unwrap();

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
fn rejects_unknown_fields_at_every_mitm_hook_level() {
    for (network, unknown_field) in [
        (
            json!({"enabled": true, "unexpected_network": true}),
            "unexpected_network",
        ),
        (
            json!({
                "enabled": true,
                "mitm_hooks": [{
                    "host": "api.example.com",
                    "unexpected_hook": true
                }]
            }),
            "unexpected_hook",
        ),
        (
            json!({
                "enabled": true,
                "mitm_hooks": [{
                    "host": "api.example.com",
                    "match": {"unexpected_matcher": true}
                }]
            }),
            "unexpected_matcher",
        ),
        (
            json!({
                "enabled": true,
                "mitm_hooks": [{
                    "host": "api.example.com",
                    "actions": {"unexpected_action": true}
                }]
            }),
            "unexpected_action",
        ),
        (
            json!({
                "enabled": true,
                "mitm_hooks": [{
                    "host": "api.example.com",
                    "actions": {
                        "inject_request_headers": [{
                            "name": "authorization",
                            "unexpected_header": true
                        }]
                    }
                }]
            }),
            "unexpected_header",
        ),
    ] {
        let bytes = serde_json::to_vec(&json!({"network": network})).unwrap();
        let error = parse_standalone_config(&bytes).unwrap_err();
        assert!(
            error.to_string().contains(unknown_field),
            "expected {unknown_field} in {error}"
        );
    }
}

#[test]
fn rejects_trailing_json_values() {
    let error = parse_standalone_config(br#"{"network":{"enabled":true}} {}"#).unwrap_err();
    assert!(error.to_string().contains("trailing characters"));
}
