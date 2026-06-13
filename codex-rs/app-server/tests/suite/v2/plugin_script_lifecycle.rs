#![cfg(unix)]

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_shell_command_sse_response;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml_with_chatgpt_base_url;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxPolicy;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;
use tokio::time::timeout;

use super::analytics::captured_analytics_events;
use super::analytics::mount_analytics_capture;
use super::analytics::wait_for_analytics_event;
use super::analytics::wait_for_analytics_events;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const CURATED_PLUGIN_SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const CURATED_PLUGIN_VERSION: &str = "01234567";
const PLUGIN_NAME: &str = "lifecycle";
const PLUGIN_CONFIG_NAME: &str = "lifecycle@openai-curated";

#[derive(Clone, Copy)]
enum LifecycleBackend {
    Classic,
    ZshFork,
}

struct LifecycleRun {
    backend: LifecycleBackend,
    sandbox_policy: SandboxPolicy,
    approval_policy: AskForApproval,
    accept_approval: bool,
    expected_events: usize,
    executable: Option<&'static str>,
}

impl LifecycleRun {
    fn classic() -> Self {
        Self {
            backend: LifecycleBackend::Classic,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            approval_policy: AskForApproval::Never,
            accept_approval: false,
            expected_events: 2,
            executable: Some("sh"),
        }
    }
}

struct LifecycleFixture {
    events: Vec<Value>,
    thread_id: String,
    session_id: String,
    turn_id: String,
    plugin_root: std::path::PathBuf,
}

#[tokio::test]
async fn plugin_script_emits_started_and_completed_lifecycle_analytics() -> Result<()> {
    let fixture = run_lifecycle_fixture(
        "printf 'sensitive-script-output\\n'\n",
        &["secret-argument"],
        /*interrupt*/ false,
        LifecycleRun::classic(),
    )
    .await?;
    assert_eq!(fixture.events.len(), 2);

    let started = event_with_status(&fixture.events, "started")?;
    let completed = event_with_status(&fixture.events, "completed")?;
    let started_params = &started["event_params"];
    let completed_params = &completed["event_params"];

    assert_eq!(started_params["version"], 1);
    assert_eq!(started_params["thread_id"], fixture.thread_id);
    assert_eq!(started_params["session_id"], fixture.session_id);
    assert_eq!(started_params["turn_id"], fixture.turn_id);
    assert_eq!(started_params["plugin_id"], PLUGIN_CONFIG_NAME);
    assert_eq!(started_params["script_path"], "scripts/run.sh");
    assert!(started_params["timestamp"].as_str().is_some());
    assert!(started_params.get("duration_ms").is_none());
    assert!(started_params.get("exit_code").is_none());
    assert_eq!(started_params["skill_id"], Value::Null);
    assert_eq!(
        completed_params["execution_id"],
        started_params["execution_id"]
    );
    assert!(completed_params["duration_ms"].as_u64().is_some());
    assert_eq!(completed_params["exit_code"], 0);

    let serialized_events = serde_json::to_string(&fixture.events)?;
    assert!(!serialized_events.contains("secret-argument"));
    assert!(!serialized_events.contains("sensitive-script-output"));
    assert!(!serialized_events.contains(fixture.plugin_root.to_string_lossy().as_ref()));

    Ok(())
}

#[tokio::test]
async fn zsh_fork_plugin_script_emits_terminal_lifecycle_analytics() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let Some(zsh_path) = super::turn_start_zsh_fork::find_test_zsh_path()? else {
        return Ok(());
    };
    if !super::turn_start_zsh_fork::supports_exec_wrapper_intercept(&zsh_path) {
        return Ok(());
    }
    for (script, interrupt, status, exit_code) in [
        ("exit 0\n", false, "completed", Some(0)),
        ("exit 7\n", false, "failed", Some(7)),
        ("sleep 30\n", true, "cancelled", None),
    ] {
        let fixture = run_lifecycle_fixture(
            script,
            &[],
            interrupt,
            LifecycleRun {
                backend: LifecycleBackend::ZshFork,
                ..LifecycleRun::classic()
            },
        )
        .await?;
        assert_eq!(fixture.events.len(), 2);
        let started = event_with_status(&fixture.events, "started")?;
        let terminal = event_with_status(&fixture.events, status)?;
        assert_eq!(
            terminal["event_params"]["execution_id"],
            started["event_params"]["execution_id"]
        );
        assert!(terminal["event_params"]["duration_ms"].as_u64().is_some());
        if let Some(exit_code) = exit_code {
            assert_eq!(terminal["event_params"]["exit_code"], exit_code);
        }
    }
    Ok(())
}

#[tokio::test]
async fn sandbox_denial_terminates_each_plugin_script_attempt() -> Result<()> {
    let denial = run_lifecycle_fixture(
        "printf denied > denied.txt\n",
        &[],
        /*interrupt*/ false,
        LifecycleRun {
            sandbox_policy: SandboxPolicy::ReadOnly {
                network_access: false,
            },
            ..LifecycleRun::classic()
        },
    )
    .await?;
    assert_eq!(statuses(&denial.events), vec!["started", "failed"]);

    let retry = run_lifecycle_fixture(
        "printf denied > denied.txt\n",
        &[],
        /*interrupt*/ false,
        LifecycleRun {
            sandbox_policy: SandboxPolicy::ReadOnly {
                network_access: false,
            },
            approval_policy: AskForApproval::OnFailure,
            accept_approval: true,
            expected_events: 4,
            ..LifecycleRun::classic()
        },
    )
    .await?;
    assert_eq!(
        statuses(&retry.events),
        vec!["started", "failed", "started", "completed"]
    );
    assert_ne!(
        retry.events[0]["event_params"]["execution_id"],
        retry.events[2]["event_params"]["execution_id"]
    );
    Ok(())
}

#[tokio::test]
async fn pre_spawn_plugin_script_failure_emits_no_lifecycle_analytics() -> Result<()> {
    let fixture = run_lifecycle_fixture(
        "exit 0\n",
        &[],
        /*interrupt*/ false,
        LifecycleRun {
            expected_events: 0,
            executable: None,
            ..LifecycleRun::classic()
        },
    )
    .await?;
    assert!(fixture.events.is_empty());
    Ok(())
}

async fn run_lifecycle_fixture(
    script: &str,
    script_args: &[&str],
    interrupt: bool,
    run: LifecycleRun,
) -> Result<LifecycleFixture> {
    let temp = TempDir::new()?;
    let codex_home = temp.path().join("codex-home");
    let working_directory = temp.path().join("workdir");
    std::fs::create_dir_all(&working_directory)?;
    write_curated_provenance(&codex_home)?;
    let plugin_root = codex_home
        .join("plugins/cache/openai-curated")
        .join(PLUGIN_NAME)
        .join(CURATED_PLUGIN_VERSION);
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    std::fs::create_dir_all(plugin_root.join("scripts"))?;
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "name": PLUGIN_NAME,
            "interface": { "developerName": "OpenAI" },
        }))?,
    )?;
    let script_path = plugin_root.join("scripts/run.sh");
    std::fs::write(&script_path, script)?;
    #[cfg(unix)]
    assert_eq!(
        std::fs::metadata(&script_path)?.permissions().mode() & 0o111,
        0
    );
    let script_path = script_path.to_string_lossy().into_owned();
    let mut command = match run.executable {
        Some(executable) => vec![executable.to_string(), script_path],
        None => vec![script_path],
    };
    command.extend(script_args.iter().map(|arg| (*arg).to_string()));
    let mut responses = vec![create_shell_command_sse_response(
        command,
        Some(&working_directory),
        Some(60_000),
        "plugin-script-call",
    )?];
    if !interrupt {
        responses.push(create_final_assistant_message_sse_response("done")?);
    }
    let server = create_mock_responses_server_sequence(responses).await;
    write_mock_responses_config_toml_with_chatgpt_base_url(
        &codex_home,
        &server.uri(),
        &server.uri(),
    )?;
    let config_path = codex_home.join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        &config_path,
        format!(
            r#"{config}
[shell_environment_policy.set]
PATH = "/usr/bin:/bin"

[features]
plugins = true
plugin_script_lifecycle = true
{}

[plugins."{PLUGIN_CONFIG_NAME}"]
enabled = true
"#,
            match run.backend {
                LifecycleBackend::Classic => "",
                LifecycleBackend::ZshFork =>
                    "shell_zsh_fork = true\nshell_snapshot = false\nunified_exec = false",
            },
        ),
    )?;
    mount_analytics_capture(&server, &codex_home).await?;
    let isolated_home = codex_home.to_string_lossy();
    let mut mcp = match run.backend {
        LifecycleBackend::Classic => {
            TestAppServer::new_with_env(
                &codex_home,
                &[
                    ("HOME", Some(isolated_home.as_ref())),
                    ("USERPROFILE", Some(isolated_home.as_ref())),
                ],
            )
            .await?
        }
        LifecycleBackend::ZshFork => {
            let zsh_path = super::turn_start_zsh_fork::find_test_zsh_path()?
                .ok_or_else(|| anyhow::anyhow!("zsh-fork lifecycle fixture requires zsh"))?;
            super::turn_start_zsh_fork::create_zsh_test_mcp_process(
                &codex_home,
                &working_directory,
                &zsh_path,
            )
            .await?
        }
    };
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let thread_request = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_request)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_response)?;
    if run.executable.is_none() {
        // Keep the attributed direct plugin-script command absolute so resolver attribution
        // succeeds even though this missing cwd makes the actual shell process fail to spawn.
        std::fs::remove_dir(&working_directory)?;
    }
    let turn_request = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "run the plugin script".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(working_directory),
            sandbox_policy: Some(run.sandbox_policy),
            approval_policy: Some(run.approval_policy),
            ..Default::default()
        })
        .await?;
    let turn_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_request)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response::<TurnStartResponse>(turn_response)?;
    if run.accept_approval {
        let ServerRequest::CommandExecutionRequestApproval { request_id, .. } = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_request_message(),
        )
        .await??
        else {
            panic!("expected command approval request");
        };
        mcp.send_response(
            request_id,
            serde_json::to_value(CommandExecutionRequestApprovalResponse {
                decision: CommandExecutionApprovalDecision::Accept,
            })?,
        )
        .await?;
    }
    if interrupt {
        wait_for_analytics_events(
            &server,
            DEFAULT_READ_TIMEOUT,
            "codex_plugin_lifecycle_event",
            /*expected_count*/ 1,
        )
        .await?;
        let interrupt_request = mcp
            .send_turn_interrupt_request(TurnInterruptParams {
                thread_id: thread.id.clone(),
                turn_id: turn.id.clone(),
            })
            .await?;
        let interrupt_response: JSONRPCResponse = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(interrupt_request)),
        )
        .await??;
        let _: TurnInterruptResponse = to_response(interrupt_response)?;
    }
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    // The terminal turn event is reduced after the tool result that emits plugin lifecycle
    // facts, so observing this matching marker proves analytics drained those facts without
    // relying on a sleep that could miss late duplicate lifecycle events.
    let terminal_turn_event =
        wait_for_analytics_event(&server, DEFAULT_READ_TIMEOUT, "codex_turn_event").await?;
    assert_eq!(terminal_turn_event["event_params"]["thread_id"], thread.id);
    assert_eq!(terminal_turn_event["event_params"]["turn_id"], turn.id);
    let events = lifecycle_events(&server).await?;
    assert_eq!(events.len(), run.expected_events);
    Ok(LifecycleFixture {
        events,
        thread_id: thread.id,
        session_id: thread.session_id,
        turn_id: turn.id,
        plugin_root,
    })
}

fn write_curated_provenance(codex_home: &std::path::Path) -> Result<()> {
    let marketplace_path = codex_home.join(".tmp/plugins/.agents/plugins/marketplace.json");
    std::fs::create_dir_all(
        marketplace_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("marketplace path should have a parent"))?,
    )?;
    std::fs::write(
        marketplace_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "name": "openai-curated",
            "plugins": [{
                "name": PLUGIN_NAME,
                "source": {
                    "source": "local",
                    "path": "./plugins/lifecycle",
                },
            }],
        }))?,
    )?;
    std::fs::write(codex_home.join(".tmp/plugins.sha"), CURATED_PLUGIN_SHA)?;
    Ok(())
}

async fn lifecycle_events(server: &wiremock::MockServer) -> Result<Vec<Value>> {
    Ok(captured_analytics_events(server)
        .await?
        .into_iter()
        .filter(|event| event["event_type"] == "codex_plugin_lifecycle_event")
        .collect())
}

fn statuses(events: &[Value]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|event| event["event_params"]["status"].as_str())
        .collect()
}

fn event_with_status<'a>(events: &'a [Value], status: &str) -> Result<&'a Value> {
    events
        .iter()
        .find(|event| event["event_params"]["status"] == status)
        .ok_or_else(|| anyhow::anyhow!("missing plugin lifecycle status {status}"))
}
