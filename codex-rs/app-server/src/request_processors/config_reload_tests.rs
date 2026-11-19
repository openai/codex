//! Regressions for reload entry points composing the final thread configuration.

use super::config_processor::reload_user_config;
use crate::config_manager::ConfigManager;
use crate::mcp_refresh::reload_mcp_config;
use codex_config::types::AuthKeyringBackendKind;
use codex_core::config::ConfigOverrides;
use codex_exec_server::EnvironmentManager;
use codex_login::CodexAuth;
use codex_protocol::config_types::ApprovalsReviewer::AutoReview;
use codex_protocol::config_types::ApprovalsReviewer::User;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn reloads_accept_a_provider_supplied_by_thread_overrides() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let manager = ConfigManager::without_managed_config_for_tests(home.path().to_path_buf());
    let initial = manager
        .load_for_cwd(
            Some(HashMap::from([
                ("model_provider".to_string(), json!("thread-provider")),
                (
                    "model_providers.thread-provider".to_string(),
                    json!({"name": "Thread provider", "base_url": "https://provider.example/v1", "wire_api": "responses"}),
                ),
            ])),
            ConfigOverrides::default(),
            Some(home.path().to_path_buf()),
        )
        .await?;
    let threads = Arc::new(
        codex_core::test_support::thread_manager_with_models_provider_and_home(
            CodexAuth::from_api_key("dummy"),
            initial.model_provider.clone(),
            initial.codex_home.to_path_buf(),
            Arc::new(EnvironmentManager::default_for_tests()),
        ),
    );
    let thread = threads
        .start_thread(codex_core::StartThreadOptions::new(initial))
        .await?
        .thread;

    for enabled in [true, false] {
        std::fs::write(
            home.path().join(codex_config::CONFIG_TOML_FILE),
            format!(
                "model_provider = 'missing-provider'\n[features]\nsecret_auth_storage = {enabled}\n"
            ),
        )?;
        if enabled {
            reload_mcp_config(&threads, &manager).await?;
        } else {
            reload_user_config(&manager, &threads).await;
        }
        let config = thread.config().await;
        assert_eq!(config.model_provider_id, "thread-provider");
        assert_eq!(
            config.auth_keyring_backend_kind(),
            if enabled {
                AuthKeyringBackendKind::Secrets
            } else {
                AuthKeyringBackendKind::Direct
            }
        );
    }
    Ok(())
}

#[tokio::test]
async fn user_reload_refreshes_independent_requirements_without_server_changes()
-> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let requirements_path = home.path().join("requirements.toml");
    std::fs::write(
        &requirements_path,
        "allowed_approvals_reviewers = ['auto_review']\nguardian_policy_config = 'first policy'\nguardian_extra_policy = 'first extra'\n",
    )?;
    std::fs::write(
        home.path().join(codex_config::CONFIG_TOML_FILE),
        "approvals_reviewer = 'user'\n[features]\nenable_mcp_apps = true\n",
    )?;
    let mut overrides = codex_config::LoaderOverrides::without_managed_config_for_tests();
    overrides.ignore_managed_requirements = false;
    overrides.system_requirements_path = Some(requirements_path.clone());
    let manager = ConfigManager::new(
        home.path().to_path_buf(),
        Vec::new(),
        overrides,
        /*strict_config*/ false,
        codex_config::CloudConfigBundleLoader::default(),
        codex_arg0::Arg0DispatchPaths::default(),
        Arc::new(codex_config::NoopThreadConfigLoader),
    );
    let initial = manager
        .load_latest_config(Some(home.path().to_path_buf()))
        .await?;
    let threads = Arc::new(
        codex_core::test_support::thread_manager_with_models_provider_and_home(
            CodexAuth::from_api_key("dummy"),
            initial.model_provider.clone(),
            initial.codex_home.to_path_buf(),
            Arc::new(EnvironmentManager::default_for_tests()),
        )
        .with_guardian_policy_loader(Arc::new(manager.clone())),
    );
    assert!(
        initial
            .features
            .enabled(codex_features::Feature::EnableMcpApps)
    );
    let original_servers = initial.mcp_servers.get().clone();
    let thread = threads
        .start_thread(codex_core::StartThreadOptions::new(initial))
        .await?
        .thread;
    assert_eq!(
        thread.config_snapshot().await.approvals_reviewer,
        AutoReview
    );
    std::fs::write(
        &requirements_path,
        "guardian_policy_config = 'first policy'\nguardian_extra_policy = 'first extra'\n[features]\nenable_mcp_apps = false\n[plugins.example.mcp_servers]\n",
    )?;
    reload_user_config(&manager, &threads).await;
    let config = thread.config().await;
    assert_eq!(config.approvals_reviewer, User);
    assert_eq!(config.mcp_servers.get(), &original_servers);
    assert!(
        !config
            .features
            .enabled(codex_features::Feature::EnableMcpApps)
    );
    assert!(
        config
            .config_layer_stack
            .requirements()
            .plugins
            .as_ref()
            .unwrap()
            .value
            .get("example")
            .unwrap()
            .mcp_servers
            .as_ref()
            .unwrap()
            .is_empty()
    );
    assert!(
        config
            .config_layer_stack
            .requirements()
            .feature_requirements
            .is_some()
    );
    for requirements in [
        "guardian_policy_config = 'replacement policy'\nguardian_extra_policy = 'first extra'",
        "guardian_policy_config = 'replacement policy'\nguardian_extra_policy = 'replacement extra'",
        "",
    ] {
        let requirements = format!(
            "{requirements}\n[features]\nenable_mcp_apps = false\n[plugins.example.mcp_servers]\n"
        );
        std::fs::write(&requirements_path, &requirements)?;
        let expected: codex_config::ConfigRequirementsToml = toml::from_str(&requirements)?;
        reload_user_config(&manager, &threads).await;
        let config = thread.config().await;
        assert_eq!(config.mcp_servers.get(), &original_servers);
        assert_eq!(
            (
                config.guardian_policy_config.as_deref(),
                config.guardian_extra_policy.as_deref()
            ),
            (
                expected.guardian_policy_config.as_deref(),
                expected.guardian_extra_policy.as_deref()
            )
        );
    }
    // Guardian still applies changed requirements when user config cannot load.
    let model = thread.config_snapshot().await.model;
    std::fs::write(
        &requirements_path,
        format!(
            "allowed_approvals_reviewers = ['auto_review']\n[auto_review]\nrequired_on_models = ['{model}']"
        ),
    )?;
    std::fs::write(
        home.path().join(codex_config::CONFIG_TOML_FILE),
        "model_provider = 'missing-provider'",
    )?;
    reload_user_config(&manager, &threads).await;
    assert_eq!(
        thread.config_snapshot().await.approvals_reviewer,
        AutoReview
    );
    let config = thread.config().await;
    assert!(
        config
            .config_layer_stack
            .requirements()
            .approvals_reviewer
            .can_set(&AutoReview)
            .is_ok()
    );
    let restored = thread.restorable_thread_settings().await;
    thread.restore_thread_settings(restored).await?;
    // An explicit user reload resolves new local fallbacks on its first invocation.
    std::fs::write(&requirements_path, "")?;
    std::fs::write(
        home.path().join(codex_config::CONFIG_TOML_FILE),
        "[auto_review]\npolicy = 'new user fallback'",
    )?;
    reload_user_config(&manager, &threads).await;
    let config = thread.config().await;
    assert_eq!(
        (
            config.guardian_policy_config.as_deref(),
            config.guardian_extra_policy.as_deref(),
            config.approvals_reviewer
        ),
        (Some("new user fallback"), None, User)
    );
    thread
        .restore_thread_settings(codex_core::CodexThreadSettingsOverrides {
            approvals_reviewer: Some(AutoReview),
            ..Default::default()
        })
        .await?;
    reload_user_config(&manager, &threads).await;
    assert_eq!(
        thread.config_snapshot().await.approvals_reviewer,
        AutoReview
    );
    Ok(())
}

#[test_case::test_case(codex_config::CloudConfigBundleBindingStatus::Stale)]
#[test_case::test_case(codex_config::CloudConfigBundleBindingStatus::Suspended)]
#[tokio::test]
async fn mcp_refresh_rejects_superseded_app_server_policy(
    status: codex_config::CloudConfigBundleBindingStatus,
) -> anyhow::Result<()> {
    use codex_config::CloudConfigBundle;
    use codex_config::CloudConfigBundleLoader;
    use codex_config::CloudConfigBundlePolicy;
    use codex_config::CloudConfigBundleSnapshot;
    use codex_core::ConfigRefreshOutcome;

    let home = tempfile::tempdir()?;
    let policy = CloudConfigBundlePolicy::default();
    let mut snapshot = CloudConfigBundleSnapshot {
        config_server: None,
        bundle: Ok(None),
        binding: None,
    };
    policy.publish_snapshot(&mut snapshot);
    let loader =
        CloudConfigBundleLoader::default().with_ema_policy_snapshots(policy.clone(), move || {
            let snapshot = snapshot.clone();
            async move { snapshot }
        });
    let manager = ConfigManager::new(
        home.path().to_path_buf(),
        Vec::new(),
        codex_config::LoaderOverrides::without_managed_config_for_tests(),
        /*strict_config*/ false,
        loader,
        codex_arg0::Arg0DispatchPaths::default(),
        Arc::new(codex_config::NoopThreadConfigLoader),
    );
    let initial = manager
        .load_latest_config(Some(home.path().to_path_buf()))
        .await?;
    let threads = codex_core::test_support::thread_manager_with_models_provider_and_home(
        CodexAuth::from_api_key("dummy"),
        initial.model_provider.clone(),
        initial.codex_home.to_path_buf(),
        Arc::new(EnvironmentManager::default_for_tests()),
    );
    let thread = threads
        .start_thread(codex_core::StartThreadOptions::new(initial))
        .await?
        .thread;
    let current = thread.config().await;
    let next = manager
        .load_latest_config_with_session_layers(&current.config_layer_stack, &current.cwd)
        .await?;
    let mut replacement = CloudConfigBundle::default();
    replacement
        .config_toml
        .enterprise_managed
        .push(codex_config::CloudConfigFragment {
            id: "replacement".into(),
            name: "replacement".into(),
            contents: "model = 'replacement-model'".into(),
        });
    policy.observe_remote_bundle(&replacement);
    if status == codex_config::CloudConfigBundleBindingStatus::Stale {
        policy.publish_snapshot(&mut CloudConfigBundleSnapshot {
            config_server: None,
            bundle: Ok(Some(replacement)),
            binding: None,
        });
    }
    assert_eq!(
        next.config_layer_stack
            .cloud_config_binding()
            .expect("app-server must retain the policy binding")
            .read()
            .status,
        status,
    );
    let outcome = thread.refresh_mcp_config(Arc::clone(&current), next).await;
    let expected = match status {
        codex_config::CloudConfigBundleBindingStatus::Stale => ConfigRefreshOutcome::Stale,
        codex_config::CloudConfigBundleBindingStatus::Suspended => ConfigRefreshOutcome::Rejected,
        codex_config::CloudConfigBundleBindingStatus::Current => unreachable!(),
    };
    assert_eq!(outcome, expected);
    if status == codex_config::CloudConfigBundleBindingStatus::Stale {
        assert!(Arc::ptr_eq(&current, &thread.config().await));
    } else {
        assert!(
            thread
                .config()
                .await
                .config_layer_stack
                .cloud_config_binding()
                .is_none()
        );
    }
    Ok(())
}
