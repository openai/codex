use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::protocol::TurnEnvironmentSelections;
use codex_protocol::user_input::UserInput;
use codex_utils_path_uri::PathUri;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event_with_timeout;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::ChildStdout;
use tokio::process::Command;
use tokio::time::Instant;
use tokio::time::timeout;

const FIRST_ENVIRONMENT_ID: &str = "first";
const SECOND_ENVIRONMENT_ID: &str = "second";
const FIRST_CALL_ID: &str = "write-from-first";
const SECOND_CALL_ID: &str = "write-from-second";
const EXEC_SERVER_START_TIMEOUT: Duration = Duration::from_secs(30);
const TURN_COMPLETE_TIMEOUT: Duration = Duration::from_secs(30);

struct ExecServerProcess {
    _codex_home: TempDir,
    child: Child,
    _stdout: BufReader<ChildStdout>,
    websocket_url: String,
}

impl ExecServerProcess {
    async fn start() -> Result<Self> {
        let codex_home = TempDir::new()?;
        let mut child = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
            .args(["exec-server", "--listen", "ws://127.0.0.1:0"])
            .env("CODEX_HOME", codex_home.path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let stdout = child
            .stdout
            .take()
            .context("exec-server stdout should be piped")?;
        let mut stdout = BufReader::new(stdout);
        let deadline = Instant::now() + EXEC_SERVER_START_TIMEOUT;
        let websocket_url = loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .context("timed out waiting for exec-server listen URL")?;
            let mut line = String::new();
            let bytes_read = timeout(remaining, stdout.read_line(&mut line))
                .await
                .context("timed out reading exec-server listen URL")??;
            if bytes_read == 0 {
                bail!("exec-server exited before printing its listen URL");
            }
            let line = line.trim();
            if line.starts_with("ws://") {
                break line.to_string();
            }
        };

        Ok(Self {
            _codex_home: codex_home,
            child,
            _stdout: stdout,
            websocket_url,
        })
    }
}

impl Drop for ExecServerProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_exec_servers_isolate_workspace_write_roots() -> Result<()> {
    let first_exec_server = ExecServerProcess::start().await?;
    let second_exec_server = ExecServerProcess::start().await?;
    let first_workspace = TempDir::new()?;
    let second_workspace = TempDir::new()?;

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config
            .features
            .enable(Feature::UnifiedExec)
            .expect("test config should allow unified exec");
    });
    let test = builder.build(&server).await?;
    let environment_manager = test.thread_manager.environment_manager();
    environment_manager.upsert_environment(
        FIRST_ENVIRONMENT_ID.to_string(),
        first_exec_server.websocket_url.clone(),
        /*connect_timeout*/ None,
    )?;
    environment_manager.upsert_environment(
        SECOND_ENVIRONMENT_ID.to_string(),
        second_exec_server.websocket_url.clone(),
        /*connect_timeout*/ None,
    )?;
    for environment_id in [FIRST_ENVIRONMENT_ID, SECOND_ENVIRONMENT_ID] {
        let environment = environment_manager
            .get_environment(environment_id)
            .with_context(|| format!("missing environment {environment_id}"))?;
        timeout(EXEC_SERVER_START_TIMEOUT, environment.wait_until_ready())
            .await
            .with_context(|| format!("timed out starting environment {environment_id}"))??;
    }

    let first_cross_write = second_workspace.path().join("cross-from-first.txt");
    let second_cross_write = first_workspace.path().join("cross-from-second.txt");
    let first_script = format!(
        "printf first > own-first.txt; if printf cross > '{}' 2>/dev/null; then printf cross-write-unexpected; else printf cross-write-denied; fi",
        first_cross_write.display()
    );
    let second_script = format!(
        "printf second > own-second.txt; if printf cross > '{}' 2>/dev/null; then printf cross-write-unexpected; else printf cross-write-denied; fi",
        second_cross_write.display()
    );
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    FIRST_CALL_ID,
                    "exec_command",
                    &json!({
                        "cmd": first_script,
                        "login": false,
                        "yield_time_ms": 30_000,
                        "environment_id": FIRST_ENVIRONMENT_ID,
                    })
                    .to_string(),
                ),
                ev_function_call(
                    SECOND_CALL_ID,
                    "exec_command",
                    &json!({
                        "cmd": second_script,
                        "login": false,
                        "yield_time_ms": 30_000,
                        "environment_id": SECOND_ENVIRONMENT_ID,
                    })
                    .to_string(),
                ),
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

    let first_workspace_uri = PathUri::from_host_native_path(first_workspace.path())?;
    let second_workspace_uri = PathUri::from_host_native_path(second_workspace.path())?;
    let environments = vec![
        TurnEnvironmentSelection {
            environment_id: FIRST_ENVIRONMENT_ID.to_string(),
            cwd: first_workspace_uri.clone(),
            workspace_roots: vec![first_workspace_uri],
        },
        TurnEnvironmentSelection {
            environment_id: SECOND_ENVIRONMENT_ID.to_string(),
            cwd: second_workspace_uri.clone(),
            workspace_roots: vec![second_workspace_uri],
        },
    ];
    let permission_profile = PermissionProfile::workspace_write_with(
        &[],
        NetworkSandboxPolicy::Restricted,
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
    );
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(permission_profile, test.config.cwd.as_path());
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "write one file in each environment".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentSelections::new(
                    test.config.cwd.clone(),
                    environments,
                )),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                ..Default::default()
            },
        })
        .await?;
    wait_for_event_with_timeout(
        &test.codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        TURN_COMPLETE_TIMEOUT,
    )
    .await;

    let first_output = response_mock
        .function_call_output_text(FIRST_CALL_ID)
        .context("missing first exec output")?;
    let second_output = response_mock
        .function_call_output_text(SECOND_CALL_ID)
        .context("missing second exec output")?;
    assert!(
        first_output.contains("cross-write-denied"),
        "{first_output}"
    );
    assert!(
        second_output.contains("cross-write-denied"),
        "{second_output}"
    );
    assert!(!first_output.contains("cross-write-unexpected"));
    assert!(!second_output.contains("cross-write-unexpected"));
    assert_eq!(
        std::fs::read_to_string(first_workspace.path().join("own-first.txt"))?,
        "first"
    );
    assert_eq!(
        std::fs::read_to_string(second_workspace.path().join("own-second.txt"))?,
        "second"
    );
    assert!(!first_cross_write.exists());
    assert!(!second_cross_write.exists());

    Ok(())
}
