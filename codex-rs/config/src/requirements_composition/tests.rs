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
use crate::McpServerIdentity;
use crate::McpServerRequirement;
use crate::NetworkDomainPermissionToml;
use crate::NetworkDomainPermissionsToml;
use crate::NetworkUnixSocketPermissionToml;
use crate::NetworkUnixSocketPermissionsToml;
use crate::RequirementSource;
use crate::RequirementsExecPolicyDecisionToml;
use crate::RequirementsExecPolicyPatternTokenToml;
use crate::RequirementsExecPolicyPrefixRuleToml;
use crate::RequirementsExecPolicyToml;
use crate::SandboxModeRequirement;
use crate::Sourced;
use crate::config_requirements::FilesystemRequirementsToml;
use crate::config_requirements::PermissionsRequirementsToml;
use codex_protocol::protocol::AskForApproval;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn layer(id: &str, name: &str, contents: &str) -> RequirementsLayer {
    RequirementsLayer::from_toml(
        RequirementSource::EnterpriseManaged {
            id: id.to_string(),
            name: name.to_string(),
        },
        contents,
    )
}

fn compose(
    layers: Vec<RequirementsLayer>,
) -> Result<Option<ConfigRequirementsToml>, RequirementsCompositionError> {
    Ok(
        compose_requirements_for_hostname(layers, /*hostname*/ None)?
            .map(ConfigRequirementsWithSources::into_toml),
    )
}

fn compose_with_hook_directory_field(
    layers: Vec<RequirementsLayer>,
    hook_directory_field: HookDirectoryField,
) -> Result<Option<ConfigRequirementsToml>, RequirementsCompositionError> {
    Ok(compose_requirements_for_hostname_and_hook_directory(
        layers,
        /*hostname*/ None,
        hook_directory_field,
    )?
    .map(ConfigRequirementsWithSources::into_toml))
}

#[test]
fn empty_layers_compose_to_none() {
    let composed = compose(Vec::new()).expect("compose empty layers");
    assert_eq!(composed, None);
}

#[test]
fn top_level_values_use_toml_priority() {
    let composed = compose(vec![
        layer(
            "req_low",
            "Low",
            r#"
allowed_approval_policies = ["on-request"]
allowed_sandbox_modes = ["workspace-write"]
"#,
        ),
        layer(
            "req_high",
            "High",
            r#"
allowed_approval_policies = ["never"]
allowed_sandbox_modes = ["read-only"]
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
fn composition_strategy_applies_to_non_cloud_layers() {
    let mdm_source = RequirementSource::MdmManagedPreferences {
        domain: "com.openai.codex".to_string(),
        key: "requirements_toml_base64".to_string(),
    };
    let system_file = if cfg!(windows) {
        "C:\\requirements.toml"
    } else {
        "/etc/codex/requirements.toml"
    };
    let system_source = RequirementSource::SystemRequirementsToml {
        file: AbsolutePathBuf::from_absolute_path(system_file).expect("absolute path"),
    };
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

    let composed = compose_requirements_for_hostname(
        vec![
            RequirementsLayer::from_toml(
                system_source,
                format!(
                    r#"
allowed_approval_policies = ["on-request"]

[features]
shared = false
system = true

[[rules.prefix_rules]]
pattern = [{{ token = "npm" }}]
decision = "prompt"

[permissions.filesystem]
deny_read = [{low_path:?}]
"#
                ),
            ),
            RequirementsLayer::from_toml(
                mdm_source.clone(),
                format!(
                    r#"
allowed_approval_policies = ["never"]

[features]
shared = true

[[rules.prefix_rules]]
pattern = [{{ token = "git" }}]
decision = "forbidden"

[permissions.filesystem]
deny_read = [{high_path:?}]
"#
                ),
            ),
        ],
        /*hostname*/ None,
    )
    .expect("compose requirements")
    .expect("requirements present");

    assert_eq!(
        composed.allowed_approval_policies,
        Some(Sourced::new(vec![AskForApproval::Never], mdm_source))
    );
    assert_eq!(
        composed
            .feature_requirements
            .expect("feature requirements")
            .value,
        FeatureRequirementsToml {
            entries: BTreeMap::from([("shared".to_string(), true), ("system".to_string(), true),]),
        }
    );
    assert_eq!(composed.rules.expect("rules").value.prefix_rules.len(), 2);
    assert_eq!(
        composed
            .permissions
            .expect("permissions")
            .value
            .filesystem
            .expect("filesystem")
            .deny_read
            .expect("deny_read"),
        vec![
            AbsolutePathBuf::from_absolute_path(high_path)
                .expect("absolute path")
                .into(),
            AbsolutePathBuf::from_absolute_path(low_path)
                .expect("absolute path")
                .into(),
        ]
    );
}

#[test]
fn single_regular_layer_keeps_enterprise_managed_source() {
    let composed = compose_requirements_for_hostname(
        vec![layer(
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
fn regular_toml_merge_recurses_into_tables() {
    let composed = compose(vec![
        layer(
            "req_low",
            "Low",
            r#"
[features]
beta = false
shared = false

[apps.connector_1]
enabled = false

[apps.connector_1.tools.search]
approval_mode = "prompt"

[apps.connector_1.tools.list]
approval_mode = "prompt"
"#,
        ),
        layer(
            "req_high",
            "High",
            r#"
[features]
alpha = true
shared = true

[apps.connector_1]
enabled = true

[apps.connector_1.tools.search]
approval_mode = "approve"
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
    assert_eq!(
        composed.apps,
        Some(AppsRequirementsToml {
            apps: BTreeMap::from([(
                "connector_1".to_string(),
                AppRequirementToml {
                    enabled: Some(true),
                    tools: Some(AppToolsRequirementsToml {
                        tools: BTreeMap::from([
                            (
                                "list".to_string(),
                                AppToolRequirementToml {
                                    approval_mode: Some(AppToolApproval::Prompt),
                                },
                            ),
                            (
                                "search".to_string(),
                                AppToolRequirementToml {
                                    approval_mode: Some(AppToolApproval::Approve),
                                },
                            ),
                        ]),
                    }),
                },
            )]),
        })
    );
}

#[test]
fn merged_table_source_is_composite_in_priority_order() {
    let high_source = RequirementSource::EnterpriseManaged {
        id: "req_high".to_string(),
        name: "High".to_string(),
    };
    let low_source = RequirementSource::EnterpriseManaged {
        id: "req_low".to_string(),
        name: "Low".to_string(),
    };
    let composed = compose_requirements_for_hostname(
        vec![
            RequirementsLayer::from_toml(
                low_source.clone(),
                r#"
[features]
beta = true
"#,
            ),
            RequirementsLayer::from_toml(
                high_source.clone(),
                r#"
[features]
alpha = true
"#,
            ),
        ],
        /*hostname*/ None,
    )
    .expect("compose requirements")
    .expect("requirements present");

    assert_eq!(
        composed.feature_requirements.expect("features").source,
        RequirementSource::composite([high_source, low_source])
    );
}

#[test]
fn mcp_requirements_use_regular_toml_merge() {
    let composed = compose(vec![
        layer(
            "req_low",
            "Low",
            r#"
[mcp_servers.shared.identity]
command = "low-mcp"

[mcp_servers.low.identity]
url = "https://low.example.com/mcp"
"#,
        ),
        layer(
            "req_high",
            "High",
            r#"
[mcp_servers.shared.identity]
command = "high-mcp"
"#,
        ),
    ])
    .expect("compose requirements")
    .expect("requirements present");

    assert_eq!(
        composed.mcp_servers,
        Some(BTreeMap::from([
            (
                "low".to_string(),
                McpServerRequirement {
                    identity: McpServerIdentity::Url {
                        url: "https://low.example.com/mcp".to_string(),
                    },
                },
            ),
            (
                "shared".to_string(),
                McpServerRequirement {
                    identity: McpServerIdentity::Command {
                        command: "high-mcp".to_string(),
                    },
                },
            ),
        ]))
    );
}

#[test]
fn network_maps_use_regular_toml_merge() {
    let composed = compose(vec![
        layer(
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
        layer(
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
fn remote_sandbox_config_is_applied_per_layer() {
    let composed = compose_requirements_for_hostname(
        vec![
            layer(
                "req_low",
                "Low",
                r#"
allowed_sandbox_modes = ["read-only"]
"#,
            ),
            layer(
                "req_high",
                "High",
                r#"
[[remote_sandbox_config]]
hostname_patterns = ["build-*.example.com"]
allowed_sandbox_modes = ["workspace-write"]
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
fn unmatched_remote_sandbox_config_does_not_shadow_lower_layers() {
    let composed = compose_requirements_for_hostname(
        vec![
            layer(
                "req_low",
                "Low",
                r#"
allowed_sandbox_modes = ["read-only"]
"#,
            ),
            layer(
                "req_high",
                "High",
                r#"
[[remote_sandbox_config]]
hostname_patterns = ["mac-*.example.com"]
allowed_sandbox_modes = ["workspace-write"]
"#,
            ),
        ],
        Some("linux-01.example.com"),
    )
    .expect("compose requirements")
    .expect("requirements present")
    .into_toml();

    assert_eq!(
        composed.allowed_sandbox_modes,
        Some(vec![SandboxModeRequirement::ReadOnly])
    );
}

#[test]
fn rules_are_appended_in_priority_order() {
    let composed = compose(vec![
        layer(
            "req_low",
            "Low",
            r#"
[[rules.prefix_rules]]
pattern = [{ token = "npm" }]
decision = "prompt"
"#,
        ),
        layer(
            "req_high",
            "High",
            r#"
[[rules.prefix_rules]]
pattern = [{ token = "git" }]
decision = "forbidden"
"#,
        ),
    ])
    .expect("compose requirements")
    .expect("requirements present");

    assert_eq!(
        composed.rules,
        Some(RequirementsExecPolicyToml {
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
        })
    );
}

#[test]
fn hooks_append_groups_and_reject_conflicting_managed_dirs() {
    let composed = compose_with_hook_directory_field(
        vec![
            layer(
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
            layer(
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
        ],
        HookDirectoryField::ManagedDir,
    )
    .expect("compose requirements")
    .expect("requirements present");

    assert_eq!(
        composed.hooks,
        Some(ManagedHooksRequirementsToml {
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
        })
    );

    let err = compose_with_hook_directory_field(
        vec![
            layer(
                "req_low",
                "Low",
                r#"
[hooks]
managed_dir = "/managed/low"
"#,
            ),
            layer(
                "req_high",
                "High",
                r#"
[hooks]
managed_dir = "/managed/high"
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
            layer(
                "req_low",
                "Low",
                r#"
[hooks]
windows_managed_dir = 'C:\managed\low'
"#,
            ),
            layer(
                "req_high",
                "High",
                r#"
[hooks]
windows_managed_dir = 'C:\managed\high'
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
            layer(
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
            layer(
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
            layer(
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
            layer(
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
fn permissions_deny_read_unions_while_profiles_use_regular_toml_merge() {
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
        layer(
            "req_low",
            "Low",
            &format!(
                r#"
[permissions.filesystem]
deny_read = [{high_path:?}, {low_path:?}]

[permissions.managed-standard]
description = "Low profile"
extends = ":workspace"
"#
            ),
        ),
        layer(
            "req_high",
            "High",
            &format!(
                r#"
[permissions.filesystem]
deny_read = [{high_path:?}]

[permissions.managed-standard]
description = "High profile"
"#
            ),
        ),
    ])
    .expect("compose requirements")
    .expect("requirements present");

    let permissions = composed.permissions.expect("permissions");
    assert_eq!(
        permissions.filesystem,
        Some(FilesystemRequirementsToml {
            deny_read: Some(vec![
                AbsolutePathBuf::from_absolute_path(high_path)
                    .expect("absolute path")
                    .into(),
                AbsolutePathBuf::from_absolute_path(low_path)
                    .expect("absolute path")
                    .into(),
            ]),
        })
    );

    let profile = permissions
        .profiles
        .get("managed-standard")
        .expect("merged profile");
    assert_eq!(profile.description.as_deref(), Some("High profile"));
    assert_eq!(profile.extends.as_deref(), Some(":workspace"));
}

#[test]
fn deny_read_only_layers_do_not_leave_empty_permissions_tables() {
    let path = if cfg!(windows) {
        "C:\\secret"
    } else {
        "/secret"
    };
    let composed = compose(vec![layer(
        "req_high",
        "High",
        &format!(
            r#"
[permissions.filesystem]
deny_read = [{path:?}]
"#
        ),
    )])
    .expect("compose requirements")
    .expect("requirements present");

    assert_eq!(
        composed.permissions,
        Some(PermissionsRequirementsToml {
            filesystem: Some(FilesystemRequirementsToml {
                deny_read: Some(vec![
                    AbsolutePathBuf::from_absolute_path(path)
                        .expect("absolute path")
                        .into(),
                ]),
            }),
            profiles: BTreeMap::new(),
        })
    );
}

#[test]
fn parse_error_names_layer() {
    let err = compose(vec![layer(
        "req_bad",
        "Bad layer",
        "allowed_approval_policies = [1]",
    )])
    .expect_err("invalid layer should fail");

    assert!(err.to_string().contains("Bad layer (req_bad)"));
    assert!(err.to_string().contains("allowed_approval_policies"));
}
