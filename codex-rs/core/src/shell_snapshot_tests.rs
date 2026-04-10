use super::*;
use pretty_assertions::assert_eq;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::process::Command;
#[cfg(target_os = "linux")]
use std::process::Command as StdCommand;

use tempfile::tempdir;

#[cfg(unix)]
struct BlockingStdinPipe {
    original: i32,
    write_end: i32,
}

#[cfg(unix)]
impl BlockingStdinPipe {
    fn install() -> Result<Self> {
        let mut fds = [0i32; 2];
        if unsafe { libc::pipe(fds.as_mut_ptr()) } == -1 {
            return Err(std::io::Error::last_os_error()).context("create stdin pipe");
        }

        let original = unsafe { libc::dup(libc::STDIN_FILENO) };
        if original == -1 {
            let err = std::io::Error::last_os_error();
            unsafe {
                libc::close(fds[0]);
                libc::close(fds[1]);
            }
            return Err(err).context("dup stdin");
        }

        if unsafe { libc::dup2(fds[0], libc::STDIN_FILENO) } == -1 {
            let err = std::io::Error::last_os_error();
            unsafe {
                libc::close(fds[0]);
                libc::close(fds[1]);
                libc::close(original);
            }
            return Err(err).context("replace stdin");
        }

        unsafe {
            libc::close(fds[0]);
        }

        Ok(Self {
            original,
            write_end: fds[1],
        })
    }
}

#[cfg(unix)]
impl Drop for BlockingStdinPipe {
    fn drop(&mut self) {
        unsafe {
            libc::dup2(self.original, libc::STDIN_FILENO);
            libc::close(self.original);
            libc::close(self.write_end);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn assert_posix_snapshot_sections(snapshot: &str) {
    assert!(snapshot.contains("# Snapshot file"));
    assert!(snapshot.contains("aliases "));
    assert!(snapshot.contains("exports "));
    assert!(
        snapshot.contains("PATH"),
        "snapshot should capture a PATH export"
    );
    assert!(snapshot.contains("setopts "));
}

async fn get_snapshot(shell_type: ShellType) -> Result<String> {
    let dir = tempdir()?;
    let path = dir.path().join("snapshot.sh");
    write_shell_snapshot(shell_type, &path, dir.path()).await?;
    let content = fs::read_to_string(&path).await?;
    Ok(content)
}

#[test]
fn strip_snapshot_preamble_removes_leading_output() {
    let snapshot = "noise\n# Snapshot file\nexport PATH=/bin\n";
    let cleaned = strip_snapshot_preamble(snapshot).expect("snapshot marker exists");
    assert_eq!(cleaned, "# Snapshot file\nexport PATH=/bin\n");
}

#[test]
fn strip_snapshot_preamble_requires_marker() {
    let result = strip_snapshot_preamble("missing header");
    assert!(result.is_err());
}

#[test]
fn snapshot_file_name_parser_supports_legacy_and_suffixed_names() {
    let session_id = "019cf82b-6a62-7700-bbbd-46909794ef89";

    assert_eq!(
        snapshot_session_id_from_file_name(&format!("{session_id}.sh")),
        Some(session_id)
    );
    assert_eq!(
        snapshot_session_id_from_file_name(&format!("{session_id}.123.sh")),
        Some(session_id)
    );
    assert_eq!(
        snapshot_session_id_from_file_name(&format!("{session_id}.tmp-123")),
        Some(session_id)
    );
    assert_eq!(
        snapshot_session_id_from_file_name("not-a-snapshot.txt"),
        None
    );
}

#[cfg(unix)]
#[test]
fn bash_snapshot_filters_invalid_exports() -> Result<()> {
    let output = Command::new("/bin/bash")
        .arg("-c")
        .arg(bash_snapshot_script())
        .env("BASH_ENV", "/dev/null")
        .env("VALID_NAME", "ok")
        .env("PWD", "/tmp/stale")
        .env("NEXTEST_BIN_EXE_codex-write-config-schema", "/path/to/bin")
        .env("BAD-NAME", "broken")
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("VALID_NAME"));
    assert!(!stdout.contains("PWD=/tmp/stale"));
    assert!(!stdout.contains("NEXTEST_BIN_EXE_codex-write-config-schema"));
    assert!(!stdout.contains("BAD-NAME"));

    Ok(())
}

#[cfg(unix)]
#[test]
fn bash_snapshot_preserves_multiline_exports() -> Result<()> {
    let multiline_cert = "-----BEGIN CERTIFICATE-----\nabc\n-----END CERTIFICATE-----";
    let output = Command::new("/bin/bash")
        .arg("-c")
        .arg(bash_snapshot_script())
        .env("BASH_ENV", "/dev/null")
        .env("MULTILINE_CERT", multiline_cert)
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("MULTILINE_CERT=") || stdout.contains("MULTILINE_CERT"),
        "snapshot should include the multiline export name"
    );

    let dir = tempdir()?;
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(&snapshot_path, stdout.as_bytes())?;

    let validate = Command::new("/bin/bash")
        .arg("-c")
        .arg("set -e; . \"$1\"")
        .arg("bash")
        .arg(&snapshot_path)
        .env("BASH_ENV", "/dev/null")
        .output()?;

    assert!(
        validate.status.success(),
        "snapshot validation failed: {}",
        String::from_utf8_lossy(&validate.stderr)
    );

    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn zsh_snapshot_preserves_tied_path_export() -> Result<()> {
    let dir = tempdir()?;
    let tool_dir = dir.path().join("toolbin");
    let expected_path = format!("{}:/usr/bin:/bin", tool_dir.display());
    std::fs::write(
        dir.path().join(".zshrc"),
        format!("export PATH='{expected_path}'\n"),
    )?;

    let output = Command::new("/bin/zsh")
        .arg("-fc")
        .arg(zsh_snapshot_script())
        .env("ZDOTDIR", dir.path())
        .output()?;
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("export -T PATH path="),
        "snapshot should include zsh tied PATH export; stdout={stdout:?}"
    );

    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(&snapshot_path, stdout.as_bytes())?;

    let validate = Command::new("/bin/zsh")
        .arg("-fc")
        .arg("PATH=/should/not/survive; . \"$1\"; print -r -- \"$PATH\"")
        .arg("zsh")
        .arg(&snapshot_path)
        .output()?;
    assert!(
        validate.status.success(),
        "snapshot validation failed: {}",
        String::from_utf8_lossy(&validate.stderr)
    );

    assert_eq!(
        String::from_utf8_lossy(&validate.stdout).trim(),
        expected_path
    );

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn try_new_creates_and_deletes_snapshot_file() -> Result<()> {
    let dir = tempdir()?;
    let shell = Shell {
        shell_type: ShellType::Bash,
        shell_path: PathBuf::from("/bin/bash"),
        shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
    };

    let snapshot = ShellSnapshot::try_new(dir.path(), ThreadId::new(), dir.path(), &shell)
        .await
        .expect("snapshot should be created");
    let path = snapshot.path.clone();
    assert!(path.exists());
    assert_eq!(snapshot.cwd, dir.path().to_path_buf());

    drop(snapshot);

    assert!(!path.exists());

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn try_new_uses_distinct_generation_paths() -> Result<()> {
    let dir = tempdir()?;
    let session_id = ThreadId::new();
    let shell = Shell {
        shell_type: ShellType::Bash,
        shell_path: PathBuf::from("/bin/bash"),
        shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
    };

    let initial_snapshot = ShellSnapshot::try_new(dir.path(), session_id, dir.path(), &shell)
        .await
        .expect("initial snapshot should be created");
    let refreshed_snapshot = ShellSnapshot::try_new(dir.path(), session_id, dir.path(), &shell)
        .await
        .expect("refreshed snapshot should be created");
    let initial_path = initial_snapshot.path.clone();
    let refreshed_path = refreshed_snapshot.path.clone();

    assert_ne!(initial_path, refreshed_path);
    assert_eq!(initial_path.exists(), true);
    assert_eq!(refreshed_path.exists(), true);

    drop(initial_snapshot);

    assert_eq!(initial_path.exists(), false);
    assert_eq!(refreshed_path.exists(), true);

    drop(refreshed_snapshot);

    assert_eq!(refreshed_path.exists(), false);

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn snapshot_shell_does_not_inherit_stdin() -> Result<()> {
    let _stdin_guard = BlockingStdinPipe::install()?;

    let dir = tempdir()?;
    let home = dir.path();
    let read_status_path = home.join("stdin-read-status");
    let read_status_display = read_status_path.display();
    // Persist the startup `read` exit status so the test can assert whether
    // bash saw EOF on stdin after the snapshot process exits.
    let bashrc = format!("read -t 1 -r ignored\nprintf '%s' \"$?\" > \"{read_status_display}\"\n");
    fs::write(home.join(".bashrc"), bashrc).await?;

    let shell = Shell {
        shell_type: ShellType::Bash,
        shell_path: PathBuf::from("/bin/bash"),
        shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
    };

    let home_display = home.display();
    let script = format!(
        "HOME=\"{home_display}\"; export HOME; {}",
        bash_snapshot_script()
    );
    let output = run_script_with_timeout(
        &shell,
        &script,
        Duration::from_secs(2),
        /*use_login_shell*/ true,
        home,
    )
    .await
    .context("run snapshot command")?;
    let read_status = fs::read_to_string(&read_status_path)
        .await
        .context("read stdin probe status")?;

    assert_eq!(
        read_status, "1",
        "expected shell startup read to see EOF on stdin; status={read_status:?}"
    );

    assert!(
        output.contains("# Snapshot file"),
        "expected snapshot marker in output; output={output:?}"
    );

    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn timed_out_snapshot_shell_is_terminated() -> Result<()> {
    use std::process::Stdio;
    use tokio::time::Duration as TokioDuration;
    use tokio::time::Instant;
    use tokio::time::sleep;

    let dir = tempdir()?;
    let pid_path = dir.path().join("pid");
    let script = format!("echo $$ > \"{}\"; sleep 30", pid_path.display());

    let shell = Shell {
        shell_type: ShellType::Sh,
        shell_path: PathBuf::from("/bin/sh"),
        shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
    };

    let err = run_script_with_timeout(
        &shell,
        &script,
        Duration::from_secs(1),
        /*use_login_shell*/ true,
        dir.path(),
    )
    .await
    .expect_err("snapshot shell should time out");
    assert!(
        err.to_string().contains("timed out"),
        "expected timeout error, got {err:?}"
    );

    let pid = fs::read_to_string(&pid_path)
        .await
        .expect("snapshot shell writes its pid before timing out")
        .trim()
        .parse::<i32>()?;

    let deadline = Instant::now() + TokioDuration::from_secs(1);
    loop {
        let kill_status = StdCommand::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .status()?;
        if !kill_status.success() {
            break;
        }
        if Instant::now() >= deadline {
            panic!("timed out snapshot shell is still alive after grace period");
        }
        sleep(TokioDuration::from_millis(50)).await;
    }

    Ok(())
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn macos_zsh_snapshot_includes_sections() -> Result<()> {
    let snapshot = get_snapshot(ShellType::Zsh).await?;
    assert_posix_snapshot_sections(&snapshot);
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn linux_bash_snapshot_includes_sections() -> Result<()> {
    let snapshot = get_snapshot(ShellType::Bash).await?;
    assert_posix_snapshot_sections(&snapshot);
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn linux_sh_snapshot_includes_sections() -> Result<()> {
    let snapshot = get_snapshot(ShellType::Sh).await?;
    assert_posix_snapshot_sections(&snapshot);
    Ok(())
}

#[cfg(target_os = "windows")]
#[ignore]
#[tokio::test]
async fn windows_powershell_snapshot_includes_sections() -> Result<()> {
    let snapshot = get_snapshot(ShellType::PowerShell).await?;
    assert!(snapshot.contains("# Snapshot file"));
    assert!(snapshot.contains("aliases "));
    assert!(snapshot.contains("exports "));
    Ok(())
}

async fn write_rollout_stub(codex_home: &Path, session_id: ThreadId) -> Result<PathBuf> {
    let dir = codex_home
        .join("sessions")
        .join("2025")
        .join("01")
        .join("01");
    fs::create_dir_all(&dir).await?;
    let path = dir.join(format!("rollout-2025-01-01T00-00-00-{session_id}.jsonl"));
    fs::write(&path, "").await?;
    Ok(path)
}

#[tokio::test]
async fn cleanup_stale_snapshots_removes_orphans_and_keeps_live() -> Result<()> {
    let dir = tempdir()?;
    let codex_home = dir.path();
    let snapshot_dir = codex_home.join(SNAPSHOT_DIR);
    fs::create_dir_all(&snapshot_dir).await?;

    let live_session = ThreadId::new();
    let orphan_session = ThreadId::new();
    let live_snapshot = snapshot_dir.join(format!("{live_session}.123.sh"));
    let orphan_snapshot = snapshot_dir.join(format!("{orphan_session}.456.sh"));
    let invalid_snapshot = snapshot_dir.join("not-a-snapshot.txt");

    write_rollout_stub(codex_home, live_session).await?;
    fs::write(&live_snapshot, "live").await?;
    fs::write(&orphan_snapshot, "orphan").await?;
    fs::write(&invalid_snapshot, "invalid").await?;

    cleanup_stale_snapshots(codex_home, ThreadId::new()).await?;

    assert_eq!(live_snapshot.exists(), true);
    assert_eq!(orphan_snapshot.exists(), false);
    assert_eq!(invalid_snapshot.exists(), false);
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn cleanup_stale_snapshots_removes_stale_rollouts() -> Result<()> {
    let dir = tempdir()?;
    let codex_home = dir.path();
    let snapshot_dir = codex_home.join(SNAPSHOT_DIR);
    fs::create_dir_all(&snapshot_dir).await?;

    let stale_session = ThreadId::new();
    let stale_snapshot = snapshot_dir.join(format!("{stale_session}.123.sh"));
    let rollout_path = write_rollout_stub(codex_home, stale_session).await?;
    fs::write(&stale_snapshot, "stale").await?;

    set_file_mtime(&rollout_path, SNAPSHOT_RETENTION + Duration::from_secs(60))?;

    cleanup_stale_snapshots(codex_home, ThreadId::new()).await?;

    assert_eq!(stale_snapshot.exists(), false);
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn cleanup_stale_snapshots_skips_active_session() -> Result<()> {
    let dir = tempdir()?;
    let codex_home = dir.path();
    let snapshot_dir = codex_home.join(SNAPSHOT_DIR);
    fs::create_dir_all(&snapshot_dir).await?;

    let active_session = ThreadId::new();
    let active_snapshot = snapshot_dir.join(format!("{active_session}.123.sh"));
    let rollout_path = write_rollout_stub(codex_home, active_session).await?;
    fs::write(&active_snapshot, "active").await?;

    set_file_mtime(&rollout_path, SNAPSHOT_RETENTION + Duration::from_secs(60))?;

    cleanup_stale_snapshots(codex_home, active_session).await?;

    assert_eq!(active_snapshot.exists(), true);
    Ok(())
}

#[cfg(unix)]
fn set_file_mtime(path: &Path, age: Duration) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs()
        .saturating_sub(age.as_secs());
    let tv_sec = now
        .try_into()
        .map_err(|_| anyhow!("Snapshot mtime is out of range for libc::timespec"))?;
    let ts = libc::timespec { tv_sec, tv_nsec: 0 };
    let times = [ts, ts];
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
    let result = unsafe { libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), 0) };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}
