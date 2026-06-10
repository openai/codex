use super::super::RequirementsLayerEntry;
use super::super::hooks::HookDirectoryField;
use super::RequirementsCompositionError;
use super::compose_requirements_for_hostname;
use super::compose_requirements_for_hostname_and_hook_directory;
use crate::ConfigRequirementsToml;
use crate::ConfigRequirementsWithSources;
use crate::RequirementSource;
use crate::Sourced;
use codex_protocol::protocol::AskForApproval;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

fn layer(id: &str, name: &str, contents: &str) -> RequirementsLayerEntry {
    RequirementsLayerEntry::from_toml(
        RequirementSource::EnterpriseManaged {
            id: id.to_string(),
            name: name.to_string(),
        },
        contents,
    )
}

fn compose(
    layers: Vec<RequirementsLayerEntry>,
) -> Result<Option<ConfigRequirementsToml>, RequirementsCompositionError> {
    Ok(
        compose_requirements_for_hostname(layers, /*hostname*/ None)?
            .map(ConfigRequirementsWithSources::into_toml),
    )
}

fn compose_with_hook_directory_field(
    layers: Vec<RequirementsLayerEntry>,
    hook_directory_field: HookDirectoryField,
) -> Result<Option<ConfigRequirementsToml>, RequirementsCompositionError> {
    Ok(compose_requirements_for_hostname_and_hook_directory(
        layers,
        /*hostname*/ None,
        hook_directory_field,
    )?
    .map(ConfigRequirementsWithSources::into_toml))
}

fn expected_requirements(contents: impl AsRef<str>) -> ConfigRequirementsToml {
    toml::from_str(contents.as_ref()).expect("parse expected requirements TOML")
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
default_permissions = ":workspace"
allow_guardian_fail_open = true

[allowed_permission_profiles]
":read-only" = true
":workspace" = true
"#,
        ),
        layer(
            "req_high",
            "High",
            r#"
allowed_approval_policies = ["never"]
allowed_sandbox_modes = ["read-only"]
default_permissions = ":read-only"
allow_guardian_fail_open = false

[allowed_permission_profiles]
":danger-full-access" = false
":workspace" = false
"#,
        ),
    ])
    .expect("compose requirements")
    .expect("requirements present");

    assert_eq!(
        composed,
        expected_requirements(
            r#"
allowed_approval_policies = ["never"]
allowed_sandbox_modes = ["read-only"]
default_permissions = ":read-only"
allow_guardian_fail_open = false

[allowed_permission_profiles]
":danger-full-access" = false
":read-only" = true
":workspace" = false
"#
        )
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
            RequirementsLayerEntry::from_toml(
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
            RequirementsLayerEntry::from_toml(
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
        composed.clone().into_toml(),
        expected_requirements(format!(
            r#"
allowed_approval_policies = ["never"]

[features]
shared = true
system = true

[[rules.prefix_rules]]
pattern = [{{ token = "git" }}]
decision = "forbidden"

[[rules.prefix_rules]]
pattern = [{{ token = "npm" }}]
decision = "prompt"

[permissions.filesystem]
deny_read = [{high_path:?}, {low_path:?}]
"#
        ))
    );
    assert_eq!(
        composed.allowed_approval_policies,
        Some(Sourced::new(vec![AskForApproval::Never], mdm_source))
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
        composed,
        expected_requirements(
            r#"
[features]
alpha = true
beta = false
shared = true

[apps.connector_1]
enabled = true

[apps.connector_1.tools.list]
approval_mode = "prompt"

[apps.connector_1.tools.search]
approval_mode = "approve"
"#
        )
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
            RequirementsLayerEntry::from_toml(
                low_source.clone(),
                r#"
[features]
beta = true
"#,
            ),
            RequirementsLayerEntry::from_toml(
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
        composed.feature_requirements.expect("features"),
        Sourced::new(
            crate::FeatureRequirementsToml {
                entries: BTreeMap::from([("alpha".to_string(), true), ("beta".to_string(), true),]),
            },
            RequirementSource::composite([high_source, low_source]),
        )
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
        composed,
        expected_requirements(
            r#"
[mcp_servers.low.identity]
url = "https://low.example.com/mcp"

[mcp_servers.shared.identity]
command = "high-mcp"
"#
        )
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
"/tmp/shared.sock" = "deny"
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
"/tmp/admin.sock" = "deny"
"#,
        ),
    ])
    .expect("compose requirements")
    .expect("requirements present");

    assert_eq!(
        composed,
        expected_requirements(
            r#"
[experimental_network.domains]
"example.com" = "allow"
"high.example.com" = "allow"
"internal.example.com" = "deny"
"low.example.com" = "deny"

[experimental_network.unix_sockets]
"/tmp/admin.sock" = "deny"
"/tmp/high.sock" = "allow"
"/tmp/low.sock" = "allow"
"/tmp/shared.sock" = "allow"
"#
        )
    );
}

#[test]
fn windows_requirements_use_regular_toml_merge() {
    let composed = compose(vec![
        layer(
            "req_low",
            "Low",
            r#"
[windows]
allowed_sandbox_implementations = ["unelevated"]
"#,
        ),
        layer(
            "req_high",
            "High",
            r#"
[windows]
allowed_sandbox_implementations = ["elevated"]
"#,
        ),
    ])
    .expect("compose requirements")
    .expect("requirements present");

    assert_eq!(
        composed,
        expected_requirements(
            r#"
[windows]
allowed_sandbox_implementations = ["elevated"]
"#
        )
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
        composed,
        expected_requirements(
            r#"
allowed_sandbox_modes = ["workspace-write"]
"#
        )
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
        composed,
        expected_requirements(
            r#"
allowed_sandbox_modes = ["read-only"]
"#
        )
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
        composed,
        expected_requirements(
            r#"
[[rules.prefix_rules]]
pattern = [{ token = "git" }]
decision = "forbidden"

[[rules.prefix_rules]]
pattern = [{ token = "npm" }]
decision = "prompt"
"#
        )
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
        composed,
        expected_requirements(
            r#"
[hooks]
managed_dir = "/managed/hooks"

[[hooks.PreToolUse]]
matcher = "Edit"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "high"

[[hooks.PreToolUse]]
matcher = "Bash"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "low"
"#
        )
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

    assert_eq!(
        composed,
        expected_requirements(
            r#"
[hooks]
managed_dir = "/managed/hooks"
windows_managed_dir = 'C:\managed\high'

[[hooks.PreToolUse]]
matcher = "Edit"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "high"

[[hooks.PreToolUse]]
matcher = "Bash"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "low"
"#
        )
    );

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

    assert_eq!(
        composed,
        expected_requirements(
            r#"
[hooks]
managed_dir = "/managed/high"
windows_managed_dir = 'C:\managed\hooks'

[[hooks.PreToolUse]]
matcher = "Edit"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "high"

[[hooks.PreToolUse]]
matcher = "Bash"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "low"
"#
        )
    );
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

    assert_eq!(
        composed,
        expected_requirements(format!(
            r#"
[permissions.filesystem]
deny_read = [{high_path:?}, {low_path:?}]

[permissions.managed-standard]
description = "High profile"
extends = ":workspace"
"#
        ))
    );
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
        composed,
        expected_requirements(format!(
            r#"
[permissions.filesystem]
deny_read = [{path:?}]
"#
        ))
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
