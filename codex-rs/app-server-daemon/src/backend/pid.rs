use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use tokio::fs;
use tokio::process::Command;
use tokio::time::sleep;

const STOP_POLL_INTERVAL: Duration = Duration::from_millis(50);
const STOP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub(crate) struct PidBackend {
    codex_bin: PathBuf,
    pid_file: PathBuf,
    remote_control_enabled: bool,
}

impl PidBackend {
    pub(crate) fn new(codex_bin: PathBuf, pid_file: PathBuf, remote_control_enabled: bool) -> Self {
        Self {
            codex_bin,
            pid_file,
            remote_control_enabled,
        }
    }

    pub(crate) async fn is_running(&self) -> Result<bool> {
        let Some(pid) = self.read_pid().await? else {
            return Ok(false);
        };
        if process_exists(pid) {
            return Ok(true);
        }
        let _ = fs::remove_file(&self.pid_file).await;
        Ok(false)
    }

    pub(crate) async fn start(&self) -> Result<Option<u32>> {
        if let Some(parent) = self.pid_file.parent() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed to create pid directory {}", parent.display()))?;
        }

        let mut command = Command::new(&self.codex_bin);
        if self.remote_control_enabled {
            command.args(["--enable", "remote_control"]);
        }
        command
            .args(["app-server", "--listen", "unix://"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(unix)]
        {
            unsafe {
                command.pre_exec(|| {
                    if libc::setsid() == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }

        let child = command.spawn().with_context(|| {
            format!(
                "failed to spawn detached app-server process using {}",
                self.codex_bin.display()
            )
        })?;
        let pid = child
            .id()
            .context("spawned app-server process has no pid")?;
        fs::write(&self.pid_file, format!("{pid}\n"))
            .await
            .with_context(|| format!("failed to write pid file {}", self.pid_file.display()))?;
        Ok(Some(pid))
    }

    pub(crate) async fn stop(&self) -> Result<()> {
        let Some(pid) = self.read_pid().await? else {
            return Ok(());
        };

        terminate_process(pid)?;
        let deadline = tokio::time::Instant::now() + STOP_TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            if !process_exists(pid) {
                let _ = fs::remove_file(&self.pid_file).await;
                return Ok(());
            }
            sleep(STOP_POLL_INTERVAL).await;
        }

        bail!("timed out waiting for pid-managed app server {pid} to stop")
    }

    async fn read_pid(&self) -> Result<Option<u32>> {
        let contents = match fs::read_to_string(&self.pid_file).await {
            Ok(contents) => contents,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to read pid file {}", self.pid_file.display())
                });
            }
        };
        let pid = contents
            .trim()
            .parse::<u32>()
            .with_context(|| format!("invalid pid file contents in {}", self.pid_file.display()))?;
        Ok(Some(pid))
    }
}

#[cfg(unix)]
fn process_exists(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    let result = unsafe { libc::kill(pid, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
fn process_exists(_pid: u32) -> bool {
    false
}

#[cfg(unix)]
fn terminate_process(pid: u32) -> Result<()> {
    let raw_pid = libc::pid_t::try_from(pid)
        .with_context(|| format!("pid-managed app server pid {pid} is out of range"))?;
    let result = unsafe { libc::kill(raw_pid, libc::SIGTERM) };
    if result == 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    Err(err).with_context(|| format!("failed to terminate pid-managed app server {pid}"))
}

#[cfg(not(unix))]
fn terminate_process(_pid: u32) -> Result<()> {
    bail!("pid-managed app-server shutdown is unsupported on this platform")
}
