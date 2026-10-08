//! Runtime-refresh regressions; pure authority resolution is tested with Config.

use super::*;
use pretty_assertions::assert_eq;

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
