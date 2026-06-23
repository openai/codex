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
use codex_protocol::permissions::FileSystemSandboxPolicy;
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
use core_test_support::get_remote_test_env;
use core_test_support::managed_network_requirements_loader;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::skip_if_windows;
use core_test_support::skip_if_wine_exec;
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

#[derive(Clone, Copy)]
enum ManagedNetworkTestEnvironments {
    Local,
    RemoteAndLocal,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_receives_all_possible_triggers_for_concurrent_network_requests() -> Result<()> {
    skip_if_wine_exec!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_windows!(Ok(()));

    let server = start_mock_server().await;
    let test =
        managed_network_unified_exec_test(&server, ManagedNetworkTestEnvironments::Local).await?;
    let barrier_dir = TempDir::new()?;
    let first_marker = barrier_dir.path().join("first");
    let second_marker = barrier_dir.path().join("second");
    let network_command = |marker: &PathBuf, peer_marker: &PathBuf| {
        format!(
            "touch '{}' && while [ ! -e '{}' ]; do sleep 0.01; done && python3 -c \"import urllib.request; opener = urllib.request.build_opener(urllib.request.ProxyHandler()); opener.open('http://{NETWORK_TEST_HOST}', timeout=10).read()\"",
            marker.display(),
            peer_marker.display(),
        )
    };
    let first_command = network_command(&first_marker, &second_marker);
    let second_command = network_command(&second_marker, &first_marker);
    let first_args = network_exec_args(LOCAL_ENVIRONMENT_ID, &first_command);
    let second_args = network_exec_args(LOCAL_ENVIRONMENT_ID, &second_command);
    let denial_rationale = "The destination is not authorized by the user.";
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-network-concurrent"),
                ev_function_call(
                    "exec-network-first",
                    "exec_command",
                    &serde_json::to_string(&first_args)?,
                ),
                ev_function_call(
                    "exec-network-second",
                    "exec_command",
                    &serde_json::to_string(&second_args)?,
                ),
                ev_completed("resp-network-concurrent"),
            ]),
            sse(vec![
                ev_response_created("resp-network-guardian"),
                ev_assistant_message(
                    "msg-network-guardian",
                    &json!({
                        "risk_level": "high",
                        "user_authorization": "low",
                        "outcome": "deny",
                        "rationale": denial_rationale,
                    })
                    .to_string(),
                ),
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
        "run both network requests",
        vec![local(test.config.cwd.clone())],
        AskForApproval::OnRequest,
        ApprovalsReviewer::AutoReview,
    )
    .await?;
    wait_for_turn_complete(&test).await;

    let guardian_request = responses
        .requests()
        .into_iter()
        .find(|request| {
            request.body_json()["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
        })
        .expect("expected Guardian network review request");
    let user_texts = guardian_request.message_input_texts("user");
    let action: Value = serde_json::from_str(
        user_texts
            .iter()
            .find(|text| text.contains("\"possibleTriggers\""))
            .context("expected network access JSON in Guardian request")?
            .trim(),
    )?;
    assert_eq!(action.get("trigger"), None);
    let mut actual_triggers = action["possibleTriggers"]
        .as_array()
        .context("expected possible network access triggers")?
        .iter()
        .map(|trigger| {
            Ok((
                trigger["callId"]
                    .as_str()
                    .context("expected possible trigger call id")?,
                trigger["command"]
                    .as_array()
                    .and_then(|command| command.last())
                    .and_then(Value::as_str)
                    .context("expected possible trigger command")?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    actual_triggers.sort_unstable();
    let mut expected_triggers = vec![
        ("exec-network-first", first_command.as_str()),
        ("exec-network-second", second_command.as_str()),
    ];
    expected_triggers.sort_unstable();
    assert_eq!(actual_triggers, expected_triggers);
    assert!(
        user_texts
            .concat()
            .contains("Approve if at least one candidate")
    );
    assert!(responses.requests().into_iter().any(|request| {
        request.message_input_texts("developer").iter().any(|text| {
            text.contains("<guardian_network_access_denied>")
                && text.contains(NETWORK_TEST_TARGET)
                && text.contains(denial_rationale)
        })
    }));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approved_network_host_for_one_environment_still_prompts_in_another() -> Result<()> {
    skip_if_wine_exec!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_windows!(Ok(()));
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let server = start_mock_server().await;
    let test =
        managed_network_unified_exec_test(&server, ManagedNetworkTestEnvironments::RemoteAndLocal)
            .await?;
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
        AskForApproval::OnFailure,
        ApprovalsReviewer::User,
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
        AskForApproval::OnFailure,
        ApprovalsReviewer::User,
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

async fn managed_network_unified_exec_test(
    server: &wiremock::MockServer,
    environments: ManagedNetworkTestEnvironments,
) -> Result<TestCodex> {
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
    let permission_profile_for_config = managed_network_test_permission_profile();
    let mut builder = test_codex()
        .with_home(home)
        .with_cloud_config_bundle(managed_network_requirements_loader())
        .with_config(move |config| {
            config.use_experimental_unified_exec_tool = true;
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("test config should allow feature update");
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnFailure);
            config
                .permissions
                .set_permission_profile(permission_profile_for_config)
                .expect("set permission profile");
        });
    let test = match environments {
        ManagedNetworkTestEnvironments::Local => builder.build(server).await?,
        ManagedNetworkTestEnvironments::RemoteAndLocal => {
            builder.build_with_remote_and_local_env(server).await?
        }
    };
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

fn managed_network_test_permission_profile() -> PermissionProfile {
    PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::unrestricted(),
        NetworkSandboxPolicy::Enabled,
    )
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
    network_exec_args(environment_id, &command)
}

fn network_exec_args(environment_id: &str, command: &str) -> Value {
    json!({
        "shell": "/bin/sh",
        "cmd": command,
        "login": false,
        "yield_time_ms": 1_000,
        "environment_id": environment_id,
    })
}

async fn submit_managed_network_turn(
    test: &TestCodex,
    prompt: &str,
    environments: Vec<TurnEnvironmentSelection>,
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
) -> Result<()> {
    let permission_profile = managed_network_test_permission_profile();
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
