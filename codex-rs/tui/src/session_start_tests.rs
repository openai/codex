use super::*;
use crate::legacy_core::config::ConfigBuilder;
use app_test_support::create_fake_rollout;
use app_test_support::rollout_path;
use codex_config::LoaderOverrides;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use std::cell::Cell;
use tempfile::TempDir;

async fn start_session(
    app_server: &mut AppServerSession,
    config: &Config,
    target: &SessionTarget,
    action: SessionStartAction,
    confirm: impl AsyncFnOnce() -> Result<UnarchiveChoice>,
) -> Result<Option<AppServerStartedThread>> {
    let initial_result = action.start(app_server, config, target).await;
    complete_session_start(app_server, config, target, action, initial_result, confirm).await
}

#[tokio::test]
async fn archived_session_requires_confirmation_before_resume_or_fork() -> Result<()> {
    for action in [
        SessionStartAction::Resume(ResumeModelSettings::RestoreFromThread),
        SessionStartAction::Fork,
    ] {
        let codex_home = TempDir::new()?;
        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .build()
            .await?;
        let timestamp = "2025-01-05T12-00-00";
        let id = create_fake_rollout(
            codex_home.path(),
            timestamp,
            "2025-01-05T12:00:00Z",
            "Archived saved user message",
            Some(config.model_provider_id.as_str()),
            /*git_info*/ None,
        )
        .expect("create source rollout");
        let active_path = rollout_path(codex_home.path(), timestamp, &id);
        let archived_dir = codex_home.path().join("archived_sessions");
        std::fs::create_dir_all(&archived_dir)?;
        let archived_path = archived_dir.join(active_path.file_name().unwrap());
        std::fs::rename(&active_path, &archived_path)?;
        let target = SessionTarget {
            path: Some(archived_path.clone()),
            thread_id: ThreadId::from_string(&id)?,
            history_mode: None,
        };
        let mut app_server = crate::start_embedded_app_server_for_picker(&config).await?;
        let prompts = Cell::new(0);

        let cancelled = start_session(&mut app_server, &config, &target, action, async || {
            prompts.set(prompts.get() + 1);
            Ok(UnarchiveChoice::Cancel)
        })
        .await?;
        assert!(cancelled.is_none());
        assert_eq!(
            (prompts.get(), archived_path.exists(), active_path.exists()),
            (1, true, false)
        );

        let started = start_session(&mut app_server, &config, &target, action, async || {
            prompts.set(prompts.get() + 1);
            Ok(UnarchiveChoice::Unarchive)
        })
        .await?
        .expect("confirmed session should start");
        assert_eq!(
            (prompts.get(), archived_path.exists(), active_path.exists()),
            (2, false, true)
        );
        match action {
            SessionStartAction::Resume(_) => {
                assert_eq!(started.session.thread_id, target.thread_id)
            }
            SessionStartAction::Fork => {
                assert_ne!(started.session.thread_id, target.thread_id);
                assert_eq!(
                    app_server
                        .thread_read(started.session.thread_id, /*include_turns*/ false)
                        .await?
                        .forked_from_id,
                    Some(id)
                );
            }
        }

        let resumed = start_session(
            &mut app_server,
            &config,
            &target,
            SessionStartAction::Resume(ResumeModelSettings::RestoreFromThread),
            async || panic!("active sessions must not prompt"),
        )
        .await?
        .expect("active session should start");
        assert_eq!(resumed.session.thread_id, target.thread_id);

        let missing = SessionTarget {
            path: None,
            thread_id: ThreadId::new(),
            history_mode: None,
        };
        let error = start_session(&mut app_server, &config, &missing, action, async || {
            panic!("unrelated failures must not prompt")
        })
        .await
        .expect_err("missing session should fail");
        assert!(
            error
                .to_string()
                .starts_with(&format!("Failed to {} session", action.verb()))
        );
        app_server.shutdown().await?;
    }
    Ok(())
}

#[test]
fn session_start_error_surfaces_archived_guidance_without_rollout_path() {
    let thread_id =
        ThreadId::from_string("019e72f4-e09a-70f2-b2c2-a153a57b8cc0").expect("thread id");
    let target_session = SessionTarget {
        path: Some(std::path::PathBuf::from(
            "/Users/me/.codex/archived_sessions/rollout.jsonl",
        )),
        thread_id,
        history_mode: None,
    };
    let expected = format!(
        "session {thread_id} is archived. Run `codex unarchive {thread_id}` to unarchive it first."
    );

    for action in ["resume", "fork"] {
        let err = color_eyre::eyre::eyre!(
            "thread/{action} failed during TUI bootstrap: thread/{action} failed: {expected} (code -32600)"
        );

        assert_eq!(
            session_start_error(action, &target_session, err).to_string(),
            expected
        );
    }
}
