use super::BrokeredCredentialProvider;
use super::CredentialAuthMethod;
use super::CredentialBroker;
use super::CredentialProviderConfig;
use super::brokered_credential_marker_env_keys;
use super::brokered_credential_value_env_keys;
use crate::NetworkProxyConfig;
use base64::Engine as _;
use pretty_assertions::assert_eq;
use rama_http::HeaderMap;
use rama_http::HeaderValue;
use rama_http::header::AUTHORIZATION;
use std::collections::BTreeMap;
use std::collections::HashMap;

fn broker_for(provider: CredentialProviderConfig) -> CredentialBroker {
    let broker = CredentialBroker::new(/*enabled*/ true);
    broker.configure(&NetworkProxyConfig {
        credential_broker: true,
        credential_providers: BTreeMap::from([("custom".to_string(), provider)]),
        ..NetworkProxyConfig::default()
    });
    broker
}

#[test]
fn adjacent_credentials_keep_original_boundaries() {
    let broker = broker_for(CredentialProviderConfig {
        env: vec!["FIRST_TOKEN".to_string(), "SECOND_TOKEN".to_string()],
        patterns: vec![
            r"^(?:first_a{30}-|first_[b-z]{30}x)$".to_string(),
            r"\bsecond_[a-z]{20}\b".to_string(),
        ],
        url_prefixes: vec!["https://provider.example/v1".to_string()],
        ..CredentialProviderConfig::default()
    });
    let first = format!("first_{}-", "a".repeat(30));
    let second = format!("second_{}", "a".repeat(20));
    let original = format!("{first}{second}");
    let mut env = HashMap::from([
        ("FIRST_TOKEN".to_string(), first.clone()),
        ("SECOND_TOKEN".to_string(), second.clone()),
        ("BUNDLE".to_string(), original.clone()),
    ]);

    broker.virtualize_child_env(&mut env);
    assert_ne!(env["FIRST_TOKEN"], first);
    assert_ne!(env["SECOND_TOKEN"], second);
    assert!(env["FIRST_TOKEN"].ends_with('x'));

    let expected = format!("{}{}", env["FIRST_TOKEN"], env["SECOND_TOKEN"]);
    let mut text = original.clone();
    assert!(broker.virtualize_text(&mut text, &env));
    assert_eq!((&env["BUNDLE"], &text), (&expected, &expected));

    let mut snapshot = format!("export BUNDLE='{original}'\n");
    assert!(broker.virtualize_text(&mut snapshot, &env));
    assert_eq!(snapshot, format!("export BUNDLE='{expected}'\n"));
    assert!(broker.restore_text(&mut snapshot));
    assert_eq!(snapshot, format!("export BUNDLE='{original}'\n"));
    assert!(broker.provider_sources_allowed(&expected, "", &env, |_| true));
    for denied in ["FIRST_TOKEN", "SECOND_TOKEN"] {
        assert!(!broker.provider_sources_allowed(&expected, "", &env, |key| key != denied));
    }

    let mut boundary_control = format!("x{second}");
    assert!(broker.virtualize_text(&mut boundary_control, &env));
    assert_eq!(boundary_control, format!("x{second}"));
    let mut dummy_control = format!("x{}", env["SECOND_TOKEN"]);
    assert!(!broker.restore_text(&mut dummy_control));
    assert_eq!(dummy_control, format!("x{}", env["SECOND_TOKEN"]));

    env.remove("FIRST_TOKEN");
    env.remove("SECOND_TOKEN");
    broker.virtualize_child_env_for_environment(&mut env, Some("child"));
    assert_eq!(env["BUNDLE"], expected);
    assert_eq!(
        brokered_credential_marker_env_keys(&env),
        vec!["BUNDLE", "FIRST_TOKEN", "SECOND_TOKEN"]
    );
    assert_eq!(brokered_credential_value_env_keys(&env), vec!["BUNDLE"]);
    assert!(broker.child_alias_matches("BUNDLE", &expected, &expected, /*environment_id*/ None));
    let mut restored = env.clone();
    restored.insert("UNOBSERVED".to_string(), expected.clone());
    broker.restore_and_disable_child_env(&mut restored, &mut []);
    assert_eq!(
        restored,
        HashMap::from([
            ("BUNDLE".to_string(), original),
            ("UNOBSERVED".to_string(), expected),
        ])
    );
}

#[test]
fn adjacent_aliases_survive_rebinding_to_existing_environment() {
    for retain_sources in [false, true] {
        for target in [None, Some("child")] {
            let broker = broker_for(CredentialProviderConfig {
                env: vec!["FIRST_TOKEN".to_string(), "SECOND_TOKEN".to_string()],
                patterns: vec![
                    r"^(?:first_a{30}-|first_[b-z]{30}x)$".to_string(),
                    r"\bsecond_[a-z]{20}\b".to_string(),
                ],
                url_prefixes: vec!["https://provider.example/v1".to_string()],
                ..CredentialProviderConfig::default()
            });
            let first = format!("first_{}-", "a".repeat(30));
            let second = format!("second_{}", "a".repeat(20));
            let original = format!("{first}{second}");
            let mut parent = HashMap::from([
                ("FIRST_TOKEN".to_string(), first),
                ("SECOND_TOKEN".to_string(), second),
                ("BUNDLE".to_string(), original.clone()),
            ]);
            let mut expected = parent.clone();
            let mut child = parent.clone();
            child.remove("BUNDLE");
            broker.virtualize_child_env_for_environment(&mut parent, Some("parent"));
            broker.virtualize_child_env_for_environment(&mut child, target);
            assert_ne!(parent["FIRST_TOKEN"], child["FIRST_TOKEN"]);
            if !retain_sources {
                for key in ["FIRST_TOKEN", "SECOND_TOKEN"] {
                    parent.remove(key);
                    expected.remove(key);
                }
            }
            broker.virtualize_child_env_for_environment(&mut parent, target);
            assert_eq!(
                parent["BUNDLE"],
                format!("{}{}", child["FIRST_TOKEN"], child["SECOND_TOKEN"])
            );
            assert_eq!(
                brokered_credential_marker_env_keys(&parent),
                vec!["BUNDLE", "FIRST_TOKEN", "SECOND_TOKEN"]
            );
            let mut text = parent["BUNDLE"].clone();
            assert!(broker.restore_text(&mut text));
            assert_eq!(text, original);
            broker.restore_and_disable_child_env(&mut parent, &mut []);
            assert_eq!(parent, expected);
        }
    }
}

#[test]
fn generated_alias_spans_recognize_overlapping_dummy_candidates() {
    let broker = broker_for(CredentialProviderConfig {
        // Register the short dummy before the longer dummy containing its bytes.
        env: vec!["SECOND_TOKEN".to_string(), "FIRST_TOKEN".to_string()],
        patterns: vec![
            r"^(?:first_a{30}-|first_b{31})$".to_string(),
            r"\b(?:a{16}|b{16})\b".to_string(),
        ],
        url_prefixes: vec!["https://provider.example/v1".to_string()],
        ..CredentialProviderConfig::default()
    });
    let first = format!("first_{}-", "a".repeat(30));
    let second = "a".repeat(16);
    let original = format!("{first}{second}");
    let mut env = HashMap::from([
        ("FIRST_TOKEN".to_string(), first),
        ("SECOND_TOKEN".to_string(), second),
        ("BUNDLE".to_string(), original.clone()),
    ]);
    broker.virtualize_child_env(&mut env);
    assert_eq!(env["BUNDLE"], format!("first_{}", "b".repeat(47)));
    let state = broker.read_state();
    let second = state
        .credentials
        .iter()
        .find(|credential| credential.env_var == "SECOND_TOKEN")
        .unwrap();
    assert_eq!(second.generated_dummy_ranges(&env["BUNDLE"]), vec![37..53]);
    drop(state);
    assert!(!broker.provider_sources_allowed(&env["BUNDLE"], "", &env, |key| key == "FIRST_TOKEN"));
    let mut repeated = format!("\u{03bb}{};{}", env["BUNDLE"], env["BUNDLE"]);
    assert!(broker.restore_text(&mut repeated));
    assert_eq!(repeated, format!("\u{03bb}{original};{original}"));
    env.remove("FIRST_TOKEN");
    env.remove("SECOND_TOKEN");
    broker.virtualize_child_env(&mut env);
    assert_eq!(
        brokered_credential_marker_env_keys(&env),
        vec!["BUNDLE", "FIRST_TOKEN", "SECOND_TOKEN"]
    );
    broker.restore_and_disable_child_env(&mut env, &mut []);
    assert_eq!(env, HashMap::from([("BUNDLE".to_string(), original)]));
}

#[test]
fn adjacent_dummy_restoration_keeps_original_boundaries() {
    let broker = broker_for(CredentialProviderConfig {
        env: vec!["FIRST_TOKEN".to_string(), "SECOND_TOKEN".to_string()],
        patterns: vec![
            r"^(?:first_a{30}x|first_[b-z]{30}-)$".to_string(),
            r"\bsecond_[a-z]{20}\b".to_string(),
        ],
        url_prefixes: vec!["https://provider.example/v1".to_string()],
        ..CredentialProviderConfig::default()
    });
    let first = format!("first_{}x", "a".repeat(30));
    let second = format!("second_{}", "a".repeat(20));
    let mut env = HashMap::from([
        ("FIRST_TOKEN".to_string(), first.clone()),
        ("SECOND_TOKEN".to_string(), second.clone()),
    ]);
    broker.virtualize_child_env(&mut env);
    assert!(env["FIRST_TOKEN"].ends_with('-'));
    let mut text = format!("{}{}", env["FIRST_TOKEN"], env["SECOND_TOKEN"]);
    env.insert("BUNDLE".to_string(), text.clone());
    broker.virtualize_child_env(&mut env);
    let mut partial = env.clone();
    broker
        .read_state()
        .restore_child_env(&mut partial, |credential| {
            credential.env_var == "FIRST_TOKEN"
        });
    let mut partial_text = partial["BUNDLE"].clone();
    assert_eq!(partial_text, format!("{first}{}", env["SECOND_TOKEN"]));
    assert!(
        !broker.provider_sources_allowed(&partial_text, "", &partial, |key| key == "FIRST_TOKEN")
    );
    assert!(broker.restore_text(&mut partial_text));
    assert_eq!(partial_text, format!("{first}{second}"));
    assert!(broker.restore_text(&mut text));
    assert_eq!(text, format!("{first}{second}"));
    broker.restore_and_disable_child_env(&mut env, &mut []);
    assert_eq!(
        env,
        HashMap::from([
            ("FIRST_TOKEN".to_string(), first.clone()),
            ("SECOND_TOKEN".to_string(), second.clone()),
            ("BUNDLE".to_string(), format!("{first}{second}"))
        ])
    );
}

#[test]
fn local_proxy_bypass_preserves_credentials_and_aliases_across_reload() {
    let token = "vendor_abcdefghijklmnopqrstuvwx";
    let github = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
    for (destination, bypassed) in [
        ("http://localhost:1234/v1", true),
        ("http://127.0.0.1:1234/v1", true),
        ("http://[::1]:1234/v1", true),
        ("https://10.1.2.3/v1", true),
        ("https://172.16.2.3/v1", true),
        ("https://192.168.2.3/v1", true),
        ("https://api.localhost/v1", true),
        ("https://127.0.0.2/v1", false),
        ("https://172.32.2.3/v1", false),
        ("https://api.vendor.example/v1", false),
    ] {
        let broker = CredentialBroker::new(/*enabled*/ true);
        let mut config = NetworkProxyConfig {
            credential_broker: true,
            credential_providers: BTreeMap::from([(
                "vendor".to_string(),
                CredentialProviderConfig {
                    env: vec!["VENDOR_TOKEN".to_string()],
                    patterns: vec!["^vendor_[a-z]{24}$".to_string()],
                    url_prefixes: vec!["https://public.vendor.example/v1".to_string()],
                    url_prefix_from_env: Some("VENDOR_URL".to_string()),
                    ..CredentialProviderConfig::default()
                },
            )]),
            ..NetworkProxyConfig::default()
        };
        broker.configure(&config);
        let mut env = HashMap::from([
            ("VENDOR_TOKEN".to_string(), token.to_string()),
            ("VENDOR_URL".to_string(), destination.to_string()),
            ("AUTH_HEADER".to_string(), format!("Bearer {token}")),
            ("GH_TOKEN".to_string(), github.to_string()),
        ]);
        broker.virtualize_child_env(&mut env);
        let dummy = env["VENDOR_TOKEN"].clone();
        let github_dummy = env["GH_TOKEN"].clone();
        assert_ne!(dummy, token);
        assert_ne!(github_dummy, github);

        config.allow_local_binding = Some(true);
        let revision = broker.config_revision();
        broker.configure(&config);
        assert_eq!(broker.config_revision(), revision + 1);
        for _ in 0..2 {
            broker.virtualize_child_env(&mut env);
            let expected = if bypassed { token } else { &dummy };
            assert_eq!(
                (&env["VENDOR_TOKEN"], &env["AUTH_HEADER"], &env["GH_TOKEN"]),
                (
                    &expected.to_string(),
                    &format!("Bearer {expected}"),
                    &github_dummy
                ),
                "{destination}"
            );
            assert_eq!(
                brokered_credential_value_env_keys(&env),
                if bypassed {
                    vec!["GH_TOKEN"]
                } else {
                    vec!["AUTH_HEADER", "GH_TOKEN", "VENDOR_TOKEN"]
                }
            );
        }
        config.allow_local_binding = Some(false);
        let mut snapshot_env = env.clone();
        broker.virtualize_snapshot_env(&mut snapshot_env, /*environment_id*/ None);
        assert_eq!(snapshot_env["VENDOR_TOKEN"], dummy);
        assert_eq!(snapshot_env["AUTH_HEADER"], format!("Bearer {dummy}"));
        broker.configure(&config);
        broker.virtualize_child_env(&mut env);
        assert_eq!(env["VENDOR_TOKEN"], dummy);
        assert_eq!(env["AUTH_HEADER"], format!("Bearer {dummy}"));
    }
}

#[test]
fn child_alias_identity_survives_scoped_dummies_and_partial_direct_restoration() {
    let local = "local_abcdefghijklmnopqrstuvwx";
    let remote = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
    for allow_local_binding in [false, true] {
        let broker = CredentialBroker::new(/*enabled*/ true);
        broker.configure(&NetworkProxyConfig {
            credential_broker: true,
            allow_local_binding: Some(allow_local_binding),
            credential_providers: BTreeMap::from([(
                "local".to_string(),
                CredentialProviderConfig {
                    env: vec!["LOCAL_TOKEN".to_string()],
                    patterns: vec!["^local_[a-z]{24}$".to_string()],
                    url_prefixes: vec!["http://127.0.0.1:1234".to_string()],
                    ..CredentialProviderConfig::default()
                },
            )]),
            ..NetworkProxyConfig::default()
        });
        let real_alias = format!("Local {local}; Remote {remote}");
        let mut env = HashMap::from([
            ("LOCAL_TOKEN".to_string(), local.to_string()),
            ("GH_TOKEN".to_string(), remote.to_string()),
            ("AUTH_HEADER".to_string(), real_alias.clone()),
        ]);
        let mut snapshot = env.clone();
        broker.virtualize_snapshot_env(&mut snapshot, Some("snapshot"));
        let snapshot_alias = snapshot["AUTH_HEADER"].clone();
        broker.virtualize_child_env_for_environment(&mut env, Some("child"));
        env.remove("LOCAL_TOKEN");
        env.remove("GH_TOKEN");
        assert_eq!(env["AUTH_HEADER"].contains(local), allow_local_binding);
        assert!(!env["AUTH_HEADER"].contains(remote));
        assert!(broker.child_alias_matches(
            "AUTH_HEADER",
            &env["AUTH_HEADER"],
            &snapshot_alias,
            Some("child")
        ));
        assert!(!broker.child_alias_matches(
            "AUTH_HEADER",
            &real_alias,
            &snapshot_alias,
            Some("child")
        ));
        assert!(!broker.child_alias_matches(
            "AUTH_HEADER",
            &format!("{} altered", env["AUTH_HEADER"]),
            &snapshot_alias,
            Some("child")
        ));
        assert!(!broker.child_alias_matches(
            "OTHER_HEADER",
            &env["AUTH_HEADER"],
            &snapshot_alias,
            Some("child")
        ));
    }
}

#[test]
fn local_proxy_bypass_is_scoped_for_inherited_credentials_and_aliases() {
    let token = "vendor_abcdefghijklmnopqrstuvwx";
    for parent_is_local in [false, true] {
        for retain_source in [false, true] {
            let broker = CredentialBroker::new(/*enabled*/ true);
            broker.configure(&NetworkProxyConfig {
                credential_broker: true,
                allow_local_binding: Some(true),
                credential_providers: BTreeMap::from([(
                    "vendor".to_string(),
                    CredentialProviderConfig {
                        env: vec!["VENDOR_TOKEN".to_string()],
                        patterns: vec!["^vendor_[a-z]{24}$".to_string()],
                        url_prefix_from_env: Some("VENDOR_URL".to_string()),
                        ..CredentialProviderConfig::default()
                    },
                )]),
                ..NetworkProxyConfig::default()
            });
            let local = "http://127.0.0.1:1234/v1";
            let public = "https://api.vendor.example/v1";
            let mut snapshot = HashMap::from([
                ("VENDOR_TOKEN".to_string(), token.to_string()),
                (
                    "VENDOR_URL".to_string(),
                    if parent_is_local { local } else { public }.to_string(),
                ),
                ("AUTH_HEADER".to_string(), format!("Bearer {token}")),
            ]);
            broker.virtualize_snapshot_env(&mut snapshot, Some("parent"));
            let dummy = snapshot["VENDOR_TOKEN"].clone();
            let snapshot_alias = snapshot["AUTH_HEADER"].clone();
            let mut child = snapshot.clone();
            child.insert(
                "VENDOR_URL".to_string(),
                if parent_is_local { public } else { local }.to_string(),
            );
            if !retain_source {
                child.remove("VENDOR_TOKEN");
            }
            for (id, env, bypasses) in [
                ("child", &mut child, !parent_is_local),
                ("parent", &mut snapshot, parent_is_local),
            ] {
                broker.virtualize_child_env_for_environment(env, Some(id));
                let expected = if bypasses { token } else { &dummy };
                assert_eq!(
                    env.get("VENDOR_TOKEN").map(String::as_str),
                    (retain_source || id == "parent").then_some(expected)
                );
                assert_eq!(env["AUTH_HEADER"], format!("Bearer {expected}"));
                assert!(broker.child_alias_matches(
                    "AUTH_HEADER",
                    &env["AUTH_HEADER"],
                    &snapshot_alias,
                    Some(id)
                ));
                if !bypasses {
                    assert!(!broker.child_alias_matches(
                        "AUTH_HEADER",
                        &format!("Bearer {token}"),
                        &snapshot_alias,
                        Some(id)
                    ));
                }
            }
        }
    }
}

#[test]
fn configured_provider_virtualizes_credentials_aliases_and_snapshots() {
    let token = "stripe_live_abcdefghijklmnopqrstuvwx";
    let broker = broker_for(CredentialProviderConfig {
        env: vec!["STRIPE_API_KEY".to_string()],
        patterns: vec!["^stripe_live_[a-z]{24}$".to_string()],
        url_prefixes: vec!["api.stripe.com".to_string(), "*.stripe.example".to_string()],
        ..CredentialProviderConfig::default()
    });
    let mut env = HashMap::from([
        ("STRIPE_API_KEY".to_string(), token.to_string()),
        ("AUTH_HEADER".to_string(), format!("Bearer {token}")),
    ]);

    broker.virtualize_child_env(&mut env);

    let dummy = &env["STRIPE_API_KEY"];
    assert_ne!(dummy, token);
    assert!(
        regex::Regex::new("^stripe_live_[a-z]{24}$")
            .expect("valid credential pattern")
            .is_match(dummy)
    );
    assert_eq!(env["AUTH_HEADER"], format!("Bearer {dummy}"));
    assert_eq!(
        brokered_credential_value_env_keys(&env),
        vec!["AUTH_HEADER", "STRIPE_API_KEY"]
    );
    assert_eq!(
        broker.environment(&env).credential_keys,
        vec!["STRIPE_API_KEY".to_string()]
    );
    let mut snapshot = format!("token={token}");
    assert!(broker.virtualize_text(&mut snapshot, &env));
    assert_eq!(snapshot, format!("token={dummy}"));
    let mut unknown = "stripe_live_yyyyyyyyyyyyyyyyyyyyyyyy".to_string();
    assert!(!broker.virtualize_text(&mut unknown, &env));
    assert!(unknown.is_empty());
    assert!(broker.host_requires_mitm("api.stripe.com", /*port*/ 443));
    assert!(broker.host_requires_mitm("billing.stripe.example", /*port*/ 443));
    assert!(!broker.host_requires_mitm("stripe.example", /*port*/ 443));
    assert!(!broker.host_requires_mitm("attacker.example", /*port*/ 443));
    let dummy_header = format!("Bearer {dummy}");
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&dummy_header).expect("valid dummy authentication"),
    );
    broker.inject_request_headers("https://attacker.example/", &mut headers);
    assert_eq!(
        headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
        Some(dummy_header.as_str())
    );

    let mut unmanaged_env = env.clone();
    crate::strip_managed_proxy_env(&mut unmanaged_env);
    assert!(!unmanaged_env.contains_key("STRIPE_API_KEY"));
    env.remove("STRIPE_API_KEY");
    broker.virtualize_child_env(&mut env);
    assert_eq!(
        brokered_credential_marker_env_keys(&env),
        vec!["AUTH_HEADER", "STRIPE_API_KEY"]
    );
    assert_eq!(
        brokered_credential_value_env_keys(&env),
        vec!["AUTH_HEADER"]
    );
}

#[test]
fn configured_provider_discovers_credentials_without_canonical_variables() {
    let token = "stripe_live_abcdefghijklmnopqrstuvwx";
    let broker = broker_for(CredentialProviderConfig {
        env: vec!["STRIPE_API_KEY".to_string()],
        patterns: vec!["^stripe_live_[a-z]{24}$".to_string()],
        url_prefixes: vec!["api.stripe.com".to_string()],
        ..CredentialProviderConfig::default()
    });
    let authorization_header = format!("Bearer {token}");
    let mut env = HashMap::from([("AUTH_HEADER".to_string(), authorization_header.clone())]);

    broker.virtualize_child_env(&mut env);

    assert!(!env.contains_key("STRIPE_API_KEY"));
    let dummy_header = &env["AUTH_HEADER"];
    assert_ne!(dummy_header, &authorization_header);
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(dummy_header).expect("valid dummy authentication"),
    );
    broker.inject_request_headers("https://api.stripe.com/", &mut headers);
    assert_eq!(
        headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
        Some(authorization_header.as_str())
    );
    let mut snapshot = format!("export AUTH_HEADER='{authorization_header}'");
    assert!(broker.virtualize_text(&mut snapshot, &env));
    assert_eq!(snapshot, format!("export AUTH_HEADER='{dummy_header}'"));

    broker.restore_child_env(&mut env, &mut []);
    assert_eq!(env["AUTH_HEADER"], authorization_header);
    assert!(!env.contains_key("STRIPE_API_KEY"));
}

#[test]
fn configured_provider_uses_private_destination_context() {
    let token = "pin_abcdefgh";
    let destination = "https://api.vendor.example/v2";
    let mut config = NetworkProxyConfig {
        credential_broker: true,
        credential_providers: BTreeMap::from([(
            "vendor".to_string(),
            CredentialProviderConfig {
                env: vec!["VENDOR_PASSWORD".to_string()],
                patterns: vec!["pin_[a-z]{8}".to_string()],
                url_prefix_from_env: Some("VENDOR_HOST".to_string()),
                ..CredentialProviderConfig::default()
            },
        )]),
        ..NetworkProxyConfig::default()
    };
    config.configure_credential_broker_environment(&HashMap::from([
        ("VENDOR_HOST".to_string(), destination.to_string()),
        ("UNRELATED_SECRET".to_string(), token.to_string()),
    ]));
    assert!(!format!("{config:?}").contains(destination));
    assert!(
        !serde_json::to_string(&config)
            .unwrap()
            .contains(destination)
    );
    let broker = CredentialBroker::new(/*enabled*/ true);
    broker.configure(&config);
    let mut env = HashMap::from([
        ("VENDOR_PASSWORD".to_string(), token.to_string()),
        ("AUTH_HEADER".to_string(), format!("Bearer {token}")),
    ]);

    for environment_id in [None, Some("child")] {
        broker.virtualize_child_env_for_environment(&mut env, environment_id);
        assert!(!env.contains_key("VENDOR_HOST"));
        assert!(!env.contains_key("UNRELATED_SECRET"));
        assert_ne!(env["AUTH_HEADER"], format!("Bearer {token}"));
        let dummy_header = env["AUTH_HEADER"].clone();
        env.remove("VENDOR_PASSWORD");
        broker.virtualize_child_env_for_environment(&mut env, environment_id);
        assert_eq!(env["AUTH_HEADER"], dummy_header);
        let mut snapshot = format!("export AUTH_HEADER='Bearer {token}'");
        assert!(broker.virtualize_text(&mut snapshot, &env));
        assert_eq!(snapshot, format!("export AUTH_HEADER='{dummy_header}'"));
        for host in [destination, "https://other.example/v2"] {
            let mut headers = HeaderMap::new();
            headers.insert(AUTHORIZATION, HeaderValue::from_str(&dummy_header).unwrap());
            broker.inject_request_headers_for_environment(host, &mut headers, environment_id);
            assert_eq!(
                headers[AUTHORIZATION],
                if host == destination {
                    format!("Bearer {token}")
                } else {
                    dummy_header.clone()
                }
            );
        }
    }

    for host in ["https://override.example/v3", "", "not a valid destination"] {
        let mut env = HashMap::from([
            ("VENDOR_PASSWORD".to_string(), token.to_string()),
            ("VENDOR_HOST".to_string(), host.to_string()),
        ]);
        broker.virtualize_child_env(&mut env);
        assert_eq!(env["VENDOR_HOST"], host);
        assert_eq!(
            env["VENDOR_PASSWORD"] == token,
            !host.starts_with("https://")
        );
        let dummy_header = format!("Bearer {}", env["VENDOR_PASSWORD"]);
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&dummy_header).unwrap());
        broker.inject_request_headers(destination, &mut headers);
        assert_eq!(headers[AUTHORIZATION], format!("Bearer {token}"));
        if host.starts_with("https://") {
            headers.insert(AUTHORIZATION, HeaderValue::from_str(&dummy_header).unwrap());
            broker.inject_request_headers(host, &mut headers);
            assert_eq!(headers[AUTHORIZATION], format!("Bearer {token}"));
        }
    }
}

#[test]
fn configured_provider_preserves_credentials_without_a_resolved_destination() {
    let token = "stripe_live_abcdefghijklmnopqrstuvwx";
    let broker = broker_for(CredentialProviderConfig {
        env: vec!["STRIPE_API_KEY".to_string()],
        patterns: vec!["^stripe_live_[a-z]{24}$".to_string()],
        url_prefix_from_env: Some("STRIPE_HOST".to_string()),
        ..CredentialProviderConfig::default()
    });
    let mut bound_env = HashMap::from([
        ("STRIPE_API_KEY".to_string(), token.to_string()),
        ("STRIPE_HOST".to_string(), "api.stripe.com".to_string()),
    ]);
    broker.virtualize_child_env(&mut bound_env);
    assert_ne!(bound_env["STRIPE_API_KEY"], token);

    let mut env = HashMap::from([("STRIPE_API_KEY".to_string(), token.to_string())]);

    broker.virtualize_child_env(&mut env);

    assert_eq!(env["STRIPE_API_KEY"], token);
    assert!(broker.environment(&env).credential_keys.is_empty());
}

#[test]
fn configured_provider_does_not_guess_ambiguous_alias_destinations() {
    for (token, pattern, overlaps_builtin) in [
        (
            "shared_abcdefghijklmnopqrstuvwxyz",
            "^shared_[a-z]{26}$",
            false,
        ),
        (
            "sk-proj-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
            "^sk-proj-[A-Za-z0-9]+$",
            true,
        ),
    ] {
        let first = CredentialProviderConfig {
            env: vec!["FIRST_TOKEN".to_string()],
            patterns: vec![pattern.to_string()],
            url_prefixes: vec!["first.example".to_string()],
            ..CredentialProviderConfig::default()
        };
        let mut providers = BTreeMap::from([("first".to_string(), first.clone())]);
        if !overlaps_builtin {
            providers.insert(
                "second".to_string(),
                CredentialProviderConfig {
                    env: vec!["SECOND_TOKEN".to_string()],
                    url_prefixes: vec!["second.example".to_string()],
                    ..first
                },
            );
        }
        let broker = CredentialBroker::new(/*enabled*/ true);
        broker.configure(&NetworkProxyConfig {
            credential_broker: true,
            credential_providers: providers,
            ..NetworkProxyConfig::default()
        });
        let authorization_header = format!("Bearer {token}");
        let mut env = HashMap::from([("AUTH_HEADER".to_string(), authorization_header.clone())]);

        broker.virtualize_child_env(&mut env);

        assert_eq!(env["AUTH_HEADER"], authorization_header);
        assert!(!broker.host_requires_mitm("first.example", /*port*/ 443));
        assert!(!broker.host_requires_mitm("second.example", /*port*/ 443));
        if overlaps_builtin {
            assert!(!broker.host_requires_mitm("api.openai.com", /*port*/ 443));
        }

        env.insert("FIRST_TOKEN".to_string(), token.to_string());
        broker.virtualize_child_env(&mut env);
        assert_ne!(env["AUTH_HEADER"], authorization_header);
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&env["AUTH_HEADER"]).expect("valid dummy authentication"),
        );
        broker.inject_request_headers("https://first.example/", &mut headers);
        assert_eq!(
            headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some(authorization_header.as_str())
        );
    }
}

#[test]
fn configured_owner_keeps_prefixed_builtin_shaped_aliases_scoped() {
    let aliases = [
        ("COPIED", "x", ""),
        ("SUFFIXED", "", "_prod"),
        ("WRAPPED", "backup_", "-staging"),
        (
            "LONG_SUFFIX",
            "",
            "_production_shared_service_account_westus2",
        ),
    ];
    for (pattern, token_length, dummy_prefix) in [
        (r"\bsk-proj-[a-z]{64}\b", 64, Some("sk-proj-")),
        // The fixed real branch forces an independently generated vendor-shaped dummy.
        (
            r"\b(?:sk-proj-a{64}|vendor_[a-z]{64})\b",
            64,
            Some("vendor_"),
        ),
        // With no distinct long alternative, registration must remain fail-open.
        (r"\b(?:sk-proj-a{64}|pin)\b", 64, None),
        (r"\b(?:sk-proj-[a-z]{64}|pin)\b", 64, Some("sk-proj-")),
        (r"^sk-proj-[a-z]{7}$", 7, Some("sk-proj-")),
    ] {
        let token = format!("sk-proj-{}", "a".repeat(token_length));
        for (environment_id, retain_source) in [
            (None, true),
            (Some("local"), true),
            (None, false),
            (Some("local"), false),
        ] {
            for (static_destination, destination) in [
                (None, Some("https://configured.example/v1")),
                (Some("https://configured.example/v1"), None),
                (None, None),
            ] {
                let broker = broker_for(CredentialProviderConfig {
                    env: vec!["VENDOR_KEY".to_string()],
                    patterns: vec![pattern.to_string()],
                    url_prefixes: static_destination.into_iter().map(str::to_string).collect(),
                    url_prefix_from_env: Some("VENDOR_URL".to_string()),
                    ..CredentialProviderConfig::default()
                });
                let mut env = HashMap::from([("VENDOR_KEY".to_string(), token.clone())]);
                env.extend(aliases.map(|(key, prefix, suffix)| {
                    (key.to_string(), format!("{prefix}{token}{suffix}"))
                }));
                if let Some(destination) = destination {
                    env.insert("VENDOR_URL".to_string(), destination.to_string());
                }
                if !retain_source {
                    let parent_env = env.clone();
                    env.remove("VENDOR_KEY");
                    broker.discover_parent_credentials_for_environment(
                        &parent_env,
                        &env,
                        environment_id,
                    );
                }
                let mut original_env = env.clone();

                broker.virtualize_child_env_for_environment(&mut env, environment_id);

                // Arrive after registration so environment collision checks cannot mask a short dummy.
                let mut ordinary = "spinning".to_string();
                assert!(!broker.restore_text(&mut ordinary));
                assert_eq!(ordinary, "spinning");
                env.insert("UNRELATED".to_string(), ordinary.clone());
                original_env.insert("UNRELATED".to_string(), ordinary);
                let brokered = (static_destination.is_some() || destination.is_some())
                    && dummy_prefix.is_some();
                let value = env["COPIED"].strip_prefix('x').unwrap();
                assert_eq!(
                    env.get("VENDOR_KEY").map(String::as_str),
                    retain_source.then_some(value)
                );
                assert_eq!(value != token, brokered);
                assert!(
                    !broker
                        .host_protocols_for_environment(
                            "api.openai.com",
                            /*port*/ 443,
                            environment_id,
                        )
                        .tls
                );
                let headers_for = |value: &str| {
                    HeaderMap::from_iter([(
                        AUTHORIZATION,
                        HeaderValue::from_str(&format!("Bearer {value}")).unwrap(),
                    )])
                };
                for target in [
                    "https://api.openai.com/v1/models",
                    "https://api.github.com/v1/models",
                    "https://configured.example/v2/models",
                    "https://configured.example/v1/models",
                ] {
                    let mut headers = headers_for(value);
                    broker.inject_request_headers_for_environment(
                        target,
                        &mut headers,
                        environment_id,
                    );
                    let expected = if brokered && target == "https://configured.example/v1/models" {
                        &token
                    } else {
                        value
                    };
                    assert_eq!(headers, headers_for(expected));
                }
                let mut alias_only = env.clone();
                alias_only.remove("VENDOR_KEY");
                broker.virtualize_child_env_for_environment(&mut alias_only, environment_id);
                for (key, prefix, suffix) in aliases {
                    assert_eq!(env[key], format!("{prefix}{value}{suffix}"));
                    assert_eq!(alias_only[key], env[key]);
                    assert!(broker.provider_sources_allowed(
                        &original_env[key],
                        &env[key],
                        &original_env,
                        |source| source == "VENDOR_KEY"
                    ));
                    assert!(!broker.provider_sources_allowed(
                        &original_env[key],
                        &env[key],
                        &original_env,
                        |source| source == "OPENAI_API_KEY"
                    ));
                    let mut snapshot_alias = original_env[key].clone();
                    if brokered || token.len() >= super::MIN_EMBEDDED_CREDENTIAL_LENGTH {
                        assert_eq!(
                            broker.virtualize_text(&mut snapshot_alias, &alias_only),
                            brokered,
                        );
                        assert!(!snapshot_alias.contains(&token));
                    }
                    if brokered {
                        assert!(value.starts_with(dummy_prefix.unwrap()));
                        assert!(
                            value.len() >= token.len().min(super::MIN_EMBEDDED_CREDENTIAL_LENGTH)
                        );
                        assert!(
                            brokered_credential_marker_env_keys(&alias_only)
                                .contains(&key.to_string())
                        );
                        assert_eq!(snapshot_alias, env[key]);
                        assert!(
                            broker
                                .environment_for_text(&snapshot_alias, &env)
                                .credential_keys
                                .contains(&"VENDOR_KEY".to_string())
                        );
                        assert!(broker.source_matches_text(
                            "VENDOR_KEY",
                            &token,
                            &original_env[key]
                        ));
                        assert!(broker.child_alias_matches(
                            key,
                            &env[key],
                            &snapshot_alias,
                            environment_id,
                        ));
                        assert!(broker.restore_text(&mut snapshot_alias));
                        assert_eq!(snapshot_alias, original_env[key]);
                    }
                }
                broker.restore_and_disable_child_env(&mut env, &mut []);
                assert_eq!(env, original_env);
            }
        }
    }
}

#[test]
fn known_credential_spans_preserve_overlaps_and_uncovered_adjacent_credentials() {
    let short = format!("sk-proj-{}", "a".repeat(64));
    let long = format!("{short}bbbbbbbb");
    let known = format!(":known_{}", "b".repeat(24));
    let github = "ghp_spinningabcdefghijklmnopqrstuvwxyz0123456789";
    for (separator, extra, extra_pattern) in [
        ("", "extra_abcdefghijklmnopqrstuvwx", "^extra_[a-z]{24}$"),
        ("_", "extra_abcdefghijklmnopqrstuvwx", "^extra_[a-z]{24}$"),
        ("-", "extra_abcdefghijklmnopqrstuvwx", "^extra_[a-z]{24}$"),
        ("", "extra_abcdefghijklmnopqrstuvwx_", "^extra_[a-z]{24}_$"),
        ("", "extra_abcdefghijklmnopqrstuvwx-", "^extra_[a-z]{24}-$"),
    ] {
        let broker = CredentialBroker::new(/*enabled*/ true);
        broker.configure(&NetworkProxyConfig {
            credential_broker: true,
            credential_providers: BTreeMap::from([
                (
                    "vendor".to_string(),
                    CredentialProviderConfig {
                        env: vec![
                            "SHORT_KEY".to_string(),
                            "LONG_KEY".to_string(),
                            "KNOWN_KEY".to_string(),
                            "PIN_KEY".to_string(),
                        ],
                        patterns: vec![
                            r"\bsk-proj-[ab]{64,72}\b".to_string(),
                            "^:known_[a-z]{24}$".to_string(),
                            r"\bpin\b".to_string(),
                        ],
                        url_prefixes: vec!["https://vendor.example/v1".to_string()],
                        ..CredentialProviderConfig::default()
                    },
                ),
                (
                    "extra".to_string(),
                    CredentialProviderConfig {
                        env: vec!["EXTRA_KEY".to_string()],
                        // The broad Basic-capable alternative must not discover masked known values.
                        patterns: vec![extra_pattern.to_string(), "(?s)^.{80,}$".to_string()],
                        url_prefixes: vec!["https://extra.example/v1".to_string()],
                        auth: vec![CredentialAuthMethod::Bearer, CredentialAuthMethod::Basic],
                        ..CredentialProviderConfig::default()
                    },
                ),
            ]),
            ..NetworkProxyConfig::default()
        });
        let original = HashMap::from([
            ("SHORT_KEY".to_string(), short.clone()),
            ("LONG_KEY".to_string(), long.clone()),
            ("KNOWN_KEY".to_string(), known.clone()),
            ("PIN_KEY".to_string(), "pin".to_string()),
            // This observed short generic owner must not hide the distinct full GitHub token.
            ("GH_TOKEN".to_string(), "ghp_".to_string()),
            (
                "BUNDLE".to_string(),
                format!("{github}{separator}{long}{separator}{extra}{known}"),
            ),
            ("OVERLAP".to_string(), format!("{long}_{short}")),
        ]);
        assert!(!broker.provider_sources_allowed(
            &format!("{extra}{known}"),
            "",
            &original,
            |source| source == "KNOWN_KEY"
        ));
        let mut env = original.clone();
        broker.virtualize_child_env(&mut env);
        let state = broker.read_state();
        assert_eq!(state.credentials.len(), 6);
        assert!(
            state
                .credentials
                .iter()
                .all(|credential| !credential.real_value.contains('\0'))
        );
        let dummy = |real| {
            state
                .credentials
                .iter()
                .find(|credential| credential.real_value == real)
                .unwrap_or_else(|| {
                    panic!("missing registration for {real}, separator {separator:?}")
                })
                .dummy_value
                .clone()
        };
        let github_dummy = dummy(github);
        let extra_dummy = dummy(extra);
        assert_eq!(
            env["BUNDLE"],
            format!(
                "{github_dummy}{separator}{}{separator}{extra_dummy}{}",
                env["LONG_KEY"], env["KNOWN_KEY"]
            )
        );
        assert_eq!(
            env["OVERLAP"],
            format!("{}_{}", env["LONG_KEY"], env["SHORT_KEY"])
        );
        drop(state);
        for (key, allowed) in [("LONG_KEY", true), ("SHORT_KEY", false)] {
            assert_eq!(
                broker.provider_sources_allowed(
                    &long,
                    &env["LONG_KEY"],
                    &original,
                    |source| source == key
                ),
                allowed
            );
        }
        assert!(!broker.provider_sources_allowed(
            &original["BUNDLE"],
            &env["BUNDLE"],
            &original,
            |source| source == "LONG_KEY"
        ));
        assert!(broker.provider_sources_allowed(
            &original["BUNDLE"],
            &env["BUNDLE"],
            &original,
            |_| true
        ));
        for (real, dummy, destination) in [
            (github, &github_dummy, "https://api.github.com/v1"),
            (extra, &extra_dummy, "https://extra.example/v1"),
        ] {
            let mut headers = HeaderMap::from_iter([(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {dummy}")).unwrap(),
            )]);
            broker.inject_request_headers(destination, &mut headers);
            assert_eq!(headers[AUTHORIZATION], format!("Bearer {real}"));
        }
        broker.restore_and_disable_child_env(&mut env, &mut []);
        assert_eq!(env, original);
    }
}

#[test]
fn configured_provider_does_not_claim_embedded_builtin_credentials() {
    let github_token = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";
    let openai_token = "sk-proj-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let broker = broker_for(CredentialProviderConfig {
        env: vec!["COMBINED_TOKEN".to_string()],
        patterns: vec!["^combo_[A-Za-z0-9_-]{80,180}$".to_string()],
        url_prefixes: vec!["combo.example".to_string()],
        ..CredentialProviderConfig::default()
    });
    let combined = format!("combo_{github_token}_{openai_token}");
    let mut env = HashMap::from([("AUTH_HEADER".to_string(), combined.clone())]);

    broker.virtualize_child_env(&mut env);

    assert_ne!(env["AUTH_HEADER"], combined);
    assert!(!env["AUTH_HEADER"].contains(github_token));
    assert!(!env["AUTH_HEADER"].contains(openai_token));
    assert!(!broker.host_requires_mitm("combo.example", /*port*/ 443));
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&env["AUTH_HEADER"]).expect("valid dummy authentication"),
    );
    broker.inject_request_headers("https://combo.example/", &mut headers);
    assert_eq!(
        headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
        Some(env["AUTH_HEADER"].as_str())
    );
}

#[test]
fn configured_discovery_does_not_claim_overlapping_credential_spans() {
    let suffix = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUV";
    for token in [format!("sk-{suffix}"), format!("sk-proj-{suffix}{suffix}")] {
        for patterns in [
            vec!["^[A-Za-z0-9]{48}$".to_string()],
            vec![
                "^vendor_[a-z]{24}$".to_string(),
                "^[A-Za-z0-9]{48}$".to_string(),
            ],
        ] {
            let broker = broker_for(CredentialProviderConfig {
                env: vec!["VENDOR_TOKEN".to_string()],
                patterns,
                url_prefixes: vec!["vendor.example".to_string()],
                ..CredentialProviderConfig::default()
            });
            let mut env = HashMap::from([("AUTH_HEADER".to_string(), format!("Bearer {token}"))]);
            broker.virtualize_child_env(&mut env);
            assert!(!broker.host_requires_mitm("vendor.example", /*port*/ 443));
            assert!(broker.read_state().credentials.iter().all(|credential| {
                !matches!(
                    credential.provider,
                    BrokeredCredentialProvider::Configured(_)
                )
            }));

            // An explicit canonical source still identifies the provider unambiguously.
            env.insert("VENDOR_TOKEN".to_string(), suffix.to_string());
            broker.virtualize_child_env(&mut env);
            let mut headers = HeaderMap::from_iter([(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", env["VENDOR_TOKEN"])).unwrap(),
            )]);
            broker.inject_request_headers("https://vendor.example/", &mut headers);
            assert_eq!(headers[AUTHORIZATION], format!("Bearer {suffix}"));
        }
    }

    for (value, outer, inner) in [
        (
            format!("outer_{suffix}"),
            "^outer_[A-Za-z0-9]{48}$",
            "^[A-Za-z0-9]{48}$",
        ),
        (
            "first_aaaa.second_bbbb/cccc".to_string(),
            r"^first_[a-z]{4}\.second_[a-z]{4}$",
            "^second_[a-z]{4}/[a-z]{4}$",
        ),
    ] {
        let broker = CredentialBroker::new(/*enabled*/ true);
        broker.configure(&NetworkProxyConfig {
            credential_broker: true,
            credential_providers: [("outer", outer), ("inner", inner)]
                .into_iter()
                .map(|(id, pattern)| {
                    (
                        id.to_string(),
                        CredentialProviderConfig {
                            env: vec![format!("{}_TOKEN", id.to_uppercase())],
                            patterns: vec![pattern.to_string()],
                            url_prefixes: vec![format!("{id}.example")],
                            ..CredentialProviderConfig::default()
                        },
                    )
                })
                .collect(),
            ..NetworkProxyConfig::default()
        });
        let mut env = HashMap::from([("AUTH_HEADER".to_string(), format!("Bearer {value}"))]);
        broker.virtualize_child_env(&mut env);
        assert!(broker.read_state().credentials.is_empty());
        assert_eq!(env["AUTH_HEADER"], format!("Bearer {value}"));
    }
}

#[test]
fn configured_dummy_does_not_embed_another_provider_credential() {
    let broker = CredentialBroker::new(/*enabled*/ true);
    broker.configure(&NetworkProxyConfig {
        credential_broker: true,
        credential_providers: BTreeMap::from([
            (
                "backup".to_string(),
                CredentialProviderConfig {
                    env: vec!["BACKUP_TOKEN".to_string()],
                    patterns: vec!["^backup_(?:stage|prod)$".to_string()],
                    url_prefixes: vec!["backup.example".to_string()],
                    ..CredentialProviderConfig::default()
                },
            ),
            (
                "tenant".to_string(),
                CredentialProviderConfig {
                    env: vec!["TENANT_TOKEN".to_string()],
                    patterns: vec!["^[a-z]{4}$".to_string()],
                    url_prefixes: vec!["tenant.example".to_string()],
                    ..CredentialProviderConfig::default()
                },
            ),
        ]),
        ..NetworkProxyConfig::default()
    });
    let real_value = "backup_stage";
    let mut env = HashMap::from([("BACKUP_TOKEN".to_string(), real_value.to_string())]);

    broker.virtualize_child_env(&mut env);

    assert_eq!(env["BACKUP_TOKEN"], real_value);
}

#[test]
fn discovered_credential_aliases_rebind_when_the_destination_changes() {
    for (token, host_key) in [
        ("stripe_live_abcdefghijklmnopqrstuvwx", "STRIPE_HOST"),
        ("ghp_abcdefghijklmnopqrstuvwxyz1234567890", "GH_HOST"),
        (
            "sk-proj-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
            "OPENAI_BASE_URL",
        ),
    ] {
        let broker = broker_for(CredentialProviderConfig {
            env: vec!["STRIPE_API_KEY".to_string()],
            patterns: vec!["^stripe_live_[a-z]{24}$".to_string()],
            url_prefix_from_env: Some("STRIPE_HOST".to_string()),
            ..CredentialProviderConfig::default()
        });
        let authorization_header = format!("Bearer {token}");
        for host in ["first.example", "second.example"] {
            let host_value = if host_key == "OPENAI_BASE_URL" {
                format!("https://{host}/v1")
            } else {
                host.to_string()
            };
            let mut env = HashMap::from([
                (host_key.to_string(), host_value),
                ("AUTH_HEADER".to_string(), authorization_header.clone()),
            ]);

            broker.virtualize_child_env(&mut env);

            let mut headers = HeaderMap::new();
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&env["AUTH_HEADER"]).expect("valid dummy authentication"),
            );
            broker.inject_request_headers(&format!("https://{host}/v1"), &mut headers);
            assert_eq!(
                headers
                    .get(AUTHORIZATION)
                    .and_then(|value| value.to_str().ok()),
                Some(authorization_header.as_str()),
                "provider host: {host}"
            );
        }

        let mut unbound =
            HashMap::from([("AUTH_HEADER".to_string(), authorization_header.clone())]);
        broker.virtualize_child_env(&mut unbound);
        if host_key == "OPENAI_BASE_URL" {
            assert_ne!(unbound["AUTH_HEADER"], authorization_header);
        } else {
            assert_eq!(unbound["AUTH_HEADER"], authorization_header);
            assert!(!broker.host_requires_mitm("api.github.com", /*port*/ 443));
        }
    }
}

#[test]
fn credential_rotation_preserves_alias_destination_ownership() {
    for (key, rotated_key, host_key, prefix, static_host) in [
        (
            "PROVIDER_TOKEN",
            "PROVIDER_TOKEN",
            "PROVIDER_URL",
            "provider_",
            None,
        ),
        (
            "PROVIDER_TOKEN",
            "PROVIDER_TOKEN_FALLBACK",
            "PROVIDER_URL",
            "provider_",
            None,
        ),
        (
            "PROVIDER_TOKEN",
            "PROVIDER_TOKEN",
            "PROVIDER_URL",
            "provider_",
            Some("static.example"),
        ),
        (
            "GH_ENTERPRISE_TOKEN",
            "GH_ENTERPRISE_TOKEN",
            "GH_HOST",
            "ghp_",
            None,
        ),
        (
            "GH_ENTERPRISE_TOKEN",
            "GITHUB_ENTERPRISE_TOKEN",
            "GH_HOST",
            "ghp_",
            None,
        ),
        (
            "OPENAI_API_KEY",
            "OPENAI_API_KEY",
            "OPENAI_BASE_URL",
            "sk-proj-",
            Some("api.openai.com"),
        ),
    ] {
        for (environment, real_aliases, discover_parent) in [
            ("parent", false, false),
            ("child", false, false),
            ("parent", true, false),
            ("child", true, false),
            ("parent", true, true),
            ("child", true, true),
        ] {
            let broker = broker_for(CredentialProviderConfig {
                env: vec![
                    "PROVIDER_TOKEN".to_string(),
                    "PROVIDER_TOKEN_FALLBACK".to_string(),
                ],
                patterns: vec!["^provider_[a-z]{36}$".to_string()],
                url_prefixes: static_host
                    .map(|host| format!("https://{host}"))
                    .into_iter()
                    .collect(),
                url_prefix_from_env: Some("PROVIDER_URL".to_string()),
                ..CredentialProviderConfig::default()
            });
            let suffix_len = if key == "OPENAI_API_KEY" { 64 } else { 36 };
            let first = format!("{prefix}{}", "a".repeat(suffix_len));
            let second = format!("{prefix}{}", "b".repeat(suffix_len));
            let auxiliary = format!("{prefix}{}", "c".repeat(suffix_len));
            let endpoint = |host: &str| {
                if host_key == "GH_HOST" {
                    host.to_string()
                } else {
                    format!("https://{host}/v1")
                }
            };
            let mut env = HashMap::from([
                (key.to_string(), first.clone()),
                (host_key.to_string(), endpoint("first.example")),
                ("AUTH_HEADER".to_string(), format!("Bearer {first}")),
                (
                    "SECOND_AUTH_HEADER".to_string(),
                    format!("Bearer {auxiliary}"),
                ),
            ]);
            let parent_env = env.clone();
            broker.virtualize_child_env_for_environment(&mut env, Some("parent"));
            let old_dummy = env[key].clone();
            let alias_dummy = env["SECOND_AUTH_HEADER"]
                .strip_prefix("Bearer ")
                .unwrap()
                .to_string();
            assert_ne!(old_dummy, first);
            assert_ne!(alias_dummy, auxiliary);
            env.remove(key);
            env.insert(rotated_key.to_string(), second.clone());
            env.insert(host_key.to_string(), endpoint("second.example"));
            if real_aliases {
                env.insert("AUTH_HEADER".to_string(), format!("Bearer {first}"));
                env.insert(
                    "SECOND_AUTH_HEADER".to_string(),
                    format!("Bearer {auxiliary}"),
                );
            }
            if discover_parent {
                broker.discover_parent_credentials_for_environment(
                    &parent_env,
                    &env,
                    Some(environment),
                );
            }
            broker.virtualize_child_env_for_environment(&mut env, Some(environment));
            let new_dummy = env[rotated_key].clone();
            assert_eq!(
                env["AUTH_HEADER"],
                format!("Bearer {old_dummy}"),
                "{key}, {environment}, real_aliases={real_aliases}, discover_parent={discover_parent}"
            );
            assert_eq!(env["SECOND_AUTH_HEADER"], format!("Bearer {alias_dummy}"));
            assert!(!env.values().any(|value| {
                [&first, &second, &auxiliary]
                    .iter()
                    .any(|real| value.contains(real.as_str()))
            }));
            for (dummy, real, host, allowed) in [
                (&old_dummy, &first, "second.example", false),
                (&old_dummy, &first, "first.example", true),
                (&alias_dummy, &auxiliary, "second.example", false),
                (&alias_dummy, &auxiliary, "first.example", true),
                (&new_dummy, &second, "first.example", false),
                (&new_dummy, &second, "second.example", true),
            ] {
                let mut headers = HeaderMap::from_iter([(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {dummy}")).unwrap(),
                )]);
                broker.inject_request_headers_for_environment(
                    &format!("https://{host}/v1"),
                    &mut headers,
                    Some(environment),
                );
                assert_eq!(
                    headers[AUTHORIZATION],
                    format!("Bearer {}", if allowed { real } else { dummy }),
                    "{key}, {environment}, {host}, real_aliases={real_aliases}, discover_parent={discover_parent}"
                );
            }

            env.insert(host_key.to_string(), String::new());
            if real_aliases {
                env.insert("AUTH_HEADER".to_string(), format!("Bearer {first}"));
                env.insert(
                    "SECOND_AUTH_HEADER".to_string(),
                    format!("Bearer {auxiliary}"),
                );
            }
            if discover_parent {
                broker.discover_parent_credentials_for_environment(
                    &parent_env,
                    &env,
                    Some(environment),
                );
            }
            broker.virtualize_child_env_for_environment(&mut env, Some(environment));
            for host in ["first.example", "second.example"]
                .into_iter()
                .chain(static_host)
            {
                for (dummy, real) in [
                    (&old_dummy, &first),
                    (&alias_dummy, &auxiliary),
                    (&new_dummy, &second),
                ] {
                    let mut headers = HeaderMap::from_iter([(
                        AUTHORIZATION,
                        HeaderValue::from_str(&format!("Bearer {dummy}")).unwrap(),
                    )]);
                    broker.inject_request_headers_for_environment(
                        &format!("https://{host}/v1"),
                        &mut headers,
                        Some(environment),
                    );
                    assert_eq!(
                        headers[AUTHORIZATION],
                        format!(
                            "Bearer {}",
                            if Some(host) == static_host {
                                real
                            } else {
                                dummy
                            }
                        )
                    );
                }
            }
            let mut parent_headers = HeaderMap::from_iter([(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {old_dummy}")).unwrap(),
            )]);
            broker.inject_request_headers_for_environment(
                "https://first.example/v1",
                &mut parent_headers,
                Some("parent"),
            );
            assert_eq!(
                parent_headers[AUTHORIZATION],
                format!(
                    "Bearer {}",
                    if environment == "child" {
                        &first
                    } else {
                        &old_dummy
                    }
                )
            );
        }
    }
}

#[test]
fn configured_provider_accepts_anchored_alternatives_and_ascii_character_classes() {
    for (pattern, token) in [
        ("(?i)^token_[a-z]{24}$", "TOKEN_abcdefghijklmnopqrstuvwx"),
        ("(?i:^token_[a-z]{24}$)", "TOKEN_abcdefghijklmnopqrstuvwx"),
        (
            "(?x)   ^ token_[a-z]{24} $",
            "token_abcdefghijklmnopqrstuvwx",
        ),
        (
            "(?x)^token_[a-z]{24}$ # provider token",
            "token_abcdefghijklmnopqrstuvwx",
        ),
        (
            "(?x)^token_[a-z]{24} # [legacy provider\n$",
            "token_abcdefghijklmnopqrstuvwx",
        ),
        (r"\btoken_[a-z]{24}\b", "token_abcdefghijklmnopqrstuvwx"),
        (r"\Atoken_[a-z]{24}\z", "token_abcdefghijklmnopqrstuvwx"),
        (
            "^(token_[a-z]{8}|token_[a-z]{24})$",
            "token_abcdefghijklmnopqrstuvwx",
        ),
        (r"^token_\d{24}$", "token_012345678901234567890123"),
        ("^token_.{24}$", "token_abcdefghijklmnopqrstuvwx"),
        ("^token_[^:]{24}$", "token_abcdefghijklmnopqrstuvwx"),
        ("^token_[]$a-z]{8}$", "token_a$bcd]ef"),
    ] {
        let broker = broker_for(CredentialProviderConfig {
            env: vec!["PROVIDER_TOKEN".to_string()],
            patterns: vec![pattern.to_string()],
            url_prefixes: vec!["api.provider.example".to_string()],
            ..CredentialProviderConfig::default()
        });
        let mut env = HashMap::from([("PROVIDER_TOKEN".to_string(), token.to_string())]);

        broker.virtualize_child_env(&mut env);

        let dummy = &env["PROVIDER_TOKEN"];
        assert_ne!(dummy, token, "credential pattern: {pattern}");
        assert!(dummy.is_ascii(), "credential pattern: {pattern}");
        assert!(
            regex::Regex::new(pattern)
                .expect("valid credential pattern")
                .is_match(dummy),
            "credential pattern: {pattern}"
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {dummy}")).expect("valid dummy authentication"),
        );
        broker.inject_request_headers("https://api.provider.example/", &mut headers);
        assert_eq!(
            headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some(format!("Bearer {token}").as_str()),
            "credential pattern: {pattern}"
        );

        let replacement = if token.ends_with(|character: char| character.is_ascii_digit()) {
            '9'
        } else {
            'z'
        };
        let unknown = format!("{}{replacement}", &token[..token.len() - 1]);
        let mut copied = format!("AUTH_HEADER=Bearer {unknown}");
        assert!(
            !broker.virtualize_text(&mut copied, &env),
            "credential pattern: {pattern}"
        );
        assert!(!copied.contains(&unknown), "credential pattern: {pattern}");
    }
}

#[test]
fn configured_provider_preserves_word_boundaries_during_unbound_discovery() {
    let token = "token_abcdefghijklmnopqrstuvwx";
    let broker = broker_for(CredentialProviderConfig {
        env: vec!["PROVIDER_TOKEN".to_string()],
        patterns: vec![r"\btoken_[a-z]{24}\b".to_string()],
        url_prefixes: vec!["api.provider.example".to_string()],
        ..CredentialProviderConfig::default()
    });
    let prefixed = format!("my{token}");
    let mut env = HashMap::from([
        ("APPLICATION_ID".to_string(), prefixed.clone()),
        ("AUTH_HEADER".to_string(), format!("Bearer {token}")),
    ]);

    broker.virtualize_child_env(&mut env);

    assert_eq!(env["APPLICATION_ID"], prefixed);
    assert_ne!(env["AUTH_HEADER"], format!("Bearer {token}"));
}

#[test]
fn configured_provider_replaces_exact_known_values_with_lazy_patterns() {
    for (pattern, token) in [
        (r"^token_[a-z]+?$", "token_abcdefghijklmnopqrstuvwx"),
        (
            r"^token_(?:[a-z]|[a-z]{24})$",
            "token_abcdefghijklmnopqrstuvwx",
        ),
        (r"\btoken_[a-z]+?\b", "token_abcdefghijklmnopqrstuvwx"),
        (r"^token_\B[a-z]+?$", "token_abcdefghijklmnopqrstuvwx"),
        (r"^[a-z]{24}\b$", "abcabcabcabcabcabcabcabc"),
    ] {
        let broker = broker_for(CredentialProviderConfig {
            env: vec!["PROVIDER_TOKEN".to_string()],
            patterns: vec![pattern.to_string()],
            url_prefixes: vec!["api.provider.example".to_string()],
            ..CredentialProviderConfig::default()
        });
        let mut env = HashMap::from([
            ("PROVIDER_TOKEN".to_string(), token.to_string()),
            ("AUTH_HEADER".to_string(), format!("Bearer {token}")),
            ("REPEATED".to_string(), format!("{token}\n{token}")),
            ("ADJACENT".to_string(), format!("é{token}é")),
            ("OVERLAPPED".to_string(), format!("Bearer abc{token}")),
        ]);

        broker.virtualize_child_env(&mut env);

        let dummy = &env["PROVIDER_TOKEN"];
        assert_ne!(dummy, token);
        assert_eq!(env["AUTH_HEADER"], format!("Bearer {dummy}"));
        assert_eq!(env["REPEATED"], format!("{dummy}\n{dummy}"));
        assert_eq!(
            env["ADJACENT"],
            format!(
                "é{}é",
                if pattern.contains(r"\b") {
                    token
                } else {
                    dummy
                }
            )
        );
        assert_eq!(
            env["OVERLAPPED"],
            format!(
                "Bearer abc{}",
                if pattern.starts_with(r"\b") {
                    token
                } else {
                    dummy
                }
            )
        );
        let mut snapshot = format!("{token}\nBearer {token}\n{token}");
        assert!(broker.virtualize_text(&mut snapshot, &env));
        assert_eq!(snapshot, format!("{dummy}\nBearer {dummy}\n{dummy}"));
    }
}

#[test]
fn configured_provider_discovers_complete_unregistered_values() {
    for (pattern, token) in [
        (r"^token_[a-z]+?$", "token_abcdefghijklmnopqrstuvwx"),
        (r"\btoken_[a-z]+?\b", "token_abcdefghijklmnopqrstuvwx"),
        (r"^token_\B[a-z]+?$", "token_abcdefghijklmnopqrstuvwx"),
        (
            r"^token_(?:[a-z]{8,32}|[a-z]{24}[0-9]{8})$",
            "token_abcdefghijklmnopqrstuvwx12345678",
        ),
        (
            r"^token_(?:[a-z]+|[a-z]+[0-9]+)$",
            "token_abcdefghijklmnopqrstuvwx12345678",
        ),
        (
            r"^token_(?:[a-z]{8,32}|[a-z]{24}_[0-9]{7})$",
            "token_abcdefghijklmnopqrstuvwx_1234567",
        ),
        (
            r"^token_(?:[a-z]{8,32}|[a-z]{24}-[0-9]{7})$",
            "token_abcdefghijklmnopqrstuvwx-1234567",
        ),
        (
            r"^token_(?:[a-z]+|[a-z]+/[a-z]+)$",
            "token_abcdefghijklmnopqrstuvwx/zyxwvutsrqponmlkjihgfedcb",
        ),
        (
            r"^token_(?:[a-z]{8,128}|[a-z]{24}\.[a-z]{24})$",
            "token_abcdefghijklmnopqrstuvwx.zyxwvutsrqponmlkjihgfedc",
        ),
        (
            r"\btoken_(?:[a-z]+|[a-z]+\+[a-z]+)\b",
            "token_abcdefghijklmnopqrstuvwx+zyxwvutsrqponmlkjihgfedcb",
        ),
        (
            r"^token_(?:[a-z]+|[a-z]+:[a-z]+)$",
            "token_abcdefghijklmnopqrstuvwx:zyxwvutsrqponmlkjihgfedcb",
        ),
    ] {
        let broker = broker_for(CredentialProviderConfig {
            env: vec!["PROVIDER_TOKEN".to_string()],
            patterns: vec![pattern.to_string()],
            url_prefixes: vec!["api.provider.example".to_string()],
            ..CredentialProviderConfig::default()
        });
        let mut env = HashMap::from([
            ("AUTH_HEADER".to_string(), format!("Bearer {token}")),
            ("REPEATED".to_string(), format!("{token},{token}")),
        ]);

        let mut unknown_snapshot = format!("export AUTH_HEADER='Bearer {token}'");
        assert!(
            !broker.virtualize_text(&mut unknown_snapshot, &HashMap::new()),
            "{pattern}"
        );
        assert_eq!(
            unknown_snapshot, "export AUTH_HEADER='Bearer '",
            "{pattern}"
        );
        broker.virtualize_child_env(&mut env);

        let dummy = env["AUTH_HEADER"].strip_prefix("Bearer ").unwrap();
        assert_ne!(dummy, token, "{pattern}");
        assert_eq!(env["REPEATED"], format!("{dummy},{dummy}"), "{pattern}");
        let mut snapshot = format!("Bearer {token}");
        assert!(broker.virtualize_text(&mut snapshot, &env), "{pattern}");
        assert_eq!(snapshot, env["AUTH_HEADER"], "{pattern}");
        let mut headers = HeaderMap::from_iter([(
            AUTHORIZATION,
            HeaderValue::from_str(&env["AUTH_HEADER"]).unwrap(),
        )]);
        broker.inject_request_headers("https://api.provider.example/", &mut headers);
        assert_eq!(
            headers,
            HeaderMap::from_iter([(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
            )]),
            "{pattern}"
        );
    }
}

#[test]
fn mixed_provider_aliases_keep_credentials_on_separate_destinations() {
    let github = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";
    for (pattern, vendor, separator) in [
        ("^vendor_[a-z]{24}$", "vendor_abcdefghijklmnopqrstuvwx"),
        (
            r"^vendor_[a-z]{8}(?:\.[a-z]{8})?$",
            "vendor_abcdefgh.ijklmnop",
        ),
    ]
    .into_iter()
    .flat_map(|(pattern, vendor)| ["_", "-", ""].map(|separator| (pattern, vendor, separator)))
    {
        let broker = broker_for(CredentialProviderConfig {
            env: vec!["VENDOR_TOKEN".to_string()],
            patterns: vec![pattern.to_string()],
            url_prefixes: vec!["api.vendor.example".to_string()],
            ..CredentialProviderConfig::default()
        });
        let composite = format!("{github}{separator}{vendor}");
        let mut env = HashMap::from([("AUTH_BUNDLE".to_string(), composite.clone())]);
        broker.virtualize_child_env(&mut env);
        let records = broker.read_state();
        let github_dummy = records
            .credentials
            .iter()
            .find(|record| record.real_value == github)
            .expect("separate GitHub credential")
            .dummy_value
            .clone();
        let vendor_dummy = records
            .credentials
            .iter()
            .find(|record| record.real_value == vendor)
            .expect("separate vendor credential")
            .dummy_value
            .clone();
        drop(records);
        assert_eq!(
            env["AUTH_BUNDLE"],
            format!("{github_dummy}{separator}{vendor_dummy}")
        );
        let mut snapshot = composite;
        // Without canonical sources, the snapshot cannot authorize a multi-token alias.
        assert!(!broker.virtualize_text(&mut snapshot, &env));
        assert!(!snapshot.contains(github));
        assert!(!snapshot.contains(vendor));
        for (host, dummy, expected) in [
            ("api.github.com", &github_dummy, github),
            ("api.vendor.example", &vendor_dummy, vendor),
            ("api.github.com", &vendor_dummy, vendor_dummy.as_str()),
        ] {
            let mut headers = HeaderMap::from_iter([(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {dummy}")).unwrap(),
            )]);
            broker.inject_request_headers(&format!("https://{host}/"), &mut headers);
            assert_eq!(
                headers,
                HeaderMap::from_iter([(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {expected}")).unwrap(),
                )])
            );
        }
        let composite = format!("{github}{separator}{vendor}");
        let mut canonical = HashMap::from([("GH_TOKEN".to_string(), composite.clone())]);
        broker.virtualize_child_env(&mut canonical);
        let mut headers = HeaderMap::from_iter([(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", canonical["GH_TOKEN"])).unwrap(),
        )]);
        broker.inject_request_headers("api.github.com", &mut headers);
        assert_eq!(
            headers,
            HeaderMap::from_iter([(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {composite}")).unwrap(),
            )])
        );
    }
}

#[test]
fn configured_provider_virtualizes_short_credentials_in_aliases_and_snapshots() {
    for (pattern, token, unregistered) in [
        ("^pin_[a-z]{8}$", "pin_abcdefgh", "pin_hgfedcba"),
        (r"\bpin_[a-z]{8}\b", "pin_abcdefgh", "pin_hgfedcba"),
        (
            "^(pin_[a-z]{4}|pin_[a-z]{8})$",
            "pin_abcdefgh",
            "pin_hgfedcba",
        ),
        ("(?i)^pin_[a-z]{8}$", "PIN_abcdefgh", "PIN_hgfedcba"),
        (
            "^(pin_[a-z]{8}|key_[a-z]{8})$",
            "key_abcdefgh",
            "key_hgfedcba",
        ),
        ("^pin/[a-z]{8}$", "pin/abcdefgh", "pin/hgfedcba"),
        (r"^pin\\[a-z]{8}$", r"pin\abcdefgh", r"pin\hgfedcba"),
    ] {
        let preserves_word_boundaries = pattern.contains(r"\b");
        let broker = broker_for(CredentialProviderConfig {
            env: vec!["PROVIDER_PASSWORD".to_string()],
            patterns: vec![pattern.to_string()],
            url_prefixes: vec!["api.provider.example".to_string()],
            ..CredentialProviderConfig::default()
        });
        let mut env = HashMap::from([
            ("PROVIDER_PASSWORD".to_string(), token.to_string()),
            ("AUTH_HEADER".to_string(), format!("Bearer {token}")),
            ("BACKUP".to_string(), format!("backup_{token}_prod")),
            (
                "PATH".to_string(),
                format!("/opt/venvs/build_{token}_prod/bin:/usr/bin"),
            ),
            (
                "TOOL_PATH".to_string(),
                format!("tools/{token}/bin:/usr/bin"),
            ),
            (
                "WINDOWS_TOOL_PATH".to_string(),
                format!(r"tools\{token}\bin"),
            ),
            ("UNRELATED".to_string(), "pin_setting".to_string()),
        ]);

        broker.virtualize_child_env(&mut env);

        let dummy = env["PROVIDER_PASSWORD"].clone();
        assert_ne!(dummy, token, "credential pattern: {pattern}");
        assert_eq!(env["AUTH_HEADER"], format!("Bearer {dummy}"));
        let mut alias_only = env.clone();
        alias_only.remove("PROVIDER_PASSWORD");
        broker.virtualize_child_env(&mut alias_only);
        assert!(
            brokered_credential_marker_env_keys(&alias_only)
                .contains(&"PROVIDER_PASSWORD".to_string())
        );
        assert!(
            brokered_credential_marker_env_keys(&alias_only).contains(&"AUTH_HEADER".to_string())
        );
        assert!(
            brokered_credential_value_env_keys(&alias_only).contains(&"AUTH_HEADER".to_string())
        );
        assert_eq!(
            env["BACKUP"],
            if preserves_word_boundaries {
                format!("backup_{token}_prod")
            } else {
                format!("backup_{dummy}_prod")
            }
        );
        assert_eq!(
            env["PATH"],
            format!("/opt/venvs/build_{token}_prod/bin:/usr/bin")
        );
        assert_eq!(env["TOOL_PATH"], format!("tools/{token}/bin:/usr/bin"));
        assert_eq!(env["WINDOWS_TOOL_PATH"], format!(r"tools\{token}\bin"));
        assert_eq!(env["UNRELATED"], "pin_setting");
        let mut snapshot = format!("export AUTH_HEADER='Bearer {token}'");
        assert!(broker.virtualize_text(&mut snapshot, &env));
        assert_eq!(snapshot, format!("export AUTH_HEADER='Bearer {dummy}'"));
        for original_path in [
            "/opt/venvs/pin_hgfedcba/bin:/usr/bin",
            "tools/pin_hgfedcba/bin:/usr/bin",
            r"tools\pin_hgfedcba\bin",
        ] {
            let mut path = original_path.to_string();
            assert!(broker.virtualize_text(&mut path, &env));
            assert_eq!(path, original_path);
        }
        for (mut copied, expected_allowed) in [
            (
                format!("BACKUP=backup_{unregistered}_prod"),
                preserves_word_boundaries,
            ),
            (
                format!("WEBHOOK_URL=https://api.vendor.example/{unregistered}"),
                false,
            ),
        ] {
            assert_eq!(
                broker.virtualize_text(&mut copied, &env),
                expected_allowed,
                "credential pattern: {pattern}",
            );
            assert_eq!(
                copied.contains(unregistered),
                expected_allowed,
                "credential pattern: {pattern}",
            );
        }

        broker.restore_child_env(&mut env, &mut []);
        assert_eq!(env["PROVIDER_PASSWORD"], token);
        assert_eq!(env["AUTH_HEADER"], format!("Bearer {token}"));
        assert_eq!(env["BACKUP"], format!("backup_{token}_prod"));
        assert_eq!(
            env["PATH"],
            format!("/opt/venvs/build_{token}_prod/bin:/usr/bin")
        );
        assert_eq!(env["TOOL_PATH"], format!("tools/{token}/bin:/usr/bin"));
        assert_eq!(env["WINDOWS_TOOL_PATH"], format!(r"tools\{token}\bin"));
    }
}

#[test]
fn configured_provider_generates_bounded_independent_dummies() {
    let long_token = format!("token_{}", "a".repeat(4096));
    let long_broker = broker_for(CredentialProviderConfig {
        env: vec!["PROVIDER_TOKEN".to_string()],
        patterns: vec!["^token_[a-z]+$".to_string()],
        url_prefixes: vec!["api.provider.example".to_string()],
        ..CredentialProviderConfig::default()
    });
    let mut long_env = HashMap::from([("PROVIDER_TOKEN".to_string(), long_token.clone())]);

    long_broker.virtualize_child_env(&mut long_env);

    let long_dummy = &long_env["PROVIDER_TOKEN"];
    assert_ne!(long_dummy, &long_token);
    assert!(long_dummy.len() <= 2048);

    let narrow_token = format!("token_{}", "a".repeat(64));
    for _ in 0..3 {
        let narrow_broker = broker_for(CredentialProviderConfig {
            env: vec!["PROVIDER_TOKEN".to_string()],
            patterns: vec!["^token_[ab]{64}$".to_string()],
            url_prefixes: vec!["api.provider.example".to_string()],
            ..CredentialProviderConfig::default()
        });
        let mut narrow_env = HashMap::from([("PROVIDER_TOKEN".to_string(), narrow_token.clone())]);

        narrow_broker.virtualize_child_env(&mut narrow_env);

        let changed = narrow_token
            .bytes()
            .zip(narrow_env["PROVIDER_TOKEN"].bytes())
            .filter(|(real, dummy)| real != dummy)
            .count();
        assert!(changed >= 12, "dummy changed only {changed} bytes");
    }
}

#[test]
fn configured_provider_rejects_aliases_containing_disallowed_credentials() {
    let first = "token_abcdefghijklmnopqrstuvwx";
    let second = "token_zyxwvutsrqponmlkjihgfedc";
    let broker = broker_for(CredentialProviderConfig {
        env: vec!["FIRST_TOKEN".to_string(), "SECOND_TOKEN".to_string()],
        patterns: vec!["token_[a-z]{24}".to_string()],
        url_prefixes: vec!["api.provider.example".to_string()],
        ..CredentialProviderConfig::default()
    });
    let source_env = HashMap::from([("FIRST_TOKEN".to_string(), first.to_string())]);

    assert!(
        broker
            .provider_sources_allowed(first, "", &source_env, |source| { source == "FIRST_TOKEN" })
    );
    assert!(!broker.provider_sources_allowed(
        &format!("{first}|{second}"),
        "",
        &source_env,
        |source| source == "FIRST_TOKEN",
    ));

    let overlapping = broker_for(CredentialProviderConfig {
        env: vec!["FIRST_TOKEN".to_string(), "SECOND_TOKEN".to_string()],
        patterns: vec!["^(token_[a-z]{4}|token_[a-z]{8})$".to_string()],
        url_prefixes: vec!["api.provider.example".to_string()],
        ..CredentialProviderConfig::default()
    });
    let first = "token_abcd";
    let second = "token_abcdefgh";
    for source_env in [
        HashMap::from([
            ("FIRST_TOKEN".to_string(), first.to_string()),
            ("SECOND_TOKEN".to_string(), second.to_string()),
        ]),
        HashMap::from([("FIRST_TOKEN".to_string(), first.to_string())]),
    ] {
        assert!(
            !overlapping.provider_sources_allowed(second, "", &source_env, |source| source
                == "FIRST_TOKEN",)
        );
    }
}

#[test]
fn configured_provider_preserves_operational_values_while_redacting_url_credentials() {
    let broker = broker_for(CredentialProviderConfig {
        env: vec!["PROVIDER_PIN".to_string()],
        patterns: vec![r"^\d{3}$".to_string()],
        url_prefixes: vec!["api.provider.example".to_string()],
        ..CredentialProviderConfig::default()
    });
    let mut env = HashMap::from([
        ("PROVIDER_PIN".to_string(), "127".to_string()),
        ("AUTH_HEADER".to_string(), "Bearer 127".to_string()),
        (
            "HTTP_PROXY".to_string(),
            "http://127.0.0.1:3128".to_string(),
        ),
        ("PATH".to_string(), "/opt/sdk127/bin:/usr/bin".to_string()),
        ("HTTP_STATUS".to_string(), "443".to_string()),
        ("RETRIES".to_string(), "200".to_string()),
        (
            "WEBHOOK_URL".to_string(),
            "https://127.0.0.1/auth/127".to_string(),
        ),
    ]);

    broker.virtualize_child_env(&mut env);

    let dummy = env["PROVIDER_PIN"].clone();
    assert_ne!(dummy, "127");
    assert_eq!(env["AUTH_HEADER"], format!("Bearer {dummy}"));
    assert_eq!(env["HTTP_PROXY"], "http://127.0.0.1:3128");
    assert_eq!(env["PATH"], "/opt/sdk127/bin:/usr/bin");
    assert_eq!(env["HTTP_STATUS"], "443");
    assert_eq!(env["RETRIES"], "200");
    assert_eq!(
        env["WEBHOOK_URL"],
        format!("https://127.0.0.1/auth/{dummy}")
    );
    let mut ordinary_settings = "export HTTP_STATUS=443\nexport RETRIES=200\n".to_string();
    assert!(broker.virtualize_text(&mut ordinary_settings, &env));
    assert_eq!(
        ordinary_settings,
        "export HTTP_STATUS=443\nexport RETRIES=200\n"
    );
    let mut credential_alias = "AUTH_HEADER=Bearer 127".to_string();
    assert!(broker.virtualize_text(&mut credential_alias, &env));
    assert_eq!(credential_alias, format!("AUTH_HEADER=Bearer {dummy}"));

    broker.restore_child_env(&mut env, &mut []);
    assert_eq!(env["PROVIDER_PIN"], "127");
    assert_eq!(env["AUTH_HEADER"], "Bearer 127");
    assert_eq!(env["HTTP_PROXY"], "http://127.0.0.1:3128");
    assert_eq!(env["PATH"], "/opt/sdk127/bin:/usr/bin");
    assert_eq!(env["HTTP_STATUS"], "443");
    assert_eq!(env["RETRIES"], "200");
    assert_eq!(env["WEBHOOK_URL"], "https://127.0.0.1/auth/127");
}

#[test]
fn configured_provider_does_not_rescan_registered_credential_prefixes() {
    let token = "token_abcdefghijkl";
    let broker = broker_for(CredentialProviderConfig {
        env: vec!["PROVIDER_TOKEN".to_string()],
        patterns: vec!["token_[a-z]{4}".to_string(), "token_[a-z]{12}".to_string()],
        url_prefixes: vec!["api.provider.example".to_string()],
        ..CredentialProviderConfig::default()
    });
    let mut env = HashMap::from([("PROVIDER_TOKEN".to_string(), token.to_string())]);
    broker.virtualize_child_env(&mut env);
    let dummy = &env["PROVIDER_TOKEN"];

    let mut registered = format!("AUTH_HEADER=Bearer {dummy}");
    assert!(broker.virtualize_text(&mut registered, &env));
    assert_eq!(registered, format!("AUTH_HEADER=Bearer {dummy}"));

    let mut unknown = "AUTH_HEADER=Bearer token_zzzzzzzzzzzz".to_string();
    assert!(!broker.virtualize_text(&mut unknown, &env));
    assert_eq!(unknown, "AUTH_HEADER=Bearer ");
}

#[test]
fn configured_provider_does_not_register_a_credential_inside_an_existing_dummy() {
    let broker = broker_for(CredentialProviderConfig {
        env: vec!["PRIMARY_TOKEN".to_string(), "SECONDARY_TOKEN".to_string()],
        patterns: vec!["token_[a-z]{4}".to_string(), "token_[a-z]{12}".to_string()],
        url_prefixes: vec!["api.provider.example".to_string()],
        ..CredentialProviderConfig::default()
    });
    let mut env = HashMap::from([(
        "PRIMARY_TOKEN".to_string(),
        "token_abcdefghijkl".to_string(),
    )]);
    broker.virtualize_child_env(&mut env);
    let primary_dummy = env["PRIMARY_TOKEN"].clone();
    let overlapping_real = primary_dummy[..10].to_string();
    env.insert("SECONDARY_TOKEN".to_string(), overlapping_real.clone());

    broker.virtualize_child_env(&mut env);

    assert_eq!(env["PRIMARY_TOKEN"], primary_dummy);
    assert_eq!(env["SECONDARY_TOKEN"], overlapping_real);
}

#[test]
fn configured_provider_reload_preserves_credentials_and_rejects_overlapping_sources() {
    let token = "provider_abcdefghijklmnopqrstuvwx";
    let provider = CredentialProviderConfig {
        env: vec!["PROVIDER_TOKEN".to_string()],
        patterns: vec!["provider_[a-z]{24}".to_string()],
        url_prefixes: vec!["api.provider.example".to_string()],
        ..CredentialProviderConfig::default()
    };
    let broker = broker_for(provider.clone());
    let mut env = HashMap::from([("PROVIDER_TOKEN".to_string(), token.to_string())]);
    broker.virtualize_child_env(&mut env);
    let dummy = env["PROVIDER_TOKEN"].clone();

    let overlapping_builtin = CredentialProviderConfig {
        env: vec!["GH_TOKEN".to_string()],
        url_prefixes: vec!["builtin-overlap.example".to_string()],
        ..provider.clone()
    };
    let overlapping_configured = CredentialProviderConfig {
        url_prefixes: vec!["configured-overlap.example".to_string()],
        ..provider.clone()
    };
    let unrelated = CredentialProviderConfig {
        env: vec!["ANOTHER_TOKEN".to_string()],
        url_prefixes: vec!["another.example".to_string()],
        ..provider.clone()
    };
    broker.configure(&NetworkProxyConfig {
        credential_broker: true,
        credential_providers: BTreeMap::from([
            ("aaa-overlap".to_string(), overlapping_configured),
            ("builtin".to_string(), overlapping_builtin),
            ("custom".to_string(), provider),
            ("second".to_string(), unrelated),
        ]),
        ..NetworkProxyConfig::default()
    });

    broker.virtualize_child_env(&mut env);
    assert_eq!(env["PROVIDER_TOKEN"], dummy);
    assert!(!broker.host_requires_mitm("builtin-overlap.example", /*port*/ 443));
    assert!(!broker.host_requires_mitm("configured-overlap.example", /*port*/ 443));
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {dummy}")).expect("valid authentication"),
    );
    broker.inject_request_headers("https://api.provider.example/", &mut headers);
    assert_eq!(
        headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
        Some(format!("Bearer {token}").as_str())
    );
}

#[test]
fn configured_provider_preserves_bearer_token_basic_and_custom_header_auth() {
    let token = "provider_abcdefghijklmnopqrstuvwx";
    let github_token = "ghp_abcdefghijklmnopqrstuvwxyz1234567890";
    for (method, header_name, header_value) in [
        (
            CredentialAuthMethod::Bearer,
            "authorization",
            format!("Bearer {token}"),
        ),
        (
            CredentialAuthMethod::Token,
            "authorization",
            format!("token {token}"),
        ),
        (
            CredentialAuthMethod::Basic,
            "authorization",
            format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode(format!("user:{token}"))
            ),
        ),
        (
            CredentialAuthMethod::Header,
            "x-api-key",
            format!("Key {token}"),
        ),
    ] {
        let host = if method == CredentialAuthMethod::Header {
            "api.github.com"
        } else {
            "api.provider.example"
        };
        let broker = broker_for(CredentialProviderConfig {
            env: vec!["PROVIDER_TOKEN".to_string()],
            patterns: vec!["provider_[a-z]{24}".to_string()],
            url_prefixes: vec![host.to_string()],
            auth: if method == CredentialAuthMethod::Header {
                vec![CredentialAuthMethod::Bearer, method]
            } else {
                vec![method]
            },
            header: (method == CredentialAuthMethod::Header).then(|| "x-api-key".to_string()),
            prefix: (method == CredentialAuthMethod::Header).then(|| "Key ".to_string()),
            ..CredentialProviderConfig::default()
        });
        let mut env = HashMap::from([("PROVIDER_TOKEN".to_string(), token.to_string())]);
        if method == CredentialAuthMethod::Header {
            env.insert("GH_TOKEN".to_string(), github_token.to_string());
        }
        broker.virtualize_child_env(&mut env);
        let dummy = &env["PROVIDER_TOKEN"];
        let dummy_header = if method == CredentialAuthMethod::Basic {
            format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode(format!("user:{dummy}"))
            )
        } else {
            header_value.replace(token, dummy)
        };
        let mut headers = HeaderMap::new();
        headers.insert(
            rama_http::HeaderName::from_bytes(header_name.as_bytes()).expect("valid header"),
            HeaderValue::from_str(&dummy_header).expect("valid dummy authentication"),
        );
        if method == CredentialAuthMethod::Header {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_static("Bearer unrelated-session-token"),
            );
        }

        broker.inject_request_headers(&format!("https://{host}/"), &mut headers);

        assert_eq!(
            headers
                .get(header_name)
                .and_then(|value| value.to_str().ok()),
            Some(header_value.as_str()),
            "authentication method: {method:?}"
        );
        if method == CredentialAuthMethod::Header {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {dummy}"))
                    .expect("valid dummy authentication"),
            );
            headers.insert(
                "x-api-key",
                HeaderValue::from_str(&dummy_header).expect("valid dummy authentication"),
            );
            broker.inject_request_headers(&format!("https://{host}/"), &mut headers);
            assert_eq!(
                headers
                    .get(AUTHORIZATION)
                    .and_then(|value| value.to_str().ok()),
                Some(format!("Bearer {token}").as_str())
            );
            assert_eq!(
                headers
                    .get("x-api-key")
                    .and_then(|value| value.to_str().ok()),
                Some(header_value.as_str())
            );

            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", env["GH_TOKEN"]))
                    .expect("valid dummy authentication"),
            );
            headers.insert(
                "x-api-key",
                HeaderValue::from_str(&dummy_header).expect("valid dummy authentication"),
            );
            broker.inject_request_headers(&format!("https://{host}/"), &mut headers);
            assert_eq!(
                headers
                    .get(AUTHORIZATION)
                    .and_then(|value| value.to_str().ok()),
                Some(format!("Bearer {github_token}").as_str())
            );
            assert_eq!(
                headers
                    .get("x-api-key")
                    .and_then(|value| value.to_str().ok()),
                Some(header_value.as_str())
            );
        }
    }
}

#[test]
fn configured_provider_translates_full_basic_pairs_only_on_exact_match() {
    let token = "user:provider_abcdefghijklmnopqrstuvwx:abcd";
    for (pattern, brokered) in [
        ("^user:provider_[a-z]{24}:[a-z]{4}$", true),
        ("^[a-z]{4}(?::provider_[a-z]{24}:[a-z]{4})?$", true),
        (
            "^(?:user:provider_abcdefghijklmnopqrstuvwx:abcd|[a-z]{32})$",
            false,
        ),
    ] {
        let broker = broker_for(CredentialProviderConfig {
            env: vec!["PROVIDER_CREDENTIALS".to_string()],
            patterns: vec![pattern.to_string()],
            url_prefixes: vec!["api.provider.example".to_string()],
            auth: vec![CredentialAuthMethod::Basic],
            ..CredentialProviderConfig::default()
        });
        let mut env = HashMap::from([("PROVIDER_CREDENTIALS".to_string(), token.to_string())]);
        broker.virtualize_child_env(&mut env);
        let dummy = &env["PROVIDER_CREDENTIALS"];
        assert_eq!(dummy != token, brokered);

        for (destination, suffix, injected) in [
            ("https://api.provider.example/", "", true),
            ("https://api.provider.example/", "extra", false),
            ("https://other.example/", "", false),
        ] {
            // curl --user adds an empty password when the argument has no colon.
            let pair = if dummy.contains(':') {
                dummy.clone()
            } else {
                format!("{dummy}:")
            };
            let input = format!("{pair}{suffix}");
            let header = |value: &str| {
                HeaderMap::from_iter([(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!(
                        "bAsIc {}",
                        base64::engine::general_purpose::STANDARD.encode(value)
                    ))
                    .expect("valid Basic authentication"),
                )])
            };
            let mut headers = header(&input);

            broker.inject_request_headers(destination, &mut headers);

            assert_eq!(headers, header(if injected { token } else { &input }));
        }
    }
}

#[test]
fn configured_provider_generates_unicode_basic_dummies() {
    for (pattern, secret, token) in [
        (
            "^pass_wörd[a-z]{8} $",
            "pass_wördabcdefgh ".to_string(),
            "pass_wördabcdefgh ".to_string(),
        ),
        (
            r"^pass_\B[éöü]{24}[a-z]$",
            "éöü".repeat(8),
            format!("pass_{}a", "éöü".repeat(8)),
        ),
        (r"^(?:α{32}|β{32})$", "α".repeat(32), "α".repeat(32)),
        (
            r"^(?:α{32}|β{32}|\x00{32})$",
            "α".repeat(32),
            "α".repeat(32),
        ),
        (
            r"^pass_\B(?:[éöü]{24}|[-+]{24})[a-z]$",
            "éöü".repeat(8),
            format!("pass_{}a", "éöü".repeat(8)),
        ),
    ] {
        let broker = broker_for(CredentialProviderConfig {
            env: vec!["PROVIDER_PASSWORD".to_string()],
            patterns: vec![pattern.to_string()],
            url_prefixes: vec!["api.provider.example".to_string()],
            auth: vec![CredentialAuthMethod::Basic],
            ..CredentialProviderConfig::default()
        });
        let mut env = HashMap::from([("PROVIDER_PASSWORD".to_string(), token.clone())]);

        broker.virtualize_child_env(&mut env);

        let dummy = &env["PROVIDER_PASSWORD"];
        assert_ne!(dummy, &token);
        assert!(!dummy.contains('\0'));
        assert!(!dummy.contains(&secret));
        assert!(regex::Regex::new(pattern).unwrap().is_match(dummy));
        let mut headers = HeaderMap::from_iter([(
            AUTHORIZATION,
            HeaderValue::from_str(&format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode(format!("user:{dummy}"))
            ))
            .expect("valid dummy authentication"),
        )]);
        broker.inject_request_headers("https://api.provider.example/", &mut headers);
        let expected = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("user:{token}"))
        );
        assert_eq!(
            headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some(expected.as_str())
        );
    }
}

#[test]
fn configured_provider_does_not_break_usable_auth_methods_when_generating_dummies() {
    let token = "provider_abcdefghijklmnopqrstuvwx";
    for (alternative, method) in [
        "β{32}",
        "provider_[a-z]{23} ",
        " provider_[a-z]{23}",
        "provider_[a-z]{23}\\t",
        "provider_[a-z]{12}:[a-z]{12}",
    ]
    .into_iter()
    .flat_map(|alternative| {
        [CredentialAuthMethod::Bearer, CredentialAuthMethod::Header]
            .map(|method| (alternative, method))
    })
    .chain(std::iter::once((r"\x00{24}", CredentialAuthMethod::Basic)))
    {
        let broker = broker_for(CredentialProviderConfig {
            env: vec!["PROVIDER_PASSWORD".to_string()],
            patterns: vec![format!("^(?:{token}|{alternative})$")],
            url_prefixes: vec!["api.provider.example".to_string()],
            auth: vec![method, CredentialAuthMethod::Basic],
            header: (method == CredentialAuthMethod::Header).then(|| "x-api-key".to_string()),
            ..CredentialProviderConfig::default()
        });
        let mut env = HashMap::from([("PROVIDER_PASSWORD".to_string(), token.to_string())]);

        broker.virtualize_child_env(&mut env);

        assert_eq!(env["PROVIDER_PASSWORD"], token);
    }
}

#[test]
fn configured_basic_dummies_preserve_username_password_and_whole_value_auth() {
    let token = "provider_abcdefghijklmnopqrstuvwx";
    let second = "provider_zyxwvutsrqponmlkjihgfedcb";
    let github = "ghp_abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGH";
    let broker = broker_for(CredentialProviderConfig {
        env: vec![
            "PROVIDER_TOKEN".to_string(),
            "PROVIDER_SECRET".to_string(),
            "PROVIDER_COPY".to_string(),
        ],
        patterns: vec!["^provider_(?:[a-z]{24}|[a-z]{12}:[a-z]{12})$".to_string()],
        url_prefixes: vec!["https://api.github.com/v1".to_string()],
        auth: vec![CredentialAuthMethod::Basic],
        ..CredentialProviderConfig::default()
    });
    let mut env = HashMap::from([
        ("PROVIDER_TOKEN".to_string(), token.to_string()),
        ("PROVIDER_SECRET".to_string(), second.to_string()),
        ("PROVIDER_COPY".to_string(), token.to_string()),
        ("GH_TOKEN".to_string(), github.to_string()),
    ]);
    broker.virtualize_child_env(&mut env);
    let dummy = &env["PROVIDER_TOKEN"];
    assert_ne!(dummy, token);
    assert!(!dummy.contains(':'));
    for (input, expected) in [
        (
            format!("{dummy}:x-oauth-basic"),
            format!("{token}:x-oauth-basic"),
        ),
        (format!("user:{dummy}"), format!("user:{token}")),
        (format!("{dummy}:{dummy}"), format!("{token}:{token}")),
        (
            format!("{dummy}:{}", env["PROVIDER_COPY"]),
            format!("{token}:{token}"),
        ),
        (
            format!("{}:{dummy}", env["PROVIDER_COPY"]),
            format!("{token}:{token}"),
        ),
        (
            format!("{dummy}:{}", env["PROVIDER_SECRET"]),
            format!("{token}:{second}"),
        ),
        (
            format!("{}:{dummy}", env["PROVIDER_SECRET"]),
            format!("{second}:{token}"),
        ),
        (
            format!("{}:{dummy}", env["GH_TOKEN"]),
            format!("{github}:{token}"),
        ),
        (
            format!("{dummy}:{}", env["GH_TOKEN"]),
            format!("{token}:{github}"),
        ),
        (dummy.clone(), token.to_string()),
    ] {
        let headers_for = |value: &str| {
            HeaderMap::from_iter([(
                AUTHORIZATION,
                HeaderValue::from_str(&format!(
                    "Basic {}",
                    base64::engine::general_purpose::STANDARD.encode(value)
                ))
                .unwrap(),
            )])
        };
        let mut headers = headers_for(&input);
        broker.inject_request_headers("https://other.example/", &mut headers);
        assert_eq!(headers, headers_for(&input));
        broker.inject_request_headers(
            "https://api.github.com/public/%2e%2e/v1/models",
            &mut headers,
        );
        assert_eq!(headers, headers_for(&input));
        broker.inject_request_headers("https://api.github.com/v1/models", &mut headers);
        assert_eq!(headers, headers_for(&expected));
    }
}

#[test]
fn configured_provider_destination_history_is_scoped_to_the_environment() {
    let token = "stripe_live_abcdefghijklmnopqrstuvwx";
    let broker = broker_for(CredentialProviderConfig {
        env: vec!["STRIPE_API_KEY".to_string()],
        patterns: vec!["^stripe_live_[a-z]{24}$".to_string()],
        url_prefixes: vec!["static.example".to_string()],
        url_prefix_from_env: Some("STRIPE_HOST".to_string()),
        ..CredentialProviderConfig::default()
    });
    let mut first_env = HashMap::from([
        ("STRIPE_API_KEY".to_string(), token.to_string()),
        ("STRIPE_HOST".to_string(), "first.example".to_string()),
    ]);
    broker.virtualize_child_env_for_environment(&mut first_env, Some("first-environment"));
    let first_dummy = first_env["STRIPE_API_KEY"].clone();
    let mut second_env = HashMap::from([
        ("STRIPE_HOST".to_string(), "second.example".to_string()),
        ("AUTH_HEADER".to_string(), format!("Bearer {first_dummy}")),
    ]);

    broker.virtualize_child_env_for_environment(&mut second_env, Some("second-environment"));

    let second_dummy = second_env["AUTH_HEADER"]
        .strip_prefix("Bearer ")
        .expect("dummy bearer credential")
        .to_string();
    assert_eq!(second_dummy, first_dummy);
    assert!(!second_env.contains_key("STRIPE_API_KEY"));
    assert_eq!(second_env["AUTH_HEADER"], format!("Bearer {second_dummy}"));
    let translated = |environment_id: &str, destination: &str, dummy: &str| {
        let mut headers = HeaderMap::from_iter([(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {dummy}")).expect("valid dummy authentication"),
        )]);
        broker.inject_request_headers_for_environment(
            &format!("https://{destination}/"),
            &mut headers,
            Some(environment_id),
        );
        headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .expect("authorization header")
            .to_string()
    };
    assert_eq!(
        translated("first-environment", "first.example", &first_dummy),
        format!("Bearer {token}")
    );
    assert_eq!(
        translated("second-environment", "second.example", &second_dummy),
        format!("Bearer {token}")
    );
    assert_eq!(
        translated("first-environment", "static.example", &first_dummy),
        format!("Bearer {token}")
    );

    second_env.insert("STRIPE_HOST".to_string(), "third.example".to_string());
    broker.virtualize_child_env_for_environment(&mut second_env, Some("second-environment"));

    assert_eq!(second_env["AUTH_HEADER"], format!("Bearer {second_dummy}"));
    assert_eq!(
        translated("second-environment", "second.example", &second_dummy),
        format!("Bearer {token}")
    );
    assert_eq!(
        translated("second-environment", "third.example", &second_dummy),
        format!("Bearer {token}")
    );
    assert_eq!(
        translated("first-environment", "first.example", &first_dummy),
        format!("Bearer {token}")
    );
    assert_eq!(
        translated("second-environment", "first.example", &second_dummy),
        format!("Bearer {second_dummy}")
    );
    for host in ["second.example", "third.example"] {
        assert_eq!(
            translated("first-environment", host, &first_dummy),
            format!("Bearer {first_dummy}")
        );
    }

    let revision = broker.config_revision();
    broker.configure(&NetworkProxyConfig {
        credential_broker: true,
        ..NetworkProxyConfig::default()
    });
    assert_eq!(broker.config_revision(), revision + 1);
    for host in ["second.example", "third.example", "static.example"] {
        assert_eq!(
            translated("second-environment", host, &second_dummy),
            format!("Bearer {second_dummy}")
        );
        assert!(
            !broker
                .host_protocols_for_environment(host, /*port*/ 443, Some("second-environment"),)
                .tls
        );
    }
}

#[test]
fn configured_provider_preserves_filtered_destinations_but_honors_explicit_overrides() {
    let token = "stripe_live_abcdefghijklmnopqrstuvwx";
    let second_token = "stripe_live_zyxwvutsrqponmlkjihgfedc";
    let broker = broker_for(CredentialProviderConfig {
        env: vec!["STRIPE_API_KEY".to_string()],
        patterns: vec!["^stripe_live_[a-z]{24}$".to_string()],
        url_prefix_from_env: Some("STRIPE_HOST".to_string()),
        ..CredentialProviderConfig::default()
    });
    let mut env = HashMap::from([
        ("STRIPE_API_KEY".to_string(), token.to_string()),
        ("STRIPE_HOST".to_string(), "first.example".to_string()),
        ("AUTH_HEADER".to_string(), format!("Bearer {token}")),
        (
            "SECOND_AUTH_HEADER".to_string(),
            format!("Bearer {second_token}"),
        ),
    ]);
    broker.virtualize_child_env_for_environment(&mut env, Some("environment"));
    let dummy = env["STRIPE_API_KEY"].clone();

    let mut filtered_child = env.clone();
    filtered_child.remove("STRIPE_HOST");
    let expected = filtered_child.clone();
    broker.virtualize_child_env_for_environment(&mut filtered_child, Some("filtered-child"));
    assert_eq!(filtered_child, expected);
    for (alias, real) in [("AUTH_HEADER", token), ("SECOND_AUTH_HEADER", second_token)] {
        let mut headers = HeaderMap::from_iter([(
            AUTHORIZATION,
            HeaderValue::from_str(&filtered_child[alias]).unwrap(),
        )]);
        broker.inject_request_headers_for_environment(
            "https://first.example/",
            &mut headers,
            Some("filtered-child"),
        );
        assert_eq!(headers[AUTHORIZATION], format!("Bearer {real}"));
    }

    let mut child_env = env.clone();
    child_env.insert("STRIPE_HOST".to_string(), "child.example".to_string());
    broker.virtualize_child_env_for_environment(&mut child_env, Some("child"));
    child_env.remove("STRIPE_HOST");
    env.remove("STRIPE_HOST");
    for (environment, destination, current_env) in [
        ("environment", "first.example", &mut env),
        ("child", "child.example", &mut child_env),
    ] {
        let expected = current_env.clone();
        broker.virtualize_child_env_for_environment(current_env, Some(environment));
        assert_eq!(*current_env, expected);
        for (alias, real) in [("AUTH_HEADER", token), ("SECOND_AUTH_HEADER", second_token)] {
            let mut headers = HeaderMap::from_iter([(
                AUTHORIZATION,
                HeaderValue::from_str(&current_env[alias]).unwrap(),
            )]);
            broker.inject_request_headers_for_environment(
                &format!("https://{destination}/"),
                &mut headers,
                Some(environment),
            );
            assert_eq!(headers[AUTHORIZATION], format!("Bearer {real}"));
        }
    }

    env.insert("STRIPE_HOST".to_string(), String::new());
    broker.virtualize_child_env_for_environment(&mut env, Some("environment"));

    assert_eq!(env["STRIPE_API_KEY"], token);
    assert_eq!(env["AUTH_HEADER"], format!("Bearer {token}"));
    assert_eq!(env["SECOND_AUTH_HEADER"], format!("Bearer {second_token}"));
    let mut headers = HeaderMap::from_iter([(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {dummy}")).expect("valid dummy authentication"),
    )]);
    broker.inject_request_headers_for_environment(
        "https://first.example/",
        &mut headers,
        Some("environment"),
    );
    assert_eq!(
        headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
        Some(format!("Bearer {dummy}").as_str())
    );
    assert!(
        !broker
            .host_protocols_for_environment("first.example", /*port*/ 443, Some("environment"))
            .tls
    );
}

#[test]
fn explicit_invalid_destinations_clear_previous_dynamic_bindings() {
    for (key, host_key, token, static_host) in [
        (
            "GH_ENTERPRISE_TOKEN",
            "GH_HOST",
            "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
            None,
        ),
        (
            "GITHUB_ENTERPRISE_TOKEN",
            "GH_HOST",
            "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
            None,
        ),
        (
            "OPENAI_API_KEY",
            "OPENAI_BASE_URL",
            "sk-proj-abcdefghijklmnopqrstuvwxyz0123456789",
            Some("api.openai.com"),
        ),
        (
            "PROVIDER_TOKEN",
            "PROVIDER_ENDPOINT",
            "provider_abcdefghijklmnopqrstuvwx",
            Some("static.example"),
        ),
        (
            "PROVIDER_TOKEN",
            "PROVIDER_ENDPOINT",
            "provider_abcdefghijklmnopqrstuvwx",
            None,
        ),
    ] {
        for use_real in [false, true] {
            for use_alias in [false, true] {
                for invalid in [
                    "",
                    " ",
                    "not a valid destination",
                    "http://untrusted.example",
                ]
                .into_iter()
                .chain((key == "PROVIDER_TOKEN").then_some("https://*.example"))
                .chain((host_key == "GH_HOST").then_some("github.com"))
                .chain(
                    (host_key == "GH_HOST")
                        .then_some([
                            "second.example:bad-port",
                            "127.0.0.1:bad-port",
                            "[::1]bad",
                            "[::1]:bad-port",
                            "second.example:99999",
                            "[example.com]",
                        ])
                        .into_iter()
                        .flatten(),
                ) {
                    let broker = broker_for(CredentialProviderConfig {
                        env: vec!["PROVIDER_TOKEN".to_string()],
                        patterns: vec!["^provider_[a-z]{24}$".to_string()],
                        url_prefixes: static_host
                            .map(|host| format!("https://{host}"))
                            .into_iter()
                            .collect(),
                        url_prefix_from_env: Some("PROVIDER_ENDPOINT".to_string()),
                        ..CredentialProviderConfig::default()
                    });
                    let mut env = HashMap::from([
                        (key.to_string(), token.to_string()),
                        (
                            host_key.to_string(),
                            if host_key == "GH_HOST" {
                                "first.example".to_string()
                            } else {
                                "https://first.example/v1".to_string()
                            },
                        ),
                    ]);
                    broker.virtualize_child_env(&mut env);
                    let dummy = env[key].clone();
                    let second_host = static_host.unwrap_or("second.example");
                    env.insert(
                        host_key.to_string(),
                        if host_key == "GH_HOST" {
                            second_host.to_string()
                        } else {
                            format!("https://{second_host}/v1")
                        },
                    );
                    broker.virtualize_child_env(&mut env);
                    assert!(broker.host_requires_mitm("first.example", /*port*/ 443));
                    env.clear();
                    let value = if use_real { token } else { &dummy };
                    let (input_key, input_value) = if use_alias {
                        ("AUTH_HEADER", format!("Bearer {value}"))
                    } else {
                        (key, value.to_string())
                    };
                    env.insert(input_key.to_string(), input_value);
                    env.insert(host_key.to_string(), invalid.to_string());
                    broker.virtualize_child_env(&mut env);

                    for host in ["first.example", second_host] {
                        let expected = if static_host == Some(host) {
                            token
                        } else {
                            &dummy
                        };
                        let mut headers = HeaderMap::from_iter([(
                            AUTHORIZATION,
                            HeaderValue::from_str(&format!("Bearer {dummy}")).unwrap(),
                        )]);
                        broker.inject_request_headers(&format!("https://{host}/v1"), &mut headers);
                        assert_eq!(
                            headers[AUTHORIZATION],
                            format!("Bearer {expected}"),
                            "{key}, {invalid:?}, {host}, real={use_real}, alias={use_alias}"
                        );
                    }
                    assert!(!broker.host_requires_mitm("first.example", /*port*/ 443));
                    if host_key == "GH_HOST" {
                        for _ in 0..2 {
                            broker.virtualize_child_env(&mut env);
                            assert_eq!(
                                env[input_key],
                                if use_alias {
                                    format!("Bearer {token}")
                                } else {
                                    token.to_string()
                                },
                                "{key}, {invalid:?}, real={use_real}, alias={use_alias}"
                            );
                            for cloud in ["api.github.com", "github.com", "tenant.ghe.com"] {
                                assert!(!broker.host_requires_mitm(cloud, /*port*/ 443));
                            }
                        }

                        env.insert(host_key.to_string(), "restored.example".to_string());
                        broker.virtualize_child_env(&mut env);
                        let restored_dummy = if use_alias {
                            env[input_key].strip_prefix("Bearer ").unwrap()
                        } else {
                            &env[input_key]
                        };
                        assert_ne!(restored_dummy, token);
                        let mut headers = HeaderMap::from_iter([(
                            AUTHORIZATION,
                            HeaderValue::from_str(&format!("Bearer {restored_dummy}")).unwrap(),
                        )]);
                        broker.inject_request_headers("https://restored.example/", &mut headers);
                        assert_eq!(headers[AUTHORIZATION], format!("Bearer {token}"));
                        assert!(!broker.host_requires_mitm("first.example", /*port*/ 443));
                        assert!(!broker.host_requires_mitm("second.example", /*port*/ 443));
                        assert!(!broker.host_requires_mitm("api.github.com", /*port*/ 443));
                        env.clear();
                        env.insert("GH_TOKEN".to_string(), token.to_string());
                        broker.virtualize_child_env(&mut env);
                        let mut headers = HeaderMap::from_iter([(
                            AUTHORIZATION,
                            HeaderValue::from_str(&format!("Bearer {}", env["GH_TOKEN"])).unwrap(),
                        )]);
                        broker.inject_request_headers("https://api.github.com/", &mut headers);
                        assert_eq!(headers[AUTHORIZATION], format!("Bearer {token}"));
                    }
                }
            }
        }
    }
}

#[test]
fn configured_provider_limits_injection_to_url_prefixes() {
    let token = "provider_abcdefghijklmnopqrstuvwx";
    let broker = broker_for(CredentialProviderConfig {
        env: vec!["PROVIDER_TOKEN".to_string()],
        patterns: vec!["provider_[a-z]{24}".to_string()],
        url_prefixes: vec![
            "https://root.provider.example".to_string(),
            "https://api.provider.example/v1".to_string(),
            "enterprise.example/v2/".to_string(),
            "https://*.provider.example:8443/private".to_string(),
            "http://localhost:443/v1".to_string(),
            "127.0.0.1:443/v1".to_string(),
            "http://[::1]:443/v1".to_string(),
            "http://localhost:8080/v1".to_string(),
            "localhost/v2".to_string(),
            "https://localhost:443/v3".to_string(),
            "https://localhost:80/v4".to_string(),
        ],
        ..CredentialProviderConfig::default()
    });
    let mut env = HashMap::from([("PROVIDER_TOKEN".to_string(), token.to_string())]);
    broker.virtualize_child_env(&mut env);
    let dummy = &env["PROVIDER_TOKEN"];
    assert!(broker.host_requires_mitm("team.provider.example", /*port*/ 8443));
    assert!(!broker.host_requires_mitm("team.provider.example", /*port*/ 443));

    for (destination, injected) in [
        ("root.provider.example", false),
        ("https://root.provider.example/models", true),
        ("https://api.provider.example/v1", true),
        ("https://api.provider.example/v1/models?limit=1", true),
        ("https://api.provider.example/v10/models", false),
        ("https://api.provider.example/private", false),
        (
            "https://api.provider.example/public/%2e%2e/v1/models",
            false,
        ),
        ("https://api.provider.example/v1%2f../private", false),
        ("http://api.provider.example/v1", false),
        ("https://enterprise.example/v2/models", true),
        ("https://enterprise.example/v2", false),
        ("https://team.provider.example:8443/private/models", true),
        ("https://team.provider.example/private/models", false),
        ("https://provider.example:8443/private/models", false),
        ("http://localhost:443/v1/models", true),
        ("http://localhost/v1/models", false),
        ("https://localhost/v1/models", false),
        ("http://127.0.0.1:443/v1/models", true),
        ("http://127.0.0.1/v1/models", false),
        ("http://[::1]:443/v1/models", true),
        ("http://[::1]/v1/models", false),
        ("http://localhost:8080/v1/models", true),
        ("http://localhost/v2/models", true),
        ("http://localhost:443/v2/models", false),
        ("https://localhost/v3/models", true),
        ("http://localhost:443/v3/models", false),
        ("https://localhost:80/v4/models", true),
        ("https://localhost/v4/models", false),
    ] {
        let dummy_header = format!("Bearer {dummy}");
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&dummy_header).expect("valid dummy authentication"),
        );

        broker.inject_request_headers(destination, &mut headers);

        let expected = if injected {
            format!("Bearer {token}")
        } else {
            dummy_header
        };
        assert_eq!(
            headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some(expected.as_str()),
            "destination: {destination}"
        );
    }
}

#[test]
fn configured_provider_accepts_hostname_or_https_url_from_one_environment_key() {
    let token = "provider_abcdefghijklmnopqrstuvwx";
    for (host_value, expected_host) in [
        ("enterprise.example", Some("enterprise.example")),
        ("https://gateway.example/v1", Some("gateway.example")),
        ("http://plaintext.example/v1", None),
    ] {
        let broker = broker_for(CredentialProviderConfig {
            env: vec!["PROVIDER_TOKEN".to_string()],
            patterns: vec!["provider_[a-z]{24}".to_string()],
            url_prefix_from_env: Some("PROVIDER_HOST".to_string()),
            ..CredentialProviderConfig::default()
        });
        let mut env = HashMap::from([
            ("PROVIDER_TOKEN".to_string(), token.to_string()),
            ("PROVIDER_HOST".to_string(), host_value.to_string()),
        ]);

        broker.virtualize_child_env(&mut env);

        match expected_host {
            Some(host) => {
                assert_ne!(env["PROVIDER_TOKEN"], token);
                assert!(broker.host_requires_mitm(host, /*port*/ 443));
                assert_eq!(
                    broker.environment(&env).binding_keys,
                    vec!["PROVIDER_HOST".to_string()]
                );
                assert_eq!(
                    broker.environment(&env).configured_provider_context_keys,
                    vec!["PROVIDER_HOST".to_string()]
                );
            }
            None => {
                assert_eq!(env["PROVIDER_TOKEN"], token);
                let mut snapshot = token.to_string();
                assert!(broker.virtualize_text(&mut snapshot, &env));
                assert_eq!(snapshot, token);
            }
        }
    }
}

#[test]
fn configured_provider_rejects_unsafe_destinations_and_unusable_dummy_patterns() {
    let impossible_assertion = CredentialProviderConfig {
        env: vec!["PROVIDER_PASSWORD".to_string()],
        patterns: vec![r"^pass_\b[a-z]{24}$".to_string()],
        url_prefixes: vec!["api.example".to_string()],
        ..CredentialProviderConfig::default()
    };
    assert!(
        super::configured::ConfiguredCredentialProvider::compile("custom", &impossible_assertion)
            .is_err()
    );

    for (pattern, url_prefixes, first, second) in [
        (
            r"\b|token_[a-z]{24}",
            vec!["api.example"],
            "token_abcdefghijklmnopqrstuvwx",
            None,
        ),
        (
            "provider_[a-z]{24}",
            vec!["*"],
            "provider_abcdefghijklmnopqrstuvwx",
            None,
        ),
        (
            "token_[01]",
            vec!["api.example"],
            "token_0",
            Some("token_1"),
        ),
        (
            "only_one_token",
            vec!["api.example"],
            "only_one_token",
            None,
        ),
    ] {
        let broker = broker_for(CredentialProviderConfig {
            env: vec!["FIRST_TOKEN".to_string(), "SECOND_TOKEN".to_string()],
            patterns: vec![pattern.to_string()],
            url_prefixes: url_prefixes.into_iter().map(str::to_string).collect(),
            ..CredentialProviderConfig::default()
        });
        let mut env = HashMap::from([("FIRST_TOKEN".to_string(), first.to_string())]);
        if let Some(second) = second {
            env.insert("SECOND_TOKEN".to_string(), second.to_string());
        }

        broker.virtualize_child_env(&mut env);

        assert_eq!(env["FIRST_TOKEN"], first);
        if let Some(second) = second {
            assert_eq!(env["SECOND_TOKEN"], second);
        }
        assert!(broker.environment(&env).credential_keys.is_empty());
        let mut snapshot = first.to_string();
        assert!(broker.virtualize_text(&mut snapshot, &env));
        assert_eq!(snapshot, first);
    }
}
