//! Exercise provider defaults and explicit choices through real app-server requests.

use super::*;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn starts_and_forks_use_server_provider_unless_explicitly_selected() -> Result<()> {
    let home = tempfile::tempdir()?;
    write_provider_config(home.path())?;
    let server_config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    let client_config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            model_provider: Some("client-provider".into()),
            ..Default::default()
        })
        .build()
        .await?;
    let local_settings = LocalSettings::from(&client_config);
    let parent = crate::tests::write_session_rollout(
        home.path(),
        "2025-01-02T10-00-00",
        "2025-01-02T10:00:00Z",
        "Server history",
        "server-provider",
        server_config.cwd.as_path(),
    )?;
    let mut server = crate::start_embedded_app_server_for_picker(&server_config).await?;
    assert_eq!(
        crate::lookup_session_target_with_app_server(
            crate::resume_picker::SessionPickerAction::Resume,
            &mut server,
            &client_config,
            "Server history"
        )
        .await?
        .unwrap()
        .thread_id,
        parent
    );
    let resumed = server
        .resume_thread(
            &local_settings,
            client_config.clone(),
            parent,
            ResumeModelSettings::OverrideFromCurrentConfig,
        )
        .await?;
    assert_eq!(resumed.session.model_provider_id, "server-provider");
    for explicit in [false, true] {
        server.model_provider_override = explicit.then(|| "client-provider".to_string());
        let expected = if explicit {
            "client-provider"
        } else {
            "server-provider"
        };
        let started = server.start_thread(&client_config).await?;
        let forked = server
            .fork_thread(&local_settings, client_config.clone(), parent)
            .await?;
        assert_eq!(
            (
                started.session.model_provider_id.as_str(),
                forked.session.model_provider_id.as_str()
            ),
            (expected, expected)
        );
    }
    server.model_provider_override = None;
    let path = home.path().join("config.toml");
    let updated = std::fs::read_to_string(&path)?.replacen(
        "model_provider = \"server-provider\"",
        "model_provider = \"client-provider\"",
        1,
    );
    std::fs::write(path, updated)?;
    assert_eq!(
        server.history_model_provider(&client_config).await?,
        Some("client-provider".into())
    );
    assert_eq!(
        server
            .start_thread(&client_config)
            .await?
            .session
            .model_provider_id,
        "client-provider"
    );
    // A fork of the active session preserves its provider even if launch defaults differ.
    server.model_provider_override = Some("client-provider".into());
    let forked = server
        .fork_thread_at(
            &local_settings,
            server_config,
            parent,
            /*last_turn_id*/ None,
            /*before_turn_id*/ None,
            ForkGoalContinuation::StartIfIdle,
            /*selected_profile*/ None,
        )
        .await?;
    assert_eq!(forked.session.model_provider_id, "server-provider");
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn selected_profile_provider_is_explicit() -> Result<()> {
    let home = tempfile::tempdir()?;
    let profile_path = AbsolutePathBuf::try_from(home.path().join("selected.config.toml"))?;
    std::fs::write(&profile_path, "model_provider = \"openai\"\n")?;
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(codex_config::LoaderOverrides {
            user_config_path: Some(profile_path),
            user_config_profile: Some("selected".parse()?),
            ..codex_config::LoaderOverrides::without_managed_config_for_tests()
        })
        .build()
        .await?;
    assert_eq!(
        provider_selection::explicit_provider(&config),
        Some("openai".into())
    );
    Ok(())
}

#[tokio::test]
async fn required_provider_overrides_oss_history_selection() -> Result<()> {
    let home = tempfile::tempdir()?;
    std::fs::write(
        home.path().join("requirements.toml"),
        "model_provider = 'openai'",
    )?;
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(
            codex_config::LoaderOverrides::with_managed_config_path_for_tests(
                home.path().join("managed_config.toml"),
            ),
        )
        .harness_overrides(ConfigOverrides {
            model_provider: Some("ollama".into()),
            ..Default::default()
        })
        .build()
        .await?;
    let mut server = crate::start_embedded_app_server_for_picker(&config).await?;
    server.model_provider_override = Some("ollama".into());
    assert_eq!(config.model_provider_id, "openai");
    assert_eq!(
        server.history_model_provider(&config).await?,
        Some("openai".into())
    );
    server.shutdown().await?;
    Ok(())
}

fn write_provider_config(home: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(
        home.join("config.toml"),
        r#"
model_provider = "server-provider"
model = "gpt-5.4"
[model_providers.server-provider]
name = "Server provider"
base_url = "http://127.0.0.1:9/v1"
wire_api = "responses"
requires_openai_auth = false
[model_providers.client-provider]
name = "Client provider"
base_url = "http://127.0.0.1:9/v1"
wire_api = "responses"
requires_openai_auth = false
"#,
    )?;
    Ok(())
}

#[tokio::test]
async fn resume_discovers_other_providers_but_rejects_mismatched_resume() -> Result<()> {
    let home = tempfile::tempdir()?;
    write_provider_config(home.path())?;
    let original = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    let parent = crate::tests::write_session_rollout(
        home.path(),
        "2025-01-02T10-00-00",
        "2025-01-02T10:00:00Z",
        "Old provider history",
        "server-provider",
        original.cwd.as_path(),
    )?;
    let path = home.path().join("config.toml");
    let updated = std::fs::read_to_string(&path)?.replacen(
        "model_provider = \"server-provider\"",
        "model_provider = \"client-provider\"",
        1,
    );
    std::fs::write(path, updated)?;
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await?;
    let local_settings = LocalSettings::from(&config);
    let mut server = crate::start_embedded_app_server_for_picker(&config).await?;
    let named = crate::lookup_session_target_with_app_server(
        crate::resume_picker::SessionPickerAction::Resume,
        &mut server,
        &config,
        "Old provider history",
    )
    .await?
    .expect("old-provider thread by name");
    let latest = crate::lookup_latest_session_target_with_app_server(
        crate::resume_picker::SessionPickerAction::Resume,
        &mut server,
        &config,
        /*cwd_filter*/ None,
        /*include_non_interactive*/ false,
    )
    .await?
    .expect("old-provider thread via --last");
    assert_eq!((named.thread_id, latest.thread_id), (parent, parent));
    assert!(
        crate::lookup_session_target_with_app_server(
            crate::resume_picker::SessionPickerAction::Fork,
            &mut server,
            &config,
            "Old provider history",
        )
        .await?
        .is_none()
    );
    assert!(
        crate::lookup_latest_session_target_with_app_server(
            crate::resume_picker::SessionPickerAction::Fork,
            &mut server,
            &config,
            /*cwd_filter*/ None,
            /*include_non_interactive*/ false,
        )
        .await?
        .is_none()
    );
    let error = server
        .resume_thread(
            &local_settings,
            config.clone(),
            parent,
            ResumeModelSettings::RestoreFromThread,
        )
        .await
        .expect_err("a different configured provider must block resume");
    assert!(error.to_string().contains(
        "This chat uses server-provider, but your current provider is client-provider. Switch to server-provider to continue."
    ));
    assert_eq!(
        server
            .start_thread(&config)
            .await?
            .session
            .model_provider_id,
        "client-provider"
    );
    server.shutdown().await?;
    Ok(())
}
