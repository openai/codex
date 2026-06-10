use anyhow::Context;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Output;
use std::process::Stdio;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::process::CommandExt as _;

pub async fn wait_for_pid_file(path: &Path) -> anyhow::Result<String> {
    let pid = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(contents) = fs::read_to_string(path) {
                let trimmed = contents.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .context("timed out waiting for pid file")?;

    Ok(pid)
}

pub fn process_is_alive(pid: &str) -> anyhow::Result<bool> {
    let status = std::process::Command::new("kill")
        .args(["-0", pid])
        .status()
        .context("failed to probe process liveness with kill -0")?;
    Ok(status.success())
}

async fn wait_for_process_exit_inner(pid: String) -> anyhow::Result<()> {
    loop {
        if !process_is_alive(&pid)? {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

pub async fn wait_for_process_exit(pid: &str) -> anyhow::Result<()> {
    let pid = pid.to_string();
    tokio::time::timeout(Duration::from_secs(2), wait_for_process_exit_inner(pid))
        .await
        .context("timed out waiting for process to exit")??;

    Ok(())
}

#[cfg(unix)]
pub fn configure_std_command_for_process_tree_cleanup(command: &mut std::process::Command) {
    command.process_group(0);
}

#[cfg(not(unix))]
pub fn configure_std_command_for_process_tree_cleanup(_: &mut std::process::Command) {}

pub struct ChildProcessCleanupGuard(u32);

impl ChildProcessCleanupGuard {
    pub fn new(process_id: u32) -> Self {
        Self(process_id)
    }

    pub fn cleanup(&self) {
        #[cfg(unix)]
        {
            let _ = codex_utils_pty::process_group::kill_process_group(self.0);
        }

        #[cfg(windows)]
        {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &self.0.to_string(), "/T", "/F"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }

        #[cfg(not(any(unix, windows)))]
        {
            let _ = self.0;
        }
    }
}

impl Drop for ChildProcessCleanupGuard {
    fn drop(&mut self) {
        self.cleanup();
    }
}

pub fn output_with_process_tree_cleanup(
    command: &mut std::process::Command,
    timeout: Duration,
) -> io::Result<Output> {
    configure_std_command_for_process_tree_cleanup(command);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = command.spawn()?;
    let cleanup = ChildProcessCleanupGuard::new(child.id());
    let (sender, receiver) = mpsc::sync_channel(1);
    let _waiter = thread::spawn(move || {
        let _ = sender.send(child.wait_with_output());
    });

    match receiver.recv_timeout(timeout) {
        Ok(output) => {
            cleanup.cleanup();
            output
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            cleanup.cleanup();
            Err(io::Error::new(io::ErrorKind::TimedOut, "process timed out"))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(io::Error::other("process output reader thread exited"))
        }
    }
}
