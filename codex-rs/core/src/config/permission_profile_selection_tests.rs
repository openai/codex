use crate::config::ResolvedPermissionProfileSelection;
use crate::config::resolve_permission_profile_selection;
use codex_config::ConfigRequirementsToml;
use codex_config::permissions_toml::PermissionsToml;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn resolves_managed_profile_without_compiling_executor_paths() -> anyhow::Result<()> {
    let configured_profiles = toml::from_str::<PermissionsToml>(
        r#"
[configured-windows]
extends = ":workspace"

[configured-windows.workspace_roots]
'C:\Users\agent\workspace' = true

[configured-windows.filesystem]
'C:\Users\agent\workspace' = "write"
"#,
    )?;
    let requirements = toml::from_str::<ConfigRequirementsToml>(
        r#"
default_permissions = "managed-windows"

[allowed_permission_profiles]
configured-windows = false
managed-windows = true

[permissions.managed-windows.workspace_roots]
'D:\Managed\workspace' = true

[permissions.managed-windows.filesystem]
'D:\Managed\workspace' = "read"
"#,
    )?;
    let expected_profiles = PermissionsToml {
        entries: BTreeMap::from([
            (
                "configured-windows".to_string(),
                configured_profiles.entries["configured-windows"].clone(),
            ),
            (
                "managed-windows".to_string(),
                requirements
                    .permissions
                    .as_ref()
                    .expect("managed profiles should deserialize")
                    .profiles["managed-windows"]
                    .clone(),
            ),
        ]),
    };

    let resolved = resolve_permission_profile_selection(
        Some(&configured_profiles),
        Some("configured-windows"),
        &requirements,
    )?;

    assert_eq!(
        resolved,
        ResolvedPermissionProfileSelection {
            profile_id: Some("managed-windows"),
            profiles: Some(expected_profiles),
        }
    );

    Ok(())
}

#[test]
fn rejects_undefined_allowlisted_permission_profile() -> anyhow::Result<()> {
    let requirements = toml::from_str::<ConfigRequirementsToml>(
        r#"
default_permissions = "missing-profile"

[allowed_permission_profiles]
missing-profile = true
"#,
    )?;

    let error = resolve_permission_profile_selection(
        /*configured_profiles*/ None,
        /*configured_default_profile_id*/ None,
        &requirements,
    )
    .expect_err("undefined allowlisted profiles should be rejected");

    assert_eq!(
        error.to_string(),
        "requirements.toml allowed_permission_profiles refers to undefined profile `missing-profile`"
    );

    Ok(())
}
