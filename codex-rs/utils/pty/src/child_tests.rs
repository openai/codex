//! Regression coverage for output-pipe lifetimes while waiting for a child.

use std::future::Future;
use std::future::poll_fn;
use std::os::fd::AsRawFd;
use std::os::unix::process::ExitStatusExt;
use std::task::Poll;
use std::time::Duration;

use pretty_assertions::assert_eq;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use crate::Command;

#[tokio::test]
async fn wait_with_output_keeps_eof_pipes_open_until_exit() -> anyhow::Result<()> {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "exec 1>&- 2>&-; read -r line"]);
    let mut child = command.spawn()?;
    let mut stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("stdin"))?;
    let stdout = child
        .stdout
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("stdout"))?;
    let stderr = child
        .stderr
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("stderr"))?;
    let fds = [stdout.as_raw_fd(), stderr.as_raw_fd()];

    // Cache EOF readiness on both pipes while stdin keeps the child alive.
    let mut byte = [0];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), stdout.read(&mut byte)).await??,
        0
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), stderr.read(&mut byte)).await??,
        0
    );
    let output = child.wait_with_output();
    tokio::pin!(output);
    poll_fn(|cx| {
        assert!(output.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    for fd in fds {
        // SAFETY: F_GETFD only inspects the descriptor; it does not modify it.
        assert_ne!(
            unsafe { libc::fcntl(fd, libc::F_GETFD) },
            -1,
            "EOF pipe closed before child exit"
        );
    }

    stdin.write_all(b"exit\n").await?;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), output).await??,
        std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    );
    Ok(())
}
