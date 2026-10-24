#![cfg(test)]

//! Tool hooks report local command cwd and retain host cwd for remote commands.

use super::*;
use codex_exec_server::CreateDirectoryOptions;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::test_codex::local;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[test_case(None, "allow"; "default")]
#[test_case(Some(""), "allow"; "empty")]
#[test_case(Some("nested/../nested"), "allow"; "relative")]
#[test_case(Some("absolute"), "allow"; "absolute")]
#[test_case(Some("nested"), "json_deny"; "denied")]
#[test_case(Some("nested"), "rewrite"; "rewritten")]
#[tokio::test]
async fn pre_tool_use_cwd_matches_execution(
    workdir: Option<&str>,
    mode: &'static str,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex()
        .with_pre_build_hook(move |home| {
            if mode == "rewrite" {
                write_updating_pre_tool_use_hook(
                    home,
                    "^Bash$",
                    &json!({ "command": "cat rewritten-marker.txt" }),
                )
                .expect("write rewriting hook");
            } else {
                write_pre_tool_use_hook(home, Some("^Bash$"), mode, "blocked by cwd hook")
                    .expect("write recording hook");
            }
            let script_path = home.join("pre_tool_use_hook.py");
            let script = fs::read_to_string(&script_path)
                .expect("read hook script")
                .replace(
                    "payload = json.load(sys.stdin)",
                    "payload = json.load(sys.stdin)\npayload['process_cwd'] = str(Path.cwd())",
                );
            fs::write(script_path, script).expect("record hook process cwd");
        })
        .with_config(trust_discovered_hooks)
        .build_with_remote_and_local_env(&server)
        .await?;

    // Hooks run on the host even when commands use the remote test executor.
    let local_hook_directory = TempDir::new()?;
    let mut environments = vec![test.executor_environment().selection().clone()];
    let hook_cwd = if test.executor_environment().environment().is_remote() {
        let cwd = AbsolutePathBuf::try_from(local_hook_directory.path())?;
        environments.push(local(cwd.clone()));
        cwd
    } else {
        test.config.cwd.clone()
    };
    let root = test.executor_environment().selection().cwd.clone();
    let nested = root.join("nested")?;
    test.fs()
        .create_directory(
            &nested,
            CreateDirectoryOptions {
                recursive: true,
                follow_symlinks: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    for (directory, marker) in [(&root, "root-marker"), (&nested, "nested-marker")] {
        for filename in ["cwd-marker.txt", "rewritten-marker.txt"] {
            test.fs()
                .write_file(
                    &directory.join(filename)?,
                    format!("{marker}:{filename}").into_bytes(),
                    Default::default(),
                    /*sandbox*/ None,
                )
                .await?;
        }
    }
    let mut args = json!({ "cmd": "cat cwd-marker.txt", "login": false });
    if let Some(workdir) = workdir {
        args["workdir"] = if workdir == "absolute" {
            json!(nested.inferred_native_path_string())
        } else {
            json!(workdir)
        };
    }
    let call_id = "hook-cwd";
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "exec_command", &args.to_string()),
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
    test.submit_turn_with_environments(
        "read the marker in the requested directory",
        Some(environments),
    )
    .await?;

    let inputs = read_pre_tool_use_hook_inputs(test.codex_home_path())?;
    let expected_cwd = if workdir.is_none_or(str::is_empty) {
        &root
    } else {
        &nested
    };
    assert_eq!(
        inputs
            .iter()
            .map(|input| json!({
                "cwd": input["cwd"],
                "process_cwd": input["process_cwd"],
                "tool_input": input["tool_input"],
            }))
            .collect::<Vec<_>>(),
        vec![json!({
            "cwd": if test.executor_environment().environment().is_remote() {
                dunce::canonicalize(&hook_cwd)?.to_string_lossy().into_owned()
            } else {
                expected_cwd.inferred_native_path_string()
            },
            "process_cwd": dunce::canonicalize(&hook_cwd)?.to_string_lossy(),
            "tool_input": { "command": "cat cwd-marker.txt" },
        })],
    );
    let requests = responses.requests();
    let output = requests[1].function_call_output(call_id);
    let output = output["output"].as_str().context("command output")?;
    if mode == "json_deny" {
        assert!(output.contains("Command blocked by PreToolUse hook: blocked by cwd hook"));
    } else {
        let marker = if expected_cwd == &root {
            "root-marker"
        } else {
            "nested-marker"
        };
        let filename = if mode == "rewrite" {
            "rewritten-marker.txt"
        } else {
            "cwd-marker.txt"
        };
        assert!(output.contains(&format!("{marker}:{filename}")), "{output}");
    }
    Ok(())
}

#[tokio::test]
#[tracing_test::traced_test]
async fn invalid_execution_cwd_records_failure_without_running_hooks() -> Result<()> {
    skip_if_no_network!(Ok(()));
    for (field, value) in [("environment_id", "missing-environment"), ("workdir", "\0")] {
        let server = start_mock_server().await;
        let mut builder = test_codex()
            .with_pre_build_hook(|home| {
                write_pre_tool_use_hook(home, Some("^Bash$"), "allow", "")
                    .expect("write recording hook");
            })
            .with_config(trust_discovered_hooks);
        let test = builder.build_with_auto_env(&server).await?;
        let args = json!({ "cmd": "git version", field: value });
        let responses = mount_sse_sequence(
            &server,
            vec![
                sse(vec![
                    ev_response_created("resp-1"),
                    ev_function_call(field, "exec_command", &args.to_string()),
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
        test.submit_turn("try the invalid shell arguments").await?;
        assert!(
            !test
                .codex_home_path()
                .join("pre_tool_use_hook_log.jsonl")
                .exists()
        );
        let output = responses.requests()[1].function_call_output(field);
        let output = output["output"].as_str().context("tool failure")?;
        assert!(!output.is_empty());
        logs_assert(|lines: &[&str]| {
            lines
                .iter()
                .any(|line| {
                    line.contains("codex.tool_result")
                        && line.contains(&format!("call_id={field}"))
                        && line.contains("success=false")
                })
                .then_some(())
                .ok_or_else(|| format!("missing failure telemetry for {field}"))
        });
    }
    Ok(())
}

#[test_case(None, "allow"; "default")]
#[test_case(Some(""), "allow"; "empty")]
#[test_case(Some("nested/../nested"), "allow"; "relative")]
#[test_case(Some("absolute"), "allow"; "absolute")]
#[test_case(Some("nested"), "deny"; "permission_denied")]
#[test_case(Some("nested"), "rewrite"; "rewritten")]
#[test_case(Some("nested"), "delayed"; "completion_in_another_turn")]
#[test_case(Some("nested"), "stdin"; "stdin_approval_in_another_turn")]
#[tokio::test]
async fn command_hooks_keep_execution_cwd(workdir: Option<&str>, mode: &'static str) -> Result<()> {
    use codex_protocol::protocol::TurnEnvironmentRequests;

    skip_if_no_network!(Ok(()));
    if mode == "stdin" {
        core_test_support::skip_if_target_windows!(Ok(()), "uses a POSIX read/tty fixture");
    }
    let later_turn = matches!(mode, "delayed" | "stdin");
    let server = start_mock_server().await;
    let test = test_codex()
        .with_pre_build_hook(move |home| {
            let script = format!(
                r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
payload['process_cwd'] = str(Path.cwd())
with Path({log_path:?}).open('a', encoding='utf-8') as log:
    log.write(json.dumps(payload) + '\n')
event = payload['hook_event_name']
if event == 'PermissionRequest':
    print(json.dumps({{'hookSpecificOutput': {{'hookEventName': event, 'decision': {{
        'behavior': 'deny' if {mode:?} == 'deny' else 'allow', 'message': 'cwd test denied'
    }}}}}}))
elif event == 'PreToolUse' and {mode:?} == 'rewrite':
    print(json.dumps({{'hookSpecificOutput': {{'hookEventName': event,
        'permissionDecision': 'allow',
        'updatedInput': {{'command': 'cat rewritten-marker.txt'}}
    }}}}))
"#,
                log_path = home.join("tool_cwd_hook_log.jsonl").to_string_lossy()
            );
            let script_path = home.join("tool_cwd_hook.py");
            fs::write(&script_path, script).expect("write hook recorder");
            let group = json!([{"matcher": "^(Bash|write_stdin)$", "hooks": [{
                "type": "command", "command": format!("python3 {}", script_path.display()),
            }]}]);
            fs::write(
                home.join("hooks.json"),
                json!({"hooks": {
                    "PreToolUse": group, "PermissionRequest": group, "PostToolUse": group,
                }})
                .to_string(),
            )
            .expect("write hook configuration");
        })
        .with_config(move |config| {
            trust_discovered_hooks(config);
            if mode == "stdin" {
                config
                    .features
                    .enable(Feature::WriteStdinApproval)
                    .expect("enable stdin approval");
            }
        })
        .build_with_remote_and_local_env(&server)
        .await?;
    // Remote command paths never replace the host cwd in hook input.
    let local_hook_directory = TempDir::new()?;
    let mut environments = vec![test.executor_environment().request()];
    let hook_cwd = if test.executor_environment().environment().is_remote() {
        let cwd = AbsolutePathBuf::try_from(local_hook_directory.path())?;
        environments.push(core_test_support::test_codex::local_request(cwd.clone()));
        cwd
    } else {
        test.config.cwd.clone()
    };
    let root = test.executor_environment().selection().cwd.clone();
    let nested = root.join("nested")?;
    let other = root.join("other")?;
    for dir in [&nested, &other] {
        test.fs()
            .create_directory(
                dir,
                CreateDirectoryOptions {
                    recursive: true,
                    follow_symlinks: true,
                },
                /*sandbox*/ None,
            )
            .await?;
    }
    for (dir, marker) in [(&root, "root"), (&nested, "nested")] {
        for filename in ["cwd-marker.txt", "rewritten-marker.txt"] {
            test.fs()
                .write_file(
                    &dir.join(filename)?,
                    format!("{marker}:{filename}").into_bytes(),
                    Default::default(),
                    /*sandbox*/ None,
                )
                .await?;
        }
    }
    let command = if mode == "stdin" {
        "read -r line; cat cwd-marker.txt"
    } else if mode == "delayed" {
        match test_target_os() {
            TestTargetOs::Windows => {
                "while (!(Test-Path release.txt)) { Start-Sleep -Milliseconds 50 }; Get-Content cwd-marker.txt"
            }
            TestTargetOs::Linux | TestTargetOs::MacOs => {
                "while [ ! -f release.txt ]; do sleep 0.05; done; cat cwd-marker.txt"
            }
        }
    } else {
        "cat cwd-marker.txt"
    };
    // A cold remote shell can take more than a second to start, especially under Wine.
    // Only the cross-turn cases intentionally return before the command completes.
    let yield_time_ms = if later_turn { 1000 } else { 30000 };
    let mut args = json!({"cmd": command, "login": false, "yield_time_ms": yield_time_ms,
        "sandbox_permissions": "require_escalated"});
    if mode == "stdin" {
        args["tty"] = json!(true);
    }
    if let Some(workdir) = workdir {
        args["workdir"] = if workdir == "absolute" {
            json!(nested.inferred_native_path_string())
        } else {
            json!(workdir)
        };
    }
    let call_id = "command-cwd";
    let mut response_events = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "exec_command", &args.to_string()),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    if later_turn {
        response_events.extend([
            sse(vec![
                ev_response_created("resp-3"),
                ev_function_call(
                    "poll-cwd",
                    "write_stdin",
                    &json!({
                        "session_id": 1000, "chars": if mode == "stdin" { "go\n" } else { "" }, "yield_time_ms": 5000,
                    })
                    .to_string(),
                ),
                ev_completed("resp-3"),
            ]),
            sse(vec![
                ev_response_created("resp-4"),
                ev_assistant_message("msg-2", "collected"),
                ev_completed("resp-4"),
            ]),
        ]);
    }
    let responses = mount_sse_sequence(&server, response_events).await;
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::read_only(), test.config.cwd.as_path());
    let turn_settings = ThreadSettingsOverrides {
        environments: Some(TurnEnvironmentRequests::new(
            test.config.cwd.clone(),
            environments.clone(),
        )),
        approval_policy: Some(AskForApproval::OnRequest),
        sandbox_policy: Some(sandbox_policy),
        permission_profile,
        ..Default::default()
    };
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "run the command".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(turn_settings.clone()),
        )
        .await?;
    wait_for_event(&test.codex, |event| {
        assert!(
            !matches!(event, EventMsg::ExecApprovalRequest(_)),
            "hook should decide approval"
        );
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let log_path = test.codex_home_path().join("tool_cwd_hook_log.jsonl");
    let expected_cwd = if workdir.is_none_or(str::is_empty) {
        &root
    } else {
        &nested
    };
    let mut post_process_cwd = dunce::canonicalize(&hook_cwd)?
        .to_string_lossy()
        .into_owned();
    let expected_hook_cwd = if test.executor_environment().environment().is_remote() {
        post_process_cwd.clone()
    } else {
        expected_cwd.inferred_native_path_string()
    };
    if later_turn {
        let first_output = responses.requests()[1].function_call_output(call_id);
        assert!(
            first_output["output"]
                .as_str()
                .context("running command output")?
                .contains("Process running with session ID 1000")
        );
        assert_eq!(
            fs::read_to_string(&log_path)?.lines().count(),
            2,
            "no post hook until completion"
        );
        if mode == "delayed" {
            test.fs()
                .write_file(
                    &expected_cwd.join("release.txt")?,
                    Vec::new(),
                    Default::default(),
                    /*sandbox*/ None,
                )
                .await?;
        }
        environments[0].cwd = other.clone();
        if !test.executor_environment().environment().is_remote() {
            post_process_cwd = dunce::canonicalize(other.to_abs_path()?.as_path())?
                .to_string_lossy()
                .into_owned();
        }
        test.codex
            .start_or_steer_turn(
                TurnInputRequest::user_input(vec![UserInput::Text {
                    text: "collect the command result".into(),
                    text_elements: Vec::new(),
                }])
                .with_thread_settings(ThreadSettingsOverrides {
                    environments: Some(TurnEnvironmentRequests::new(
                        test.config.cwd.clone(),
                        environments,
                    )),
                    ..turn_settings
                }),
            )
            .await?;
        wait_for_event(&test.codex, |event| {
            assert!(
                !matches!(event, EventMsg::ExecApprovalRequest(_)),
                "hook should decide stdin approval"
            );
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    }
    let inputs: Vec<Value> = fs::read_to_string(log_path)?
        .lines()
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    let executed_command = if mode == "rewrite" {
        "cat rewritten-marker.txt"
    } else {
        command
    };
    let mut expected = vec![];
    for (event, event_command) in [
        ("PreToolUse", command),
        ("PermissionRequest", executed_command),
        ("PostToolUse", executed_command),
    ] {
        if mode == "deny" && event == "PostToolUse" {
            continue;
        }
        if mode == "stdin" && event == "PostToolUse" {
            expected.push(json!({
                "event": "PermissionRequest", "tool_name": "write_stdin",
                "cwd": expected_hook_cwd, "process_cwd": post_process_cwd,
                "tool_input": {
                    "session_id": 1000, "chars": "go\n", "parent_call_id": call_id,
                    "approval_id": "poll-cwd", "environment_id": test.executor_environment().selection().environment_id,
                    "cwd": expected_cwd, "tty": true, "sandbox_permissions": "require_escalated",
                    "additional_permissions": null,
                },
            }));
        }
        expected.push(json!({
            "event": event, "tool_name": "Bash", "cwd": expected_hook_cwd,
            "process_cwd": if event == "PostToolUse" { post_process_cwd.clone() } else { dunce::canonicalize(&hook_cwd)?.to_string_lossy().into_owned() },
            "tool_input": {"command": event_command},
        }));
    }
    assert_eq!(
        inputs
            .iter()
            .map(|input| json!({
                "event": input["hook_event_name"], "tool_name": input["tool_name"], "cwd": input["cwd"],
                "process_cwd": input["process_cwd"], "tool_input": input["tool_input"],
            }))
            .collect::<Vec<_>>(),
        expected
    );
    let requests = responses.requests();
    let output = requests
        .last()
        .context("final model request")?
        .function_call_output(if later_turn { "poll-cwd" } else { call_id });
    let output = output["output"].as_str().context("command result")?;
    if mode == "deny" {
        assert!(output.contains("cwd test denied"), "{output}");
    } else {
        let filename = if mode == "rewrite" {
            "rewritten-marker.txt"
        } else {
            "cwd-marker.txt"
        };
        let marker = if workdir.is_none_or(str::is_empty) {
            "root"
        } else {
            "nested"
        };
        assert!(output.contains(&format!("{marker}:{filename}")), "{output}");
        assert_eq!(
            inputs.last().context("completion hook input")?["tool_use_id"],
            call_id
        );
    }
    Ok(())
}
