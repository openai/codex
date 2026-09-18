//! Command lifecycle callbacks observe the command and filesystem selected for execution.

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use codex_config::Constrained;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_extension_api::CommandStartInput;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use codex_utils_path_uri::PathUri;
use core_test_support::TestTargetOs;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use core_test_support::test_target_os;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_with_timeout;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;
use tokio::sync::Notify;
use tokio::sync::oneshot;
use tokio::time::timeout;

#[derive(Debug, PartialEq)]
struct RecordedCommand {
    call_id: String,
    command: Vec<String>,
    cwd: PathUri,
    fixture: String,
}

#[derive(Default)]
struct CommandRecorder {
    commands: Mutex<Vec<RecordedCommand>>,
}

impl ToolLifecycleContributor for CommandRecorder {
    fn on_command_start<'a>(&'a self, input: CommandStartInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            let fixture_path = input
                .cwd
                .join("executor-fixture.txt")
                .expect("fixture path");
            let fixture = input
                .file_system
                .read_file_text(&fixture_path, Default::default(), /*sandbox*/ None)
                .await
                .expect("callback must use the selected executor's filesystem");
            let prepared_path = input.cwd.join("prepared.txt").expect("prepared path");
            input
                .file_system
                .write_file(
                    &prepared_path,
                    fixture.as_bytes().to_vec(),
                    Default::default(),
                    /*sandbox*/ None,
                )
                .await
                .expect("prepare the command's executor before it starts");
            self.commands
                .lock()
                .expect("command records")
                .push(RecordedCommand {
                    call_id: input.call_id.to_owned(),
                    command: input.command.to_vec(),
                    cwd: input.cwd.clone(),
                    fixture,
                });
        })
    }
}

#[derive(Clone, Copy)]
enum CommandCallMode {
    Direct,
    CodeMode,
}

impl CommandCallMode {
    fn call(self, call_id: &str, arguments: serde_json::Value) -> serde_json::Value {
        match self {
            Self::Direct => {
                responses::ev_function_call(call_id, "exec_command", &arguments.to_string())
            }
            Self::CodeMode => responses::ev_custom_tool_call(
                call_id,
                "exec",
                &format!("text(await tools.exec_command({arguments}));"),
            ),
        }
    }
}

fn configure_command_test_permissions(config: &mut Config) {
    // Match TestCodex::submit_turn: these fixtures exercise execution callbacks
    // without waiting for interactive approval on hosts lacking a sandbox backend.
    config.permissions.approval_policy = Constrained::allow_any(AskForApproval::Never);
    config
        .permissions
        .set_permission_profile(PermissionProfile::Disabled)
        .expect("set command test permissions");
}

async fn start_command_turn(test: &TestCodex) -> Result<()> {
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Run the command.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    Ok(())
}

#[test_case(CommandCallMode::Direct; "direct")]
#[test_case(CommandCallMode::CodeMode; "code_mode")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn command_start_receives_rewritten_command_and_executor_workdir(
    mode: CommandCallMode,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "command hooks require a host-native executor");

    let rewritten_command = match test_target_os() {
        TestTargetOs::Linux | TestTargetOs::MacOs => "cat prepared.txt",
        TestTargetOs::Windows => "Get-Content -Raw prepared.txt",
    };
    let hook_output = json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "updatedInput": { "command": rewritten_command },
        }
    });
    let recorder = Arc::new(CommandRecorder::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(recorder.clone());
    let builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(configure_command_test_permissions)
        .with_pre_build_hook(move |home| {
            super::write_pre_tool_hook(home, "^Bash$", &hook_output)
                .expect("write command-rewriting hook");
        })
        .with_config(trust_discovered_hooks)
        .with_config(move |config| {
            if matches!(mode, CommandCallMode::CodeMode) {
                let _ = config.features.enable(Feature::CodeMode);
            }
        });
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;
    let (test, server) = (harness.test(), harness.server());

    let cwd = test.workspace_path_uri("command-workdir")?;
    let fixture = "selected-executor-command-marker";
    harness
        .write_file("command-workdir/executor-fixture.txt", fixture)
        .await?;

    let call_id = "command-lifecycle-call";
    let arguments = json!({
        "cmd": "echo original-command-must-not-run",
        "workdir": "command-workdir",
        "yield_time_ms": 5_000,
    });
    let call = mode.call(call_id, arguments);
    responses::mount_sse_once(
        server,
        responses::sse(vec![call, responses::ev_completed("first-response")]),
    )
    .await;
    let follow_up = responses::mount_sse_once(
        server,
        responses::sse(vec![responses::ev_completed("second-response")]),
    )
    .await;
    start_command_turn(test).await?;

    let mut begin = None;
    wait_for_event_with_timeout(
        &test.codex,
        |event| {
            assert!(
                !matches!(event, EventMsg::ExecApprovalRequest(_)),
                "command fixture unexpectedly requested approval: {event:?}"
            );
            if let EventMsg::ExecCommandBegin(command) = event {
                begin = Some(command.clone());
            }
            matches!(event, EventMsg::TurnComplete(_))
        },
        Duration::from_secs(30),
    )
    .await;
    let request = follow_up.single_request();
    let output = match mode {
        CommandCallMode::Direct => request.function_call_output(call_id),
        CommandCallMode::CodeMode => request.custom_tool_call_output(call_id),
    };
    let begin = begin.unwrap_or_else(|| panic!("command did not start: {output}"));

    assert_eq!(begin.cwd, cwd);
    assert!(
        begin
            .command
            .last()
            .is_some_and(|argument| argument.contains(rewritten_command)),
        "the executor must receive the post-hook shell command"
    );
    assert_eq!(
        *recorder.commands.lock().expect("command records"),
        vec![RecordedCommand {
            call_id: begin.call_id,
            command: begin.command,
            cwd,
            fixture: fixture.to_owned(),
        }]
    );
    assert!(
        output["output"].to_string().contains(fixture),
        "execution must wait for the callback to prepare its selected filesystem"
    );
    Ok(())
}

struct BlockingCommandContributor {
    entered: Mutex<Option<oneshot::Sender<()>>>,
    completed: Mutex<Option<oneshot::Sender<()>>>,
    release: Notify,
}

impl ToolLifecycleContributor for BlockingCommandContributor {
    fn on_command_start<'a>(&'a self, _input: CommandStartInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            let Some(entered) = self.entered.lock().expect("entry sender").take() else {
                return;
            };
            // Closing this channel tells the test that cancellation dropped the callback.
            let _completion = self.completed.lock().expect("completion sender").take();
            entered.send(()).expect("test awaits command preparation");
            self.release.notified().await;
        })
    }
}

#[test_case(CommandCallMode::Direct; "direct")]
#[test_case(CommandCallMode::CodeMode; "code_mode")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupting_command_preparation_does_not_start_the_command(
    mode: CommandCallMode,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "basic PowerShell execution through Wine is not passing yet"
    );

    let (entered_tx, entered_rx) = oneshot::channel();
    let (completed_tx, completed_rx) = oneshot::channel();
    let contributor = Arc::new(BlockingCommandContributor {
        entered: Mutex::new(Some(entered_tx)),
        completed: Mutex::new(Some(completed_tx)),
        release: Notify::new(),
    });
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(contributor.clone());
    let builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(configure_command_test_permissions)
        .with_config(move |config| {
            if matches!(mode, CommandCallMode::CodeMode) {
                let _ = config.features.enable(Feature::CodeMode);
                let _ = config.features.enable(Feature::CodeModeInterrupt);
            }
        });
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;
    let (test, server) = (harness.test(), harness.server());
    let command = match test_target_os() {
        TestTargetOs::Linux | TestTargetOs::MacOs => "printf executed > must-not-run.txt",
        TestTargetOs::Windows => "Set-Content must-not-run.txt executed",
    };
    responses::mount_sse_once(
        server,
        responses::sse(vec![
            mode.call("interrupted-command", json!({ "cmd": command })),
            responses::ev_completed("first-response"),
        ]),
    )
    .await;
    start_command_turn(test).await?;
    timeout(Duration::from_secs(30), entered_rx).await??;
    test.codex.submit(Op::Interrupt).await?;
    wait_for_event(&test.codex, |event| {
        assert!(
            !matches!(event, EventMsg::ExecApprovalRequest(_)),
            "command fixture unexpectedly requested approval: {event:?}"
        );
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;
    timeout(Duration::from_secs(30), completed_rx)
        .await?
        .expect_err("cancellation must drop the callback's completion sender");
    contributor.release.notify_one();

    assert!(
        !harness.path_exists("must-not-run.txt").await?,
        "the command must not start after its preparation is cancelled"
    );

    let call_id = "command-after-interruption";
    responses::mount_sse_once(
        server,
        responses::sse(vec![
            mode.call(call_id, json!({ "cmd": "echo command-after-interruption" })),
            responses::ev_completed("second-response"),
        ]),
    )
    .await;
    let follow_up = responses::mount_sse_once(
        server,
        responses::sse(vec![responses::ev_completed("third-response")]),
    )
    .await;
    start_command_turn(test).await?;
    let mut process_id = None;
    let mut exit_code = None;
    wait_for_event_with_timeout(
        &test.codex,
        |event| {
            assert!(
                !matches!(event, EventMsg::ExecApprovalRequest(_)),
                "command fixture unexpectedly requested approval: {event:?}"
            );
            match event {
                EventMsg::ExecCommandBegin(begin) => {
                    process_id = begin.process_id.clone();
                }
                EventMsg::ExecCommandEnd(end) => {
                    exit_code = Some(end.exit_code);
                }
                _ => {}
            }
            matches!(event, EventMsg::TurnComplete(_))
        },
        Duration::from_secs(30),
    )
    .await;
    assert_eq!(process_id.as_deref(), Some("1000"));
    assert_eq!(exit_code, Some(0));
    let request = follow_up.single_request();
    let output = match mode {
        CommandCallMode::Direct => request.function_call_output(call_id),
        CommandCallMode::CodeMode => request.custom_tool_call_output(call_id),
    };
    assert!(
        output["output"]
            .to_string()
            .contains("command-after-interruption")
    );
    Ok(())
}
