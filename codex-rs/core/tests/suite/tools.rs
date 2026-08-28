#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used)]

use std::fs;

use anyhow::Context;
use anyhow::Result;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_core::sandboxing::SandboxPermissions;
use codex_features::Feature;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceTool;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::assert_regex_match;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_custom_tool_call_with_namespace;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_response_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::responses::strip_response_item_ids_from_json;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::local;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::Value;
use serde_json::json;
use test_case::test_case;
use wiremock::ResponseTemplate;

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

#[test_case(false, false; "normal sampling")]
#[test_case(true, false; "pre sampling compaction")]
#[test_case(false, true; "namespace collision")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_tool_collisions_fail_the_turn_before_sampling(
    pre_compact: bool,
    namespace_collision: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        config.tool_registry.error_on_tool_collisions = true;
        if pre_compact {
            config.model_auto_compact_token_limit = Some(0);
        }
    });
    let test = builder.build_with_auto_env(&server).await?;
    let dynamic_tools = if namespace_collision {
        [
            ("first", "First namespace description."),
            ("second", "Second namespace description."),
        ]
        .into_iter()
        .map(|(name, description)| {
            DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
                name: "shared".to_string(),
                description: description.to_string(),
                tools: vec![DynamicToolNamespaceTool::Function(
                    DynamicToolFunctionSpec {
                        name: name.to_string(),
                        description: format!("The {name} tool."),
                        input_schema: json!({
                            "type": "object",
                            "properties": {},
                            "additionalProperties": false,
                        }),
                        defer_loading: false,
                    },
                )],
            })
        })
        .collect()
    } else {
        vec![DynamicToolSpec::Function(DynamicToolFunctionSpec {
            name: "update_plan".to_string(),
            description: "Collides with the built-in planning tool.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
            defer_loading: false,
        })]
    };
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            dynamic_tools,
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?
        .thread;

    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "use the planning tool".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;

    let EventMsg::Error(error) =
        wait_for_event(&thread, |event| matches!(event, EventMsg::Error(_))).await
    else {
        unreachable!("event predicate guarantees an error");
    };
    let expected_collision = if namespace_collision {
        "duplicate tool: shared"
    } else {
        "duplicate tool: functions.update_plan"
    };
    assert_eq!(error.message, expected_collision);

    let EventMsg::TurnComplete(completed) =
        wait_for_event(&thread, |event| matches!(event, EventMsg::TurnComplete(_))).await
    else {
        unreachable!("event predicate guarantees turn completion");
    };
    assert_eq!(completed.error, Some(error));
    assert!(
        server
            .received_requests()
            .await
            .context("mock server should expose received requests")?
            .iter()
            .all(|request| request.url.path() != "/v1/responses"),
        "a colliding turn should fail before making a model request"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn strict_tool_collisions_do_not_duplicate_unrelated_compaction_errors() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let error = json!({
        "error": {
            "message": "compaction request is invalid",
            "code": "invalid_request",
        },
    });
    let compact_mock =
        mount_response_once(&server, ResponseTemplate::new(400).set_body_json(&error)).await;
    let mut builder = test_codex().with_config(|config| {
        config.tool_registry.error_on_tool_collisions = true;
        config.model_auto_compact_token_limit = Some(0);
    });
    let test = builder.build_with_auto_env(&server).await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "trigger compaction".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;

    let mut errors = Vec::new();
    wait_for_event(&test.codex, |event| match event {
        EventMsg::Error(error) => {
            errors.push(error.message.clone());
            false
        }
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;

    assert_eq!(
        errors,
        vec![format!("Error running remote compact task: {error}")]
    );
    assert_eq!(compact_mock.requests().len(), 1);

    Ok(())
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
    for environment_tool in ["exec_command", "write_stdin", "apply_patch", "view_image"] {
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
    assert!(
        item.pointer("/internal_chat_message_metadata_passthrough/executed_tool_calls")
            .is_none(),
        "attempted-tool metadata must be disabled by default",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn namespaced_custom_tool_call_preserves_namespace_through_dispatch_and_replay() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex();
    builder = builder.with_config(|config| {
        let _ = config.features.enable(Feature::ExecutedToolCallMetadata);
    });
    let test = builder.build(&server).await?;

    let call_id = "custom-namespaced";
    let namespace = "test_namespace::";
    let tool_name = "unsupported_tool";
    let input = "\"payload\"";

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call_with_namespace(call_id, namespace, tool_name, input),
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
        "invoke namespaced custom tool",
        AskForApproval::Never,
        PermissionProfile::Disabled,
    )
    .await?;

    let request = mock.single_request();
    let custom_tool_calls = request.inputs_of_type("custom_tool_call");
    let turn_id = custom_tool_calls
        .first()
        .and_then(|item| item.pointer("/internal_chat_message_metadata_passthrough/turn_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .expect("custom tool call should include turn metadata");
    let custom_tool_output = request.custom_tool_call_output(call_id);
    let output_create_time = custom_tool_output
        .pointer("/internal_chat_message_metadata_passthrough/create_time")
        .and_then(Value::as_f64)
        .expect("custom tool output should include a creation timestamp");
    assert_eq!(
        (
            strip_response_item_ids_from_json(Value::Array(custom_tool_calls)),
            strip_response_item_ids_from_json(custom_tool_output),
        ),
        (
            Value::Array(vec![json!({
                "type": "custom_tool_call",
                "call_id": call_id,
                "namespace": namespace,
                "name": tool_name,
                "input": input,
                "internal_chat_message_metadata_passthrough": {
                    "turn_id": turn_id,
                },
            })]),
            json!({
                "type": "custom_tool_call_output",
                "call_id": call_id,
                "output": format!("unsupported custom tool call: {namespace}{tool_name}"),
                "internal_chat_message_metadata_passthrough": {
                    "turn_id": turn_id,
                    "create_time": output_create_time,
                    "executed_tool_calls": [{
                        "name": format!("{namespace}__{tool_name}"),
                        "arguments": input,
                    }],
                },
            }),
        )
    );
    let escaped_call_id = "custom-namespaced-escaped";
    let escaped_input = "\\".repeat(4_096);
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            ev_custom_tool_call_with_namespace(
                escaped_call_id,
                namespace,
                tool_name,
                &escaped_input,
            ),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let escaped_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "done"),
            ev_completed("resp-4"),
        ]),
    )
    .await;
    test.submit_turn_with_approval_and_permission_profile(
        "invoke namespaced custom tool with escaped arguments",
        AskForApproval::Never,
        PermissionProfile::Disabled,
    )
    .await?;
    let escaped_request = escaped_mock.single_request();
    assert_eq!(
        escaped_request.custom_tool_call_output(call_id)["internal_chat_message_metadata_passthrough"]
            ["executed_tool_calls"],
        json!([{
            "name": format!("{namespace}__{tool_name}"),
            "arguments": input,
        }]),
    );
    let expected_escaped_calls = json!([{
        "name": format!("{namespace}__{tool_name}"),
        "arguments": {
            "_codex_executed_tool_call_truncated": {
                "original_bytes": serde_json::to_vec(&escaped_input)?.len(),
                "max_bytes": 8 * 1024,
            },
        },
    }]);
    assert_eq!(
        escaped_request.custom_tool_call_output(escaped_call_id)["internal_chat_message_metadata_passthrough"]
            ["executed_tool_calls"],
        expected_escaped_calls,
    );

    let direct_exec_call_id = "custom-direct-exec";
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-5"),
            ev_custom_tool_call(
                direct_exec_call_id,
                codex_code_mode::PUBLIC_TOOL_NAME,
                input,
            ),
            ev_completed("resp-5"),
        ]),
    )
    .await;
    let direct_exec_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-3", "done"),
            ev_completed("resp-6"),
        ]),
    )
    .await;

    test.submit_turn_with_approval_and_permission_profile(
        "invoke direct custom exec outside code mode",
        AskForApproval::Never,
        PermissionProfile::Disabled,
    )
    .await?;

    let direct_exec_request = direct_exec_mock.single_request();
    assert_eq!(
        direct_exec_request.custom_tool_call_output(call_id)["internal_chat_message_metadata_passthrough"]
            ["executed_tool_calls"],
        json!([{
            "name": format!("{namespace}__{tool_name}"),
            "arguments": input,
        }]),
    );
    assert_eq!(
        direct_exec_request.custom_tool_call_output(escaped_call_id)["internal_chat_message_metadata_passthrough"]
            ["executed_tool_calls"],
        expected_escaped_calls,
    );
    let direct_exec_output = direct_exec_request.custom_tool_call_output(direct_exec_call_id);
    assert_eq!(
        direct_exec_output["output"],
        json!("unsupported custom tool call: exec"),
    );
    assert_eq!(
        direct_exec_output["internal_chat_message_metadata_passthrough"]["executed_tool_calls"],
        json!([{
            "name": codex_code_mode::PUBLIC_TOOL_NAME,
            "arguments": input,
        }]),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_command_escalated_permissions_rejected_then_ok() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_model("test-gpt-5-codex")
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        });
    let test = builder.build(&server).await?;
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            approval_policy: Some(AskForApproval::Never),
            permission_profile: Some(PermissionProfile::Disabled),
            ..Default::default()
        },
    )
    .await?;

    let command = "echo shell ok";
    let call_id_blocked = "exec-command-blocked";
    let call_id_success = "exec-command-success";

    let first_args = json!({
        "cmd": command,
        "login": false,
        "yield_time_ms": 1_000,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
    });
    let second_args = json!({
        "cmd": command,
        "login": false,
        "yield_time_ms": 10_000,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                call_id_blocked,
                "exec_command",
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
                "exec_command",
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

    test.submit_text_turn("run the exec_command script").await?;

    let policy = AskForApproval::Never;
    let expected_message = format!(
        "approval policy is {policy:?}; reject command — you cannot ask for escalated permissions if the approval policy is {policy:?}"
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
        r"(?s)^(?:Chunk ID: [^\n]+\n)?Wall time: [0-9]+(?:\.[0-9]+)? seconds\nProcess exited with code 0\n(?:Original token count: \d+\n)?Output:\nshell ok\n?$",
        &success_output,
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sandbox_denied_exec_command_returns_original_output() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.4");
    let fixture = builder.build(&server).await?;

    let call_id = "sandbox-denied-exec-command";
    let target_path = fixture.workspace_path("sandbox-denied.txt");
    let sentinel = "sandbox-denied sentinel output";
    let command = format!(
        "printf {sentinel:?} >&2; printf {content:?} > {path:?}",
        sentinel = format!("{sentinel}\n"),
        content = "sandbox denied",
        path = &target_path
    );
    let args = json!({
        "cmd": command,
        "login": false,
        "yield_time_ms": 5_000,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
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
    let exit_code = output_text
        .lines()
        .find_map(|line| line.strip_prefix("Process exited with code "))
        .context("exit code line present")?
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
async fn exec_command_enforces_glob_deny_read_policy() -> Result<()> {
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
                    missing_path_behavior: None,
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

    let call_id = "exec-command-glob-deny-read";
    let command = format!(
        "rc=0; cat {denied_path:?} || rc=$?; cat {allowed_path:?}; exit \"$rc\"",
        denied_path = denied_path.to_string_lossy(),
        allowed_path = allowed_path.to_string_lossy(),
    );
    let args = json!({
        "cmd": command,
        "login": false,
        "yield_time_ms": 10_000,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
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
    let exit_code = output_text
        .lines()
        .find_map(|line| line.strip_prefix("Process exited with code "))
        .context("exit code line present")?
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

#[derive(Clone, Copy, Debug)]
enum CommandToolAvailability {
    Default,
    ManagedUnifiedExecDisabled,
    ShellToolDisabled,
    ModelDisabled,
}

async fn collect_tools(availability: CommandToolAvailability) -> Result<Vec<String>> {
    let server = start_mock_server().await;

    let responses = vec![sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-1"),
    ])];
    let mock = mount_sse_sequence(&server, responses).await;

    let mut builder = match availability {
        CommandToolAvailability::Default => test_codex(),
        CommandToolAvailability::ManagedUnifiedExecDisabled => test_codex()
            .with_cloud_config_bundle(
                CloudConfigBundleFixture::loader_with_enterprise_requirement(
                    r#"
[features]
unified_exec = false
shell_tool = true
"#,
                ),
            ),
        CommandToolAvailability::ShellToolDisabled => test_codex().with_config(|config| {
            config
                .features
                .disable(Feature::ShellTool)
                .expect("test config should allow feature update");
        }),
        CommandToolAvailability::ModelDisabled => {
            test_codex().with_model_info_override("gpt-5.4", |model| {
                model.shell_type = ConfigShellToolType::Disabled;
            })
        }
    };
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

    for availability in [
        CommandToolAvailability::ShellToolDisabled,
        CommandToolAvailability::ModelDisabled,
    ] {
        let tools = collect_tools(availability).await?;
        for command_tool in ["exec_command", "write_stdin"] {
            assert!(
                !tools.iter().any(|name| name == command_tool),
                "tools list should not include {command_tool} for {availability:?}: {tools:?}"
            );
        }
    }

    for availability in [CommandToolAvailability::Default] {
        let tools = collect_tools(availability).await?;
        for command_tool in ["exec_command", "write_stdin"] {
            assert!(
                tools.iter().any(|name| name == command_tool),
                "tools list should include {command_tool} for {availability:?}: {tools:?}"
            );
        }
    }

    let tools = collect_tools(CommandToolAvailability::ManagedUnifiedExecDisabled).await?;
    assert!(
        tools.iter().any(|name| name == "exec_command"),
        "managed unified-exec disable should keep one-shot command execution: {tools:?}"
    );
    assert!(
        !tools.iter().any(|name| name == "write_stdin"),
        "managed unified-exec disable must not expose retained process authority: {tools:?}"
    );

    Ok(())
}
