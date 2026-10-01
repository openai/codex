//! Verify exec forwards Cyber selection per turn and rejects unsupported commands.

use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex_exec::test_codex_exec;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cyber_access_program_is_selected_per_turn() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    std::fs::write(
        test.home_path().join("config.toml"),
        "[features]\napi_key_cyber_access_programs = true\napi_key_model_discovery = true\n",
    )?;
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        (0..4)
            .map(|index| {
                responses::sse(vec![responses::ev_completed(&format!(
                    "resp-cyber-{index}"
                ))])
            })
            .collect(),
    )
    .await;

    let seed = test
        .cmd_with_server(&server)
        .args([
            "--skip-git-repo-check",
            "--json",
            "--cyber-access-program",
            "daybreak_blue",
            "Respond only OK.",
        ])
        .assert()
        .success();
    let events = std::str::from_utf8(&seed.get_output().stdout)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let thread_id = events
        .iter()
        .find(|event| event["type"] == "thread.started")
        .and_then(|event| event["thread_id"].as_str())
        .expect("new turn should emit a thread id");

    for (command, prompt, program) in [
        ("resume", "Respond only OK.", Some("daybreak_red")),
        ("fork", "-", Some("standard")),
        ("fork", "Respond only OK.", None),
    ] {
        let mut cmd = test.cmd_with_server(&server);
        cmd.args(["--skip-git-repo-check", command, thread_id]);
        if let Some(program) = program {
            cmd.args(["--cyber-access-program", program]);
        }
        if prompt == "-" {
            cmd.write_stdin("Respond only OK.");
        }
        cmd.arg(prompt).assert().success();
    }

    assert_eq!(
        response_mock
            .requests()
            .iter()
            .map(|request| request.body_json().get("access_programs").cloned())
            .collect::<Vec<_>>(),
        vec![
            Some(json!({"cyber": "daybreak_blue"})),
            Some(json!({"cyber": "daybreak_red"})),
            Some(json!({"cyber": "standard"})),
            None,
        ]
    );
    Ok(())
}

#[test]
fn cyber_access_program_requires_a_supported_turn() {
    let test = test_codex_exec();
    for (args, message) in [
        (
            vec!["review", "--uncommitted"],
            "is not supported with `codex exec review`",
        ),
        (vec!["fork", "synthetic-session"], "requires a prompt"),
    ] {
        test.cmd()
            .args(["--cyber-access-program", "daybreak_blue"])
            .args(args)
            .assert()
            .failure()
            .stderr(predicates::str::contains(message));
    }
}
