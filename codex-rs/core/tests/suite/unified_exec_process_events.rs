use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::TurnInputRequest;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::protocol::TurnEnvironmentSelections;
use codex_protocol::user_input::UserInput;
use codex_utils_path_uri::PathUri;
use core_test_support::managed_network_requirements_loader;
use core_test_support::responses::ev_apply_patch_custom_tool_call;
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
#[cfg_attr(windows, allow(dead_code))]
enum PushedExecScenario {
    Complete,
    DirectDenied,
    ElevatedPowerShell,
    SandboxedInterceptedPatch,
    SandboxedDirectPatch,
    SandboxedDirectPatchDenied,
    SandboxedDirectPatchRetry,
    UnsandboxedInterceptedPatch,
    FullDiskInterceptedPatch,
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

async fn send_environment_info(
    websocket: &mut WebSocketStream<TcpStream>,
    scenario: PushedExecScenario,
) {
    let info = read_exec_server_json(websocket).await;
    assert_eq!(info["method"], "environment/info");
    respond_environment_info(websocket, &info["id"], scenario).await;
}

async fn respond_environment_info(
    websocket: &mut WebSocketStream<TcpStream>,
    id: &Value,
    scenario: PushedExecScenario,
) {
    let shell = if matches!(scenario, PushedExecScenario::ElevatedPowerShell) {
        json!({ "name": "powershell", "path": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" })
    } else {
        json!({ "name": "zsh", "path": "/bin/zsh" })
    };
    send_exec_server_json(
        websocket,
        json!({
            "id": id,
            "result": {
                "shell": shell,
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
    let unrestricted_patch = matches!(
        scenario,
        PushedExecScenario::UnsandboxedInterceptedPatch
            | PushedExecScenario::FullDiskInterceptedPatch
    );
    let mut websocket = accept_initialized_exec_server(listener).await;
    send_environment_info(&mut websocket, scenario).await;

    let process_start = loop {
        let request = read_exec_server_json(&mut websocket).await;
        match request["method"].as_str() {
            Some("process/start") => break request,
            Some("environment/info") => {
                respond_environment_info(&mut websocket, &request["id"], scenario).await;
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
            Some("fs/readFile")
                if matches!(
                    scenario,
                    PushedExecScenario::SandboxedInterceptedPatch
                        | PushedExecScenario::SandboxedDirectPatch
                        | PushedExecScenario::SandboxedDirectPatchDenied
                        | PushedExecScenario::SandboxedDirectPatchRetry
                ) =>
            {
                if !request["params"]["sandbox"].is_null() {
                    assert_eq!(request["params"]["sandbox"]["cwd"], "file:///C:/workspace");
                    assert_eq!(
                        request["params"]["sandbox"]["workspaceRoots"],
                        json!(["file:///C:/workspace", "file:///D:/other-workspace"])
                    );
                    assert_eq!(
                        request["params"]["sandbox"]["windowsSandboxLevel"],
                        "restricted-token"
                    );
                }
                send_exec_server_json(
                    &mut websocket,
                    json!({
                        "id": request["id"],
                        "result": { "dataBase64": BASE64_STANDARD.encode("old\n") }
                    }),
                )
                .await;
            }
            Some("fs/readFile") if unrestricted_patch => {
                send_exec_server_json(
                    &mut websocket,
                    json!({
                        "id": request["id"],
                        "error": { "code": -32004, "message": "not found" }
                    }),
                )
                .await;
            }
            Some("fs/writeFile")
                if matches!(
                    scenario,
                    PushedExecScenario::SandboxedDirectPatchDenied
                        | PushedExecScenario::SandboxedDirectPatchRetry
                ) && !request["params"]["sandbox"].is_null() =>
            {
                send_exec_server_json(
                    &mut websocket,
                    json!({
                        "id": request["id"],
                        "error": { "code": -32600, "message": "Access is denied. (os error 5)" }
                    }),
                )
                .await;
                if matches!(scenario, PushedExecScenario::SandboxedDirectPatchDenied) {
                    return PushedExecServerResult {
                        process_read_requests: 0,
                        process_start: request,
                    };
                }
            }
            Some("fs/writeFile")
                if matches!(
                    scenario,
                    PushedExecScenario::SandboxedInterceptedPatch
                        | PushedExecScenario::SandboxedDirectPatch
                        | PushedExecScenario::SandboxedDirectPatchRetry
                        | PushedExecScenario::UnsandboxedInterceptedPatch
                        | PushedExecScenario::FullDiskInterceptedPatch
                ) =>
            {
                send_exec_server_json(&mut websocket, json!({ "id": request["id"], "result": {} }))
                    .await;
                return PushedExecServerResult {
                    process_read_requests: 0,
                    process_start: request,
                };
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
        PushedExecScenario::Complete | PushedExecScenario::ElevatedPowerShell => {
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
        PushedExecScenario::SandboxedInterceptedPatch
        | PushedExecScenario::SandboxedDirectPatch
        | PushedExecScenario::SandboxedDirectPatchDenied
        | PushedExecScenario::SandboxedDirectPatchRetry => {
            panic!("cross-platform sandboxed patches must use the remote filesystem")
        }
        PushedExecScenario::UnsandboxedInterceptedPatch
        | PushedExecScenario::FullDiskInterceptedPatch => {
            panic!("unsandboxed intercepted patches must write through the remote filesystem")
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
                    PushedExecScenario::ElevatedPowerShell => {
                        panic!("elevated remote PowerShell must not read a remote process")
                    }
                    PushedExecScenario::SandboxedInterceptedPatch
                    | PushedExecScenario::SandboxedDirectPatch
                    | PushedExecScenario::SandboxedDirectPatchDenied
                    | PushedExecScenario::SandboxedDirectPatchRetry => {
                        panic!("cross-platform sandboxed patches must not read a remote process")
                    }
                    PushedExecScenario::UnsandboxedInterceptedPatch
                    | PushedExecScenario::FullDiskInterceptedPatch => {
                        panic!("unsandboxed intercepted patches must not read a remote process")
                    }
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

#[test_case(PushedExecScenario::Complete, false, false, false ; "complete_event_stream")]
#[test_case(PushedExecScenario::DirectDenied, false, false, false ; "direct_sandbox_denial")]
#[test_case(PushedExecScenario::LegacyExit, false, false, false ; "legacy_exit_metadata")]
#[test_case(PushedExecScenario::ReplayGap, false, false, false ; "truncated_event_replay")]
#[test_case(PushedExecScenario::Complete, true, true, false ; "managed_network_uses_executor_proxy_launch")]
#[test_case(PushedExecScenario::Complete, true, false, false ; "strict_managed_allowlist_omits_policy_callbacks")]
#[cfg_attr(not(windows), test_case(PushedExecScenario::Complete, true, true, true ; "foreign_windows_managed_network_preserves_approval_registration"))]
#[cfg_attr(not(windows), test_case(PushedExecScenario::Complete, false, false, true ; "foreign_windows_workspace_sandbox"))]
#[test_case(PushedExecScenario::ElevatedPowerShell, false, false, true ; "windows_elevated_powershell_disables_profile")]
#[cfg_attr(not(windows), test_case(PushedExecScenario::SandboxedInterceptedPatch, false, false, true ; "foreign_windows_intercepted_patch_is_sandboxed"))]
#[cfg_attr(not(windows), test_case(PushedExecScenario::SandboxedDirectPatch, false, false, true ; "foreign_windows_direct_patch_is_sandboxed"))]
#[cfg_attr(not(windows), test_case(PushedExecScenario::SandboxedDirectPatchDenied, false, false, true ; "foreign_windows_direct_patch_denial_requests_approval"))]
#[cfg_attr(not(windows), test_case(PushedExecScenario::SandboxedDirectPatchRetry, false, false, true ; "foreign_windows_direct_patch_denial_approval_retries_unsandboxed"))]
#[cfg_attr(not(windows), test_case(PushedExecScenario::UnsandboxedInterceptedPatch, false, false, true ; "foreign_windows_unsandboxed_intercepted_patch_succeeds"))]
#[cfg_attr(not(windows), test_case(PushedExecScenario::FullDiskInterceptedPatch, false, false, true ; "foreign_windows_full_disk_intercepted_patch_succeeds"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_command_consumes_pushed_remote_process_events(
    scenario: PushedExecScenario,
    managed_network: bool,
    policy_callbacks: bool,
    foreign_cwd: bool,
) -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let server = start_mock_server().await;
    let tool_call = match scenario {
        PushedExecScenario::SandboxedDirectPatch
        | PushedExecScenario::SandboxedDirectPatchDenied
        | PushedExecScenario::SandboxedDirectPatchRetry => ev_apply_patch_custom_tool_call(
            CALL_ID,
            "*** Begin Patch\n*** Update File: secret.txt\n@@\n-old\n+new\n*** End Patch",
        ),
        _ => ev_function_call(
            CALL_ID,
            "exec_command",
            &json!({
                "cmd": match scenario {
                    PushedExecScenario::SandboxedInterceptedPatch => {
                        "apply_patch <<'PATCH'\n*** Begin Patch\n*** Update File: secret.txt\n@@\n-old\n+new\n*** End Patch\nPATCH"
                    }
                    PushedExecScenario::UnsandboxedInterceptedPatch
                    | PushedExecScenario::FullDiskInterceptedPatch => {
                        "apply_patch <<'PATCH'\n*** Begin Patch\n*** Add File: allowed.txt\n+allowed\n*** End Patch\nPATCH"
                    }
                    _ => "pwd",
                },
                "yield_time_ms": 1_000,
            })
            .to_string(),
        ),
    };
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                tool_call,
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
        let cloud_config_bundle = if policy_callbacks {
            managed_network_requirements_loader()
        } else {
            CloudConfigBundleFixture::loader_with_enterprise_requirement(
                r#"
[experimental_network]
enabled = true
allow_local_binding = true
managed_allowed_domains_only = true

[experimental_network.domains]
"allowed.example.com" = "allow"
"#,
            )
        };
        builder = builder
            .with_cloud_config_bundle(cloud_config_bundle)
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

[features]
hooks = true

[hooks]

[[hooks.PermissionRequest]]

[[hooks.PermissionRequest.hooks]]
type = "command"
command = "unused"
timeout = 900
"#,
                )
                .expect("write managed-network test config");
            });
    }
    let mut builder = builder.with_config(move |config| {
        config.project_doc_max_bytes = 0;
        if matches!(scenario, PushedExecScenario::ElevatedPowerShell) {
            config.set_windows_elevated_sandbox_enabled(/*value*/ true);
        }
        if managed_network {
            #[cfg(windows)]
            config.set_windows_sandbox_enabled(/*value*/ true);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.bypass_hook_trust = true;
        }
    });
    let test = timeout(Duration::from_secs(5), builder.build(&server))
        .await
        .context("thread startup should connect to the fake exec-server")??;

    let turn_permission_profile = if managed_network {
        test.session_configured.permission_profile.clone()
    } else if matches!(scenario, PushedExecScenario::FullDiskInterceptedPatch) {
        PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                FileSystemAccessMode::Write,
            )]),
            NetworkSandboxPolicy::Enabled,
        )
    } else if foreign_cwd && !matches!(scenario, PushedExecScenario::UnsandboxedInterceptedPatch) {
        PermissionProfile::workspace_write()
    } else {
        PermissionProfile::Disabled
    };
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(turn_permission_profile, test.config.cwd.as_path());
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "run a one-shot remote command".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: foreign_cwd.then(|| {
                    let cwd = PathUri::parse("file:///C:/workspace").expect("valid Windows cwd");
                    TurnEnvironmentSelections::new(
                        test.config.cwd.clone(),
                        vec![TurnEnvironmentSelection {
                            environment_id: codex_exec_server::REMOTE_ENVIRONMENT_ID.to_string(),
                            cwd: cwd.clone(),
                            workspace_roots: vec![
                                cwd,
                                PathUri::parse("file:///D:/other-workspace")
                                    .expect("valid Windows workspace root"),
                            ],
                            config: EnvironmentConfigState::FromThread,
                        }],
                    )
                }),
                approval_policy: Some(
                    if managed_network
                        || matches!(
                            scenario,
                            PushedExecScenario::SandboxedDirectPatchDenied
                                | PushedExecScenario::SandboxedDirectPatchRetry
                        )
                    {
                        AskForApproval::OnRequest
                    } else {
                        AskForApproval::Never
                    },
                ),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: test.session_configured.model.clone(),
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
        .await?;
    let mut saw_exec_command_begin = false;
    let mut saw_patch_denial_approval = false;
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
                EventMsg::ApplyPatchApprovalRequest(approval)
                    if matches!(
                        scenario,
                        PushedExecScenario::SandboxedDirectPatchDenied
                            | PushedExecScenario::SandboxedDirectPatchRetry
                    ) =>
                {
                    saw_patch_denial_approval = true;
                    test.codex
                        .submit(Op::PatchApproval {
                            id: approval.call_id,
                            decision: if matches!(
                                scenario,
                                PushedExecScenario::SandboxedDirectPatchRetry
                            ) {
                                ReviewDecision::Approved
                            } else {
                                ReviewDecision::Denied {
                                    rejection: "denied by test".to_string(),
                                }
                            },
                        })
                        .await?;
                }
                EventMsg::TurnComplete(_) => break,
                _ => {}
            }
        }
    }
    if matches!(
        scenario,
        PushedExecScenario::SandboxedDirectPatchDenied
            | PushedExecScenario::SandboxedDirectPatchRetry
    ) {
        assert!(
            saw_patch_denial_approval,
            "executor-managed sandbox denial should request patch approval"
        );
        let exec_server_result = timeout(Duration::from_secs(5), exec_server)
            .await
            .context("fake exec-server should observe the denied patch write")??;
        assert_eq!(exec_server_result.process_start["method"], "fs/writeFile");
        if matches!(scenario, PushedExecScenario::SandboxedDirectPatchRetry) {
            assert_eq!(
                exec_server_result.process_start["params"]["sandbox"],
                Value::Null
            );
            let request = response_mock
                .last_request()
                .context("model should receive the approved patch result")?;
            let (output, success) = request
                .custom_tool_call_output_content_and_success(CALL_ID)
                .context("approved patch result should be model visible")?;
            assert_ne!(success, Some(false));
            assert!(
                output
                    .context("approved patch result should contain text")?
                    .contains("Success. Updated the following files:")
            );
        }
        return Ok(());
    }
    if matches!(
        scenario,
        PushedExecScenario::SandboxedInterceptedPatch | PushedExecScenario::SandboxedDirectPatch
    ) {
        assert!(!saw_exec_command_begin);
        let request = response_mock
            .last_request()
            .context("model should receive the sandboxed patch result")?;
        let (output, success) = if matches!(scenario, PushedExecScenario::SandboxedDirectPatch) {
            request.custom_tool_call_output_content_and_success(CALL_ID)
        } else {
            request.function_call_output_content_and_success(CALL_ID)
        }
        .context("sandboxed patch result should be model visible")?;
        assert_ne!(success, Some(false));
        assert!(
            output
                .context("sandboxed patch result should contain text")?
                .contains("Success. Updated the following files:")
        );
        let exec_server_result = timeout(Duration::from_secs(5), exec_server)
            .await
            .context("fake exec-server should observe the sandboxed patch write")??;
        let write_request = exec_server_result.process_start;
        assert_eq!(write_request["method"], "fs/writeFile");
        assert_eq!(
            write_request["params"]["path"],
            "file:///C:/workspace/secret.txt"
        );
        assert_eq!(
            write_request["params"]["sandbox"]["windowsSandboxLevel"],
            "restricted-token"
        );
        assert_eq!(
            write_request["params"]["sandbox"]["workspaceRoots"],
            json!(["file:///C:/workspace", "file:///D:/other-workspace"])
        );
        assert_eq!(
            BASE64_STANDARD.decode(
                write_request["params"]["dataBase64"]
                    .as_str()
                    .expect("filesystem write should include encoded contents")
            )?,
            b"new\n"
        );
        return Ok(());
    }
    if matches!(
        scenario,
        PushedExecScenario::UnsandboxedInterceptedPatch
            | PushedExecScenario::FullDiskInterceptedPatch
    ) {
        let request = response_mock
            .last_request()
            .context("model should receive the unrestricted patch result")?;
        let (output, success) = request
            .function_call_output_content_and_success(CALL_ID)
            .context("unrestricted patch result should be model visible")?;
        assert_ne!(success, Some(false));
        let output = output.context("unrestricted patch result should contain text")?;
        assert!(
            output.contains("Success. Updated the following files:"),
            "unrestricted intercepted patch failed: {output}"
        );
        let exec_server_result = timeout(Duration::from_secs(5), exec_server)
            .await
            .context("fake exec-server should observe the unrestricted patch write")??;
        let write_request = exec_server_result.process_start;
        assert_eq!(write_request["method"], "fs/writeFile");
        assert_eq!(
            write_request["params"]["path"],
            "file:///C:/workspace/allowed.txt"
        );
        assert_eq!(write_request["params"]["sandbox"], Value::Null);
        assert_eq!(
            BASE64_STANDARD.decode(
                write_request["params"]["dataBase64"]
                    .as_str()
                    .expect("filesystem write should include encoded contents")
            )?,
            b"allowed\n"
        );
        return Ok(());
    }
    let cleanup_timeout = if managed_network {
        Duration::from_secs(15)
    } else {
        Duration::from_secs(5)
    };
    let exec_server_result = timeout(cleanup_timeout, exec_server)
        .await
        .context("fake exec-server should observe process cleanup")??;
    if foreign_cwd {
        let params = &exec_server_result.process_start["params"];
        assert_eq!(params["cwd"], "file:///C:/workspace");
        assert_eq!(params["sandbox"]["cwd"], "file:///C:/workspace");
        assert_eq!(
            params["sandbox"]["workspaceRoots"],
            json!(["file:///C:/workspace", "file:///D:/other-workspace"])
        );
        if matches!(scenario, PushedExecScenario::ElevatedPowerShell) {
            assert_eq!(params["sandbox"]["windowsSandboxLevel"], "elevated");
            assert!(
                params["argv"]
                    .as_array()
                    .is_some_and(|argv| argv.iter().any(|arg| arg == "-NoProfile")),
                "elevated remote PowerShell must not load a user profile"
            );
        } else {
            assert_eq!(params["sandbox"]["windowsSandboxLevel"], "restricted-token");
        }
    }
    if managed_network {
        let params = &exec_server_result.process_start["params"];
        assert_eq!(params["enforceManagedNetwork"], true);
        assert_eq!(params["managedNetwork"], Value::Null);
        assert_eq!(params["env"]["HTTP_PROXY"], Value::Null);
        assert_eq!(params["networkProxy"]["proxy"]["enabled"], true);
        assert_eq!(params["networkProxy"]["proxy"]["mode"], "full");
        assert_eq!(
            params["networkProxy"]["policyDecisionTimeoutMs"].as_u64(),
            policy_callbacks.then_some(1_000_000)
        );
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
        PushedExecScenario::Complete | PushedExecScenario::ElevatedPowerShell => {
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
        PushedExecScenario::SandboxedInterceptedPatch
        | PushedExecScenario::SandboxedDirectPatch
        | PushedExecScenario::SandboxedDirectPatchDenied
        | PushedExecScenario::SandboxedDirectPatchRetry => {
            unreachable!("sandboxed patch returned early")
        }
        PushedExecScenario::UnsandboxedInterceptedPatch
        | PushedExecScenario::FullDiskInterceptedPatch => {
            unreachable!("unsandboxed intercepted patch returned early")
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
