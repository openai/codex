#[cfg(not(target_os = "linux"))]
compile_error!("the Wine exec-server test can only run on Linux");

#[path = "non_native_cwd_tests.rs"]
mod non_native_cwd_tests;

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_exec_server::ExecEnvPolicy;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecServerClient;
use codex_exec_server::ProcessId;
use codex_exec_server::ReadParams;
use codex_exec_server::RemoteExecServerConnectArgs;
use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tokio::time::timeout;
use wine_exec_server_harness::POWERSHELL_PATH;
use wine_exec_server_harness::WineExecServerHarness;

const TEST_TIMEOUT: Duration = Duration::from_secs(180);
const POWERSHELL_PREFLIGHT_MARKER: &str = "WINE_PWSH_PREFLIGHT";
const WINDOWS_WORKSPACE: &str = r"C:\workspace";
const POWERSHELL_PREFLIGHT_SCRIPT: &str = concat!(
    "$ErrorActionPreference = 'Stop'; ",
    "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); ",
    "$separatorCode = [int]([System.IO.Path]::DirectorySeparatorChar); ",
    "Write-Output ('WINE_PWSH_PREFLIGHT|' + ",
    "$PSVersionTable.PSVersion.ToString() + '|' + ",
    "$PSVersionTable.PSEdition + '|' + ",
    "$IsWindows.ToString().ToLowerInvariant() + '|' + ",
    "(Get-Location).ProviderPath + '|' + $separatorCode)",
);

struct CommandOutput {
    exit_code: Option<i32>,
    output: String,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn windows_exec_server_runs_powershell_and_cmd_under_wine() -> Result<()> {
    timeout(TEST_TIMEOUT, async {
        let (server, websocket_url) = WineExecServerHarness::builder()
            .workspace(WINDOWS_WORKSPACE)
            .start()
            .await?;
        server.scope(exercise_exec_server(websocket_url)).await
    })
    .await
    .context("Wine exec-server test timed out")?
}

async fn exercise_exec_server(websocket_url: String) -> Result<()> {
    let client = ExecServerClient::connect_websocket(RemoteExecServerConnectArgs::new(
        websocket_url,
        "wine-windows-bazel-test".to_string(),
    ))
    .await?;

    let info = client.environment_info().await?;
    assert_eq!(info.shell.name, "powershell");
    assert!(
        info.shell.path.eq_ignore_ascii_case(POWERSHELL_PATH),
        "expected pinned PowerShell path, got {info:?}",
    );

    let powershell = run_non_tty_command(
        &client,
        "wine-pwsh-preflight",
        vec![
            info.shell.path,
            "-NoLogo".to_string(),
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-Command".to_string(),
            POWERSHELL_PREFLIGHT_SCRIPT.to_string(),
        ],
        PathUri::parse("file:///C:/workspace")?,
    )
    .await?;
    assert_eq!(
        powershell.exit_code,
        Some(0),
        "unexpected PowerShell output: {:?}",
        powershell.output
    );
    let preflight = powershell
        .output
        .lines()
        .find(|line| line.starts_with(POWERSHELL_PREFLIGHT_MARKER))
        .with_context(|| {
            format!(
                "PowerShell preflight marker was missing from {:?}",
                powershell.output
            )
        })?;
    let fields = preflight.split('|').collect::<Vec<_>>();
    anyhow::ensure!(fields.len() == 6, "unexpected PowerShell preflight: {preflight}");
    assert_eq!(fields[0], POWERSHELL_PREFLIGHT_MARKER);
    assert_eq!(
        fields[1].split('.').next(),
        Some("7"),
        "expected PowerShell 7.x, got {}",
        fields[1],
    );
    assert_eq!(
        &fields[2..],
        &["Core", "true", WINDOWS_WORKSPACE, "92"]
    );

    let cmd = run_non_tty_command(
        &client,
        "wine-cmd-smoke",
        vec![
            r"C:\windows\system32\cmd.exe".to_string(),
            "/D".to_string(),
            "/C".to_string(),
            "echo WINE_BAZEL_OK&&cd".to_string(),
        ],
        PathUri::parse("file:///C:/workspace")?,
    )
    .await?;
    assert_eq!(cmd.exit_code, Some(0));
    assert!(
        cmd.output.contains("WINE_BAZEL_OK"),
        "unexpected output: {:?}",
        cmd.output
    );
    assert!(
        cmd.output.contains(WINDOWS_WORKSPACE),
        "unexpected output: {:?}",
        cmd.output
    );

    Ok(())
}

async fn run_non_tty_command(
    client: &ExecServerClient,
    process_id: &str,
    argv: Vec<String>,
    cwd: PathUri,
) -> Result<CommandOutput> {
    let process_id = ProcessId::from(process_id);
    let response = client
        .exec(ExecParams {
            process_id: process_id.clone(),
            argv,
            cwd,
            env_policy: Some(ExecEnvPolicy {
                inherit: ShellEnvironmentPolicyInherit::Core,
                ignore_default_excludes: true,
                exclude: Vec::new(),
                r#set: HashMap::new(),
                include_only: Vec::new(),
            }),
            env: HashMap::from([
                ("DOTNET_CLI_TELEMETRY_OPTOUT".to_string(), "1".to_string()),
                ("DOTNET_NOLOGO".to_string(), "1".to_string()),
                (
                    "POWERSHELL_TELEMETRY_OPTOUT".to_string(),
                    "1".to_string(),
                ),
                ("POWERSHELL_UPDATECHECK".to_string(), "Off".to_string()),
            ]),
            tty: false,
            pipe_stdin: false,
            arg0: None,
        })
        .await?;
    assert_eq!(response.process_id, process_id);

    let mut after_seq = None;
    let mut output = Vec::new();
    let exit_code = loop {
        let response = client
            .read(ReadParams {
                process_id: process_id.clone(),
                after_seq,
                max_bytes: Some(1024 * 1024),
                wait_ms: Some(5_000),
            })
            .await?;
        for chunk in response.chunks {
            output.extend(chunk.chunk.into_inner());
        }
        if response.closed {
            break response.exit_code;
        }
        after_seq = response.next_seq.checked_sub(1);
    };

    Ok(CommandOutput {
        exit_code,
        output: String::from_utf8(output)?,
    })
}
