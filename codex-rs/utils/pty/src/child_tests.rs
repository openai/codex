//! Regression coverage for output-pipe lifetimes and runtime-independent reaping.

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

#[test]
fn non_killing_drop_reaps_after_runtime_shutdown() -> anyhow::Result<()> {
    use crate::child_command::ChildDropPolicy;
    use std::time::Duration;
    if std::env::var_os("CODEX_TEST_ISOLATED_REAPER").is_none() {
        // Other Tokio tests must not drain this process's global orphan queue.
        let output = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "child::tests::non_killing_drop_reaps_after_runtime_shutdown",
                "--nocapture",
            ])
            .env("CODEX_TEST_ISOLATED_REAPER", "1")
            .output()?;
        assert!(
            output.status.success(),
            "isolated reaper test failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return Ok(());
    }
    for program in ["cat", "/bin/cat"] {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let mut child = runtime.block_on(async {
            let mut command = crate::Command::new(program);
            command.drop_policy(ChildDropPolicy::ReapOnly);
            command.spawn()
        })?;
        let stdin = child.stdin.take();
        let pid = child.id().expect("live PID");
        let next = runtime.block_on(async {
            let mut command = crate::Command::new(program);
            command.drop_policy(ChildDropPolicy::ReapOnly);
            command.spawn()
        })?;
        let next_pid = next.id().expect("live PID");
        drop(runtime);
        drop(child);
        drop(next);
        // The shared reaper must collect the second child while the first remains alive.
        wait_until_reaped(next_pid)?;
        // Keep stdin open: cat must remain alive even after its owner and runtime go away.
        std::thread::sleep(Duration::from_millis(50));
        let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
        // SAFETY: waitid only observes our child without reaping it, with writable storage.
        assert_eq!(
            unsafe {
                libc::waitid(
                    libc::P_PID,
                    pid,
                    info.as_mut_ptr(),
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            },
            0
        );
        // SAFETY: waitid succeeded and initialized the zeroed signal information.
        assert_eq!(unsafe { info.assume_init().si_pid() }, 0);
        drop(stdin);
        wait_until_reaped(pid)?;
    }
    Ok(())
}

fn wait_until_reaped(pid: u32) -> anyhow::Result<()> {
    use std::io;
    use std::time::Duration;
    let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        // SAFETY: WNOWAIT prevents the test from stealing the reaper's child.
        if unsafe {
            libc::waitid(
                libc::P_PID,
                pid,
                info.as_mut_ptr(),
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        } == -1
        {
            assert_eq!(
                io::Error::last_os_error().raw_os_error(),
                Some(libc::ECHILD)
            );
            break;
        }
        anyhow::ensure!(std::time::Instant::now() < deadline, "child was not reaped");
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}
