//! Exercises remote network review startup and cleanup, including MXC's local-binding default.

use super::accept_initialized_exec_server;
use super::read_exec_server_json;
use super::send_exec_server_json;
use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::TurnInputRequest;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::items::CommandExecutionStatus;
use codex_protocol::items::TurnItem;
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

#[test_case(SandboxType::WindowsRestrictedToken, "allow_local_binding = true", Ok(true); "explicit_local_binding")]
#[test_case(SandboxType::WindowsMxc, "", Ok(true); "mxc_default_local_binding")]
#[test_case(SandboxType::WindowsMxc, "allow_local_binding = false", Err("MXC cannot enforce allow_local_binding=false"); "mxc_rejects_false")]
#[test_case(SandboxType::WindowsRestrictedToken, "", Ok(false); "non_mxc_default_local_binding")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_local_binding_policy_and_review_cleanup(
    sandbox_type: SandboxType,
    local_binding: &'static str,
    expected_local_binding: Result<bool, &'static str>,
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
        let start = loop {
            let request = read_exec_server_json(&mut websocket, STARTUP_TIMEOUT).await;
            if request["method"] == "process/start" {
                break request;
            }
            respond_setup_request(&mut websocket, &request).await;
        };
        assert!(
            start["params"]["networkProxy"]["policyDecisionTimeoutMs"]
                .as_u64()
                .is_some()
        );
        let allow_local_binding =
            expected_local_binding.expect("invalid MXC config must not launch");
        assert_eq!(
            start["params"]["networkProxy"]["proxy"]["allowLocalBinding"],
            allow_local_binding
        );
        let process_id = &start["params"]["processId"];
        send_exec_server_json(
            &mut websocket,
            json!({"id": start["id"], "result": {"processId": process_id}}),
        )
        .await;
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
                        respond_setup_request(&mut websocket, &request).await;
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
                _ => respond_setup_request(&mut websocket, &request).await,
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
            #[cfg(windows)]
            config.set_windows_sandbox_enabled(/*value*/ true);
        })
        // This test supplies its own fake executor and must not select a CI executor.
        .build(&server)
        .await?;
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

async fn respond_setup_request(websocket: &mut WebSocketStream<TcpStream>, request: &Value) {
    match request["method"].as_str() {
        Some("environment/info") => {
            send_exec_server_json(websocket, json!({
                "id": request["id"],
                "result": {
                    "shell": {"name": "powershell", "path": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"},
                    "platformOs": "windows",
                    "capabilities": {"networkProxyLaunch": true}
                }
            })).await;
        }
        Some("fs/getMetadata" | "fs/readFile" | "fs/readDirectory") => {
            send_exec_server_json(
                websocket,
                json!({
                    "id": request["id"],
                    "error": {"code": -32004, "message": "not found"}
                }),
            )
            .await;
        }
        Some("fs/canonicalize") => {
            send_exec_server_json(
                websocket,
                json!({
                    "id": request["id"],
                    "result": {"path": request["params"]["path"]}
                }),
            )
            .await;
        }
        Some("fs/walk") => {
            send_exec_server_json(
                websocket,
                json!({
                    "id": request["id"],
                    "result": {"entries": [], "errors": [], "truncated": false}
                }),
            )
            .await;
        }
        _ => panic!("unexpected setup request: {request}"),
    }
}
