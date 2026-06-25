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
use super::sandbox_home;
use super::workspace_roots_for;
use crate::unified_exec::spawn_windows_sandbox_session_legacy;
use codex_protocol::models::PermissionProfile;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;

fn assert_restricted_session_stops_grandchild(mode: SessionMode, ending: SessionEnding) {
    let _guard = windows_process_test_guard();
    let runtime = current_thread_runtime();
    runtime.block_on(async move {
        let cwd = sandbox_cwd();
        let powershell = windows_powershell_path();
        let fixture = grandchild_fixture(&cwd, &powershell, ending.root_tail());
        let codex_home = sandbox_home(&format!(
            "restricted-grandchild-{}-{}",
            mode.label(),
            match ending {
                SessionEnding::ExplicitTermination => "terminate",
                SessionEnding::RootExit => "root-exit",
            }
        ));
        let permission_profile = PermissionProfile::workspace_write();
        let spawned = spawn_windows_sandbox_session_legacy(
            &permission_profile,
            workspace_roots_for(cwd.as_path()).as_slice(),
            codex_home.path(),
            fixture.command.clone(),
            cwd.as_path(),
            HashMap::new(),
            Some(30_000),
            &[],
            &[],
            /*tty*/ mode.tty(),
            /*stdin_open*/ mode.tty(),
            /*use_private_desktop*/ mode.tty(),
        )
        .await
        .expect("spawn restricted-token grandchild session");

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
            .expect("timed out waiting for restricted-token root exit")
            .unwrap_or(-1);
        match ending {
            SessionEnding::ExplicitTermination => assert_ne!(exit_code, 0),
            SessionEnding::RootExit => assert_eq!(exit_code, 0),
        }
        assert_grandchild_stopped(&fixture);
    });
}

#[test]
fn restricted_non_tty_termination_stops_grandchild() {
    assert_restricted_session_stops_grandchild(
        SessionMode::Pipe,
        SessionEnding::ExplicitTermination,
    );
}

#[test]
fn restricted_tty_termination_stops_grandchild() {
    assert_restricted_session_stops_grandchild(
        SessionMode::Tty,
        SessionEnding::ExplicitTermination,
    );
}

#[test]
fn restricted_non_tty_root_exit_stops_grandchild() {
    assert_restricted_session_stops_grandchild(SessionMode::Pipe, SessionEnding::RootExit);
}

#[test]
fn restricted_tty_root_exit_stops_grandchild() {
    assert_restricted_session_stops_grandchild(SessionMode::Tty, SessionEnding::RootExit);
}
