use std::process::Stdio;
use std::time::Duration;

use pretty_assertions::assert_eq;
use tempfile::TempDir;

use codex_app_server_transport::REMOTE_CONTROL_DISABLED_ENV_VAR;

use super::PidBackend;
use super::PidCommandKind;
use super::PidFileState;
use super::PidLogTail;
use super::PidRecord;
use super::read_process_start_time;
use super::read_stderr_log_tail;
use super::stderr_log_file_for_pid_file;
use super::try_lock_file;

#[tokio::test]
async fn locked_empty_pid_file_is_treated_as_active_reservation() {
    let temp_dir = TempDir::new().expect("temp dir");
    let pid_file = temp_dir.path().join("app-server.pid");
    tokio::fs::write(&pid_file, "")
        .await
        .expect("write pid file");
    let backend = PidBackend::new(
        temp_dir.path().join("codex"),
        pid_file.clone(),
        /*remote_control_enabled*/ false,
    );
    let reservation = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&backend.lock_file)
        .await
        .expect("open pid lock file");
    assert!(try_lock_file(&reservation).expect("lock reservation"));

    assert_eq!(
        backend.read_pid_file_state().await.expect("read pid"),
        PidFileState::Starting
    );
    assert!(pid_file.exists());
}

#[tokio::test]
async fn unlocked_empty_pid_file_is_treated_as_stale_reservation() {
    let temp_dir = TempDir::new().expect("temp dir");
    let pid_file = temp_dir.path().join("app-server.pid");
    tokio::fs::write(&pid_file, "")
        .await
        .expect("write pid file");
    let backend = PidBackend::new(
        temp_dir.path().join("codex"),
        pid_file.clone(),
        /*remote_control_enabled*/ false,
    );

    assert_eq!(
        backend.read_pid_file_state().await.expect("read pid"),
        PidFileState::Missing
    );
    assert!(!pid_file.exists());
}

#[tokio::test]
async fn stop_waits_for_live_reservation_to_resolve() {
    let temp_dir = TempDir::new().expect("temp dir");
    let pid_file = temp_dir.path().join("app-server.pid");
    tokio::fs::write(&pid_file, "")
        .await
        .expect("write pid file");
    let backend = PidBackend::new(
        temp_dir.path().join("codex"),
        pid_file.clone(),
        /*remote_control_enabled*/ false,
    );
    let reservation = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&backend.lock_file)
        .await
        .expect("open pid lock file");
    assert!(try_lock_file(&reservation).expect("lock reservation"));
    let cleanup = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        drop(reservation);
        tokio::fs::remove_file(pid_file)
            .await
            .expect("remove pid file");
    });

    backend.stop().await.expect("stop");
    cleanup.await.expect("cleanup task");
}

#[tokio::test]
async fn start_retries_stale_empty_pid_file_under_its_own_lock() {
    let temp_dir = TempDir::new().expect("temp dir");
    let pid_file = temp_dir.path().join("app-server.pid");
    tokio::fs::write(&pid_file, "")
        .await
        .expect("write pid file");
    let backend = PidBackend::new(
        temp_dir.path().join("missing-codex"),
        pid_file,
        /*remote_control_enabled*/ false,
    );

    let err = backend.start().await.expect_err("start");
    assert!(
        err.to_string()
            .starts_with("failed to spawn detached app-server process using ")
    );
}

#[tokio::test]
async fn stale_record_cleanup_preserves_replacement_record() {
    let temp_dir = TempDir::new().expect("temp dir");
    let pid_file = temp_dir.path().join("app-server.pid");
    let backend = PidBackend::new(
        temp_dir.path().join("codex"),
        pid_file.clone(),
        /*remote_control_enabled*/ false,
    );
    let stale = PidRecord {
        pid: 1,
        process_start_time: "old".to_string(),
    };
    let replacement = PidRecord {
        pid: 2,
        process_start_time: "new".to_string(),
    };
    tokio::fs::write(
        &pid_file,
        serde_json::to_vec(&replacement).expect("serialize replacement"),
    )
    .await
    .expect("write replacement pid file");

    assert_eq!(
        backend
            .refresh_after_stale_record(&stale)
            .await
            .expect("cleanup"),
        PidFileState::Running(replacement)
    );
}

#[tokio::test]
async fn stop_reaps_untracked_app_server_child() {
    let temp_dir = TempDir::new().expect("temp dir");
    let pid_file = temp_dir.path().join("app-server.pid");
    let mut child = std::process::Command::new("sleep")
        .arg("5")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn app-server shim");
    let pid = child.id();
    let record = PidRecord {
        pid,
        process_start_time: read_process_start_time(pid).await.expect("start time"),
    };
    tokio::fs::write(
        &pid_file,
        serde_json::to_vec(&record).expect("serialize pid"),
    )
    .await
    .expect("write pid file");
    let backend = PidBackend::new(
        temp_dir.path().join("codex"),
        pid_file.clone(),
        /*remote_control_enabled*/ false,
    );

    let result = tokio::time::timeout(Duration::from_secs(2), backend.stop()).await;
    if matches!(child.try_wait(), Ok(None)) {
        let _ = child.kill();
        let _ = child.wait();
    }

    // `sleep` is not tracked by Tokio, so stop must reap it instead of leaving a zombie.
    result.expect("stop timed out").expect("stop");
    assert!(!pid_file.exists());
}

#[test]
fn update_loop_uses_hidden_app_server_subcommand() {
    let backend = PidBackend {
        codex_bin: "codex".into(),
        pid_file: "updater.pid".into(),
        lock_file: "updater.pid.lock".into(),
        command_kind: PidCommandKind::UpdateLoop,
    };

    assert_eq!(
        backend.command_args(),
        vec!["app-server", "daemon", "pid-update-loop"]
    );
}

#[test]
fn app_server_remote_control_uses_runtime_flag() {
    let backend = PidBackend::new(
        "codex".into(),
        "app-server.pid".into(),
        /*remote_control_enabled*/ true,
    );

    assert_eq!(
        backend.command_args(),
        vec!["app-server", "--remote-control", "--listen", "unix://"]
    );
}

#[test]
fn app_server_disabled_remote_control_uses_compatible_args_and_runtime_env() {
    let backend = PidBackend::new(
        "codex".into(),
        "app-server.pid".into(),
        /*remote_control_enabled*/ false,
    );

    assert_eq!(
        backend.command_args(),
        vec!["app-server", "--listen", "unix://"]
    );
    assert_eq!(
        backend.command_env(),
        Some((REMOTE_CONTROL_DISABLED_ENV_VAR, "1"))
    );
}

#[tokio::test]
async fn read_stderr_log_tail_returns_recent_complete_lines() {
    let temp_dir = TempDir::new().expect("temp dir");
    let pid_file = temp_dir.path().join("app-server.pid");
    let log_file = stderr_log_file_for_pid_file(&pid_file);
    let contents = format!("{}\nrecent error\nusage", "x".repeat(4100));
    tokio::fs::write(&log_file, contents)
        .await
        .expect("write stderr log");

    assert_eq!(
        read_stderr_log_tail(&pid_file)
            .await
            .expect("read stderr log"),
        Some(PidLogTail {
            path: log_file,
            contents: "recent error\nusage".to_string(),
        })
    );
}
