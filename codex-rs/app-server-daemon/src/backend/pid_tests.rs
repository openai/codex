use std::time::Duration;

use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::PidBackend;
use super::PidCommandKind;
use super::PidFileState;
use super::PidRecord;
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
