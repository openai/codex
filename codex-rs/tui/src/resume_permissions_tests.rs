use super::ResumePermissions;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn only_direct_launch_choices_override_saved_permissions() -> color_eyre::Result<()> {
    let home = tempfile::tempdir()?;
    let profile = home.path().join("work.config.toml");
    std::fs::write(
        &profile,
        "default_permissions = \":read-only\"\napproval_policy = \"never\"\napprovals_reviewer = \"auto_review\"\n",
    )?;
    let builder = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(codex_config::LoaderOverrides {
            user_config_path: Some(AbsolutePathBuf::try_from(profile)?),
            user_config_profile: Some("work".parse()?),
            ..codex_config::LoaderOverrides::without_managed_config_for_tests()
        });
    let config = builder.clone().build().await?;
    assert_eq!(
        ResumePermissions::from_overrides(&config, &ConfigOverrides::default()),
        ResumePermissions::default(),
    );
    let direct_rules = builder
        .clone()
        .cli_overrides(vec![(
            "permissions.safe.extends".into(),
            ":read-only".into(),
        )])
        .build()
        .await?;
    assert_eq!(
        ResumePermissions::from_overrides(&direct_rules, &ConfigOverrides::default()),
        ResumePermissions {
            profile: true,
            ..Default::default()
        },
    );
    let config = builder
        .cli_overrides(vec![("approval_policy".into(), "on-request".into())])
        .build()
        .await?;
    assert_eq!(
        ResumePermissions::from_overrides(
            &config,
            &ConfigOverrides {
                default_permissions: Some(":read-only".into()),
                cwd: Some(home.path().to_path_buf()),
                ..Default::default()
            },
        ),
        ResumePermissions {
            approval_policy: true,
            profile: true,
            workspace_roots: true,
            ..Default::default()
        },
    );
    Ok(())
}
