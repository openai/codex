//! Verify that remote lifecycle requests use workspace roots owned by the server.

use super::*;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use codex_app_server_protocol::ServerNotification;
use core_test_support::responses;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn remote_workspace_roots_survive_start_turn_resume_and_fork() -> Result<()> {
    let model_server = responses::start_mock_server().await;
    let response = responses::mount_sse_once(
        &model_server,
        responses::sse(vec![
            responses::ev_response_created("response"),
            responses::ev_completed("response"),
        ]),
    )
    .await;
    let server_home = tempfile::tempdir()?;
    let client_home = tempfile::tempdir()?;
    let workspace = tempfile::tempdir()?;
    let remote_cwd = AbsolutePathBuf::from_absolute_path(workspace.path().canonicalize()?)?;
    let extra_root = remote_cwd.join("shared");
    std::fs::create_dir(extra_root.as_path())?;
    let base_url = model_server.uri();
    let extra_root_toml = serde_json::to_string(&extra_root)?;
    std::fs::write(
        server_home.path().join("config.toml"),
        format!(
            r#"
model = "gpt-5.2"
model_provider = "workspace-test"
sandbox_mode = "workspace-write"
[sandbox_workspace_write]
writable_roots = [{extra_root_toml}]
[model_providers.workspace-test]
name = "OpenAI"
base_url = "{base_url}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )?;
    let server_config = ConfigBuilder::default()
        .codex_home(server_home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(remote_cwd.to_path_buf()),
            ..Default::default()
        })
        .build()
        .await?;
    let client_config = ConfigBuilder::default()
        .codex_home(client_home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(client_home.path().to_path_buf()),
            ..Default::default()
        })
        .build()
        .await?;
    let local_settings = LocalSettings::from(&client_config);
    let expected_roots = vec![remote_cwd.clone(), extra_root.clone()];
    let mut app_server = crate::start_embedded_app_server_for_picker(&server_config).await?;
    // Exercise remote request semantics with a real server and separate client config.
    app_server.thread_params_mode = ThreadParamsMode::Remote;
    app_server.remote_cwd_override = Some(remote_cwd.to_path_buf());
    let started = app_server.start_thread(&client_config).await?;
    let thread_id = started.session.thread_id;
    assert_eq!(started.session.cwd, remote_cwd);
    assert_eq!(started.session.runtime_workspace_roots, expected_roots);

    let (mut chat, _sender, _events, _commands) =
        crate::chatwidget::tests::helpers::make_chatwidget_manual_with_sender().await;
    chat.handle_thread_session(started.session);
    let config = chat.config_ref();
    assert_eq!(config.workspace_roots, expected_roots);
    app_server
        .turn_start(
            thread_id,
            "user-message".to_string(),
            vec![UserInput::Text {
                text: "hello".to_string(),
                text_elements: Vec::new(),
            }],
            config.cwd.to_path_buf(),
            /*approval_policy*/ None,
            /*approvals_reviewer*/ None,
            TurnPermissionsOverride::Preserve,
            config.permissions.user_visible_workspace_roots(),
            "gpt-5.2".to_string(),
            /*effort*/ None,
            /*summary*/ None,
            /*service_tier*/ None,
            /*collaboration_mode*/ None,
            /*personality*/ None,
            /*output_schema*/ None,
        )
        .await?;
    tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 30), async {
        while let Some(event) = app_server.next_event().await {
            if let AppServerEvent::ServerNotification(notification) = event
                && let ServerNotification::TurnCompleted(completed) = *notification
            {
                assert_eq!(
                    completed.turn.status,
                    codex_app_server_protocol::TurnStatus::Completed
                );
                return;
            }
        }
        panic!("app-server disconnected before completing the turn");
    })
    .await?;
    response.single_request();

    let resumed = app_server
        .resume_thread(
            &local_settings,
            client_config.clone(),
            thread_id,
            ResumeModelSettings::RestoreFromThread,
        )
        .await?;
    assert_eq!(resumed.session.runtime_workspace_roots, expected_roots);
    let forked = app_server
        .fork_thread(&local_settings, client_config.clone(), thread_id)
        .await?;
    assert_eq!(forked.session.runtime_workspace_roots, expected_roots);

    // Existing tasks may retain roots that are no longer in the server defaults.
    let config_path = server_home.path().join("config.toml");
    let updated_config = std::fs::read_to_string(&config_path)?.replace(
        &format!("writable_roots = [{extra_root_toml}]"),
        "writable_roots = []",
    );
    std::fs::write(config_path, updated_config)?;
    let session_config = chat.config_ref().clone();
    let forked = app_server
        .fork_thread_at(
            &local_settings,
            session_config.clone(),
            thread_id,
            /*last_turn_id*/ None,
            /*before_turn_id*/ None,
            ForkGoalContinuation::StartIfIdle,
            /*selected_profile*/ None,
        )
        .await?;
    assert_eq!(forked.session.runtime_workspace_roots, expected_roots);
    let side = app_server
        .fork_side_thread(&local_settings, session_config, thread_id)
        .await?;
    assert_eq!(side.session.runtime_workspace_roots, expected_roots);

    app_server.remote_cwd_override = None;
    let started = app_server.start_thread(&client_config).await?;
    let default_cwd = AbsolutePathBuf::current_dir()?;
    assert_eq!(started.session.cwd, default_cwd);
    assert_eq!(started.session.runtime_workspace_roots, vec![default_cwd]);
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn embedded_lifecycle_requests_preserve_explicit_workspace_roots() -> Result<()> {
    let home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            additional_writable_roots: vec![home.path().to_path_buf()],
            ..Default::default()
        })
        .build()
        .await?;
    let expected = Some(config.workspace_roots.clone());
    let thread_id = ThreadId::new();
    let start = thread_start_params_from_config(
        &config,
        ThreadParamsMode::Embedded,
        /*remote_cwd_override*/ None,
        /*session_start_source*/ None,
    );
    let resume = thread_resume_params_from_config(
        config.clone(),
        thread_id,
        ThreadParamsMode::Embedded,
        /*remote_cwd_override*/ None,
        ResumeModelSettings::RestoreFromThread,
    );
    let fork = thread_fork_params_from_config(
        config,
        thread_id,
        ThreadParamsMode::Embedded,
        /*remote_cwd_override*/ None,
    );
    assert_eq!(
        [
            start.runtime_workspace_roots,
            resume.runtime_workspace_roots,
            fork.runtime_workspace_roots
        ],
        [expected.clone(), expected.clone(), expected]
    );
    Ok(())
}
