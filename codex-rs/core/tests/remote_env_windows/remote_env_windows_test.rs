//! Bazel-only integration coverage for a Windows exec-server running under Wine.

use anyhow::Context;
use anyhow::Result;
use app_test_support::PathBufExt;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnEnvironmentParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_exec_server::CODEX_EXEC_SERVER_URL_ENV_VAR;
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
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use tempfile::TempDir;
use tokio::time::timeout;
use wine_exec_server_test_support::WineExecServer;

const APP_SERVER_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn windows_exec_server_runs_with_native_shell_and_cwd() -> Result<()> {
    const CALL_ID: &str = "wine-cmd-smoke";
    const PATCH_CALL_ID: &str = "wine-apply-patch";
    const VERIFY_CALL_ID: &str = "wine-verify-patch";
    const PATCH_FILE: &str = "codex-apply-patch-smoke.txt";
    const COMMAND: &str = r#"if ((Get-Location).Path -ne 'C:\windows') { exit 1 }"#;
    const VERIFY_COMMAND: &str = r#"$path = Join-Path (Get-Location) 'codex-apply-patch-smoke.txt'; if (-not (Test-Path $path)) { exit 1 }; if ([IO.File]::ReadAllText($path) -ne "patched through unified exec`n") { exit 2 }; Remove-Item $path; Write-Output 'PATCH_VERIFIED'"#;

    WineExecServer
        .scope(|exec_server_url, _wine_prefix| async move {
            let server = start_mock_server().await;
            let arguments = serde_json::to_string(&json!({
                "cmd": COMMAND,
                "login": false,
                // An absolute foreign workdir should replace the selected environment cwd and
                // reach exec-server without conversion to the host path convention.
                "workdir": r"C:\windows",
                "yield_time_ms": 10_000,
            }))?;
            let patch = format!(
                "*** Begin Patch\n*** Add File: {PATCH_FILE}\n+patched through unified exec\n*** End Patch"
            );
            let patch_arguments = serde_json::to_string(&json!({
                "cmd": format!("apply_patch <<'EOF'\n{patch}\nEOF\n"),
                "login": false,
                // Resolve this relative workdir using the selected Windows environment cwd.
                "workdir": r"apply-patch-smoke\nested",
                "yield_time_ms": 10_000,
            }))?;
            let verify_arguments = serde_json::to_string(&json!({
                "cmd": VERIFY_COMMAND,
                "login": false,
                "workdir": r"apply-patch-smoke\nested",
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
                        ev_function_call(PATCH_CALL_ID, "exec_command", &patch_arguments),
                        ev_completed("resp-2"),
                    ]),
                    sse(vec![
                        ev_response_created("resp-3"),
                        ev_function_call(VERIFY_CALL_ID, "exec_command", &verify_arguments),
                        ev_completed("resp-3"),
                    ]),
                    sse(vec![
                        ev_response_created("resp-4"),
                        ev_assistant_message("msg-1", "done"),
                        ev_completed("resp-4"),
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
                    cwd: PathUri::parse("file:///C:/codex-home")?,
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
            let mut patch_end = None;
            let mut turn_complete = false;
            loop {
                match wait_for_event(&test.codex, |_| true).await {
                    EventMsg::ExecCommandBegin(event) if event.call_id == CALL_ID => {
                        begin = Some(event)
                    }
                    EventMsg::ExecCommandEnd(event) if event.call_id == CALL_ID => {
                        end = Some(event)
                    }
                    EventMsg::PatchApplyEnd(event) if event.call_id == PATCH_CALL_ID => {
                        patch_end = Some(event)
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
            assert_eq!(&begin.command[1..], ["-NoProfile", "-Command", COMMAND]);

            let end = end.context("exec_command should emit an end event")?;
            let expected_cwd = PathUri::parse("file:///C:/windows")?;
            assert_eq!((&begin.cwd, &end.cwd), (&expected_cwd, &expected_cwd));
            assert_eq!((end.exit_code, end.status), (0, ExecCommandStatus::Completed));

            let patch_end = patch_end.context("intercepted apply_patch should emit an end event")?;
            assert!(
                patch_end.success,
                "intercepted apply_patch failed: stdout={:?} stderr={:?}",
                patch_end.stdout, patch_end.stderr
            );
            assert!(
                patch_end
                    .changes
                    .contains_key(&std::path::PathBuf::from(format!(
                        r"C:\codex-home\apply-patch-smoke\nested\{PATCH_FILE}"
                    ))),
                "apply_patch should retain the Windows cwd: {:?}",
                patch_end.changes
            );
            let request = response_mock
                .last_request()
                .context("model should receive the command output")?;
            let (verify_output, verify_success) = request
                .function_call_output_content_and_success(VERIFY_CALL_ID)
                .context("verification output should be present")?;
            anyhow::ensure!(
                verify_success != Some(false),
                "verification command failed: {verify_output:?}"
            );
            anyhow::ensure!(
                verify_output
                    .as_deref()
                    .is_some_and(|output| output.contains("PATCH_VERIFIED")),
                "verification command did not confirm the patched file: {verify_output:?}"
            );

            let (_output, success) = request
                .function_call_output_content_and_success(CALL_ID)
                .context("command output should be present")?;
            assert_ne!(success, Some(false));
            let (patch_output, patch_success) = request
                .function_call_output_content_and_success(PATCH_CALL_ID)
                .context("apply_patch output should be present")?;
            let patch_output = patch_output.context("apply_patch output should contain text")?;
            assert!(patch_output.contains(PATCH_FILE));
            assert_ne!(patch_success, Some(false));
            Ok(())
        })
        .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn app_server_starts_thread_with_windows_environment_native_cwd() -> Result<()> {
    const AGENTS_INSTRUCTIONS: &str = "remote Windows workspace instructions";
    const NATIVE_CWD: &str = r"C:\windows";

    WineExecServer
        .scope(|exec_server_url, wine_prefix| async move {
            let agents_path = PathUri::parse("file:///C:/windows/AGENTS.md")?;
            fs::write(
                wine_prefix.join("drive_c").join("windows").join("AGENTS.md"),
                AGENTS_INSTRUCTIONS,
            )?;

            let codex_home = TempDir::new()?;
            let server = create_mock_responses_server_repeating_assistant("done").await;
            write_mock_responses_config_toml(
                codex_home.path(),
                &server.uri(),
                &BTreeMap::new(),
                100_000,
                /*requires_openai_auth*/ None,
                "mock",
                "compact",
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
                        cwd: serde_json::from_value::<LegacyAppPathString>(json!(NATIVE_CWD))?,
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
            let host_cwd = codex_home.path().to_path_buf().abs();
            // TODO(anp): Return the selected environment's native cwd from thread/start.
            assert_eq!(response.cwd, host_cwd);
            // TODO(anp): Derive runtime workspace roots from the selected remote environment.
            assert_eq!(response.runtime_workspace_roots, vec![host_cwd]);
            assert_eq!(
                response.instruction_sources,
                vec![LegacyAppPathString::from_path_uri(
                    &agents_path,
                    PathConvention::Windows,
                )?]
            );
            // TODO(anp): Report the implicit built-in permission profile instead of None.
            assert_eq!(response.active_permission_profile, None);

            let turn_request_id = app_server
                .send_turn_start_request(TurnStartParams {
                    thread_id: response.thread.id,
                    client_user_message_id: None,
                    input: vec![V2UserInput::Text {
                        text: "say done".to_string(),
                        text_elements: Vec::new(),
                    }],
                    ..Default::default()
                })
                .await?;
            let turn_response = timeout(
                APP_SERVER_READ_TIMEOUT,
                app_server.read_stream_until_response_message(RequestId::Integer(turn_request_id)),
            )
            .await??;
            let _: TurnStartResponse = to_response(turn_response)?;
            timeout(
                APP_SERVER_READ_TIMEOUT,
                app_server.read_stream_until_notification_message("turn/completed"),
            )
            .await??;

            let requests = server
                .received_requests()
                .await
                .context("failed to fetch received requests")?;
            let first_request = requests
                .iter()
                .find(|request| request.url.path().ends_with("/responses"))
                .context("turn should send a Responses request")?;
            let body = first_request.body_json::<Value>()?;
            let remote_instructions = body["input"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|item| item.get("role").and_then(Value::as_str) == Some("user"))
                .filter_map(|item| item.get("content").and_then(Value::as_array))
                .flatten()
                .filter_map(|content| content.get("text").and_then(Value::as_str))
                .find(|text| text.contains(AGENTS_INSTRUCTIONS))
                .context("remote workspace instructions should be model visible")?;
            assert!(remote_instructions.contains(r"# AGENTS.md instructions for C:\windows"));
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
            let host_workspace_roots = format!(
                "<workspace_roots><root>{}</root></workspace_roots>",
                codex_home.path().display()
            );
            // TODO(anp): Derive model-visible workspace roots from the selected remote environment
            // and render them using its native path convention.
            assert!(environment_context.contains(&host_workspace_roots));

            Ok(())
        })
        .await
}
