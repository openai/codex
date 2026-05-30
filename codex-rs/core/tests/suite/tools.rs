#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use codex_core::sandboxing::SandboxPermissions;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsEvent;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::request_permissions::WorkspaceMutationOperation;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::assert_regex_match;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::test_codex::local;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use regex_lite::Regex;
use serde_json::Value;
use serde_json::json;

fn tool_names(body: &Value) -> Vec<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| {
                    tool.get("name")
                        .or_else(|| tool.get("type"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn workspace_write_excluding_tmp() -> PermissionProfile {
    PermissionProfile::workspace_write_with(
        &[],
        NetworkSandboxPolicy::Restricted,
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
    )
}

async fn submit_workspace_mutation_turn(
    test: &TestCodex,
    prompt: &str,
    approval_policy: AskForApproval,
    permission_profile: PermissionProfile,
) -> Result<()> {
    let session_model = test.session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(permission_profile, test.config.cwd.as_path());
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }],
            environments: None,
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                cwd: Some(test.config.cwd.to_path_buf()),
                approval_policy: Some(approval_policy),
                approvals_reviewer: Some(ApprovalsReviewer::User),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: session_model,
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

async fn expect_workspace_mutation_request(
    test: &TestCodex,
    expected_call_id: &str,
) -> RequestPermissionsEvent {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::RequestPermissions(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::RequestPermissions(request) => {
            assert_eq!(request.call_id, expected_call_id);
            request
        }
        EventMsg::TurnComplete(_) => panic!("expected request_permissions before completion"),
        other => panic!("unexpected event: {other:?}"),
    }
}

async fn wait_for_completion(test: &TestCodex) {
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn empty_turn_environments_omits_environment_backed_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

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

    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::UnifiedExec)
            .expect("unified exec should enable for test");
    });
    let test = builder.build(&server).await?;

    test.submit_turn_with_environments("which tools are available?", Some(vec![]))
        .await?;

    let tools = tool_names(&response_mock.single_request().body_json());
    assert!(
        tools.contains(&"update_plan".to_string()),
        "non-environment tool should remain available; got {tools:?}"
    );
    for environment_tool in [
        "exec_command",
        "write_stdin",
        "apply_patch",
        "view_image",
        "set_working_directory",
        "add_workspace_root",
    ] {
        assert!(
            !tools.contains(&environment_tool.to_string()),
            "{environment_tool} should be omitted for explicit empty turn environments; got {tools:?}"
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turn_environment_selection_keeps_environment_backed_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

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

    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::UnifiedExec)
            .expect("unified exec should enable for test");
    });
    let test = builder.build(&server).await?;

    test.submit_turn_with_environments(
        "which tools are available?",
        Some(vec![local(test.config.cwd.clone())]),
    )
    .await?;

    let tools = tool_names(&response_mock.single_request().body_json());
    assert!(
        tools.contains(&"exec_command".to_string()),
        "environment tool should remain available with selected local environment; got {tools:?}"
    );
    assert!(tools.contains(&"set_working_directory".to_string()));
    assert!(tools.contains(&"add_workspace_root".to_string()));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_mutation_updates_same_batch_shell_cwd() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5-codex");
    let test = builder.build(&server).await?;
    let next_cwd = test.config.cwd.join("workspace-mutation-next");
    fs::create_dir_all(next_cwd.as_path())?;
    let mutation_call_id = "set-cwd";
    let shell_call_id = "pwd";
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                mutation_call_id,
                "set_working_directory",
                &serde_json::to_string(&json!({ "path": next_cwd }))?,
            ),
            ev_function_call(
                shell_call_id,
                "shell_command",
                &serde_json::to_string(&json!({
                    "command": "pwd",
                    "login": false,
                    "timeout_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("change directories and print the working directory")
        .await?;

    let request = second_mock.single_request();
    let mutation_output = request
        .function_call_output_text(mutation_call_id)
        .expect("mutation output");
    assert_eq!(
        serde_json::from_str::<Value>(&mutation_output)?,
        json!({
            "changed": true,
            "cwd": next_cwd,
            "workspace_roots": test.config.workspace_roots,
        })
    );
    let shell_output = request
        .function_call_output_text(shell_call_id)
        .expect("shell output");
    assert!(shell_output.contains(next_cwd.as_path().to_string_lossy().as_ref()));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_mutations_run_in_model_provided_order() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5-codex");
    let test = builder.build(&server).await?;
    let first_cwd = test.config.cwd.join("workspace-mutation-first");
    let second_cwd = first_cwd.join("nested");
    fs::create_dir_all(second_cwd.as_path())?;
    let first_mutation_call_id = "set-first-cwd";
    let second_mutation_call_id = "set-second-cwd";
    let shell_call_id = "pwd";
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                first_mutation_call_id,
                "set_working_directory",
                &serde_json::to_string(&json!({ "path": "workspace-mutation-first" }))?,
            ),
            ev_function_call(
                second_mutation_call_id,
                "set_working_directory",
                &serde_json::to_string(&json!({ "path": "nested" }))?,
            ),
            ev_function_call(
                shell_call_id,
                "shell_command",
                &serde_json::to_string(&json!({
                    "command": "pwd",
                    "login": false,
                    "timeout_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("change directories twice and print the working directory")
        .await?;

    let request = second_mock.single_request();
    let first_mutation_output = request
        .function_call_output_text(first_mutation_call_id)
        .expect("first mutation output");
    assert_eq!(
        serde_json::from_str::<Value>(&first_mutation_output)?,
        json!({
            "changed": true,
            "cwd": first_cwd,
            "workspace_roots": test.config.workspace_roots,
        })
    );
    let second_mutation_output = request
        .function_call_output_text(second_mutation_call_id)
        .expect("second mutation output");
    assert_eq!(
        serde_json::from_str::<Value>(&second_mutation_output)?,
        json!({
            "changed": true,
            "cwd": second_cwd,
            "workspace_roots": test.config.workspace_roots,
        })
    );
    let shell_output = request
        .function_call_output_text(shell_call_id)
        .expect("shell output");
    assert!(shell_output.contains(second_cwd.as_path().to_string_lossy().as_ref()));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn add_workspace_root_under_existing_root_is_noop() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5-codex");
    let test = builder.build(&server).await?;
    let covered_child = test.config.cwd.join("covered-child");
    fs::create_dir_all(covered_child.as_path())?;
    let mutation_call_id = "add-covered-root";
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                mutation_call_id,
                "add_workspace_root",
                &serde_json::to_string(&json!({ "path": covered_child }))?,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("add an already covered workspace root")
        .await?;

    let output = second_mock
        .single_request()
        .function_call_output_text(mutation_call_id)
        .expect("mutation output");
    assert_eq!(
        serde_json::from_str::<Value>(&output)?,
        json!({
            "changed": false,
            "cwd": test.config.cwd,
            "workspace_roots": test.config.workspace_roots,
        })
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_mutation_requires_session_scoped_approval_and_cancels_suffix() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5-codex");
    let test = builder.build(&server).await?;
    let external_root = tempfile::tempdir()?;
    let external_root = AbsolutePathBuf::try_from(external_root.path().canonicalize()?)?;
    let mutation_call_id = "add-external-root";
    let suffix_call_id = "suffix-pwd";
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    mutation_call_id,
                    "add_workspace_root",
                    &serde_json::to_string(&json!({ "path": external_root }))?,
                ),
                ev_function_call(
                    suffix_call_id,
                    "shell_command",
                    &serde_json::to_string(&json!({
                        "command": "pwd",
                        "login": false,
                        "timeout_ms": 1_000,
                    }))?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    submit_workspace_mutation_turn(
        &test,
        "add an external workspace root and print cwd",
        AskForApproval::OnRequest,
        workspace_write_excluding_tmp(),
    )
    .await?;
    let request = expect_workspace_mutation_request(&test, mutation_call_id).await;
    let mut expected_workspace_roots = test.config.workspace_roots.clone();
    expected_workspace_roots.push(external_root.clone());
    let expected_permissions = RequestPermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            /*read*/ None,
            Some(vec![external_root.clone()]),
        )),
        ..Default::default()
    };
    assert_eq!(request.permissions, expected_permissions);
    assert_eq!(
        request.workspace_mutation,
        Some(
            codex_protocol::request_permissions::WorkspaceMutationApprovalRequest {
                operation: WorkspaceMutationOperation::AddWorkspaceRoot,
                target: external_root.clone(),
                resulting_workspace_roots: expected_workspace_roots,
            }
        )
    );
    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: mutation_call_id.to_string(),
            response: RequestPermissionsResponse {
                permissions: expected_permissions,
                scope: PermissionGrantScope::Turn,
                strict_auto_review: false,
            },
        })
        .await?;
    wait_for_completion(&test).await;

    let mutation_output = responses
        .function_call_output_text(mutation_call_id)
        .expect("mutation output");
    assert_eq!(
        serde_json::from_str::<Value>(&mutation_output)?,
        json!({
            "code": "approval_denied",
            "message": "workspace mutation requires session-scoped approval",
            "cwd": test.config.cwd,
            "workspace_roots": test.config.workspace_roots,
        })
    );
    let suffix_output = responses
        .function_call_output_text(suffix_call_id)
        .expect("cancelled suffix output");
    assert_eq!(
        serde_json::from_str::<Value>(&suffix_output)?,
        json!({
            "code": "dependency_cancelled",
            "message": format!("cancelled because workspace mutation `{mutation_call_id}` failed"),
            "failed_mutation_call_id": mutation_call_id,
            "failed_mutation_code": "approval_denied",
        })
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approved_workspace_root_is_available_to_same_batch_shell() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5-codex");
    let test = builder.build(&server).await?;
    let external_root = tempfile::tempdir()?;
    let external_root = AbsolutePathBuf::try_from(external_root.path().canonicalize()?)?;
    let mutation_call_id = "approve-external-root";
    let shell_call_id = "pwd-external-root";
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    mutation_call_id,
                    "add_workspace_root",
                    &serde_json::to_string(&json!({ "path": external_root }))?,
                ),
                ev_function_call(
                    shell_call_id,
                    "shell_command",
                    &serde_json::to_string(&json!({
                        "command": "pwd",
                        "workdir": external_root,
                        "login": false,
                        "timeout_ms": 1_000,
                    }))?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    submit_workspace_mutation_turn(
        &test,
        "add an external workspace root and use it immediately",
        AskForApproval::OnRequest,
        workspace_write_excluding_tmp(),
    )
    .await?;
    let request = expect_workspace_mutation_request(&test, mutation_call_id).await;
    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: mutation_call_id.to_string(),
            response: RequestPermissionsResponse {
                permissions: request.permissions,
                scope: PermissionGrantScope::Session,
                strict_auto_review: false,
            },
        })
        .await?;
    wait_for_completion(&test).await;

    let mut expected_workspace_roots = test.config.workspace_roots.clone();
    expected_workspace_roots.push(external_root.clone());
    let mutation_output = responses
        .function_call_output_text(mutation_call_id)
        .expect("mutation output");
    assert_eq!(
        serde_json::from_str::<Value>(&mutation_output)?,
        json!({
            "changed": true,
            "cwd": test.config.cwd,
            "workspace_roots": expected_workspace_roots,
        })
    );
    let shell_output = responses
        .function_call_output_text(shell_call_id)
        .expect("shell output");
    assert!(shell_output.contains(external_root.as_path().to_string_lossy().as_ref()));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_working_directory_rejects_unreadable_target() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5-codex");
    let test = builder.build(&server).await?;
    let external_root = tempfile::tempdir()?;
    let external_root = AbsolutePathBuf::try_from(external_root.path().canonicalize()?)?;
    let mutation_call_id = "set-unreadable-cwd";
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                mutation_call_id,
                "set_working_directory",
                &serde_json::to_string(&json!({ "path": external_root }))?,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(Vec::new()),
        NetworkSandboxPolicy::Restricted,
    );

    test.submit_turn_with_approval_and_permission_profile(
        "change to an unreadable directory",
        AskForApproval::OnRequest,
        permission_profile,
    )
    .await?;

    let output = second_mock
        .single_request()
        .function_call_output_text(mutation_call_id)
        .expect("mutation output");
    assert_eq!(
        serde_json::from_str::<Value>(&output)?,
        json!({
            "code": "permission_denied",
            "message": format!(
                "working directory is not readable under the active permission profile: {}",
                external_root.as_path().display()
            ),
            "cwd": test.config.cwd,
            "workspace_roots": test.config.workspace_roots,
        })
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn custom_tool_unknown_returns_custom_output_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build(&server).await?;

    let call_id = "custom-unsupported";
    let tool_name = "unsupported_tool";

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call(call_id, tool_name, "\"payload\""),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn_with_approval_and_permission_profile(
        "invoke custom tool",
        AskForApproval::Never,
        PermissionProfile::Disabled,
    )
    .await?;

    let item = mock.single_request().custom_tool_call_output(call_id);
    let output = item
        .get("output")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let expected = format!("unsupported custom tool call: {tool_name}");
    assert_eq!(output, expected);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_command_escalated_permissions_rejected_then_ok() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5-codex");
    let test = builder.build(&server).await?;

    let command = "echo shell ok";
    let call_id_blocked = "shell-command-blocked";
    let call_id_success = "shell-command-success";

    let first_args = json!({
        "command": command,
        "login": false,
        "timeout_ms": 1_000,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
    });
    let second_args = json!({
        "command": command,
        "login": false,
        "timeout_ms": 1_000,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                call_id_blocked,
                "shell_command",
                &serde_json::to_string(&first_args)?,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-2"),
            ev_function_call(
                call_id_success,
                "shell_command",
                &serde_json::to_string(&second_args)?,
            ),
            ev_completed("resp-2"),
        ]),
    )
    .await;
    let third_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-3"),
        ]),
    )
    .await;

    test.submit_turn_with_approval_and_permission_profile(
        "run the shell_command script",
        AskForApproval::Never,
        PermissionProfile::Disabled,
    )
    .await?;

    let policy = AskForApproval::Never;
    let expected_message = format!(
        "approval policy is {policy:?}; reject command — you should not ask for escalated permissions if the approval policy is {policy:?}"
    );

    let blocked_output = second_mock
        .single_request()
        .function_call_output_content_and_success(call_id_blocked)
        .and_then(|(content, _)| content)
        .expect("blocked output string");
    assert_eq!(
        blocked_output, expected_message,
        "unexpected rejection message"
    );

    let success_output = third_mock
        .single_request()
        .function_call_output_content_and_success(call_id_success)
        .and_then(|(content, _)| content)
        .expect("success output string");
    assert_regex_match(
        r"(?s)^Exit code: 0\nWall time: [0-9]+(?:\.[0-9]+)? seconds\nOutput:\nshell ok\n?$",
        &success_output,
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sandbox_denied_shell_command_returns_original_output() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.4");
    let fixture = builder.build(&server).await?;

    let call_id = "sandbox-denied-shell-command";
    let target_path = fixture.workspace_path("sandbox-denied.txt");
    let sentinel = "sandbox-denied sentinel output";
    let command = format!(
        "printf {sentinel:?}; printf {content:?} > {path:?}",
        sentinel = format!("{sentinel}\n"),
        content = "sandbox denied",
        path = &target_path
    );
    let args = json!({
        "command": command,
        "login": false,
        "timeout_ms": 5_000,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    fixture
        .submit_turn_with_permission_profile(
            "run a command that should be denied by the read-only sandbox",
            PermissionProfile::read_only(),
        )
        .await?;

    let output_text = mock
        .function_call_output_text(call_id)
        .context("shell output present")?;
    let exit_code_line = output_text
        .lines()
        .next()
        .context("exit code line present")?;
    let exit_code = exit_code_line
        .strip_prefix("Exit code: ")
        .context("exit code prefix present")?
        .trim()
        .parse::<i32>()
        .context("exit code is integer")?;
    let body = output_text;

    let body_lower = body.to_lowercase();
    // Required for multi-OS.
    let has_denial = body_lower.contains("permission denied")
        || body_lower.contains("operation not permitted")
        || body_lower.contains("read-only file system");
    assert!(
        has_denial,
        "expected sandbox denial details in tool output: {body}"
    );
    assert!(
        body.contains(sentinel),
        "expected sentinel output from command to reach the model: {body}"
    );
    let target_path_str = target_path
        .to_str()
        .context("target path string representation")?;
    assert!(
        body.contains(target_path_str),
        "expected sandbox error to mention denied path: {body}"
    );
    assert!(
        !body_lower.contains("failed in sandbox"),
        "expected original tool output, found fallback message: {body}"
    );
    assert_ne!(
        exit_code, 0,
        "sandbox denial should surface a non-zero exit code"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_command_enforces_glob_deny_read_policy() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_model("gpt-5.4")
        .with_config(move |config| {
            let mut file_system_sandbox_policy = FileSystemSandboxPolicy::default();
            file_system_sandbox_policy
                .entries
                .push(FileSystemSandboxEntry {
                    path: FileSystemPath::GlobPattern {
                        pattern: format!("{}/**/*.env", config.cwd.as_path().display()),
                    },
                    access: FileSystemAccessMode::Deny,
                });
            config
                .permissions
                .set_permission_profile(PermissionProfile::from_runtime_permissions(
                    &file_system_sandbox_policy,
                    NetworkSandboxPolicy::Restricted,
                ))
                .expect("set permission profile");
        });
    let fixture = builder.build(&server).await?;

    let fixture_dir = fixture.workspace_path("glob-deny-read");
    fs::create_dir_all(&fixture_dir).context("create glob deny-read fixture directory")?;
    let denied_path = fixture_dir.join("secret.env");
    let allowed_path = fixture_dir.join("notes.txt");
    let secret = "shell glob deny-read secret";
    let allowed = "shell glob deny-read allowed";
    fs::write(&denied_path, format!("{secret}\n")).context("write denied fixture")?;
    fs::write(&allowed_path, format!("{allowed}\n")).context("write allowed fixture")?;

    let call_id = "shell-command-glob-deny-read";
    let command = format!(
        "rc=0; cat {denied_path:?} || rc=$?; cat {allowed_path:?}; exit \"$rc\"",
        denied_path = denied_path.to_string_lossy(),
        allowed_path = allowed_path.to_string_lossy(),
    );
    let args = json!({
        "command": command,
        "login": false,
        "timeout_ms": 1_000,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    let permission_profile = fixture.session_configured.permission_profile.clone();
    fixture
        .submit_turn_with_permission_profile("read the fixture files", permission_profile)
        .await?;

    let output_text = mock
        .function_call_output_text(call_id)
        .context("shell output present")?;
    let exit_code_line = output_text
        .lines()
        .next()
        .context("exit code line present")?;
    let exit_code = exit_code_line
        .strip_prefix("Exit code: ")
        .context("exit code prefix present")?
        .trim()
        .parse::<i32>()
        .context("exit code is integer")?;

    assert_ne!(
        exit_code, 0,
        "glob deny-read should surface a non-zero exit code"
    );
    assert!(
        output_text.contains(allowed),
        "expected allowed file contents in shell output: {output_text}"
    );
    assert!(
        !output_text.contains(secret),
        "denied file contents leaked into shell output: {output_text}"
    );
    let output_lower = output_text.to_lowercase();
    let has_denial = output_lower.contains("permission denied")
        || output_lower.contains("operation not permitted")
        || output_lower.contains("read-only file system");
    assert!(
        has_denial,
        "expected sandbox denial details in shell output: {output_text}"
    );

    Ok(())
}

async fn collect_tools(use_unified_exec: bool) -> Result<Vec<String>> {
    let server = start_mock_server().await;

    let responses = vec![sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-1"),
    ])];
    let mock = mount_sse_sequence(&server, responses).await;

    let mut builder = test_codex().with_config(move |config| {
        if use_unified_exec {
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("test config should allow feature update");
        } else {
            config
                .features
                .disable(Feature::UnifiedExec)
                .expect("test config should allow feature update");
        }
    });
    let test = builder.build(&server).await?;

    test.submit_turn_with_approval_and_permission_profile(
        "list tools",
        AskForApproval::Never,
        PermissionProfile::Disabled,
    )
    .await?;

    let first_body = mock.single_request().body_json();
    Ok(tool_names(&first_body))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_spec_toggle_end_to_end() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let tools_disabled = collect_tools(/*use_unified_exec*/ false).await?;
    assert!(
        !tools_disabled.iter().any(|name| name == "exec_command"),
        "tools list should not include exec_command when disabled: {tools_disabled:?}"
    );
    assert!(
        !tools_disabled.iter().any(|name| name == "write_stdin"),
        "tools list should not include write_stdin when disabled: {tools_disabled:?}"
    );

    let tools_enabled = collect_tools(/*use_unified_exec*/ true).await?;
    assert!(
        tools_enabled.iter().any(|name| name == "exec_command"),
        "tools list should include exec_command when enabled: {tools_enabled:?}"
    );
    assert!(
        tools_enabled.iter().any(|name| name == "write_stdin"),
        "tools list should include write_stdin when enabled: {tools_enabled:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_command_timeout_includes_timeout_prefix_and_metadata() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5-codex");
    let test = builder.build(&server).await?;

    let call_id = "shell-command-timeout";
    let timeout_ms = 50u64;
    let args = json!({
        "command": "yes line | head -n 400; sleep 1",
        "login": false,
        "timeout_ms": timeout_ms,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn_with_approval_and_permission_profile(
        "run a long command",
        AskForApproval::Never,
        PermissionProfile::Disabled,
    )
    .await?;

    let timeout_item = second_mock.single_request().function_call_output(call_id);

    let output_str = timeout_item
        .get("output")
        .and_then(Value::as_str)
        .expect("timeout output string");

    // The exec path can report a timeout in two ways depending on timing:
    // 1) Structured JSON with exit_code 124 and a timeout prefix (preferred), or
    // 2) A plain error string if the child is observed as killed by a signal first.
    if let Ok(output_json) = serde_json::from_str::<Value>(output_str) {
        assert_eq!(
            output_json["metadata"]["exit_code"].as_i64(),
            Some(124),
            "expected timeout exit code 124",
        );

        let stdout = output_json["output"].as_str().unwrap_or_default();
        assert!(
            stdout.contains("command timed out"),
            "timeout output missing `command timed out`: {stdout}"
        );
    } else {
        let normalized_output = output_str
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .trim_end_matches('\n')
            .to_string();

        let shell_output_pattern = r"(?s)^Exit code: 124\nWall time: [0-9]+(?:\.[0-9]+)? seconds\nOutput:\ncommand timed out after [0-9]+ milliseconds\n(?:.*)?$";
        if Regex::new(shell_output_pattern)
            .expect("shell timeout output regex should compile")
            .is_match(&normalized_output)
        {
            return Ok(());
        }

        // Fallback: accept the signal classification path to deflake the test.
        let signal_pattern = r"(?is)^execution error:.*signal.*$";
        assert_regex_match(signal_pattern, output_str);
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_command_timeout_handles_background_grandchild_stdout() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.4").with_config(|config| {
        config
            .permissions
            .set_permission_profile(PermissionProfile::Disabled)
            .expect("set permission profile");
    });
    let test = builder.build(&server).await?;

    let call_id = "shell-command-grandchild-timeout";
    let pid_path = test.cwd.path().join("grandchild_pid.txt");
    let script_path = test.cwd.path().join("spawn_detached.py");
    let script = format!(
        r#"import subprocess
import time
from pathlib import Path

# Spawn a detached grandchild that inherits stdout/stderr so the pipe stays open.
proc = subprocess.Popen(["/bin/sh", "-c", "sleep 60"], start_new_session=True)
Path({pid_path:?}).write_text(str(proc.pid))
time.sleep(60)
"#
    );
    fs::write(&script_path, script)?;

    let args = json!({
        "command": format!("python3 {:?}", script_path.to_string_lossy()),
        "login": false,
        "timeout_ms": 200,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let start = Instant::now();
    let output_str = tokio::time::timeout(Duration::from_secs(10), async {
        test.submit_turn_with_approval_and_permission_profile(
            "run a command with a detached grandchild",
            AskForApproval::Never,
            PermissionProfile::Disabled,
        )
        .await?;
        let timeout_item = second_mock.single_request().function_call_output(call_id);
        timeout_item
            .get("output")
            .and_then(Value::as_str)
            .map(str::to_string)
            .context("timeout output string")
    })
    .await
    .context("exec call should not hang waiting for grandchild pipes to close")??;
    let elapsed = start.elapsed();

    if let Ok(output_json) = serde_json::from_str::<Value>(&output_str) {
        assert_eq!(
            output_json["metadata"]["exit_code"].as_i64(),
            Some(124),
            "expected timeout exit code 124",
        );
    } else {
        let timeout_pattern = r"(?is)command timed out|timeout";
        assert_regex_match(timeout_pattern, &output_str);
    }

    assert!(
        elapsed < Duration::from_secs(9),
        "command should return shortly after timeout even with live grandchildren: {elapsed:?}"
    );

    if let Ok(pid_str) = fs::read_to_string(&pid_path)
        && let Ok(pid) = pid_str.trim().parse::<libc::pid_t>()
    {
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }

    Ok(())
}
