//! Runtime-refresh regressions; pure authority resolution is tested with Config.

use super::*;
use crate::StartThreadOptions;
use crate::ThreadManager;
use codex_config::loader::load_guardian_policy_snapshot;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_exec_server::LOCAL_FS;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn guardian_refresh_updates_idle_thread_while_another_thread_is_busy() -> anyhow::Result<()> {
    struct TestPolicyLoader(AbsolutePathBuf);
    impl crate::GuardianPolicyLoader for TestPolicyLoader {
        fn load_guardian_policy_snapshot(
            &self,
            revision: u64,
        ) -> BoxFuture<'_, std::io::Result<codex_config::GuardianPolicySnapshot>> {
            Box::pin(async move {
                load_guardian_policy_snapshot(
                    LOCAL_FS.as_ref(),
                    self.0.as_path(),
                    &LoaderOverrides::without_managed_config_for_tests(),
                    &codex_config::CloudConfigBundleLoader::default(),
                    revision,
                )
                .await
            })
        }
    }

    const UPDATE_TIMEOUT: Duration = Duration::from_secs(5);
    let home = tempfile::tempdir()?;
    let mut config = crate::config::test_config().await;
    config.codex_home = home.path().abs();
    config.cwd = config.codex_home.clone();
    let manager = ThreadManager::with_models_provider_and_home_for_tests(
        CodexAuth::from_api_key("dummy"),
        config.model_provider.clone(),
        config.codex_home.to_path_buf(),
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
    )
    .with_guardian_policy_loader(Arc::new(TestPolicyLoader(config.codex_home.clone())));

    manager
        .start_thread(StartThreadOptions::new(config.clone()))
        .await?;
    manager
        .start_thread(StartThreadOptions::new(config))
        .await?;
    // The manager lists threads in the registry's iteration order. Lock the first one.
    let threads = manager.list_thread_ids().await;
    let [busy, idle] = threads.as_slice() else {
        panic!("expected two live threads");
    };
    let busy = manager.get_thread(*busy).await?;
    let idle = manager.get_thread(*idle).await?;
    let startup_revision = idle.config().await.guardian_policy_revision;
    let (locked, state_locked) = tokio::sync::oneshot::channel();
    let (release_state, release) = tokio::sync::oneshot::channel();
    let held_state = tokio::task::spawn_blocking(move || {
        let _state = busy.session.state.blocking_lock();
        let _ = locked.send(());
        let _ = release.blocking_recv();
    });
    state_locked.await?;

    let (refresh, idle_update) = tokio::join!(manager.refresh_guardian_policy(), async {
        let result = tokio::time::timeout(UPDATE_TIMEOUT, async {
            while idle.config().await.guardian_policy_revision <= startup_revision {
                tokio::task::yield_now().await;
            }
        })
        .await;
        let _ = release_state.send(());
        result
    });
    held_state.await?;
    idle_update.expect("the idle thread should update while the first thread holds its lock");
    refresh?;
    manager.shutdown_all_threads_bounded(UPDATE_TIMEOUT).await;
    Ok(())
}

#[tokio::test]
async fn mcp_reload_survives_guardian_only_publications() {
    let (session, _) = make_session_and_context().await;
    let before = session.get_config().await;
    let mut incoming = before.as_ref().clone();
    incoming.config_layer_stack = incoming
        .config_layer_stack
        .with_session_plugins(toml::from_str("[_default]\nenabled = false").unwrap())
        .unwrap();
    let first_revision = before.guardian_policy_revision + 1;
    let latest_revision = first_revision + 1;
    for (revision, policy) in [
        (first_revision, "first policy"),
        (latest_revision, "latest policy"),
    ] {
        let loader = CloudConfigBundleFixture::loader_with_enterprise_requirement(format!(
            "guardian_policy_config = {policy:?}"
        ));
        let snapshot = load_guardian_policy_snapshot(
            LOCAL_FS.as_ref(),
            before.codex_home.as_path(),
            &LoaderOverrides::without_managed_config_for_tests(),
            &loader,
            revision,
        )
        .await
        .unwrap();
        assert_eq!(
            session.refresh_guardian_policy(&snapshot).await,
            crate::ConfigRefreshOutcome::Published
        );
    }

    assert_eq!(
        session.refresh_mcp_config(before, incoming).await,
        crate::ConfigRefreshOutcome::Published
    );
    let published = session.get_config().await;
    assert!(!published.plugins.allows_plugin("browser"));
    assert_eq!(published.guardian_policy_revision, latest_revision);
    assert_eq!(
        published.guardian_policy_config.as_deref(),
        Some("latest policy")
    );
}

async fn runtime_servers(session: &Session) -> HashMap<String, McpServerConfig> {
    let config = session.get_config().await;
    codex_mcp::configured_mcp_servers(
        &config
            .to_mcp_config(session.services.plugins_manager.as_ref())
            .await,
    )
}

#[test_case::test_case(true; "host enabled")]
#[test_case::test_case(false; "host disabled")]
#[tokio::test]
async fn file_reload_restores_host_feature_defaults_after_override_removal(host_default: bool) {
    let (session, _) = make_session_and_context().await;
    let home = session.codex_home().await;
    let file = home.join(CONFIG_TOML_FILE);
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(&file, "").unwrap();
    let mut initial = ConfigBuilder::without_managed_config_for_tests()
        .codex_home(home.to_path_buf())
        .build()
        .await
        .unwrap();
    let features = [
        Feature::SecretAuthStorage,
        Feature::McpOAuthRefreshCoordination,
    ];
    initial.runtime_feature_defaults = features
        .into_iter()
        .map(|feature| (feature, host_default))
        .collect();
    for feature in features {
        initial.features.set_enabled(feature, host_default).unwrap();
    }
    session
        .state
        .lock()
        .await
        .session_configuration
        .original_config_do_not_use = Arc::new(initial);

    for override_value in [None, Some(true), Some(false), None] {
        let contents = override_value.map_or_else(String::new, |enabled| {
            format!("[features]\nsecret_auth_storage = {enabled}\nmcp_oauth_refresh_coordination = {enabled}\n")
        });
        std::fs::write(&file, contents).unwrap();
        session.reload_user_config_layer().await;
        let config = session.get_config().await;
        assert_eq!(
            features.map(|feature| config.features.enabled(feature)),
            [override_value.unwrap_or(host_default); 2]
        );
    }
}

#[test_case::test_case("config.toml"; "user config")]
#[test_case::test_case("profiles/work.toml"; "profile config")]
#[tokio::test]
async fn reload_user_config_layer_resolves_paths_without_resetting_runtime_settings(
    config_file: &str,
) {
    let (session, _) = make_session_and_context().await;
    let home = session.codex_home().await;
    let file = home.join(config_file);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let ordinary =
        "[mcp_servers.ordinary]\nurl = 'https://ordinary.example/mcp'\nenabled = false\n";
    std::fs::write(
        &file,
        format!("model = 'original-model'\nnotify = ['original-notify']\n{ordinary}"),
    )
    .unwrap();
    let mut initial = ConfigBuilder::without_managed_config_for_tests()
        .codex_home(home.to_path_buf())
        .loader_overrides(LoaderOverrides {
            user_config_path: Some(file.clone()),
            user_config_profile: (config_file != CONFIG_TOML_FILE).then(|| "work".parse().unwrap()),
            ..LoaderOverrides::without_managed_config_for_tests()
        })
        .build()
        .await
        .unwrap();
    let mut servers = initial.mcp_servers.get().clone();
    let McpServerTransportConfig::StreamableHttp { http_headers, .. } =
        &mut servers.get_mut("ordinary").unwrap().transport
    else {
        unreachable!()
    };
    *http_headers = Some(HashMap::from([(
        "Authorization".into(),
        "Bearer synthetic-host".into(),
    )]));
    initial.mcp_servers.set(servers).unwrap();
    session
        .state
        .lock()
        .await
        .session_configuration
        .original_config_do_not_use = Arc::new(initial.clone());
    let original_servers = runtime_servers(&session).await;
    std::fs::write(
        &file,
        format!(
            "model = 'new-model'\nnotify = ['new-notify']\nlog_dir = 'relative-logs'\n{ordinary}"
        ),
    )
    .unwrap();

    session.reload_user_config_layer().await;

    let config = session.get_config().await;
    assert_eq!(
        config.config_layer_stack.effective_config()["log_dir"].as_str(),
        file.parent()
            .unwrap()
            .join("relative-logs")
            .as_path()
            .to_str()
    );
    assert_eq!(config.log_dir, initial.log_dir);
    assert_eq!(config.model, initial.model);
    assert_eq!(config.notify, initial.notify);
    assert_eq!(runtime_servers(&session).await, original_servers);
}

#[tokio::test]
async fn hook_refresh_retries_current_plugin_policy_after_owner_replacement() -> anyhow::Result<()>
{
    let (session, _) = make_session_and_context().await;
    let home = session.codex_home().await;
    let file = home.join(CONFIG_TOML_FILE);
    let source = tempfile::tempdir()?;
    std::fs::create_dir_all(source.path().join(".codex-plugin"))?;
    std::fs::write(
        source.path().join(".codex-plugin/plugin.json"),
        r#"{"name":"sample","hooks":{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"echo sample"}]}]}}}"#,
    )?;
    codex_core_plugins::store::PluginStore::new(home.to_path_buf()).install(
        AbsolutePathBuf::try_from(source.path().to_path_buf())?,
        codex_plugin::PluginId::parse("sample@test")?,
    )?;
    let user_config = "[features]\nplugins = true\nhooks = true\n[plugins.\"sample@test\"]\n";
    std::fs::write(&file, user_config)?;
    let mut initial = load_latest_config_for_session(&session).await;
    initial.bypass_hook_trust = true;
    let initial = Arc::new(initial);
    session
        .state
        .lock()
        .await
        .session_configuration
        .original_config_do_not_use = Arc::clone(&initial);
    session.refresh_hooks(Arc::clone(&initial)).await;
    let request = codex_hooks::SessionStartRequest {
        session_id: session.thread_id,
        cwd: initial.cwd.clone(),
        transcript_path: None,
        model: "gpt-5.2".to_string(),
        permission_mode: "default".to_string(),
        target: codex_hooks::StartHookTarget::SessionStart {
            source: codex_hooks::SessionStartSource::Startup,
        },
    };
    assert_eq!(session.hooks().preview_session_start(&request).len(), 1);

    let mut incoming = initial.as_ref().clone();
    incoming.config_layer_stack = initial.config_layer_stack.with_user_config(
        &file,
        toml::from_str(&format!(
            "{user_config}[plugins._default]\nenabled = false\n"
        ))?,
    )?;
    let denied = Arc::new(initial.resolve_runtime_refresh(&incoming, RuntimeConfigRefresh::User)?);
    // A concurrent refresh publishes its policy before its hook rebuild completes.
    session
        .state
        .lock()
        .await
        .session_configuration
        .original_config_do_not_use = Arc::clone(&denied);
    assert_eq!(session.hooks().preview_session_start(&request).len(), 1);

    session.disable_mcp_enterprise_auth().await;

    let published = session.get_config().await;
    assert!(!Arc::ptr_eq(&published, &denied));
    assert_eq!(published.plugins, denied.plugins);
    assert_eq!(session.hooks().preview_session_start(&request).len(), 1);

    // Complete the pending deny rebuild after its owner was replaced.
    session.refresh_hooks(denied).await;
    assert!(Arc::ptr_eq(&session.get_config().await, &published));
    assert!(session.hooks().preview_session_start(&request).is_empty());

    // Finishing a hook rebuild from the previous allow policy must not restore it.
    session.refresh_hooks(initial).await;
    assert!(session.hooks().preview_session_start(&request).is_empty());
    Ok(())
}

#[test_case::test_case(RuntimeConfigRefresh::User, false; "host enables")]
#[test_case::test_case(RuntimeConfigRefresh::User, true; "host disables")]
#[test_case::test_case(RuntimeConfigRefresh::Mcp, false; "MCP retains disabled")]
#[test_case::test_case(RuntimeConfigRefresh::Mcp, true; "MCP retains enabled")]
#[test_case::test_case(RuntimeConfigRefresh::UserFiles, false; "file retains disabled")]
#[test_case::test_case(RuntimeConfigRefresh::UserFiles, true; "file retains enabled")]
#[tokio::test]
async fn critical_path_refresh_changes_only_future_turns(
    scope: RuntimeConfigRefresh,
    initially_enabled: bool,
) {
    let (session, _) = make_session_and_context().await;
    let mut initial = session.get_config().await.as_ref().clone();
    initial
        .features
        .set_enabled(Feature::CriticalPathTiming, initially_enabled)
        .unwrap();
    let original = Arc::new(initial);
    session
        .state
        .lock()
        .await
        .session_configuration
        .original_config_do_not_use = Arc::clone(&original);
    let active_turn = session.new_default_turn().await;
    let mut next_config = original.as_ref().clone();
    next_config
        .features
        .set_enabled(Feature::CriticalPathTiming, !initially_enabled)
        .unwrap();

    assert_eq!(
        session.refresh_config(original, next_config, scope).await,
        crate::ConfigRefreshOutcome::Published
    );

    let next_turn = session.new_default_turn().await;
    assert_eq!(
        active_turn
            .config
            .features
            .enabled(Feature::CriticalPathTiming),
        initially_enabled
    );
    assert_eq!(
        next_turn
            .config
            .features
            .enabled(Feature::CriticalPathTiming),
        initially_enabled != matches!(scope, RuntimeConfigRefresh::User)
    );
}

#[tokio::test]
async fn critical_path_refresh_respects_managed_disable() {
    let (session, _) = make_session_and_context().await;
    let mut initial = session.get_config().await.as_ref().clone();
    initial.features = crate::config::ManagedFeatures::from_configured(
        initial.features.get().clone(),
        Some(Sourced::new(
            serde_json::from_value(json!({"critical_path_timing": false})).unwrap(),
            RequirementSource::LegacyManagedConfigTomlFromMdm,
        )),
    )
    .unwrap();
    let original = Arc::new(initial);
    session
        .state
        .lock()
        .await
        .session_configuration
        .original_config_do_not_use = Arc::clone(&original);
    let mut next_config = original.as_ref().clone();
    let mut incoming_features = original.features.get().clone();
    incoming_features.enable(Feature::CriticalPathTiming);
    next_config.features = incoming_features.into();

    assert_eq!(
        session.refresh_runtime_config(original, next_config).await,
        crate::ConfigRefreshOutcome::Published
    );
    assert!(
        !session
            .new_default_turn()
            .await
            .config
            .features
            .enabled(Feature::CriticalPathTiming)
    );
}
