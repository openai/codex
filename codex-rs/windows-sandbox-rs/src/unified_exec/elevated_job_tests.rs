#![cfg(target_os = "windows")]

use super::current_thread_runtime;
use super::job_test_support::SessionEnding;
use super::job_test_support::SessionMode;
use super::job_test_support::assert_grandchild_stopped;
use super::job_test_support::grandchild_fixture;
use super::job_test_support::wait_for_grandchild;
use super::job_test_support::windows_powershell_path;
use super::job_test_support::windows_process_test_guard;
use super::sandbox_cwd;
use super::sandbox_log;
use super::workspace_roots_for;
use crate::WindowsSandboxProxySettingsMode;
use crate::ipc_framed::Message;
use crate::ipc_framed::SpawnRequest;
use crate::ipc_framed::read_frame;
use crate::resolved_permissions::ResolvedWindowsSandboxPermissions;
use crate::runner_client::spawn_runner_transport;
use crate::spawn_prep::prepare_elevated_spawn_context_for_permissions;
use crate::unified_exec::spawn_windows_sandbox_session_elevated_for_permission_profile;
use anyhow::Context;
use codex_protocol::models::PermissionProfile;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::time::timeout;

static ELEVATED_TEST_CODEX_HOME: OnceLock<PathBuf> = OnceLock::new();

fn elevated_test_codex_home() -> &'static Path {
    ELEVATED_TEST_CODEX_HOME
        .get_or_init(|| {
            let path = if let Some(test_tmpdir) = std::env::var_os("TEST_TMPDIR") {
                // Elevated setup provisions machine-local users. Bazel retries reuse the same
                // Windows VM, so keep CODEX_HOME stable and reconcile its persisted ACL state.
                PathBuf::from(test_tmpdir).join("elevated-job-lifecycle-codex-home")
            } else {
                std::env::temp_dir().join(format!(
                    "codex-windows-sandbox-elevated-job-lifecycle-{}",
                    std::process::id()
                ))
            };
            fs::create_dir_all(&path).unwrap_or_else(|err| {
                panic!(
                    "create stable elevated test CODEX_HOME {}: {err}",
                    path.display()
                )
            });
            path
        })
        .as_path()
}

fn stage_windows_sandbox_helpers() -> anyhow::Result<()> {
    let test_exe = std::env::current_exe().context("resolve current Windows test executable")?;
    let test_exe_dir = test_exe
        .parent()
        .context("Windows test executable should have a parent directory")?;
    let resources_dir = test_exe_dir.join("codex-resources");
    match fs::create_dir_all(&resources_dir) {
        Ok(()) => {}
        Err(err)
            if err.kind() == std::io::ErrorKind::PermissionDenied && resources_dir.is_dir() => {}
        Err(err) => {
            return Err(err)
                .with_context(|| format!("create resources dir {}", resources_dir.display()));
        }
    }
    for helper_name in ["codex-windows-sandbox-setup", "codex-command-runner"] {
        let file_name = Path::new(helper_name).with_extension("exe");
        let helper = match codex_utils_cargo_bin::cargo_bin(helper_name) {
            Ok(helper) => helper,
            Err(cargo_bin_err) if codex_utils_cargo_bin::runfiles_available() => {
                codex_utils_cargo_bin::resolve_bazel_runfile(
                    option_env!("BAZEL_PACKAGE"),
                    &file_name,
                )
                .with_context(|| {
                    format!(
                        "resolve Bazel runfile for {helper_name} after cargo_bin failed: {cargo_bin_err}"
                    )
                })?
            }
            Err(err) => return Err(err.into()),
        };
        let destination = resources_dir.join(file_name);
        if let Err(err) = fs::copy(&helper, &destination) {
            // A runner from a preceding Bazel retry can briefly retain the staged executable.
            // In that case the existing copy is the binary that retry already launched.
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

fn wait_for_runner_exit(mut pipe_read: std::fs::File) -> i32 {
    let (result_tx, result_rx) = std::sync::mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let result = loop {
            match read_frame(&mut pipe_read) {
                Ok(Some(frame)) => match frame.message {
                    Message::Exit { payload } => break Ok(payload.exit_code),
                    Message::Error { payload } => {
                        break Err(anyhow::anyhow!("runner error: {}", payload.message));
                    }
                    Message::Output { .. }
                    | Message::SpawnReady { .. }
                    | Message::SpawnRequest { .. }
                    | Message::Stdin { .. }
                    | Message::CloseStdin { .. }
                    | Message::Resize { .. }
                    | Message::Terminate { .. } => {}
                },
                Ok(None) => break Err(anyhow::anyhow!("runner pipe closed before exit")),
                Err(err) => break Err(err.context("read runner exit frame")),
            }
        };
        let _ = result_tx.send(result);
    });
    let exit_code = result_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("timed out waiting for runner exit after control transport EOF")
        .expect("runner should report exit after control transport EOF");
    reader.join().expect("runner output reader should finish");
    exit_code
}

fn assert_elevated_session_stops_grandchild(mode: SessionMode, ending: SessionEnding) {
    let _guard = windows_process_test_guard();
    stage_windows_sandbox_helpers().expect("stage elevated sandbox helpers");
    let runtime = current_thread_runtime();
    runtime.block_on(async move {
        let cwd = sandbox_cwd();
        let powershell = windows_powershell_path();
        let fixture = grandchild_fixture(&cwd, &powershell, ending.root_tail());
        let codex_home = elevated_test_codex_home();
        let permission_profile = PermissionProfile::workspace_write();
        let spawned = spawn_windows_sandbox_session_elevated_for_permission_profile(
            &permission_profile,
            workspace_roots_for(cwd.as_path()).as_slice(),
            codex_home,
            fixture.command.clone(),
            cwd.as_path(),
            HashMap::new(),
            /*proxy_enforced*/ false,
            Some(30_000),
            /*read_roots_override*/ None,
            /*read_roots_include_platform_defaults*/ false,
            /*write_roots_override*/ None,
            &[],
            &[],
            /*tty*/ mode.tty(),
            /*stdin_open*/ mode.tty(),
            /*use_private_desktop*/ false,
        )
        .await
        .unwrap_or_else(|err| {
            panic!(
                "spawn elevated {} grandchild session: {err:#}\n{}",
                mode.label(),
                sandbox_log(codex_home)
            )
        });

        wait_for_grandchild(&fixture);

        let codex_utils_pty::SpawnedProcess {
            session,
            stdout_rx: _stdout_rx,
            stderr_rx: _stderr_rx,
            exit_rx,
        } = spawned;
        if matches!(ending, SessionEnding::ExplicitTermination) {
            session.request_terminate();
        }
        let exit_code = timeout(Duration::from_secs(10), exit_rx)
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "timed out waiting for elevated session exit\n{}",
                    sandbox_log(codex_home)
                )
            })
            .unwrap_or(-1);
        match ending {
            SessionEnding::ExplicitTermination => assert_ne!(exit_code, 0),
            SessionEnding::RootExit => assert_eq!(exit_code, 0),
        }
        assert_grandchild_stopped(&fixture);
    });
}

#[test]
fn elevated_non_tty_termination_stops_grandchild() {
    assert_elevated_session_stops_grandchild(SessionMode::Pipe, SessionEnding::ExplicitTermination);
}

#[test]
fn elevated_tty_termination_stops_grandchild() {
    assert_elevated_session_stops_grandchild(SessionMode::Tty, SessionEnding::ExplicitTermination);
}

#[test]
fn elevated_non_tty_root_exit_stops_grandchild() {
    assert_elevated_session_stops_grandchild(SessionMode::Pipe, SessionEnding::RootExit);
}

#[test]
fn elevated_tty_root_exit_stops_grandchild() {
    assert_elevated_session_stops_grandchild(SessionMode::Tty, SessionEnding::RootExit);
}

#[test]
fn elevated_control_transport_eof_stops_grandchild() {
    let _guard = windows_process_test_guard();
    stage_windows_sandbox_helpers().expect("stage elevated sandbox helpers");
    let cwd = sandbox_cwd();
    let powershell = windows_powershell_path();
    let fixture = grandchild_fixture(&cwd, &powershell, "Start-Sleep -Seconds 30");
    let codex_home = elevated_test_codex_home();
    let permission_profile = PermissionProfile::workspace_write();
    let workspace_roots = workspace_roots_for(&cwd);
    let mut env_map = HashMap::new();
    let permissions =
        ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_workspace_roots(
            &permission_profile,
            &workspace_roots,
        )
        .expect("resolve elevated test permissions");
    let elevated = prepare_elevated_spawn_context_for_permissions(
        permissions,
        codex_home,
        &cwd,
        &mut env_map,
        &fixture.command,
        /*read_roots_override*/ None,
        /*read_roots_include_platform_defaults*/ false,
        /*write_roots_override*/ None,
        &[],
        &[],
        /*proxy_enforced*/ false,
        WindowsSandboxProxySettingsMode::Reconcile,
    )
    .unwrap_or_else(|err| {
        panic!(
            "prepare elevated transport-drop session: {err:#}\n{}",
            sandbox_log(codex_home)
        )
    });
    let spawn_request = SpawnRequest {
        command: fixture.command.clone(),
        cwd: cwd.clone(),
        env: env_map,
        permission_profile,
        workspace_roots,
        codex_home: elevated.sandbox_base.clone(),
        real_codex_home: codex_home.to_path_buf(),
        cap_sids: elevated.cap_sids.clone(),
        timeout_ms: Some(30_000),
        tty: false,
        stdin_open: false,
        use_private_desktop: false,
    };
    let transport = spawn_runner_transport(
        codex_home,
        &cwd,
        &elevated.sandbox_creds,
        elevated.logs_base_dir.as_deref(),
        spawn_request,
    )
    .unwrap_or_else(|err| {
        panic!(
            "spawn elevated runner transport: {err:#}\n{}",
            sandbox_log(codex_home)
        )
    });

    wait_for_grandchild(&fixture);
    let (pipe_write, pipe_read) = transport.into_files();
    drop(pipe_write);

    let exit_code = wait_for_runner_exit(pipe_read);
    assert_ne!(exit_code, 0);
    assert_grandchild_stopped(&fixture);
}
