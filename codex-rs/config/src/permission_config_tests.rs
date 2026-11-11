#![cfg(test)]

use super::*;
use crate::ConfigLayerEntry;
use crate::ConfigRequirements;
use crate::ConfigRequirementsToml;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn permission_config_merges_legacy_fields_without_changing_sources() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let root = crate::AbsolutePathBuf::from_absolute_path(home.path())?;
    let cloud = ConfigLayerEntry::new(
        ConfigLayerSource::EnterpriseManaged {
            id: "policy".into(),
            name: "policy".into(),
        },
        toml::from_str(&format!(
            r#"
default_permissions = ":read-only"
[sandbox_workspace_write]
writable_roots = [{}]
exclude_tmpdir_env_var = true
"#,
            serde_json::to_string(&root)?
        ))?,
    );
    let user = ConfigLayerEntry::new(
        ConfigLayerSource::User {
            file: root.join("config.toml"),
            profile: None,
        },
        toml::from_str("[sandbox_workspace_write]\nnetwork_access = true")?,
    );
    let managed = ConfigLayerEntry::new(
        ConfigLayerSource::LegacyManagedConfigTomlFromMdm,
        toml::from_str("sandbox_mode = 'workspace-write'")?,
    );
    let cloud_metadata = cloud.metadata();
    let user_metadata = user.metadata();
    let managed_metadata = managed.metadata();
    let stack = ConfigLayerStack::new(
        vec![cloud, user, managed],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )?;
    let original = stack.effective_config();
    let config = ConfigToml::try_from(original.clone())?;
    let Some(PermissionConfig::Legacy {
        mode,
        workspace_write,
    }) = resolve_permission_config(&stack, &config, /*sandbox_mode_override*/ None)
    else {
        anyhow::bail!("MDM legacy selector should override the cloud profile");
    };
    assert_eq!(
        translate_legacy_permissions(
            mode,
            workspace_write,
            Platform::Linux,
            WindowsSandboxLevel::Disabled
        ),
        PermissionProfile::workspace_write_with(
            &[root],
            NetworkSandboxPolicy::Enabled,
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ false
        ),
    );
    let origins = stack.origins();
    assert_eq!(
        (
            origins.get("sandbox_mode"),
            origins.get("sandbox_workspace_write.writable_roots.0"),
            origins.get("sandbox_workspace_write.network_access")
        ),
        (
            Some(&managed_metadata),
            Some(&cloud_metadata),
            Some(&user_metadata)
        ),
    );
    assert_eq!(stack.effective_config(), original);
    Ok(())
}

#[test]
fn permission_config_preserves_mixed_selector_precedence() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    for (user, session, managed, expected) in [
        (
            "default_permissions = ':workspace'",
            "",
            "sandbox_mode = 'read-only'",
            PermissionConfig::Legacy {
                mode: SandboxMode::ReadOnly,
                workspace_write: None,
            },
        ),
        (
            "sandbox_mode = 'read-only'",
            "",
            "default_permissions = ':workspace'",
            PermissionConfig::Named { id: ":workspace" },
        ),
        (
            "",
            "default_permissions = ':workspace'",
            "sandbox_mode = 'read-only'",
            PermissionConfig::Named { id: ":workspace" },
        ),
        (
            "sandbox_mode = 'read-only'\ndefault_permissions = ':workspace'",
            "",
            "",
            PermissionConfig::Named { id: ":workspace" },
        ),
    ] {
        let stack = ConfigLayerStack::new(
            vec![
                ConfigLayerEntry::new(
                    ConfigLayerSource::User {
                        file: crate::AbsolutePathBuf::from_absolute_path(
                            home.path().join("config.toml"),
                        )?,
                        profile: None,
                    },
                    toml::from_str(user)?,
                ),
                ConfigLayerEntry::new(ConfigLayerSource::SessionFlags, toml::from_str(session)?),
                ConfigLayerEntry::new(
                    ConfigLayerSource::LegacyManagedConfigTomlFromMdm,
                    toml::from_str(managed)?,
                ),
            ],
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )?;
        let config = ConfigToml::try_from(stack.effective_config())?;
        assert_eq!(
            resolve_permission_config(&stack, &config, /*sandbox_mode_override*/ None),
            Some(expected)
        );
        assert_eq!(
            resolve_permission_config(&stack, &config, Some(SandboxMode::ReadOnly)),
            Some(PermissionConfig::Legacy {
                mode: SandboxMode::ReadOnly,
                workspace_write: None
            }),
        );
    }
    Ok(())
}

#[test]
fn permission_config_translation_uses_executor_sandbox_availability() {
    assert_eq!(
        translate_legacy_permissions(
            SandboxMode::WorkspaceWrite,
            /*workspace_write*/ None,
            Platform::Windows,
            WindowsSandboxLevel::Disabled
        ),
        PermissionProfile::read_only(),
    );
}
