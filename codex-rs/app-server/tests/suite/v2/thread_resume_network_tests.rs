//! Cold resume must initialize the live proxy from the thread's restored permissions.

use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use codex_app_server_protocol::ApprovalsReviewer;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::NetworkApprovalContext;
use codex_app_server_protocol::NetworkApprovalProtocol;
use codex_app_server_protocol::NetworkPolicyAmendment;
use codex_app_server_protocol::NetworkPolicyRuleAction;
use codex_app_server_protocol::SandboxMode;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerRequestResolvedNotification;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use core_test_support::responses;
use core_test_support::skip_if_remote;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use tempfile::NamedTempFile;
use tempfile::TempDir;
use tokio::time::timeout;

#[tokio::test]
async fn cold_resume_restores_live_proxy_grants_and_persists_new_approvals() -> Result<()> {
    skip_if_remote!(
        Ok(()),
        "network probe uses a host-local test executable and loopback servers"
    );

    let server = responses::start_mock_server().await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_approval_policy("on-request")
        .enable_feature(Feature::UnifiedExec)
        .write(codex_home.path())?;
    // Permit loopback destinations to enter the approval flow. The sandbox still
    // isolates direct host access, which the reused probe checks below.
    std::fs::write(
        codex_home.path().join("requirements.toml"),
        r#"[experimental_network]
enabled = true
http_port = 0
socks_port = 0
allow_upstream_proxy = false
allow_local_binding = true

[experimental_network.domains]
"managed.example" = "allow"
"#,
    )?;
    let rules_dir = codex_home.path().join("rules");
    std::fs::create_dir_all(&rules_dir)?;
    let rules_path = rules_dir.join("default.rules");
    std::fs::write(
        &rules_path,
        "network_rule(host=\"127.0.0.2\", protocol=\"http\", decision=\"allow\")\n",
    )?;

    let thread_id = {
        let mut app = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .without_auto_env()
            .build_initialized()
            .await?;
        let start_id = app
            .send_thread_start_request(ThreadStartParams {
                sandbox: Some(SandboxMode::ReadOnly),
                approval_policy: Some(AskForApproval::OnRequest),
                approvals_reviewer: Some(ApprovalsReviewer::User),
                ..Default::default()
            })
            .await?;
        let ThreadStartResponse {
            thread,
            active_permission_profile,
            ..
        } = app.read_response(start_id).await?;
        assert_eq!(active_permission_profile, None);
        responses::mount_sse_once(
            &server,
            create_final_assistant_message_sse_response("Persisted read-only thread")?,
        )
        .await;
        let completed = app
            .start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "persist the thread".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        assert_eq!(completed.turn.status, TurnStatus::Completed);
        app.shutdown_gracefully().await?;
        thread.id
    };

    // The process default now disagrees with the saved unnamed policy. Building
    // the proxy from this default would pin its managed allowlist and reject grants.
    MockResponsesConfig::new(&server.uri())
        .with_approval_policy("on-request")
        .with_root_config("default_permissions = \":danger-full-access\"")
        .enable_feature(Feature::UnifiedExec)
        .write(codex_home.path())?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    let resume_id = app
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            ..Default::default()
        })
        .await?;
    let resumed: ThreadResumeResponse = app.read_response(resume_id).await?;
    assert_eq!(
        (
            resumed.sandbox,
            resumed.approval_policy,
            resumed.active_permission_profile
        ),
        (
            SandboxPolicy::ReadOnly {
                network_access: false
            },
            AskForApproval::OnRequest,
            None
        ),
    );

    let test_executable = std::env::current_exe()?;
    let test_executable = test_executable
        .to_str()
        .context("test executable must be UTF-8")?;
    let probe_timeout = Duration::from_secs(/* secs */ 30);
    let disconnect_signal = NamedTempFile::new()?;
    disconnect_signal.as_file().lock()?;
    for (call_id, host, expected_approvals) in [
        ("remembered-domain", "127.0.0.2", 0),
        ("new-domain", "127.0.0.3", 1),
        ("reuse-new-domain", "127.0.0.3", 0),
    ] {
        let listener = tokio::net::TcpListener::bind((host, 0)).await?;
        let target = listener.local_addr()?;
        let router = axum::Router::new().route(
            "/sandbox",
            axum::routing::get(|| async { "sandbox proxy response" }),
        );
        let _origin = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
            axum::serve(listener, router).await
        }));
        // Reuse the existing probe: it checks the HTTP response body and proves
        // the command cannot reach the origin directly outside the proxy.
        let target_env = format!("CODEX_TEST_MANAGED_NETWORK_TARGET={target}");
        let command = shlex::try_join([
            "env",
            &target_env,
            test_executable,
            "--exact",
            "suite::v2::turn_start::managed_network_sandbox_probe",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])?;
        let mut probes = Vec::new();
        if call_id == "new-domain" {
            let signal = format!(
                "CODEX_TEST_DISCONNECT_NETWORK_PROBE={}",
                disconnect_signal.path().display()
            );
            probes.push((
                "withdrawn-probe",
                format!("env {} {command}", shlex::try_quote(&signal)?),
            ));
        }
        probes.push((call_id, command));
        for (probe_id, command) in probes {
            let args = json!({
                "cmd": command,
                "shell": "/bin/sh",
                "login": false,
                "yield_time_ms": probe_timeout.as_millis(),
            });
            responses::mount_sse_once(
                &server,
                responses::sse(vec![
                    responses::ev_exec_command_call_with_args(probe_id, &args),
                    responses::ev_completed(probe_id),
                ]),
            )
            .await;
        }
        let final_response = responses::mount_sse_once(
            &server,
            create_final_assistant_message_sse_response("Network probe complete")?,
        )
        .await;
        let turn_id = app
            .send_turn_start_request(TurnStartParams {
                thread_id: thread_id.clone(),
                input: vec![UserInput::Text {
                    text: call_id.to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        let started: TurnStartResponse = app.read_response(turn_id).await?;
        let mut approvals = 0;
        let mut withdrawn_request = None;
        let mut sent_late_cancel = false;
        loop {
            match timeout(probe_timeout, app.read_next_message()).await?? {
                JSONRPCMessage::Request(request) => {
                    let request: ServerRequest = request.try_into()?;
                    let ServerRequest::CommandExecutionRequestApproval { request_id, params } =
                        request
                    else {
                        anyhow::bail!("unexpected approval request: {request:?}");
                    };
                    assert_eq!(
                        params.network_approval_context,
                        Some(NetworkApprovalContext {
                            host: host.to_string(),
                            protocol: NetworkApprovalProtocol::Http
                        }),
                    );
                    if call_id == "new-domain" {
                        if withdrawn_request.is_none() {
                            withdrawn_request = Some(request_id);
                            disconnect_signal.as_file().unlock()?;
                            continue;
                        }
                        assert!(sent_late_cancel, "old approval still visible");
                    }
                    approvals += 1;
                    assert_eq!(
                        approvals, expected_approvals,
                        "unexpected approval for {call_id}"
                    );
                    app.send_response(
                        request_id,
                        serde_json::to_value(CommandExecutionRequestApprovalResponse {
                            decision:
                                CommandExecutionApprovalDecision::ApplyNetworkPolicyAmendment {
                                    network_policy_amendment: NetworkPolicyAmendment {
                                        host: host.to_string(),
                                        action: NetworkPolicyRuleAction::Allow,
                                    },
                                },
                        })?,
                    )
                    .await?;
                }
                JSONRPCMessage::Notification(notification)
                    if notification.method == "serverRequest/resolved" =>
                {
                    let resolved: ServerRequestResolvedNotification = serde_json::from_value(
                        notification
                            .params
                            .context("serverRequest/resolved params")?,
                    )?;
                    if withdrawn_request.as_ref() == Some(&resolved.request_id) {
                        assert_eq!(resolved.thread_id, thread_id);
                        app.send_response(
                            resolved.request_id,
                            serde_json::to_value(CommandExecutionRequestApprovalResponse {
                                decision: CommandExecutionApprovalDecision::Cancel,
                            })?,
                        )
                        .await?;
                        sent_late_cancel = true;
                    }
                }
                JSONRPCMessage::Notification(notification)
                    if notification.method == "turn/completed" =>
                {
                    let completed: TurnCompletedNotification = serde_json::from_value(
                        notification.params.context("turn/completed params")?,
                    )?;
                    assert_eq!(
                        (completed.turn.id, completed.turn.status),
                        (started.turn.id, TurnStatus::Completed)
                    );
                    break;
                }
                JSONRPCMessage::Error(error) => anyhow::bail!("unexpected RPC error: {error:?}"),
                JSONRPCMessage::Notification(_) | JSONRPCMessage::Response(_) => {}
            }
        }
        assert_eq!(approvals, expected_approvals);
        let output = final_response
            .single_request()
            .function_call_output_text(call_id)
            .context("missing network probe output")?;
        assert!(
            output.contains("proxied request succeeded; direct access blocked"),
            "{call_id}: {output}"
        );
    }
    let rules = std::fs::read_to_string(rules_path)?;
    assert!(
        rules.contains("network_rule(host=\"127.0.0.3\", protocol=\"http\", decision=\"allow\""),
        "{rules}"
    );
    app.shutdown_gracefully().await?;
    Ok(())
}
