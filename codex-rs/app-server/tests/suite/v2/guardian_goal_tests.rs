//! Goal API edits and clears enter model history across loaded and cold-resume paths.

use super::*;
use codex_app_server_protocol::ThreadGoalSetResponse;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[test_case(false; "loaded")]
#[test_case(true; "unloaded with path resume")]
#[tokio::test]
async fn user_goal_updates_survive_resume_and_clear(unloaded: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("done")]),
    )
    .await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_provider_config("supports_websockets = false")
        .enable_feature(Feature::Goals)
        .disable_feature(Feature::EnableRequestCompression)
        .write(codex_home.path())?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized_with_timeout(TIMEOUT)
        .await?;
    let thread_id = if unloaded {
        create_fake_rollout(
            codex_home.path(),
            "2025-01-05T12-00-00",
            "2025-01-05T12:00:00Z",
            "",
            Some("mock_provider"),
            /*git_info*/ None,
        )?
    } else {
        app.start_thread(ThreadStartParams::default())
            .await?
            .thread
            .id
    };
    // Automatic lifecycle calls mutate goal state without creating user authorization.
    let id = app
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread_id, "objective": "Automatically generated goal.",
                "status": "paused", "origin": "automatic",
            })),
        )
        .await?;
    let _: ThreadGoalSetResponse = app.read_response(id).await?;
    let mut resume_request = None;
    for objective in [
        "Send the approved report.",
        "Draft the report, but do not send it.",
    ] {
        if unloaded && objective.starts_with("Draft") {
            let path = codex_rollout::find_thread_path_by_id_str(
                codex_home.path(),
                &thread_id,
                /*state_db_ctx*/ None,
            )
            .await?
            .expect("unloaded rollout");
            resume_request = Some(
                app.send_thread_resume_request(ThreadResumeParams {
                    thread_id: String::new(),
                    path: Some(path),
                    ..Default::default()
                })
                .await?,
            );
        }
        let id = app
            .send_raw_request(
                "thread/goal/set",
                Some(json!({
                    "threadId": thread_id, "objective": objective, "status": "paused", "origin": "user",
                })),
            )
            .await?;
        let _: ThreadGoalSetResponse = app.read_response(id).await?;
    }
    if let Some(id) = resume_request {
        let _: ThreadResumeResponse = app.read_response(id).await?;
    }
    let id = app
        .send_raw_request(
            "thread/goal/clear",
            Some(json!({"threadId": thread_id, "origin": "user"})),
        )
        .await?;
    let _: codex_app_server_protocol::ThreadGoalClearResponse = app.read_response(id).await?;
    let id = app
        .send_turn_start_request(TurnStartParams {
            thread_id,
            input: vec![UserInput::Text {
                text: "What is the status?".to_owned(),
                text_elements: vec![],
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = app.read_response(id).await?;
    let completed: TurnCompletedNotification =
        timeout(TIMEOUT, app.read_notification("turn/completed")).await??;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    let history = mock.single_request().message_input_texts("user").join("\n");
    assert!(
        !history.contains("Automatically generated goal."),
        "{history}"
    );
    let old = history
        .find("User set the goal: \"Send the approved report.\"")
        .expect("original user goal");
    let new = history
        .find("User set the goal: \"Draft the report, but do not send it.\"")
        .expect("updated user goal");
    assert!(old < new, "{history}");
    assert!(
        history.contains("User set goal status: \"paused\"."),
        "{history}"
    );
    assert!(history.contains("User cleared the goal."), "{history}");
    app.shutdown_gracefully().await?;
    Ok(())
}
