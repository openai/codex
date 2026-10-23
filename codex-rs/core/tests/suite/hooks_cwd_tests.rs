#![cfg(test)]

//! PreToolUse reports local command cwd and retains host cwd for remote commands.

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
