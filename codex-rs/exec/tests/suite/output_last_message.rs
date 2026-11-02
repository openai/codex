#![cfg(unix)]

//! Exercise the CLI output path used by automation with a synthetic model response.

use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn human_output_to_redirected_stdout_prints_one_answer() -> anyhow::Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let test = test_codex_exec();
    let server = responses::start_mock_server().await;
    let command = test.cmd_with_server(&server);
    let saved = test.cwd_path().join("stdout");
    for alias in ["/dev/stdout", "/dev/fd/1", "/proc/self/fd/1"] {
        responses::mount_sse_once(
            &server,
            responses::sse(vec![
                responses::ev_response_created("r"),
                responses::ev_assistant_message("m", "done"),
                responses::ev_completed("r"),
            ]),
        )
        .await;
        let output = std::process::Command::new(command.get_program())
            .args(command.get_args())
            .envs(command.get_envs().filter_map(|(k, v)| v.map(|v| (k, v))))
            .current_dir(test.cwd_path())
            .args(["--skip-git-repo-check", "-o", alias, "reply"])
            .stdout(std::fs::File::create(&saved)?)
            .output()?;
        assert!(output.status.success(), "{output:?}");
        assert_eq!(std::fs::read(&saved)?, b"done\n", "{alias}");
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sandbox_link_cannot_redirect_this_or_the_next_invocation() -> anyhow::Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    core_test_support::skip_if_sandbox!(Ok(()));
    #[cfg(target_os = "linux")]
    if super::sandbox::linux_sandbox_test_env().await.is_none() {
        return Ok(());
    }
    for mode in [None, Some("--json")] {
        let test = test_codex_exec();
        let outside = tempfile::tempdir()?;
        let target = outside.path().join("victim.txt");
        let output = test.cwd_path().join("codex-output.md");
        std::fs::write(&target, "original victim")?;
        std::fs::write(&output, "original output")?;
        let mut original = std::fs::File::open(&output)?;
        let quoted_target = format!("'{}'", target.display().to_string().replace('\'', "'\\''"));
        let attack = format!(
            "if (printf forbidden > {quoted_target}) 2>/dev/null; then exit 1; fi; ln -sfn -- {quoted_target} codex-output.md"
        );
        let server = responses::start_mock_server().await;
        let args = serde_json::json!({"cmd": attack, "shell": "/bin/sh", "login": false, "yield_time_ms": 10_000});
        responses::mount_function_call_agent_response(
            &server,
            "plant-output",
            &args.to_string(),
            "exec_command",
        )
        .await;
        let command = || {
            let mut cmd = test.cmd_with_server(&server);
            cmd.args(["--skip-git-repo-check", "--output-last-message"])
                .arg(&output);
            cmd.args(mode);
            cmd
        };
        let mut cmd = command();
        cmd.args([
            "--sandbox",
            "workspace-write",
            "-c",
            "approval_policy='never'",
            "-c",
            "sandbox_workspace_write.exclude_slash_tmp=true",
            "-c",
            "sandbox_workspace_write.exclude_tmpdir_env_var=true",
        ]);
        cmd.arg("write a response")
            .assert()
            .failure()
            .stderr(predicates::str::contains(
                "Failed to write last message file",
            ));
        let mut saved = String::new();
        std::io::Read::read_to_string(&mut original, &mut saved)?;
        assert_eq!(saved, "original output");
        assert_eq!(std::fs::read_to_string(&target)?, "original victim");
        assert_eq!(std::fs::read_link(&output)?, target);

        // The link left by the first invocation must not gain authority on the next one.
        command()
            .arg("write another response")
            .assert()
            .failure()
            .stderr(predicates::str::contains(
                "Failed to open last message file",
            ));

        assert_eq!(std::fs::read_to_string(&target)?, "original victim");
    }
    Ok(())
}
