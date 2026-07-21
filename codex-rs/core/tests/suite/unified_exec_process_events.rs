use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::managed_network_requirements_loader;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::time::Duration;
use test_case::test_case;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

const CALL_ID: &str = "pushed-remote-process-events";
const COMPLETE_OUTPUT: &str = "pushed remote output\n";
const RECOVERED_OUTPUT: &str = "recovered missing output\n";
const RETAINED_OUTPUT: &str = "retained output\n";
const REPLAY_OUTPUT_EVENT_COUNT: u64 = 1024;
const REPLAY_RETAINED_OUTPUT_SEQ: u64 = 800;

#[derive(Debug, Clone, Copy)]
enum PushedExecScenario {
    Complete,
    DirectDenied,
    LegacyExit,
    ReplayGap,
}

#[derive(Debug)]
struct PushedExecServerResult {
    process_read_requests: usize,
    process_start: Value,
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

async fn send_exec_server_json(websocket: &mut WebSocketStream<TcpStream>, message: Value) {
    websocket
        .send(Message::Text(message.to_string().into()))
        .await
        .expect("exec-server message should send");
}

async fn accept_initialized_exec_server(listener: TcpListener) -> WebSocketStream<TcpStream> {
    let (stream, _) = listener.accept().await.expect("connection");
    let mut websocket = accept_async(stream).await.expect("websocket handshake");

    let initialize = read_exec_server_json(&mut websocket).await;
    assert_eq!(initialize["method"], "initialize");
    send_exec_server_json(
        &mut websocket,
        json!({
            "id": initialize["id"],
            "result": { "sessionId": "test-session" }
        }),
    )
    .await;
    let initialized = read_exec_server_json(&mut websocket).await;
    assert_eq!(initialized["method"], "initialized");

    websocket
}

async fn send_environment_info(websocket: &mut WebSocketStream<TcpStream>) {
    let info = read_exec_server_json(websocket).await;
    assert_eq!(info["method"], "environment/info");
    respond_environment_info(websocket, &info["id"]).await;
}

async fn respond_environment_info(websocket: &mut WebSocketStream<TcpStream>, id: &Value) {
    send_exec_server_json(
        websocket,
        json!({
            "id": id,
            "result": {
                "shell": { "name": "zsh", "path": "/bin/zsh" },
                "capabilities": { "networkProxyLaunch": true }
            }
        }),
    )
    .await;
}

async fn serve_exec_with_pushed_events(
    listener: TcpListener,
    scenario: PushedExecScenario,
) -> PushedExecServerResult {
    let mut websocket = accept_initialized_exec_server(listener).await;
    send_environment_info(&mut websocket).await;

    let process_start = loop {
        let request = read_exec_server_json(&mut websocket).await;
        match request["method"].as_str() {
            Some("process/start") => break request,
            Some("environment/info") => {
                respond_environment_info(&mut websocket, &request["id"]).await;
            }
            Some("fs/getMetadata") => {
                send_exec_server_json(
                    &mut websocket,
                    json!({
                        "id": request["id"],
                        "error": { "code": -32004, "message": "not found" }
                    }),
                )
                .await;
            }
            Some("fs/canonicalize") => {
                send_exec_server_json(
                    &mut websocket,
                    json!({
                        "id": request["id"],
                        "result": { "path": request["params"]["path"] }
                    }),
                )
                .await;
            }
            Some("fs/walk") => {
                send_exec_server_json(
                    &mut websocket,
                    json!({
                        "id": request["id"],
                        "result": { "entries": [], "errors": [], "truncated": false }
                    }),
                )
                .await;
            }
            method => panic!("unexpected exec-server request before process/start: {method:?}"),
        }
    };
    let process_id = process_start["params"]["processId"]
        .as_str()
        .expect("process/start should include processId")
        .to_string();

    let replay_output = |seq| -> &'static [u8] {
        match seq {
            1 => RECOVERED_OUTPUT.as_bytes(),
            REPLAY_RETAINED_OUTPUT_SEQ => RETAINED_OUTPUT.as_bytes(),
            _ => b"x",
        }
    };
    if matches!(scenario, PushedExecScenario::ReplayGap) {
        // The process replay log retains 256 events. This burst is much larger
        // than both that log and the JSON-RPC event queue, so the reader must
        // apply enough notifications to evict seq 1 before it can read the
        // start response. The total output stays well below the server's 1 MiB
        // retained-output limit, making the subsequent read genuinely able to
        // recover every missing chunk.
        for seq in 1..=REPLAY_OUTPUT_EVENT_COUNT {
            send_exec_server_json(
                &mut websocket,
                json!({
                    "method": "process/output",
                    "params": {
                        "processId": &process_id,
                        "seq": seq,
                        "stream": "stdout",
                        "chunk": BASE64_STANDARD.encode(replay_output(seq)),
                    }
                }),
            )
            .await;
        }
        send_exec_server_json(
            &mut websocket,
            json!({
                "method": "process/exited",
                "params": {
                    "processId": &process_id,
                    "seq": REPLAY_OUTPUT_EVENT_COUNT + 1,
                    "exitCode": 0,
                    "sandboxDenied": false,
                }
            }),
        )
        .await;
    }

    send_exec_server_json(
        &mut websocket,
        json!({
            "id": process_start["id"],
            "result": { "processId": &process_id }
        }),
    )
    .await;

    match scenario {
        PushedExecScenario::Complete => {
            let encoded_output = BASE64_STANDARD.encode(COMPLETE_OUTPUT);
            for message in [
                json!({
                    "method": "process/output",
                    "params": {
                        "processId": &process_id,
                        "seq": 1,
                        "stream": "stdout",
                        "chunk": encoded_output,
                    }
                }),
                json!({
                    "method": "process/exited",
                    "params": {
                        "processId": &process_id,
                        "seq": 2,
                        "exitCode": 0,
                        "sandboxDenied": false,
                    }
                }),
                json!({
                    "method": "process/closed",
                    "params": { "processId": &process_id, "seq": 3 }
                }),
            ] {
                send_exec_server_json(&mut websocket, message).await;
            }
        }
        PushedExecScenario::DirectDenied => {
            send_exec_server_json(
                &mut websocket,
                json!({
                    "method": "process/exited",
                    "params": {
                        "processId": &process_id,
                        "seq": 1,
                        "exitCode": 1,
                        "sandboxDenied": true,
                    }
                }),
            )
            .await;
        }
        PushedExecScenario::LegacyExit => {
            send_exec_server_json(
                &mut websocket,
                json!({
                    "method": "process/exited",
                    "params": {
                        "processId": &process_id,
                        "seq": 1,
                        "exitCode": 1,
                    }
                }),
            )
            .await;
        }
        PushedExecScenario::ReplayGap => {}
    }

    let mut process_read_requests = 0;
    loop {
        let request = read_exec_server_json(&mut websocket).await;
        match request["method"].as_str() {
            Some("process/read") => {
                process_read_requests += 1;
                let result = match scenario {
                    PushedExecScenario::Complete => json!({
                        "chunks": [{
                            "seq": 1,
                            "stream": "stdout",
                            "chunk": BASE64_STANDARD.encode(COMPLETE_OUTPUT),
                        }],
                        "nextSeq": 4,
                        "exited": true,
                        "exitCode": 0,
                        "closed": true,
                        "failure": null,
                        "sandboxDenied": false,
                    }),
                    PushedExecScenario::DirectDenied => json!({
                        "chunks": [],
                        "nextSeq": 2,
                        "exited": true,
                        "exitCode": 1,
                        "closed": false,
                        "failure": null,
                        "sandboxDenied": true,
                    }),
                    PushedExecScenario::LegacyExit => json!({
                        "chunks": [],
                        "nextSeq": 3,
                        "exited": true,
                        "exitCode": 1,
                        "closed": true,
                        "failure": null,
                        "sandboxDenied": true,
                    }),
                    PushedExecScenario::ReplayGap => {
                        let chunks = (1..=REPLAY_OUTPUT_EVENT_COUNT)
                            .map(|seq| {
                                json!({
                                    "seq": seq,
                                    "stream": "stdout",
                                    "chunk": BASE64_STANDARD.encode(replay_output(seq)),
                                })
                            })
                            .collect::<Vec<_>>();
                        json!({
                            "chunks": chunks,
                            "nextSeq": REPLAY_OUTPUT_EVENT_COUNT + 2,
                            "exited": true,
                            "exitCode": 0,
                            "closed": false,
                            "failure": null,
                            "sandboxDenied": false,
                        })
                    }
                };
                send_exec_server_json(
                    &mut websocket,
                    json!({
                        "id": request["id"],
                        "result": result,
                    }),
                )
                .await;
                if matches!(scenario, PushedExecScenario::ReplayGap) && process_read_requests == 1 {
                    send_exec_server_json(
                        &mut websocket,
                        json!({
                            "method": "process/closed",
                            "params": {
                                "processId": &process_id,
                                "seq": REPLAY_OUTPUT_EVENT_COUNT + 2,
                            }
                        }),
                    )
                    .await;
                }
            }
            Some("process/terminate") => {
                send_exec_server_json(
                    &mut websocket,
                    json!({
                        "id": request["id"],
                        "result": { "running": false }
                    }),
                )
                .await;
                return PushedExecServerResult {
                    process_read_requests,
                    process_start,
                };
            }
            method => panic!("unexpected exec-server request: {method:?}"),
        }
    }
}

#[test_case(PushedExecScenario::Complete, false ; "complete_event_stream")]
#[test_case(PushedExecScenario::DirectDenied, false ; "direct_sandbox_denial")]
#[test_case(PushedExecScenario::LegacyExit, false ; "legacy_exit_metadata")]
#[test_case(PushedExecScenario::ReplayGap, false ; "truncated_event_replay")]
#[test_case(PushedExecScenario::Complete, true ; "managed_network_uses_executor_proxy_launch")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_command_consumes_pushed_remote_process_events(
    scenario: PushedExecScenario,
    managed_network: bool,
) -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    CALL_ID,
                    "exec_command",
                    &json!({
                        "cmd": "pwd",
                        "yield_time_ms": 1_000,
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
    let exec_server_url = format!("ws://{}", listener.local_addr()?);
    let exec_server = tokio::spawn(serve_exec_with_pushed_events(listener, scenario));
    let mut builder = test_codex().with_exec_server_url(exec_server_url);
    if managed_network {
        builder = builder
            .with_cloud_config_bundle(managed_network_requirements_loader())
            .with_pre_build_hook(|home| {
                fs::write(
                    home.join("config.toml"),
                    r#"default_permissions = "workspace"

[permissions.workspace.filesystem]
":minimal" = "read"

[permissions.workspace.network]
enabled = true
mode = "full"
allow_local_binding = true
"#,
                )
                .expect("write managed-network test config");
            });
    }
    let mut builder = builder.with_config(|config| {
        config.project_doc_max_bytes = 0;
        config.use_experimental_unified_exec_tool = true;
        config
            .features
            .enable(Feature::UnifiedExec)
            .expect("test config should allow feature update");
    });
    let test = timeout(Duration::from_secs(5), builder.build(&server))
        .await
        .context("thread startup should connect to the fake exec-server")??;

    let turn_permission_profile = if managed_network {
        test.session_configured.permission_profile.clone()
    } else {
        PermissionProfile::Disabled
    };
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(turn_permission_profile, test.config.cwd.as_path());
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "run a one-shot remote command".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
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
    let mut saw_exec_command_begin = false;
    if !managed_network {
        loop {
            let event = timeout(Duration::from_secs(5), test.codex.next_event())
                .await
                .context("turn should complete")??
                .msg;
            match event {
                EventMsg::ExecCommandBegin(event) if event.call_id == CALL_ID => {
                    saw_exec_command_begin = true;
                }
                EventMsg::TurnComplete(_) => break,
                _ => {}
            }
        }
    }
    let cleanup_timeout = if managed_network {
        Duration::from_secs(15)
    } else {
        Duration::from_secs(5)
    };
    let exec_server_result = timeout(cleanup_timeout, exec_server)
        .await
        .context("fake exec-server should observe process cleanup")??;
    if managed_network {
        let params = &exec_server_result.process_start["params"];
        assert_eq!(params["enforceManagedNetwork"], true);
        assert_eq!(params["managedNetwork"], Value::Null);
        assert_eq!(params["env"]["HTTP_PROXY"], Value::Null);
        assert_eq!(params["networkProxy"]["proxy"]["enabled"], true);
        assert_eq!(params["networkProxy"]["proxy"]["mode"], "full");
        assert_eq!(params["networkProxy"]["environmentId"], "remote");
        assert!(params["networkProxy"]["executionId"].as_str().is_some());
        timeout(Duration::from_secs(5), async {
            while response_mock.requests().len() < 2 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .context("model should receive the remote exec output")?;
        return Ok(());
    }
    let request = response_mock
        .last_request()
        .context("model should receive the exec_command output")?;
    let (output, success) = request
        .function_call_output_content_and_success(CALL_ID)
        .context("exec_command output should be model visible")?;
    let output = output.context("exec_command output should contain text")?;
    let process_read_requests = exec_server_result.process_read_requests;
    match scenario {
        PushedExecScenario::Complete => {
            assert_ne!(success, Some(false));
            assert!(saw_exec_command_begin);
            assert!(output.contains("Process exited with code 0"));
            assert!(output.contains(COMPLETE_OUTPUT));
            assert_eq!(process_read_requests, 0, "unexpected compatibility read");
        }
        PushedExecScenario::DirectDenied => {
            assert!(!saw_exec_command_begin);
            assert!(output.contains("Process exited with code 1"));
            assert_eq!(process_read_requests, 0, "unexpected compatibility read");
        }
        PushedExecScenario::LegacyExit => {
            assert!(!saw_exec_command_begin);
            assert!(output.contains("Process exited with code 1"));
            assert_eq!(process_read_requests, 1, "expected compatibility read");
        }
        PushedExecScenario::ReplayGap => {
            assert_ne!(success, Some(false));
            assert!(saw_exec_command_begin);
            assert_eq!(output.matches(RECOVERED_OUTPUT).count(), 1);
            assert_eq!(output.matches(RETAINED_OUTPUT).count(), 1);
            assert_eq!(process_read_requests, 1, "expected replay recovery read");
        }
    }

    Ok(())
}
