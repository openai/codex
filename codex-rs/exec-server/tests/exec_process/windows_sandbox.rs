//! Shared Windows sandbox behavior over the real exec-server RPC connection.

use super::*;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use pretty_assertions::assert_eq;

#[cfg_attr(not(windows), ignore = "requires a native Windows sandbox")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(remote_exec_server)]
async fn mxc_tmpdir_uses_command_environment_over_rpc() -> Result<()> {
    crate::skip_if_mxc_unavailable!(Ok(()));
    let root = TempDir::new()?;
    let command_temp = root.path().join("command temp");
    let server_temp = root.path().join("server temp");
    std::fs::create_dir(&command_temp)?;
    std::fs::create_dir(&server_temp)?;
    let outside = server_temp.join("outside.txt");
    std::fs::write(&outside, "original")?;
    let server = common::exec_server::exec_server_with_env(
        [
            ("TEMP", server_temp.as_os_str()),
            ("TMP", server_temp.as_os_str()),
        ],
        &[],
    )
    .await?;
    let environment = Environment::create_for_tests(Some(server.websocket_url().to_owned()))?;
    let cwd = PathUri::from_host_native_path(root.path())?;
    let fs = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Read,
        ),
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Tmpdir,
            },
            FileSystemAccessMode::Write,
        ),
    ]);
    let mut sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(&fs, NetworkSandboxPolicy::Restricted),
        cwd.clone(),
    );
    sandbox.windows_sandbox_selection = codex_exec_server::WindowsSandboxSelection::Mxc;
    let command_temp = command_temp.to_string_lossy().into_owned();
    let started = environment
        .get_exec_backend()
        .start(ExecParams {
            process_id: ProcessId::from("windows-sandbox-temp"),
            metadata: None,
            argv: vec![
                r"C:\Windows\System32\cmd.exe".to_owned(),
                "/D".to_owned(),
                "/S".to_owned(),
                "/C".to_owned(),
                format!(
                    "echo allowed>\"%TEMP%\\allowed.txt\" & 2>\"%TEMP%\\denied.txt\" echo modified>\"{}\" & exit /b 0",
                    outside.display()
                ),
            ],
            cwd,
            env_policy: None,
            shell_snapshot: None,
            env: HashMap::from([
                ("SystemRoot".to_owned(), std::env::var("SystemRoot")?),
                ("TEMP".to_owned(), command_temp.clone()),
                ("TMP".to_owned(), command_temp),
            ]),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: Some(sandbox),
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;
    assert_eq!(
        started.sandbox_type,
        Some(codex_sandboxing::SandboxType::WindowsMxc)
    );
    assert_eq!(
        collect_process_output_from_events_with_timeout(
            started.process,
            Duration::from_secs(/*secs*/ 30),
        )
        .await?,
        (String::new(), String::new(), Some(0), true)
    );
    assert_eq!(
        (
            std::fs::read_to_string(root.path().join("command temp").join("allowed.txt"))?
                .trim_end()
                .to_owned(),
            std::fs::read_to_string(outside)?
        ),
        ("allowed".to_owned(), "original".to_owned())
    );
    Ok(())
}
