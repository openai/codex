use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CyberAccessProgram;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadMetadataUpdateParams;
use codex_app_server_protocol::ThreadMetadataUpdateResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_login::AuthCredentialsStoreMode;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn thread_start_stages_daybreak_until_persistence() -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    responses::mount_sse_sequence(
        &server,
        (0..3)
            .map(|index| responses::sse(vec![responses::ev_completed(&format!("resp-{index}"))]))
            .collect(),
    )
    .await;
    let home = TempDir::new()?;
    let mut app = start_chatgpt_app(home.path(), &server).await?;
    let mut threads = Vec::new();
    for daybreak_enabled in [Some(true), Some(false), None] {
        let started = app
            .start_thread(ThreadStartParams {
                daybreak_enabled,
                ..Default::default()
            })
            .await?;
        assert_eq!(started.thread.daybreak_enabled, daybreak_enabled);
        let notification: ThreadStartedNotification =
            app.read_notification("thread/started").await?;
        assert_eq!(notification.thread, started.thread);
        let read: ThreadReadResponse = app
            .request(|request_id| ClientRequest::ThreadRead {
                request_id,
                params: ThreadReadParams {
                    thread_id: started.thread.id.clone(),
                    include_turns: false,
                },
            })
            .await?;
        assert_eq!(read.thread.daybreak_enabled, daybreak_enabled);
        let completed = app
            .start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: started.thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "start this task".to_owned(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        assert_eq!(completed.turn.status, TurnStatus::Completed);
        threads.push((started.thread.id, daybreak_enabled));
    }
    app.shutdown_gracefully().await?;

    let mut restarted = start_chatgpt_app(home.path(), &server).await?;
    for (thread_id, daybreak_enabled) in threads {
        let read: ThreadReadResponse = restarted
            .request(|request_id| ClientRequest::ThreadRead {
                request_id,
                params: ThreadReadParams {
                    thread_id: thread_id.clone(),
                    include_turns: false,
                },
            })
            .await?;
        assert_eq!(
            (read.thread.id, read.thread.daybreak_enabled),
            (thread_id, daybreak_enabled)
        );
    }
    Ok(())
}

#[tokio::test]
async fn thread_start_rejects_daybreak_for_ephemeral_threads() -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let home = TempDir::new()?;
    let mut app = start_chatgpt_app(home.path(), &server).await?;
    let request_id = app
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            daybreak_enabled: Some(true),
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;
    let error = app
        .read_stream_until_error_message(RequestId::Integer(request_id))
        .await?;
    assert_eq!(
        error.error,
        JSONRPCErrorError {
            code: -32600,
            message: "daybreakEnabled is not supported for ephemeral threads".to_owned(),
            data: None,
        }
    );
    Ok(())
}

#[tokio::test]
async fn turn_start_forwards_explicit_cyber_access_program() -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let programs = [
        None,
        Some(CyberAccessProgram::DaybreakBlue),
        Some(CyberAccessProgram::DaybreakRed),
        None,
        Some(CyberAccessProgram::Standard),
        None,
    ];
    let requests = responses::mount_sse_sequence(
        &server,
        programs
            .iter()
            .enumerate()
            .map(|(index, _)| {
                responses::sse(vec![responses::ev_completed(&format!("resp-{index}"))])
            })
            .collect(),
    )
    .await;
    let home = TempDir::new()?;
    let mut app = start_chatgpt_app(home.path(), &server).await?;
    let thread = app
        .start_thread(ThreadStartParams {
            daybreak_enabled: Some(true),
            ..Default::default()
        })
        .await?
        .thread;
    for program in programs {
        let completed = app
            .start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: thread.id.clone(),
                cyber_access_program: program,
                input: vec![UserInput::Text {
                    text: "hello".to_owned(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        assert_eq!(completed.turn.status, TurnStatus::Completed);
    }
    let requests = requests.requests();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.body_json().get("access_programs").cloned())
            .collect::<Vec<_>>(),
        [
            None,
            Some(json!({"cyber": "daybreak_blue"})),
            Some(json!({"cyber": "daybreak_red"})),
            None,
            Some(json!({"cyber": "standard"})),
            None,
        ]
    );
    Ok(())
}

#[tokio::test]
async fn daybreak_thread_metadata_persists_independently_across_restart_and_fork() -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let requests = responses::mount_sse_sequence(
        &server,
        (0..4)
            .map(|index| responses::sse(vec![responses::ev_completed(&format!("resp-{index}"))]))
            .collect(),
    )
    .await;
    let home = TempDir::new()?;
    let mut app = start_chatgpt_app(home.path(), &server).await?;
    let first = app
        .start_thread(ThreadStartParams {
            daybreak_enabled: Some(false),
            ..Default::default()
        })
        .await?;
    let second = app
        .start_thread(ThreadStartParams {
            daybreak_enabled: Some(true),
            ..Default::default()
        })
        .await?;
    assert_eq!(
        (
            first.thread.daybreak_enabled,
            second.thread.daybreak_enabled
        ),
        (Some(false), Some(true))
    );

    for (thread_id, enabled) in [(&first.thread.id, true), (&second.thread.id, false)] {
        let updated: ThreadMetadataUpdateResponse = app
            .request(|request_id| ClientRequest::ThreadMetadataUpdate {
                request_id,
                params: ThreadMetadataUpdateParams {
                    thread_id: thread_id.clone(),
                    project_id: None,
                    git_info: None,
                    daybreak_enabled: Some(enabled),
                },
            })
            .await?;
        assert_eq!(
            (updated.thread.id, updated.thread.daybreak_enabled),
            (thread_id.clone(), Some(enabled))
        );
    }
    // Leave the second thread empty to exercise metadata durability without a rollout.
    let completed = app
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: first.thread.id.clone(),
            input: vec![UserInput::Text {
                text: "start this task".to_owned(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    assert_eq!(requests.requests().len(), 1);
    app.shutdown_gracefully().await?;

    let mut restarted = start_chatgpt_app(home.path(), &server).await?;
    for (thread_id, enabled) in [(&first.thread.id, true), (&second.thread.id, false)] {
        let read: ThreadReadResponse = restarted
            .request(|request_id| ClientRequest::ThreadRead {
                request_id,
                params: ThreadReadParams {
                    thread_id: thread_id.clone(),
                    include_turns: false,
                },
            })
            .await?;
        assert_eq!(
            (read.thread.id, read.thread.daybreak_enabled),
            (thread_id.clone(), Some(enabled))
        );
    }
    let resumed: ThreadResumeResponse = restarted
        .request(|request_id| ClientRequest::ThreadResume {
            request_id,
            params: ThreadResumeParams {
                thread_id: first.thread.id.clone(),
                ..Default::default()
            },
        })
        .await?;
    assert_eq!(resumed.thread.daybreak_enabled, Some(true));
    let forked: ThreadForkResponse = restarted
        .request(|request_id| ClientRequest::ThreadFork {
            request_id,
            params: ThreadForkParams {
                thread_id: first.thread.id.clone(),
                ..Default::default()
            },
        })
        .await?;
    assert_eq!(forked.thread.daybreak_enabled, Some(true));
    let updated: ThreadMetadataUpdateResponse = restarted
        .request(|request_id| ClientRequest::ThreadMetadataUpdate {
            request_id,
            params: ThreadMetadataUpdateParams {
                thread_id: first.thread.id.clone(),
                project_id: None,
                git_info: None,
                daybreak_enabled: Some(false),
            },
        })
        .await?;
    assert_eq!(
        (updated.thread.id, updated.thread.daybreak_enabled),
        (first.thread.id.clone(), Some(false))
    );
    assert_eq!(requests.requests().len(), 1);
    restarted.shutdown_gracefully().await?;

    let mut restarted = start_chatgpt_app(home.path(), &server).await?;
    for (thread_id, program, enabled) in [
        (&forked.thread.id, Some(CyberAccessProgram::Standard), true),
        (&forked.thread.id, None, true),
        (&first.thread.id, None, false),
    ] {
        let resumed: ThreadResumeResponse = restarted
            .request(|request_id| ClientRequest::ThreadResume {
                request_id,
                params: ThreadResumeParams {
                    thread_id: thread_id.clone(),
                    ..Default::default()
                },
            })
            .await?;
        assert_eq!(resumed.thread.daybreak_enabled, Some(enabled));
        let completed = restarted
            .start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: thread_id.clone(),
                cyber_access_program: program,
                input: vec![UserInput::Text {
                    text: "hello".to_owned(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        assert_eq!(completed.turn.status, TurnStatus::Completed);
        let resumed: ThreadResumeResponse = restarted
            .request(|request_id| ClientRequest::ThreadResume {
                request_id,
                params: ThreadResumeParams {
                    thread_id: thread_id.clone(),
                    ..Default::default()
                },
            })
            .await?;
        assert_eq!(resumed.thread.daybreak_enabled, Some(enabled));
    }
    assert_eq!(
        requests
            .requests()
            .iter()
            .map(|request| request.body_json()["access_programs"].clone())
            .collect::<Vec<_>>(),
        [
            json!(null),
            json!({"cyber": "standard"}),
            json!(null),
            json!(null),
        ]
    );
    Ok(())
}

#[tokio::test]
async fn turn_start_forwards_cyber_access_program_with_personal_access_token() -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    Mock::given(method("GET"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(426))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/user-auth-credential/whoami"))
        .and(header("Authorization", "Bearer at-test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "email": null,
            "chatgpt_user_id": "user-123",
            "chatgpt_account_id": "account-123",
            "chatgpt_plan_type": "enterprise_cbp_automation",
            "chatgpt_account_is_fedramp": false,
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/config/bundle"))
        .and(header("Authorization", "Bearer at-test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&server)
        .await;
    let request = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("resp-1")]),
    )
    .await;
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            "model = \"gpt-5.5\"\napproval_policy = \"never\"\nopenai_base_url = \"{0}/v1\"\nchatgpt_base_url = \"{0}/backend-api\"\n",
            server.uri(),
        ),
    )?;
    let authapi_base_url = server.uri();
    let mut app = TestAppServer::builder()
        .with_codex_home(home.path())
        .without_managed_config()
        .with_env_overrides(&[
            ("OPENAI_API_KEY", None),
            ("CODEX_ACCESS_TOKEN", Some("at-test-token")),
            ("CODEX_AUTHAPI_BASE_URL", Some(authapi_base_url.as_str())),
        ])
        .build_initialized()
        .await?;
    let thread = app.start_thread(ThreadStartParams::default()).await?.thread;

    app.start_turn_and_wait_for_completion(TurnStartParams {
        thread_id: thread.id,
        cyber_access_program: Some(CyberAccessProgram::DaybreakBlue),
        input: vec![UserInput::Text {
            text: "hello".to_owned(),
            text_elements: Vec::new(),
        }],
        ..Default::default()
    })
    .await?;

    assert_eq!(
        request.single_request().body_json()["access_programs"],
        json!({"cyber": "daybreak_blue"})
    );
    Ok(())
}

async fn start_chatgpt_app(home: &Path, server: &MockServer) -> Result<TestAppServer> {
    Mock::given(method("GET"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(/*s*/ 426))
        .mount(server)
        .await;
    std::fs::write(
        home.join("config.toml"),
        format!(
            "model = \"gpt-5.5\"\napproval_policy = \"never\"\nopenai_base_url = \"{}/v1\"\ncli_auth_credentials_store = \"file\"\n",
            server.uri()
        ),
    )?;
    write_chatgpt_auth(
        home,
        ChatGptAuthFixture::new("chatgpt-test-token").plan_type("pro"),
        AuthCredentialsStoreMode::File,
    )?;
    TestAppServer::builder()
        .with_codex_home(home)
        .without_managed_config()
        .build_initialized()
        .await
}
