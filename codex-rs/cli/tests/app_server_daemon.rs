#![cfg(unix)]

use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tempfile::TempDir;

struct TestDaemon {
    home: TempDir,
    codex: PathBuf,
    unmanaged: Option<Child>,
}

impl TestDaemon {
    fn new() -> Result<Self> {
        let home = tempfile::Builder::new().tempdir_in("/tmp")?;
        let codex = codex_utils_cargo_bin::cargo_bin("codex")?;
        let codex_source = std::fs::canonicalize(&codex)?;
        let target = if cfg!(target_os = "macos") {
            format!("{}-apple-darwin", std::env::consts::ARCH)
        } else {
            format!("{}-unknown-linux-musl", std::env::consts::ARCH)
        };
        let standalone = home.path().join("packages/standalone");
        let release_name = format!("0.0.0-{target}");
        let managed = standalone
            .join("releases")
            .join(&release_name)
            .join("bin/codex");
        std::fs::create_dir_all(managed.parent().context("managed bin parent")?)?;
        std::fs::hard_link(&codex_source, &managed)
            .or_else(|_| std::fs::copy(&codex_source, managed).map(|_| ()))?;
        std::fs::write(standalone.join("auto-update-version"), &release_name)?;
        std::os::unix::fs::symlink(
            PathBuf::from("releases").join(release_name),
            standalone.join("current"),
        )?;
        // Model a daemon that was previously launched and is currently stopped.
        let state = home.path().join("app-server-daemon");
        std::fs::create_dir(&state)?;
        std::fs::write(state.join("app-server.stderr.log"), b"")?;
        Ok(Self {
            home,
            codex,
            unmanaged: None,
        })
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.codex);
        command.env("CODEX_HOME", self.home.path());
        command
    }

    fn lifecycle(&self, action: &str) -> Result<Value> {
        let output = self
            .command()
            .args(["app-server", "daemon", action])
            .output()?;
        ensure!(
            output.status.success(),
            "daemon {action} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(serde_json::from_slice(&output.stdout)?)
    }

    fn pid(&self, name: &str) -> Result<u32> {
        let record = std::fs::read(self.home.path().join("app-server-daemon").join(name))
            .with_context(|| format!("failed to read {name}"))?;
        Ok(serde_json::from_slice::<Value>(&record)?["pid"]
            .as_u64()
            .context("pid missing")? as u32)
    }
}

impl Drop for TestDaemon {
    fn drop(&mut self) {
        if let Some(mut child) = self.unmanaged.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = self.lifecycle("stop");
        if let Ok(pid) = self.pid("app-server-updater.pid") {
            let _ = signal(pid, libc::SIGTERM);
        }
    }
}

fn signal(pid: u32, signal: libc::c_int) -> Result<()> {
    let raw_pid = libc::pid_t::try_from(pid).context("pid out of range")?;
    ensure!(
        unsafe { libc::kill(raw_pid, signal) } == 0,
        "failed to signal pid {pid}: {}",
        std::io::Error::last_os_error()
    );
    Ok(())
}

fn wait_for_exit(pid: u32) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let output = Command::new("/bin/ps")
            .args(["-p", &pid.to_string(), "-o", "stat="])
            .output()
            .context("failed to invoke ps")?;
        let state = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() || state.trim().starts_with('Z') {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "pid {pid} did not exit: {state}");
        std::thread::sleep(Duration::from_millis(50));
    }
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
    assert_eq!(daemon.lifecycle("start")?["status"], "alreadyRunning");
    assert_eq!(daemon.pid("app-server.pid")?, backend_pid);
    let replacement_pid = daemon.pid("app-server-updater.pid")?;
    assert_ne!(replacement_pid, updater_pid);
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
    assert_eq!(daemon.lifecycle("update")?["status"], "unsupported");
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
            .join("packages/standalone/auto-update-version"),
    )?;

    assert_eq!(daemon.lifecycle("update")?["status"], "unsupported");
    assert!(daemon.pid("app-server.pid").is_err());
    assert!(daemon.pid("app-server-updater.pid").is_err());
    Ok(())
}

fn packaged_daemon_launch(action: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut daemon = TestDaemon::new()?;
    let standalone = daemon.home.path().join("packages/standalone");
    let package = if action == "start" {
        standalone.join("releases/caller")
    } else {
        daemon.home.path().join("cli-package")
    };
    for directory in ["bin", "codex-path", "codex-resources"] {
        std::fs::create_dir_all(package.join(directory))?;
    }
    std::fs::copy(&daemon.codex, package.join("bin/codex"))?;
    daemon.codex = package.join("bin/codex");
    for helper in [
        "bin/codex-code-mode-host",
        "codex-path/rg",
        "codex-resources/bwrap",
    ] {
        std::fs::write(package.join(helper), b"runtime fixture")?;
        std::fs::set_permissions(package.join(helper), std::fs::Permissions::from_mode(0o755))?;
    }
    let target = format!(
        "{}-{}",
        std::env::consts::ARCH,
        if cfg!(target_os = "macos") {
            "apple-darwin"
        } else if cfg!(target_env = "gnu") {
            "unknown-linux-gnu"
        } else {
            "unknown-linux-musl"
        }
    );
    std::fs::write(
        package.join("codex-package.json"),
        serde_json::to_vec(&serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"), "target": target, "entrypoint": "bin/codex"
        }))?,
    )?;
    if action == "start" {
        std::fs::remove_file(standalone.join("current"))?;
        std::os::unix::fs::symlink(&package, standalone.join("current"))?;
    }
    let cli_selection = standalone.join("current").canonicalize()?;
    let state = daemon.home.path().join("app-server-daemon");
    std::fs::remove_file(state.join("app-server.stderr.log"))?;
    std::fs::write(
        state.join("settings.json"),
        br#"{"shutdownGraceSeconds":0}"#,
    )?;
    let cli_before = daemon.codex.canonicalize()?;
    let result = daemon
        .command()
        .args(["app-server", "daemon", action])
        .output()?;
    ensure!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stderr).contains("Installing daemon from CLI version"));
    let output: Value = serde_json::from_slice(&result.stdout)?;
    let dedicated = daemon
        .home
        .path()
        .canonicalize()?
        .join("packages/app-server-daemon");
    assert_eq!(
        output["managedCodexPath"],
        dedicated
            .join("current/bin/codex")
            .to_str()
            .context("managed path is not UTF-8")?
    );
    assert_eq!(
        std::fs::read(dedicated.join("current/codex-path/rg"))?,
        b"runtime fixture"
    );
    assert_eq!(daemon.codex.canonicalize()?, cli_before);
    assert_eq!(standalone.join("current").canonicalize()?, cli_selection);
    assert!(state.join("daemon.pid").exists());
    assert!(!state.join("app-server.pid").exists());
    assert!(!state.join("app-server-updater.pid").exists());
    if action == "bootstrap" {
        assert_eq!(output["autoUpdateEnabled"], false);
    }
    Ok(())
}

#[test]
fn packaged_daemon_start_seeds_local_package() -> Result<()> {
    packaged_daemon_launch("start")
}

#[test]
fn packaged_daemon_restart_seeds_local_package() -> Result<()> {
    packaged_daemon_launch("restart")
}

#[test]
fn packaged_daemon_bootstrap_seeds_local_package() -> Result<()> {
    packaged_daemon_launch("bootstrap")
}
