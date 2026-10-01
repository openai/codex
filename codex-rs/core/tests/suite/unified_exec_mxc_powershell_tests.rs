//! Exercises local PowerShell discovery before launching commands in native MXC.

use anyhow::Context;
use anyhow::Result;
use codex_protocol::models::PermissionProfile;
use codex_shell_command::shell_detect::DetectedShell;
use codex_shell_command::shell_detect::ShellType;
use core_test_support::responses::mount_function_call_agent_response;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mxc_replaces_store_powershell_before_launch() -> Result<()> {
    if !codex_sandboxing::windows_mxc_available() {
        eprintln!("skipping test: native MXC is unavailable on this host");
        return Ok(());
    }

    // The Store executable need not exist: the fallback must replace it before launch.
    let shell = DetectedShell {
        shell_type: ShellType::PowerShell,
        shell_path: PathBuf::from(
            r"C:\Program Files\WindowsApps\Microsoft.PowerShell_test\pwsh.exe",
        ),
    };
    let builder = test_codex()
        .with_user_shell(shell.into())
        .with_pre_build_hook(|home| {
            std::fs::write(home.join("config.toml"), "[windows]\nsandbox = \"mxc\"\n")
                .expect("write MXC config");
        })
        .with_config(|config| {
            config.codex_self_exe = Some(std::env::current_exe().expect("test executable"));
        });
    // Discovery is intentionally host-local; remote execution must not use the host's shell.
    let harness = TestCodexHarness::with_builder(builder).await?;
    let mock = mount_function_call_agent_response(
        harness.server(),
        "mxc-powershell",
        &json!({
            "cmd": "Set-Content -LiteralPath fallback.txt -Value mxc-powershell -NoNewline",
            "login": false,
            "yield_time_ms": 10_000,
        })
        .to_string(),
        "exec_command",
    )
    .await;
    harness
        .submit_with_permission_profile(
            "Write a file using PowerShell in the sandbox.",
            PermissionProfile::workspace_write(),
        )
        .await?;

    let request = mock.completion.single_request();
    let tool_output = request.function_call_output("mxc-powershell");
    let mut output = tool_output["output"]
        .as_str()
        .expect("command output")
        .to_owned();
    // A slow MXC launch may outlive exec_command's yield window.
    tokio::time::timeout(Duration::from_secs(120), async {
        for poll in 0.. {
            let Some(session_id) = output
                .lines()
                .find_map(|line| line.strip_prefix("Process running with session ID "))
            else {
                break;
            };
            let session_id = session_id.parse::<u32>()?;
            let call_id = format!("mxc-powershell-poll-{poll}");
            let poll_mock = mount_function_call_agent_response(
                harness.server(),
                &call_id,
                &json!({
                    "session_id": session_id,
                    "chars": "",
                    "yield_time_ms": 1_000,
                })
                .to_string(),
                "write_stdin",
            )
            .await;
            harness
                .submit_with_permission_profile(
                    "Wait for the PowerShell command to finish.",
                    PermissionProfile::workspace_write(),
                )
                .await?;
            let request = poll_mock.completion.single_request();
            let tool_output = request.function_call_output(&call_id);
            output = tool_output["output"]
                .as_str()
                .expect("poll output")
                .to_owned();
        }
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("timed out waiting for MXC PowerShell to exit")??;
    assert!(output.contains("Process exited with code 0"), "{output}");
    assert_eq!(
        std::fs::read_to_string(harness.path("fallback.txt"))?,
        "mxc-powershell"
    );
    let metadata: serde_json::Value = serde_json::from_str(
        &mock
            .function_call
            .single_request()
            .header("x-codex-turn-metadata")
            .expect("turn metadata"),
    )?;
    assert_eq!(metadata["sandbox"], "windows_mxc");
    Ok(())
}
