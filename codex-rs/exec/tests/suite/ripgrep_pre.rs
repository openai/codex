#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex_exec::test_codex_exec;
use serde_json::json;
use std::os::unix::fs::PermissionsExt;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blocks_escaped_ripgrep_preprocessor() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let marker = test.cwd_path().join("preprocessor-ran");
    std::fs::write(test.cwd_path().join("input.txt"), "needle\n")?;

    let preprocessor = test.cwd_path().join("pre.sh");
    std::fs::write(
        &preprocessor,
        format!(
            "#!/bin/sh\nprintf ran > {:?}\ncat\n",
            marker.to_string_lossy()
        ),
    )?;
    let mut permissions = std::fs::metadata(&preprocessor)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&preprocessor, permissions)?;

    let call_id = "rg-pre-poc";
    let args = json!({
        "command": r"rg --pre\=./pre.sh needle input.txt",
        "login": false,
        "timeout_ms": 1_000,
    });
    let server = responses::start_mock_server().await;
    let provider_override = format!(
        "model_providers.mock={{ name = \"mock\", base_url = \"{}/v1\", env_key = \"PATH\", wire_api = \"responses\" }}",
        server.uri()
    );
    let resp_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call(
                    call_id,
                    "shell_command",
                    &serde_json::to_string(&args)?,
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-2"),
                responses::ev_assistant_message("msg-1", "done"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut cmd = test.cmd();
    let output = cmd
        .env_remove("CODEX_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .arg("-c")
        .arg(&provider_override)
        .arg("-c")
        .arg("model_provider=\"mock\"")
        .arg("--skip-git-repo-check")
        .arg("-s")
        .arg("workspace-write")
        .arg("run the ripgrep preprocessor proof of concept")
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("rejected")
            || stderr.contains("approval")
            || stderr.contains("declined")
            || stderr.contains("unacceptable risk"),
        "blocked run should report approval rejection\nStatus: {}\nStdout:\n{}\nStderr:\n{}",
        output.status,
        stdout,
        stderr
    );
    if let Some(tool_output) = resp_mock.function_call_output_text(call_id) {
        assert!(
            tool_output.contains("rejected")
                || tool_output.contains("approval")
                || tool_output.contains("unacceptable risk"),
            "blocked command should report approval rejection: {tool_output}"
        );
        assert!(
            !tool_output.contains("Exit code: 0"),
            "blocked command should not return a successful shell result: {tool_output}"
        );
    } else {
        assert!(
            !output.status.success(),
            "successful run should send the blocked command result back to the model\nStatus: {}\nStdout:\n{}\nStderr:\n{}",
            output.status,
            stdout,
            stderr
        );
    }
    assert!(!marker.exists(), "ripgrep --pre helper should not execute");

    Ok(())
}
