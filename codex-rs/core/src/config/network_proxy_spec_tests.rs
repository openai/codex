use super::*;
use crate::config_loader::NetworkDomainPermissionToml;
use crate::config_loader::NetworkDomainPermissionsToml;
use codex_network_proxy::NetworkDomainPermission;
use pretty_assertions::assert_eq;

fn domain_permissions(
    entries: impl IntoIterator<Item = (&'static str, NetworkDomainPermissionToml)>,
) -> NetworkDomainPermissionsToml {
    NetworkDomainPermissionsToml {
        entries: entries
            .into_iter()
            .map(|(pattern, permission)| (pattern.to_string(), permission))
            .collect(),
    }
}

#[test]
fn build_state_with_audit_metadata_threads_metadata_to_state() {
    let spec = NetworkProxySpec {
        config: NetworkProxyConfig::default(),
        constraints: NetworkProxyConstraints::default(),
        hard_deny_allowlist_misses: false,
    };
    let metadata = NetworkProxyAuditMetadata {
        conversation_id: Some("conversation-1".to_string()),
        app_version: Some("1.2.3".to_string()),
        user_account_id: Some("acct-1".to_string()),
        ..NetworkProxyAuditMetadata::default()
    };

    let state = spec
        .build_state_with_audit_metadata(metadata.clone())
        .expect("state should build");
    assert_eq!(state.audit_metadata(), &metadata);
}

#[test]
fn requirements_allowed_domains_are_a_baseline_for_user_allowlist() {
    let mut config = NetworkProxyConfig::default();
    config
        .network
        .set_allowed_domains(vec!["api.example.com".to_string()]);
    let requirements = NetworkConstraints {
        domains: Some(domain_permissions([(
            "*.example.com",
            NetworkDomainPermissionToml::Allow,
        )])),
        ..Default::default()
    };

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &SandboxPolicy::new_read_only_policy(),
    )
    .expect("config should stay within the managed allowlist");

    assert_eq!(
        spec.config.network.allowed_domains(),
        Some(vec![
            "*.example.com".to_string(),
            "api.example.com".to_string()
        ])
    );
    assert_eq!(
        spec.constraints.allowed_domains,
        Some(vec!["*.example.com".to_string()])
    );
    assert_eq!(spec.constraints.allowlist_expansion_enabled, Some(true));
}

#[test]
fn requirements_allowed_domains_do_not_override_user_denies_for_same_pattern() {
    let mut config = NetworkProxyConfig::default();
    config
        .network
        .set_denied_domains(vec!["api.example.com".to_string()]);
    let requirements = NetworkConstraints {
        domains: Some(domain_permissions([(
            "api.example.com",
            NetworkDomainPermissionToml::Allow,
        )])),
        ..Default::default()
    };

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &SandboxPolicy::new_workspace_write_policy(),
    )
    .expect("managed allowlist should not erase a user deny");

    assert_eq!(spec.config.network.allowed_domains(), None);
    assert_eq!(
        spec.config.network.denied_domains(),
        Some(vec!["api.example.com".to_string()])
    );
    assert_eq!(
        spec.constraints.allowed_domains,
        Some(vec!["api.example.com".to_string()])
    );
}

#[test]
fn requirements_allowlist_expansion_keeps_user_entries_mutable() {
    let mut config = NetworkProxyConfig::default();
    config
        .network
        .set_allowed_domains(vec!["api.example.com".to_string()]);
    let requirements = NetworkConstraints {
        domains: Some(domain_permissions([(
            "*.example.com",
            NetworkDomainPermissionToml::Allow,
        )])),
        ..Default::default()
    };

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &SandboxPolicy::new_workspace_write_policy(),
    )
    .expect("managed baseline should still allow user edits");

    let mut candidate = spec.config.clone();
    candidate.network.upsert_domain_permission(
        "api.example.com".to_string(),
        NetworkDomainPermission::Deny,
        normalize_host,
    );

    assert_eq!(
        candidate.network.allowed_domains(),
        Some(vec!["*.example.com".to_string()])
    );
    assert_eq!(
        candidate.network.denied_domains(),
        Some(vec!["api.example.com".to_string()])
    );
    validate_policy_against_constraints(&candidate, &spec.constraints)
        .expect("user allowlist entries should not become managed constraints");
}

#[test]
fn danger_full_access_keeps_managed_allowlist_and_denylist_fixed() {
    let mut config = NetworkProxyConfig::default();
    config
        .network
        .set_allowed_domains(vec!["evil.com".to_string()]);
    config
        .network
        .set_denied_domains(vec!["more-blocked.example.com".to_string()]);
    let requirements = NetworkConstraints {
        domains: Some(domain_permissions([
            ("*.example.com", NetworkDomainPermissionToml::Allow),
            ("blocked.example.com", NetworkDomainPermissionToml::Deny),
        ])),
        ..Default::default()
    };

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &SandboxPolicy::DangerFullAccess,
    )
    .expect("yolo mode should pin the effective policy to the managed baseline");

    assert_eq!(
        spec.config.network.allowed_domains(),
        Some(vec!["*.example.com".to_string()])
    );
    assert_eq!(
        spec.config.network.denied_domains(),
        Some(vec!["blocked.example.com".to_string()])
    );
    assert_eq!(spec.constraints.allowlist_expansion_enabled, Some(false));
    assert_eq!(spec.constraints.denylist_expansion_enabled, Some(false));
}

#[test]
fn explicit_empty_legacy_allowed_domains_pin_full_access_allowlist_empty() {
    let mut config = NetworkProxyConfig::default();
    config
        .network
        .set_allowed_domains(vec!["api.example.com".to_string()]);
    let requirements: NetworkConstraints =
        toml::from_str("allowed_domains = []").expect("legacy allowlist should parse");

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &SandboxPolicy::DangerFullAccess,
    )
    .expect("explicit empty legacy allowlist should remain constraining");

    assert_eq!(spec.config.network.allowed_domains(), None);
    assert_eq!(spec.constraints.allowed_domains, Some(Vec::new()));
    assert_eq!(spec.constraints.allowlist_expansion_enabled, Some(false));
}

#[test]
fn explicit_empty_legacy_denied_domains_pin_full_access_denylist_empty() {
    let mut config = NetworkProxyConfig::default();
    config
        .network
        .set_denied_domains(vec!["blocked.example.com".to_string()]);
    let requirements: NetworkConstraints =
        toml::from_str("denied_domains = []").expect("legacy denylist should parse");

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &SandboxPolicy::DangerFullAccess,
    )
    .expect("explicit empty legacy denylist should remain constraining");

    assert_eq!(spec.config.network.denied_domains(), None);
    assert_eq!(spec.constraints.denied_domains, Some(Vec::new()));
    assert_eq!(spec.constraints.denylist_expansion_enabled, Some(false));
}

#[test]
fn explicit_empty_legacy_allow_unix_sockets_disables_allow_all_unix_sockets() {
    let mut config = NetworkProxyConfig::default();
    config.network.dangerously_allow_all_unix_sockets = true;
    let requirements: NetworkConstraints =
        toml::from_str("allow_unix_sockets = []").expect("legacy unix socket list should parse");

    let err = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &SandboxPolicy::DangerFullAccess,
    )
    .expect_err("explicit empty legacy unix socket allowlist should remain constraining");

    assert!(
        err.to_string()
            .contains("network proxy constraints are invalid: invalid value for network.dangerously_allow_all_unix_sockets"),
        "unexpected error: {err:#}"
    );
}

#[test]
fn managed_allowed_domains_only_disables_default_mode_allowlist_expansion() {
    let mut config = NetworkProxyConfig::default();
    config
        .network
        .set_allowed_domains(vec!["api.example.com".to_string()]);
    let requirements = NetworkConstraints {
        domains: Some(domain_permissions([(
            "*.example.com",
            NetworkDomainPermissionToml::Allow,
        )])),
        managed_allowed_domains_only: Some(true),
        ..Default::default()
    };

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &SandboxPolicy::new_workspace_write_policy(),
    )
    .expect("managed baseline should still load");

    assert_eq!(
        spec.config.network.allowed_domains(),
        Some(vec!["*.example.com".to_string()])
    );
    assert_eq!(spec.constraints.allowlist_expansion_enabled, Some(false));
}

#[test]
fn managed_allowed_domains_only_ignores_user_allowlist_and_hard_denies_misses() {
    let mut config = NetworkProxyConfig::default();
    config
        .network
        .set_allowed_domains(vec!["api.example.com".to_string()]);
    let requirements = NetworkConstraints {
        domains: Some(domain_permissions([(
            "managed.example.com",
            NetworkDomainPermissionToml::Allow,
        )])),
        managed_allowed_domains_only: Some(true),
        ..Default::default()
    };

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &SandboxPolicy::new_workspace_write_policy(),
    )
    .expect("managed-only allowlist should still load");

    assert_eq!(
        spec.config.network.allowed_domains(),
        Some(vec!["managed.example.com".to_string()])
    );
    assert_eq!(
        spec.constraints.allowed_domains,
        Some(vec!["managed.example.com".to_string()])
    );
    assert_eq!(spec.constraints.allowlist_expansion_enabled, Some(false));
    assert!(spec.hard_deny_allowlist_misses);
}

#[test]
fn managed_allowed_domains_only_without_managed_allowlist_blocks_all_user_domains() {
    let mut config = NetworkProxyConfig::default();
    config
        .network
        .set_allowed_domains(vec!["api.example.com".to_string()]);
    let requirements = NetworkConstraints {
        managed_allowed_domains_only: Some(true),
        ..Default::default()
    };

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &SandboxPolicy::new_workspace_write_policy(),
    )
    .expect("managed-only mode should treat missing managed allowlist as empty");

    assert_eq!(spec.config.network.allowed_domains(), None);
    assert_eq!(spec.constraints.allowed_domains, Some(Vec::new()));
    assert_eq!(spec.constraints.allowlist_expansion_enabled, Some(false));
    assert!(spec.hard_deny_allowlist_misses);
}

#[test]
fn managed_allowed_domains_only_blocks_all_user_domains_in_full_access_without_managed_list() {
    let mut config = NetworkProxyConfig::default();
    config
        .network
        .set_allowed_domains(vec!["api.example.com".to_string()]);
    let requirements = NetworkConstraints {
        managed_allowed_domains_only: Some(true),
        ..Default::default()
    };

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &SandboxPolicy::DangerFullAccess,
    )
    .expect("managed-only mode should treat missing managed allowlist as empty");

    assert_eq!(spec.config.network.allowed_domains(), None);
    assert_eq!(spec.constraints.allowed_domains, Some(Vec::new()));
    assert_eq!(spec.constraints.allowlist_expansion_enabled, Some(false));
    assert!(spec.hard_deny_allowlist_misses);
}

#[test]
fn deny_only_requirements_do_not_create_allow_constraints_in_full_access() {
    let mut config = NetworkProxyConfig::default();
    config
        .network
        .set_allowed_domains(vec!["api.example.com".to_string()]);
    let requirements = NetworkConstraints {
        domains: Some(domain_permissions([(
            "managed-blocked.example.com",
            NetworkDomainPermissionToml::Deny,
        )])),
        ..Default::default()
    };

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &SandboxPolicy::DangerFullAccess,
    )
    .expect("deny-only requirements should not constrain the allowlist");

    assert_eq!(
        spec.config.network.allowed_domains(),
        Some(vec!["api.example.com".to_string()])
    );
    assert_eq!(spec.constraints.allowed_domains, None);
    assert_eq!(spec.constraints.allowlist_expansion_enabled, None);
    assert_eq!(
        spec.config.network.denied_domains(),
        Some(vec!["managed-blocked.example.com".to_string()])
    );
}

#[test]
fn allow_only_requirements_do_not_create_deny_constraints_in_full_access() {
    let mut config = NetworkProxyConfig::default();
    config
        .network
        .set_denied_domains(vec!["blocked.example.com".to_string()]);
    let requirements = NetworkConstraints {
        domains: Some(domain_permissions([(
            "managed.example.com",
            NetworkDomainPermissionToml::Allow,
        )])),
        ..Default::default()
    };

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &SandboxPolicy::DangerFullAccess,
    )
    .expect("allow-only requirements should not constrain the denylist");

    assert_eq!(
        spec.config.network.allowed_domains(),
        Some(vec!["managed.example.com".to_string()])
    );
    assert_eq!(
        spec.config.network.denied_domains(),
        Some(vec!["blocked.example.com".to_string()])
    );
    assert_eq!(spec.constraints.denied_domains, None);
    assert_eq!(spec.constraints.denylist_expansion_enabled, None);
}

#[test]
fn requirements_denied_domains_are_a_baseline_for_default_mode() {
    let mut config = NetworkProxyConfig::default();
    config
        .network
        .set_denied_domains(vec!["blocked.example.com".to_string()]);
    let requirements = NetworkConstraints {
        domains: Some(domain_permissions([(
            "managed-blocked.example.com",
            NetworkDomainPermissionToml::Deny,
        )])),
        ..Default::default()
    };

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &SandboxPolicy::new_workspace_write_policy(),
    )
    .expect("default mode should merge managed and user deny entries");

    assert_eq!(
        spec.config.network.denied_domains(),
        Some(vec![
            "managed-blocked.example.com".to_string(),
            "blocked.example.com".to_string()
        ])
    );
    assert_eq!(
        spec.constraints.denied_domains,
        Some(vec!["managed-blocked.example.com".to_string()])
    );
    assert_eq!(spec.constraints.denylist_expansion_enabled, Some(true));
}

#[test]
fn requirements_denylist_expansion_keeps_user_entries_mutable() {
    let mut config = NetworkProxyConfig::default();
    config
        .network
        .set_denied_domains(vec!["blocked.example.com".to_string()]);
    let requirements = NetworkConstraints {
        domains: Some(domain_permissions([(
            "managed-blocked.example.com",
            NetworkDomainPermissionToml::Deny,
        )])),
        ..Default::default()
    };

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &SandboxPolicy::new_workspace_write_policy(),
    )
    .expect("managed baseline should still allow user edits");

    let mut candidate = spec.config.clone();
    candidate.network.upsert_domain_permission(
        "blocked.example.com".to_string(),
        NetworkDomainPermission::Allow,
        normalize_host,
    );

    assert_eq!(
        candidate.network.allowed_domains(),
        Some(vec!["blocked.example.com".to_string()])
    );
    assert_eq!(
        candidate.network.denied_domains(),
        Some(vec!["managed-blocked.example.com".to_string()])
    );
    validate_policy_against_constraints(&candidate, &spec.constraints)
        .expect("user denylist entries should not become managed constraints");
}
