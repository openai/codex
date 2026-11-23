#[cfg(unix)]
#[path = "common/remote_exec.rs"]
mod remote_exec_support;

use std::path::Path;
use std::process::Output;

use anyhow::Context;
use anyhow::Result;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

const THREAD_ID: &str = "123e4567-e89b-12d3-a456-426614174000";

enum QueueResponse {
    Success,
    ResumeRejected,
    MethodNotFound,
    #[cfg(unix)]
    UnknownMethodVariant,
}

#[cfg(unix)]
fn socket_test_home() -> Result<TempDir> {
    // macOS temporary paths can exceed the Unix socket path limit.
    #[cfg(target_os = "macos")]
    let home = tempfile::tempdir_in("/tmp")?;
    #[cfg(not(target_os = "macos"))]
    let home = TempDir::new()?;
    Ok(home)
}

async fn respond_to_queue_request<S>(
    stream: S,
    codex_home: &Path,
    response: QueueResponse,
) -> Result<Value>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let mut websocket = accept_async(stream).await?;
    let initialize = websocket
        .next()
        .await
        .context("missing initialize request")??;
    let initialize: Value = serde_json::from_str(initialize.to_text()?)?;
    assert_eq!(initialize["method"], "initialize");
    assert_eq!(
        initialize["params"]["capabilities"]["experimentalApi"],
        true
    );
    let initialized_response = json!({
        "id": initialize["id"],
        "result": {
            "userAgent": "codex_cli_rs/0.0.0-test",
            "codexHome": codex_home,
        },
    });
    websocket
        .send(Message::Text(initialized_response.to_string().into()))
        .await?;

    let initialized = websocket
        .next()
        .await
        .context("missing initialized notification")??;
    let initialized: Value = serde_json::from_str(initialized.to_text()?)?;
    assert_eq!(initialized["method"], "initialized");

    let request = websocket.next().await.context("missing queue request")??;
    let request: Value = serde_json::from_str(request.to_text()?)?;
    assert_eq!(request["method"], "thread/queue/add");
    assert_eq!(request["params"]["threadId"], THREAD_ID);

    let result = match response {
        QueueResponse::Success | QueueResponse::ResumeRejected => json!({
            "id": request["id"],
            "result": {
                "queuedSubmission": {
                    "id": "queued-submission-id",
                    "input": request["params"]["input"],
                    "clientUserMessageId": request["params"]["clientUserMessageId"],
                },
            },
        }),
        QueueResponse::MethodNotFound => json!({
            "id": request["id"],
            "error": { "code": -32601, "message": "Method not found" },
        }),
        #[cfg(unix)]
        QueueResponse::UnknownMethodVariant => json!({
            "id": request["id"],
            "error": {
                "code": -32600,
                "message": "Invalid request: unknown variant `thread/queue/add`, expected `thread/list`",
            },
        }),
    };
    websocket
        .send(Message::Text(result.to_string().into()))
        .await?;
    if matches!(response, QueueResponse::ResumeRejected) {
        for (method, mut reply) in [
            (
                "thread/read",
                json!({"result": {"thread": {"status": {"type": "notLoaded"}, "cwd": r"C:\workspace"}}}),
            ),
            (
                "thread/resume",
                json!({"error": {"code": -32603, "message": "thread already has an active writer"}}),
            ),
        ] {
            let rpc = websocket.next().await.context("missing resume request")??;
            let rpc: Value = serde_json::from_str(rpc.to_text()?)?;
            assert_eq!(rpc["method"], method);
            assert_eq!(rpc["params"]["threadId"], THREAD_ID);
            reply["id"] = rpc["id"].clone();
            websocket
                .send(Message::Text(reply.to_string().into()))
                .await?;
        }
    }
    Ok(request)
}

#[tokio::test]
async fn queue_submits_message_to_remote_app_server() -> Result<()> {
    let (output, request) = run_remote_queue_command(QueueResponse::Success).await?;

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request["params"]["threadId"], THREAD_ID);
    assert_eq!(request["params"]["input"][0]["text"], "do the thing");
    assert_eq!(
        String::from_utf8(output.stdout)?,
        format!("Queued message queued-submission-id for thread {THREAD_ID}.\n")
    );
    Ok(())
}

#[tokio::test]
async fn queue_succeeds_when_another_server_owns_the_thread() -> Result<()> {
    let (output, _) = run_remote_queue_command(QueueResponse::ResumeRejected).await?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)?,
        format!("Queued message queued-submission-id for thread {THREAD_ID}.\n")
    );
    assert!(String::from_utf8(output.stderr)?.contains("thread already has an active writer"));
    Ok(())
}

#[tokio::test]
async fn queue_does_not_fallback_from_unsupported_explicit_remote() -> Result<()> {
    let (output, _) = run_remote_queue_command(QueueResponse::MethodNotFound).await?;

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)?
            .contains("remote app server does not support thread/queue/add")
    );
    Ok(())
}

#[test]
fn queue_rejects_empty_message() -> Result<()> {
    let output = std::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .args(["queue", "--thread", THREAD_ID, "--message", ""])
        .output()?;
    assert!(!output.status.success());
    Ok(())
}

#[test]
fn queue_rejects_unsupported_options() -> Result<()> {
    let codex_home = TempDir::new()?;
    for (args, expected) in [
        ("queue --image screenshot.png", "image attachments"),
        ("queue --model other-model", "`--model`"),
        ("--oss queue", "`--oss`"),
        ("queue --local-provider ollama", "`--local-provider`"),
        (
            "queue --dangerously-bypass-hook-trust",
            "`--dangerously-bypass-hook-trust`",
        ),
        (
            "-c model=other queue -c analytics.enabled=false",
            "`model` config override",
        ),
        ("queue -s read-only", "`--sandbox`"),
        ("--approve-for-me queue", "`--approve-for-me`"),
        ("prompt queue", "positional prompt; use `--message`"),
        ("-C . queue --remote unix://", "`--cd` with `--remote`"),
        ("--remote unix:// queue -C .", "`--cd` with `--remote`"),
    ] {
        let output = std::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
            .env("CODEX_HOME", codex_home.path())
            .args(args.split_whitespace())
            .args(["--thread", THREAD_ID, "--message", "do the thing"])
            .output()?;
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr)?;
        assert!(
            stderr.contains("`codex queue`") && stderr.contains(expected),
            "{args:?}: {stderr}"
        );
    }
    Ok(())
}

async fn run_remote_queue_command(response: QueueResponse) -> Result<(Output, Value)> {
    let codex_home = TempDir::new()?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("ws://{}", listener.local_addr()?);
    let remote_first = matches!(response, QueueResponse::MethodNotFound);
    let resume = matches!(response, QueueResponse::ResumeRejected);
    let server_home = codex_home.path().to_path_buf();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        respond_to_queue_request(stream, server_home.as_path(), response).await
    });

    let remote_args = if remote_first {
        [
            "--remote",
            endpoint.as_str(),
            "--remote-auth-token-env",
            "CODEX_REMOTE_TOKEN",
            "queue",
        ]
    } else {
        [
            "queue",
            "--remote",
            endpoint.as_str(),
            "--remote-auth-token-env",
            "CODEX_REMOTE_TOKEN",
        ]
    };
    let output = tokio::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .env("CODEX_HOME", codex_home.path())
        .env("CODEX_REMOTE_TOKEN", "test-token")
        .args(remote_args)
        .args(resume.then_some("--resume"))
        .args(["--thread", THREAD_ID, "--message", "do the thing"])
        .output()
        .await?;
    Ok((output, server.await??))
}

#[cfg(unix)]
async fn wait_for_socket(socket_path: &Path) -> Result<()> {
    use std::time::Duration;
    use tokio::time::timeout;

    timeout(Duration::from_secs(30), async {
        while !socket_path.try_exists()? {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Ok::<_, anyhow::Error>(())
    })
    .await?
}

#[cfg(unix)]
#[tokio::test]
async fn exec_keeps_saved_approval_settings_for_local_and_remote() -> Result<()> {
    use app_test_support::MockResponsesConfig;
    use app_test_support::TestAppServer;
    use app_test_support::create_escalated_command_execution_sse_response;
    use app_test_support::create_final_assistant_message_sse_response;
    use codex_app_server_protocol::ApprovalsReviewer::AutoReview;
    use codex_app_server_protocol::ApprovalsReviewer::User;
    use codex_app_server_protocol::AskForApproval::Never;
    use codex_app_server_protocol::AskForApproval::OnRequest;
    use codex_app_server_protocol::ThreadResumeParams;
    use codex_app_server_protocol::ThreadResumeResponse;
    use codex_app_server_protocol::ThreadStartParams;
    use codex_app_server_protocol::TurnStartParams;
    use codex_app_server_protocol::UserInput;
    use core_test_support::responses;
    use tokio::process::Command;

    let home = socket_test_home()?;
    let codex = codex_utils_cargo_bin::cargo_bin("codex")?;
    let model = responses::start_mock_server().await;
    let server_config = || {
        MockResponsesConfig::new(&model.uri())
            .with_root_config("features.plugins = false\nanalytics.enabled = false")
    };
    let human_review = r#"approvals_reviewer="user""#;
    server_config()
        .with_root_config(human_review)
        .with_approval_policy("never")
        .write(home.path())?;
    let config = core_test_support::load_default_config_for_test(&home).await;
    let mut model_info =
        codex_core::test_support::construct_model_info_offline("mock-model", &config);
    model_info.auto_review_model_override = Some("mock-model".into());
    app_test_support::write_models_cache_with_models(home.path(), vec![model_info]).await?;
    let server = || {
        TestAppServer::builder()
            .with_program(&codex)
            .with_codex_home(home.path())
            .with_plugin_startup_tasks()
            .without_managed_config()
            .with_args(&["app-server"])
    };
    let done = create_final_assistant_message_sse_response("done")?;
    let mut seed = server().build_initialized().await?;
    let mut seed_thread = async |reviewer| -> Result<String> {
        let id = seed
            .start_thread(ThreadStartParams {
                approval_policy: Some(OnRequest),
                approvals_reviewer: Some(reviewer),
                ..Default::default()
            })
            .await?
            .thread
            .id;
        let _mock = responses::mount_sse_once(&model, done.clone()).await;
        seed.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: id.clone(),
            input: vec![UserInput::Text {
                text: "seed saved settings".into(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
        Ok(id)
    };
    let human = seed_thread(User).await?;
    let auto = seed_thread(AutoReview).await?;
    drop(seed);

    let deny = create_final_assistant_message_sse_response(
        r#"{"risk_level":"high","user_authorization":"low","outcome":"deny","rationale":"Test reviewer denied this action."}"#,
    )?;
    let auto_review = r#"approvals_reviewer="auto_review""#;
    let never = r#"approval_policy="never""#;
    // Check warning counts on every turn. Only request escalation when checking the denial.
    let human_denial = (1, Some("approval request failed"));
    let auto_denial = (0, Some("test reviewer denied this action"));
    let never_denial = (0, Some("approval policy is never"));
    let human_warning = (1, None);
    let mut count = 0;
    let mut run = async |args: &[&str], expected| -> Result<(String, String)> {
        let (expected_warnings, denial): (usize, Option<&str>) = expected;
        count += 1;
        let call_id = format!("action_{count}");
        let mut bodies = Vec::new();
        if denial.is_some() {
            bodies.push(create_escalated_command_execution_sse_response(
                vec!["echo".into(), call_id.clone()],
                /*workdir*/ None,
                /*timeout_ms*/ Some(500),
                &call_id,
            )?);
            if expected == auto_denial {
                bodies.push(deny.clone());
            }
        }
        bodies.push(done.clone());
        let mock = responses::mount_sse_sequence(&model, bodies).await;
        let output = Command::new(&codex)
            .env("CODEX_HOME", home.path())
            .current_dir(home.path())
            .arg("exec")
            .args(args)
            .args(["--skip-git-repo-check", "--json", "test approval"])
            .output()
            .await?;
        let stderr = String::from_utf8(output.stderr)?;
        assert!(output.status.success(), "{args:?}: {stderr}");
        let header = "WARNING: This thread is configured to ask a human";
        let warnings = stderr.matches(header).count();
        assert_eq!(warnings, expected_warnings, "{args:?}: {stderr}");
        if let Some(expected) = denial {
            let denied = mock
                .function_call_output_text(&call_id)
                .context("missing denied tool output")?
                .to_lowercase();
            assert!(denied.contains(expected), "{args:?}: {denied}");
        }
        let events = std::str::from_utf8(&output.stdout)?
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<serde_json::Result<Vec<_>>>()?;
        let id = serde_json::from_value(events[0]["thread_id"].clone())?;
        let warning = stderr
            .lines()
            .skip_while(|line| !line.starts_with(header))
            .take_while(|line| line.starts_with(header) || line.starts_with("- "))
            .collect::<Vec<_>>()
            .join("\n");
        Ok((id, warning))
    };
    let (_, warning) = run(&["resume", &human], human_denial).await?;
    insta::assert_snapshot!(warning, @r#"
    WARNING: This thread is configured to ask a human for approval. `codex exec` runs non-interactively and immediately rejects approval requests it receives.
    - To review requests yourself, run this turn from the TUI.
    - To change the thread's saved reviewer, pass `--config 'approvals_reviewer="auto_review"'` to `codex exec resume`.
    - To disable approval prompts for this and future turns, pass `--config 'approval_policy="never"'` to `codex exec resume`.
    "#);
    run(&["resume", &auto], auto_denial).await?;

    // An explicit change persists. Each resume inherits the field left unspecified.
    let on_request = r#"approval_policy="on-request""#;
    let (explicit_fresh, _) = run(&["-c", on_request], human_denial).await?;
    let (fresh, _) = run(&[], never_denial).await?;
    run(&["-c", on_request, "resume", &fresh], human_warning).await?;
    run(&["-c", auto_review, "resume", &fresh], auto_denial).await?;
    std::fs::write(home.path().join("bot.config.toml"), never)?;
    run(&["--profile", "bot", "resume", &fresh], never_denial).await?;

    // Forks inherit each field independently. The source threads must stay unchanged.
    let (inherited_fork, _) = run(&["fork", &auto], auto_denial).await?;
    let (human_fork, _) = run(&["fork", &human], human_warning).await?;
    let (reviewer_fork, _) = run(&["-c", auto_review, "fork", &human], auto_denial).await?;
    let (never_fork, _) = run(&["-c", never, "fork", &auto], never_denial).await?;

    let socket = codex_app_server::app_server_control_socket_path(home.path())?;
    let endpoint = format!("unix://{}", socket.as_path().display());
    let remote = server().with_args(&["--listen", "unix://"]).build().await?;
    wait_for_socket(socket.as_path()).await?;
    let (remote_fresh, _) = run(&["--remote", &endpoint], never_denial).await?;
    let (_, warning) = run(&["--remote", &endpoint, "resume", &human], human_denial).await?;
    insta::assert_snapshot!(warning, @r#"
    WARNING: This thread is configured to ask a human for approval. `codex exec` runs non-interactively and immediately rejects approval requests it receives.
    - To review requests yourself, run this turn from the TUI.
    "#);
    run(&["--remote", &endpoint, "resume", &auto], auto_denial).await?;

    // Fresh threads use the configured policy and reviewer on the first turn.
    server_config()
        .with_root_config(human_review)
        .with_approval_policy("on-request")
        .write(home.path())?;
    let (local_human, _) = run(&[], human_denial).await?;
    let (remote_human, _) = run(&["--remote", &endpoint], human_denial).await?;
    server_config()
        .with_approval_policy("on-request")
        .with_root_config(auto_review)
        .write(home.path())?;
    let (remote_auto, _) = run(&["--remote", &endpoint], auto_denial).await?;
    drop(remote);

    let automatic = (OnRequest, AutoReview);
    let manual = (OnRequest, User);
    let default = (Never, User);
    let mut cold = server().build_initialized().await?;
    for (id, saved) in [
        (human, manual),
        (auto, automatic),
        (explicit_fresh, manual),
        (fresh, (Never, AutoReview)),
        (remote_fresh, default),
        (local_human, manual),
        (remote_human, manual),
        (remote_auto, automatic),
        (inherited_fork, automatic),
        (human_fork, manual),
        (reviewer_fork, automatic),
        (never_fork, (Never, AutoReview)),
    ] {
        let request = cold
            .send_thread_resume_request(ThreadResumeParams {
                thread_id: id,
                exclude_turns: true,
                ..Default::default()
            })
            .await?;
        let resumed: ThreadResumeResponse = cold.read_response(request).await?;
        assert_eq!((resumed.approval_policy, resumed.approvals_reviewer), saved);
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn remote_session_commands_with_workload_identity_use_server_auth() -> Result<()> {
    use std::time::Duration;

    use codex_app_server_client::AppServerEvent;
    use codex_app_server_protocol::ClientRequest;
    use codex_app_server_protocol::RequestId;
    use codex_app_server_protocol::ServerNotification;
    use codex_app_server_protocol::ThreadListParams;
    use codex_app_server_protocol::ThreadListResponse;
    use codex_app_server_protocol::ThreadQueueListParams;
    use codex_app_server_protocol::ThreadQueueListResponse;
    use codex_app_server_protocol::ThreadReadParams;
    use codex_app_server_protocol::ThreadReadResponse;
    use codex_app_server_protocol::ThreadSetNameParams;
    use codex_app_server_protocol::ThreadSetNameResponse;
    use codex_app_server_protocol::ThreadStatus;
    use codex_app_server_protocol::TurnStatus;
    use codex_protocol::shell_environment::OPENAI_FEDERATION_RULE_ID_ENV_VAR;
    use codex_protocol::shell_environment::OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR;
    use tokio::process::Command;
    use tokio::time::timeout;

    let codex_home = socket_test_home()?;
    let codex = codex_utils_cargo_bin::cargo_bin("codex")?;
    let (fixture, mut app, thread_id) = remote_exec_support::RemoteExecFixture::start(
        &codex,
        codex_home.path(),
        "queued message consumed",
        /*expected_requests*/ 4,
        |config| config.with_provider_config("env_key = \"CODEX_QUEUE_SERVER_API_KEY\""),
        &[
            ("OPENAI_API_KEY", None),
            ("CODEX_API_KEY", None),
            ("CODEX_ACCESS_TOKEN", None),
            (OPENAI_FEDERATION_RULE_ID_ENV_VAR, None),
            (OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR, None),
            ("CODEX_QUEUE_SERVER_API_KEY", Some("server-only-test-key")),
        ],
    )
    .await?;
    let endpoint = &fixture.endpoint;
    let message = "synthetic WIF queue end-to-end message";
    let output = timeout(
        Duration::from_secs(30),
        Command::new(&codex)
            .env("CODEX_HOME", codex_home.path())
            .env(OPENAI_FEDERATION_RULE_ID_ENV_VAR, "rule-test")
            .env(
                OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR,
                codex_home.path().join("missing-identity-token"),
            )
            .env_remove("CODEX_QUEUE_SERVER_API_KEY")
            .kill_on_drop(true)
            .args(["queue", "--resume", "--remote", "unix://"])
            .args(["--thread", &thread_id, "--message", message])
            .output(),
    )
    .await??;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let completed = timeout(Duration::from_secs(30), async {
        while let Some(event) = app.next_event().await {
            if let AppServerEvent::ServerNotification(notification) = event
                && let ServerNotification::TurnCompleted(completed) = *notification
            {
                return Ok::<_, anyhow::Error>(completed);
            }
        }
        anyhow::bail!("app server disconnected before the queued turn completed")
    })
    .await??;
    assert_eq!(completed.thread_id, thread_id);
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    let queue: ThreadQueueListResponse = app
        .request_typed(ClientRequest::ThreadQueueList {
            request_id: RequestId::Integer(2),
            params: ThreadQueueListParams {
                thread_id: thread_id.clone(),
                cursor: None,
                limit: None,
            },
        })
        .await?;
    assert!(queue.data.is_empty());

    let requests = fixture.requests.received_requests().await;
    let request = requests
        .first()
        .context("queued message did not reach the model")?;
    assert!(
        request.body_json::<Value>()?["input"]
            .to_string()
            .contains(message)
    );
    assert_eq!(
        request.headers["authorization"].to_str()?,
        "Bearer server-only-test-key"
    );
    let metadata: Value = serde_json::from_str(request.headers["x-codex-turn-metadata"].to_str()?)?;
    assert_eq!(metadata["turn_trigger"], "queue");

    for args in [
        vec!["exec", "--remote", &endpoint, "--json", "new task"],
        vec![
            "exec",
            "resume",
            "--remote",
            &endpoint,
            &thread_id,
            "continue task",
        ],
    ] {
        let output = fixture
            .caller_command()
            .env_remove("CODEX_QUEUE_SERVER_API_KEY")
            .args(args)
            .output()
            .await?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout)?;
        assert!(stdout.contains("queued message consumed"));
        if stdout.starts_with('{') {
            let events = stdout
                .lines()
                .map(serde_json::from_str::<Value>)
                .collect::<serde_json::Result<Vec<_>>>()?;
            assert_ne!(events[0]["thread_id"], thread_id);
            assert!(events.iter().any(|event| event["type"] == "turn.completed"));
        } else {
            let stderr = String::from_utf8(output.stderr)?;
            let header = stderr
                .lines()
                .find(|line| line.starts_with("thread id: "))
                .context("missing remote exec header")?
                .replace(&thread_id, "[thread-id]");
            insta::assert_snapshot!(header, @"thread id: [thread-id]");
        }
    }
    let shared: ThreadReadResponse = app
        .request_typed(ClientRequest::ThreadRead {
            request_id: RequestId::Integer(20),
            params: ThreadReadParams {
                thread_id: thread_id.clone(),
                include_turns: true,
            },
        })
        .await?;
    assert_eq!(shared.thread.turns.len(), 2);

    let name = "remote-session-lifecycle";
    let _: ThreadSetNameResponse = app
        .request_typed(ClientRequest::ThreadSetName {
            request_id: RequestId::Integer(3),
            params: ThreadSetNameParams {
                thread_id: thread_id.clone(),
                name: name.to_string(),
            },
        })
        .await?;
    // A separate caller home makes name resolution rely on the selected server.
    let caller_home = TempDir::new()?;
    let remote_command = || {
        let mut command = Command::new(&codex);
        command
            .env("CODEX_HOME", caller_home.path())
            .current_dir(caller_home.path())
            .env(OPENAI_FEDERATION_RULE_ID_ENV_VAR, "rule-test")
            .env(
                OPENAI_IDENTITY_TOKEN_FILE_ENV_VAR,
                caller_home.path().join("missing-identity-token"),
            )
            .env_remove("CODEX_QUEUE_SERVER_API_KEY")
            .kill_on_drop(true)
            .args(["--remote", endpoint]);
        command
    };
    let output = timeout(
        Duration::from_secs(30),
        remote_command().args(["delete", &thread_id]).output(),
    )
    .await??;
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)?.contains("cannot confirm session deletion"));

    let cold_message = "Continue after the thread was unloaded";
    let cases: &[(&[&str], Option<bool>)] = &[
        (&["archive", name], Some(true)),
        (&["unarchive", name], Some(false)),
        (&["delete", "--force", &thread_id], None),
    ];
    for &(args, expected_archived) in cases {
        let output = timeout(
            Duration::from_secs(30),
            remote_command().args(args).output(),
        )
        .await??;
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        for archived in [false, true] {
            let threads: ThreadListResponse = app
                .request_typed(ClientRequest::ThreadList {
                    request_id: RequestId::Integer(4),
                    params: ThreadListParams {
                        include_attached_threads: false,
                        current_account_only: false,
                        included_thread_ids: None,
                        excluded_thread_ids: None,
                        attached_to_thread_id: None,
                        cursor: None,
                        limit: None,
                        sort_key: None,
                        sort_direction: None,
                        model_providers: None,
                        source_kinds: None,
                        originators: None,
                        archived: Some(archived),
                        section_id: None,
                        project_id: None,
                        cwd: None,
                        use_state_db_only: false,
                        search_term: None,
                        parent_thread_id: None,
                        ancestor_thread_id: None,
                    },
                })
                .await?;
            let thread = threads.data.iter().find(|thread| thread.id == thread_id);
            assert_eq!(
                thread.is_some(),
                expected_archived == Some(archived),
                "{args:?}: archived={archived}"
            );
            if expected_archived == Some(false)
                && let Some(thread) = thread
            {
                assert_eq!(thread.status, ThreadStatus::NotLoaded);
            }
        }
        let Some(archived) = expected_archived else {
            continue;
        };
        // Archived threads are reachable by ID; active threads can also be targeted by name.
        let (target, text) = if archived {
            (thread_id.as_str(), "still archived")
        } else {
            (name, cold_message)
        };
        let output = remote_command()
            .args(["queue", "--resume", "--thread", target, "--message", text])
            .output()
            .await?;
        if archived {
            assert!(!output.status.success());
            assert!(
                String::from_utf8(output.stderr)?
                    .contains(&format!("session {thread_id} is archived"))
            );
        } else {
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            timeout(
                Duration::from_secs(30),
                fixture.requests.wait_until_satisfied(),
            )
            .await?;
            let requests = fixture.requests.received_requests().await;
            assert!(
                requests[3].body_json::<Value>()?["input"]
                    .to_string()
                    .contains(cold_message)
            );
            // The mock receives the request before the resumed turn finishes. Deletion
            // requires an idle thread, and archiving removed our original subscription.
            timeout(Duration::from_secs(30), async {
                loop {
                    let read: ThreadReadResponse = app
                        .request_typed(ClientRequest::ThreadRead {
                            request_id: RequestId::Integer(5),
                            params: ThreadReadParams {
                                thread_id: thread_id.clone(),
                                include_turns: true,
                            },
                        })
                        .await?;
                    if let Some(last) = read.thread.turns.last()
                        && last.id != completed.turn.id
                        && last.status == TurnStatus::Completed
                        && matches!(
                            read.thread.status,
                            ThreadStatus::Idle | ThreadStatus::NotLoaded
                        )
                    {
                        return anyhow::Ok(());
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            })
            .await
            .context("resumed queued turn did not finish before deletion")??;
        }
    }
    Ok(())
}

#[test]
fn remote_exec_rejects_conflicting_endpoints() -> Result<()> {
    for command in [vec!["exec"], vec!["exec", "resume"]] {
        let output = std::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
            .env_remove("CODEX_TEST_REMOTE_TOKEN")
            .args([
                "--remote",
                "wss://a.example:443",
                "--remote-auth-token-env",
                "CODEX_TEST_REMOTE_TOKEN",
            ])
            .args(command)
            .args(["--remote", "wss://b.example:443"])
            .output()?;
        assert!(!output.status.success());
        assert!(String::from_utf8(output.stderr)?.contains("conflicting remote server endpoints"));
    }
    Ok(())
}

#[test]
fn tui_session_commands_reject_conflicting_remotes_before_token_lookup() -> Result<()> {
    const MISSING_TOKEN: &str = "CODEX_TEST_MISSING_TUI_REMOTE_TOKEN";
    for command in ["resume", "fork"] {
        let output = std::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
            .env_remove(MISSING_TOKEN)
            .args([
                "--remote",
                "wss://a.example:443",
                "--remote-auth-token-env",
                MISSING_TOKEN,
                command,
                "--remote",
                "wss://b.example:443",
                THREAD_ID,
            ])
            .output()?;
        let stderr = String::from_utf8(output.stderr)?;
        assert!(!output.status.success());
        assert!(
            stderr.contains(&format!(
                "`codex {command}` received conflicting remote server endpoints"
            )),
            "{command}: {stderr}"
        );
        assert!(!stderr.contains(MISSING_TOKEN), "{command}: {stderr}");
    }
    Ok(())
}

#[test]
fn remote_session_commands_validate_config() -> Result<()> {
    let cases: &[(&[&str], &str, &str)] = &[
        (
            &["--strict-config"],
            "config.toml",
            "unknown_queue_option = true\n",
        ),
        (&["--profile", "queue"], "queue.config.toml", "model = 42\n"),
        (
            &[],
            "config.toml",
            "experimental_thread_store_endpoint = \"https://example.com\"\n",
        ),
    ];
    let commands: &[&[&str]] = &[
        &["queue", "--thread", THREAD_ID, "--message", "do the thing"],
        &["archive", THREAD_ID],
        &["unarchive", THREAD_ID],
        &["delete", "--force", THREAD_ID],
    ];
    for &command in commands {
        for &(args, config_file, config) in cases {
            let codex_home = TempDir::new()?;
            std::fs::write(codex_home.path().join(config_file), config)?;
            let output = std::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
                .env("CODEX_HOME", codex_home.path())
                .current_dir(codex_home.path())
                .args(args)
                .args(["--remote", "ws://127.0.0.1:1"])
                .args(command)
                .output()?;
            let stderr = String::from_utf8(output.stderr)?;
            assert!(!output.status.success());
            assert!(
                stderr.contains("failed to load config.toml"),
                "{command:?}, {config:?}: {stderr}"
            );
        }
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn queue_rejects_local_daemon_that_does_not_support_queueing() -> Result<()> {
    let codex_home = socket_test_home()?;
    let socket_path = codex_app_server::app_server_control_socket_path(codex_home.path())?;
    std::fs::create_dir_all(
        socket_path
            .as_path()
            .parent()
            .context("missing socket parent")?,
    )?;
    let listener = tokio::net::UnixListener::bind(socket_path.as_path())?;
    let server_home = codex_home.path().to_path_buf();
    let server = tokio::spawn(async move {
        let (probe, _) = listener.accept().await?;
        drop(probe);
        let (stream, _) = listener.accept().await?;
        respond_to_queue_request(
            stream,
            server_home.as_path(),
            QueueResponse::UnknownMethodVariant,
        )
        .await
    });

    let output = tokio::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .env("CODEX_HOME", codex_home.path())
        .args(["queue", "--thread", THREAD_ID, "--message", "do the thing"])
        .output()
        .await?;
    server.await??;

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)?
            .contains("local app-server daemon does not support thread/queue/add")
    );
    assert!(!codex_home.path().join("queue_1.sqlite").exists());
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn queue_rejects_overrides_that_bypass_local_daemon() -> Result<()> {
    let codex_home = socket_test_home()?;
    let socket_path = codex_app_server::app_server_control_socket_path(codex_home.path())?;
    std::fs::create_dir_all(
        socket_path
            .as_path()
            .parent()
            .context("missing socket parent")?,
    )?;
    let listener = tokio::net::UnixListener::bind(socket_path.as_path())?;
    let server = tokio::spawn(async move {
        let (probe, _) = listener.accept().await?;
        drop(probe);
        Ok::<_, std::io::Error>(())
    });

    let output = tokio::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .env("CODEX_HOME", codex_home.path())
        .args([
            "queue",
            "-C",
            ".",
            "-c",
            "analytics.enabled=false",
            "--thread",
            THREAD_ID,
            "--message",
            "do the thing",
        ])
        .output()
        .await?;
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)?
            .contains("embedded app server while a local app-server daemon is running")
    );
    server.await??;
    Ok(())
}
