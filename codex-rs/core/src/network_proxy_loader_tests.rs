use super::*;

use codex_execpolicy::Decision;
use codex_execpolicy::NetworkRuleProtocol;
use codex_execpolicy::Policy;
use pretty_assertions::assert_eq;

#[test]
fn higher_precedence_profile_network_overlays_domain_entries() {
    let lower_network: toml::Value = toml::from_str(
        r#"
default_permissions = "workspace"

[permissions.workspace.network]

[permissions.workspace.network.domains]
"lower.example.com" = "allow"
"blocked.example.com" = "deny"
"#,
    )
    .expect("lower layer should parse");
    let higher_network: toml::Value = toml::from_str(
        r#"
default_permissions = "workspace"

[permissions.workspace.network]

[permissions.workspace.network.domains]
"higher.example.com" = "allow"
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

    assert_eq!(
        config.network.allowed_domains(),
        Some(vec![
            "lower.example.com".to_string(),
            "higher.example.com".to_string()
        ])
    );
    assert_eq!(
        config.network.denied_domains(),
        Some(vec!["blocked.example.com".to_string()])
    );
}

#[test]
fn higher_precedence_profile_network_overrides_matching_domain_entries() {
    let lower_network: toml::Value = toml::from_str(
        r#"
default_permissions = "workspace"

[permissions.workspace.network]

[permissions.workspace.network.domains]
"shared.example.com" = "deny"
"other.example.com" = "allow"
"#,
    )
    .expect("lower layer should parse");
    let higher_network: toml::Value = toml::from_str(
        r#"
default_permissions = "workspace"

[permissions.workspace.network]

[permissions.workspace.network.domains]
"shared.example.com" = "allow"
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

    assert_eq!(
        config.network.allowed_domains(),
        Some(vec![
            "other.example.com".to_string(),
            "shared.example.com".to_string()
        ])
    );
    assert_eq!(config.network.denied_domains(), None);
}

#[test]
fn higher_precedence_profile_network_overrides_mitm_hooks() {
    let lower_network: toml::Value = toml::from_str(
        r#"
default_permissions = "workspace"

[permissions.workspace.network]
mode = "limited"

[permissions.workspace.network.domains]
"lower.example.com" = "allow"

[permissions.workspace.network.mitm]
enabled = false

[permissions.workspace.network.mitm.hooks.lower.match]
host = "lower.example.com"
methods = ["POST"]
path_prefixes = ["/repos/openai/"]
"#,
    )
    .expect("lower layer should parse");
    let higher_network: toml::Value = toml::from_str(
        r#"
default_permissions = "workspace"

[permissions.workspace.network]
mode = "full"

[permissions.workspace.network.domains]
"higher.example.com" = "allow"

[permissions.workspace.network.mitm]
enabled = true

[permissions.workspace.network.mitm.hooks.github_write.match]
host = "api.github.com"
methods = ["PUT"]
path_prefixes = ["/repos/openai/"]
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

    assert_eq!(config.network.mode, codex_network_proxy::NetworkMode::Full);
    assert!(config.network.mitm);
    assert_eq!(
        config.network.allowed_domains(),
        Some(vec![
            "lower.example.com".to_string(),
            "higher.example.com".to_string()
        ])
    );
    assert_eq!(config.network.mitm_hooks.len(), 1);
    assert_eq!(config.network.mitm_hooks[0].host, "api.github.com");
    assert_eq!(config.network.mitm_hooks[0].matcher.methods, vec!["PUT"]);
}

#[test]
fn execpolicy_network_rules_overlay_network_lists() {
    let mut config = NetworkProxyConfig::default();
    config
        .network
        .set_allowed_domains(vec!["config.example.com".to_string()]);
    config
        .network
        .set_denied_domains(vec!["blocked.example.com".to_string()]);

    let mut exec_policy = Policy::empty();
    exec_policy
        .add_network_rule(
            "blocked.example.com",
            NetworkRuleProtocol::Https,
            Decision::Allow,
            /*justification*/ None,
        )
        .expect("allow rule should be valid");
    exec_policy
        .add_network_rule(
            "api.example.com",
            NetworkRuleProtocol::Http,
            Decision::Forbidden,
            /*justification*/ None,
        )
        .expect("deny rule should be valid");

    apply_exec_policy_network_rules(&mut config, &exec_policy);

    assert_eq!(
        config.network.allowed_domains(),
        Some(vec![
            "config.example.com".to_string(),
            "blocked.example.com".to_string()
        ])
    );
    assert_eq!(
        config.network.denied_domains(),
        Some(vec!["api.example.com".to_string()])
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

#[test]
fn apply_network_constraints_skips_empty_domain_sides() {
    let config: toml::Value = toml::from_str(
        r#"
default_permissions = "workspace"

[permissions.workspace.network]

[permissions.workspace.network.domains]
"managed.example.com" = "allow"
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

    assert_eq!(
        constraints.allowed_domains,
        Some(vec!["managed.example.com".to_string()])
    );
    assert_eq!(constraints.denied_domains, None);
}

#[test]
fn apply_network_constraints_overlay_domain_entries() {
    let lower_network: toml::Value = toml::from_str(
        r#"
default_permissions = "workspace"

[permissions.workspace.network]

[permissions.workspace.network.domains]
"blocked.example.com" = "deny"
"#,
    )
    .expect("lower layer should parse");
    let higher_network: toml::Value = toml::from_str(
        r#"
default_permissions = "workspace"

[permissions.workspace.network]

[permissions.workspace.network.domains]
"api.example.com" = "allow"
"#,
    )
    .expect("higher layer should parse");

    let lower_network = selected_network_from_tables(
        network_tables_from_toml(&lower_network).expect("lower layer should deserialize"),
    )
    .expect("lower layer should select a network table")
    .expect("lower network table should be present");
    let higher_network = selected_network_from_tables(
        network_tables_from_toml(&higher_network).expect("higher layer should deserialize"),
    )
    .expect("higher layer should select a network table")
    .expect("higher network table should be present");

    let mut constraints = NetworkProxyConstraints::default();
    apply_network_constraints(lower_network, &mut constraints);
    apply_network_constraints(higher_network, &mut constraints);

    assert_eq!(
        constraints.allowed_domains,
        Some(vec!["api.example.com".to_string()])
    );
    assert_eq!(
        constraints.denied_domains,
        Some(vec!["blocked.example.com".to_string()])
    );
}
