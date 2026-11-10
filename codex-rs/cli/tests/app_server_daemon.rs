//! Managed daemon lifecycle through the public CLI, using isolated standalone fixtures.
#![cfg(unix)]

#[path = "app_server_daemon/support.rs"]
mod support;

use support::TestDaemon;
use support::signal;
use support::wait_for_exit;

use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use pretty_assertions::assert_eq;
use serde_json::Value;

#[test]
fn managed_daemon_restarts_from_deleted_working_directory() -> Result<()> {
    let daemon = TestDaemon::new()?;
    let project = daemon.home.path().join("project");
    std::fs::create_dir(&project)?;
    let started = daemon
        .command()
        .current_dir(&project)
        .args(["app-server", "daemon", "start"])
        .output()?;
    ensure!(
        started.status.success(),
        "{}",
        String::from_utf8_lossy(&started.stderr)
    );
    let old_pid = daemon.pid("app-server.pid")?;
    let old_updater_pid = daemon.pid("app-server-updater.pid")?;
    signal(old_updater_pid, libc::SIGTERM)?;
    wait_for_exit(old_updater_pid)?;

    // Delete the restart caller's cwd without changing the test process's cwd.
    let restarted = Command::new("/bin/sh")
        .args([
            "-c",
            "rmdir \"$1\" && exec \"$2\" app-server daemon restart",
            "restart",
        ])
        .arg(&project)
        .arg(&daemon.codex)
        .current_dir(&project)
        .env("CODEX_HOME", daemon.home.path())
        .output()?;
    ensure!(
        restarted.status.success(),
        "{}",
        String::from_utf8_lossy(&restarted.stderr)
    );
    let restarted: Value = serde_json::from_slice(&restarted.stdout)?;
    assert_eq!(restarted["status"], "restarted");
    assert_ne!(daemon.pid("app-server.pid")?, old_pid);
    assert_ne!(daemon.pid("app-server-updater.pid")?, old_updater_pid);
    assert_eq!(daemon.lifecycle("version")?["status"], "running");
    let updater_socket = daemon
        .home
        .path()
        .join("app-server-daemon/app-server-updater.sock");
    let deadline = Instant::now() + Duration::from_secs(30);
    while std::os::unix::net::UnixStream::connect(&updater_socket).is_err() {
        ensure!(
            Instant::now() < deadline,
            "replacement updater did not become ready"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

#[test]
fn bootstrap_updater_is_ready_in_deep_codex_home() -> Result<()> {
    let mut daemon = TestDaemon::new()?;
    let deep_home = tempfile::Builder::new()
        .prefix(&"deep-codex-home-".repeat(8))
        .tempdir_in("/tmp")?;
    for name in ["packages", "app-server-daemon"] {
        std::fs::rename(daemon.home.path().join(name), deep_home.path().join(name))?;
    }
    daemon.home = deep_home;
    let state = daemon.home.path().join("app-server-daemon");
    std::fs::write(
        state.join("settings.json"),
        br#"{"shutdownGraceSeconds":0}"#,
    )?;
    let socket = state.join("app-server-updater.sock");
    let result = daemon.lifecycle("bootstrap")?;
    assert_eq!(result["autoUpdateEnabled"], true);
    assert_eq!(result["status"], "bootstrapped");
    signal(daemon.pid("app-server-updater.pid")?, /*signal*/ 0)?;
    let target = socket.canonicalize()?;
    let _connection = std::os::unix::net::UnixStream::connect(target)?;
    assert_eq!(daemon.lifecycle("version")?["status"], "running");
    Ok(())
}

#[test]
fn bootstrap_reports_updater_bind_failure() -> Result<()> {
    let daemon = TestDaemon::new()?;
    let state = daemon.home.path().join("app-server-daemon");
    std::fs::write(
        state.join("settings.json"),
        br#"{"shutdownGraceSeconds":0}"#,
    )?;
    let socket = state.join("app-server-updater.sock");
    std::fs::write(&socket, b"not a socket")?;
    let result = daemon
        .command()
        .args(["app-server", "daemon", "bootstrap"])
        .output()?;
    assert!(!result.status.success());
    assert_eq!(std::fs::read(socket)?, b"not a socket");
    assert_eq!(daemon.lifecycle("version")?["status"], "running");
    Ok(())
}

#[test]
fn start_applies_feature_overrides_only_when_launching() -> Result<()> {
    let daemon = TestDaemon::new()?;
    let settings_path = daemon.home.path().join("app-server-daemon/settings.json");
    std::fs::write(
        &settings_path,
        br#"{"updater":{"autoUpdateEnabled":false}}"#,
    )?;
    let mut previous_pid = None;
    for (prefer_mxc, expected_status) in [(true, "started"), (false, "alreadyRunning")] {
        let output = daemon
            .command()
            .args(["app-server", "daemon", "start", "-c"])
            .arg(format!("features.prefer_mxc={prefer_mxc}"))
            .output()?;
        ensure!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(output["status"], expected_status);
        let settings: Value = serde_json::from_slice(&std::fs::read(&settings_path)?)?;
        assert_eq!(
            settings["featureOverrides"],
            serde_json::json!({"prefer_mxc": true})
        );
        let pid = daemon.pid("app-server.pid")?;
        if let Some(previous_pid) = previous_pid {
            assert_eq!(pid, previous_pid);
        }
        previous_pid = Some(pid);
    }
    Ok(())
}

#[test]
fn explicit_restart_applies_feature_overrides_and_preserves_saved_settings() -> Result<()> {
    let daemon = TestDaemon::new()?;
    let settings_path = daemon.home.path().join("app-server-daemon/settings.json");
    std::fs::write(
        &settings_path,
        serde_json::to_vec(&serde_json::json!({
            "shutdownGraceSeconds": 0,
            "updater": { "autoUpdateEnabled": false },
            "featureOverrides": {
                "api_key_model_discovery": false,
                "auth_elicitation": true,
                "connectors": true,
                "apps": true
            }
        }))?,
    )?;
    let mut previous_pid = None;
    // Start from a stopped daemon, change running settings, and restart again
    // with omitted flags. Every explicit restart replaces the process.
    for (args, discovery) in [
        (
            vec![
                "--enable",
                "api_key_model_discovery",
                "--disable",
                "connectors",
                "-c",
                "features.network_proxy.enabled=false",
            ],
            true,
        ),
        (vec!["--disable", "api_key_model_discovery"], false),
        (vec!["-c", "features.api_key_model_discovery=true"], true),
        (vec![], true),
    ] {
        let output = daemon
            .command()
            .args(["app-server", "daemon", "restart"])
            .args(args)
            .output()?;
        ensure!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(output["status"], "restarted");
        let pid = output["pid"].as_u64().context("restarted daemon PID")?;
        assert_ne!(Some(pid), previous_pid);
        previous_pid = Some(pid);
        let settings: Value = serde_json::from_slice(&std::fs::read(&settings_path)?)?;
        assert_eq!(
            settings["featureOverrides"],
            serde_json::json!({
                "api_key_model_discovery": discovery,
                "auth_elicitation": true,
                "apps": false,
                "network_proxy": false
            })
        );
        assert_eq!(settings["updater"]["autoUpdateEnabled"], false);
    }
    // Non-toggle config keys must fail before stopping the daemon or saving settings.
    let saved_settings = std::fs::read(&settings_path)?;
    let mut errors = String::new();
    for action in ["start", "restart"] {
        let output = daemon
            .command()
            .args(["app-server", "daemon", action, "--enable", "tool_registry"])
            .output()?;
        assert!(!output.status.success());
        errors.push_str(&String::from_utf8(output.stderr)?);
        assert_eq!(std::fs::read(&settings_path)?, saved_settings);
        assert_eq!(Some(u64::from(daemon.pid("app-server.pid")?)), previous_pid);
    }
    insta::assert_snapshot!(
        errors,
        @"
        Error: daemon start requires a boolean feature toggle: tool_registry
        Error: daemon restart requires a boolean feature toggle: tool_registry
        "
    );
    assert_eq!(daemon.lifecycle("version")?["status"], "running");
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn managed_identity_survives_locale_and_timezone_changes() -> Result<()> {
    let daemon = TestDaemon::new()?;
    let mut original = Vec::new();
    let mut updater_pid = 0;
    let pid_file = daemon.home.path().join("app-server-daemon/app-server.pid");
    for (action, locale, timezone, expected_status) in [
        ("start", "C", "UTC0", "started"),
        ("version", "en_AU.UTF-8", "PST8PDT", "running"),
        ("restart", "en_AU.UTF-8", "PST8PDT", "restarted"),
        ("stop", "C", "UTC0", "stopped"),
    ] {
        let output = daemon
            .command()
            .args(["app-server", "daemon", action])
            .env("LC_ALL", locale)
            .env("TZ", timezone)
            .output()?;
        ensure!(
            output.status.success(),
            "daemon {action} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(output["status"], expected_status);
        if action == "start" {
            updater_pid = daemon.pid("app-server-updater.pid")?;
            original = std::fs::read(&pid_file)?;
            let record: Value = serde_json::from_slice(&original)?;
            assert!(record["processIdentity"].is_object());
            // Older clients still receive the original, unnormalized ps value.
            let legacy = Command::new("ps")
                .args([
                    "-p",
                    &daemon.pid("app-server.pid")?.to_string(),
                    "-o",
                    "lstart=",
                ])
                .env("LC_ALL", locale)
                .env("TZ", timezone)
                .output()?;
            ensure!(legacy.status.success(), "failed to read legacy start time");
            assert_eq!(
                record["processStartTime"],
                String::from_utf8(legacy.stdout)?.trim()
            );
        } else if action == "version" {
            assert_eq!(output["backend"], "pid");
            assert_eq!(daemon.pid("app-server-updater.pid")?, updater_pid);
            assert_eq!(std::fs::read(&pid_file)?, original);

            // Finish startup promotion before deliberately restoring a legacy record.
            let updater_socket = daemon
                .home
                .path()
                .join("app-server-daemon/app-server-updater.sock");
            let deadline = Instant::now() + Duration::from_secs(30);
            while !updater_socket.exists() {
                ensure!(Instant::now() < deadline, "updater did not become ready");
                std::thread::sleep(Duration::from_millis(50));
            }
            let mut legacy: Value = serde_json::from_slice(&original)?;
            legacy
                .as_object_mut()
                .context("PID record object")?
                .remove("processIdentity");
            let legacy_bytes = serde_json::to_vec(&legacy)?;
            std::fs::write(&pid_file, &legacy_bytes)?;
            let result = daemon
                .command()
                .args(["app-server", "daemon", "version"])
                .env("LC_ALL", locale)
                .env("TZ", timezone)
                .output();
            let preserved = std::fs::read(&pid_file);
            // Restore the native identity before assertions so Drop can stop the daemon.
            std::fs::write(&pid_file, &original)?;
            let result = result?;
            assert!(!result.status.success());
            assert_eq!(preserved?, legacy_bytes);
            let stderr =
                String::from_utf8(result.stderr)?.replace(&legacy["pid"].to_string(), "[PID]");
            insta::assert_snapshot!(stderr, @"Error: cannot verify pid-managed process [PID]: legacy start time changed; PID record retained. Retry with the locale and timezone used to start the daemon. If the system clock changed, stop the original process before restarting the daemon");
        }
        if action == "restart" {
            assert_eq!(daemon.pid("app-server-updater.pid")?, updater_pid);
        }
    }
    assert!(!pid_file.exists());
    Ok(())
}

#[test]
fn package_ownership_check_does_not_start_an_updater() -> Result<()> {
    let daemon = TestDaemon::new()?;
    let output = daemon
        .command()
        .args([
            "app-server",
            "daemon",
            "pid-update-loop",
            "--check-package-ownership",
        ])
        .output()?;
    ensure!(
        output.status.success(),
        "ownership check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let state = daemon.home.path().join("app-server-daemon");
    assert!(!state.join("app-server-updater.pid").exists());
    assert!(!state.join("daemon-updater.pid").exists());
    Ok(())
}

#[test]
fn managed_starts_ensure_one_updater_and_recover_a_missing_one() -> Result<()> {
    let daemon = TestDaemon::new()?;
    assert_eq!(daemon.lifecycle("start")?["status"], "started");
    let backend_pid = daemon.pid("app-server.pid")?;
    let updater_pid = daemon.pid("app-server-updater.pid")?;
    assert_ne!(backend_pid, updater_pid);

    assert_eq!(daemon.lifecycle("start")?["status"], "alreadyRunning");
    assert_eq!(daemon.pid("app-server.pid")?, backend_pid);
    assert_eq!(daemon.pid("app-server-updater.pid")?, updater_pid);

    signal(updater_pid, libc::SIGTERM)?;
    wait_for_exit(updater_pid)?;
    // A replacement updater also upgrades still-verifiable records left by an old CLI.
    let server_record_path = daemon.home.path().join("app-server-daemon/app-server.pid");
    let mut legacy: Value = serde_json::from_slice(&std::fs::read(&server_record_path)?)?;
    let native = legacy.as_object_mut().unwrap().remove("processIdentity");
    std::fs::write(&server_record_path, serde_json::to_vec(&legacy)?)?;
    assert_eq!(daemon.lifecycle("start")?["status"], "alreadyRunning");
    assert_eq!(daemon.pid("app-server.pid")?, backend_pid);
    let replacement_pid = daemon.pid("app-server-updater.pid")?;
    assert_ne!(replacement_pid, updater_pid);
    if let Some(native) = native {
        legacy["processIdentity"] = native;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let record: Value = serde_json::from_slice(&std::fs::read(&server_record_path)?)?;
            if record == legacy {
                break;
            }
            ensure!(
                Instant::now() < deadline,
                "legacy record was not upgraded: {record}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    assert_eq!(daemon.lifecycle("restart")?["status"], "restarted");
    assert_ne!(daemon.pid("app-server.pid")?, backend_pid);
    assert_eq!(daemon.pid("app-server-updater.pid")?, replacement_pid);
    daemon.lifecycle("stop")?;
    assert!(
        !daemon
            .home
            .path()
            .join("app-server-daemon/app-server.pid")
            .exists()
    );
    assert_eq!(daemon.lifecycle("start")?["status"], "started");
    assert!(
        !daemon
            .home
            .path()
            .join("packages/app-server-daemon")
            .exists()
    );
    Ok(())
}

#[test]
fn managed_start_succeeds_when_updater_record_is_invalid() -> Result<()> {
    let daemon = TestDaemon::new()?;
    let state_dir = daemon.home.path().join("app-server-daemon");
    std::fs::create_dir_all(&state_dir)?;
    std::fs::write(state_dir.join("app-server-updater.pid"), "not a PID record")?;

    assert_eq!(daemon.lifecycle("start")?["status"], "started");
    let server_pid = daemon.pid("app-server.pid")?;
    assert_eq!(daemon.lifecycle("start")?["status"], "alreadyRunning");
    assert_eq!(daemon.pid("app-server.pid")?, server_pid);
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn wall_clock_shift_keeps_server_and_updater_managed() -> Result<()> {
    let daemon = TestDaemon::new()?;
    assert_eq!(daemon.lifecycle("start")?["status"], "started");
    let server_pid = daemon.pid("app-server.pid")?;
    let updater_pid = daemon.pid("app-server-updater.pid")?;
    for name in ["app-server.pid", "app-server-updater.pid"] {
        let path = daemon.home.path().join("app-server-daemon").join(name);
        let mut record: Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        record["processStartTime"] = "historical wall-clock start time".into();
        let replacement = path.with_extension("replacement");
        std::fs::write(&replacement, serde_json::to_vec(&record)?)?;
        std::fs::rename(replacement, path)?;
    }

    let version = daemon.lifecycle("version")?;
    assert_eq!(version["status"], "running");
    assert_eq!(version["backend"], "pid");
    assert_eq!(daemon.lifecycle("start")?["status"], "alreadyRunning");
    assert_eq!(daemon.pid("app-server.pid")?, server_pid);
    assert_eq!(daemon.pid("app-server-updater.pid")?, updater_pid);
    assert_eq!(daemon.lifecycle("restart")?["status"], "restarted");
    wait_for_exit(server_pid)?;
    assert_ne!(daemon.pid("app-server.pid")?, server_pid);
    assert_eq!(daemon.pid("app-server-updater.pid")?, updater_pid);
    Ok(())
}

#[test]
fn managed_start_keeps_updater_on_marker_mismatch_but_stops_it_for_pin() -> Result<()> {
    let daemon = TestDaemon::new()?;
    assert_eq!(daemon.lifecycle("start")?["status"], "started");
    let updater_pid = daemon.pid("app-server-updater.pid")?;
    let marker = daemon
        .home
        .path()
        .join("packages/standalone/auto-update-version");
    std::fs::write(&marker, "0.1.0-other-target")?;
    assert_eq!(daemon.lifecycle("start")?["status"], "alreadyRunning");
    assert_eq!(daemon.pid("app-server-updater.pid")?, updater_pid);
    std::thread::sleep(Duration::from_millis(250));
    let updater_state = Command::new("/bin/ps")
        .args(["-p", &updater_pid.to_string(), "-o", "stat="])
        .output()?;
    ensure!(
        updater_state.status.success()
            && !String::from_utf8_lossy(&updater_state.stdout)
                .trim()
                .starts_with('Z'),
        "updater exited during the latest marker transition"
    );

    std::fs::remove_file(marker)?;

    assert_eq!(daemon.lifecycle("start")?["status"], "alreadyRunning");
    wait_for_exit(updater_pid)?;
    assert!(
        !daemon
            .home
            .path()
            .join("app-server-daemon/app-server-updater.pid")
            .exists()
    );
    Ok(())
}

#[test]
fn restart_applies_saved_updater_preference() -> Result<()> {
    let daemon = TestDaemon::new()?;
    assert_eq!(daemon.lifecycle("start")?["status"], "started");
    let updater_pid = daemon.pid("app-server-updater.pid")?;
    let settings = daemon.home.path().join("app-server-daemon/settings.json");
    std::fs::write(
        &settings,
        serde_json::to_vec(&serde_json::json!({
            "updater": {"autoUpdateEnabled": false, "updateIntervalMinutes": 2},
        }))?,
    )?;
    assert_eq!(daemon.lifecycle("restart")?["status"], "restarted");
    wait_for_exit(updater_pid)?;
    assert!(daemon.pid("app-server-updater.pid").is_err());
    assert_eq!(daemon.lifecycle("bootstrap")?["autoUpdateEnabled"], false);
    assert!(daemon.pid("app-server-updater.pid").is_err());

    std::fs::write(
        &settings,
        serde_json::to_vec(&serde_json::json!({
            "updater": {"autoUpdateEnabled": true, "updateIntervalMinutes": 2},
        }))?,
    )?;
    assert_eq!(daemon.lifecycle("restart")?["status"], "restarted");
    assert_ne!(daemon.pid("app-server-updater.pid")?, updater_pid);

    std::fs::write(&settings, "{malformed")?;
    assert_eq!(daemon.lifecycle("stop")?["status"], "stopped");
    Ok(())
}

#[test]
fn unmanaged_app_server_does_not_launch_updater() -> Result<()> {
    let mut daemon = TestDaemon::new()?;
    daemon.unmanaged = Some(
        daemon
            .command()
            .args(["app-server", "--listen", "unix://"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?,
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    while !daemon
        .command()
        .args(["app-server", "daemon", "version"])
        .output()?
        .status
        .success()
    {
        ensure!(Instant::now() < deadline, "app server did not become ready");
        std::thread::sleep(Duration::from_millis(50));
    }

    let output = daemon.lifecycle("start")?;
    assert_eq!(output["status"], "alreadyRunning");
    assert_eq!(output["backend"], Value::Null);
    assert_eq!(daemon.lifecycle("update")?["status"], "unsupported");
    assert!(
        !daemon
            .home
            .path()
            .join("app-server-daemon/app-server-updater.pid")
            .exists()
    );
    Ok(())
}

#[test]
fn manual_update_rejects_an_unowned_installation() -> Result<()> {
    let daemon = TestDaemon::new()?;
    std::fs::remove_file(
        daemon
            .home
            .path()
            .join("packages/standalone/current/bin/codex"),
    )?;

    assert_eq!(daemon.lifecycle("update")?["status"], "unsupported");
    assert!(daemon.pid("app-server.pid").is_err());
    assert!(daemon.pid("app-server-updater.pid").is_err());
    Ok(())
}
