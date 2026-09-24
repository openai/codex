use super::*;
use crate::config::EnvironmentNetworkConfigError;
use crate::config::validate_environment_network_policy;
use codex_config::NetworkDomainPermissionToml;
use codex_config::NetworkDomainPermissionsToml;
use codex_execpolicy::Decision::Allow;
use codex_execpolicy::NetworkRuleProtocol::Https;
use codex_network_proxy::LocalBindingPolicy::DefaultFalse;
use codex_network_proxy::LocalBindingPolicy::RequireTrue;
use codex_network_proxy::NetworkDomainPermission;
use codex_network_proxy::NetworkUnixSocketPermission;
use codex_network_proxy::NetworkUnixSocketPermissions;
use codex_protocol::models::ManagedFileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
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

#[tokio::test]
async fn attachment_socket_grants_respect_configured_restrictions_at_remote_launch()
-> anyhow::Result<()> {
    let profile = PermissionProfile::workspace_write();
    let owner = EnvironmentNetworkPolicy::from_config(
        &NetworkProxyConfig {
            dangerously_allow_all_unix_sockets: Some(true),
            ..Default::default()
        },
        /*managed_allowed_domains_only*/ false,
    );
    for (name, toml, required, expected) in [
        ("omitted", "", None, true),
        (
            "explicit_deny",
            "dangerously_allow_all_unix_sockets = false",
            None,
            false,
        ),
        (
            "explicit_allow",
            "dangerously_allow_all_unix_sockets = true",
            None,
            true,
        ),
        (
            "finite",
            "unix_sockets = { '/tmp/allowed.sock' = 'allow', '/tmp/denied.sock' = 'deny' }",
            None,
            false,
        ),
        ("empty", "unix_sockets = {}", None, false),
        (
            "managed_deny",
            "dangerously_allow_all_unix_sockets = true",
            Some(false),
            false,
        ),
    ] {
        let configured: codex_config::permissions_toml::NetworkToml = toml::from_str(toml)?;
        let controller = NetworkProxySpec::from_config_and_constraints(
            configured.to_network_proxy_config(),
            Some(NetworkConstraints {
                enabled: Some(true),
                dangerously_allow_all_unix_sockets: required,
                ..Default::default()
            }),
            &profile,
        )?;
        let composed = NetworkProxySpec::for_environment(
            Some(&controller),
            &owner,
            &profile,
            &Policy::empty(),
            DefaultFalse,
        )?;
        assert_eq!(
            composed.config.dangerously_allow_all_unix_sockets,
            Some(expected),
            "{name}"
        );
        let state = Arc::new(controller.build_state_with_audit_metadata(
            Default::default(),
            Platform::Linux,
            DefaultFalse,
        )?);
        let proxy = NetworkProxy::builder()
            .state(Arc::clone(&state))
            .managed_by_codex(false)
            .build()
            .await?;
        let scoped = proxy.for_execution(
            "socket-test",
            name,
            format!("socket-{name}"),
            Some(composed.environment_policy()),
            /*fallback_policy_decider*/ None,
        )?;
        let launch = scoped.remote_launch_config(DefaultFalse).await?;
        assert_eq!(
            launch.proxy.dangerously_allow_all_unix_sockets, expected,
            "{name}"
        );
        assert_eq!(
            launch.proxy.unix_sockets, composed.config.unix_sockets,
            "{name}"
        );
        if name == "finite" {
            assert_eq!(launch.proxy.unix_sockets, controller.config.unix_sockets);
        }
        if name == "omitted" {
            // Inheritance must not grant sockets to ordinary controller-only commands.
            assert!(
                !proxy
                    .remote_launch_config(DefaultFalse)
                    .await?
                    .proxy
                    .dangerously_allow_all_unix_sockets
            );
            state.add_allowed_domain("granted.example").await?;
            assert!(
                scoped
                    .remote_launch_config(DefaultFalse)
                    .await?
                    .proxy
                    .dangerously_allow_all_unix_sockets
            );

            // A live handle must also honor a subsequently installed explicit restriction.
            let restricted = NetworkProxySpec::from_config_and_constraints(
                NetworkProxyConfig {
                    enabled: true,
                    dangerously_allow_all_unix_sockets: Some(false),
                    ..Default::default()
                },
                /*requirements*/ None,
                &profile,
            )?;
            proxy
                .replace_config_state(restricted.build_config_state_for_spec(Platform::Linux)?)
                .await?;
            assert!(
                !scoped
                    .remote_launch_config(DefaultFalse)
                    .await?
                    .proxy
                    .dangerously_allow_all_unix_sockets
            );
        }
    }
    Ok(())
}

#[test]
fn build_state_with_audit_metadata_threads_metadata_to_state() {
    let spec = NetworkProxySpec {
        base_config: NetworkProxyConfig::default(),
        requirements: None,
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
        .build_state_with_audit_metadata(metadata.clone(), Platform::Linux, DefaultFalse)
        .expect("state should build");
    assert_eq!(state.audit_metadata(), &metadata);
}

#[cfg(target_os = "windows")]
#[test]
fn windows_sandbox_proxy_listeners_preserve_effective_protocol_roles() {
    let spec = NetworkProxySpec::from_config_and_constraints(
        NetworkProxyConfig {
            enabled: true,
            proxy_url: "http://127.0.0.1:48081".to_string(),
            socks_url: "socks5h://127.0.0.1:3128".to_string(),
            allow_local_binding: Some(true),
            ..NetworkProxyConfig::default()
        },
        /*requirements*/ None,
        &PermissionProfile::workspace_write(),
    )
    .expect("effective network configuration should be valid");

    assert_eq!(
        spec.windows_sandbox_proxy_listeners()
            .expect("effective proxy listeners should resolve"),
        (
            codex_windows_sandbox::WindowsSandboxProvisioningSettings {
                proxy_ports: vec![3128, 48081],
                allow_local_binding: true,
            },
            codex_windows_sandbox::WindowsSandboxProxyListeners {
                http_ports: vec![48081],
                socks_ports: vec![3128],
            },
        )
    );
}

#[test]
fn environment_policy_replaces_soft_controller_allowlist_and_preserves_denials() {
    let requirements = NetworkConstraints {
        domains: Some(domain_permissions([
            ("controller.example", NetworkDomainPermissionToml::Allow),
            ("blocked.example", NetworkDomainPermissionToml::Deny),
        ])),
        ..Default::default()
    };
    let profile = PermissionProfile::workspace_write();
    let spec = NetworkProxySpec::from_config_and_constraints(
        NetworkProxyConfig {
            enabled: true,
            allow_upstream_proxy: false,
            unix_sockets: Some(NetworkUnixSocketPermissions {
                entries: [
                    (
                        "/tmp/controller.sock".to_string(),
                        NetworkUnixSocketPermission::Deny,
                    ),
                    (
                        "/tmp/allowed.sock".to_string(),
                        NetworkUnixSocketPermission::Allow,
                    ),
                ]
                .into(),
            }),
            ..NetworkProxyConfig::default()
        },
        Some(requirements),
        &profile,
    )
    .expect("controller policy should be valid");
    let mut owner = NetworkProxyConfig::default();
    owner.set_allowed_domains(vec!["owner.example".to_string()]);
    owner.set_denied_domains(vec!["owner-blocked.example".to_string()]);
    owner.set_allow_unix_sockets(vec![
        "/tmp/controller.sock".to_string(),
        "/private/tmp/controller.sock".to_string(),
        "/tmp/allowed.sock".to_string(),
    ]);
    owner.dangerously_allow_all_unix_sockets = Some(true);
    owner.allow_local_binding = Some(true);
    let owner_policy =
        EnvironmentNetworkPolicy::from_config(&owner, /*managed_allowed_domains_only*/ false);
    assert_eq!(
        validate_environment_network_policy(&owner_policy, &profile, Platform::Linux),
        Ok(())
    );
    assert_eq!(
        validate_environment_network_policy(
            &owner_policy,
            &PermissionProfile::Disabled,
            Platform::Linux
        ),
        Err(EnvironmentNetworkConfigError)
    );
    let compose = |controller: Option<&NetworkProxySpec>,
                   policy: &EnvironmentNetworkPolicy,
                   profile: &PermissionProfile,
                   rules: &Policy| {
        NetworkProxySpec::for_environment(controller, policy, profile, rules, DefaultFalse)
    };
    let empty = Policy::empty();
    let disabled_controller = NetworkProxySpec::from_config_and_constraints(
        NetworkProxyConfig::default(),
        /*requirements*/ None,
        &profile,
    )
    .expect("disabled controller policy should be valid");
    assert!(compose(Some(&disabled_controller), &owner_policy, &profile, &empty).is_err());
    let restricted = compose(Some(&spec), &owner_policy, &profile, &empty)
        .expect("owner policy should replace soft controller grants");
    let mut saved = Policy::empty();
    for host in ["saved.example", "owner-blocked.example"] {
        saved
            .add_network_rule(host, Https, Allow, /*justification*/ None)
            .expect("saved network grant should be valid");
    }
    let rootless = compose(/*controller*/ None, &owner_policy, &profile, &saved)
        .expect("an owner policy can create executor-side proxy state");
    assert_eq!(
        rootless.config.allowed_domains().unwrap(),
        ["owner.example", "saved.example"]
    );

    owner.upsert_domain_permission(
        "blocked.example".to_string(),
        NetworkDomainPermission::Deny,
        normalize_host,
    );
    owner.unix_sockets.clone_from(&spec.config.unix_sockets);
    owner.allow_upstream_proxy = false;
    owner.dangerously_allow_all_unix_sockets = Some(false);
    owner.allow_local_binding = Some(true);
    assert_eq!(
        restricted.environment_policy(),
        EnvironmentNetworkPolicy::from_config(&owner, /*managed_allowed_domains_only*/ false)
    );
    let external = PermissionProfile::External {
        network: NetworkSandboxPolicy::Enabled,
    };
    let external_rootless = compose(/*controller*/ None, &owner_policy, &external, &saved)
        .expect("an externally sandboxed owner policy should remain strict");
    assert_eq!(
        external_rootless.environment_policy(),
        EnvironmentNetworkPolicy {
            managed_allowed_domains_only: true,
            ..owner_policy.clone()
        }
    );
    let controller_policy = spec.environment_policy();
    let external_rooted = compose(Some(&spec), &controller_policy, &external, &saved)
        .expect("an externally sandboxed owner policy may retain its controller allowlist");
    assert_eq!(
        external_rooted.environment_policy(),
        EnvironmentNetworkPolicy {
            allow_local_binding: Some(false),
            managed_allowed_domains_only: true,
            ..controller_policy
        }
    );
    assert!(compose(Some(&spec), &owner_policy, &external, &empty).is_err());
    owner.set_allowed_domains(vec!["*".to_string()]);
    let wildcard_policy =
        EnvironmentNetworkPolicy::from_config(&owner, /*managed_allowed_domains_only*/ false);
    assert!(compose(Some(&spec), &wildcard_policy, &profile, &empty).is_err());
    assert_eq!(
        validate_environment_network_policy(&wildcard_policy, &profile, Platform::Linux),
        Err(EnvironmentNetworkConfigError)
    );
    owner.set_allowed_domains(vec!["[".to_string()]);
    let malformed_policy =
        EnvironmentNetworkPolicy::from_config(&owner, /*managed_allowed_domains_only*/ false);
    assert_eq!(
        validate_environment_network_policy(&malformed_policy, &profile, Platform::Linux),
        Err(EnvironmentNetworkConfigError)
    );
}

#[test]
fn requirements_allowed_domains_are_a_baseline_for_user_allowlist() {
    let mut config = NetworkProxyConfig::default();
    config.set_allowed_domains(vec!["api.example.com".to_string()]);
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
        &PermissionProfile::read_only(),
    )
    .expect("config should stay within the managed allowlist");

    assert_eq!(
        spec.config.allowed_domains(),
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
    config.set_denied_domains(vec!["api.example.com".to_string()]);
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
        &PermissionProfile::workspace_write(),
    )
    .expect("managed allowlist should not erase a user deny");

    assert_eq!(spec.config.allowed_domains(), None);
    assert_eq!(
        spec.config.denied_domains(),
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
    config.set_allowed_domains(vec!["api.example.com".to_string()]);
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
        &PermissionProfile::workspace_write(),
    )
    .expect("managed baseline should still allow user edits");

    let mut candidate = spec.config.clone();
    candidate.upsert_domain_permission(
        "api.example.com".to_string(),
        NetworkDomainPermission::Deny,
        normalize_host,
    );

    assert_eq!(
        candidate.allowed_domains(),
        Some(vec!["*.example.com".to_string()])
    );
    assert_eq!(
        candidate.denied_domains(),
        Some(vec!["api.example.com".to_string()])
    );
    validate_policy_against_constraints(&candidate, &spec.constraints)
        .expect("user allowlist entries should not become managed constraints");
}

#[test]
fn managed_unrestricted_profile_allows_domain_expansion() {
    let mut config = NetworkProxyConfig::default();
    config.set_allowed_domains(vec!["api.example.com".to_string()]);
    let requirements = NetworkConstraints {
        domains: Some(domain_permissions([(
            "*.example.com",
            NetworkDomainPermissionToml::Allow,
        )])),
        ..Default::default()
    };
    let permission_profile = PermissionProfile::Managed {
        file_system: ManagedFileSystemPermissions::Unrestricted,
        network: NetworkSandboxPolicy::Restricted,
    };

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &permission_profile,
    )
    .expect("managed unrestricted filesystem should still use managed network constraints");

    assert_eq!(
        spec.config.allowed_domains(),
        Some(vec![
            "*.example.com".to_string(),
            "api.example.com".to_string()
        ])
    );
    assert_eq!(spec.constraints.allowlist_expansion_enabled, Some(true));
}

#[test]
fn danger_full_access_keeps_managed_allowlist_and_denylist_fixed() {
    let mut config = NetworkProxyConfig::default();
    config.set_allowed_domains(vec!["evil.com".to_string()]);
    config.set_denied_domains(vec!["more-blocked.example.com".to_string()]);
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
        &PermissionProfile::Disabled,
    )
    .expect("yolo mode should pin the effective policy to the managed baseline");

    assert_eq!(
        spec.config.allowed_domains(),
        Some(vec!["*.example.com".to_string()])
    );
    assert_eq!(
        spec.config.denied_domains(),
        Some(vec!["blocked.example.com".to_string()])
    );
    assert_eq!(spec.constraints.allowlist_expansion_enabled, Some(false));
    assert_eq!(spec.constraints.denylist_expansion_enabled, Some(false));
}

#[test]
fn managed_allowed_domains_only_disables_default_mode_allowlist_expansion() {
    let mut config = NetworkProxyConfig::default();
    config.set_allowed_domains(vec!["api.example.com".to_string()]);
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
        &PermissionProfile::workspace_write(),
    )
    .expect("managed baseline should still load");

    assert_eq!(
        spec.config.allowed_domains(),
        Some(vec!["*.example.com".to_string()])
    );
    assert_eq!(spec.constraints.allowlist_expansion_enabled, Some(false));
}

#[test]
fn managed_allowed_domains_only_ignores_user_allowlist_and_hard_denies_misses() {
    let mut config = NetworkProxyConfig::default();
    config.set_allowed_domains(vec!["api.example.com".to_string()]);
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
        &PermissionProfile::workspace_write(),
    )
    .expect("managed-only allowlist should still load");

    assert_eq!(
        spec.config.allowed_domains(),
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
    config.set_allowed_domains(vec!["api.example.com".to_string()]);
    let requirements = NetworkConstraints {
        managed_allowed_domains_only: Some(true),
        ..Default::default()
    };

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &PermissionProfile::workspace_write(),
    )
    .expect("managed-only mode should treat missing managed allowlist as empty");

    assert_eq!(spec.config.allowed_domains(), None);
    assert_eq!(spec.constraints.allowed_domains, Some(Vec::new()));
    assert_eq!(spec.constraints.allowlist_expansion_enabled, Some(false));
    assert!(spec.hard_deny_allowlist_misses);
}

#[test]
fn managed_allowed_domains_only_blocks_all_user_domains_in_full_access_without_managed_list() {
    let mut config = NetworkProxyConfig::default();
    config.set_allowed_domains(vec!["api.example.com".to_string()]);
    let requirements = NetworkConstraints {
        managed_allowed_domains_only: Some(true),
        ..Default::default()
    };

    let spec = NetworkProxySpec::from_config_and_constraints(
        config,
        Some(requirements),
        &PermissionProfile::Disabled,
    )
    .expect("managed-only mode should treat missing managed allowlist as empty");

    assert_eq!(spec.config.allowed_domains(), None);
    assert_eq!(spec.constraints.allowed_domains, Some(Vec::new()));
    assert_eq!(spec.constraints.allowlist_expansion_enabled, Some(false));
    assert!(spec.hard_deny_allowlist_misses);
}

#[test]
fn deny_only_requirements_do_not_create_allow_constraints_in_full_access() {
    let mut config = NetworkProxyConfig::default();
    config.set_allowed_domains(vec!["api.example.com".to_string()]);
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
        &PermissionProfile::Disabled,
    )
    .expect("deny-only requirements should not constrain the allowlist");

    assert_eq!(
        spec.config.allowed_domains(),
        Some(vec!["api.example.com".to_string()])
    );
    assert_eq!(spec.constraints.allowed_domains, None);
    assert_eq!(spec.constraints.allowlist_expansion_enabled, None);
    assert_eq!(
        spec.config.denied_domains(),
        Some(vec!["managed-blocked.example.com".to_string()])
    );
}

#[test]
fn allow_only_requirements_do_not_create_deny_constraints_in_full_access() {
    let mut config = NetworkProxyConfig::default();
    config.set_denied_domains(vec!["blocked.example.com".to_string()]);
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
        &PermissionProfile::Disabled,
    )
    .expect("allow-only requirements should not constrain the denylist");

    assert_eq!(
        spec.config.allowed_domains(),
        Some(vec!["managed.example.com".to_string()])
    );
    assert_eq!(
        spec.config.denied_domains(),
        Some(vec!["blocked.example.com".to_string()])
    );
    assert_eq!(spec.constraints.denied_domains, None);
    assert_eq!(spec.constraints.denylist_expansion_enabled, None);
}

#[test]
fn requirements_denied_domains_are_a_baseline_for_default_mode() {
    let mut config = NetworkProxyConfig::default();
    config.set_denied_domains(vec!["blocked.example.com".to_string()]);
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
        &PermissionProfile::workspace_write(),
    )
    .expect("default mode should merge managed and user deny entries");

    assert_eq!(
        spec.config.denied_domains(),
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
    config.set_denied_domains(vec!["blocked.example.com".to_string()]);
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
        &PermissionProfile::workspace_write(),
    )
    .expect("managed baseline should still allow user edits");

    let mut candidate = spec.config.clone();
    candidate.upsert_domain_permission(
        "blocked.example.com".to_string(),
        NetworkDomainPermission::Allow,
        normalize_host,
    );

    assert_eq!(
        candidate.allowed_domains(),
        Some(vec!["blocked.example.com".to_string()])
    );
    assert_eq!(
        candidate.denied_domains(),
        Some(vec!["managed-blocked.example.com".to_string()])
    );
    validate_policy_against_constraints(&candidate, &spec.constraints)
        .expect("user denylist entries should not become managed constraints");
}

#[tokio::test]
async fn environment_local_binding_preserves_explicit_denials_and_inherits_omitted_settings()
-> anyhow::Result<()> {
    let managed: codex_config::ConfigRequirementsToml = toml::from_str(
        r#"
        [experimental_network]
        enabled = true
        managed_allowed_domains_only = true
        allowed_domains = ["example.com"]
        "#,
    )?;
    for (name, configured, required, expected_bindings) in [
        (
            "absent",
            None,
            None,
            [false, false, true, false, false, true],
        ),
        (
            "omitted",
            Some(None),
            None,
            [false, false, true, true, false, true],
        ),
        (
            "managed_omitted",
            Some(None),
            managed.network.map(Into::into),
            [false, false, true, true, false, true],
        ),
        ("configured_deny", Some(Some(false)), None, [false; 6]),
        (
            "configured_allow",
            Some(Some(true)),
            None,
            [true, false, true, true, false, true],
        ),
        (
            "managed_deny",
            Some(Some(true)),
            Some(NetworkConstraints {
                allow_local_binding: Some(false),
                ..Default::default()
            }),
            [false; 6],
        ),
        (
            "managed_allow",
            Some(Some(false)),
            Some(NetworkConstraints {
                allow_local_binding: Some(true),
                ..Default::default()
            }),
            [true, false, true, true, false, true],
        ),
    ] {
        let profile = PermissionProfile::workspace_write();
        let controller = configured
            .map(|allow_local_binding| {
                NetworkProxySpec::from_config_and_constraints(
                    NetworkProxyConfig {
                        enabled: true,
                        allow_local_binding,
                        ..Default::default()
                    },
                    required,
                    &profile,
                )
            })
            .transpose()?;
        if name == "managed_omitted" {
            let controller = controller.as_ref().unwrap();
            assert_eq!(
                (
                    controller.config.allow_local_binding,
                    controller.constraints.allow_local_binding,
                ),
                (None, None)
            );
        }
        let original_controller = controller.clone();
        for ((local_binding_policy, owner_binding), expected) in [
            (DefaultFalse, None),
            (DefaultFalse, Some(false)),
            (DefaultFalse, Some(true)),
            (RequireTrue, None),
            (RequireTrue, Some(false)),
            (RequireTrue, Some(true)),
        ]
        .into_iter()
        .zip(expected_bindings)
        {
            let mut owner_config = NetworkProxyConfig {
                allow_local_binding: owner_binding,
                ..Default::default()
            };
            owner_config.set_allowed_domains(vec!["example.com".to_string()]);
            let owner =
                EnvironmentNetworkPolicy::from_config(&owner_config, name == "managed_omitted");
            let composed = NetworkProxySpec::for_environment(
                controller.as_ref(),
                &owner,
                &profile,
                &Policy::empty(),
                local_binding_policy,
            )?;
            assert_eq!(
                composed.environment_policy(),
                EnvironmentNetworkPolicy {
                    allow_local_binding: Some(expected),
                    ..owner.clone()
                },
                "{name}: {local_binding_policy:?}, owner={owner_binding:?}"
            );
            let carrier = controller.as_ref().unwrap_or(&composed);
            let proxy = NetworkProxy::builder()
                .state(Arc::new(carrier.build_state_with_audit_metadata(
                    Default::default(),
                    Platform::Linux,
                    DefaultFalse,
                )?))
                .managed_by_codex(/*managed_by_codex*/ false)
                .build()
                .await?;
            for policy in [owner, composed.environment_policy()] {
                let scoped = proxy.for_execution(
                    "remote",
                    "binding-test",
                    "binding-token".to_string(),
                    Some(policy),
                    Some(Arc::new(|_request| async { NetworkDecision::Allow })),
                )?;
                let launch = scoped.remote_launch_config(local_binding_policy).await;
                assert_eq!(proxy.current_cfg().await?, carrier.config, "{name}");
                if local_binding_policy == RequireTrue && !expected {
                    assert!(
                        launch
                            .unwrap_err()
                            .to_string()
                            .contains("MXC cannot enforce allow_local_binding=false"),
                        "{name}"
                    );
                    continue;
                }
                let launch = launch?;
                assert_eq!(launch.proxy.allow_local_binding, expected, "{name}");
                let Some(decider) = scoped.remote_policy_decider(launch.proxy.allow_local_binding)
                else {
                    assert_eq!(name, "managed_omitted");
                    continue;
                };
                let decision = decider
                    .decide(codex_network_proxy::NetworkPolicyRequest::new(
                        codex_network_proxy::NetworkPolicyRequestArgs {
                            protocol: codex_network_proxy::NetworkProtocol::HttpsConnect,
                            host: "10.0.0.1".to_string(),
                            port: 443,
                            environment_id: None,
                            client_addr: None,
                            method: None,
                            command: None,
                            exec_policy_hint: None,
                        },
                    ))
                    .await;
                assert_eq!(
                    decision,
                    if expected {
                        NetworkDecision::Allow
                    } else {
                        NetworkDecision::deny_with_source(
                            "not_allowed_local",
                            codex_network_proxy::NetworkDecisionSource::BaselinePolicy,
                        )
                    },
                    "{name}"
                );
            }
        }
        assert_eq!(controller, original_controller, "{name}");
    }
    Ok(())
}
