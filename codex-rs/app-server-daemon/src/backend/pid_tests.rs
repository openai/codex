#[cfg(any(unix, windows))]
use std::process::Stdio;
use std::time::Duration;

use pretty_assertions::assert_eq;
use tempfile::TempDir;
#[cfg(any(unix, windows))]
use tokio::time::sleep;

use codex_app_server_transport::REMOTE_CONTROL_DISABLED_ENV_VAR;

use super::PidBackend;
use super::PidCommandKind;
use super::PidFileState;
use super::PidLogTail;
use super::PidRecord;
#[cfg(unix)]
use super::read_process_start_time;
use super::read_stderr_log_tail;
use super::stderr_log_file_for_pid_file;
use super::try_lock_file;

#[cfg(windows)]
fn is_elevated_test_process() -> anyhow::Result<bool> {
    let output = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)",
        ])
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "administrator membership query failed"
    );
    match String::from_utf8(output.stdout)?.trim() {
        "True" => Ok(true),
        "False" => Ok(false),
        other => anyhow::bail!("unexpected administrator membership: {other}"),
    }
}

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
    let state_dir = temp_dir.path().join("state");
    codex_uds::prepare_private_socket_directory(&state_dir)
        .await
        .expect("private state directory");
    let pid_file = state_dir.join("app-server.pid");
    tokio::fs::write(&pid_file, "")
        .await
        .expect("write pid file");
    let backend = PidBackend::new(
        temp_dir.path().join("missing-codex"),
        pid_file,
        /*remote_control_enabled*/ false,
    );

    let err = backend.start().await.expect_err("start");
    #[cfg(windows)]
    if is_elevated_test_process().expect("query administrator membership") {
        assert!(err.to_string().contains("non-elevated terminal"));
        assert_eq!(
            tokio::fs::read(&backend.pid_file).await.unwrap(),
            Vec::<u8>::new()
        );
        assert!(!backend.lock_file.exists());
        return;
    }
    assert!(
        err.to_string()
            .starts_with("failed to spawn detached app-server process using ")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_launch_clears_recovery_best_effort() {
    for snapshot_is_directory in [false, true] {
        let home = TempDir::new().expect("temp dir");
        let state_dir = home.path().join("app-server-daemon");
        std::fs::create_dir_all(&state_dir).expect("state dir");
        let recovery_file = codex_app_server_transport::daemon_recovery_file_path(home.path());
        if snapshot_is_directory {
            std::fs::create_dir(&recovery_file).expect("invalid snapshot directory");
        } else {
            std::fs::write(&recovery_file, "{}").expect("pending snapshot");
        }
        let backend = PidBackend::new(
            home.path().join("missing-codex"),
            state_dir.join("app-server.pid"),
            /*remote_control_enabled*/ false,
        );

        let error = backend.start().await.expect_err("missing binary");
        assert!(
            error
                .to_string()
                .starts_with("failed to spawn detached app-server process using "),
            "{error:#}"
        );
        assert_eq!(recovery_file.exists(), snapshot_is_directory);
    }
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
        process_identity: None,
        executable_identity: None,
    };
    let replacement = PidRecord {
        pid: 2,
        process_start_time: "new".to_string(),
        process_identity: None,
        executable_identity: None,
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

#[cfg(unix)]
#[tokio::test]
async fn pid_record_captures_the_resolved_launch_binary() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new().expect("temp dir");
    let original = temp.path().join("original-codex");
    let replacement = temp.path().join("replacement-codex");
    for (path, bytes) in [
        (&original, b"#!/bin/sh\nexec sleep 30\n".as_slice()),
        (
            &replacement,
            b"#!/bin/sh\n# replacement\nexec sleep 30\n".as_slice(),
        ),
    ] {
        std::fs::write(path, bytes).expect("binary");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("executable");
    }
    let selected = temp.path().join("current-codex");
    std::os::unix::fs::symlink(&original, &selected).expect("selected binary");
    let backend = PidBackend::new(
        selected.clone(),
        temp.path().join("app-server.pid"),
        /*remote_control_enabled*/ false,
    );
    backend.start().await.expect("start daemon");
    let record: PidRecord =
        serde_json::from_slice(&std::fs::read(&backend.pid_file).expect("PID record"))
            .expect("parse PID record");
    assert_eq!(
        record.executable_identity,
        Some(
            crate::managed_install::executable_identity(&original)
                .await
                .expect("original digest")
        )
    );
    std::fs::remove_file(&selected).expect("remove link");
    std::os::unix::fs::symlink(&replacement, &selected).expect("retarget link");
    assert_ne!(
        record.executable_identity,
        Some(
            crate::managed_install::executable_identity(&selected)
                .await
                .expect("new digest")
        )
    );
    assert_eq!(
        serde_json::from_str::<PidRecord>(r#"{"pid":1,"processStartTime":"old"}"#)
            .expect("legacy PID record")
            .executable_identity,
        None
    );
    backend.stop().await.expect("stop daemon");
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_start_time_mismatch_preserves_record_and_process() {
    let temp = TempDir::new().unwrap();
    let backend = PidBackend::new(
        temp.path().join("codex"),
        temp.path().join("app-server.pid"),
        /*remote_control_enabled*/ false,
    );
    let contents = serde_json::to_vec(&serde_json::json!({
        "pid": std::process::id(),
        "processStartTime": "historical wall-clock start time",
    }))
    .unwrap();
    std::fs::write(&backend.pid_file, &contents).unwrap();
    for result in [
        backend.is_starting_or_running().await.map(|_| ()),
        backend.start().await.map(|_| ()),
        backend.stop().await,
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        backend.promote_legacy_identity().await,
    ] {
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("PID record retained")
        );
    }
    assert_eq!(std::fs::read(&backend.pid_file).unwrap(), contents);
}

#[cfg(unix)]
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
        process_identity: None,
        executable_identity: None,
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

#[cfg(any(unix, windows))]
#[tokio::test]
async fn shutdown_grace_child() {
    let Some(ready) = std::env::var_os("CODEX_TEST_SHUTDOWN_GRACE_READY") else {
        return;
    };
    let ready = std::path::PathBuf::from(ready);
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("install shutdown handler");
    tokio::fs::write(&ready, "").await.expect("ready marker");
    let _ = tokio::time::timeout(Duration::from_secs(8), async {
        #[cfg(unix)]
        terminate.recv().await;
        #[cfg(windows)]
        loop {
            if ready.with_extension("shutdown").exists() {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        if std::env::var_os("CODEX_TEST_SHUTDOWN_GRACE_EXIT").is_some() {
            sleep(Duration::from_millis(150)).await;
            tokio::fs::write(ready.with_extension("exited"), "")
                .await
                .expect("graceful exit marker");
            return;
        }
        std::future::pending::<()>().await;
    })
    .await;
}

#[cfg(any(unix, windows))]
#[tokio::test]
async fn shutdown_grace_handles_process_exit() {
    let temp = TempDir::new().expect("temp dir");
    for (name, grace_seconds, exits) in [
        ("zero", 0, false),
        ("finite_exit", 1, true),
        ("finite_force", 1, false),
    ] {
        let ready = temp.path().join(format!("{name}.ready"));
        let mut child = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args(["--exact", "backend::pid::tests::shutdown_grace_child"])
            .env("CODEX_TEST_SHUTDOWN_GRACE_READY", &ready)
            .envs(exits.then_some(("CODEX_TEST_SHUTDOWN_GRACE_EXIT", "1")))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn daemon shim");
        let pid = child.id();
        let wait_until = tokio::time::Instant::now() + Duration::from_secs(3);
        while !ready.exists() {
            assert!(
                tokio::time::Instant::now() < wait_until,
                "shim did not start"
            );
            sleep(Duration::from_millis(10)).await;
        }
        let pid_file = temp.path().join(format!("{name}.pid"));
        let record = PidRecord {
            pid,
            process_start_time: super::read_process_start_time(pid)
                .await
                .expect("start time"),
            process_identity: None,
            executable_identity: None,
        };
        tokio::fs::write(
            &pid_file,
            serde_json::to_vec(&record).expect("serialize pid"),
        )
        .await
        .expect("write pid file");
        #[cfg(unix)]
        let backend = PidBackend::new(
            temp.path().join("codex"),
            pid_file,
            /*remote_control_enabled*/ false,
        );
        #[cfg(windows)]
        let backend = PidBackend::new_update_loop(
            temp.path().join("codex"),
            pid_file,
            /*restore_release*/ None,
        );
        let result = tokio::time::timeout(
            Duration::from_secs(3),
            backend.stop_with_grace(grace_seconds),
        )
        .await;
        let still_running = backend
            .record_is_active(&record)
            .await
            .expect("child status");
        if still_running {
            child.kill().expect("clean up daemon shim");
        }
        let _ = child.wait(); // The backend may already have reaped a Unix child.
        result.expect("stop deadline").expect("stop outcome");
        assert_eq!(ready.with_extension("exited").exists(), exits);
        assert!(!still_running, "{name}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn stopping_updater_signals_its_installer_process_group() {
    use std::os::unix::process::CommandExt;

    let temp = TempDir::new().expect("temp dir");
    let ready = temp.path().join("installer.ready");
    let stopped = temp.path().join("installer.stopped");
    let mut command = std::process::Command::new("/bin/sh");
    command
        .args([
            "-c",
            "sh -c 'trap \"touch $STOP_MARKER; exit 0\" TERM; touch $READY_MARKER; while :; do sleep 1; done' & wait",
        ])
        .env("READY_MARKER", &ready)
        .env("STOP_MARKER", &stopped)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().expect("spawn updater shim");
    let pid = child.id();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while !ready.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "installer did not start"
        );
        sleep(Duration::from_millis(10)).await;
    }
    let pid_file = temp.path().join("updater.pid");
    tokio::fs::write(
        &pid_file,
        serde_json::to_vec(&PidRecord {
            pid,
            process_start_time: read_process_start_time(pid).await.expect("start time"),
            process_identity: None,
            executable_identity: None,
        })
        .expect("serialize pid"),
    )
    .await
    .expect("write pid file");
    let backend = PidBackend::new_update_loop(
        temp.path().join("codex"),
        pid_file,
        /*restore_release*/ None,
    );
    backend.stop().await.expect("stop updater");
    // The backend normally reaps the shim, so a second wait may return ECHILD.
    let _ = child.wait();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while !stopped.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "installer was not stopped"
        );
        sleep(Duration::from_millis(10)).await;
    }
}

#[cfg(unix)]
#[tokio::test]
async fn exited_unreaped_updater_is_reaped() {
    let temp = TempDir::new().expect("temp dir");
    let mut child = std::process::Command::new("sleep")
        .arg("60")
        .spawn()
        .expect("spawn updater shim");
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let process_identity = Some(
        super::identity::read_process_details(child.id())
            .await
            .unwrap()
            .1,
    );
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let process_identity = None;
    let record = PidRecord {
        pid: child.id(),
        process_start_time: read_process_start_time(child.id())
            .await
            .expect("start time"),
        process_identity,
        executable_identity: None,
    };
    let backend = PidBackend::new_update_loop(
        temp.path().join("codex"),
        temp.path().join("updater.pid"),
        /*restore_release*/ None,
    );
    child.kill().expect("terminate updater shim");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let result = loop {
        let result = backend.record_is_active(&record).await;
        if matches!(&result, Ok(false)) || tokio::time::Instant::now() >= deadline {
            break result;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert!(!result.expect("check zombie updater"));
    assert_eq!(
        child
            .wait()
            .expect_err("updater shim was already reaped")
            .raw_os_error(),
        Some(libc::ECHILD)
    );
}

#[test]
fn update_loop_uses_hidden_app_server_subcommand() {
    let backend = PidBackend {
        feature_overrides: Default::default(),
        codex_bin: "codex".into(),
        pid_file: "updater.pid".into(),
        lock_file: "updater.pid.lock".into(),
        command_kind: PidCommandKind::UpdateLoop {
            restore_release: None,
        },
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

#[tokio::test]
async fn stale_creation_time_never_stops_reused_pid() {
    let temp = TempDir::new().expect("temp");
    let backend = PidBackend::new(
        temp.path().join("codex.exe"),
        temp.path().join("server.pid"),
        /*remote_control_enabled*/ false,
    );
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let identities = {
        use super::identity::ProcessIdentity;
        let (_, identity) = super::identity::read_process_details(std::process::id())
            .await
            .unwrap();
        let mut stale_time = identity.clone();
        let mut stale_epoch = identity;
        match &mut stale_time {
            ProcessIdentity::Linux { start_ticks, .. } => *start_ticks += 1,
            ProcessIdentity::MacOs {
                start_microseconds, ..
            }
            | ProcessIdentity::MacOsUnique {
                start_microseconds, ..
            } => *start_microseconds += 1,
        }
        match &mut stale_epoch {
            ProcessIdentity::Linux { boot_id, .. } => boot_id.push_str("-previous"),
            ProcessIdentity::MacOs { start_seconds, .. } => *start_seconds += 1,
            ProcessIdentity::MacOsUnique { unique_id, .. } => *unique_id += 1,
        }
        [Some(stale_time), Some(stale_epoch)]
    };
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let identities = [None];
    for process_identity in identities {
        let record = PidRecord {
            pid: std::process::id(),
            process_start_time: "stale".into(),
            process_identity,
            executable_identity: None,
        };
        let contents = serde_json::to_vec(&record).unwrap();
        std::fs::write(&backend.pid_file, &contents).unwrap();
        backend.stop().await.expect("stale record cleanup");
        assert!(!backend.pid_file.exists());
        assert!(!backend.pid_file.with_extension("shutdown").exists());
    }
}

#[cfg(windows)]
#[tokio::test]
async fn failed_updater_handoff_preserves_predecessor_record() {
    let elevated = is_elevated_test_process().expect("query administrator membership");
    let temp = TempDir::new().expect("temp");
    let state_dir = temp.path().join("state");
    codex_uds::prepare_private_socket_directory(&state_dir)
        .await
        .expect("private state directory");
    let backend = PidBackend::new_update_loop(
        temp.path().join("missing-codex.exe"),
        state_dir.join("updater.pid"),
        /*restore_release*/ None,
    );
    let record = PidRecord {
        pid: std::process::id(),
        process_start_time: super::read_process_start_time(std::process::id())
            .await
            .unwrap(),
        process_identity: None,
        executable_identity: None,
    };
    for record in [
        record.clone(),
        PidRecord {
            process_start_time: "stale".into(),
            ..record
        },
    ] {
        tokio::fs::write(&backend.pid_file, serde_json::to_vec(&record).unwrap())
            .await
            .unwrap();
        let error = backend.replace_current_updater().await.unwrap_err();
        if elevated {
            assert!(error.to_string().contains("non-elevated terminal"));
        } else {
            // Windows may reject breakaway before reporting the missing executable.
            // Both must reach process I/O rather than rejecting stale ownership.
            assert!(error.root_cause().is::<std::io::Error>(), "{error:#}");
        }
        assert_eq!(
            backend.read_pid_file_state().await.unwrap(),
            PidFileState::Running(record)
        );
    }
}

#[cfg(windows)]
#[tokio::test]
async fn updater_readiness_and_post_publication_failure_preserve_ownership() {
    use futures::FutureExt;
    use windows_sys::Win32::Security::ImpersonateAnonymousToken;
    use windows_sys::Win32::Security::RevertToSelf;
    use windows_sys::Win32::System::Threading::GetCurrentThread;

    let temp = TempDir::new().expect("temp");
    let backend = PidBackend::new_update_loop(
        temp.path().join("codex.exe"),
        temp.path().join("updater.pid"),
        /*restore_release*/ None,
    );
    let _lock = backend
        .acquire_reservation_lock()
        .await
        .expect("reservation lock");
    let mut child = tokio::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Start-Sleep 60",
        ])
        .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
        .kill_on_drop(true)
        .spawn()
        .expect("successor fixture");
    let pid = child.id().expect("pid");
    let successor = PidRecord {
        pid,
        process_start_time: super::read_process_start_time(pid)
            .await
            .expect("creation time"),
        process_identity: None,
        executable_identity: None,
    };
    let predecessor = PidRecord {
        pid: std::process::id(),
        process_start_time: super::read_process_start_time(std::process::id())
            .await
            .expect("creation time"),
        process_identity: None,
        executable_identity: None,
    };
    tokio::fs::write(&backend.pid_file, serde_json::to_vec(&successor).unwrap())
        .await
        .unwrap();
    let ready = backend.pid_file.with_extension("ready");
    tokio::fs::write(&ready, b"").await.unwrap();
    backend
        .finish_updater_start(&successor, Some(&predecessor))
        .await
        .expect("ready successor");
    assert!(!ready.exists());
    assert_eq!(
        backend.read_pid_file_state().await.unwrap(),
        PidFileState::Running(successor.clone())
    );

    // If cleanup cannot even query the successor, leave its ownership intact.
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                assert_ne!(unsafe { ImpersonateAnonymousToken(GetCurrentThread()) }, 0);
                let result = backend
                    .finish_updater_start(&successor, Some(&predecessor))
                    .now_or_never();
                let reverted = unsafe { RevertToSelf() };
                assert_ne!(reverted, 0);
                let error = result
                    .expect("denied cleanup must not suspend")
                    .unwrap_err();
                assert_eq!(
                    error
                        .downcast_ref::<std::io::Error>()
                        .unwrap()
                        .raw_os_error(),
                    Some(windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED as i32)
                );
            })
            .join()
            .expect("anonymous cleanup check");
    });
    assert_eq!(
        backend.read_pid_file_state().await.unwrap(),
        PidFileState::Running(successor.clone())
    );

    let reused_pid = PidRecord {
        process_start_time: "stale".into(),
        ..successor.clone()
    };
    assert!(
        backend
            .finish_updater_start(&reused_pid, Some(&predecessor))
            .await
            .is_err()
    );
    assert_eq!(
        backend.read_pid_file_state().await.unwrap(),
        PidFileState::Running(predecessor.clone())
    );
    assert!(
        child.try_wait().unwrap().is_none(),
        "PID reuse must not terminate another process"
    );
    tokio::fs::write(&backend.pid_file, serde_json::to_vec(&successor).unwrap())
        .await
        .unwrap();
    // An invalid readiness path triggers cleanup while the successor is alive.
    tokio::fs::create_dir(&ready).await.unwrap();
    assert!(
        backend
            .finish_updater_start(&successor, Some(&predecessor))
            .await
            .is_err()
    );
    assert!(
        child.try_wait().unwrap().is_some(),
        "successor must exit before rollback"
    );
    assert_eq!(
        backend.read_pid_file_state().await.unwrap(),
        PidFileState::Running(predecessor)
    );
}

#[cfg(windows)]
#[test]
fn inaccessible_pid_preserves_identity_check_error() {
    use futures::FutureExt;
    use windows_sys::Win32::Security::ImpersonateAnonymousToken;
    use windows_sys::Win32::Security::RevertToSelf;
    use windows_sys::Win32::System::Threading::GetCurrentThread;

    // Isolate impersonation from Tokio workers and other tests. Query our own
    // process anonymously instead of assuming a system PID is inaccessible.
    std::thread::spawn(|| {
        let record = PidRecord {
            pid: std::process::id(),
            process_start_time: "stale".into(),
            process_identity: None,
            executable_identity: None,
        };
        assert!(
            crate::backend::windows::Process::open(record.pid)
                .unwrap()
                .is_some()
        );
        assert_ne!(unsafe { ImpersonateAnonymousToken(GetCurrentThread()) }, 0);
        let opened = crate::backend::windows::Process::open(record.pid);
        // Windows identity checks are synchronous; never suspend while impersonating.
        let matches = super::process_matches_record(&record).now_or_never();
        let reverted = unsafe { RevertToSelf() };
        assert_ne!(reverted, 0);

        let error = match opened {
            Err(error) => error,
            Ok(_) => panic!("anonymous caller unexpectedly queried the test process"),
        };
        assert_eq!(
            error
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .raw_os_error(),
            Some(windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED as i32),
        );
        assert_eq!(
            matches
                .expect("identity check must not suspend")
                .unwrap_err()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .raw_os_error(),
            Some(windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED as i32),
        );
    })
    .join()
    .expect("anonymous identity check");
}
