use anyhow::Context;
use anyhow::Result;
use codex_config::CloudConfigBundleLoader;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::CodexThreadSettingsOverrides;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_target_windows;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::RwLock;
use tempfile::TempDir;

#[tokio::test]
async fn refreshed_cloud_bundle_updates_later_sessions() -> Result<()> {
    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    let initial_bundle = CloudConfigBundleFixture::enterprise_requirement(
        r#"allowed_approval_policies = ["never"]"#,
    )
    .add_enterprise_config(r#"developer_instructions = "initial managed instructions""#)
    .into_bundle();
    let latest = Arc::new(RwLock::new(initial_bundle));
    let getter_latest = Arc::clone(&latest);
    let loader = CloudConfigBundleLoader::from_getter(move || {
        let latest = Arc::clone(&getter_latest);
        async move { Ok(Some(latest.read().expect("bundle state lock").clone())) }
    });

    let mut initial_builder = test_codex()
        .with_home(Arc::clone(&home))
        .with_cloud_config_bundle(loader.clone());
    let initial = initial_builder.build_with_auto_env(&server).await?;
    assert_eq!(
        initial.session_configured.approval_policy,
        AskForApproval::Never
    );
    assert_eq!(
        initial
            .codex
            .config()
            .await
            .developer_instructions
            .as_deref(),
        Some("initial managed instructions")
    );

    *latest.write().expect("bundle state lock") = CloudConfigBundleFixture::enterprise_requirement(
        r#"allowed_approval_policies = ["on-request"]"#,
    )
    .add_enterprise_config(r#"developer_instructions = "refreshed managed instructions""#)
    .into_bundle();

    let mut refreshed_builder = test_codex()
        .with_home(home)
        .with_cloud_config_bundle(loader);
    let refreshed = refreshed_builder.build_with_auto_env(&server).await?;
    assert_eq!(
        refreshed.session_configured.approval_policy,
        AskForApproval::OnRequest
    );
    assert_eq!(
        refreshed
            .codex
            .config()
            .await
            .developer_instructions
            .as_deref(),
        Some("refreshed managed instructions")
    );

    Ok(())
}

#[tokio::test]
async fn managed_deny_read_requirements_follow_thread_permission_updates() -> Result<()> {
    skip_if_target_windows!(
        Ok(()),
        "Windows restricted-token sandbox cannot enforce deny-read policies"
    );

    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    let denied_root = home.path().join("managed-private");
    let nested_root = denied_root.join("nested");
    std::fs::create_dir_all(&nested_root)?;
    let nested_root = AbsolutePathBuf::from_absolute_path(nested_root)?;

    let mut builder = test_codex()
        .with_home(home)
        .with_cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(format!(
                "[permissions.filesystem]\ndeny_read = [{denied_root:?}]\n"
            )),
        )
        .with_config(|config| {
            config
                .permissions
                .set_permission_profile(PermissionProfile::workspace_write())
                .expect("safe workspace permissions should preserve managed denies");
        });
    let test = builder.build_with_auto_env(&server).await?;

    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            permission_profile: Some(PermissionProfile::read_only()),
            ..Default::default()
        },
    )
    .await?;
    let snapshot = test.codex.config_snapshot().await;
    assert!(
        !snapshot
            .permission_profile
            .file_system_sandbox_policy()
            .can_read_path_with_cwd(nested_root.as_path(), snapshot.cwd().as_path()),
        "a live permission change must preserve managed deny-read rules"
    );

    let conflicting_profile = PermissionProfile::workspace_write_with(
        std::slice::from_ref(&nested_root),
        NetworkSandboxPolicy::Restricted,
        /*exclude_tmpdir_env_var*/ false,
        /*exclude_slash_tmp*/ false,
    );
    let error = test
        .codex
        .preview_thread_settings_overrides(CodexThreadSettingsOverrides {
            permission_profile: Some(conflicting_profile),
            ..Default::default()
        })
        .await
        .err()
        .context("a concrete writable root must not override a managed deny")?;
    assert!(error.to_string().contains("permissions.filesystem"));

    Ok(())
}

#[tokio::test]
async fn managed_guardian_v1_requirements_disable_guardian_v2() -> Result<()> {
    let server = start_mock_server().await;

    for (requirements, guardian_v2_enabled) in [
        (r#"allowed_approvals_reviewers = ["auto_review"]"#, false),
        (
            r#"allowed_approvals_reviewers = ["guardian_subagent"]"#,
            false,
        ),
        (
            r#"allowed_approvals_reviewers = ["auto_review", "user"]"#,
            true,
        ),
        ("[features]\nguardian_approval = true\n", true),
        ("[features]\nauto_review = true\n", true),
    ] {
        let mut builder = test_codex()
            .with_pre_build_hook(|home| {
                std::fs::write(home.join("config.toml"), "[features]\nguardianv2 = true\n")
                    .expect("Guardian v2 configuration should be written");
            })
            .with_cloud_config_bundle(
                CloudConfigBundleFixture::loader_with_enterprise_requirement(requirements),
            );
        let test = builder.build_with_auto_env(&server).await?;

        assert!(test.config.features.enabled(Feature::GuardianApproval));
        assert_eq!(
            test.config.features.enabled(Feature::GuardianV2),
            guardian_v2_enabled,
            "{requirements}"
        );
    }

    Ok(())
}
