use super::*;
use crate::AppRequirementToml;
use crate::AppToolApproval;
use crate::AppToolRequirementToml;
use crate::AppToolsRequirementsToml;
use crate::AppsRequirementsToml;
use crate::FeatureRequirementsToml;
use crate::HookEventsToml;
use crate::HookHandlerConfig;
use crate::ManagedHooksRequirementsToml;
use crate::MatcherGroup;
use crate::NetworkDomainPermissionToml;
use crate::NetworkDomainPermissionsToml;
use crate::NetworkUnixSocketPermissionToml;
use crate::NetworkUnixSocketPermissionsToml;
use crate::RequirementSource;
use crate::RequirementsExecPolicyDecisionToml;
use crate::RequirementsExecPolicyPatternTokenToml;
use crate::RequirementsExecPolicyPrefixRuleToml;
use crate::SandboxModeRequirement;
use crate::Sourced;
use crate::config_requirements::FilesystemRequirementsToml;
use crate::config_requirements::PermissionsRequirementsToml;
use codex_protocol::protocol::AskForApproval;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::path::PathBuf;

// These tests intentionally exercise the composition boundary instead of the
// private helper modules. The public behavior depends on parsing, layer order,
// source provenance, and diagnostics together. Add focused cases here when a
// merge policy changes; use helper-level tests only for purely local algorithms.

fn fragment(id: &str, name: &str, contents: &str) -> CloudRequirementsFragment {
    CloudRequirementsFragment {
        id: id.to_string(),
        name: name.to_string(),
        contents: contents.to_string(),
    }
}

fn compose(
    fragments: Vec<CloudRequirementsFragment>,
) -> Result<Option<ConfigRequirementsToml>, CloudRequirementsCompositionError> {
    Ok(
        compose_cloud_requirements_for_hostname(fragments, /*hostname*/ None)?
            .map(ConfigRequirementsWithSources::into_toml),
    )
}

fn compose_with_hook_directory_field(
    fragments: Vec<CloudRequirementsFragment>,
    hook_directory_field: HookDirectoryField,
) -> Result<Option<ConfigRequirementsToml>, CloudRequirementsCompositionError> {
    Ok(compose_cloud_requirements_for_hostname_and_hook_directory(
        fragments,
        /*hostname*/ None,
        hook_directory_field,
    )?
    .map(ConfigRequirementsWithSources::into_toml))
}

#[test]
fn empty_fragments_compose_to_none() {
    let composed = compose(Vec::new()).expect("compose empty fragments");
    assert_eq!(composed, None);
}

#[test]
fn first_wins_for_top_level_fields() {
    let composed = compose(vec![
        fragment(
            "req_high",
            "High",
            r#"
allowed_approval_policies = ["never"]
allowed_sandbox_modes = ["read-only"]
"#,
        ),
        fragment(
            "req_low",
            "Low",
            r#"
allowed_approval_policies = ["on-request"]
allowed_sandbox_modes = ["workspace-write"]
"#,
        ),
    ])
    .expect("compose requirements")
    .expect("requirements present");

    assert_eq!(
        composed.allowed_approval_policies,
        Some(vec![AskForApproval::Never])
    );
    assert_eq!(
        composed.allowed_sandbox_modes,
        Some(vec![SandboxModeRequirement::ReadOnly])
    );
}

#[test]
fn scalar_fields_keep_enterprise_managed_source() {
    let composed = compose_cloud_requirements_for_hostname(
        vec![fragment(
            "req_1",
            "Security baseline",
            r#"
allow_managed_hooks_only = true
"#,
        )],
        /*hostname*/ None,
    )
    .expect("compose requirements")
    .expect("requirements present");

    assert_eq!(
        composed.allow_managed_hooks_only,
        Some(Sourced::new(
            /*value*/ true,
            RequirementSource::EnterpriseManaged {
                id: "req_1".to_string(),
                name: "Security baseline".to_string(),
            },
        ))
    );
}

#[test]
fn remote_sandbox_config_is_applied_per_fragment() {
    let composed = compose_cloud_requirements_for_hostname(
        vec![
            fragment(
                "req_high",
                "High",
                r#"
[[remote_sandbox_config]]
hostname_patterns = ["build-*.example.com"]
allowed_sandbox_modes = ["workspace-write"]
"#,
            ),
            fragment(
                "req_low",
                "Low",
                r#"
allowed_sandbox_modes = ["read-only"]
"#,
            ),
        ],
        Some("BUILD-01.EXAMPLE.COM."),
    )
    .expect("compose requirements")
    .expect("requirements present")
    .into_toml();

    assert_eq!(
        composed.allowed_sandbox_modes,
        Some(vec![SandboxModeRequirement::WorkspaceWrite])
    );
}

#[test]
fn feature_requirements_are_key_first_wins() {
    let composed = compose(vec![
        fragment(
            "req_high",
            "High",
            r#"
[features]
alpha = true
shared = true
"#,
        ),
        fragment(
            "req_low",
            "Low",
            r#"
[features]
beta = false
shared = false
"#,
        ),
    ])
    .expect("compose requirements")
    .expect("requirements present");

    assert_eq!(
        composed.feature_requirements,
        Some(FeatureRequirementsToml {
            entries: BTreeMap::from([
                ("alpha".to_string(), true),
                ("beta".to_string(), false),
                ("shared".to_string(), true),
            ]),
        })
    );
}

#[test]
fn mcp_servers_union_compatible_duplicates() {
    let composed = compose(vec![
        fragment(
            "req_high",
            "High",
            r#"
[mcp_servers.shared.identity]
command = "shared-mcp"

[mcp_servers.high.identity]
url = "https://high.example.com/mcp"
"#,
        ),
        fragment(
            "req_low",
            "Low",
            r#"
[mcp_servers.shared.identity]
command = "shared-mcp"

[mcp_servers.low.identity]
command = "low-mcp"
"#,
        ),
    ])
    .expect("compose requirements")
    .expect("requirements present");

    let mcp_servers = composed.mcp_servers.expect("mcp servers");
    assert_eq!(mcp_servers.len(), 3);
}

#[test]
fn empty_mcp_servers_allowlist_is_preserved() {
    let composed = compose(vec![fragment(
        "req_high",
        "High",
        r#"
[mcp_servers]
"#,
    )])
    .expect("compose requirements")
    .expect("requirements present");

    assert_eq!(composed.mcp_servers, Some(BTreeMap::new()));
}

#[test]
fn mcp_servers_conflict_on_incompatible_duplicates() {
    let err = compose(vec![
        fragment(
            "req_high",
            "High",
            r#"
[mcp_servers.shared.identity]
command = "high-mcp"
"#,
        ),
        fragment(
            "req_low",
            "Low",
            r#"
[mcp_servers.shared.identity]
command = "low-mcp"
"#,
        ),
    ])
    .expect_err("incompatible mcp servers should fail closed");

    assert!(err.to_string().contains("mcp_servers.shared"));
    assert!(err.to_string().contains("High (req_high)"));
    assert!(err.to_string().contains("Low (req_low)"));
}

#[test]
fn plugin_mcp_servers_conflict_on_incompatible_duplicates() {
    let err = compose(vec![
        fragment(
            "req_high",
            "High",
            r#"
[plugins.search.mcp_servers.shared.identity]
command = "high-mcp"
"#,
        ),
        fragment(
            "req_low",
            "Low",
            r#"
[plugins.search.mcp_servers.shared.identity]
command = "low-mcp"
"#,
        ),
    ])
    .expect_err("incompatible plugin mcp servers should fail closed");

    assert!(
        err.to_string()
            .contains("plugins.search.mcp_servers.shared")
    );
    assert!(err.to_string().contains("High (req_high)"));
    assert!(err.to_string().contains("Low (req_low)"));
}

#[test]
fn empty_plugin_mcp_servers_allowlist_is_preserved() {
    let composed = compose(vec![fragment(
        "req_high",
        "High",
        r#"
[plugins.search.mcp_servers]
"#,
    )])
    .expect("compose requirements")
    .expect("requirements present");

    assert_eq!(
        composed.plugins,
        Some(BTreeMap::from([(
            "search".to_string(),
            crate::PluginRequirementsToml {
                mcp_servers: Some(BTreeMap::new()),
            }
        )]))
    );
}

#[test]
fn network_maps_union_unique_keys_and_keep_highest_priority_duplicates() {
    let composed = compose(vec![
        fragment(
            "req_high",
            "High",
            r#"
[experimental_network.domains]
"example.com" = "allow"
"high.example.com" = "allow"
"internal.example.com" = "deny"

[experimental_network.unix_sockets]
"/tmp/shared.sock" = "allow"
"/tmp/high.sock" = "allow"
"/tmp/admin.sock" = "none"
"#,
        ),
        fragment(
            "req_low",
            "Low",
            r#"
[experimental_network.domains]
"example.com" = "deny"
"low.example.com" = "deny"
"internal.example.com" = "allow"

[experimental_network.unix_sockets]
"/tmp/shared.sock" = "none"
"/tmp/low.sock" = "allow"
"/tmp/admin.sock" = "allow"
"#,
        ),
    ])
    .expect("compose requirements")
    .expect("requirements present");

    let network = composed.network.expect("network requirements");
    assert_eq!(
        network.domains,
        Some(NetworkDomainPermissionsToml {
            entries: BTreeMap::from([
                (
                    "example.com".to_string(),
                    NetworkDomainPermissionToml::Allow,
                ),
                (
                    "high.example.com".to_string(),
                    NetworkDomainPermissionToml::Allow,
                ),
                (
                    "internal.example.com".to_string(),
                    NetworkDomainPermissionToml::Deny,
                ),
                (
                    "low.example.com".to_string(),
                    NetworkDomainPermissionToml::Deny,
                ),
            ]),
        })
    );
    assert_eq!(
        network.unix_sockets,
        Some(NetworkUnixSocketPermissionsToml {
            entries: BTreeMap::from([
                (
                    "/tmp/admin.sock".to_string(),
                    NetworkUnixSocketPermissionToml::None,
                ),
                (
                    "/tmp/high.sock".to_string(),
                    NetworkUnixSocketPermissionToml::Allow,
                ),
                (
                    "/tmp/low.sock".to_string(),
                    NetworkUnixSocketPermissionToml::Allow,
                ),
                (
                    "/tmp/shared.sock".to_string(),
                    NetworkUnixSocketPermissionToml::Allow,
                ),
            ]),
        })
    );
}

#[test]
fn filesystem_deny_read_is_union_deduped() {
    let high_path = if cfg!(windows) {
        "C:\\secret"
    } else {
        "/secret"
    };
    let low_path = if cfg!(windows) {
        "C:\\other-secret"
    } else {
        "/other-secret"
    };
    let composed = compose(vec![
        fragment(
            "req_high",
            "High",
            &format!(
                r#"
[permissions.filesystem]
deny_read = [{high_path:?}]
"#
            ),
        ),
        fragment(
            "req_low",
            "Low",
            &format!(
                r#"
[permissions.filesystem]
deny_read = [{high_path:?}, {low_path:?}]
"#
            ),
        ),
    ])
    .expect("compose requirements")
    .expect("requirements present");

    let permissions = composed.permissions.expect("permissions");
    assert_eq!(
        permissions,
        PermissionsRequirementsToml {
            filesystem: Some(FilesystemRequirementsToml {
                deny_read: Some(vec![
                    AbsolutePathBuf::from_absolute_path(high_path)
                        .expect("absolute path")
                        .into(),
                    AbsolutePathBuf::from_absolute_path(low_path)
                        .expect("absolute path")
                        .into(),
                ]),
            }),
            profiles: BTreeMap::new(),
        }
    );
}

#[test]
fn permission_profiles_merge_by_name_with_highest_priority_winning() {
    let composed = compose(vec![
        fragment(
            "req_high",
            "High",
            r#"
[permissions.managed-standard]
description = "High profile"
extends = ":read-only"
"#,
        ),
        fragment(
            "req_low",
            "Low",
            r#"
[permissions.managed-standard]
description = "Low profile"
extends = ":workspace"

[permissions.managed-build]
extends = ":workspace"
"#,
        ),
    ])
    .expect("compose requirements")
    .expect("requirements present");

    let permissions = composed.permissions.expect("permissions");
    assert_eq!(
        permissions.profiles.keys().collect::<Vec<_>>(),
        vec!["managed-build", "managed-standard"]
    );
    assert_eq!(
        permissions
            .profiles
            .get("managed-standard")
            .and_then(|profile| profile.description.as_deref()),
        Some("High profile")
    );
}

#[test]
fn rules_are_appended_in_bundle_order() {
    let composed = compose(vec![
        fragment(
            "req_high",
            "High",
            r#"
[[rules.prefix_rules]]
pattern = [{ token = "git" }]
decision = "forbidden"
"#,
        ),
        fragment(
            "req_low",
            "Low",
            r#"
[[rules.prefix_rules]]
pattern = [{ token = "npm" }]
decision = "prompt"
"#,
        ),
    ])
    .expect("compose requirements")
    .expect("requirements present");

    let rules = composed.rules.expect("rules");
    assert_eq!(
        rules,
        RequirementsExecPolicyToml {
            prefix_rules: vec![
                RequirementsExecPolicyPrefixRuleToml {
                    pattern: vec![RequirementsExecPolicyPatternTokenToml {
                        token: Some("git".to_string()),
                        any_of: None,
                    }],
                    decision: Some(RequirementsExecPolicyDecisionToml::Forbidden),
                    justification: None,
                },
                RequirementsExecPolicyPrefixRuleToml {
                    pattern: vec![RequirementsExecPolicyPatternTokenToml {
                        token: Some("npm".to_string()),
                        any_of: None,
                    }],
                    decision: Some(RequirementsExecPolicyDecisionToml::Prompt),
                    justification: None,
                },
            ],
        }
    );
}

#[test]
fn hooks_append_groups_and_reject_conflicting_managed_dirs() {
    let composed = compose_with_hook_directory_field(
        vec![
            fragment(
                "req_high",
                "High",
                r#"
[hooks]
managed_dir = "/managed/hooks"

[[hooks.PreToolUse]]
matcher = "Edit"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "high"
"#,
            ),
            fragment(
                "req_low",
                "Low",
                r#"
[hooks]
managed_dir = "/managed/hooks"

[[hooks.PreToolUse]]
matcher = "Bash"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "low"
"#,
            ),
        ],
        HookDirectoryField::ManagedDir,
    )
    .expect("compose requirements")
    .expect("requirements present");

    let hooks = composed.hooks.expect("hooks");
    assert_eq!(
        hooks,
        ManagedHooksRequirementsToml {
            managed_dir: Some(PathBuf::from("/managed/hooks")),
            windows_managed_dir: None,
            hooks: HookEventsToml {
                pre_tool_use: vec![
                    MatcherGroup {
                        matcher: Some("Edit".to_string()),
                        hooks: vec![HookHandlerConfig::Command {
                            command: "high".to_string(),
                            command_windows: None,
                            timeout_sec: None,
                            r#async: false,
                            status_message: None,
                        }],
                    },
                    MatcherGroup {
                        matcher: Some("Bash".to_string()),
                        hooks: vec![HookHandlerConfig::Command {
                            command: "low".to_string(),
                            command_windows: None,
                            timeout_sec: None,
                            r#async: false,
                            status_message: None,
                        }],
                    },
                ],
                ..HookEventsToml::default()
            },
        }
    );

    let err = compose_with_hook_directory_field(
        vec![
            fragment(
                "req_high",
                "High",
                r#"
[hooks]
managed_dir = "/managed/high"
"#,
            ),
            fragment(
                "req_low",
                "Low",
                r#"
[hooks]
managed_dir = "/managed/low"
"#,
            ),
        ],
        HookDirectoryField::ManagedDir,
    )
    .expect_err("conflicting managed dirs should fail closed");
    assert!(err.to_string().contains("hooks.managed_dir"));
    assert!(err.to_string().contains("High (req_high)"));
    assert!(err.to_string().contains("Low (req_low)"));
}

#[test]
fn active_windows_managed_dir_conflicts_fail_closed() {
    let err = compose_with_hook_directory_field(
        vec![
            fragment(
                "req_high",
                "High",
                r#"
[hooks]
windows_managed_dir = 'C:\managed\high'
"#,
            ),
            fragment(
                "req_low",
                "Low",
                r#"
[hooks]
windows_managed_dir = 'C:\managed\low'
"#,
            ),
        ],
        HookDirectoryField::WindowsManagedDir,
    )
    .expect_err("conflicting windows managed dirs should fail closed");

    assert!(err.to_string().contains("hooks.windows_managed_dir"));
    assert!(err.to_string().contains("High (req_high)"));
    assert!(err.to_string().contains("Low (req_low)"));
}

#[test]
fn inactive_hook_dir_conflicts_do_not_fail_composition() {
    let composed = compose_with_hook_directory_field(
        vec![
            fragment(
                "req_high",
                "High",
                r#"
[hooks]
managed_dir = "/managed/hooks"
windows_managed_dir = 'C:\managed\high'

[[hooks.PreToolUse]]
matcher = "Edit"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "high"
"#,
            ),
            fragment(
                "req_low",
                "Low",
                r#"
[hooks]
managed_dir = "/managed/hooks"
windows_managed_dir = 'C:\managed\low'

[[hooks.PreToolUse]]
matcher = "Bash"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "low"
"#,
            ),
        ],
        HookDirectoryField::ManagedDir,
    )
    .expect("inactive windows managed dir conflict should not fail")
    .expect("requirements present");

    let hooks = composed.hooks.expect("hooks");
    assert_eq!(hooks.managed_dir, Some(PathBuf::from("/managed/hooks")));
    assert_eq!(
        hooks.windows_managed_dir,
        Some(PathBuf::from(r"C:\managed\high"))
    );
    assert_eq!(hooks.hooks.pre_tool_use.len(), 2);

    let composed = compose_with_hook_directory_field(
        vec![
            fragment(
                "req_high",
                "High",
                r#"
[hooks]
managed_dir = "/managed/high"
windows_managed_dir = 'C:\managed\hooks'

[[hooks.PreToolUse]]
matcher = "Edit"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "high"
"#,
            ),
            fragment(
                "req_low",
                "Low",
                r#"
[hooks]
managed_dir = "/managed/low"
windows_managed_dir = 'C:\managed\hooks'

[[hooks.PreToolUse]]
matcher = "Bash"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "low"
"#,
            ),
        ],
        HookDirectoryField::WindowsManagedDir,
    )
    .expect("inactive managed dir conflict should not fail")
    .expect("requirements present");

    let hooks = composed.hooks.expect("hooks");
    assert_eq!(hooks.managed_dir, Some(PathBuf::from("/managed/high")));
    assert_eq!(
        hooks.windows_managed_dir,
        Some(PathBuf::from(r"C:\managed\hooks"))
    );
    assert_eq!(hooks.hooks.pre_tool_use.len(), 2);
}

#[test]
fn hook_source_collapses_when_later_layer_sets_managed_dir() {
    let composed = compose_cloud_requirements_for_hostname_and_hook_directory(
        vec![
            fragment(
                "req_high",
                "High",
                r#"
[[hooks.PreToolUse]]
matcher = "Edit"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "high"
"#,
            ),
            fragment(
                "req_low",
                "Low",
                r#"
[hooks]
managed_dir = "/managed/hooks"
"#,
            ),
        ],
        /*hostname*/ None,
        HookDirectoryField::ManagedDir,
    )
    .expect("compose requirements")
    .expect("requirements present");

    assert_eq!(
        composed.hooks,
        Some(Sourced::new(
            ManagedHooksRequirementsToml {
                managed_dir: Some(PathBuf::from("/managed/hooks")),
                windows_managed_dir: None,
                hooks: HookEventsToml {
                    pre_tool_use: vec![MatcherGroup {
                        matcher: Some("Edit".to_string()),
                        hooks: vec![HookHandlerConfig::Command {
                            command: "high".to_string(),
                            command_windows: None,
                            timeout_sec: None,
                            r#async: false,
                            status_message: None,
                        }],
                    }],
                    ..HookEventsToml::default()
                },
            },
            RequirementSource::CloudRequirements,
        ))
    );
}

#[test]
fn apps_reuse_disable_wins_behavior() {
    let composed = compose(vec![
        fragment(
            "req_high",
            "High",
            r#"
[apps.connector_1]
enabled = true

[apps.connector_1.tools.search]
approval_mode = "approve"
"#,
        ),
        fragment(
            "req_low",
            "Low",
            r#"
[apps.connector_1]
enabled = false

[apps.connector_1.tools.search]
approval_mode = "prompt"
"#,
        ),
    ])
    .expect("compose requirements")
    .expect("requirements present");

    assert_eq!(
        composed.apps,
        Some(AppsRequirementsToml {
            apps: BTreeMap::from([(
                "connector_1".to_string(),
                AppRequirementToml {
                    enabled: Some(false),
                    tools: Some(AppToolsRequirementsToml {
                        tools: BTreeMap::from([(
                            "search".to_string(),
                            AppToolRequirementToml {
                                approval_mode: Some(AppToolApproval::Approve),
                            },
                        )]),
                    }),
                },
            )]),
        })
    );
}

#[test]
fn parse_error_names_fragment() {
    let err = compose(vec![fragment(
        "req_bad",
        "Bad layer",
        "allowed_approval_policies = [1]",
    )])
    .expect_err("invalid fragment should fail");

    assert!(err.to_string().contains("Bad layer (req_bad)"));
    assert!(err.to_string().contains("allowed_approval_policies"));
}
