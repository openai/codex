//! Exercises remote network launch policy, including owner hooks and MXC's local-binding default.

use super::accept_initialized_exec_server;
use super::read_exec_server_json;
use super::send_exec_server_json;
use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::TurnInputRequest;
use codex_core::config::NetworkProxySpec;
use codex_exec_server::ExecParams;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::items::CommandExecutionStatus;
use codex_protocol::items::TurnItem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::sandbox::SandboxType;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::mount_response_once_match;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::startup::STARTUP_TIMEOUT;
use core_test_support::test_codex::environment_config_for_selection;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
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

#[test_case(SandboxType::WindowsRestrictedToken, "allow_local_binding = true", Ok(true), false; "explicit_local_binding")]
#[test_case(SandboxType::WindowsMxc, "", Ok(true), false; "mxc_default_local_binding")]
#[test_case(SandboxType::WindowsMxc, "allow_local_binding = false", Err("MXC cannot enforce allow_local_binding=false"), false; "mxc_rejects_false")]
#[test_case(SandboxType::WindowsRestrictedToken, "", Ok(false), false; "non_mxc_default_local_binding")]
#[test_case(SandboxType::WindowsRestrictedToken, "", Ok(false), true; "local_mxc_preference_keeps_remote_legacy")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_local_binding_policy_and_review_cleanup(
    sandbox_type: SandboxType,
    local_binding: &'static str,
    expected_local_binding: Result<bool, &'static str>,
    prefer_mxc: bool,
) -> Result<()> {
    let server = start_mock_server().await;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let exec_server_url = format!("ws://{}", listener.local_addr()?);
    let call_id = "pending-network-at-exit";
    let parent = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            let body = review_body(request);
            body.pointer("/client_metadata/x-openai-subagent") != Some(&json!("guardian"))
                && !body.to_string().contains("pending-network-at-exit")
        },
        sse(vec![
            ev_function_call(
                call_id,
                "exec_command",
                &json!({"cmd": "build-site", "yield_time_ms": 10_000}).to_string(),
            ),
            ev_completed("start"),
        ]),
    )
    .await;
    let pending_review = mount_response_once_match(
        &server,
        |request: &wiremock::Request| {
            review_body(request).pointer("/client_metadata/x-openai-subagent")
                == Some(&json!("guardian"))
        },
        sse_response(sse(vec![ev_completed("review")]))
            .set_delay(Duration::from_secs(/*secs*/ 60)),
    )
    .await;
    let final_response = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            let body = review_body(request);
            body.pointer("/client_metadata/x-openai-subagent") != Some(&json!("guardian"))
                && body.to_string().contains("pending-network-at-exit")
        },
        sse(vec![
            ev_assistant_message("done", "done"),
            ev_completed("done"),
        ]),
    )
    .await;
    let exec_server = tokio::spawn(async move {
        let mut websocket = accept_initialized_exec_server(listener).await;
        let hook = accept_process_start(&mut websocket).await;
        // The owner hook keeps its filesystem access, but must use the command's network policy.
        assert_eq!(
            hook.sandbox.expect("owner hook sandbox policy").permissions,
            PermissionProfile::Disabled
        );
        assert!(hook.enforce_managed_network);
        let hook_launch = hook.network_proxy.expect("owner hook requires the proxy");
        assert!(hook_launch.proxy.enabled);
        assert_eq!(
            hook_launch.execution_id.as_deref(),
            Some(hook.process_id.as_str())
        );

        let command = accept_process_start(&mut websocket).await;
        let launch = command.network_proxy.expect("command requires the proxy");
        assert_ne!(
            command.sandbox.expect("command sandbox policy").permissions,
            PermissionProfile::Disabled
        );
        assert_eq!(hook_launch.environment_id, launch.environment_id);
        assert_eq!(hook_launch.proxy, launch.proxy);
        assert!(launch.policy_decision_timeout_ms.is_some());
        let allow_local_binding =
            expected_local_binding.expect("invalid MXC config must not launch");
        assert_eq!(launch.proxy.allow_local_binding, allow_local_binding);
        let process_id = command.process_id;
        let host = if allow_local_binding {
            "10.0.0.1"
        } else {
            "8.8.8.8"
        };
        send_exec_server_json(&mut websocket, json!({
            "id": 1, "method": "network/policyRequest",
            "params": {"processId": process_id, "request": {"protocol": "https_connect", "host": host, "port": 443}}
        })).await;
        timeout(Duration::from_secs(/*secs*/ 10), async {
            while !pending_review.requests().iter().any(|request| {
                request
                    .body_json()
                    .pointer("/client_metadata/x-openai-subagent")
                    == Some(&json!("guardian"))
            }) {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(/*millis*/ 10)) => {},
                    request = read_exec_server_json(&mut websocket, Duration::from_secs(/*secs*/ 10)) => {
                        respond_to_setup_or_hook_request(&mut websocket, &request).await;
                    }
                }
            }
        })
        .await
        .expect("Guardian must start before process completion");
        for message in [
            json!({"method": "process/output", "params": {"processId": process_id, "seq": 1, "stream": "stdout", "chunk": BASE64_STANDARD.encode("build complete\n")}}),
            json!({"method": "process/exited", "params": {"processId": process_id, "seq": 2, "exitCode": 0, "sandboxDenied": false}}),
            json!({"method": "process/closed", "params": {"processId": process_id, "seq": 3}}),
        ] {
            send_exec_server_json(&mut websocket, message).await;
        }
        loop {
            let request =
                read_exec_server_json(&mut websocket, Duration::from_secs(/*secs*/ 10)).await;
            match request["method"].as_str() {
                Some("process/terminate") => {
                    send_exec_server_json(
                        &mut websocket,
                        json!({"id": request["id"], "result": {"running": false}}),
                    )
                    .await;
                    break;
                }
                Some("process/read") => {
                    send_exec_server_json(&mut websocket, json!({"id": request["id"], "result": {"chunks": [], "nextSeq": 4, "exited": true, "exitCode": 0, "closed": true, "failure": null, "sandboxDenied": false}})).await;
                }
                None => panic!("withdrawn request must not be approved: {request}"),
                _ => respond_to_setup_or_hook_request(&mut websocket, &request).await,
            }
        }
    });
    let test = test_codex()
        .with_exec_server_url(exec_server_url)
        .with_cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(format!(
                "[experimental_network]\nenabled = true\n{local_binding}"
            )),
        )
        .with_pre_build_hook(move |home| {
            fs::write(
                home.join("config.toml"),
                format!(
                    r#"default_permissions = "workspace"
[permissions.workspace.filesystem]
":minimal" = "read"
[permissions.workspace.network]
enabled = true
mode = "full"
{local_binding}
"#
                ),
            )
            .expect("managed network config");
        })
        .with_config(move |config| {
            config.project_doc_max_bytes = 0;
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.permissions.windows_sandbox_type = sandbox_type;
            config.prefer_mxc = prefer_mxc;
            #[cfg(windows)]
            config.set_windows_sandbox_enabled(/*value*/ true);
        })
        // This test supplies its own fake executor and must not select a CI executor.
        .build(&server)
        .await?;
    if prefer_mxc {
        assert_eq!(
            test.config.effective_local_windows_sandbox_type(),
            SandboxType::WindowsMxc
        );
    }
    let selection = test
        .codex
        .environment_selections()
        .await
        .pop()
        .expect("remote environment");
    let mut config = environment_config_for_selection(&test.config, &selection);
    config.network_policy = test
        .config
        .permissions
        .network
        .as_ref()
        .map(NetworkProxySpec::environment_policy);
    config.environment_hooks = serde_json::from_value(json!({
        "UserPromptSubmit": [{"hooks": [{"type": "command", "command": "exit 0"}]}],
    }))?;
    test.codex.environment_ready(&selection, config).await?;
    let (sandbox_policy, permission_profile) = turn_permission_fields(
        test.session_configured.permission_profile.clone(),
        test.config.cwd.as_path(),
    );
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "build the site".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                approval_policy: Some(AskForApproval::OnRequest),
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                ..Default::default()
            }),
        )
        .await?;
    timeout(Duration::from_secs(/*secs*/ 15), async {
        while final_response.function_call_output_text(call_id).is_none() {
            tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
        }
    })
    .await?;
    let output = final_response
        .function_call_output_text(call_id)
        .context("tool output")?;
    if let Err(expected_error) = expected_local_binding {
        assert!(output.contains(expected_error), "{output}");
        exec_server.abort();
        assert!(exec_server.await.is_err_and(|err| err.is_cancelled()));
        return Ok(());
    }
    let completed = timeout(Duration::from_secs(/*secs*/ 15), async {
        loop {
            match test.codex.next_event().await?.msg {
                EventMsg::ItemCompleted(event) => {
                    if let TurnItem::CommandExecution(item) = event.item {
                        return Ok::<_, anyhow::Error>(item);
                    }
                }
                EventMsg::ExecApprovalRequest(_) => anyhow::bail!("unexpected user approval"),
                _ => {}
            }
        }
    })
    .await
    .context("command should complete")??;
    assert_eq!(
        (
            completed.status,
            completed.exit_code,
            completed.aggregated_output.as_deref()
        ),
        (
            CommandExecutionStatus::Completed,
            Some(0),
            Some("build complete\n")
        )
    );
    assert!(output.contains("Process exited with code 0"), "{output}");
    assert!(output.contains("build complete"), "{output}");
    assert_eq!(parent.requests().len(), 1);
    timeout(Duration::from_secs(/*secs*/ 10), exec_server).await??;
    Ok(())
}

async fn accept_process_start(websocket: &mut WebSocketStream<TcpStream>) -> ExecParams {
    loop {
        let mut request = read_exec_server_json(websocket, STARTUP_TIMEOUT).await;
        if request["method"] == "process/start" {
            let start: ExecParams =
                serde_json::from_value(request["params"].take()).expect("process start");
            send_exec_server_json(
                websocket,
                json!({"id": request["id"], "result": {"processId": start.process_id}}),
            )
            .await;
            return start;
        }
        respond_to_setup_or_hook_request(websocket, &request).await;
    }
}

fn review_body(request: &wiremock::Request) -> Value {
    let bytes = if request
        .headers
        .get("content-encoding")
        .is_some_and(|value| value == "zstd")
    {
        zstd::stream::decode_all(std::io::Cursor::new(&request.body)).expect("decode request")
    } else {
        request.body.clone()
    };
    serde_json::from_slice(&bytes).expect("request JSON")
}

async fn respond_to_setup_or_hook_request(
    websocket: &mut WebSocketStream<TcpStream>,
    request: &Value,
) {
    let result = match request["method"].as_str() {
        Some("environment/info") => json!({
            "shell": {"name": "powershell", "path": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"},
            "platformOs": "windows",
            "capabilities": {"networkProxyLaunch": true}
        }),
        Some("fs/getMetadata" | "fs/readFile" | "fs/readDirectory") => {
            send_exec_server_json(
                websocket,
                json!({
                    "id": request["id"],
                    "error": {"code": -32004, "message": "not found"}
                }),
            )
            .await;
            return;
        }
        Some("fs/canonicalize") => json!({"path": request["params"]["path"]}),
        Some("fs/walk") => json!({"entries": [], "errors": [], "truncated": false}),
        Some("process/write") => json!({"status": "accepted"}),
        Some("process/read") => {
            json!({"chunks": [], "nextSeq": 1, "exited": true, "exitCode": 0, "closed": true})
        }
        _ => panic!("unexpected setup or hook request: {request}"),
    };
    send_exec_server_json(websocket, json!({"id": request["id"], "result": result})).await;
}
