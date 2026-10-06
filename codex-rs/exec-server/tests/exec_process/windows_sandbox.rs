//! Shared Windows sandbox behavior over the real exec-server RPC connection.

use super::*;
use codex_exec_server::ExecEnvPolicy;
use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use pretty_assertions::assert_eq;

/// Windows alias casing must not prevent fallback when the primary runtime is absent from PATH.
#[test_case::test_case("pwsh.exe"; "lowercase_pwsh")]
#[test_case::test_case("PWSH.exe"; "uppercase_pwsh")]
#[test_case::test_case("powershell.exe"; "lowercase_powershell")]
#[test_case::test_case("PowerShell.exe"; "mixed_case_powershell")]
#[cfg_attr(not(windows), ignore = "requires a native Windows sandbox")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(remote_exec_server)]
async fn powershell_alias_falls_back_without_primary_runtime_over_rpc(
    shell_name: &str,
) -> Result<()> {
    crate::skip_if_mxc_unavailable!(Ok(()));
    let root = TempDir::new()?;
    let system32 = std::path::PathBuf::from(std::env::var("SystemRoot")?).join("System32");
    let server =
        common::exec_server::exec_server_with_env([("PATH", system32.as_os_str())], &[]).await?;
    let environment = Environment::create_for_tests(Some(server.websocket_url().to_owned()))?;
    let cwd = PathUri::from_host_native_path(root.path())?;
    let mut sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::read_only(),
        cwd.clone(),
    );
    sandbox.windows_sandbox_selection = codex_exec_server::WindowsSandboxSelection::Mxc;
    let started = environment
        .get_exec_backend()
        .start(ExecParams {
            process_id: ProcessId::from("windows-powershell-fallback"),
            metadata: None,
            argv: vec![
                root.path()
                    .join("WindowsApps")
                    .join(shell_name)
                    .to_string_lossy()
                    .into_owned(),
                "-NoProfile".to_owned(),
                "-Command".to_owned(),
                "Write-Output 'fallback-ok'".to_owned(),
            ],
            cwd,
            env_policy: None,
            shell_snapshot: None,
            env: HashMap::new(),
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
    let (stdout, stderr, exit_code, exited) = collect_process_output_from_events_with_timeout(
        started.process,
        Duration::from_secs(/*secs*/ 30),
    )
    .await?;
    assert_eq!(
        (stdout.trim(), stderr, exit_code, exited),
        ("fallback-ok", String::new(), Some(0), true)
    );
    Ok(())
}

#[test_case::test_case(WindowsSandboxSelection::Mxc, codex_sandboxing::SandboxType::WindowsMxc; "mxc")]
#[test_case::test_case(WindowsSandboxSelection::Elevated, codex_sandboxing::SandboxType::WindowsRestrictedToken; "elevated")]
#[cfg_attr(not(windows), ignore = "requires a native Windows sandbox")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial(remote_exec_server)]
async fn tmpdir_uses_command_environment_over_rpc(
    selection: WindowsSandboxSelection,
    expected_type: codex_sandboxing::SandboxType,
) -> Result<()> {
    if selection == WindowsSandboxSelection::Mxc {
        crate::skip_if_mxc_unavailable!(Ok(()));
    }
    #[cfg(windows)]
    let _account_guard = if selection == WindowsSandboxSelection::Elevated {
        let guard = codex_windows_sandbox_test_support::WindowsSandboxAccountTestGuard::acquire()?;
        // The RPC server re-enters this test executable, as in file_system_windows.
        let executable = std::env::current_exe()?;
        let resources = executable
            .parent()
            .context("Windows test executable should have a parent directory")?
            .join("codex-resources");
        if let Err(error) = std::fs::create_dir_all(&resources)
            && !(error.kind() == std::io::ErrorKind::PermissionDenied && resources.is_dir())
        {
            return Err(error).context("create Windows sandbox test resources");
        }
        for name in ["codex-windows-sandbox-setup", "codex-command-runner"] {
            let source = codex_utils_cargo_bin::cargo_bin(name)?;
            let destination = resources.join(std::path::Path::new(name).with_extension("exe"));
            if let Err(error) = codex_utils_cargo_bin::copy_executable(&source, &destination)
                && !(error.kind() == std::io::ErrorKind::PermissionDenied && destination.is_file())
            {
                return Err(error).with_context(|| format!("stage Windows sandbox helper {name}"));
            }
        }
        Some(guard)
    } else {
        None
    };
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
    sandbox.windows_sandbox_selection = selection;
    let env = HashMap::from([
        ("SystemRoot".to_owned(), std::env::var("SystemRoot")?),
        (
            "ALLOWED_FILE".to_owned(),
            format!("\"{}\"", command_temp.join("allowed.txt").display()),
        ),
        (
            "OUTSIDE_FILE".to_owned(),
            format!("\"{}\"", outside.display()),
        ),
    ]);
    // Raw params have no TEMP. The executor must finish the env policy before
    // resolving permissions. No inheritance: omitted TMP must not grant server_temp.
    let mut configured_temp = HashMap::from([(
        "TEMP".to_owned(),
        command_temp.to_string_lossy().into_owned(),
    )]);
    if selection == WindowsSandboxSelection::Mxc {
        configured_temp.insert(
            "TMP".to_owned(),
            command_temp.to_string_lossy().into_owned(),
        );
    }
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
                "echo CHILD-TEMP:%TEMP% & echo allowed>%ALLOWED_FILE% & echo modified>%OUTSIDE_FILE% & exit /b 0".to_owned(),
            ],
            cwd,
            env_policy: Some(ExecEnvPolicy {
                inherit: ShellEnvironmentPolicyInherit::None,
                ignore_default_excludes: false,
                exclude: Vec::new(),
                r#set: configured_temp,
                include_only: Vec::new(),
            }),
            shell_snapshot: None,
            env,
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: Some(sandbox),
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        })
        .await?;
    assert_eq!(started.sandbox_type, Some(expected_type));
    let (stdout, stderr, code, exited) = collect_process_output_from_events_with_timeout(
        started.process,
        Duration::from_secs(/*secs*/ 30),
    )
    .await?;
    assert_eq!(
        (stdout.trim(), code, exited),
        (
            format!("CHILD-TEMP:{}", command_temp.display()).as_str(),
            Some(0),
            true
        ),
        "unexpected launch result, stderr: {stderr}"
    );
    assert_eq!(
        (
            std::fs::read_to_string(command_temp.join("allowed.txt"))?
                .trim_end()
                .to_owned(),
            std::fs::read_to_string(outside)?
        ),
        ("allowed".to_owned(), "original".to_owned())
    );
    Ok(())
}
