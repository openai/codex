use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_config::types::ApprovalsReviewer;
use codex_core::compact::SUMMARIZATION_PROMPT;
use codex_core::config::Constrained;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::EnvironmentReadyInfo;
use codex_exec_server::ExecServerError;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::NoiseChannelPublicKey;
use codex_exec_server::NoiseRendezvousConnectBundle;
use codex_exec_server::NoiseRendezvousConnectProvider;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_exec_server::RemoveOptions;
use codex_features::Feature;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::SandboxPermissions;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ENVIRONMENTS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::protocol::TurnEnvironmentSelections;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use core_test_support::PathBufExt;
use core_test_support::PathExt;
use core_test_support::TestTargetOs;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_apply_patch_custom_tool_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_no_remote_env;
use core_test_support::skip_if_target_windows;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::test_env;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::test_docker_container_name;
use core_test_support::test_target_os;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use futures::SinkExt;
use futures::StreamExt;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
async fn unified_exec_test(server: &wiremock::MockServer) -> Result<TestCodex> {
    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        let result = config.features.enable(Feature::UnifiedExec);
        assert!(
            result.is_ok(),
            "unified exec should enable for test: {result:?}",
        );
    });
    builder.build_with_remote_and_local_env(server).await
}

async fn submit_turn_with_approval_and_environments(
    test: &TestCodex,
    prompt: &str,
    environments: Vec<TurnEnvironmentSelection>,
    approval_policy: AskForApproval,
) -> Result<()> {
    let turn_environment_selections = codex_protocol::protocol::TurnEnvironmentSelections::new(
        test.config.cwd.clone(),
        environments,
    );
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(turn_environment_selections),
                approval_policy: Some(approval_policy),
                approvals_reviewer: Some(ApprovalsReviewer::User),
                sandbox_policy: Some(SandboxPolicy::new_read_only_policy()),
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: test.session_configured.model.clone(),
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    Ok(())
}

async fn expect_patch_approval(
    test: &TestCodex,
    expected_call_id: &str,
) -> ApplyPatchApprovalRequestEvent {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ApplyPatchApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::ApplyPatchApprovalRequest(approval) => {
            assert_eq!(approval.call_id, expected_call_id);
            approval
        }
        EventMsg::TurnComplete(_) => panic!("expected patch approval request before completion"),
        other => panic!("unexpected event: {other:?}"),
    }
}

async fn wait_for_completion_without_patch_approval(test: &TestCodex) {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ApplyPatchApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::TurnComplete(_) => {}
        EventMsg::ApplyPatchApprovalRequest(event) => {
            panic!("unexpected patch approval request: {:?}", event.call_id)
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_can_connect_and_use_filesystem() -> Result<()> {
    skip_if_no_remote_env!(Ok(()));

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let file_path_uri = test_env.selection().cwd.join("remote-test-env-ok")?;
    let payload = b"remote-test-env-ok".to_vec();

    file_system
        .write_file(&file_path_uri, payload.clone(), /*sandbox*/ None)
        .await?;
    let actual = file_system
        .read_file(&file_path_uri, /*sandbox*/ None)
        .await?;
    assert_eq!(actual, payload);

    file_system
        .remove(
            &file_path_uri,
            RemoveOptions {
                recursive: false,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_exposes_target_shell_to_model() -> Result<()> {
    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let test = test_codex().build_with_auto_env(&server).await?;

    test.submit_turn("report remote environment").await?;

    let request = response_mock.single_request();
    let environment_context = request
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.starts_with("<environment_context>"))
        .context("environment context should be model visible")?;
    // TODO(anp): Assert Wine-exec exposes a `C:\\...` cwd after model-visible paths preserve
    // target-native spelling instead of the Linux orchestrator's `/C:/...` representation.
    let expected_shell = match test_target_os() {
        TestTargetOs::Linux => "<shell>bash</shell>",
        TestTargetOs::Windows => "<shell>powershell</shell>",
        TestTargetOs::MacOs => unreachable!("remote test targets do not run macOS"),
    };
    assert_eq!(
        environment_context
            .lines()
            .find(|line| line.trim_start().starts_with("<shell>"))
            .map(str::trim),
        Some(expected_shell),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_remote_shell_runs_in_remote_cwd() -> Result<()> {
    const CALL_ID: &str = "remote-explicit-shell";

    skip_if_no_remote_env!(Ok(()));

    let (shell, command) = match test_target_os() {
        TestTargetOs::Linux => (
            "bash",
            r#"case "$PWD" in /tmp/codex-core-test-cwd-*) ;; *) echo "unexpected cwd: $PWD" >&2; exit 1 ;; esac"#,
        ),
        TestTargetOs::Windows => (
            "powershell",
            r#"$cwd = (Get-Location).Path; if ($cwd -notlike 'C:\codex-core-test-cwd-*') { Write-Error "unexpected cwd: $cwd"; exit 1 }"#,
        ),
        TestTargetOs::MacOs => unreachable!("remote test targets do not run macOS"),
    };

    let server = start_mock_server().await;
    let arguments = serde_json::to_string(&json!({
        "cmd": command,
        "shell": shell,
        "login": false,
        "yield_time_ms": 10_000,
    }))?;
    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config
            .features
            .enable(Feature::UnifiedExec)
            .expect("test config should allow feature update");
    });
    let test = builder.build_with_auto_env(&server).await?;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(CALL_ID, "exec_command", &arguments),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn_with_environments(
        "run the remote shell in the remote cwd",
        Some(vec![TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&test.config.cwd),
            workspace_roots: vec![PathUri::from_abs_path(&test.config.cwd)],
        }]),
    )
    .await?;
    let request = response_mock
        .last_request()
        .context("model should receive the command output")?;
    let (output, success) = request
        .function_call_output_content_and_success(CALL_ID)
        .context("remote shell tool result should be present")?;
    assert_ne!(success, Some(false));
    assert!(
        output.is_some_and(|output| output.contains("Process exited with code 0")),
        "remote shell command should exit successfully",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn step_world_state_does_not_duplicate_initial_environment_context() -> Result<()> {
    for deferred_executor_enabled in [false, true] {
        let server = start_mock_server().await;
        let response_mock = mount_sse_once(
            &server,
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-1"),
            ]),
        )
        .await;
        let mut builder = test_codex().with_config(move |config| {
            if deferred_executor_enabled {
                assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
            }
        });
        let test = builder.build(&server).await?;

        test.submit_turn("report the environment").await?;

        let user_context = response_mock.single_request().message_input_texts("user");
        assert_eq!(
            user_context
                .iter()
                .filter(|text| text.contains("<environment_context>"))
                .count(),
            1,
            "deferred executor enabled: {deferred_executor_enabled}",
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn settings_update_does_not_retarget_active_turn_environment() -> Result<()> {
    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    "pause-turn",
                    "request_user_input",
                    &json!({
                        "questions": [{
                            "id": "continue",
                            "header": "Continue",
                            "question": "Continue after settings update?",
                            "options": [{
                                "label": "Yes (Recommended)",
                                "description": "Continue the test."
                            }, {
                                "label": "No",
                                "description": "Stop the test."
                            }]
                        }]
                    })
                    .to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "first turn done"),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-3", "second turn done"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex().with_config(|config| {
        assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
        assert!(
            config
                .features
                .enable(Feature::DefaultModeRequestUserInput)
                .is_ok()
        );
    });
    let test = builder.build(&server).await?;
    let initial_cwd = test.config.cwd.clone();
    let next_workspace = TempDir::new()?;
    let next_cwd = next_workspace.path().abs();

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "pause before continuing".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;

    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            environments: Some(TurnEnvironmentSelections::new(
                next_cwd.clone(),
                vec![local(next_cwd.clone())],
            )),
            ..Default::default()
        },
    )
    .await?;
    test.codex
        .submit(Op::UserInputAnswer {
            id: request.turn_id,
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    "continue".to_string(),
                    RequestUserInputAnswer {
                        answers: vec!["Yes (Recommended)".to_string()],
                    },
                )]),
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_turn("start the next turn").await?;

    let request_texts = response_mock
        .requests()
        .iter()
        .map(|request| request.message_input_texts("user").join("\n"))
        .collect::<Vec<_>>();
    let initial_cwd = format!("<cwd>{}</cwd>", initial_cwd.as_path().display());
    let next_cwd = format!("<cwd>{}</cwd>", next_cwd.as_path().display());
    assert_eq!(
        request_texts
            .iter()
            .map(|text| text.contains(&next_cwd))
            .collect::<Vec<_>>(),
        vec![false, false, true]
    );
    assert!(request_texts[0].contains(&initial_cwd));
    assert!(request_texts[1].contains(&initial_cwd));

    Ok(())
}

async fn read_exec_server_json(websocket: &mut WebSocketStream<TcpStream>) -> Value {
    loop {
        match timeout(Duration::from_secs(5), websocket.next())
            .await
            .expect("websocket read should not time out")
            .expect("websocket should stay open")
            .expect("websocket frame should read")
        {
            Message::Text(text) => {
                return serde_json::from_str(text.as_ref()).expect("valid JSON-RPC message");
            }
            Message::Binary(bytes) => {
                return serde_json::from_slice(bytes.as_ref()).expect("valid JSON-RPC message");
            }
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("expected JSON-RPC message, got {other:?}"),
        }
    }
}

async fn accept_initialized_exec_server(listener: TcpListener) -> WebSocketStream<TcpStream> {
    let (stream, _) = listener.accept().await.expect("connection");
    let mut websocket = accept_async(stream).await.expect("websocket handshake");

    let initialize = read_exec_server_json(&mut websocket).await;
    assert_eq!(initialize["method"], "initialize");
    websocket
        .send(Message::Text(
            json!({
                "id": initialize["id"],
                "result": { "sessionId": "test-session" }
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("initialize response");
    let initialized = read_exec_server_json(&mut websocket).await;
    assert_eq!(initialized["method"], "initialized");

    websocket
}

async fn send_environment_info(websocket: &mut WebSocketStream<TcpStream>) {
    let info = read_exec_server_json(websocket).await;
    assert_eq!(info["method"], "environment/info");
    websocket
        .send(Message::Text(
            json!({
                "id": info["id"],
                "result": { "shell": { "name": "zsh", "path": "/bin/zsh" } }
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("environment info response");
}

async fn serve_environment_info(listener: TcpListener) {
    let mut websocket = accept_initialized_exec_server(listener).await;
    send_environment_info(&mut websocket).await;
}

async fn serve_environment_with_agents_md(
    listener: TcpListener,
    contents: &str,
    attach: tokio::sync::oneshot::Receiver<()>,
    mut shutdown: tokio::sync::oneshot::Receiver<()>,
) -> usize {
    let mut websocket = accept_initialized_exec_server(listener).await;
    attach.await.expect("attach signal");
    send_environment_info(&mut websocket).await;

    let mut agents_md_reads = 0;
    loop {
        let request = tokio::select! {
            request = read_exec_server_json(&mut websocket) => request,
            _ = &mut shutdown => return agents_md_reads,
        };
        let is_agents_md = request["params"]["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("/AGENTS.md"));
        let response = match request["method"].as_str() {
            Some("fs/getMetadata") if is_agents_md => {
                json!({
                    "id": request["id"],
                    "result": {
                        "isDirectory": false,
                        "isFile": true,
                        "isSymlink": false,
                        "size": contents.len(),
                        "createdAtMs": 0,
                        "modifiedAtMs": 0,
                    }
                })
            }
            Some("fs/getMetadata") => json!({
                "id": request["id"],
                "error": { "code": -32004, "message": "not found" }
            }),
            Some("fs/readFile") if is_agents_md => {
                agents_md_reads += 1;
                json!({
                    "id": request["id"],
                    "result": { "dataBase64": BASE64_STANDARD.encode(contents) }
                })
            }
            method => panic!("unexpected exec-server request: {method:?}"),
        };
        websocket
            .send(Message::Text(response.to_string().into()))
            .await
            .expect("filesystem response");
    }
}

fn tool_names(body: &Value) -> Vec<String> {
    body["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_owned))
        .collect()
}

#[derive(Default)]
struct FailingNoiseConnectProvider {
    calls: AtomicUsize,
}

impl NoiseRendezvousConnectProvider for FailingNoiseConnectProvider {
    fn connect_bundle(
        &self,
        _: NoiseChannelPublicKey,
    ) -> BoxFuture<'_, std::result::Result<NoiseRendezvousConnectBundle, ExecServerError>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Box::pin(async {
            Err(ExecServerError::Protocol(
                "test Noise connection failed".to_string(),
            ))
        })
    }
}

async fn wait_for_response_request_count(response_mock: &ResponseMock, expected_count: usize) {
    timeout(Duration::from_secs(5), async {
        while response_mock.requests().len() < expected_count {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timed out waiting for Responses API request");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deferred_executor_starts_noise_connection_after_registration() -> Result<()> {
    let server = start_mock_server().await;
    let wait_call_id = "wait-for-startup";
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    wait_call_id,
                    "wait_for_environment",
                    &json!({
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                    })
                    .to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
        assert!(config.features.enable(Feature::UnifiedExec).is_ok());
    });
    let test = timeout(Duration::from_secs(5), builder.build(&server))
        .await
        .context("thread startup should not wait for the remote environment")??;
    let environment_manager = test.thread_manager.environment_manager();
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    let registration = environment_manager
        .register_deferred_noise_environment(REMOTE_ENVIRONMENT_ID.to_string(), provider.clone())?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "wait for the environment".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentSelections::new(
                    test.config.cwd.clone(),
                    vec![TurnEnvironmentSelection {
                        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                        cwd: PathUri::from_abs_path(&test.config.cwd),
                        workspace_roots: vec![PathUri::from_abs_path(&test.config.cwd)],
                    }],
                )),
                ..Default::default()
            },
        })
        .await?;
    wait_for_response_request_count(&response_mock, /*expected_count*/ 1).await;
    assert_eq!(response_mock.requests().len(), 1);
    assert_eq!(provider.calls.load(Ordering::Relaxed), 0);
    registration.complete(Ok(EnvironmentReadyInfo::default()))?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(provider.calls.load(Ordering::Relaxed), 1);

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let starting_tools = tool_names(&requests[0].body_json());
    assert!(starting_tools.contains(&"wait_for_environment".to_string()));
    assert!(!starting_tools.contains(&"exec_command".to_string()));
    let (wait_output, _) = requests[1]
        .function_call_output_content_and_success(wait_call_id)
        .context("wait_for_environment output should be present")?;
    assert!(
        wait_output
            .context("wait output should contain text")?
            .contains("failed to start")
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deferred_executor_loads_agents_md_when_environment_becomes_ready() -> Result<()> {
    const AGENTS_CONTENT: &str = "REMOTE_AGENTS_INSTRUCTIONS";

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    "wait-1",
                    "wait_for_environment",
                    &json!({ "environment_id": REMOTE_ENVIRONMENT_ID }).to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_function_call(
                    "wait-2",
                    "wait_for_environment",
                    &json!({ "environment_id": REMOTE_ENVIRONMENT_ID }).to_string(),
                ),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-3", "done"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex()
        .with_exec_server_url(format!("ws://{}", listener.local_addr()?))
        .with_config(|config| {
            assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
        });
    let (attach_tx, attach_rx) = tokio::sync::oneshot::channel();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let exec_server = tokio::spawn(serve_environment_with_agents_md(
        listener,
        AGENTS_CONTENT,
        attach_rx,
        shutdown_rx,
    ));
    let test = timeout(Duration::from_secs(5), builder.build(&server))
        .await
        .context("thread startup should not wait for the remote environment")??;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "load the environment instructions".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_response_request_count(&response_mock, /*expected_count*/ 1).await;
    let agents_path = PathUri::from_abs_path(&test.config.cwd).join("AGENTS.md")?;
    attach_tx.send(()).expect("attach environment");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    shutdown_tx.send(()).expect("stop exec server");
    let agents_md_reads = exec_server.await?;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(agents_md_reads, 1);
    assert_eq!(agents_md_occurrences(&requests[0], AGENTS_CONTENT), 0);
    assert_eq!(agents_md_occurrences(&requests[1], AGENTS_CONTENT), 1);
    assert_eq!(agents_md_occurrences(&requests[2], AGENTS_CONTENT), 1);
    assert_eq!(environment_instructions_occurrences(&requests[0]), 1);
    assert_eq!(environment_instructions_occurrences(&requests[1]), 1);
    assert_eq!(environment_instructions_occurrences(&requests[2]), 1);
    assert_eq!(test.codex.instruction_sources().await, vec![agents_path]);

    Ok(())
}

fn agents_md_occurrences(request: &ResponsesRequest, contents: &str) -> usize {
    request
        .message_input_texts("user")
        .iter()
        .filter(|text| text.contains(contents))
        .count()
}

fn environment_instructions_occurrences(request: &ResponsesRequest) -> usize {
    request
        .message_input_texts("developer")
        .iter()
        .filter(|text| text.contains(ENVIRONMENTS_INSTRUCTIONS_OPEN_TAG))
        .count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deferred_executor_wait_reports_startup_failure() -> Result<()> {
    let server = start_mock_server().await;
    let wait_call_id = "wait-for-failure";
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    wait_call_id,
                    "wait_for_environment",
                    &json!({
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                    })
                    .to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
        assert!(config.features.enable(Feature::UnifiedExec).is_ok());
    });
    let test = timeout(Duration::from_secs(5), builder.build(&server))
        .await
        .context("thread startup should not wait for the remote environment")??;
    let environment_manager = test.thread_manager.environment_manager();
    let registration = environment_manager.register_deferred_noise_environment(
        REMOTE_ENVIRONMENT_ID.to_string(),
        Arc::new(FailingNoiseConnectProvider::default()),
    )?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "wait for the environment".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentSelections::new(
                    test.config.cwd.clone(),
                    vec![TurnEnvironmentSelection {
                        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                        cwd: PathUri::from_abs_path(&test.config.cwd),
                        workspace_roots: vec![PathUri::from_abs_path(&test.config.cwd)],
                    }],
                )),
                ..Default::default()
            },
        })
        .await?;
    wait_for_response_request_count(&response_mock, /*expected_count*/ 1).await;
    assert_eq!(response_mock.requests().len(), 1);
    registration.complete(Err("CCA provisioning failed".to_string()))?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let starting_tools = tool_names(&requests[0].body_json());
    let failed_tools = tool_names(&requests[1].body_json());
    assert!(starting_tools.contains(&"wait_for_environment".to_string()));
    assert!(!starting_tools.contains(&"exec_command".to_string()));
    assert!(failed_tools.contains(&"wait_for_environment".to_string()));
    assert!(!failed_tools.contains(&"exec_command".to_string()));
    let (wait_output, _) = requests[1]
        .function_call_output_content_and_success(wait_call_id)
        .context("wait_for_environment output should be present")?;
    assert_eq!(
        wait_output.as_deref(),
        Some("Environment `remote` failed to start and is unavailable. Continue without it.")
    );
    assert!(
        requests[1]
            .message_input_texts("user")
            .iter()
            .any(|text| text.contains("status=\"unavailable\""))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deferred_executor_compaction_preserves_then_updates_environment_once() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    "wait-for-startup",
                    "request_user_input",
                    &json!({
                        "questions": [{
                            "id": "continue",
                            "header": "Continue",
                            "question": "Continue after startup?",
                            "options": [{
                                "label": "Yes (Recommended)",
                                "description": "Continue the test."
                            }, {
                                "label": "No",
                                "description": "Stop the test."
                            }]
                        }]
                    })
                    .to_string(),
                ),
                ev_completed_with_tokens("resp-1", /*total_tokens*/ 96),
            ]),
            sse(vec![
                ev_assistant_message("msg-compact", "AUTO_COMPACT_SUMMARY"),
                ev_completed_with_tokens("resp-compact", /*total_tokens*/ 10),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex()
        .with_exec_server_url(format!("ws://{}", listener.local_addr()?))
        .with_config(|config| {
            config.project_doc_max_bytes = 0;
            assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
            assert!(
                config
                    .features
                    .enable(Feature::DefaultModeRequestUserInput)
                    .is_ok()
            );
            config.model_provider.name = "OpenAI (test)".to_string();
            config.compact_prompt = Some(SUMMARIZATION_PROMPT.to_string());
            config.model_context_window = Some(100);
            config.model_auto_compact_token_limit = Some(90);
        });
    let test = timeout(Duration::from_secs(5), builder.build(&server))
        .await
        .context("thread startup should not wait for the remote environment")??;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "wait for the environment".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;

    serve_environment_info(listener).await;
    test.codex
        .submit(Op::UserInputAnswer {
            id: request.turn_id,
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    "continue".to_string(),
                    RequestUserInputAnswer {
                        answers: vec!["Yes (Recommended)".to_string()],
                    },
                )]),
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 3);
    let initial_context = requests[0].message_input_texts("user");
    assert!(
        initial_context
            .iter()
            .any(|text| text.contains("<status>starting</status>"))
    );

    let post_compaction_context = requests[2].message_input_texts("user");
    assert_eq!(
        post_compaction_context
            .iter()
            .filter(|text| text.contains("<status>starting</status>"))
            .count(),
        1
    );
    assert_eq!(
        post_compaction_context
            .iter()
            .filter(|text| text.contains("<shell>zsh</shell>"))
            .count(),
        1
    );
    let starting_index = post_compaction_context
        .iter()
        .position(|text| text.contains("<status>starting</status>"))
        .expect("compaction should preserve the prior environment state");
    let ready_index = post_compaction_context
        .iter()
        .position(|text| text.contains("<shell>zsh</shell>"))
        .expect("the next sampling step should report that the environment is ready");
    assert!(starting_index < ready_index);

    test.codex.ensure_rollout_materialized().await;
    test.codex.flush_rollout().await?;
    let rollout_path = test.codex.rollout_path().context("rollout path")?;
    let rollout = fs::read_to_string(rollout_path)?;
    let world_state_items = rollout
        .lines()
        .map(serde_json::from_str::<RolloutLine>)
        .collect::<serde_json::Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::WorldState(item) => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        world_state_items
            .iter()
            .map(|item| item.full)
            .collect::<Vec<_>>(),
        vec![true, true, false]
    );
    assert_eq!(
        world_state_items[0]
            .state
            .pointer("/environments/environments/remote/status"),
        Some(&json!("starting"))
    );
    assert_eq!(
        world_state_items[2]
            .state
            .pointer("/environments/environments/remote/status"),
        Some(&json!("available"))
    );
    assert_eq!(
        world_state_items[2]
            .state
            .pointer("/environments/environments/remote/shell"),
        Some(&json!("zsh"))
    );

    Ok(())
}

fn absolute_path(path: PathBuf) -> AbsolutePathBuf {
    AbsolutePathBuf::try_from(path).expect("path should be absolute")
}

fn read_only_sandbox(readable_root: PathBuf) -> FileSystemSandboxContext {
    let readable_root = absolute_path(readable_root);
    FileSystemSandboxContext::from_permission_profile(PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: readable_root,
            },
            access: FileSystemAccessMode::Read,
        }]),
        NetworkSandboxPolicy::Restricted,
    ))
}

fn workspace_write_sandbox(writable_root: PathBuf) -> FileSystemSandboxContext {
    let writable_root = absolute_path(writable_root);
    FileSystemSandboxContext::from_permission_profile(PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: writable_root,
            },
            access: FileSystemAccessMode::Write,
        }]),
        NetworkSandboxPolicy::Restricted,
    ))
}

fn assert_normalized_path_rejected(error: &std::io::Error) {
    match error.kind() {
        std::io::ErrorKind::NotFound => assert!(
            error.to_string().contains("No such file or directory"),
            "unexpected not-found message: {error}",
        ),
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::PermissionDenied => {
            let message = error.to_string();
            assert!(
                message.contains("is not permitted")
                    || message.contains("Operation not permitted")
                    || message.contains("Permission denied"),
                "unexpected rejection message: {message}",
            );
        }
        other => panic!("unexpected normalized-path error kind: {other:?}: {error:?}"),
    }
}

fn remote_exec(script: &str) -> Result<()> {
    let container_name = test_docker_container_name()
        .context("test requires direct access to the Docker container")?;
    let output = Command::new("docker")
        .args(["exec", container_name.as_str(), "sh", "-lc", script])
        .output()?;
    assert!(
        output.status.success(),
        "remote exec failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    Ok(())
}

async fn exec_command_routing_output(
    test: &TestCodex,
    server: &wiremock::MockServer,
    call_id: &str,
    arguments: Value,
    environments: Option<Vec<TurnEnvironmentSelection>>,
) -> Result<String> {
    let response_mock = mount_sse_sequence(
        server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "exec_command", &serde_json::to_string(&arguments)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn_with_environments("route exec command", environments)
        .await?;

    response_mock
        .function_call_output_text(call_id)
        .with_context(|| format!("missing function_call_output for {call_id}"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_command_routes_to_selected_remote_environment() -> Result<()> {
    skip_if_no_network!(Ok(()));
    // TODO(anp): Remove after remote path fixtures use target-native paths.
    skip_if_target_windows!(Ok(()), "requires the Docker-backed POSIX executor");
    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let test = unified_exec_test(&server).await?;
    let local_cwd = TempDir::new()?;
    fs::write(local_cwd.path().join("marker.txt"), "local-routing")?;
    let local_selection = local(local_cwd.path().abs());
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-remote-routing-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let remote_marker_name = "marker.txt";
    let remote_cwd_uri = PathUri::from_host_native_path(&remote_cwd)?;
    let remote_marker_uri = PathUri::from_host_native_path(remote_cwd.join(remote_marker_name))?;
    test.fs()
        .create_directory(
            &remote_cwd_uri,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;
    test.fs()
        .write_file(
            &remote_marker_uri,
            b"remote-routing".to_vec(),
            /*sandbox*/ None,
        )
        .await?;
    let remote_selection = TurnEnvironmentSelection {
        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
        cwd: PathUri::from_abs_path(&remote_cwd),
        workspace_roots: vec![PathUri::from_abs_path(&remote_cwd)],
    };
    let multi_env_output = exec_command_routing_output(
        &test,
        &server,
        "call-multi-env",
        json!({
            "shell": "/bin/sh",
            "cmd": format!("cat {remote_marker_name}"),
            "login": false,
            "yield_time_ms": 1_000,
            "environment_id": REMOTE_ENVIRONMENT_ID,
        }),
        Some(vec![local_selection, remote_selection]),
    )
    .await?;
    assert!(
        multi_env_output.contains("remote-routing"),
        "unexpected multi-env output: {multi_env_output}",
    );
    assert!(
        !multi_env_output.contains("local-routing"),
        "multi-env command should not route to local: {multi_env_output}",
    );

    test.fs()
        .remove(
            &remote_cwd_uri,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_exec_materializes_target_roots_before_sandbox_selection() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_target_windows!(
        Ok(()),
        "sandboxed process launch is not supported by the exec-server Windows backend"
    );
    skip_if_no_remote_env!(Ok(()));

    const SECRET: &str = "target-root-secret";
    const SECRET_FILE: &str = "secret.txt";

    let server = start_mock_server().await;
    let test = unified_exec_test(&server).await?;
    let local_cwd = TempDir::new()?;
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-remote-target-roots-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let remote_cwd_uri = PathUri::from_abs_path(&remote_cwd);
    test.fs()
        .create_directory(
            &remote_cwd_uri,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;
    test.fs()
        .write_file(
            &remote_cwd_uri.join(SECRET_FILE)?,
            SECRET.as_bytes().to_vec(),
            /*sandbox*/ None,
        )
        .await?;

    let call_id = "remote-target-root-sandbox";
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    call_id,
                    "exec_command",
                    &json!({
                        "shell": "bash",
                        "cmd": format!("cat {SECRET_FILE}"),
                        "login": false,
                        "yield_time_ms": 1_000,
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                        "sandbox_permissions": SandboxPermissions::RequireEscalated,
                    })
                    .to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Deny,
            },
        ]),
        NetworkSandboxPolicy::Restricted,
    );
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(permission_profile, test.config.cwd.as_path());
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "try to read the denied remote workspace root".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentSelections::new(
                    test.config.cwd.clone(),
                    vec![
                        TurnEnvironmentSelection {
                            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                            cwd: PathUri::from_abs_path(&local_cwd.path().abs()),
                            workspace_roots: Vec::new(),
                        },
                        TurnEnvironmentSelection {
                            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                            cwd: remote_cwd_uri.clone(),
                            workspace_roots: vec![remote_cwd_uri.clone()],
                        },
                    ],
                )),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: test.session_configured.model.clone(),
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let request = response_mock
        .last_request()
        .context("model should receive the denied remote command output")?;
    let (output, success) = request
        .function_call_output_content_and_success(call_id)
        .context("remote command output should be model visible")?;
    assert_ne!(success, Some(true));
    assert!(
        output.is_none_or(|output| !output.contains(SECRET)),
        "denied remote workspace contents should not be readable"
    );

    test.fs()
        .remove(
            &remote_cwd_uri,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_request_permissions_grant_unblocks_later_remote_exec() -> Result<()> {
    skip_if_no_network!(Ok(()));
    // TODO(anp): Remove after remote path fixtures use target-native paths.
    skip_if_target_windows!(Ok(()), "requires the Docker-backed POSIX executor");
    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        config.approvals_reviewer = ApprovalsReviewer::User;
        config
            .features
            .enable(Feature::UnifiedExec)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::ExecPermissionApprovals)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
    });
    let test = builder.build_with_remote_and_local_env(&server).await?;

    let local_cwd = TempDir::new()?;
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-remote-request-permissions-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let relative_write_root = "granted";
    let relative_target_path = "granted/request-permissions-output.txt";
    let remote_write_root = remote_cwd.join(relative_write_root);
    let remote_target_path = remote_cwd.join(relative_target_path);
    let local_write_root = local_cwd.path().join(relative_write_root);
    let local_target_path = local_cwd.path().join(relative_target_path);
    fs::create_dir(&local_write_root)?;
    let remote_write_root_uri = PathUri::from_host_native_path(&remote_write_root)?;
    test.fs()
        .create_directory(
            &remote_write_root_uri,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;

    let expected_permissions = RequestPermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![remote_write_root.clone()]),
        )),
        ..RequestPermissionProfile::default()
    };
    let approved_response = RequestPermissionsResponse {
        permissions: expected_permissions.clone(),
        scope: PermissionGrantScope::Turn,
        strict_auto_review: false,
    };
    let command = format!(
        "printf 'remote-request-permissions-ok' > {relative_target_path} && cat {relative_target_path}"
    );
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-request-permissions-remote-1"),
                ev_function_call(
                    "permissions-call",
                    "request_permissions",
                    &json!({
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                        "reason": "Allow writing inside the selected remote environment",
                        "permissions": {
                            "file_system": {
                                "write": [relative_write_root],
                            },
                        },
                    })
                    .to_string(),
                ),
                ev_completed("resp-request-permissions-remote-1"),
            ]),
            sse(vec![
                ev_response_created("resp-request-permissions-remote-2"),
                ev_function_call(
                    "exec-call",
                    "exec_command",
                    &json!({
                        "shell": "/bin/sh",
                        "cmd": command,
                        "login": false,
                        "yield_time_ms": 1_000,
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                    })
                    .to_string(),
                ),
                ev_completed("resp-request-permissions-remote-2"),
            ]),
            sse(vec![
                ev_response_created("resp-request-permissions-remote-3"),
                ev_assistant_message("msg-request-permissions-remote-1", "done"),
                ev_completed("resp-request-permissions-remote-3"),
            ]),
        ],
    )
    .await;

    submit_turn_with_approval_and_environments(
        &test,
        "request permissions, then write in the remote environment",
        vec![
            local(local_cwd.path().abs()),
            TurnEnvironmentSelection {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                cwd: PathUri::from_abs_path(&remote_cwd),
                workspace_roots: vec![PathUri::from_abs_path(&remote_cwd)],
            },
        ],
        AskForApproval::OnRequest,
    )
    .await?;

    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::RequestPermissions(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    let EventMsg::RequestPermissions(request) = event else {
        panic!("expected remote request_permissions before completion: {event:?}");
    };
    assert_eq!(request.call_id, "permissions-call");
    assert_eq!(
        request.environment_id.as_deref(),
        Some(REMOTE_ENVIRONMENT_ID)
    );
    assert_eq!(request.cwd.as_ref(), Some(&remote_cwd));
    assert_eq!(request.permissions, expected_permissions);

    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: "permissions-call".to_string(),
            response: approved_response.clone(),
        })
        .await?;

    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    match event {
        EventMsg::TurnComplete(_) => {}
        EventMsg::ExecApprovalRequest(approval) => {
            panic!("remote request_permissions grant should preapprove exec: {approval:?}");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let permissions_output: RequestPermissionsResponse = serde_json::from_str(
        &response_mock
            .function_call_output_text("permissions-call")
            .expect("expected request_permissions output"),
    )?;
    assert_eq!(permissions_output, approved_response);
    let exec_output = response_mock
        .function_call_output_text("exec-call")
        .expect("expected exec output");
    assert!(
        exec_output.contains("remote-request-permissions-ok"),
        "unexpected exec output: {exec_output}",
    );
    assert_eq!(
        test.fs()
            .read_file_text(
                &PathUri::from_host_native_path(&remote_target_path)?,
                /*sandbox*/ None,
            )
            .await?,
        "remote-request-permissions-ok"
    );
    assert!(
        !local_target_path.exists(),
        "remote exec should not write through the local environment"
    );

    test.fs()
        .remove(
            &PathUri::from_abs_path(&remote_cwd),
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_freeform_routes_to_selected_remote_environment() -> Result<()> {
    skip_if_no_network!(Ok(()));
    // TODO(anp): Remove after remote path fixtures use target-native paths.
    skip_if_target_windows!(Ok(()), "requires the Docker-backed POSIX executor");
    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build_with_remote_and_local_env(&server).await?;
    let local_cwd = TempDir::new()?;
    let file_name = "apply_patch_remote_freeform.txt";
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-remote-apply-patch-freeform-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let remote_cwd_uri = PathUri::from_host_native_path(&remote_cwd)?;
    test.fs()
        .create_directory(
            &remote_cwd_uri,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;

    let patch = format!(
        "*** Begin Patch\n*** Environment ID: {REMOTE_ENVIRONMENT_ID}\n*** Add File: {file_name}\n+patched remote freeform\n*** End Patch"
    );
    let call_id = "apply-patch-remote-freeform";
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_apply_patch_custom_tool_call(call_id, &patch),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn_with_environments(
        "apply patch to remote environment",
        Some(vec![
            local(local_cwd.path().abs()),
            TurnEnvironmentSelection {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                cwd: PathUri::from_abs_path(&remote_cwd),
                workspace_roots: vec![PathUri::from_abs_path(&remote_cwd)],
            },
        ]),
    )
    .await?;

    let remote_contents = test
        .fs()
        .read_file_text(
            &PathUri::from_host_native_path(remote_cwd.join(file_name))?,
            /*sandbox*/ None,
        )
        .await?;
    assert_eq!(remote_contents, "patched remote freeform\n");
    assert!(
        !local_cwd.path().join(file_name).exists(),
        "freeform apply_patch should not create the file in the local environment"
    );

    test.fs()
        .remove(
            &remote_cwd_uri,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_approvals_are_remembered_per_environment() -> Result<()> {
    skip_if_no_network!(Ok(()));
    // TODO(anp): Remove after remote path fixtures use target-native paths.
    skip_if_target_windows!(Ok(()), "requires the Docker-backed POSIX executor");
    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        config.approvals_reviewer = ApprovalsReviewer::User;
    });
    let test = builder.build_with_remote_and_local_env(&server).await?;
    let local_cwd = TempDir::new()?;
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-remote-apply-patch-approval-cwd-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let remote_cwd_uri = PathUri::from_host_native_path(&remote_cwd)?;
    test.fs()
        .create_directory(
            &remote_cwd_uri,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;

    let target_path = PathBuf::from(format!(
        "/tmp/codex-apply-patch-approval-scope-{}.txt",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let target_path_uri = PathUri::from_host_native_path(&target_path)?;
    let _ = fs::remove_file(&target_path);
    test.fs()
        .remove(
            &target_path_uri,
            RemoveOptions {
                recursive: false,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    let environments = vec![
        local(local_cwd.path().abs()),
        TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&remote_cwd),
            workspace_roots: vec![PathUri::from_abs_path(&remote_cwd)],
        },
    ];
    let local_patch = format!(
        "*** Begin Patch\n*** Environment ID: {LOCAL_ENVIRONMENT_ID}\n*** Add File: {}\n+local\n*** End Patch",
        target_path.display()
    );
    let remote_patch = format!(
        "*** Begin Patch\n*** Environment ID: {REMOTE_ENVIRONMENT_ID}\n*** Add File: {}\n+remote\n*** End Patch",
        target_path.display()
    );
    let remote_update_patch = format!(
        "*** Begin Patch\n*** Environment ID: {REMOTE_ENVIRONMENT_ID}\n*** Update File: {}\n@@\n-remote\n+remote updated\n*** End Patch",
        target_path.display()
    );

    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-local-1"),
                ev_apply_patch_custom_tool_call("call-local", &local_patch),
                ev_completed("resp-local-1"),
            ]),
            sse(vec![
                ev_response_created("resp-local-2"),
                ev_assistant_message("msg-local", "done"),
                ev_completed("resp-local-2"),
            ]),
            sse(vec![
                ev_response_created("resp-remote-1"),
                ev_apply_patch_custom_tool_call("call-remote", &remote_patch),
                ev_completed("resp-remote-1"),
            ]),
            sse(vec![
                ev_response_created("resp-remote-2"),
                ev_assistant_message("msg-remote", "done"),
                ev_completed("resp-remote-2"),
            ]),
            sse(vec![
                ev_response_created("resp-remote-3"),
                ev_apply_patch_custom_tool_call("call-remote-followup", &remote_update_patch),
                ev_completed("resp-remote-3"),
            ]),
            sse(vec![
                ev_response_created("resp-remote-4"),
                ev_assistant_message("msg-remote-followup", "done"),
                ev_completed("resp-remote-4"),
            ]),
        ],
    )
    .await;

    submit_turn_with_approval_and_environments(
        &test,
        "apply patch in local environment",
        environments.clone(),
        AskForApproval::OnRequest,
    )
    .await?;
    let approval = expect_patch_approval(&test, "call-local").await;
    test.codex
        .submit(Op::PatchApproval {
            id: approval.call_id,
            decision: ReviewDecision::ApprovedForSession,
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(fs::read_to_string(&target_path)?, "local\n");

    submit_turn_with_approval_and_environments(
        &test,
        "apply patch in remote environment",
        environments.clone(),
        AskForApproval::OnRequest,
    )
    .await?;
    let approval = expect_patch_approval(&test, "call-remote").await;
    test.codex
        .submit(Op::PatchApproval {
            id: approval.call_id,
            decision: ReviewDecision::ApprovedForSession,
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(
        test.fs()
            .read_file_text(&target_path_uri, /*sandbox*/ None)
            .await?,
        "remote\n"
    );

    submit_turn_with_approval_and_environments(
        &test,
        "apply patch again in remote environment",
        environments,
        AskForApproval::OnRequest,
    )
    .await?;
    wait_for_completion_without_patch_approval(&test).await;
    assert_eq!(
        test.fs()
            .read_file_text(&target_path_uri, /*sandbox*/ None)
            .await?,
        "remote updated\n"
    );

    let _ = fs::remove_file(&target_path);
    test.fs()
        .remove(
            &target_path_uri,
            RemoveOptions {
                recursive: false,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    test.fs()
        .remove(
            &remote_cwd_uri,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_intercepted_exec_command_routes_to_selected_remote_environment() -> Result<()>
{
    skip_if_no_network!(Ok(()));
    // TODO(anp): Remove after remote path fixtures use target-native paths.
    skip_if_target_windows!(Ok(()), "requires the Docker-backed POSIX executor");
    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let test = unified_exec_test(&server).await?;
    let local_cwd = TempDir::new()?;
    let file_name = "apply_patch_remote_exec.txt";
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-remote-apply-patch-exec-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let remote_cwd_uri = PathUri::from_host_native_path(&remote_cwd)?;
    test.fs()
        .create_directory(
            &remote_cwd_uri,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;

    let patch =
        format!("*** Begin Patch\n*** Add File: {file_name}\n+patched remote exec\n*** End Patch");
    let command = format!("apply_patch <<'EOF'\n{patch}\nEOF\n");
    let call_id = "apply-patch-remote-exec";
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&json!({
                        "shell": "/bin/sh",
                        "cmd": command,
                        "login": false,
                        "yield_time_ms": 5_000,
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                    }))?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn_with_environments(
        "apply patch through exec command to remote environment",
        Some(vec![
            local(local_cwd.path().abs()),
            TurnEnvironmentSelection {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                cwd: PathUri::from_abs_path(&remote_cwd),
                workspace_roots: vec![PathUri::from_abs_path(&remote_cwd)],
            },
        ]),
    )
    .await?;

    let remote_contents = test
        .fs()
        .read_file_text(
            &PathUri::from_host_native_path(remote_cwd.join(file_name))?,
            /*sandbox*/ None,
        )
        .await?;
    assert_eq!(remote_contents, "patched remote exec\n");
    assert!(
        !local_cwd.path().join(file_name).exists(),
        "intercepted apply_patch should not create the file in the local environment"
    );

    test.fs()
        .remove(
            &remote_cwd_uri,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_sandboxed_read_allows_readable_root() -> Result<()> {
    // TODO(anp): Remove after remote sandbox fixtures use target-native paths.
    skip_if_target_windows!(Ok(()), "requires the Docker-backed POSIX executor");
    skip_if_no_network!(Ok(()));
    skip_if_no_remote_env!(Ok(()));

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let allowed_dir = PathBuf::from(format!("/tmp/codex-remote-readable-{}", std::process::id()));
    let file_path = allowed_dir.join("note.txt");
    let allowed_dir_uri = PathUri::from_host_native_path(&allowed_dir)?;
    let file_path_uri = PathUri::from_host_native_path(&file_path)?;
    file_system
        .create_directory(
            &allowed_dir_uri,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;
    file_system
        .write_file(
            &file_path_uri,
            b"sandboxed hello".to_vec(),
            /*sandbox*/ None,
        )
        .await?;

    let sandbox = read_only_sandbox(allowed_dir.clone());
    let contents = file_system
        .read_file(&file_path_uri, Some(&sandbox))
        .await?;
    assert_eq!(contents, b"sandboxed hello");

    file_system
        .remove(
            &allowed_dir_uri,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_sandboxed_read_rejects_symlink_parent_dotdot_escape() -> Result<()> {
    skip_if_target_windows!(Ok(()), "tests POSIX symlink and parent traversal semantics");
    skip_if_no_network!(Ok(()));
    skip_if_no_remote_env!(Ok(()));

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let root = PathBuf::from(format!("/tmp/codex-remote-dotdot-{}", std::process::id()));
    let allowed_dir = root.join("allowed");
    let outside_dir = root.join("outside");
    let secret_path = root.join("secret.txt");
    remote_exec(&format!(
        "rm -rf {root}; mkdir -p {allowed} {outside}; printf nope > {secret}; ln -s {outside} {allowed}/link",
        root = root.display(),
        allowed = allowed_dir.display(),
        outside = outside_dir.display(),
        secret = secret_path.display(),
    ))?;

    let requested_path =
        PathUri::from_host_native_path(allowed_dir.join("link").join("..").join("secret.txt"))?;
    let sandbox = read_only_sandbox(allowed_dir.clone());
    let error = match file_system.read_file(&requested_path, Some(&sandbox)).await {
        Ok(_) => anyhow::bail!("read should fail after path normalization"),
        Err(error) => error,
    };
    assert_normalized_path_rejected(&error);

    remote_exec(&format!("rm -rf {}", root.display()))?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_remove_removes_symlink_not_target() -> Result<()> {
    skip_if_target_windows!(Ok(()), "tests POSIX symlink removal semantics");
    skip_if_no_network!(Ok(()));
    skip_if_no_remote_env!(Ok(()));

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let root = PathBuf::from(format!(
        "/tmp/codex-remote-remove-link-{}",
        std::process::id()
    ));
    let allowed_dir = root.join("allowed");
    let outside_file = root.join("outside").join("keep.txt");
    let symlink_path = allowed_dir.join("link");
    remote_exec(&format!(
        "rm -rf {root}; mkdir -p {allowed} {outside_parent}; printf outside > {outside}; ln -s {outside} {symlink}",
        root = root.display(),
        allowed = allowed_dir.display(),
        outside_parent = absolute_path(
            outside_file
                .parent()
                .context("outside parent should exist")?
                .to_path_buf(),
        )
        .display(),
        outside = outside_file.display(),
        symlink = symlink_path.display(),
    ))?;

    let sandbox = workspace_write_sandbox(allowed_dir.clone());
    file_system
        .remove(
            &PathUri::from_host_native_path(&symlink_path)?,
            RemoveOptions {
                recursive: false,
                force: false,
            },
            Some(&sandbox),
        )
        .await?;

    let symlink_exists = file_system
        .get_metadata(
            &PathUri::from_abs_path(&absolute_path(symlink_path)),
            /*sandbox*/ None,
        )
        .await
        .is_ok();
    assert!(!symlink_exists);
    let outside = file_system
        .read_file_text(
            &PathUri::from_host_native_path(&outside_file)?,
            /*sandbox*/ None,
        )
        .await?;
    assert_eq!(outside, "outside");

    file_system
        .remove(
            &PathUri::from_host_native_path(&root)?,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_copy_preserves_symlink_source() -> Result<()> {
    skip_if_target_windows!(Ok(()), "tests POSIX symlink copy semantics");
    skip_if_no_network!(Ok(()));
    skip_if_no_remote_env!(Ok(()));

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let root = PathBuf::from(format!(
        "/tmp/codex-remote-copy-link-{}",
        std::process::id()
    ));
    let allowed_dir = root.join("allowed");
    let outside_file = root.join("outside").join("outside.txt");
    let source_symlink = allowed_dir.join("link");
    let copied_symlink = allowed_dir.join("copied-link");
    remote_exec(&format!(
        "rm -rf {root}; mkdir -p {allowed} {outside_parent}; printf outside > {outside}; ln -s {outside} {source}",
        root = root.display(),
        allowed = allowed_dir.display(),
        outside_parent = outside_file.parent().expect("outside parent").display(),
        outside = outside_file.display(),
        source = source_symlink.display(),
    ))?;

    let sandbox = workspace_write_sandbox(allowed_dir.clone());
    file_system
        .copy(
            &PathUri::from_host_native_path(&source_symlink)?,
            &PathUri::from_host_native_path(&copied_symlink)?,
            CopyOptions { recursive: false },
            Some(&sandbox),
        )
        .await?;

    let container_name = test_docker_container_name()
        .context("test requires direct access to the Docker container")?;
    let link_target = Command::new("docker")
        .args([
            "exec",
            container_name.as_str(),
            "readlink",
            copied_symlink
                .to_str()
                .context("copied symlink path should be utf-8")?,
        ])
        .output()?;
    assert!(
        link_target.status.success(),
        "readlink failed: stdout={} stderr={}",
        String::from_utf8_lossy(&link_target.stdout).trim(),
        String::from_utf8_lossy(&link_target.stderr).trim(),
    );
    assert_eq!(
        String::from_utf8_lossy(&link_target.stdout).trim(),
        outside_file.to_string_lossy()
    );

    file_system
        .remove(
            &PathUri::from_host_native_path(&root)?,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    Ok(())
}
