//! Exercises approval scope across a sandbox denial reported by a fake executor.

use super::PushedExecScenario;
use super::accept_initialized_exec_server;
use super::read_exec_server_json;
use super::respond_environment_info;
use super::send_exec_server_json;
use anyhow::Context;
use anyhow::Result;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GranularApprovalConfig;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::startup::STARTUP_TIMEOUT;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_rechecks_unsandboxed_retry_after_sandboxed_approval() -> Result<()> {
    check_guardian_retry(/*allow_retry*/ false).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_approved_retry_runs_without_the_sandbox() -> Result<()> {
    check_guardian_retry(/*allow_retry*/ true).await
}

async fn check_guardian_retry(allow_retry: bool) -> Result<()> {
    let server = start_mock_server().await;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let exec_server_url = format!("ws://{}", listener.local_addr()?);
    let (stop_executor, mut stopped) = oneshot::channel();
    let executor = tokio::spawn(async move {
        let mut websocket = accept_initialized_exec_server(listener).await;
        let mut starts = Vec::new();
        loop {
            let request = tokio::select! {
                _ = &mut stopped => return starts,
                request = read_exec_server_json(&mut websocket, STARTUP_TIMEOUT) => request,
            };
            let result = match request["method"].as_str() {
                Some("environment/info") => {
                    respond_environment_info(
                        &mut websocket,
                        &request["id"],
                        PushedExecScenario::Complete,
                    )
                    .await;
                    continue;
                }
                Some("fs/getMetadata" | "fs/readFile" | "fs/readDirectory") => {
                    send_exec_server_json(
                        &mut websocket,
                        json!({
                            "id": request["id"],
                            "error": {"code": -32004, "message": "not found"}
                        }),
                    )
                    .await;
                    continue;
                }
                Some("fs/canonicalize") => json!({"path": request["params"]["path"]}),
                Some("fs/walk") => json!({"entries": [], "errors": [], "truncated": false}),
                Some("process/start") => {
                    starts.push(request["params"].clone());
                    let process_id = &request["params"]["processId"];
                    // Report completion before acknowledging start, so collecting the
                    // initial result cannot race the tool's yield deadline. A retry
                    // would succeed, making an approval bypass observable below.
                    let denied = starts.len() == 1;
                    for message in [
                        json!({"method": "process/exited", "params": {
                            "processId": process_id, "seq": 1,
                            "exitCode": if denied { 1 } else { 0 }, "sandboxDenied": denied,
                        }}),
                        json!({"method": "process/closed", "params": {"processId": process_id, "seq": 2}}),
                    ] {
                        send_exec_server_json(&mut websocket, message).await;
                    }
                    json!({"processId": process_id})
                }
                Some("process/read") => json!({
                    "chunks": [], "nextSeq": 3, "exited": true, "closed": true,
                    "exitCode": if starts.len() == 1 { 1 } else { 0 },
                    "sandboxDenied": starts.len() == 1, "failure": null,
                }),
                Some("process/terminate") => json!({"running": false}),
                method => panic!("unexpected exec-server request: {method:?}"),
            };
            send_exec_server_json(
                &mut websocket,
                json!({"id": request["id"], "result": result}),
            )
            .await;
        }
    });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_function_call(
                    "sandbox-retry",
                    "exec_command",
                    &json!({
                        "cmd": "touch marker", "login": false,
                        "sandbox_permissions": "use_default",
                    })
                    .to_string(),
                ),
                ev_completed("parent-command"),
            ]),
            sse(vec![
                ev_assistant_message("initial-review", r#"{"outcome":"allow"}"#),
                ev_completed("initial-review"),
            ]),
            sse(vec![
                ev_assistant_message(
                    "retry-review",
                    if allow_retry {
                        r#"{"outcome":"allow"}"#
                    } else {
                        r#"{"outcome":"deny","rationale":"The unsandboxed retry is not authorized."}"#
                    },
                ),
                ev_completed("retry-review"),
            ]),
            sse(vec![ev_completed("parent-done")]),
        ],
    )
    .await;
    let test = test_codex()
        .with_exec_server_url(exec_server_url)
        .with_config(|config| {
            config.project_doc_max_bytes = 0;
            let rules_dir = config.codex_home.join("rules");
            fs::create_dir_all(&rules_dir).expect("create rules directory");
            fs::write(
                rules_dir.join("default.rules"),
                r#"prefix_rule(pattern=["touch"], decision="prompt")"#,
            )
            .expect("write prompt rule");
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.permissions.approval_policy =
                Constrained::allow_any(AskForApproval::Granular(GranularApprovalConfig {
                    sandbox_approval: true,
                    rules: true,
                    skill_approval: false,
                    request_permissions: false,
                    mcp_elicitations: false,
                }));
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("read-only profile");
            config
                .features
                .disable(Feature::GuardianV2)
                .expect("synchronous Guardian reviews");
        })
        // The fake executor supplies the denial on every host, including remote-test CI.
        .build(&server)
        .await?;
    test.submit_text_turn("Run the command with limited permissions.")
        .await?;
    stop_executor.send(()).expect("executor still serving");
    let starts = executor.await?;
    let output = responses
        .function_call_output_text("sandbox-retry")
        .context("tool result")?;
    if allow_retry {
        assert_eq!(starts.len(), 2, "{output}");
        assert_eq!(starts[1]["sandbox"], Value::Null);
    } else {
        assert_eq!(starts.len(), 1, "{output}");
        assert!(
            output.contains("The unsandboxed retry is not authorized."),
            "{output}"
        );
    }
    assert!(
        starts[0]["sandbox"].is_object(),
        "the first attempt must be sandboxed"
    );

    let requests = responses.requests();
    let actions = requests
        .iter()
        .filter(|request| request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian")
        .map(|request| {
            let text = request.message_input_texts("user").join("\n");
            let (_, action) = text
                .rsplit_once("Planned action JSON:\n")
                .context("planned action")?;
            let (action, _) = action
                .split_once("\n>>> APPROVAL REQUEST END")
                .context("end of planned action")?;
            Ok(serde_json::from_str::<Value>(action)?)
        })
        .collect::<Result<Vec<_>>>()?;
    let action = json!({
        "tool": "exec_command", "command": ["/bin/zsh", "-c", "touch marker"],
        "environment_id": codex_exec_server::REMOTE_ENVIRONMENT_ID,
        "cwd": test.config.cwd.to_string_lossy(), "tty": false,
        "sandbox_permissions": "use_default",
    });
    let mut retry = action.clone();
    retry["sandbox_permissions"] = json!("require_escalated");
    assert_eq!(actions, vec![action, retry]);
    Ok(())
}
