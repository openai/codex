use anyhow::Context;
use codex_config::types::ApprovalsReviewer;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_core::exec::ExecCapturePolicy;
use codex_core::exec::ExecParams;
use codex_core::exec::process_exec_tool_call;
use codex_core::sandboxing::SandboxPermissions;
use codex_core::windows_sandbox::WindowsSandboxSetupMode;
use codex_core::windows_sandbox::WindowsSandboxSetupRequest;
use codex_core::windows_sandbox::run_windows_sandbox_setup;
use codex_core::windows_sandbox::sandbox_setup_is_complete;
use codex_features::Feature;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::user_input::UserInput;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxDirectSpawnTransformRequest;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxTransformRequest;
use codex_sandboxing::SandboxType;
use codex_sandboxing::WindowsSandboxProxySettingsMode;
use codex_utils_path_uri::PathUri;
use codex_windows_sandbox_test_support::WindowsSandboxAccountTestGuard;
use core_test_support::PathExt;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_with_timeout;
use pretty_assertions::assert_eq;
use serde_json::json;
use serial_test::serial;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

struct EnvVarGuard {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
        let original = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.original {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

enum TestCodexHome {
    Persistent(PathBuf),
    Temporary(TempDir),
}

impl TestCodexHome {
    fn path(&self) -> &Path {
        match self {
            Self::Persistent(path) => path.as_path(),
            Self::Temporary(temp_dir) => temp_dir.path(),
        }
    }
}

fn codex_home_for_windows_sandbox_test(name: &str) -> anyhow::Result<TestCodexHome> {
    // Only tests that use the elevated accounts also take the shared account guard.
    if let Some(test_tmpdir) = std::env::var_os("TEST_TMPDIR") {
        // The elevated backend provisions machine-local sandbox users. Bazel
        // retries run in the same Windows VM, so keep CODEX_HOME stable within
        // the test temp root and let setup reconcile its persisted ACL state.
        let codex_home = PathBuf::from(test_tmpdir).join(name);
        std::fs::create_dir_all(&codex_home)
            .with_context(|| format!("create stable test CODEX_HOME {}", codex_home.display()))?;
        return Ok(TestCodexHome::Persistent(codex_home));
    }

    Ok(TestCodexHome::Temporary(TempDir::new()?))
}

fn stage_windows_sandbox_helpers() -> anyhow::Result<()> {
    let test_exe = std::env::current_exe().context("resolve current Windows test executable")?;
    let test_exe_dir = test_exe
        .parent()
        .context("Windows test executable should have a parent directory")?;
    let resources_dir = test_exe_dir.join("codex-resources");
    match std::fs::create_dir_all(&resources_dir) {
        Ok(()) => {}
        Err(err)
            if err.kind() == std::io::ErrorKind::PermissionDenied && resources_dir.is_dir() => {}
        Err(err) => {
            return Err(err)
                .with_context(|| format!("create resources dir {}", resources_dir.display()));
        }
    }
    for helper_name in ["codex-windows-sandbox-setup", "codex-command-runner"] {
        let helper = codex_utils_cargo_bin::cargo_bin(helper_name)?;
        let file_name = Path::new(helper_name).with_extension("exe");
        let destination = resources_dir.join(file_name);
        if let Err(err) = codex_utils_cargo_bin::copy_executable(&helper, &destination) {
            // A sandbox helper can briefly remain alive after the sandboxed
            // command exits. Bazel may retry the test while that process still
            // has the staged executable open, so keep the already-staged copy.
            if err.kind() == std::io::ErrorKind::PermissionDenied && destination.exists() {
                continue;
            }
            return Err(err).with_context(|| {
                format!(
                    "stage Windows sandbox helper {} at {}",
                    helper.display(),
                    destination.display()
                )
            });
        }
    }
    Ok(())
}

fn escape_toml_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

fn stage_windows_sandbox_cli(fixture_bin: &Path) -> anyhow::Result<(PathBuf, PathBuf)> {
    std::fs::create_dir_all(fixture_bin)?;
    let resources_dir = fixture_bin.join("codex-resources");
    std::fs::create_dir_all(&resources_dir)?;

    let codex_source = codex_utils_cargo_bin::cargo_bin("codex")?;
    let codex = fixture_bin.join("codex.exe");
    codex_utils_cargo_bin::copy_executable(&codex_source, &codex)
        .with_context(|| format!("copy {} to {}", codex_source.display(), codex.display()))?;
    for helper_name in ["codex-windows-sandbox-setup", "codex-command-runner"] {
        let helper = codex_utils_cargo_bin::cargo_bin(helper_name)?;
        let destination = resources_dir.join(Path::new(helper_name).with_extension("exe"));
        codex_utils_cargo_bin::copy_executable(&helper, &destination)
            .with_context(|| format!("copy {} to {}", helper.display(), destination.display()))?;
    }

    let probe_source = codex_utils_cargo_bin::cargo_bin("codex-windows-managed-deny-probe")?;
    let probe = fixture_bin.join("managed-deny-probe.exe");
    codex_utils_cargo_bin::copy_executable(&probe_source, &probe)
        .with_context(|| format!("copy {} to {}", probe_source.display(), probe.display()))?;
    Ok((codex, probe))
}

fn assert_managed_deny_probe(output: &std::process::Output, launch: usize) -> anyhow::Result<()> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "managed deny probe launch {launch} failed: status={:?}; stdout={stdout}; stderr={stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains("allowed-read:OK") && stdout.contains("allowed-import:OK"),
        "managed deny probe launch {launch} lost allowed access: {stdout}"
    );
    assert!(
        stdout.contains("denied-read:DENIED") && stdout.contains("denied-import:DENIED"),
        "managed deny probe launch {launch} did not enforce denied access: {stdout}"
    );
    assert!(
        !stdout.contains("UNEXPECTED_SUCCESS"),
        "managed deny probe launch {launch} leaked denied content: {stdout}"
    );
    Ok(())
}

#[test]
#[serial(codex_home)]
fn windows_sandbox_cli_preserves_managed_deny_reads_across_launches() -> anyhow::Result<()> {
    let _account_guard = WindowsSandboxAccountTestGuard::acquire()?;
    let codex_home =
        codex_home_for_windows_sandbox_test("windows-cli-managed-deny-read-codex-home")?;

    let fixture = TempDir::new()?;
    let fixture_root = dunce::canonicalize(fixture.path())?;
    let work = fixture_root.join("work");
    let runtime = fixture_root.join("runtime");
    let denied = runtime.join("denied");
    let bin = fixture_root.join("bin");
    std::fs::create_dir_all(&work)?;
    std::fs::create_dir_all(&denied)?;
    let (codex, probe) = stage_windows_sandbox_cli(&bin)?;

    let allowed_text = runtime.join("allowed.txt");
    let denied_text = denied.join("secret.txt");
    let allowed_module = runtime.join("allowed.dll");
    let denied_module = denied.join("secret.dll");
    std::fs::write(&allowed_text, "ALLOW-CONTROL\n")?;
    std::fs::write(&denied_text, "DENIED-CONTENT\n")?;
    let system_root = std::env::var_os("SystemRoot").context("resolve SystemRoot")?;
    let system_module = PathBuf::from(system_root)
        .join("System32")
        .join("version.dll");
    std::fs::copy(&system_module, &allowed_module).with_context(|| {
        format!(
            "copy {} to {}",
            system_module.display(),
            allowed_module.display()
        )
    })?;
    std::fs::copy(&system_module, &denied_module).with_context(|| {
        format!(
            "copy {} to {}",
            system_module.display(),
            denied_module.display()
        )
    })?;

    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            "default_permissions = \"managed-deny-test\"\n\
             \n\
             [windows]\n\
             sandbox = \"elevated\"\n\
             \n\
             [shell_environment_policy]\n\
             inherit = \"all\"\n\
             \n\
             [permissions.managed-deny-test.filesystem]\n\
             \":minimal\" = \"read\"\n\
             \":root\" = \"read\"\n\
             \"{}\" = \"write\"\n\
             \"{}\" = \"deny\"\n\
             \n\
             [permissions.managed-deny-test.network]\n\
             enabled = false\n",
            escape_toml_path(&work),
            escape_toml_path(&denied),
        ),
    )?;

    for launch in 1..=2 {
        let output = Command::new(&codex)
            .current_dir(&work)
            .env("CODEX_HOME", codex_home.path())
            .env("CODEX_WINDOWS_ALLOWED_TEXT", &allowed_text)
            .env("CODEX_WINDOWS_DENIED_TEXT", &denied_text)
            .env("CODEX_WINDOWS_ALLOWED_MODULE", &allowed_module)
            .env("CODEX_WINDOWS_DENIED_MODULE", &denied_module)
            .args(["sandbox", "--permission-profile"])
            .arg("managed-deny-test")
            .arg("--cd")
            .arg(&work)
            .arg("--")
            .arg(&probe)
            .output()?;
        assert_managed_deny_probe(&output, launch)?;
    }

    Ok(())
}

#[tokio::test]
#[serial(codex_home)]
async fn windows_elevated_setup_rejects_default_root_deny() -> anyhow::Result<()> {
    let codex_home = codex_home_for_windows_sandbox_test("windows-elevated-root-deny-codex-home")?;
    let workspace = TempDir::new()?;
    let cwd = dunce::canonicalize(workspace.path())?.abs();
    let file_system_sandbox_policy =
        FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        }]);
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Restricted,
    );

    let err = run_windows_sandbox_setup(WindowsSandboxSetupRequest {
        mode: WindowsSandboxSetupMode::Elevated,
        permission_profile,
        workspace_roots: vec![cwd.clone()],
        command_cwd: cwd.to_path_buf(),
        env_map: HashMap::new(),
        codex_home: codex_home.path().to_path_buf(),
    })
    .await
    .expect_err("elevated setup should reject default root deny");

    assert_eq!(
        err.to_string(),
        "elevated Windows sandbox requires effective `:root` read access"
    );
    Ok(())
}

#[tokio::test]
#[serial(codex_home)]
async fn windows_restricted_token_rejects_exact_and_glob_deny_read_policy() -> anyhow::Result<()> {
    let codex_home =
        codex_home_for_windows_sandbox_test("windows-restricted-token-deny-read-codex-home")?;
    let _codex_home_guard = EnvVarGuard::set("CODEX_HOME", codex_home.path().as_os_str());
    let workspace = TempDir::new()?;
    let cwd = dunce::canonicalize(workspace.path())?.abs();
    let secret = cwd.join("secret.env");
    let future_secret = cwd.join("future.env");
    let public = cwd.join("public.txt");
    std::fs::write(&secret, "glob secret\n")?;
    std::fs::write(&public, "public ok\n")?;

    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern {
                pattern: "**/*.env".to_string(),
            },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: future_secret.into(),
            },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Restricted,
    );

    let err = process_exec_tool_call(
        ExecParams {
            command: vec![
                "cmd.exe".to_string(),
                "/D".to_string(),
                "/C".to_string(),
                "type secret.env >NUL 2>NUL & echo exact secret 1>future.env 2>NUL & type future.env 2>NUL & type public.txt & exit /B 0"
                    .to_string(),
            ],
            cwd: cwd.clone(),
            expiration: 10_000.into(),
            capture_policy: ExecCapturePolicy::ShellTool,
            env: HashMap::new(),
            network: None,
            network_environment_id: None,
            sandbox_permissions: SandboxPermissions::UseDefault,
            windows_sandbox_level: WindowsSandboxLevel::RestrictedToken,
            justification: None,
            arg0: None,
        },
        &permission_profile,
        &cwd,
        std::slice::from_ref(&cwd),
        &None,
        /*codex_self_exe*/ &None,
        /*use_legacy_landlock*/ false,
        /*stdout_stream*/ None,
    )
    .await
    .expect_err("restricted-token sandbox should reject deny-read restrictions");

    assert_eq!(
        err.to_string(),
        "unsupported operation: windows unelevated restricted-token sandbox cannot enforce deny-read restrictions directly; refusing to run unsandboxed"
    );
    Ok(())
}

#[tokio::test]
#[serial(codex_home)]
async fn windows_elevated_temp_only_core_and_direct_spawn_enforce_carveouts() -> anyhow::Result<()>
{
    let _account_guard = WindowsSandboxAccountTestGuard::acquire()?;
    let codex_home = codex_home_for_windows_sandbox_test("windows-temp-only-child-codex-home")?;
    let _codex_home_guard = EnvVarGuard::set("CODEX_HOME", codex_home.path().as_os_str());
    stage_windows_sandbox_helpers()?;
    let fixture = TempDir::new()?;
    let root = dunce::canonicalize(fixture.path())?;
    let (codex, _) = stage_windows_sandbox_cli(&root.join("bin"))?;
    let cwd = root.join("work").abs();
    let temp = root.join("selected-temp");
    let other = root.join("other-temp");
    let read_only = temp.join("read-only");
    let denied = temp.join("denied");
    for path in [
        cwd.as_path(),
        other.as_path(),
        read_only.as_path(),
        denied.as_path(),
    ] {
        std::fs::create_dir_all(path)?;
    }
    std::fs::write(read_only.join("public.txt"), "READ-CONTROL\n")?;
    std::fs::write(denied.join("secret.txt"), "SECRET-CONTROL\n")?;
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![
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
            FileSystemSandboxEntry::new(read_only.clone().abs().into(), FileSystemAccessMode::Read),
            FileSystemSandboxEntry::new(denied.clone().abs().into(), FileSystemAccessMode::Deny),
        ]),
        NetworkSandboxPolicy::Restricted,
    );
    let command = concat!(
        "echo allowed>%ALLOWED_FILE% 2>NUL & ",
        "echo wrong>%OTHER_FILE% 2>NUL & ",
        "echo wrong>cwd.txt 2>NUL & ",
        "type %PUBLIC_FILE% 2>NUL & ",
        "echo wrong>%READONLY_FILE% 2>NUL & ",
        "type %SECRET_FILE% 2>NUL & ",
        "echo wrong>%DENIED_FILE% 2>NUL & exit /b 0"
    );
    // Cover duplicate TEMP through Core and a lone mixed-case TMP through the
    // actual direct-spawn wrapper. In both cases the child's value must match its grant.
    for direct_spawn in [false, true] {
        // As in the deny-read test below, put quoted paths in the environment
        // rather than in cmd /C's argument: the Win32 argv encoder escapes
        // literal argument quotes for CRT parsing, which cmd does not use.
        let mut env: HashMap<String, String> = [
            ("ALLOWED_FILE", temp.join("allowed.txt")),
            ("OTHER_FILE", other.join("wrong.txt")),
            ("PUBLIC_FILE", read_only.join("public.txt")),
            ("READONLY_FILE", read_only.join("wrong.txt")),
            ("SECRET_FILE", denied.join("secret.txt")),
            ("DENIED_FILE", denied.join("wrong.txt")),
        ]
        .into_iter()
        .map(|(name, path)| (name.to_string(), format!("\"{}\"", path.display())))
        .collect();
        let child_var = if direct_spawn {
            env.insert("tMp".to_string(), temp.to_string_lossy().into_owned());
            "%TMP%"
        } else {
            env.insert("Temp".to_string(), other.to_string_lossy().into_owned());
            env.insert("TEMP".to_string(), temp.to_string_lossy().into_owned());
            "%TEMP%"
        };
        let command = format!("echo CHILD-TEMP:{child_var} & {command}");
        let (exit_code, stdout, stderr) = if direct_spawn {
            let cwd_uri = PathUri::from_abs_path(&cwd);
            let request = SandboxManager::new().transform_for_direct_spawn(
                SandboxDirectSpawnTransformRequest {
                    workspace_roots: std::slice::from_ref(&cwd),
                    windows_sandbox_proxy_settings_mode: WindowsSandboxProxySettingsMode::Preserve,
                    transform: SandboxTransformRequest {
                        command: SandboxCommand {
                            program: "cmd.exe".into(),
                            args: vec!["/D".into(), "/C".into(), command],
                            cwd: cwd_uri.clone(),
                            env,
                            managed_network: None,
                            additional_permissions: None,
                        },
                        permissions: &permission_profile,
                        sandbox: SandboxType::WindowsRestrictedToken,
                        enforce_managed_network: false,
                        environment_id: None,
                        network: None,
                        sandbox_policy_cwd: &cwd_uri,
                        sandbox_exe: Some(codex.as_path()),
                        use_legacy_landlock: false,
                        windows_sandbox_level: WindowsSandboxLevel::Elevated,
                    },
                },
            )?;
            let output = Command::new(&request.command[0])
                .args(&request.command[1..])
                .current_dir(request.cwd.to_abs_path()?)
                .env_clear()
                .envs(&request.env)
                .output()?;
            (
                output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&output.stdout).into_owned(),
                String::from_utf8_lossy(&output.stderr).into_owned(),
            )
        } else {
            let output = process_exec_tool_call(
                ExecParams {
                    command: vec!["cmd.exe".into(), "/D".into(), "/C".into(), command],
                    cwd: cwd.clone(),
                    expiration: 30_000.into(),
                    capture_policy: ExecCapturePolicy::ShellTool,
                    env,
                    network: None,
                    network_environment_id: None,
                    sandbox_permissions: SandboxPermissions::UseDefault,
                    windows_sandbox_level: WindowsSandboxLevel::Elevated,
                    justification: None,
                    arg0: None,
                },
                &permission_profile,
                &cwd,
                std::slice::from_ref(&cwd),
                &None,
                /*codex_self_exe*/ &None,
                /*use_legacy_landlock*/ false,
                /*stdout_stream*/ None,
            )
            .await?;
            (output.exit_code, output.stdout.text, output.stderr.text)
        };
        assert_eq!(
            exit_code, 0,
            "direct_spawn={direct_spawn}: {stdout} {stderr}"
        );
        let expected_temp = format!("CHILD-TEMP:{}", temp.display());
        assert!(
            stdout
                .lines()
                .any(|line| line.trim().eq_ignore_ascii_case(&expected_temp)),
            "child TEMP did not match granted directory, direct_spawn={direct_spawn}: {stdout} {stderr}"
        );
        assert!(
            stdout.contains("READ-CONTROL"),
            "direct_spawn={direct_spawn}: {stdout} {stderr}"
        );
        assert!(
            !stdout.contains("SECRET-CONTROL"),
            "direct_spawn={direct_spawn}: {stdout} {stderr}"
        );
        assert_eq!(
            std::fs::read_to_string(temp.join("allowed.txt"))?.trim(),
            "allowed"
        );
        for path in [
            other.join("wrong.txt"),
            cwd.join("cwd.txt").into_path_buf(),
            read_only.join("wrong.txt"),
            denied.join("wrong.txt"),
        ] {
            assert!(
                !path.exists(),
                "direct_spawn={direct_spawn}: unexpectedly wrote {}",
                path.display()
            );
        }
        std::fs::remove_file(temp.join("allowed.txt"))?;
    }
    Ok(())
}

#[tokio::test]
#[serial(codex_home)]
async fn windows_elevated_does_not_create_missing_workspace_metadata() -> anyhow::Result<()> {
    let _account_guard = WindowsSandboxAccountTestGuard::acquire()?;
    let codex_home =
        codex_home_for_windows_sandbox_test("windows-elevated-missing-metadata-codex-home")?;
    let _codex_home_guard = EnvVarGuard::set("CODEX_HOME", codex_home.path().as_os_str());
    stage_windows_sandbox_helpers()?;
    let workspace = TempDir::new()?;
    let cwd = dunce::canonicalize(workspace.path())?.abs();
    let permission_profile = PermissionProfile::workspace_write()
        .materialize_project_roots_with_workspace_roots(std::slice::from_ref(&cwd));

    let output = process_exec_tool_call(
        ExecParams {
            command: vec![
                "cmd.exe".to_string(),
                "/D".to_string(),
                "/C".to_string(),
                "echo sandbox-ok".to_string(),
            ],
            cwd: cwd.clone(),
            expiration: 10_000.into(),
            capture_policy: ExecCapturePolicy::ShellTool,
            env: HashMap::new(),
            network: None,
            network_environment_id: None,
            sandbox_permissions: SandboxPermissions::UseDefault,
            windows_sandbox_level: WindowsSandboxLevel::Elevated,
            justification: None,
            arg0: None,
        },
        &permission_profile,
        &cwd,
        std::slice::from_ref(&cwd),
        &None,
        /*codex_self_exe*/ &None,
        /*use_legacy_landlock*/ false,
        /*stdout_stream*/ None,
    )
    .await?;

    assert_eq!(output.exit_code, 0, "sandboxed command should complete");
    for name in codex_protocol::permissions::PROTECTED_METADATA_PATH_NAMES {
        let path = cwd.join(name);
        assert!(
            !path.exists(),
            "elevated setup should not create missing workspace metadata: {}",
            path.display()
        );
    }

    let firewall_output = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            r#"
$ErrorActionPreference = 'Stop'
$sid = (Get-LocalUser -Name 'CodexSandboxOffline').SID.Value
$rules = foreach ($name in @('codex_sandbox_offline_block_inbound', 'codex_sandbox_offline_block_outbound')) {
    $rule = @(Get-NetFirewallRule -PolicyStore ActiveStore -DisplayName $name)
    if ($rule.Count -ne 1) { throw "Expected exactly one effective rule for $name" }
    $address = $rule | Get-NetFirewallAddressFilter
    $security = $rule | Get-NetFirewallSecurityFilter
    $port = $rule | Get-NetFirewallPortFilter
    [pscustomobject]@{
        name = $name
        direction = [string]$rule[0].Direction
        action = [string]$rule[0].Action
        enabled = [string]$rule[0].Enabled
        profile = [string]$rule[0].Profile
        protocol = [string]$port.Protocol
        localAddresses = @($address.LocalAddress)
        remoteAddresses = @($address.RemoteAddress | Sort-Object)
        localPorts = @($port.LocalPort)
        remotePorts = @($port.RemotePort)
        localUser = [string]$security.LocalUser
    }
}
[pscustomobject]@{ offlineSid = $sid; rules = @($rules) } | ConvertTo-Json -Depth 4 -Compress
"#,
        ])
        .output()
        .context("read offline sandbox firewall rules")?;
    assert!(
        firewall_output.status.success(),
        "read offline sandbox firewall rules: {}",
        String::from_utf8_lossy(&firewall_output.stderr)
    );
    let firewall_rules: serde_json::Value = serde_json::from_slice(&firewall_output.stdout)
        .context("parse offline sandbox firewall rules")?;
    let offline_sid = firewall_rules["offlineSid"]
        .as_str()
        .context("offline sandbox firewall rules should include their user SID")?;
    let local_user = format!("O:LSD:(A;;CC;;;{offline_sid})");
    let expected_rules: Vec<_> = [
        ("codex_sandbox_offline_block_inbound", "Inbound"),
        ("codex_sandbox_offline_block_outbound", "Outbound"),
    ]
    .into_iter()
    .map(|(name, direction)| {
        serde_json::json!({
            "name": name,
            "direction": direction,
            "action": "Block",
            "enabled": "True",
            "profile": "Any",
            "protocol": "Any",
            "localAddresses": ["Any"],
            "remoteAddresses": [
                "::",
                "::2-ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
                "0.0.0.0-126.255.255.255",
                "128.0.0.0-255.255.255.255",
            ],
            "localPorts": ["Any"],
            "remotePorts": ["Any"],
            "localUser": local_user,
        })
    })
    .collect();
    assert_eq!(firewall_rules["rules"], serde_json::json!(expected_rules));
    Ok(())
}

#[tokio::test]
#[serial(codex_home)]
async fn windows_elevated_enforces_deny_read_and_protects_setup_marker() -> anyhow::Result<()> {
    let _account_guard = WindowsSandboxAccountTestGuard::acquire()?;
    let codex_home = codex_home_for_windows_sandbox_test("windows-elevated-deny-read-codex-home")?;
    let _codex_home_guard = EnvVarGuard::set("CODEX_HOME", codex_home.path().as_os_str());
    stage_windows_sandbox_helpers()?;
    let workspace = TempDir::new()?;
    let cwd = dunce::canonicalize(workspace.path())?.abs();
    let glob_secret = cwd.join("secret.env");
    let public = cwd.join("public.txt");
    let user_profile = TempDir::new_in(dirs::home_dir().context("resolve user profile")?)?;
    let _user_profile_guard = EnvVarGuard::set("USERPROFILE", user_profile.path().as_os_str());
    let exact_secret = user_profile.path().join("exact-secret.txt");
    std::fs::write(&exact_secret, "exact secret\n")?;
    let bundled_skill_dir = user_profile.path().join(".codex/plugins/cache");
    std::fs::create_dir_all(&bundled_skill_dir)?;
    let bundled_skill = bundled_skill_dir.join("SKILL.md");
    let setup_marker = codex_home.path().join(".sandbox").join("setup_marker.json");
    std::fs::write(&glob_secret, "glob secret\n")?;
    std::fs::write(&public, "public ok\n")?;
    std::fs::write(&bundled_skill, "bundled skill ok\n")?;

    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern {
                pattern: "**/*.env".to_string(),
            },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: exact_secret.clone().abs().into(),
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Restricted,
    );

    let ExecToolCallOutput {
        exit_code,
        stdout,
        ..
    } = process_exec_tool_call(
        ExecParams {
            command: vec![
                "cmd.exe".to_string(),
                "/D".to_string(),
                "/C".to_string(),
                "(type secret.env 1>NUL 2>NUL && echo GLOB-READ || echo GLOB-DENIED) & (type %SETUP_MARKER% 1>NUL 2>NUL && echo MARKER-READ-ALLOWED || echo MARKER-READ-DENIED) & (echo tampered > %SETUP_MARKER% 2>NUL && echo MARKER-WRITE-ALLOWED || echo MARKER-WRITE-DENIED) & type public.txt & type %BUNDLED_SKILL% & (type %EXACT_SECRET% 1>NUL 2>NUL && echo EXACT-READ || echo EXACT-DENIED)".to_string(),
            ],
            cwd: cwd.clone(),
            expiration: 10_000.into(),
            capture_policy: ExecCapturePolicy::ShellTool,
            env: [
                ("BUNDLED_SKILL", &bundled_skill),
                ("EXACT_SECRET", &exact_secret),
                ("SETUP_MARKER", &setup_marker),
            ]
            .into_iter()
            .map(|(name, path)| (name.to_string(), format!("\"{}\"", path.display())))
            .collect(),
            network: None,
            network_environment_id: None,
            sandbox_permissions: SandboxPermissions::UseDefault,
            windows_sandbox_level: WindowsSandboxLevel::Elevated,
            justification: None,
            arg0: None,
        },
        &permission_profile,
        &cwd,
        std::slice::from_ref(&cwd),
        &None,
        /*codex_self_exe*/ &None,
        /*use_legacy_landlock*/ false,
        /*stdout_stream*/ None,
    )
    .await?;

    assert_eq!(exit_code, 0, "sandboxed command should complete");
    assert!(
        stdout.text.contains("GLOB-DENIED"),
        "glob deny-read should block the secret: {stdout:?}"
    );
    assert!(
        !stdout.text.contains("GLOB-READ"),
        "glob deny-read should not allow the secret: {stdout:?}"
    );
    assert!(
        stdout.text.contains("EXACT-DENIED"),
        "exact deny-read should block the secret: {stdout:?}"
    );
    assert!(
        !stdout.text.contains("EXACT-READ"),
        "exact deny-read should not allow the secret: {stdout:?}"
    );
    assert!(
        stdout.text.contains("public ok"),
        "allowed reads should still work: {stdout:?}"
    );
    assert!(stdout.text.contains("bundled skill ok"));
    assert!(
        stdout.text.contains("MARKER-READ-DENIED"),
        "sandboxed command should not read setup readiness: {stdout:?}"
    );
    assert!(
        stdout.text.contains("MARKER-WRITE-DENIED"),
        "sandboxed command should not modify setup readiness: {stdout:?}"
    );
    assert!(
        !stdout.text.contains("MARKER-READ-ALLOWED"),
        "sandboxed command must not read setup readiness: {stdout:?}"
    );
    assert!(
        !stdout.text.contains("MARKER-WRITE-ALLOWED"),
        "sandboxed command must not modify setup readiness: {stdout:?}"
    );
    assert!(
        sandbox_setup_is_complete(codex_home.path()),
        "setup should remain ready after the tamper attempt"
    );
    Ok(())
}

#[tokio::test]
#[serial(codex_home)]
async fn windows_elevated_powershell_preserves_relative_paths() -> anyhow::Result<()> {
    let _account_guard = WindowsSandboxAccountTestGuard::acquire()?;
    let codex_home = codex_home_for_windows_sandbox_test(
        "windows-elevated-powershell-relative-paths-codex-home",
    )?;
    let _codex_home_guard = EnvVarGuard::set("CODEX_HOME", codex_home.path().as_os_str());
    stage_windows_sandbox_helpers()?;
    // The refresh caller must own the helper directory after provisioning protects its DACL.
    std::fs::create_dir_all(codex_windows_sandbox::sandbox_bin_dir(codex_home.path()))?;

    let profile_dir = TempDir::new()?;
    let fake_profile = dunce::canonicalize(profile_dir.path())?;
    // Exclude inherited sandbox permissions so this reproduces an inaccessible profile root.
    let profile_acl_setup = std::process::Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"])
        .arg(
            r#"$ErrorActionPreference = 'Stop'
$owner = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$security = [System.Security.AccessControl.DirectorySecurity]::new()
$security.SetSecurityDescriptorSddlForm("D:P(A;OICI;FA;;;${owner})(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)", [System.Security.AccessControl.AccessControlSections]::Access)
[System.IO.DirectoryInfo]::new($env:CODEX_TEST_PROFILE).SetAccessControl($security)"#,
        )
        .env("CODEX_TEST_PROFILE", &fake_profile)
        .output()
        .context("configure protected synthetic user-profile ACL")?;
    assert!(
        profile_acl_setup.status.success(),
        "synthetic profile ACL setup failed: {profile_acl_setup:?}"
    );
    let workspace = fake_profile.join("project");
    std::fs::create_dir(&workspace)?;
    let cwd = dunce::canonicalize(&workspace)?.abs();
    std::fs::write(cwd.join("public.txt"), "public ok\n")?;
    // Elevated setup requires root read access, including PowerShell's runtime.
    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Read,
        ),
        FileSystemSandboxEntry::new(cwd.clone().into(), FileSystemAccessMode::Write),
    ]);
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Restricted,
    );
    let env = HashMap::from([
        (
            "USERPROFILE".to_string(),
            fake_profile.to_string_lossy().into_owned(),
        ),
        (
            "SystemRoot".to_string(),
            std::env::var("SystemRoot").context("Windows PowerShell requires SystemRoot")?,
        ),
    ]);
    // Provision and refresh synchronously so USERPROFILE is restored before any await.
    {
        let _user_profile_guard = EnvVarGuard::set("USERPROFILE", fake_profile.as_os_str());
        codex_core::windows_sandbox::prepare_elevated_sandbox(
            &permission_profile,
            std::slice::from_ref(&cwd),
            cwd.as_path(),
            &env,
            codex_home.path(),
        )?;
    }

    let expected = format!("CWD={}\nPUBLIC=public ok", cwd.as_path().display());
    let mut attempts = 0;
    loop {
        let output = process_exec_tool_call(
            ExecParams {
                command: [
                    "powershell.exe",
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    r#"$ErrorActionPreference = 'Stop'
Write-Output ('CWD=' + (Get-Location).ProviderPath)
Write-Output ('PUBLIC=' + (Get-Content -LiteralPath .\public.txt -Raw).Trim())"#,
                ]
                .map(str::to_owned)
                .into(),
                cwd: cwd.clone(),
                expiration: 30_000.into(),
                capture_policy: ExecCapturePolicy::ShellTool,
                env: env.clone(),
                network: None,
                network_environment_id: None,
                sandbox_permissions: SandboxPermissions::UseDefault,
                windows_sandbox_level: WindowsSandboxLevel::Elevated,
                justification: None,
                arg0: None,
            },
            &permission_profile,
            &cwd,
            std::slice::from_ref(&cwd),
            &None,
            /*codex_self_exe*/ &None,
            /*use_legacy_landlock*/ false,
            /*stdout_stream*/ None,
        )
        .await?;
        if output.exit_code == 0
            && output
                .stdout
                .text
                .trim()
                .replace("\r\n", "\n")
                .eq_ignore_ascii_case(&expected)
        {
            return Ok(());
        }
        attempts += 1;
        assert!(
            attempts < 5,
            "PowerShell should preserve cwd and read .\\public.txt: {output:?}"
        );
        // The async read-ACL helper briefly uses a temporary startup junction.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(codex_home)]
async fn windows_elevated_unified_exec_enforces_large_recursive_deny_reads() -> anyhow::Result<()> {
    let _account_guard = WindowsSandboxAccountTestGuard::acquire()?;
    let codex_home =
        codex_home_for_windows_sandbox_test("windows-elevated-tool-runtime-deny-read-codex-home")?;
    let _codex_home_guard = EnvVarGuard::set("CODEX_HOME", codex_home.path().as_os_str());
    stage_windows_sandbox_helpers()?;

    let configured_codex_home = dunce::canonicalize(codex_home.path())?.abs();
    let builder = test_codex()
        .with_windows_cmd_shell()
        .with_config(move |config| {
            config.codex_home = configured_codex_home;
            config.set_windows_elevated_sandbox_enabled(true);
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("test config should allow unified exec");

            let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    access: FileSystemAccessMode::Read,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                    },
                    access: FileSystemAccessMode::Write,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::GlobPattern {
                        pattern: "**/*.env".to_string(),
                    },
                    access: FileSystemAccessMode::Deny,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: config.cwd.join("exact-secret.txt").into(),
                    },
                    access: FileSystemAccessMode::Deny,
                    missing_path_behavior: None,
                },
            ]);
            config
                .permissions
                .set_permission_profile(
                    PermissionProfile::from_runtime_permissions(
                        &file_system_sandbox_policy,
                        NetworkSandboxPolicy::Restricted,
                    )
                    .materialize_project_roots_with_workspace_roots(
                        std::slice::from_ref(&config.cwd),
                    ),
                )
                .expect("set managed deny-read permission profile");
        })
        .with_workspace_setup(|cwd, _fs| async move {
            let nested = cwd.join("nested");
            std::fs::create_dir_all(&nested)?;
            for index in 0..1_000 {
                std::fs::write(nested.join(format!("bulk-{index}.env")), "bulk secret\n")?;
            }
            std::fs::write(
                cwd.join("AGENTS.md"),
                "Preserve the large payload integration fixture.\n",
            )?;
            std::fs::write(
                cwd.join("secret.env"),
                "glob secret should remain private\n",
            )?;
            std::fs::write(
                cwd.join("exact-secret.txt"),
                "exact secret should remain private\n",
            )?;
            std::fs::write(cwd.join("public.txt"), "public ok\n")?;
            Ok(())
        });
    let harness = TestCodexHarness::with_builder(builder).await?;

    let config = &harness.test().config;
    let paths = codex_windows_sandbox::resolve_windows_deny_read_paths(
        &config.permissions.file_system_sandbox_policy(),
        &config.cwd,
    )
    .map_err(anyhow::Error::msg)?;
    assert!(
        paths.len() >= 1_002,
        "recursive deny fixture was not fully expanded"
    );
    // The deny list alone exceeds CreateProcessW's limit, so both wrapper and
    // setup-refresh must transport the full request outside argv.
    assert!(serde_json::to_string(&paths)?.encode_utf16().count() > 32_767);

    let command = concat!(
        "(type secret.env 1>NUL 2>NUL && echo GLOB-READ || echo GLOB-DENIED) & ",
        "(type exact-secret.txt 1>NUL 2>NUL && echo EXACT-READ || echo EXACT-DENIED) & ",
        "(type nested\\bulk-999.env 1>NUL 2>NUL && echo BULK-READ || echo BULK-DENIED) & ",
        "type public.txt"
    );
    let call_id = "windows-managed-deny-read-exec-command";
    let unified_args = json!({
        "cmd": command,
        "yield_time_ms": 30_000,
        "tty": false,
        "login": false,
    });
    mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![
                ev_response_created("resp-windows-unified-deny-read"),
                ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&unified_args)?,
                ),
                ev_completed("resp-windows-unified-deny-read"),
            ]),
            sse(vec![
                ev_assistant_message("msg-windows-deny-read", "done"),
                ev_completed("resp-windows-deny-read-complete"),
            ]),
        ],
    )
    .await;

    let permission_profile = harness
        .test()
        .config
        .permissions
        .effective_permission_profile();
    harness
        .submit_with_permission_profile("read the sandbox fixtures", permission_profile)
        .await?;

    let output = harness.function_call_stdout(call_id).await;
    assert!(
        harness.request_bodies().await.iter().any(|body| body
            .to_string()
            .contains("Preserve the large payload integration fixture.")),
        "sandboxed AGENTS.md discovery must load the fixture instructions"
    );
    assert!(
        output.contains("BULK-DENIED")
            && !output.contains("BULK-READ")
            && !output.contains("bulk secret"),
        "large recursive deny list must remain enforced: {output:?}"
    );
    assert!(
        output.contains("GLOB-DENIED"),
        "exec_command should reject glob-denied reads: {output:?}"
    );
    assert!(
        output.contains("EXACT-DENIED"),
        "exec_command should reject exact-path-denied reads: {output:?}"
    );
    assert!(
        output.contains("public ok"),
        "exec_command should preserve allowed reads: {output:?}"
    );
    assert!(
        !output.contains("GLOB-READ") && !output.contains("glob secret"),
        "exec_command leaked glob-denied file contents: {output:?}"
    );
    assert!(
        !output.contains("EXACT-READ") && !output.contains("exact secret"),
        "exec_command leaked exact-path-denied file contents: {output:?}"
    );

    // Re-enter real setup after corrupting this test's bookkeeping, then exercise
    // recovery through the same agent tool path rather than relying on a cached launch.
    let state_path = codex_home.path().join(".sandbox/deny_read_acl_state.json");
    std::fs::write(&state_path, b"malformed bookkeeping")?;
    let permission_profile = harness
        .test()
        .config
        .permissions
        .effective_permission_profile();
    run_windows_sandbox_setup(WindowsSandboxSetupRequest {
        mode: WindowsSandboxSetupMode::Elevated,
        permission_profile: permission_profile.clone(),
        workspace_roots: vec![harness.test().config.cwd.clone()],
        command_cwd: harness.test().cwd_path().to_path_buf(),
        env_map: std::env::vars().collect(),
        codex_home: codex_home.path().to_path_buf(),
    })
    .await?;
    let rebuilt: serde_json::Value = serde_json::from_slice(&std::fs::read(&state_path)?)?;
    let principals = rebuilt["principals"]
        .as_object()
        .expect("rebuilt principals");
    assert!(
        principals
            .values()
            .any(|paths| paths.as_array().is_some_and(|paths| !paths.is_empty()))
    );

    std::fs::write(
        harness.test().workspace_path("remove-after-recovery.txt"),
        b"must be removed by the sandboxed command",
    )?;
    let recovery_call_id = "windows-recovered-deny-read-exec-command";
    let recovery_args = json!({
        "cmd": concat!(
            "(echo edited-after-recovery)>public.txt & ",
            "del remove-after-recovery.txt & ",
            "(type secret.env 1>NUL 2>NUL && echo GLOB-READ || echo GLOB-DENIED) & ",
            "(type exact-secret.txt 1>NUL 2>NUL && echo EXACT-READ || echo EXACT-DENIED)"
        ),
        "yield_time_ms": 30_000,
        "tty": false,
        "login": false,
    });
    mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![
                ev_response_created("resp-windows-recovered"),
                ev_function_call(
                    recovery_call_id,
                    "exec_command",
                    &serde_json::to_string(&recovery_args)?,
                ),
                ev_completed("resp-windows-recovered"),
            ]),
            sse(vec![
                ev_assistant_message("msg-windows-recovered", "done"),
                ev_completed("resp-windows-recovered-complete"),
            ]),
        ],
    )
    .await;
    harness
        .submit_with_permission_profile(
            "edit and delete the allowed fixtures and try the denied reads",
            permission_profile,
        )
        .await?;
    let output = harness.function_call_stdout(recovery_call_id).await;
    assert!(
        output.contains("GLOB-DENIED") && output.contains("EXACT-DENIED"),
        "denies must survive recovery: {output:?}"
    );
    assert!(
        !output.contains("GLOB-READ") && !output.contains("EXACT-READ"),
        "recovery must not allow denied reads: {output:?}"
    );
    assert_eq!(
        std::fs::read(harness.test().workspace_path("public.txt"))?,
        b"edited-after-recovery\r\n"
    );
    assert!(
        !harness
            .test()
            .workspace_path("remove-after-recovery.txt")
            .exists()
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(codex_home)]
async fn windows_elevated_approved_git_pull_preserves_deny_read() -> anyhow::Result<()> {
    use EventMsg::ExecApprovalRequest;
    use EventMsg::ExecCommandEnd;
    use EventMsg::TurnComplete;

    let _account_guard = WindowsSandboxAccountTestGuard::acquire()?;
    let codex_home = codex_home_for_windows_sandbox_test("windows-elevated-git-pull-deny-read")?;
    let _codex_home_guard = EnvVarGuard::set("CODEX_HOME", codex_home.path().as_os_str());
    stage_windows_sandbox_helpers()?;
    let home = dunce::canonicalize(codex_home.path())?.abs();
    let builder = test_codex()
        .with_windows_cmd_shell()
        .with_config(move |config| {
            config.codex_home = home;
            config.set_windows_elevated_sandbox_enabled(true);
            config.prefer_mxc = false;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::User;
            config.features.enable(Feature::UnifiedExec).unwrap();
            let mut filesystem = FileSystemSandboxPolicy::workspace_write(
                &[],
                /*exclude_tmpdir_env_var*/ true,
                /*exclude_slash_tmp*/ true,
            );
            filesystem.entries.extend(
                [
                    config.cwd.join("exact-secret.txt").into(),
                    FileSystemPath::GlobPattern {
                        pattern: "**/*.env".into(),
                    },
                ]
                .map(|path| FileSystemSandboxEntry::new(path, FileSystemAccessMode::Deny)),
            );
            config
                .permissions
                .set_permission_profile(PermissionProfile::from_runtime_permissions(
                    &filesystem,
                    NetworkSandboxPolicy::Restricted,
                ))
                .unwrap();
        })
        .with_workspace_setup(|cwd, _fs| async move {
            std::fs::write(cwd.join("exact-secret.txt"), "exact secret")?;
            std::fs::write(cwd.join("secret.env"), "glob secret")?;
            Ok(())
        });
    let harness = TestCodexHarness::with_builder(builder).await?;
    let git = |cwd: &Path, args: &[&str]| -> anyhow::Result<String> {
        let output = Command::new("git")
            .args(["-c", "commit.gpgsign=false"])
            .current_dir(cwd)
            .args(args)
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    };
    let upstream = TempDir::new()?;
    let upstream = upstream.path();
    let upstream_path = upstream.to_str().context("UTF-8 upstream path")?;
    let cwd = harness.cwd();
    let read = |name| std::fs::read_to_string(cwd.join(name));
    let init = ["init", "--initial-branch=main"];
    let commit = ["commit", "-am", "fixture"];
    git(upstream, &init)?;
    git(upstream, &["config", "user.name", "Codex Test"])?;
    git(upstream, &["config", "user.email", "codex@test.com"])?;
    std::fs::write(upstream.join("tracked.txt"), "before\n")?;
    git(upstream, &["add", "tracked.txt"])?;
    git(upstream, &commit)?;
    git(cwd, &init)?;
    git(cwd, &["pull", "-q", "--ff-only", upstream_path, "main"])?;
    let fetch_head = cwd.join(".git/FETCH_HEAD");
    std::fs::remove_file(&fetch_head)?;
    std::fs::write(upstream.join("tracked.txt"), "after\n")?;
    git(upstream, &commit)?;
    let expected = git(upstream, &["rev-parse", "HEAD"])?;
    let bundle = cwd.join("upstream.bundle");
    let bundle_path = bundle.to_str().context("UTF-8 bundle path")?;
    git(upstream, &["bundle", "create", bundle_path, "--all"])?;

    let args = json!({"cmd": concat!(
        "git pull --quiet --ff-only .\\upstream.bundle main || exit /b 10 & ",
        "type tracked.txt >NUL || exit /b 11 & ",
        "type exact-secret.txt >NUL 2>NUL && exit /b 12 & ",
        "type secret.env >NUL 2>NUL && exit /b 13 & ",
        "echo git-and-denials-ok>git-and-denials-ok.txt || exit /b 14 & exit /b 0"
    ), "sandbox_permissions": "require_escalated",
        "justification": "Can I pull the fixture repository?",
        "yield_time_ms": 30_000, "tty": false, "login": false,
    });
    let call_id = "git-pull";
    core_test_support::responses::mount_function_call_agent_response(
        harness.server(),
        call_id,
        &args.to_string(),
        "exec_command",
    )
    .await;
    let codex = &harness.test().codex;
    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "pull the fixture repository".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let approval = wait_for_event(codex, |event| {
        matches!(event, ExecApprovalRequest(_) | TurnComplete(_))
    })
    .await;
    let ExecApprovalRequest(approval) = approval else {
        anyhow::bail!("expected approval before git ran: {approval:?}");
    };
    assert_eq!(approval.call_id, call_id);
    assert!(!fetch_head.exists());
    assert!(!cwd.join("git-and-denials-ok.txt").exists());
    assert_eq!(read("tracked.txt")?.trim(), "before");
    codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::Approved,
        })
        .await?;
    let mut process_end = None;
    let mut turn_completed = false;
    while process_end.is_none() || !turn_completed {
        match wait_for_event_with_timeout(codex, |_| true, Duration::from_secs(120)).await {
            ExecCommandEnd(event) if event.call_id == call_id => process_end = Some(event),
            TurnComplete(_) => turn_completed = true,
            ExecApprovalRequest(request) => {
                anyhow::bail!("unexpected additional Git approval: {request:?}")
            }
            _ => {}
        }
    }
    let result = process_end.expect("approved Git process should finish");
    let output = &result.aggregated_output;
    assert_eq!(result.exit_code, 0, "sandbox output: {output}");
    assert_eq!(read("git-and-denials-ok.txt")?.trim(), "git-and-denials-ok");
    assert!(read(".git/FETCH_HEAD")?.starts_with(&expected));
    assert_eq!(read("tracked.txt")?.trim(), "after");
    assert_eq!(read("exact-secret.txt")?, "exact secret");
    assert_eq!(read("secret.env")?, "glob secret");
    Ok(())
}
