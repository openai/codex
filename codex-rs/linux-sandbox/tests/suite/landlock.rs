#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used)]
use codex_core::exec::ExecCapturePolicy;
use codex_core::exec::ExecParams;
use codex_core::exec::process_exec_tool_call;
use codex_core::exec_env::create_env;
use codex_core::sandboxing::SandboxPermissions;
use codex_protocol::config_types::ShellEnvironmentPolicy;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::Result;
use codex_protocol::error::SandboxErr;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Output;
use std::time::Duration;
use tempfile::NamedTempFile;

#[path = "wslg_tests.rs"]
mod wslg_tests;

// At least on GitHub CI, the arm64 tests appear to need longer timeouts.

#[cfg(not(target_arch = "aarch64"))]
const SHORT_TIMEOUT_MS: u64 = 5_000;
#[cfg(target_arch = "aarch64")]
const SHORT_TIMEOUT_MS: u64 = 5_000;

#[cfg(not(target_arch = "aarch64"))]
const LONG_TIMEOUT_MS: u64 = 5_000;
#[cfg(target_arch = "aarch64")]
const LONG_TIMEOUT_MS: u64 = 5_000;

#[cfg(not(target_arch = "aarch64"))]
const NETWORK_TIMEOUT_MS: u64 = 10_000;
#[cfg(target_arch = "aarch64")]
const NETWORK_TIMEOUT_MS: u64 = 10_000;

const BWRAP_UNAVAILABLE_ERR: &str = "bubblewrap is unavailable: no system bwrap was found";

fn create_env_from_core_vars() -> HashMap<String, String> {
    let policy = ShellEnvironmentPolicy::default();
    create_env(&policy, /*thread_id*/ None)
}

fn codex_linux_sandbox_exe() -> PathBuf {
    let sandbox_program = PathBuf::from(env!("CARGO_BIN_EXE_codex-linux-sandbox"));
    match sandbox_program.canonicalize() {
        Ok(path) => path,
        Err(_) => sandbox_program,
    }
}

#[expect(clippy::print_stdout)]
async fn run_cmd(cmd: &[&str], writable_roots: &[PathBuf], timeout_ms: u64) {
    let output = run_cmd_output(cmd, writable_roots, timeout_ms).await;
    if output.exit_code != 0 {
        println!("stdout:\n{}", output.stdout.text);
        println!("stderr:\n{}", output.stderr.text);
        panic!("exit code: {}", output.exit_code);
    }
}

async fn run_cmd_output(
    cmd: &[&str],
    writable_roots: &[PathBuf],
    timeout_ms: u64,
) -> codex_protocol::exec_output::ExecToolCallOutput {
    run_cmd_result_with_writable_roots(
        cmd,
        writable_roots,
        timeout_ms,
        /*use_legacy_landlock*/ false,
        /*network_access*/ false,
    )
    .await
    .expect("sandboxed command should execute")
}

async fn run_cmd_result_with_writable_roots(
    cmd: &[&str],
    writable_roots: &[PathBuf],
    timeout_ms: u64,
    use_legacy_landlock: bool,
    network_access: bool,
) -> Result<codex_protocol::exec_output::ExecToolCallOutput> {
    let writable_roots = writable_roots
        .iter()
        .map(|path| AbsolutePathBuf::try_from(path.as_path()).unwrap())
        .collect::<Vec<_>>();
    let permission_profile = PermissionProfile::workspace_write_with(
        &writable_roots,
        if network_access {
            NetworkSandboxPolicy::Enabled
        } else {
            NetworkSandboxPolicy::Restricted
        },
        // Exclude tmp-related folders from writable roots because we need a
        // folder that is writable by tests but that we intentionally disallow
        // writing to in the sandbox.
        /*exclude_tmpdir_env_var*/
        true,
        /*exclude_slash_tmp*/ true,
    );
    run_cmd_result_with_permission_profile(cmd, permission_profile, timeout_ms, use_legacy_landlock)
        .await
}

async fn run_cmd_result_with_permission_profile(
    cmd: &[&str],
    permission_profile: PermissionProfile,
    timeout_ms: u64,
    use_legacy_landlock: bool,
) -> Result<codex_protocol::exec_output::ExecToolCallOutput> {
    let cwd = AbsolutePathBuf::current_dir().expect("cwd should exist");
    run_cmd_result_with_permission_profile_for_cwd(
        cmd,
        cwd,
        permission_profile,
        create_env_from_core_vars(),
        timeout_ms,
        use_legacy_landlock,
    )
    .await
}

async fn run_cmd_result_with_cwd_and_writable_roots(
    cmd: &[&str],
    cwd: &std::path::Path,
    writable_roots: &[PathBuf],
    timeout_ms: u64,
    use_legacy_landlock: bool,
    network_access: bool,
) -> Result<codex_protocol::exec_output::ExecToolCallOutput> {
    let writable_roots = writable_roots
        .iter()
        .map(|path| AbsolutePathBuf::try_from(path.as_path()).unwrap())
        .collect::<Vec<_>>();
    let permission_profile = PermissionProfile::workspace_write_with(
        &writable_roots,
        if network_access {
            NetworkSandboxPolicy::Enabled
        } else {
            NetworkSandboxPolicy::Restricted
        },
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
    );
    let cwd = AbsolutePathBuf::try_from(cwd).expect("cwd should be absolute");
    run_cmd_result_with_permission_profile_for_cwd(
        cmd,
        cwd,
        permission_profile,
        create_env_from_core_vars(),
        timeout_ms,
        use_legacy_landlock,
    )
    .await
}

async fn run_cmd_result_with_permission_profile_for_cwd(
    cmd: &[&str],
    cwd: AbsolutePathBuf,
    permission_profile: PermissionProfile,
    env: HashMap<String, String>,
    timeout_ms: u64,
    use_legacy_landlock: bool,
) -> Result<codex_protocol::exec_output::ExecToolCallOutput> {
    let sandbox_cwd = cwd.clone();
    let params = ExecParams {
        command: cmd.iter().copied().map(str::to_owned).collect(),
        cwd,
        expiration: timeout_ms.into(),
        capture_policy: ExecCapturePolicy::ShellTool,
        env,
        network: None,
        network_environment_id: None,
        sandbox_permissions: SandboxPermissions::UseDefault,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        windows_sandbox_private_desktop: false,
        justification: None,
        arg0: None,
    };
    let codex_linux_sandbox_exe = Some(codex_linux_sandbox_exe());

    process_exec_tool_call(
        params,
        &permission_profile,
        &sandbox_cwd,
        std::slice::from_ref(&sandbox_cwd),
        &codex_linux_sandbox_exe,
        /*codex_self_exe*/ &None,
        use_legacy_landlock,
        /*stdout_stream*/ None,
    )
    .await
}

fn is_bwrap_unavailable_output(output: &codex_protocol::exec_output::ExecToolCallOutput) -> bool {
    output.stderr.text.contains(BWRAP_UNAVAILABLE_ERR)
        || (output
            .stderr
            .text
            .contains("Can't mount proc on /newroot/proc")
            && (output.stderr.text.contains("Operation not permitted")
                || output.stderr.text.contains("Permission denied")
                || output.stderr.text.contains("Invalid argument")))
}

async fn should_skip_bwrap_tests() -> bool {
    match run_cmd_result_with_writable_roots(
        &["bash", "-lc", "true"],
        &[],
        NETWORK_TIMEOUT_MS,
        /*use_legacy_landlock*/ false,
        /*network_access*/ true,
    )
    .await
    {
        Ok(output) => is_bwrap_unavailable_output(&output),
        Err(err) => match err.details() {
            CodexErrorDetails::Sandbox(SandboxErr::Denied { output, .. }) => {
                is_bwrap_unavailable_output(output)
            }
            // Probe timeouts are not actionable for the bwrap-specific assertions below;
            // skip rather than fail the whole suite.
            CodexErrorDetails::Sandbox(SandboxErr::Timeout { .. }) => true,
            details => panic!("bwrap availability probe failed unexpectedly: {details:?}"),
        },
    }
}

fn expect_denied(
    result: Result<codex_protocol::exec_output::ExecToolCallOutput>,
    context: &str,
) -> codex_protocol::exec_output::ExecToolCallOutput {
    match result {
        Ok(output) => {
            assert_ne!(output.exit_code, 0, "{context}: expected nonzero exit code");
            output
        }
        Err(err) => match err.details() {
            CodexErrorDetails::Sandbox(SandboxErr::Denied { output, .. }) => {
                output.as_ref().clone()
            }
            details => panic!("{context}: {details:?}"),
        },
    }
}

async fn wsl_windows_executable(name: &str) -> Option<String> {
    if !std::path::Path::new("/run/WSL").is_dir() {
        return None;
    }

    let windows_path = format!(r"C:\Windows\System32\{name}");
    let output = tokio::time::timeout(
        Duration::from_millis(NETWORK_TIMEOUT_MS),
        tokio::process::Command::new("wslpath")
            .args(["-u", &windows_path])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!path.is_empty()).then_some(path)
}

async fn wsl_baseline_output(executable: &str, args: &[&str]) -> Option<Output> {
    tokio::time::timeout(
        Duration::from_millis(NETWORK_TIMEOUT_MS),
        tokio::process::Command::new(executable)
            .args(args)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()
    .filter(|output| output.status.success())
}

#[tokio::test]
async fn wsl_interop_cannot_run_windows_program_with_network_access() {
    let Some(powershell) = wsl_windows_executable(r"WindowsPowerShell\v1.0\powershell.exe").await
    else {
        return;
    };
    let args = [
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "Write-Output interop-escaped",
    ];
    if wsl_baseline_output(&powershell, &args).await.is_none() {
        eprintln!("skipping WSL interop test: Windows interop is unavailable on the host");
        return;
    }
    if should_skip_bwrap_tests().await {
        eprintln!("skipping WSL interop test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    let output = expect_denied(
        run_cmd_result_with_writable_roots(
            &[&powershell, args[0], args[1], args[2], args[3]],
            &[],
            NETWORK_TIMEOUT_MS,
            /*use_legacy_landlock*/ false,
            /*network_access*/ true,
        )
        .await,
        "WSL interop must not start a Windows program inside a restricted filesystem sandbox",
    );
    assert!(!output.stdout.text.contains("interop-escaped"));
}

#[tokio::test]
async fn wsl_interop_bind_alias_cannot_run_windows_program() {
    let Some(powershell) = wsl_windows_executable(r"WindowsPowerShell\v1.0\powershell.exe").await
    else {
        return;
    };
    if wsl_baseline_output(
        &powershell,
        &["-NoProfile", "-NonInteractive", "-Command", "exit 0"],
    )
    .await
    .is_none()
        || should_skip_bwrap_tests().await
        || !std::path::Path::new("/usr/bin/unshare").exists()
        || !std::path::Path::new("/usr/bin/mount").exists()
    {
        return;
    }
    let Ok(interop_socket) = std::env::var("WSL_INTEROP") else {
        eprintln!("skipping WSL bind alias test: WSL_INTEROP is unavailable");
        return;
    };
    if !std::path::Path::new(&interop_socket).exists() {
        eprintln!("skipping WSL bind alias test: interop socket is unavailable");
        return;
    }

    let mount_dir = tempfile::tempdir().expect("create mount target");
    let alias = mount_dir.path().join("WSL");
    std::fs::create_dir(&alias).expect("create interop bind target");
    let namespace_probe = tokio::time::timeout(
        Duration::from_millis(NETWORK_TIMEOUT_MS),
        tokio::process::Command::new("unshare")
            .args([
                "--user",
                "--map-root-user",
                "--mount",
                "--propagation",
                "private",
                "--",
                "/bin/sh",
                "-c",
                "mount --bind /run/WSL \"$1\" && umount \"$1\"",
                "sh",
            ])
            .arg(&alias)
            .kill_on_drop(true)
            .output(),
    )
    .await;
    let Ok(Ok(probe)) = namespace_probe else {
        eprintln!("skipping WSL bind alias test: user/mount namespace probe could not run");
        return;
    };
    if !probe.status.success() {
        eprintln!(
            "skipping WSL bind alias test: user/mount namespace unavailable: {}",
            String::from_utf8_lossy(&probe.stderr)
        );
        return;
    }

    let profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::read_only(),
        NetworkSandboxPolicy::Enabled,
    );
    let profile = serde_json::to_string(&profile).expect("serialize permission profile");
    let cwd = std::env::current_dir().expect("current directory");
    let script = r#"
        mount --bind /run/WSL "$1"
        trap 'umount "$1"' EXIT
        export WSL_INTEROP="$1/${WSL_INTEROP##*/}"
        "$2" -NoProfile -NonInteractive -Command 'Write-Output alias-baseline-ok'
        bash -c 'exec -a "$1" /init "$1" -NoProfile -NonInteractive -Command "Write-Output direct-init-baseline-ok"' bash "$2"
        "$3" --sandbox-policy-cwd "$4" --permission-profile "$5" -- \
            "$2" -NoProfile -NonInteractive -Command 'Write-Output interop-escaped'
        status=$?
        printf 'sandbox-exit=%s\n' "$status"
        if test "$status" -eq 0; then exit 1; fi
        "$3" --sandbox-policy-cwd "$4" --permission-profile "$5" -- \
            bash -c 'exec -a "$1" /init "$1" -NoProfile -NonInteractive -Command "Write-Output direct-init-escaped"' bash "$2"
        status=$?
        printf 'direct-init-exit=%s\n' "$status"
        test "$status" -ne 0
    "#;
    let output = tokio::time::timeout(
        Duration::from_millis(NETWORK_TIMEOUT_MS * 3),
        tokio::process::Command::new("unshare")
            .args([
                "--user",
                "--map-root-user",
                "--mount",
                "--propagation",
                "private",
                "--",
            ])
            .args(["/bin/sh", "-c", script, "sh"])
            .arg(&alias)
            .arg(&powershell)
            .arg(codex_linux_sandbox_exe())
            .arg(&cwd)
            .arg(profile)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .expect("WSL bind alias test should finish")
    .expect("unshare should start");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "WSL bind alias must be isolated: stdout={stdout} stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("alias-baseline-ok"), "stdout={stdout}");
    assert!(
        stdout.contains("direct-init-baseline-ok"),
        "stdout={stdout}"
    );
    assert!(stdout.contains("sandbox-exit="), "stdout={stdout}");
    assert!(stdout.contains("direct-init-exit="), "stdout={stdout}");
    assert!(!stdout.contains("interop-escaped"), "stdout={stdout}");
    assert!(!stdout.contains("direct-init-escaped"), "stdout={stdout}");
}

#[tokio::test]
async fn wsl_no_proc_masks_inherited_procfs_and_windows_interop() {
    let Some(powershell) = wsl_windows_executable(r"WindowsPowerShell\v1.0\powershell.exe").await
    else {
        return;
    };
    let args = [
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "Write-Output interop-escaped",
    ];
    if wsl_baseline_output(&powershell, &args).await.is_none() || should_skip_bwrap_tests().await {
        return;
    }

    let profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::read_only(),
        NetworkSandboxPolicy::Enabled,
    );
    let profile = serde_json::to_string(&profile).expect("serialize permission profile");
    let cwd = std::env::current_dir().expect("current directory");
    let proc_check = tokio::time::timeout(
        Duration::from_millis(NETWORK_TIMEOUT_MS),
        tokio::process::Command::new(codex_linux_sandbox_exe())
            .arg("--sandbox-policy-cwd")
            .arg(&cwd)
            .args([
                "--permission-profile",
                &profile,
                "--no-proc",
                "--",
                "/bin/sh",
                "-c",
                "test ! -e /proc/1",
            ])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .expect("procfs check should finish")
    .expect("sandbox helper should start");
    assert!(
        proc_check.status.success(),
        "inherited procfs was not masked: {}",
        String::from_utf8_lossy(&proc_check.stderr)
    );

    let output = tokio::time::timeout(
        Duration::from_millis(NETWORK_TIMEOUT_MS),
        tokio::process::Command::new(codex_linux_sandbox_exe())
            .arg("--sandbox-policy-cwd")
            .arg(&cwd)
            .args([
                "--permission-profile",
                &profile,
                "--no-proc",
                "--",
                &powershell,
            ])
            .args(args)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .expect("sandbox command should finish")
    .expect("sandbox helper should start");
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("interop-escaped"));
}

#[tokio::test]
async fn wsl_interop_cannot_reenter_distro_as_root_with_network_access() {
    let Some(wsl) = wsl_windows_executable("wsl.exe").await else {
        return;
    };
    let Ok(distro) = std::env::var("WSL_DISTRO_NAME") else {
        eprintln!("skipping WSL root test: distro name is unavailable");
        return;
    };
    let args = ["-d", &distro, "-u", "root", "--", "/usr/bin/id", "-u"];
    let Some(baseline) = wsl_baseline_output(&wsl, &args).await else {
        eprintln!("skipping WSL root test: Windows interop is unavailable on the host");
        return;
    };
    assert_eq!(baseline.stdout, b"0\n");
    if should_skip_bwrap_tests().await {
        eprintln!("skipping WSL root test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    let output = expect_denied(
        run_cmd_result_with_writable_roots(
            &[
                &wsl, args[0], args[1], args[2], args[3], args[4], args[5], args[6],
            ],
            &[],
            NETWORK_TIMEOUT_MS,
            /*use_legacy_landlock*/ false,
            /*network_access*/ true,
        )
        .await,
        "WSL interop must not start a new root session inside a restricted filesystem sandbox",
    );
    assert_ne!(output.stdout.text.trim(), "0");
}

#[tokio::test]
async fn test_root_read() {
    run_cmd(&["ls", "-l", "/bin"], &[], SHORT_TIMEOUT_MS).await;
}

#[tokio::test]
#[should_panic]
async fn test_root_write() {
    let tmpfile = NamedTempFile::new().unwrap();
    let tmpfile_path = tmpfile.path().to_string_lossy();
    run_cmd(
        &["bash", "-lc", &format!("echo blah > {tmpfile_path}")],
        &[],
        SHORT_TIMEOUT_MS,
    )
    .await;
}

#[tokio::test]
async fn test_dev_null_write() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    let output = run_cmd_result_with_writable_roots(
        &["bash", "-lc", "echo blah > /dev/null"],
        &[],
        // We have seen timeouts when running this test in CI on GitHub,
        // so we are using a generous timeout until we can diagnose further.
        LONG_TIMEOUT_MS,
        /*use_legacy_landlock*/ false,
        /*network_access*/ true,
    )
    .await
    .expect("sandboxed command should execute");

    assert_eq!(output.exit_code, 0);
}

#[tokio::test]
async fn bwrap_populates_minimal_dev_nodes() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    let output = run_cmd_result_with_writable_roots(
        &[
            "bash",
            "-lc",
            "for node in null zero full random urandom tty; do [ -c \"/dev/$node\" ] || { echo \"missing /dev/$node\" >&2; exit 1; }; done",
        ],
        &[],
        LONG_TIMEOUT_MS,
        /*use_legacy_landlock*/ false,
        /*network_access*/ true,
    )
    .await
    .expect("sandboxed command should execute");

    assert_eq!(output.exit_code, 0);
}

#[tokio::test]
async fn bwrap_preserves_writable_dev_shm_bind_mount() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }
    if !std::path::Path::new("/dev/shm").exists() {
        eprintln!("skipping bwrap test: /dev/shm is unavailable in this environment");
        return;
    }

    let target_file = match NamedTempFile::new_in("/dev/shm") {
        Ok(file) => file,
        Err(err) => {
            eprintln!("skipping bwrap test: failed to create /dev/shm temp file: {err}");
            return;
        }
    };
    let target_path = target_file.path().to_path_buf();
    std::fs::write(&target_path, "host-before").expect("seed /dev/shm file");

    let output = run_cmd_result_with_writable_roots(
        &[
            "bash",
            "-lc",
            &format!("printf sandbox-after > {}", target_path.to_string_lossy()),
        ],
        &[PathBuf::from("/dev/shm")],
        LONG_TIMEOUT_MS,
        /*use_legacy_landlock*/ false,
        /*network_access*/ true,
    )
    .await
    .expect("sandboxed command should execute");

    assert_eq!(output.exit_code, 0);
    assert_eq!(
        std::fs::read_to_string(&target_path).expect("read /dev/shm file"),
        "sandbox-after"
    );
}

#[tokio::test]
async fn test_writable_root() {
    let tmpdir = tempfile::tempdir().unwrap();
    let file_path = tmpdir.path().join("test");
    run_cmd(
        &[
            "bash",
            "-lc",
            &format!("echo blah > {}", file_path.to_string_lossy()),
        ],
        &[tmpdir.path().to_path_buf()],
        // We have seen timeouts when running this test in CI on GitHub,
        // so we are using a generous timeout until we can diagnose further.
        LONG_TIMEOUT_MS,
    )
    .await;
}

#[tokio::test]
async fn sandbox_ignores_missing_writable_roots_under_bwrap() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    let tempdir = tempfile::tempdir().expect("tempdir");
    let existing_root = tempdir.path().join("existing");
    let missing_root = tempdir.path().join("missing");
    std::fs::create_dir(&existing_root).expect("create existing root");

    let output = run_cmd_result_with_writable_roots(
        &["bash", "-lc", "printf sandbox-ok"],
        &[existing_root, missing_root],
        LONG_TIMEOUT_MS,
        /*use_legacy_landlock*/ false,
        /*network_access*/ true,
    )
    .await
    .expect("sandboxed command should execute");

    assert_eq!(output.exit_code, 0);
    assert_eq!(output.stdout.text, "sandbox-ok");
}

#[tokio::test]
async fn test_no_new_privs_is_enabled() {
    let output = run_cmd_output(
        &[
            "python3",
            "-c",
            "import ctypes; print(ctypes.CDLL(None).prctl(39, 0, 0, 0, 0))",
        ],
        &[],
        // We have seen timeouts when running this test in CI on GitHub,
        // so we are using a generous timeout until we can diagnose further.
        LONG_TIMEOUT_MS,
    )
    .await;
    assert_eq!(output.exit_code, 0);
    assert_eq!(output.stdout.text, "1\n");
}

#[tokio::test]
async fn sandboxed_command_has_no_effective_or_permitted_capabilities() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    let output = run_cmd_output(
        &[
            "python3",
            "-c",
            "import ctypes; h=(ctypes.c_uint*2)(0x20080522,0); d=(ctypes.c_uint*6)(); assert ctypes.CDLL(None).capget(h,d)==0; print(d[0],d[1],d[3],d[4])",
        ],
        &[],
        LONG_TIMEOUT_MS,
    )
    .await;
    assert_eq!(output.exit_code, 0);
    assert_eq!(output.stdout.text, "0 0 0 0\n");
}

#[tokio::test]
async fn sandbox_inner_stage_rejects_retained_capabilities() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    let user_namespace_probe = match std::process::Command::new("unshare")
        .args(["--user", "--map-root-user", "--", "/bin/true"])
        .output()
    {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("skipping capability test: unshare is unavailable");
            return;
        }
        Err(err) => panic!("failed to probe unprivileged user namespaces: {err}"),
    };
    if !user_namespace_probe.status.success() {
        eprintln!("skipping capability test: unprivileged user namespaces are unavailable");
        return;
    }

    let permission_profile = serde_json::to_string(&PermissionProfile::read_only())
        .expect("read-only permission profile should serialize");
    let output = std::process::Command::new("unshare")
        .args(["--user", "--map-root-user", "--"])
        .arg(codex_linux_sandbox_exe())
        .args(["--sandbox-policy-cwd", "/", "--permission-profile"])
        .arg(permission_profile)
        .args([
            "--apply-seccomp-then-exec",
            "--",
            "/bin/sh",
            "-c",
            "printf command-ran",
        ])
        .output()
        .expect("capability-bearing sandbox helper should execute");

    assert!(!output.status.success());
    assert_eq!(output.stdout, Vec::<u8>::new());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Linux sandbox retained effective or permitted capabilities"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
#[should_panic(expected = "Sandbox(Timeout")]
async fn test_timeout() {
    run_cmd(&["sleep", "2"], &[], /*timeout_ms*/ 50).await;
}

/// Helper that runs `cmd` under the Linux sandbox and asserts that the command
/// does NOT succeed (i.e. returns a non‑zero exit code) **unless** the binary
/// is missing in which case we silently treat it as an accepted skip so the
/// suite remains green on leaner CI images.
async fn assert_network_blocked(cmd: &[&str]) {
    let cwd = AbsolutePathBuf::current_dir().expect("cwd should exist");
    let sandbox_cwd = cwd.clone();
    let params = ExecParams {
        command: cmd.iter().copied().map(str::to_owned).collect(),
        cwd,
        // Give the tool a generous 2-second timeout so even slow DNS timeouts
        // do not stall the suite.
        expiration: NETWORK_TIMEOUT_MS.into(),
        capture_policy: ExecCapturePolicy::ShellTool,
        env: create_env_from_core_vars(),
        network: None,
        network_environment_id: None,
        sandbox_permissions: SandboxPermissions::UseDefault,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        windows_sandbox_private_desktop: false,
        justification: None,
        arg0: None,
    };

    let codex_linux_sandbox_exe: Option<PathBuf> = Some(codex_linux_sandbox_exe());
    let permission_profile = PermissionProfile::read_only();
    let result = process_exec_tool_call(
        params,
        &permission_profile,
        &sandbox_cwd,
        std::slice::from_ref(&sandbox_cwd),
        &codex_linux_sandbox_exe,
        /*codex_self_exe*/ &None,
        /*use_legacy_landlock*/ false,
        /*stdout_stream*/ None,
    )
    .await;

    let output = match result {
        Ok(output) => output,
        Err(err) => match err.details() {
            CodexErrorDetails::Sandbox(SandboxErr::Denied { output, .. }) => {
                output.as_ref().clone()
            }
            details => panic!("expected sandbox denied error, got: {details:?}"),
        },
    };

    dbg!(&output.stderr.text);
    dbg!(&output.stdout.text);
    dbg!(&output.exit_code);

    // A completely missing binary exits with 127.  Anything else should also
    // be non‑zero (EPERM from seccomp will usually bubble up as 1, 2, 13…)
    // If—*and only if*—the command exits 0 we consider the sandbox breached.

    if output.exit_code == 0 {
        panic!(
            "Network sandbox FAILED - {cmd:?} exited 0\nstdout:\n{}\nstderr:\n{}",
            output.stdout.text, output.stderr.text
        );
    }
}

#[tokio::test]
async fn sandbox_blocks_curl() {
    assert_network_blocked(&["curl", "-I", "http://openai.com"]).await;
}

#[tokio::test]
async fn sandbox_blocks_wget() {
    assert_network_blocked(&["wget", "-qO-", "http://openai.com"]).await;
}

#[tokio::test]
async fn sandbox_blocks_ping() {
    // ICMP requires raw socket – should be denied quickly with EPERM.
    assert_network_blocked(&["ping", "-c", "1", "8.8.8.8"]).await;
}

#[tokio::test]
async fn sandbox_blocks_nc() {
    // Zero‑length connection attempt to localhost.
    assert_network_blocked(&["nc", "-z", "127.0.0.1", "80"]).await;
}

#[tokio::test]
async fn sandbox_blocks_git_and_codex_writes_inside_writable_root() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    let tmpdir = tempfile::tempdir().expect("tempdir");
    let dot_git = tmpdir.path().join(".git");
    let dot_codex = tmpdir.path().join(".codex");
    std::fs::create_dir_all(&dot_git).expect("create .git");
    std::fs::create_dir_all(&dot_codex).expect("create .codex");

    let git_target = dot_git.join("config");
    let codex_target = dot_codex.join("config.toml");

    let git_output = expect_denied(
        run_cmd_result_with_writable_roots(
            &[
                "bash",
                "-lc",
                &format!("echo denied > {}", git_target.to_string_lossy()),
            ],
            &[tmpdir.path().to_path_buf()],
            LONG_TIMEOUT_MS,
            /*use_legacy_landlock*/ false,
            /*network_access*/ true,
        )
        .await,
        ".git write should be denied under bubblewrap",
    );

    let codex_output = expect_denied(
        run_cmd_result_with_writable_roots(
            &[
                "bash",
                "-lc",
                &format!("echo denied > {}", codex_target.to_string_lossy()),
            ],
            &[tmpdir.path().to_path_buf()],
            LONG_TIMEOUT_MS,
            /*use_legacy_landlock*/ false,
            /*network_access*/ true,
        )
        .await,
        ".codex write should be denied under bubblewrap",
    );
    assert_ne!(git_output.exit_code, 0);
    assert_ne!(codex_output.exit_code, 0);
}

#[tokio::test]
async fn sandbox_blocks_codex_symlink_replacement_attack() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    use std::os::unix::fs::symlink;

    let tmpdir = tempfile::tempdir().expect("tempdir");
    let decoy = tmpdir.path().join("decoy-codex");
    std::fs::create_dir_all(&decoy).expect("create decoy dir");

    let dot_codex = tmpdir.path().join(".codex");
    symlink(&decoy, &dot_codex).expect("create .codex symlink");

    let codex_target = dot_codex.join("config.toml");

    let codex_output = expect_denied(
        run_cmd_result_with_writable_roots(
            &[
                "bash",
                "-lc",
                &format!("echo denied > {}", codex_target.to_string_lossy()),
            ],
            &[tmpdir.path().to_path_buf()],
            LONG_TIMEOUT_MS,
            /*use_legacy_landlock*/ false,
            /*network_access*/ true,
        )
        .await,
        ".codex symlink replacement should be denied",
    );
    assert_ne!(codex_output.exit_code, 0);
}

#[tokio::test]
async fn sandbox_reports_codex_symlink_build_failure_without_panicking() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    use std::os::unix::fs::symlink;

    let tmpdir = tempfile::tempdir().expect("tempdir");
    let decoy = tmpdir.path().join("decoy-codex");
    std::fs::create_dir_all(&decoy).expect("create decoy dir");

    let dot_codex = tmpdir.path().join(".codex");
    symlink(&decoy, &dot_codex).expect("create .codex symlink");

    let output = match run_cmd_result_with_writable_roots(
        &["bash", "-lc", "true"],
        &[tmpdir.path().to_path_buf()],
        LONG_TIMEOUT_MS,
        /*use_legacy_landlock*/ false,
        /*network_access*/ true,
    )
    .await
    {
        Err(err) => match err.details() {
            CodexErrorDetails::Sandbox(SandboxErr::Denied { output, .. }) => {
                output.as_ref().clone()
            }
            details => panic!(".codex symlink build failure should deny: {details:?}"),
        },
        Ok(output) => panic!(".codex symlink build failure should deny: {output:?}"),
    };

    assert_eq!(output.exit_code, 1);
    assert!(
        output
            .stderr
            .text
            .contains("error building bubblewrap command:"),
        "stderr: {}",
        output.stderr.text
    );
    assert!(
        output
            .stderr
            .text
            .contains("cannot enforce sandbox read-only path"),
        "stderr: {}",
        output.stderr.text
    );
    assert!(
        !output.stderr.text.contains("panicked at"),
        "stderr: {}",
        output.stderr.text
    );
}

#[tokio::test]
async fn sandbox_rejects_symlinked_synthetic_mount_registry() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("workspace");
    let registry_target = workspace.join("registry");
    std::fs::create_dir_all(&registry_target).expect("create registry target");
    let effective_uid = unsafe { libc::geteuid() };
    let registry = temp.path().join(format!(
        "codex-bwrap-synthetic-mount-targets-{effective_uid}"
    ));
    std::os::unix::fs::symlink(&registry_target, &registry).expect("symlink registry");

    let cwd = AbsolutePathBuf::try_from(workspace).expect("absolute workspace");
    let permission_profile = PermissionProfile::workspace_write_with(
        std::slice::from_ref(&cwd),
        NetworkSandboxPolicy::Enabled,
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
    );
    let mut env = create_env_from_core_vars();
    env.insert("TMPDIR".to_string(), temp.path().display().to_string());
    let output = expect_denied(
        run_cmd_result_with_permission_profile_for_cwd(
            &["sh", "-c", "touch registry/forged-marker"],
            cwd,
            permission_profile,
            env,
            LONG_TIMEOUT_MS,
            /*use_legacy_landlock*/ false,
        )
        .await,
        "a symlinked registry must not expose writable bookkeeping",
    );
    assert!(
        output
            .stderr
            .text
            .contains("synthetic mount registry must not be a symlink"),
        "stderr: {}",
        output.stderr.text
    );
    assert_eq!(
        std::fs::read_dir(registry_target)
            .expect("read registry target")
            .count(),
        0,
        "the registry symlink must be rejected before registration or command execution"
    );
}

#[tokio::test]
async fn sandbox_keeps_parent_repo_discovery_while_blocking_child_metadata() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    let git_available = std::process::Command::new("git")
        .arg("--version")
        .status()
        .is_ok_and(|status| status.success());
    let python_available = std::process::Command::new("python3")
        .arg("--version")
        .status()
        .is_ok_and(|status| status.success());
    if !git_available || !python_available {
        eprintln!("skipping bwrap test: git or python3 is unavailable");
        return;
    }

    let tmpdir = tempfile::tempdir().expect("tempdir");
    let repo = tmpdir.path().join("repo");
    let subdir = repo.join("sub");
    let real_tmp = tmpdir.path().join("real-tmp");
    let redirected_tmp = tmpdir.path().join("redirected-tmp");
    let tmp_alias = tmpdir.path().join("tmp-alias");
    std::fs::create_dir(&real_tmp).expect("create real temp directory");
    std::fs::create_dir(&redirected_tmp).expect("create redirected temp directory");
    std::os::unix::fs::symlink(&real_tmp, &tmp_alias).expect("create temp directory alias");
    std::fs::create_dir_all(&subdir).expect("create nested workspace");
    assert!(
        std::process::Command::new("git")
            .arg("init")
            .arg("-q")
            .arg(&repo)
            .status()
            .expect("git init should run")
            .success(),
        "git init should create parent repo"
    );

    let repo = repo.to_string_lossy();
    let redirected_tmp = redirected_tmp.to_string_lossy();
    let script = format!(
        r#"set -e
test "$(git rev-parse --show-toplevel)" = '{repo}'
touch "$TMPDIR/writable-sibling"
registry="${{TMPDIR:-/tmp}}/codex-bwrap-synthetic-mount-targets-$(id -u)"
if touch "$registry/forged-marker" 2>/dev/null; then
  exit 22
fi
redirected_registry='{redirected_tmp}'/codex-bwrap-synthetic-mount-targets-$(id -u)
for marker_dir in "$registry"/*; do
  [ -d "$marker_dir" ] || continue
  mkdir -p "$redirected_registry/${{marker_dir##*/}}"
  touch "$redirected_registry/${{marker_dir##*/}}/1"
done
ln -sfn '{redirected_tmp}' "$TMPDIR"
git status --short > status.before
if grep -E '(^|[[:space:]])\.(git|codex|agents)(/|$)' status.before; then
  cat status.before
  exit 21
fi
"#,
    );

    let cwd = AbsolutePathBuf::try_from(subdir.as_path()).expect("cwd should be absolute");
    let permission_profile = PermissionProfile::workspace_write_with(
        std::slice::from_ref(&cwd),
        NetworkSandboxPolicy::Enabled,
        /*exclude_tmpdir_env_var*/ false,
        /*exclude_slash_tmp*/ false,
    );
    let mut env = create_env_from_core_vars();
    env.insert("TMPDIR".to_string(), tmp_alias.display().to_string());
    let output = run_cmd_result_with_permission_profile_for_cwd(
        &["bash", "-lc", &script],
        cwd,
        permission_profile,
        env,
        LONG_TIMEOUT_MS,
        /*use_legacy_landlock*/ false,
    )
    .await
    .expect("sandboxed command should execute");

    assert_eq!(
        output.exit_code, 0,
        "stdout:\n{}\nstderr:\n{}",
        output.stdout.text, output.stderr.text
    );
    assert!(!subdir.join(".git").exists());

    let git_init_output = expect_denied(
        run_cmd_result_with_cwd_and_writable_roots(
            &["git", "init", "-q"],
            &subdir,
            std::slice::from_ref(&subdir),
            LONG_TIMEOUT_MS,
            /*use_legacy_landlock*/ false,
            /*network_access*/ true,
        )
        .await,
        "child git init should be denied",
    );
    assert_ne!(git_init_output.exit_code, 0);
    assert!(!subdir.join(".git").exists());

    let mkdir_codex_output = expect_denied(
        run_cmd_result_with_cwd_and_writable_roots(
            &["mkdir", ".codex"],
            &subdir,
            std::slice::from_ref(&subdir),
            LONG_TIMEOUT_MS,
            /*use_legacy_landlock*/ false,
            /*network_access*/ true,
        )
        .await,
        "child .codex directory creation should be denied",
    );
    assert_ne!(mkdir_codex_output.exit_code, 0);
    assert!(!subdir.join(".codex").exists());

    let script = format!(
        r#"set -e
test "$(git rev-parse --show-toplevel)" = '{repo}'
printf '%s\n' 'import json, sys' 'for line in sys.stdin:' '    obj = json.loads(line)' '    print(obj.get("message", obj))' > jsonl_viewer.py
printf '%s\n' '{{"message":"ok"}}' | python3 jsonl_viewer.py | grep -q ok
"#,
    );
    let output = run_cmd_result_with_cwd_and_writable_roots(
        &["bash", "-lc", &script],
        &subdir,
        std::slice::from_ref(&subdir),
        LONG_TIMEOUT_MS,
        /*use_legacy_landlock*/ false,
        /*network_access*/ true,
    )
    .await
    .expect("sandboxed command should execute");
    assert_eq!(
        output.exit_code, 0,
        "stdout:\n{}\nstderr:\n{}",
        output.stdout.text, output.stderr.text
    );

    assert!(subdir.join("jsonl_viewer.py").is_file());
    assert!(!subdir.join(".git").exists());
    assert!(!subdir.join(".codex").exists());
    assert!(!subdir.join(".agents").exists());
}

#[tokio::test]
async fn sandbox_blocks_explicit_split_policy_carveouts_under_bwrap() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    let tmpdir = tempfile::tempdir().expect("tempdir");
    let blocked = tmpdir.path().join("blocked");
    std::fs::create_dir_all(&blocked).expect("create blocked dir");
    let blocked_target = blocked.join("secret.txt");
    // These tests bypass the usual legacy-policy bridge, so explicitly keep
    // the sandbox helper binary and minimal runtime paths readable.
    let sandbox_helper_dir = codex_linux_sandbox_exe()
        .parent()
        .expect("sandbox helper should have a parent")
        .to_path_buf();

    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Minimal,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: AbsolutePathBuf::try_from(sandbox_helper_dir.as_path())
                    .expect("absolute helper dir")
                    .into(),
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: AbsolutePathBuf::try_from(tmpdir.path())
                    .expect("absolute tempdir")
                    .into(),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: AbsolutePathBuf::try_from(blocked.as_path())
                    .expect("absolute blocked dir")
                    .into(),
            },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Enabled,
    );
    let output = expect_denied(
        run_cmd_result_with_permission_profile(
            &[
                "bash",
                "-lc",
                &format!("echo denied > {}", blocked_target.to_string_lossy()),
            ],
            permission_profile,
            LONG_TIMEOUT_MS,
            /*use_legacy_landlock*/ false,
        )
        .await,
        "explicit split-policy carveout should be denied under bubblewrap",
    );

    assert_ne!(output.exit_code, 0);
}

#[tokio::test]
async fn sandbox_starts_with_denied_tmp_without_exposing_registry() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(temp.path().join("AGENTS.md"), "project instructions\n")
        .expect("write instructions");
    let cwd = AbsolutePathBuf::try_from(temp.path()).expect("absolute workspace");
    let sandbox_helper = codex_linux_sandbox_exe();
    let helper_dir = AbsolutePathBuf::try_from(sandbox_helper.parent().expect("helper parent"))
        .expect("absolute helper directory");

    for tmp_root in [PathBuf::from("/tmp"), temp.path().join("denied-tmp")] {
        std::fs::create_dir_all(&tmp_root).expect("create temp root");
        let secret = NamedTempFile::new_in(&tmp_root).expect("denied file");
        std::fs::write(secret.path(), "private").expect("write denied file");
        for read_root in [FileSystemSpecialPath::Root, FileSystemSpecialPath::Minimal] {
            let denied_path = if tmp_root == std::path::Path::new("/tmp") {
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::SlashTmp,
                }
            } else {
                AbsolutePathBuf::try_from(tmp_root.as_path())
                    .expect("absolute temp root")
                    .into()
            };
            let policy = FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry::new(
                    FileSystemPath::Special { value: read_root },
                    FileSystemAccessMode::Read,
                ),
                FileSystemSandboxEntry::new(helper_dir.clone().into(), FileSystemAccessMode::Read),
                FileSystemSandboxEntry::new(cwd.clone().into(), FileSystemAccessMode::Write),
                FileSystemSandboxEntry::new(denied_path, FileSystemAccessMode::Deny),
            ]);
            let mut env = create_env_from_core_vars();
            env.insert("TMPDIR".to_string(), tmp_root.display().to_string());
            env.insert(
                "DENIED_SECRET".to_string(),
                secret.path().display().to_string(),
            );
            let output = run_cmd_result_with_permission_profile_for_cwd(
                &[
                    "sh",
                    "-c",
                    r#"set -e
cat AGENTS.md
test ! -r "$DENIED_SECRET"
if printf modified > "$DENIED_SECRET" 2>/dev/null; then exit 1; fi
registry="$TMPDIR/codex-bwrap-synthetic-mount-targets-$(id -u)"
test ! -e "$registry"
"#,
                ],
                cwd.clone(),
                PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Enabled),
                env,
                LONG_TIMEOUT_MS,
                /*use_legacy_landlock*/ false,
            )
            .await
            .expect("sandbox should start with denied temp directory");
            assert_eq!(
                (output.exit_code, output.stdout.text.as_str()),
                (0, "project instructions\n"),
                "stderr: {}",
                output.stderr.text
            );
            assert_eq!(std::fs::read_to_string(secret.path()).unwrap(), "private");
            assert!(!temp.path().join(".git").exists());
        }
    }
}

#[tokio::test]
async fn sandbox_reenables_writable_subpaths_under_unreadable_parents() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    let tmpdir = tempfile::tempdir().expect("tempdir");
    let blocked = tmpdir.path().join("blocked");
    let allowed = blocked.join("allowed");
    std::fs::create_dir_all(&allowed).expect("create blocked/allowed dir");
    let allowed_target = allowed.join("note.txt");
    // These tests bypass the usual legacy-policy bridge, so explicitly keep
    // the sandbox helper binary and minimal runtime paths readable.
    let sandbox_helper_dir = codex_linux_sandbox_exe()
        .parent()
        .expect("sandbox helper should have a parent")
        .to_path_buf();

    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Minimal,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: AbsolutePathBuf::try_from(sandbox_helper_dir.as_path())
                    .expect("absolute helper dir")
                    .into(),
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: AbsolutePathBuf::try_from(tmpdir.path())
                    .expect("absolute tempdir")
                    .into(),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: AbsolutePathBuf::try_from(blocked.as_path())
                    .expect("absolute blocked dir")
                    .into(),
            },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: AbsolutePathBuf::try_from(allowed.as_path())
                    .expect("absolute allowed dir")
                    .into(),
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        },
    ]);
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Enabled,
    );
    let output = run_cmd_result_with_permission_profile(
        &[
            "bash",
            "-lc",
            &format!(
                "printf allowed > {} && cat {}",
                allowed_target.to_string_lossy(),
                allowed_target.to_string_lossy()
            ),
        ],
        permission_profile,
        LONG_TIMEOUT_MS,
        /*use_legacy_landlock*/ false,
    )
    .await
    .expect("nested writable carveout should execute under bubblewrap");

    assert_eq!(output.exit_code, 0);
    assert_eq!(output.stdout.text.trim(), "allowed");
}

#[tokio::test]
async fn sandbox_blocks_root_read_carveouts_under_bwrap() {
    if should_skip_bwrap_tests().await {
        eprintln!("skipping bwrap test: bwrap sandbox prerequisites are unavailable");
        return;
    }

    let tmpdir = tempfile::tempdir().expect("tempdir");
    let blocked = tmpdir.path().join("blocked");
    std::fs::create_dir_all(&blocked).expect("create blocked dir");
    let blocked_target = blocked.join("secret.txt");
    std::fs::write(&blocked_target, "secret").expect("seed blocked file");

    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: AbsolutePathBuf::try_from(blocked.as_path())
                    .expect("absolute blocked dir")
                    .into(),
            },
            access: FileSystemAccessMode::Deny,
            missing_path_behavior: None,
        },
    ]);
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Enabled,
    );
    let output = expect_denied(
        run_cmd_result_with_permission_profile(
            &[
                "bash",
                "-lc",
                &format!("cat {}", blocked_target.to_string_lossy()),
            ],
            permission_profile,
            LONG_TIMEOUT_MS,
            /*use_legacy_landlock*/ false,
        )
        .await,
        "root-read carveout should be denied under bubblewrap",
    );

    assert_ne!(output.exit_code, 0);
}

#[tokio::test]
async fn sandbox_blocks_ssh() {
    // Force ssh to attempt a real TCP connection but fail quickly.  `BatchMode`
    // avoids password prompts, and `ConnectTimeout` keeps the hang time low.
    assert_network_blocked(&[
        "ssh",
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=1",
        "github.com",
    ])
    .await;
}

#[tokio::test]
async fn sandbox_blocks_getent() {
    assert_network_blocked(&["getent", "ahosts", "openai.com"]).await;
}

#[tokio::test]
async fn sandbox_blocks_dev_tcp_redirection() {
    // This syntax is only supported by bash and zsh. We try bash first.
    // Fallback generic socket attempt using /bin/sh with bash‑style /dev/tcp.  Not
    // all images ship bash, so we guard against 127 as well.
    assert_network_blocked(&["bash", "-c", "echo hi > /dev/tcp/127.0.0.1/80"]).await;
}

#[path = "daemon_sockets_tests.rs"]
mod daemon_sockets_tests;
