use super::*;

use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_config::RequirementSource;
use codex_config::RequirementsExecPolicy;
use codex_config::Sourced;
use codex_config::permissions_toml::NetworkDomainPermissionToml;
use codex_config::permissions_toml::NetworkDomainPermissionsToml;
use codex_execpolicy::Decision;
use codex_execpolicy::NetworkRuleProtocol;
use codex_execpolicy::Policy;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::fs;
use tempfile::tempdir;

#[test]
fn higher_precedence_profile_network_overlays_domain_entries() {
    let lower_network: toml::Value = toml::from_str(
        r#"
default_permissions = "dev"

[permissions.dev.network]

[permissions.dev.network.domains]
"lower.example.com" = "allow"
"blocked.example.com" = "deny"
"#,
    )
    .expect("lower layer should parse");
    let higher_network: toml::Value = toml::from_str(
        r#"
default_permissions = "dev"

[permissions.dev.network]

[permissions.dev.network.domains]
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
default_permissions = "dev"

[permissions.dev.network]

[permissions.dev.network.domains]
"shared.example.com" = "deny"
"other.example.com" = "allow"
"#,
    )
    .expect("lower layer should parse");
    let higher_network: toml::Value = toml::from_str(
        r#"
default_permissions = "dev"

[permissions.dev.network]

[permissions.dev.network.domains]
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
fn higher_precedence_profile_network_overrides_named_mitm_actions() {
    let lower_network: toml::Value = toml::from_str(
        r#"
default_permissions = "workspace"

[permissions.workspace.network]
mode = "full"

[permissions.workspace.network.domains]
"lower.example.com" = "allow"

[permissions.workspace.network.mitm.hooks.github_write]
host = "api.github.com"
methods = ["POST"]
path_prefixes = ["/repos/openai/"]
action = ["strip_auth"]

[permissions.workspace.network.mitm.actions.strip_auth]
strip_request_headers = ["authorization"]
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

[permissions.workspace.network.mitm.actions.strip_auth]
strip_request_headers = ["x-api-key"]
"#,
    )
    .expect("higher layer should parse");

    let mut accumulator = NetworkConfigAccumulator::default();
    accumulator
        .apply_network_tables(
            network_tables_from_toml(&lower_network).expect("lower layer should deserialize"),
        )
        .expect("lower layer should apply");
    accumulator
        .apply_network_tables(
            network_tables_from_toml(&higher_network).expect("higher layer should deserialize"),
        )
        .expect("higher layer should apply");
    let config = accumulator.finish().expect("merged config should build");

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
    assert_eq!(config.network.mitm_hooks[0].matcher.methods, vec!["POST"]);
    assert_eq!(
        config.network.mitm_hooks[0].actions.strip_request_headers,
        vec!["x-api-key"]
    );
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

#[tokio::test]
async fn malformed_custom_rules_preserve_managed_denied_domain() {
    let temp_dir = tempdir().expect("create temp dir");
    let policy_dir = temp_dir.path().join("rules");
    fs::create_dir_all(&policy_dir).expect("create policy dir");
    fs::write(policy_dir.join("broken.rules"), "prefix_rule(").expect("write malformed policy");

    let mut requirements_exec_policy = Policy::empty();
    requirements_exec_policy
        .add_network_rule(
            "blocked.example.com",
            NetworkRuleProtocol::Https,
            Decision::Forbidden,
            /*justification*/ None,
        )
        .expect("managed network rule should be valid");
    let requirements = ConfigRequirements {
        exec_policy: Some(Sourced::new(
            RequirementsExecPolicy::new(requirements_exec_policy),
            RequirementSource::Unknown,
        )),
        ..ConfigRequirements::default()
    };
    let dot_codex_folder = AbsolutePathBuf::from_absolute_path(temp_dir.path())
        .expect("dot codex folder should be absolute");
    let layers = ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::Project { dot_codex_folder },
            toml::Value::Table(Default::default()),
        )],
        requirements,
        ConfigRequirementsToml::default(),
    )
    .expect("layer stack should be valid");

    let state = build_config_state_from_layers(&layers)
        .await
        .expect("proxy state should tolerate malformed custom rules");

    assert_eq!(
        state.config.network.denied_domains(),
        Some(vec!["blocked.example.com".to_string()])
    );
}

#[test]
fn apply_network_constraints_includes_allow_all_unix_sockets_flag() {
    let config: toml::Value = toml::from_str(
        r#"
default_permissions = "dev"

[permissions.dev.network]
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
fn selected_network_from_tables_ignores_builtin_profile_without_permissions_table() {
    let config: toml::Value = toml::from_str(
        r#"
default_permissions = ":workspace"
"#,
    )
    .expect("built-in profile config should parse");

    let network = selected_network_from_tables(
        network_tables_from_toml(&config).expect("built-in profile config should deserialize"),
    )
    .expect("built-in profile selection should not require permissions tables");

    assert_eq!(network, None);
}

#[test]
fn selected_network_from_tables_rejects_unknown_builtin_profile_without_permissions_table() {
    let config: toml::Value = toml::from_str(
        r#"
default_permissions = ":unknown"
"#,
    )
    .expect("unknown built-in config should parse");

    let err = selected_network_from_tables(
        network_tables_from_toml(&config).expect("unknown built-in config should deserialize"),
    )
    .expect_err("unknown built-in profile should be rejected");

    assert_eq!(
        err.to_string(),
        "default_permissions refers to unknown built-in profile `:unknown`"
    );
}

#[test]
fn selected_network_from_tables_resolves_builtin_workspace_parent() {
    let config: toml::Value = toml::from_str(
        r#"
default_permissions = "dev"

[permissions.dev]
extends = ":workspace"

[permissions.dev.network]
enabled = true

[permissions.dev.network.domains]
"child.example.com" = "allow"
"#,
    )
    .expect("dev extension config should parse");

    let network = selected_network_from_tables(
        network_tables_from_toml(&config).expect("dev extension config should deserialize"),
    )
    .expect("dev extension should resolve")
    .expect("dev extension should expose child network config");

    assert_eq!(
        network,
        NetworkToml {
            enabled: Some(true),
            domains: Some(NetworkDomainPermissionsToml {
                entries: BTreeMap::from([(
                    "child.example.com".to_string(),
                    NetworkDomainPermissionToml::Allow,
                )]),
            }),
            ..Default::default()
        }
    );
}

#[test]
fn selected_network_from_tables_resolves_permission_profile_inheritance() {
    let config: toml::Value = toml::from_str(
        r#"
default_permissions = "dev"

[permissions.base.network]
enabled = true
dangerously_allow_all_unix_sockets = true

[permissions.base.network.domains]
"base.example.com" = "allow"
"shared.example.com" = "deny"

[permissions.dev]
extends = "base"

[permissions.dev.network]
allow_local_binding = true

[permissions.dev.network.domains]
"child.example.com" = "allow"
"shared.example.com" = "allow"
"#,
    )
    .expect("permissions profiles should parse");

    let network = selected_network_from_tables(
        network_tables_from_toml(&config).expect("permissions profiles should deserialize"),
    )
    .expect("permissions profiles should select a network table")
    .expect("network table should be present");

    assert_eq!(
        network,
        NetworkToml {
            enabled: Some(true),
            dangerously_allow_all_unix_sockets: Some(true),
            allow_local_binding: Some(true),
            domains: Some(NetworkDomainPermissionsToml {
                entries: BTreeMap::from([
                    (
                        "base.example.com".to_string(),
                        NetworkDomainPermissionToml::Allow,
                    ),
                    (
                        "child.example.com".to_string(),
                        NetworkDomainPermissionToml::Allow,
                    ),
                    (
                        "shared.example.com".to_string(),
                        NetworkDomainPermissionToml::Allow,
                    ),
                ]),
            }),
            ..Default::default()
        }
    );
}

#[test]
fn config_from_layers_resolves_inherited_profiles_across_layers() {
    let lower_layer = ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        toml::toml! {
            [permissions.base.network.domains]
            "base.example.com" = "allow"
        }
        .into(),
    );
    let higher_layer = ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        toml::toml! {
            default_permissions = "dev"

            [permissions.dev]
            extends = "base"

            [permissions.dev.network.domains]
            "child.example.com" = "allow"
        }
        .into(),
    );
    let layers = ConfigLayerStack::new(
        vec![lower_layer, higher_layer],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("layer stack should be valid");

    let config =
        config_from_layers(&layers, &Policy::empty()).expect("inherited profiles should load");

    assert_eq!(
        config.network.allowed_domains(),
        Some(vec![
            "base.example.com".to_string(),
            "child.example.com".to_string(),
        ])
    );
}

#[test]
fn config_from_layers_normalizes_profile_network_domains_before_merging_layers() {
    let lower_layer = ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        toml::toml! {
            default_permissions = "dev"

            [permissions.dev.network.domains]
            "example.com" = "deny"
        }
        .into(),
    );
    let higher_layer = ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        toml::toml! {
            [permissions.dev.network.domains]
            "EXAMPLE.COM" = "allow"
        }
        .into(),
    );
    let layers = ConfigLayerStack::new(
        vec![lower_layer, higher_layer],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("layer stack should be valid");

    let config = config_from_layers(&layers, &Policy::empty())
        .expect("network domain layer precedence should load");

    assert_eq!(
        config.network.allowed_domains(),
        Some(vec!["example.com".to_string()])
    );
    assert_eq!(config.network.denied_domains(), None);
}

#[test]
fn config_from_layers_uses_only_the_final_selected_profile_network() {
    let lower_layer = ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        toml::toml! {
            default_permissions = "dev"

            [permissions.dev.network.domains]
            "lower.example.com" = "allow"
        }
        .into(),
    );
    let higher_layer = ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        toml::toml! {
            default_permissions = ":workspace"
        }
        .into(),
    );
    let layers = ConfigLayerStack::new(
        vec![lower_layer, higher_layer],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("layer stack should be valid");

    let config = config_from_layers(&layers, &Policy::empty())
        .expect("final built-in profile selection should load");

    assert_eq!(config.network.allowed_domains(), None);
    assert_eq!(config.network.denied_domains(), None);
}

#[test]
fn trusted_constraints_use_only_the_final_selected_profile_network() {
    let lower_layer = ConfigLayerEntry::new(
        ConfigLayerSource::System {
            file: AbsolutePathBuf::try_from(std::path::PathBuf::from("/tmp/system.toml"))
                .expect("system config path should be absolute"),
        },
        toml::toml! {
            default_permissions = "dev"

            [permissions.dev.network.domains]
            "managed.example.com" = "allow"
        }
        .into(),
    );
    let higher_layer = ConfigLayerEntry::new(
        ConfigLayerSource::LegacyManagedConfigTomlFromFile {
            file: AbsolutePathBuf::try_from(std::path::PathBuf::from("/tmp/managed.toml"))
                .expect("managed config path should be absolute"),
        },
        toml::toml! {
            default_permissions = ":workspace"
        }
        .into(),
    );
    let layers = ConfigLayerStack::new(
        vec![lower_layer, higher_layer],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("layer stack should be valid");

    let constraints = network_constraints_from_trusted_layers(&layers)
        .expect("final built-in trusted selection should load");

    assert_eq!(constraints.allowed_domains, None);
    assert_eq!(constraints.denied_domains, None);
}

#[test]
fn trusted_constraints_normalize_profile_network_domains_before_merging_layers() {
    let lower_layer = ConfigLayerEntry::new(
        ConfigLayerSource::System {
            file: AbsolutePathBuf::try_from(std::path::PathBuf::from("/tmp/system.toml"))
                .expect("system config path should be absolute"),
        },
        toml::toml! {
            default_permissions = "dev"

            [permissions.dev.network.domains]
            "example.com" = "deny"
        }
        .into(),
    );
    let higher_layer = ConfigLayerEntry::new(
        ConfigLayerSource::LegacyManagedConfigTomlFromFile {
            file: AbsolutePathBuf::try_from(std::path::PathBuf::from("/tmp/managed.toml"))
                .expect("managed config path should be absolute"),
        },
        toml::toml! {
            [permissions.dev.network.domains]
            "EXAMPLE.COM" = "allow"
        }
        .into(),
    );
    let layers = ConfigLayerStack::new(
        vec![lower_layer, higher_layer],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("layer stack should be valid");

    let constraints = network_constraints_from_trusted_layers(&layers)
        .expect("trusted network domain layer precedence should load");

    assert_eq!(
        constraints.allowed_domains,
        Some(vec!["example.com".to_string()])
    );
    assert_eq!(constraints.denied_domains, None);
}

#[test]
fn apply_network_constraints_skips_empty_domain_sides() {
    let config: toml::Value = toml::from_str(
        r#"
default_permissions = "dev"

[permissions.dev.network]

[permissions.dev.network.domains]
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
default_permissions = "dev"

[permissions.dev.network]

[permissions.dev.network.domains]
"blocked.example.com" = "deny"
"#,
    )
    .expect("lower layer should parse");
    let higher_network: toml::Value = toml::from_str(
        r#"
default_permissions = "dev"

[permissions.dev.network]

[permissions.dev.network.domains]
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
