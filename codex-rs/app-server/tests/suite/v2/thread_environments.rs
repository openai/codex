use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use codex_app_server_protocol::ThreadEnvironment;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

use super::connection_handling_websocket::DEFAULT_READ_TIMEOUT;
use super::connection_handling_websocket::connect_websocket;
use super::connection_handling_websocket::create_config_toml;
use super::connection_handling_websocket::read_notification_for_method;
use super::connection_handling_websocket::read_response_for_id;
use super::connection_handling_websocket::send_request;
use super::connection_handling_websocket::spawn_websocket_server;

#[tokio::test]
async fn thread_environments_follow_the_loaded_thread_selection() -> Result<()> {
    let responses = create_mock_responses_server_repeating_assistant("Done").await;
    let home = TempDir::new()?;
    MockResponsesConfig::new(&responses.uri()).write(home.path())?;
    let mut first = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized()
        .await?;
    let mut selection = first.auto_env_params()?;
    let cwd = &first.auto_env()?.selection().cwd;
    let roots = vec![cwd.join("src")?.into(), cwd.join("docs")?.into()];
    selection.runtime_workspace_roots = Some(roots.clone());
    let expected = Some(vec![ThreadEnvironment {
        environment_id: selection.environment_id.clone(),
        cwd: selection.cwd.clone(),
        runtime_workspace_roots: roots,
    }]);
    let request = first
        .send_thread_start_request(ThreadStartParams {
            environments: Some(vec![selection]),
            ..Default::default()
        })
        .await?;
    let started: ThreadStartResponse =
        timeout(DEFAULT_READ_TIMEOUT, first.read_response(request)).await??;
    assert_eq!(started.thread.environments, expected);
    let thread_id = started.thread.id;
    let completed = timeout(
        DEFAULT_READ_TIMEOUT,
        first.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: vec![],
            }],
            ..Default::default()
        }),
    )
    .await??;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    assert_read_and_list_environments(&mut first, &thread_id, &expected).await?;

    // Report the thread's explicit selection rather than the server-wide defaults.
    let completed = timeout(
        DEFAULT_READ_TIMEOUT,
        first.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: "hello again".into(),
                text_elements: vec![],
            }],
            environments: Some(vec![]),
            ..Default::default()
        }),
    )
    .await??;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    assert_read_and_list_environments(&mut first, &thread_id, &Some(vec![])).await?;
    let request = first
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let resumed: ThreadResumeResponse =
        timeout(DEFAULT_READ_TIMEOUT, first.read_response(request)).await??;
    assert_eq!(resumed.thread.environments, Some(vec![]));
    timeout(DEFAULT_READ_TIMEOUT, first.shutdown_gracefully()).await??;

    // Unloaded reads have no live selection. Resume reports its newly selected defaults.
    let mut second = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized()
        .await?;
    assert_read_and_list_environments(&mut second, &thread_id, &None).await?;
    let request = second
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let resumed: ThreadResumeResponse =
        timeout(DEFAULT_READ_TIMEOUT, second.read_response(request)).await??;
    let expected = Some(vec![ThreadEnvironment {
        environment_id: second.auto_env_params()?.environment_id,
        cwd: resumed.cwd,
        runtime_workspace_roots: resumed.runtime_workspace_roots,
    }]);
    assert_eq!(resumed.thread.environments, expected);
    assert_read_and_list_environments(&mut second, &thread_id, &expected).await?;
    let request = second
        .send_thread_resume_request(ThreadResumeParams {
            thread_id,
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let resumed: ThreadResumeResponse =
        timeout(DEFAULT_READ_TIMEOUT, second.read_response(request)).await??;
    assert_eq!(resumed.thread.environments, expected);
    Ok(())
}

async fn assert_read_and_list_environments(
    server: &mut TestAppServer,
    thread_id: &str,
    expected: &Option<Vec<ThreadEnvironment>>,
) -> Result<()> {
    let request = server
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread_id.to_owned(),
            include_turns: false,
        })
        .await?;
    let read: ThreadReadResponse =
        timeout(DEFAULT_READ_TIMEOUT, server.read_response(request)).await??;
    assert_eq!(&read.thread.environments, expected);
    let request = server
        .send_thread_list_request(serde_json::from_value::<ThreadListParams>(json!({}))?)
        .await?;
    let listed: ThreadListResponse =
        timeout(DEFAULT_READ_TIMEOUT, server.read_response(request)).await??;
    assert_eq!(
        &listed
            .data
            .iter()
            .find(|thread| thread.id == thread_id)
            .expect("listed thread")
            .environments,
        expected
    );
    Ok(())
}

#[tokio::test]
async fn thread_environments_are_returned_to_a_late_subscriber() -> Result<()> {
    let responses = create_mock_responses_server_repeating_assistant("Done").await;
    let home = TempDir::new()?;
    create_config_toml(home.path(), &responses.uri(), "never")?;
    let workspace = TempDir::new()?;
    let cwd = AbsolutePathBuf::from_absolute_path(workspace.path().canonicalize()?)?;
    let expected =
        json!([{ "environmentId": "local", "cwd": cwd, "runtimeWorkspaceRoots": [cwd] }]);
    let (mut process, address) = spawn_websocket_server(home.path()).await?;
    let mut first = connect_websocket(address).await?;
    let initialize = json!({
        "clientInfo": { "name": "environment-test", "version": "1" },
        "capabilities": { "experimentalApi": true }
    });
    send_request(
        &mut first,
        "initialize",
        /*id*/ 1,
        Some(initialize.clone()),
    )
    .await?;
    read_response_for_id(&mut first, /*id*/ 1).await?;
    send_request(
        &mut first,
        "thread/start",
        /*id*/ 2,
        Some(json!({
            "environments": [{ "environmentId": "local", "cwd": cwd }]
        })),
    )
    .await?;
    let started = read_response_for_id(&mut first, /*id*/ 2).await?;
    assert_eq!(started.result["thread"]["environments"], expected);
    let thread_id = started.result["thread"]["id"].as_str().unwrap();
    send_request(
        &mut first,
        "turn/start",
        /*id*/ 3,
        Some(json!({
            "threadId": thread_id, "input": [{ "type": "text", "text": "hello" }]
        })),
    )
    .await?;
    read_response_for_id(&mut first, /*id*/ 3).await?;
    read_notification_for_method(&mut first, "turn/completed").await?;

    let mut second = connect_websocket(address).await?;
    send_request(&mut second, "initialize", /*id*/ 1, Some(initialize)).await?;
    read_response_for_id(&mut second, /*id*/ 1).await?;
    for (id, method, params) in [
        (2, "thread/read", json!({ "threadId": thread_id })),
        (
            3,
            "thread/resume",
            json!({ "threadId": thread_id, "excludeTurns": true }),
        ),
    ] {
        send_request(&mut second, method, id, Some(params)).await?;
        let response = read_response_for_id(&mut second, id).await?;
        assert_eq!(response.result["thread"]["environments"], expected);
    }
    process.kill().await?;
    Ok(())
}
