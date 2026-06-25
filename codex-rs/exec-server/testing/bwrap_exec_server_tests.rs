use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::Environment;
use codex_exec_server::ExecOutputStream;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcess;
use codex_exec_server::ExecProcessEvent;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::ProcessId;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use serde::Deserialize;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::time::timeout;

use super::BwrapExecServer;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NamespaceReport {
    user_namespace: String,
    mount_namespace: String,
    pid_namespace: String,
    network_namespace: String,
    ipc_namespace: String,
    uts_namespace: String,
    proc_pid_one_namespace: Option<String>,
    proc_file_system_type: String,
    overflow_uid: String,
    overflow_gid: String,
    effective_uid: u32,
    effective_gid: u32,
    effective_capabilities: String,
    permitted_capabilities: String,
    inheritable_capabilities: String,
    ambient_capabilities: String,
    no_new_privileges: u32,
    seccomp_mode: u32,
    connect_succeeded: Option<bool>,
    allowed_write_succeeded: Option<bool>,
    denied_read_succeeded: Option<bool>,
    denied_write_succeeded: Option<bool>,
}

#[derive(Debug)]
struct ReportOutcome {
    report: NamespaceReport,
    stderr: String,
    exit_code: Option<i32>,
}

#[derive(Clone, Copy)]
enum FixtureExit {
    Success,
    Code(u8),
}

#[derive(Debug)]
struct NamespaceIdentity {
    user: String,
    mount: String,
    pid: String,
    network: String,
    ipc: String,
    uts: String,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_sandbox_nests_with_fresh_namespaces_and_proc() -> Result<()> {
    let parent_namespaces = current_namespace_identity()?;
    let fixture = codex_utils_cargo_bin::cargo_bin("bwrap-exec-smoke")?;
    let bwrap = codex_utils_cargo_bin::cargo_bin("bwrap")?;
    let bazel_workspace = std::env::current_dir().context("resolve Bazel test workspace")?;
    let workspace = TempDir::new_in(&bazel_workspace)?;
    let outside_workspace = TempDir::new_in(&bazel_workspace)?;
    let allowed_marker = workspace.path().join("allowed-write-probe");
    let denied_marker = outside_workspace.path().join("denied-write-probe");
    std::fs::write(&allowed_marker, "sentinel")?;
    std::fs::write(&denied_marker, "sentinel")?;
    for protected_name in [".git", ".agents", ".codex"] {
        assert!(!workspace.path().join(protected_name).exists());
    }
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let listener_address = listener.local_addr()?.to_string();
    let host_tmp = TempDir::new_in("/tmp")?;
    let isolated_tmp_dir = host_tmp.path().to_path_buf();
    let isolated_tmp_marker = isolated_tmp_dir.join("same-path-marker");
    std::fs::write(&isolated_tmp_marker, "host")?;

    BwrapExecServer
        .scope(|exec_server_url| async move {
            let environment = Environment::create_for_tests(Some(exec_server_url))?;
            let file_system = environment.get_filesystem();
            let isolated_tmp_dir_uri = PathUri::from_host_native_path(&isolated_tmp_dir)?;
            let isolated_tmp_marker_uri = PathUri::from_host_native_path(&isolated_tmp_marker)?;
            file_system
                .create_directory(
                    &isolated_tmp_dir_uri,
                    CreateDirectoryOptions { recursive: true },
                    /*sandbox*/ None,
                )
                .await?;
            file_system
                .write_file(
                    &isolated_tmp_marker_uri,
                    b"remote".to_vec(),
                    /*sandbox*/ None,
                )
                .await?;
            assert_eq!(
                file_system
                    .read_file_text(&isolated_tmp_marker_uri, /*sandbox*/ None)
                    .await?,
                "remote"
            );
            assert_eq!(std::fs::read_to_string(&isolated_tmp_marker)?, "host");

            let outer = run_report(
                &environment,
                "bwrap-outer-report",
                &fixture,
                &bwrap,
                workspace.path(),
                &allowed_marker,
                &denied_marker,
                &listener_address,
                /*sandbox*/ None,
                FixtureExit::Success,
            )
            .await?;

            assert_eq!(outer.exit_code, Some(0), "{}", outer.stderr);
            let outer = outer.report;

            let _connection = timeout(Duration::from_secs(2), listener.accept())
                .await
                .context("outer bwrap did not reach the test listener")??;
            assert_eq!(outer.connect_succeeded, Some(true));
            assert_eq!(outer.allowed_write_succeeded, Some(true));
            assert_eq!(outer.denied_read_succeeded, Some(true));
            assert_eq!(outer.denied_write_succeeded, Some(true));
            assert_eq!(std::fs::read_to_string(&allowed_marker)?, "sentinelchanged");
            assert_eq!(std::fs::read_to_string(&denied_marker)?, "sentinelchanged");
            std::fs::write(&allowed_marker, "sentinel")?;
            std::fs::write(&denied_marker, "sentinel")?;

            assert_ne!(outer.user_namespace, parent_namespaces.user);
            assert_ne!(outer.mount_namespace, parent_namespaces.mount);
            assert_ne!(outer.pid_namespace, parent_namespaces.pid);
            assert_ne!(outer.ipc_namespace, parent_namespaces.ipc);
            assert_ne!(outer.uts_namespace, parent_namespaces.uts);
            assert_eq!(outer.network_namespace, parent_namespaces.network);
            assert_proc_is_available(&outer);
            assert_eq!((outer.effective_uid, outer.effective_gid), (1000, 1000));
            assert_no_capabilities(&outer)?;

            let cwd = PathUri::from_host_native_path(workspace.path())?;
            let mut file_system_policy = FileSystemSandboxPolicy::workspace_write(
                &[],
                /*exclude_tmpdir_env_var*/ true,
                /*exclude_slash_tmp*/ true,
            );
            file_system_policy.entries.push(FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: denied_marker.as_path().try_into()?,
                },
                access: FileSystemAccessMode::Deny,
            });
            let sandbox = FileSystemSandboxContext::from_permission_profile_with_cwd(
                PermissionProfile::from_runtime_permissions(
                    &file_system_policy,
                    NetworkSandboxPolicy::Restricted,
                ),
                cwd,
            );
            let inner = run_report(
                &environment,
                "bwrap-inner-report",
                &fixture,
                &bwrap,
                workspace.path(),
                &allowed_marker,
                &denied_marker,
                &listener_address,
                Some(sandbox),
                FixtureExit::Code(73),
            )
            .await?;

            assert_eq!(inner.exit_code, Some(73), "{}", inner.stderr);
            let inner = inner.report;

            assert_eq!(inner.connect_succeeded, Some(false));
            assert_eq!(inner.allowed_write_succeeded, Some(true));
            assert_eq!(inner.denied_read_succeeded, Some(false));
            assert_eq!(inner.denied_write_succeeded, Some(false));
            assert_eq!(std::fs::read_to_string(&allowed_marker)?, "sentinelchanged");
            assert_eq!(std::fs::read_to_string(&denied_marker)?, "sentinel");
            assert!(
                timeout(Duration::from_millis(200), listener.accept())
                    .await
                    .is_err(),
                "sandboxed process unexpectedly reached the outer loopback listener"
            );

            assert_ne!(inner.user_namespace, outer.user_namespace);
            assert_ne!(inner.mount_namespace, outer.mount_namespace);
            assert_ne!(inner.pid_namespace, outer.pid_namespace);
            assert_ne!(inner.network_namespace, outer.network_namespace);
            assert_proc_matches_pid_namespace(&inner);
            assert_eq!((inner.effective_uid, inner.effective_gid), (1000, 1000));
            assert_no_capabilities(&inner)?;
            assert_eq!(inner.no_new_privileges, 1);
            assert_eq!(inner.seccomp_mode, 2);
            for protected_name in [".git", ".agents", ".codex"] {
                assert!(
                    !workspace.path().join(protected_name).exists(),
                    "sandbox setup leaked synthetic {protected_name} mount target"
                );
            }
            Ok(())
        })
        .await
}

#[allow(clippy::too_many_arguments)]
async fn run_report(
    environment: &Environment,
    process_id: &str,
    fixture: &Path,
    bwrap: &Path,
    cwd: &Path,
    allowed_marker: &Path,
    denied_marker: &Path,
    listener_address: &str,
    sandbox: Option<FileSystemSandboxContext>,
    fixture_exit: FixtureExit,
) -> Result<ReportOutcome> {
    let bwrap_dir = bwrap.parent().context("bwrap runfile has no parent")?;
    let mut path_entries = vec![bwrap_dir.to_path_buf()];
    if let Some(path) = std::env::var_os("PATH") {
        path_entries.extend(std::env::split_paths(&path));
    }
    let path = std::env::join_paths(path_entries).context("build sandbox smoke PATH")?;
    let mut argv = vec![
        fixture.to_string_lossy().into_owned(),
        "--connect".to_string(),
        listener_address.to_string(),
        "--allowed-write".to_string(),
        allowed_marker.to_string_lossy().into_owned(),
        "--denied-write".to_string(),
        denied_marker.to_string_lossy().into_owned(),
    ];
    if let FixtureExit::Code(exit_code) = fixture_exit {
        argv.extend(["--exit-code".to_string(), exit_code.to_string()]);
    }
    let started = environment
        .get_exec_backend()
        .start(ExecParams {
            process_id: ProcessId::from(process_id),
            argv,
            cwd: PathUri::from_host_native_path(cwd)?,
            env_policy: /*env_policy*/ None,
            env: HashMap::from([
                (
                    "CARGO_BIN_EXE_bwrap".to_string(),
                    bwrap.to_string_lossy().into_owned(),
                ),
                ("CODEX_HOME".to_string(), cwd.to_string_lossy().into_owned()),
                ("HOME".to_string(), cwd.to_string_lossy().into_owned()),
                ("PATH".to_string(), path.to_string_lossy().into_owned()),
                ("TMPDIR".to_string(), "/tmp".to_string()),
            ]),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox,
            enforce_managed_network: false,
            managed_network: None,
        })
        .await?;
    let (stdout, stderr, exit_code) = collect_process_output(started.process).await?;
    let report = serde_json::from_str(stdout.trim())
        .with_context(|| format!("parse namespace report; stderr: {stderr}"))?;
    Ok(ReportOutcome {
        report,
        stderr,
        exit_code,
    })
}

async fn collect_process_output(
    process: Arc<dyn ExecProcess>,
) -> Result<(String, String, Option<i32>)> {
    let mut events = process.subscribe_events();
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = None;
    loop {
        match timeout(Duration::from_secs(10), events.recv())
            .await
            .context("timed out waiting for bwrap smoke process")??
        {
            ExecProcessEvent::Output(chunk) => match chunk.stream {
                ExecOutputStream::Stdout | ExecOutputStream::Pty => {
                    stdout.push_str(&String::from_utf8_lossy(&chunk.chunk.into_inner()));
                }
                ExecOutputStream::Stderr => {
                    stderr.push_str(&String::from_utf8_lossy(&chunk.chunk.into_inner()));
                }
            },
            ExecProcessEvent::Exited {
                exit_code: code, ..
            } => exit_code = Some(code),
            ExecProcessEvent::Closed { .. } => break,
            ExecProcessEvent::Failed(message) => anyhow::bail!(message),
        }
    }
    Ok((stdout, stderr, exit_code))
}

fn current_namespace_identity() -> Result<NamespaceIdentity> {
    Ok(NamespaceIdentity {
        user: namespace("user")?,
        mount: namespace("mnt")?,
        pid: namespace("pid")?,
        network: namespace("net")?,
        ipc: namespace("ipc")?,
        uts: namespace("uts")?,
    })
}

fn namespace(name: &str) -> std::io::Result<String> {
    std::fs::read_link(format!("/proc/self/ns/{name}"))
        .map(|path| path.to_string_lossy().into_owned())
}

fn assert_proc_matches_pid_namespace(report: &NamespaceReport) {
    assert_proc_is_available(report);
    assert_eq!(
        report.proc_pid_one_namespace.as_deref(),
        Some(report.pid_namespace.as_str())
    );
}

fn assert_proc_is_available(report: &NamespaceReport) {
    assert_eq!(report.proc_file_system_type, "proc");
    assert!(report.overflow_uid.parse::<u32>().is_ok());
    assert!(report.overflow_gid.parse::<u32>().is_ok());
}

fn assert_no_capabilities(report: &NamespaceReport) -> Result<()> {
    for (name, capabilities) in [
        ("effective", report.effective_capabilities.as_str()),
        ("permitted", report.permitted_capabilities.as_str()),
        ("inheritable", report.inheritable_capabilities.as_str()),
        ("ambient", report.ambient_capabilities.as_str()),
    ] {
        assert_eq!(
            u64::from_str_radix(capabilities, 16)?,
            0,
            "process retained {name} capabilities"
        );
    }
    Ok(())
}
