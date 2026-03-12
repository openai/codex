use super::*;

use codex_execpolicy::Decision;
use codex_execpolicy::NetworkRuleProtocol;
use codex_execpolicy::Policy;
use pretty_assertions::assert_eq;

#[test]
fn higher_precedence_profile_network_beats_lower_profile_network() {
    let lower_network: toml::Value = toml::from_str(
        r#"
default_permissions = "workspace"

[permissions.workspace.network]
allowed_domains = ["lower.example.com"]
"#,
    )
    .expect("lower layer should parse");
    let higher_network: toml::Value = toml::from_str(
        r#"
default_permissions = "workspace"

[permissions.workspace.network]
allowed_domains = ["higher.example.com"]
"#,
    )
    .expect("higher layer should parse");

    let mut config = NetworkProxyConfig::default();
    apply_network_tables(
        &mut config,
        network_tables_from_toml(&lower_network).expect("lower layer should deserialize"),
    )
    .expect("lower layer should apply");
    apply_network_tables(
        &mut config,
        network_tables_from_toml(&higher_network).expect("higher layer should deserialize"),
    )
    .expect("higher layer should apply");

    assert_eq!(config.network.allowed_domains, vec!["higher.example.com"]);
}

#[test]
fn execpolicy_network_rules_overlay_network_lists() {
    let mut config = NetworkProxyConfig::default();
    config.network.allowed_domains = vec!["config.example.com".to_string()];
    config.network.denied_domains = vec!["blocked.example.com".to_string()];

    let mut exec_policy = Policy::empty();
    exec_policy
        .add_network_rule(
            "blocked.example.com",
            NetworkRuleProtocol::Https,
            Decision::Allow,
            None,
        )
        .expect("allow rule should be valid");
    exec_policy
        .add_network_rule(
            "api.example.com",
            NetworkRuleProtocol::Http,
            Decision::Forbidden,
            None,
        )
        .expect("deny rule should be valid");

    apply_exec_policy_network_rules(&mut config, &exec_policy);

    assert_eq!(
        config.network.allowed_domains,
        vec![
            "config.example.com".to_string(),
            "blocked.example.com".to_string()
        ]
    );
    assert_eq!(
        config.network.denied_domains,
        vec!["api.example.com".to_string()]
    );
}

#[test]
fn apply_network_constraints_includes_allow_all_unix_sockets_flag() {
    let config: toml::Value = toml::from_str(
        r#"
default_permissions = "workspace"

[permissions.workspace.network]
dangerously_allow_all_unix_sockets = true
"#,
    )
    .expect("permissions profile should parse");
    let network = selected_network_from_tables(
        network_tables_from_toml(&config).expect("permissions profile should deserialize"),
    )
    .expect("permissions profile should select a network table")
    .expect("network table should be present");

    let mut constraints = NetworkProxyConstraints::default();
    apply_network_constraints(network, &mut constraints);

    assert_eq!(constraints.dangerously_allow_all_unix_sockets, Some(true));
}
