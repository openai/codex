//! Bazel-only integration coverage for a Windows exec-server running under Wine.

use anyhow::Context;
use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_protocol::AskForApproval as AppAskForApproval;
use codex_app_server_protocol::ActivePermissionProfile;
use codex_app_server_protocol::CommandAction;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnEnvironmentParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_exec_server::CODEX_EXEC_SERVER_URL_ENV_VAR;
use codex_exec_server::ExecServerClient;
use codex_exec_server::FsWriteFileParams;
use codex_exec_server::RemoteExecServerConnectArgs;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandStatus;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::protocol::TurnEnvironmentSelections;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use codex_utils_path_uri::ApiPathString;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeMap;
use tempfile::TempDir;
use tokio::time::timeout;
use wine_exec_server_test_support::WineExecServer;

const APP_SERVER_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn windows_exec_server_runs_with_native_shell_and_cwd() -> Result<()> {
    const CALL_ID: &str = "wine-cmd-smoke";
    const COMMAND: &str = r#"if ((Get-Location).Path -ne 'C:\windows') { exit 1 }"#;

    WineExecServer
        .scope(|exec_server_url| async move {
            let server = start_mock_server().await;
            let arguments = serde_json::to_string(&json!({
                "cmd": COMMAND,
                "login": false,
                "yield_time_ms": 10_000,
            }))?;
            let response_mock = mount_sse_sequence(
                &server,
                vec![
                    sse(vec![
                        ev_response_created("resp-1"),
                        ev_function_call(CALL_ID, "exec_command", &arguments),
                        ev_completed("resp-1"),
                    ]),
                    sse(vec![
                        ev_response_created("resp-2"),
                        ev_assistant_message("msg-1", "done"),
                        ev_completed("resp-2"),
                    ]),
                ],
            )
            .await;

            let mut builder = test_codex()
                .with_model("gpt-5.2")
                .with_exec_server_url(exec_server_url)
                .with_config(|config| {
                    config.use_experimental_unified_exec_tool = true;
                    config
                        .features
                        .enable(Feature::UnifiedExec)
                        .expect("test config should allow feature update");
                });
            let test = builder.build(&server).await?;
            let (sandbox_policy, permission_profile) =
                turn_permission_fields(PermissionProfile::Disabled, test.config.cwd.as_path());
            let environments = TurnEnvironmentSelections::new(
                test.config.cwd.clone(),
                vec![TurnEnvironmentSelection {
                    environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                    cwd: PathUri::parse("file:///C:/windows")?,
                }],
            );

            test.codex
                .submit(Op::UserInput {
                    items: vec![UserInput::Text {
                        text: "run the Windows smoke command".to_string(),
                        text_elements: Vec::new(),
                    }],
                    final_output_json_schema: None,
                    responsesapi_client_metadata: None,
                    additional_context: Default::default(),
                    thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                        environments: Some(environments),
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

            let mut begin = None;
            let mut end = None;
            let mut turn_complete = false;
            loop {
                match wait_for_event(&test.codex, |_| true).await {
                    EventMsg::ExecCommandBegin(event) if event.call_id == CALL_ID => {
                        begin = Some(event)
                    }
                    EventMsg::ExecCommandEnd(event) if event.call_id == CALL_ID => {
                        end = Some(event)
                    }
                    EventMsg::TurnComplete(_) => turn_complete = true,
                    _ => {}
                }
                if turn_complete && end.is_some() {
                    break;
                }
            }

            let begin = begin.context("exec_command should emit a begin event")?;
            assert!(
                begin.command.first().is_some_and(|command| command
                    .to_ascii_lowercase()
                    .ends_with("pwsh.exe")),
                "unexpected command: {:?}",
                begin.command
            );
            assert_eq!(
                &begin.command[1..],
                ["-NoProfile", "-Command", COMMAND]
            );

            let end = end.context("exec_command should emit an end event")?;
            assert_eq!((end.exit_code, end.status), (0, ExecCommandStatus::Completed));

            let request = response_mock
                .last_request()
                .context("model should receive the command output")?;
            let (_output, success) = request
                .function_call_output_content_and_success(CALL_ID)
                .context("command output should be present")?;
            assert_ne!(success, Some(false));

            Ok(())
        })
        .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn app_server_starts_thread_with_windows_environment_native_cwd() -> Result<()> {
    const AGENTS_INSTRUCTIONS: &str = "remote Windows workspace instructions";
    const CALL_ID: &str = "wine-cmd-smoke";
    const COMMAND: &str = "Get-Content AGENTS.md -ErrorAction Stop";
    const NATIVE_CWD: &str = r"C:\windows";

    WineExecServer
        .scope(|exec_server_url| async move {
            let exec_server_client = ExecServerClient::connect_websocket(
                RemoteExecServerConnectArgs {
                    websocket_url: exec_server_url.clone(),
                    client_name: "remote-env-windows-test".to_string(),
                    connect_timeout: APP_SERVER_READ_TIMEOUT,
                    initialize_timeout: APP_SERVER_READ_TIMEOUT,
                    resume_session_id: None,
                },
            )
            .await?;
            exec_server_client
                .fs_write_file(FsWriteFileParams {
                    path: PathUri::parse("file:///C:/windows/AGENTS.md")?,
                    data_base64: STANDARD.encode(AGENTS_INSTRUCTIONS),
                    sandbox: None,
                })
                .await?;

            let codex_home = TempDir::new()?;
            let responses = vec![
                sse(vec![
                    ev_response_created("resp-1"),
                    ev_function_call(
                        CALL_ID,
                        "exec_command",
                        &serde_json::to_string(&json!({
                            "cmd": COMMAND,
                            "login": false,
                            "yield_time_ms": 5_000,
                        }))?,
                    ),
                    ev_completed("resp-1"),
                ]),
                sse(vec![
                    ev_response_created("resp-2"),
                    ev_assistant_message("msg-1", "done"),
                    ev_completed("resp-2"),
                ]),
            ];
            let server = create_mock_responses_server_sequence(responses).await;
            write_mock_responses_config_toml(
                codex_home.path(),
                &server.uri(),
                &BTreeMap::from([(Feature::UnifiedExec, true)]),
                100_000,
                /*requires_openai_auth*/ None,
                "mock",
                "compact",
            )?;
            let config_path = codex_home.path().join("config.toml");
            let config = std::fs::read_to_string(&config_path)?;
            // Exercise the implicit built-in profile instead of the shared fixture's explicit
            // legacy `sandbox_mode`, which intentionally has no active profile identity.
            std::fs::write(
                config_path,
                config.replace("sandbox_mode = \"read-only\"\n", ""),
            )?;
            let mut app_server = TestAppServer::new_with_env(
                codex_home.path(),
                &[(
                    CODEX_EXEC_SERVER_URL_ENV_VAR,
                    Some(exec_server_url.as_str()),
                )],
            )
            .await?;
            timeout(APP_SERVER_READ_TIMEOUT, app_server.initialize()).await??;

            let request_id = app_server
                .send_thread_start_request(ThreadStartParams {
                    environments: Some(vec![TurnEnvironmentParams {
                        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                        cwd: serde_json::from_value::<ApiPathString>(json!(NATIVE_CWD))?,
                    }]),
                    ..Default::default()
                })
                .await?;
            let response = timeout(
                APP_SERVER_READ_TIMEOUT,
                app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
            )
            .await??;
            let response: ThreadStartResponse = to_response(response)?;
            assert!(!response.thread.id.is_empty());
            assert_eq!(response.cwd.as_str(), NATIVE_CWD);
            assert_eq!(response.runtime_workspace_roots, vec![response.cwd.clone()]);
            // TODO(anp): Discover and report instruction sources from the remote filesystem.
            assert_eq!(response.instruction_sources, Vec::new());
            assert_eq!(
                response.active_permission_profile,
                Some(ActivePermissionProfile::read_only())
            );

            let turn_request_id = app_server
                .send_turn_start_request(TurnStartParams {
                    thread_id: response.thread.id,
                    client_user_message_id: None,
                    input: vec![V2UserInput::Text {
                        text: "run the Windows smoke command".to_string(),
                        text_elements: Vec::new(),
                    }],
                    approval_policy: Some(AppAskForApproval::Never),
                    sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
                    ..Default::default()
                })
                .await?;
            let turn_response = timeout(
                APP_SERVER_READ_TIMEOUT,
                app_server.read_stream_until_response_message(RequestId::Integer(turn_request_id)),
            )
            .await??;
            let _: TurnStartResponse = to_response(turn_response)?;

            let completed = timeout(APP_SERVER_READ_TIMEOUT, async {
                loop {
                    let notification = app_server
                        .read_stream_until_notification_message("item/completed")
                        .await?;
                    let completed: ItemCompletedNotification = serde_json::from_value(
                        notification.params.context("item/completed params")?,
                    )?;
                    if matches!(completed.item, ThreadItem::CommandExecution { .. }) {
                        return Ok::<ThreadItem, anyhow::Error>(completed.item);
                    }
                }
            })
            .await??;
            let ThreadItem::CommandExecution {
                command_actions,
                cwd,
                id,
                status,
                exit_code,
                ..
            } = completed
            else {
                unreachable!("loop returns only command execution items");
            };
            assert_eq!(id, CALL_ID);
            assert_eq!(cwd.as_str(), r"C:\windows");
            // TODO(anp): Parse command actions using the selected environment's path convention so
            // their paths remain Windows-native instead of degrading the action to Unknown.
            assert_eq!(command_actions.len(), 1);
            assert!(matches!(command_actions[0], CommandAction::Unknown { .. }));
            assert_eq!((status, exit_code), (CommandExecutionStatus::Completed, Some(0)));
            timeout(
                APP_SERVER_READ_TIMEOUT,
                app_server.read_stream_until_notification_message("turn/completed"),
            )
            .await??;

            let requests = server
                .received_requests()
                .await
                .context("failed to fetch received requests")?;
            let initial_request = requests.first().context("missing initial model request")?;
            let model_request_includes_remote_instructions = initial_request
                .body_json::<serde_json::Value>()?
                .to_string()
                .contains(AGENTS_INSTRUCTIONS);
            // TODO(anp): Load remote workspace instructions into the model context.
            assert!(!model_request_includes_remote_instructions);

            let first_request = requests
                .iter()
                .find(|request| request.url.path().ends_with("/responses"))
                .context("turn should send a Responses request")?;
            let body = first_request.body_json::<Value>()?;
            let environment_context = body["input"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|item| item.get("role").and_then(Value::as_str) == Some("user"))
                .filter_map(|item| item.get("content").and_then(Value::as_array))
                .flatten()
                .filter_map(|content| content.get("text").and_then(Value::as_str))
                .find(|text| text.starts_with("<environment_context>"))
                .context("environment context should be model visible")?;
            // The model should see the remote environment's shell, not the Linux app-server's
            // host shell.
            assert_eq!(
                environment_context
                    .lines()
                    .find(|line| line.trim_start().starts_with("<shell>"))
                    .map(str::trim),
                Some("<shell>powershell</shell>"),
            );
            // The model should see cwd using the remote environment's native path convention, not
            // the Linux app-server's host path convention.
            assert_eq!(
                environment_context
                    .lines()
                    .find(|line| line.trim_start().starts_with("<cwd>"))
                    .map(str::trim),
                Some(r"<cwd>C:\windows</cwd>"),
            );
            assert!(environment_context.contains(
                r"<workspace_roots><root>C:\windows</root></workspace_roots>"
            ));

            Ok(())
        })
        .await
}
