use anyhow::Context;
use anyhow::Result;
use codex_config::types::ApprovalsReviewer;
use codex_core::config::Constrained;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_exec_server::RemoveOptions;
use codex_features::Feature;
use codex_protocol::approvals::NetworkApprovalContext;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::protocol::TurnEnvironmentSelections;
use codex_protocol::user_input::UserInput;
use codex_utils_path_uri::PathUri;
use core_test_support::PathBufExt;
use core_test_support::PathExt;
use core_test_support::managed_network_requirements_loader;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_host_windows;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_no_remote_env;
use core_test_support::skip_if_sandbox;
use core_test_support::skip_if_target_windows;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_with_timeout;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tempfile::TempDir;

const NETWORK_TEST_HOST: &str = "codex-network-test.invalid";
const NETWORK_TEST_TARGET: &str = "http://codex-network-test.invalid:80";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "requires the trusted Linux proxy bridge"
)]
async fn guardian_receives_exact_triggers_for_concurrent_network_requests() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let barrier_dir = TempDir::new_in(test.cwd.path())?;
    let first_marker = barrier_dir.path().join("first");
    let second_marker = barrier_dir.path().join("second");
    let network_command = |marker: &PathBuf, peer_marker: &PathBuf, host: &str| {
        format!(
            "touch '{}' && while [ ! -e '{}' ]; do sleep 0.01; done && python3 -c \"import urllib.request; urllib.request.build_opener(urllib.request.ProxyHandler()).open('http://{host}', timeout=10).read()\"",
            marker.display(),
            peer_marker.display(),
        )
    };
    let first_command = network_command(&first_marker, &second_marker, "1.1.1.1");
    let second_command = network_command(&second_marker, &first_marker, "8.8.8.8");
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            !is_guardian_request(request)
                && request_body_contains(request, "run both network requests")
                && !request_body_contains(request, "exec-network-first")
        },
        sse(vec![
            ev_response_created("resp-network-concurrent"),
            ev_function_call(
                "exec-network-first",
                "exec_command",
                &serde_json::to_string(&network_exec_args(&first_command))?,
            ),
            ev_function_call(
                "exec-network-second",
                "exec_command",
                &serde_json::to_string(&network_exec_args(&second_command))?,
            ),
            ev_completed("resp-network-concurrent"),
        ]),
    )
    .await;
    let first_guardian = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| guardian_request_is_for(request, "exec-network-first"),
        sse(vec![
            ev_response_created("resp-network-guardian-1"),
            ev_assistant_message("msg-network-guardian-1", r#"{"outcome":"deny"}"#),
            ev_completed("resp-network-guardian-1"),
        ]),
    )
    .await;
    let second_guardian = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| guardian_request_is_for(request, "exec-network-second"),
        sse(vec![
            ev_response_created("resp-network-guardian-2"),
            ev_assistant_message("msg-network-guardian-2", r#"{"outcome":"deny"}"#),
            ev_completed("resp-network-guardian-2"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            !is_guardian_request(request)
                && request_body_contains(request, "exec-network-first")
                && request_body_contains(request, "exec-network-second")
        },
        sse(vec![
            ev_response_created("resp-network-done"),
            ev_assistant_message("msg-network-done", "done"),
            ev_completed("resp-network-done"),
        ]),
    )
    .await;

    submit_managed_network_turn(
        &test,
        "run both network requests",
        vec![local(test.config.cwd.clone())],
        ApprovalsReviewer::AutoReview,
        AskForApproval::OnRequest,
    )
    .await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let actual_triggers = loop {
        let mut actual_triggers = guardian_network_triggers(&[&first_guardian, &second_guardian])?;
        actual_triggers.sort_unstable();
        actual_triggers.dedup();
        if actual_triggers.len() == 2 {
            break actual_triggers;
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for both Guardian network reviews");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    wait_for_turn_complete(&test).await;

    assert_eq!(
        actual_triggers,
        vec![
            ("exec-network-first".to_string(), first_command),
            ("exec-network-second".to_string(), second_command),
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "requires the trusted Linux proxy bridge"
)]
async fn guardian_receives_exact_trigger_for_single_network_request() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let command = "python3 -c \"import urllib.request; opener = urllib.request.build_opener(urllib.request.ProxyHandler()); print('OK:' + opener.open('http://1.1.1.1', timeout=10).read().decode(errors='replace'))\"".to_string();
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-network-single"),
                ev_function_call(
                    "exec-network-single",
                    "exec_command",
                    &serde_json::to_string(&network_exec_args(&command))?,
                ),
                ev_completed("resp-network-single"),
            ]),
            sse(vec![
                ev_response_created("resp-network-guardian"),
                ev_assistant_message("msg-network-guardian", r#"{"outcome":"deny"}"#),
                ev_completed("resp-network-guardian"),
            ]),
            sse(vec![
                ev_response_created("resp-network-done"),
                ev_assistant_message("msg-network-done", "done"),
                ev_completed("resp-network-done"),
            ]),
        ],
    )
    .await;

    submit_managed_network_turn(
        &test,
        "run one network request",
        vec![local(test.config.cwd.clone())],
        ApprovalsReviewer::AutoReview,
        AskForApproval::OnRequest,
    )
    .await?;
    wait_for_turn_complete(&test).await;

    assert_eq!(
        guardian_network_triggers(&[&responses])?,
        vec![("exec-network-single".to_string(), command)]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approved_network_host_for_one_environment_still_prompts_in_another() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let local_cwd = TempDir::new()?;
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-network-approval-{}",
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
    let environments = vec![
        local(local_cwd.path().abs()),
        TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&remote_cwd),
        },
    ];

    mount_exec_network_turn(
        &server,
        "resp-network-local",
        "exec-network-local",
        network_fetch_args(LOCAL_ENVIRONMENT_ID),
    )
    .await?;
    submit_managed_network_turn(
        &test,
        "fetch from the local environment",
        environments.clone(),
        ApprovalsReviewer::User,
        AskForApproval::UnlessTrusted,
    )
    .await?;
    let approval = expect_network_approval(&test, LOCAL_ENVIRONMENT_ID).await?;
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::ApprovedForSession,
        })
        .await?;
    wait_for_turn_complete(&test).await;

    mount_exec_network_turn(
        &server,
        "resp-network-remote",
        "exec-network-remote",
        network_fetch_args(REMOTE_ENVIRONMENT_ID),
    )
    .await?;
    submit_managed_network_turn(
        &test,
        "fetch from the remote environment",
        environments.clone(),
        ApprovalsReviewer::User,
        AskForApproval::UnlessTrusted,
    )
    .await?;
    let approval = expect_network_approval(&test, REMOTE_ENVIRONMENT_ID).await?;
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::Denied,
        })
        .await?;
    wait_for_turn_complete(&test).await;

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

async fn managed_network_unified_exec_test(server: &wiremock::MockServer) -> Result<TestCodex> {
    let home = Arc::new(TempDir::new()?);
    fs::write(
        home.path().join("config.toml"),
        r#"default_permissions = "workspace"

[permissions.workspace.filesystem]
":minimal" = "read"

[permissions.workspace.network]
enabled = true
mode = "limited"
allow_local_binding = true
"#,
    )?;
    let approval_policy = AskForApproval::OnRequest;
    let permission_profile = PermissionProfile::workspace_write_with(
        &[],
        NetworkSandboxPolicy::Enabled,
        /*exclude_tmpdir_env_var*/ false,
        /*exclude_slash_tmp*/ false,
    );
    let permission_profile_for_config = permission_profile.clone();
    let mut builder = test_codex()
        .with_home(home)
        .with_cloud_config_bundle(managed_network_requirements_loader())
        .with_config(move |config| {
            config.use_experimental_unified_exec_tool = true;
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("test config should allow feature update");
            config.permissions.approval_policy = Constrained::allow_any(approval_policy);
            config
                .permissions
                .set_permission_profile(permission_profile_for_config)
                .expect("set permission profile");
        });
    let test = builder.build_with_remote_and_local_env(server).await?;
    assert!(
        test.config.managed_network_requirements_enabled(),
        "expected managed network requirements to be enabled"
    );
    assert!(
        test.config.permissions.network.is_some(),
        "expected managed network proxy config to be present"
    );
    test.session_configured
        .network_proxy
        .as_ref()
        .expect("expected runtime managed network proxy addresses");

    Ok(test)
}

async fn mount_exec_network_turn(
    server: &wiremock::MockServer,
    response_prefix: &str,
    call_id: &str,
    args: Value,
) -> Result<ResponseMock> {
    let responses = vec![
        sse(vec![
            ev_response_created(&format!("{response_prefix}-1")),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed(&format!("{response_prefix}-1")),
        ]),
        sse(vec![
            ev_response_created(&format!("{response_prefix}-2")),
            ev_assistant_message(&format!("{response_prefix}-msg"), "done"),
            ev_completed(&format!("{response_prefix}-2")),
        ]),
    ];
    Ok(mount_sse_sequence(server, responses).await)
}

fn network_fetch_args(environment_id: &str) -> Value {
    let command = format!(
        "python3 -c \"import urllib.request; opener = urllib.request.build_opener(urllib.request.ProxyHandler()); print('OK:' + opener.open('http://{NETWORK_TEST_HOST}', timeout=2).read().decode(errors='replace'))\""
    );
    let mut args = network_exec_args(&command);
    args["environment_id"] = json!(environment_id);
    args
}

fn network_exec_args(command: &str) -> Value {
    json!({
        "shell": "/bin/sh",
        "cmd": command,
        "login": false,
        "yield_time_ms": 1_000,
    })
}

async fn submit_managed_network_turn(
    test: &TestCodex,
    prompt: &str,
    environments: Vec<TurnEnvironmentSelection>,
    approvals_reviewer: ApprovalsReviewer,
    approval_policy: AskForApproval,
) -> Result<()> {
    let permission_profile = PermissionProfile::workspace_write_with(
        &[],
        NetworkSandboxPolicy::Enabled,
        /*exclude_tmpdir_env_var*/ false,
        /*exclude_slash_tmp*/ false,
    );
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(permission_profile, test.config.cwd.as_path());
    let turn_environment_selections =
        TurnEnvironmentSelections::new(test.config.cwd.clone(), environments);

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
                approvals_reviewer: Some(approvals_reviewer),
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

    Ok(())
}

fn decoded_request_body(request: &wiremock::Request) -> Option<Vec<u8>> {
    let is_zstd = request
        .headers
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|entry| entry.trim().eq_ignore_ascii_case("zstd"))
        });
    if is_zstd {
        zstd::stream::decode_all(std::io::Cursor::new(&request.body)).ok()
    } else {
        Some(request.body.clone())
    }
}

fn request_body_contains(request: &wiremock::Request, text: &str) -> bool {
    decoded_request_body(request)
        .and_then(|body| String::from_utf8(body).ok())
        .is_some_and(|body| body.contains(text))
}

fn is_guardian_request(request: &wiremock::Request) -> bool {
    decoded_request_body(request)
        .and_then(|body| serde_json::from_slice::<Value>(&body).ok())
        .is_some_and(|body| {
            body.pointer("/client_metadata/x-openai-subagent")
                .and_then(Value::as_str)
                == Some("guardian")
        })
}

fn guardian_request_is_for(request: &wiremock::Request, call_id: &str) -> bool {
    decoded_request_body(request)
        .and_then(|body| serde_json::from_slice::<Value>(&body).ok())
        .filter(|body| {
            body.pointer("/client_metadata/x-openai-subagent")
                .and_then(Value::as_str)
                == Some("guardian")
        })
        .and_then(|body| {
            body.get("input")
                .and_then(Value::as_array)
                .and_then(|input| {
                    input
                        .iter()
                        .rev()
                        .find(|item| item.get("role").and_then(Value::as_str) == Some("user"))
                })
                .cloned()
        })
        .is_some_and(|latest_user_message| latest_user_message.to_string().contains(call_id))
}

fn guardian_network_triggers(responses: &[&ResponseMock]) -> Result<Vec<(String, String)>> {
    responses
        .iter()
        .flat_map(|responses| responses.requests())
        .filter(|request| {
            request.body_json()["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
        })
        .map(|request| {
            let user_texts = request.message_input_texts("user");
            let action: Value = serde_json::from_str(
                user_texts
                    .iter()
                    .find(|text| text.contains("\"tool\": \"network_access\""))
                    .context("expected network access JSON in Guardian request")?
                    .trim(),
            )?;
            Ok((
                action
                    .pointer("/trigger/callId")
                    .and_then(Value::as_str)
                    .context("expected exact trigger call id")?
                    .to_string(),
                action
                    .pointer("/trigger/command/2")
                    .and_then(Value::as_str)
                    .context("expected exact trigger command")?
                    .to_string(),
            ))
        })
        .collect()
}

async fn expect_network_approval(
    test: &TestCodex,
    expected_environment_id: &str,
) -> Result<ExecApprovalRequestEvent> {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let remaining = deadline
        .checked_duration_since(std::time::Instant::now())
        .context("timed out waiting for network approval request")?;
    let event = wait_for_event_with_timeout(
        &test.codex,
        |event| {
            matches!(
                event,
                EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
            )
        },
        remaining,
    )
    .await;
    match event {
        EventMsg::ExecApprovalRequest(approval) => {
            assert_eq!(
                approval.command,
                vec![
                    "network-access".to_string(),
                    NETWORK_TEST_TARGET.to_string()
                ]
            );
            assert_eq!(
                approval.network_approval_context,
                Some(NetworkApprovalContext {
                    host: NETWORK_TEST_HOST.to_string(),
                    protocol: NetworkApprovalProtocol::Http,
                })
            );
            assert_eq!(
                approval.environment_id.as_deref(),
                Some(expected_environment_id)
            );
            Ok(approval)
        }
        EventMsg::TurnComplete(_) => {
            panic!("expected network approval request before completion");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

async fn wait_for_turn_complete(test: &TestCodex) {
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
}
