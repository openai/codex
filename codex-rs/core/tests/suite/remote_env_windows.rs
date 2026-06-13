use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandSource;
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
use pretty_assertions::assert_eq;
use serde_json::json;

const WINDOWS_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_TEST_WINDOWS_EXEC_SERVER_URL";
const CALL_ID: &str = "wine-cmd-smoke";
const COMMAND: &str = "echo WINE_BAZEL_OK&&cd";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn windows_exec_server_records_host_shell_mismatch() -> Result<()> {
    let Some(exec_server_url) = std::env::var_os(WINDOWS_EXEC_SERVER_URL_ENV_VAR) else {
        eprintln!("Skipping test because {WINDOWS_EXEC_SERVER_URL_ENV_VAR} is not set.");
        return Ok(());
    };
    let exec_server_url = exec_server_url
        .into_string()
        .map_err(|_| anyhow::anyhow!("{WINDOWS_EXEC_SERVER_URL_ENV_VAR} must be valid UTF-8"))?;
    ensure!(
        !exec_server_url.trim().is_empty(),
        "{WINDOWS_EXEC_SERVER_URL_ENV_VAR} must not be empty"
    );

    let server = start_mock_server().await;
    let arguments = serde_json::to_string(&json!({
        "cmd": COMMAND,
        "login": false,
        "yield_time_ms": 5_000,
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
            cwd: test.config.cwd.clone(),
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
    loop {
        match wait_for_event(&test.codex, |_| true).await {
            EventMsg::ExecCommandBegin(event) if event.call_id == CALL_ID => begin = Some(event),
            EventMsg::ExecCommandEnd(event) if event.call_id == CALL_ID => end = Some(event),
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    let begin = begin.context("exec_command should emit a begin event")?;
    let expected_commands = [
        vec![
            "/bin/bash".to_string(),
            "-c".to_string(),
            COMMAND.to_string(),
        ],
        vec!["/bin/sh".to_string(), "-c".to_string(), COMMAND.to_string()],
    ];
    // This intentionally records the current cross-OS failure mode: the Linux
    // orchestrator resolves its own shell before sending the command to the
    // Windows exec-server, where that Unix shell cannot start.
    assert!(
        expected_commands.contains(&begin.command),
        "unexpected command: {:?}",
        begin.command,
    );
    assert_eq!(
        (begin.cwd.clone(), begin.source),
        (
            test.config.cwd.clone(),
            ExecCommandSource::UnifiedExecStartup,
        ),
    );

    let end = end.context("exec_command should emit an end event")?;
    assert_eq!(
        (
            end.command,
            end.cwd,
            end.source,
            end.stdout,
            end.stderr,
            end.aggregated_output,
            end.exit_code,
            end.status,
        ),
        (
            begin.command,
            test.config.cwd.clone(),
            ExecCommandSource::UnifiedExecStartup,
            String::new(),
            String::new(),
            String::new(),
            -1,
            ExecCommandStatus::Failed,
        ),
    );

    let request = response_mock
        .last_request()
        .context("model should receive the failed command output")?;
    let (output, success) = request
        .function_call_output_content_and_success(CALL_ID)
        .context("failed command output should be present")?;
    let output = output.context("failed command output should contain text")?;
    assert!(
        output.contains("Process exited with code -1"),
        "unexpected command output: {output:?}",
    );
    assert_ne!(success, Some(true));

    Ok(())
}
