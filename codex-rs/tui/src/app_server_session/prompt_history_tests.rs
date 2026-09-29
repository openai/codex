//! Local prompt history stays independent of server settings across thread lifecycle requests.

use super::*;
use crate::legacy_core::config::ConfigBuilder;
use app_test_support::create_fake_rollout;
use codex_config::types::HistoryPersistence;
use codex_message_history::HistoryConfig;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn lifecycle_metadata_uses_local_prompt_history() -> Result<()> {
    let local_home = tempfile::tempdir()?;
    let local_config = ConfigBuilder::default()
        .codex_home(local_home.path().to_path_buf())
        .build()
        .await?;
    let local_settings = LocalSettings::from(&local_config);
    let history_config =
        HistoryConfig::new(local_settings.codex_home.clone(), &local_settings.history);
    codex_message_history::append_entry("local prompt", ThreadId::new(), &history_config).await?;
    let (log_id, entry_count) = codex_message_history::history_metadata(&history_config).await;
    let expected = Some(MessageHistoryMetadata {
        log_id,
        entry_count,
    });
    assert_eq!(entry_count, 1);

    for mode in [ThreadParamsMode::Embedded, ThreadParamsMode::Remote] {
        let server_home = tempfile::tempdir()?;
        let mut config = ConfigBuilder::default()
            .codex_home(server_home.path().to_path_buf())
            .build()
            .await?;
        crate::legacy_core::config::set_project_trust_level(
            server_home.path(),
            &std::env::current_dir()?,
            codex_protocol::config_types::TrustLevel::Trusted,
        )
        .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
        config.history.persistence = HistoryPersistence::None;
        config.history.max_bytes = Some(1);
        let thread_id = ThreadId::from_string(
            &create_fake_rollout(
                server_home.path(),
                "2025-01-05T12-00-00",
                "2025-01-05T12:00:00Z",
                "Saved user message",
                Some(config.model_provider_id.as_str()),
                /*git_info*/ None,
            )
            .expect("create source rollout"),
        )?;
        let mut app_server = crate::start_embedded_app_server_for_picker(&config).await?;
        app_server.thread_params_mode = mode;

        let started = app_server
            .start_thread_with_session_start_source(
                &local_settings,
                &config,
                /*session_start_source*/ None,
                /*remote_cwd_override*/ None,
                /*selected_profile*/ None,
            )
            .await?;
        assert_eq!(started.session.message_history, expected);
        let startup = start_thread_with_request_handle(
            app_server.request_handle(),
            &local_settings,
            config.clone(),
            mode,
            /*remote_cwd_override*/ None,
            app_server.thread_tool_transport(),
            /*model_provider_override*/ None,
        )
        .await?;
        assert_eq!(startup.session.message_history, expected);
        let resumed = app_server
            .resume_thread(
                &local_settings,
                config.clone(),
                thread_id,
                ResumeModelSettings::RestoreFromThread,
            )
            .await?;
        assert_eq!(resumed.session.message_history, expected);
        assert_eq!(resumed.session.thread_id, thread_id);
        assert!(!resumed.turns.is_empty());
        let forked = app_server
            .fork_thread(&local_settings, config, thread_id)
            .await?;
        assert_eq!(forked.session.message_history, expected);
        assert_eq!(forked.session.forked_from_id, Some(thread_id));
        assert!(!forked.turns.is_empty());
        app_server.shutdown().await?;
    }
    Ok(())
}
