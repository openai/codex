//! Regression coverage for child descriptors, process modes, and runtime-independent reaping.

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
use crate::ProcessMode;

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

#[tokio::test]
async fn preserved_descriptor_can_use_the_last_slot_below_the_limit() -> anyhow::Result<()> {
    use std::os::fd::AsRawFd;
    use std::os::fd::FromRawFd;
    if std::env::var_os("CODEX_TEST_ISOLATED_FD_LIMIT").is_none() {
        let output = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "child::tests::preserved_descriptor_can_use_the_last_slot_below_the_limit",
                "--nocapture",
            ])
            .env("CODEX_TEST_ISOLATED_FD_LIMIT", "1")
            .output()?;
        assert!(output.status.success(), "{output:?}");
        return Ok(());
    }
    // Use a known, small limit without changing other tests' process-wide limit.
    let mut limit = std::mem::MaybeUninit::<libc::rlimit>::uninit();
    // SAFETY: getrlimit initializes writable storage.
    assert_eq!(
        unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, limit.as_mut_ptr()) },
        0
    );
    // SAFETY: getrlimit succeeded.
    let mut limit = unsafe { limit.assume_init() };
    limit.rlim_cur = limit.rlim_cur.min(256);
    // SAFETY: Only this isolated test process's soft limit is lowered.
    assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) }, 0);
    let last = i32::try_from(limit.rlim_cur - 1)?;
    let root = tempfile::tempdir()?;
    let path = root.path().join("input");
    std::fs::write(&path, "preserved")?;
    let source = std::fs::File::open(path)?;
    let fd = unsafe { libc::fcntl(source.as_raw_fd(), libc::F_DUPFD_CLOEXEC, last) };
    anyhow::ensure!(fd == last, "could not reserve the last descriptor slot");
    let fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) };
    assert_eq!(unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, 0) }, 0);
    let mut command = crate::Command::new("/bin/cat");
    command
        .arg(format!("/dev/fd/{last}"))
        .stdin(crate::ChildStdin::Null)
        .descriptor_policy(crate::DescriptorPolicy::Explicit)
        .preserve_fds(&[last]);
    let output = command.spawn()?.wait_with_output().await?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"preserved");
    Ok(())
}

#[tokio::test]
async fn process_mode_is_preserved_by_both_backends() -> anyhow::Result<()> {
    use crate::ProcessMode;
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir()?;
    std::os::unix::fs::symlink("/bin/cat", root.path().join("server"))?;
    let script = root.path().join("executable-text");
    std::fs::write(&script, "exec /bin/cat\n")?;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(/*mode*/ 0o755))?;
    for program in ["./server", "/bin/cat", "./executable-text"] {
        for mode in [
            ProcessMode::Inherit,
            ProcessMode::NewGroup,
            ProcessMode::NewSession,
        ] {
            let mut command = Command::new(program);
            command.current_dir(root.path()).process_mode(mode);
            let mut child = match command.spawn() {
                Ok(child) => child,
                Err(error) if program == "./executable-text" => {
                    // Executable-text fallback depends on both libc and std's
                    // spawn backend. Compare the same mode with std directly.
                    let mut baseline = tokio::process::Command::new(program);
                    baseline
                        .current_dir(root.path())
                        .env_clear()
                        .kill_on_drop(true);
                    match mode {
                        ProcessMode::Inherit => {}
                        ProcessMode::NewGroup => {
                            baseline.process_group(/*pgroup*/ 0);
                        }
                        ProcessMode::NewSession => {
                            // SAFETY: Session setup is async-signal-safe.
                            unsafe {
                                baseline.pre_exec(crate::process_group::detach_from_tty);
                            }
                        }
                    }
                    let expected = baseline
                        .spawn()
                        .expect_err("legacy spawn must also reject executable text");
                    assert_eq!(error.raw_os_error(), expected.raw_os_error());
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            let pid = child.id().expect("live PID") as libc::pid_t;
            // SAFETY: These calls only inspect the current process.
            let (parent_group, parent_session) = unsafe { (libc::getpgrp(), libc::getsid(0)) };
            let expected = match mode {
                ProcessMode::Inherit => (parent_group, parent_session),
                ProcessMode::NewGroup => (pid, parent_session),
                ProcessMode::NewSession => (pid, pid),
            };
            // SAFETY: The child is still owned and blocked on its stdin pipe.
            assert_eq!(unsafe { (libc::getpgid(pid), libc::getsid(pid)) }, expected);
            child.kill().await?;
        }
    }
    Ok(())
}

#[tokio::test]
async fn preserving_descriptors_keeps_parent_record_locks() -> anyhow::Result<()> {
    use std::io::Read;
    if let Some(path) = std::env::var_os("CODEX_TEST_RECORD_LOCK") {
        // Wait until spawn has returned and dropped the parent's command.
        std::io::stdin().read_exact(&mut [0])?;
        let file = std::fs::OpenOptions::new().write(true).open(path)?;
        // SAFETY: A zeroed flock with these fields describes the whole file.
        let mut lock: libc::flock = unsafe { std::mem::zeroed() };
        lock.l_type = libc::F_WRLCK as _;
        lock.l_whence = libc::SEEK_SET as _;
        assert_eq!(
            unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &lock) },
            -1
        );
        assert!(matches!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EACCES | libc::EAGAIN)
        ));
        return Ok(());
    }
    let file = tempfile::NamedTempFile::new()?;
    // SAFETY: The descriptor remains owned and open throughout the launch.
    assert_eq!(
        unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, 0) },
        0
    );
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as _;
    lock.l_whence = libc::SEEK_SET as _;
    assert_eq!(
        unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &lock) },
        0
    );
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args([
            "--exact",
            "child::tests::preserving_descriptors_keeps_parent_record_locks",
            "--nocapture",
        ])
        .env("CODEX_TEST_RECORD_LOCK", file.path())
        .descriptor_policy(crate::DescriptorPolicy::Explicit)
        .preserve_fds(&[file.as_raw_fd()]);
    let mut child = command.spawn()?;
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(b"x")
        .await?;
    let output = child.wait_with_output().await?;
    assert!(output.status.success(), "{output:?}");
    Ok(())
}

#[tokio::test]
async fn detached_spawn_preserves_child_path_cwd_environment_and_arg0() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    std::fs::create_dir(root.path().join("bin"))?;
    std::os::unix::fs::symlink("/bin/sh", root.path().join("bin/child-shell"))?;
    let mut command = Command::new("child-shell");
    command
        .current_dir(root.path())
        .env("PATH", "bin")
        .env("MARKER", "child-value")
        .arg0("custom-shell")
        .args(["-c", "printf '%s|%s|' \"$0\" \"$MARKER\"; pwd; read -r line; printf '%s' \"$line\"; printf diagnostic >&2"])
        .process_mode(ProcessMode::NewSession);
    let mut child = command.spawn()?;
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(b"input\n")
        .await?;
    assert_eq!(
        child.wait_with_output().await?,
        std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: format!(
                "custom-shell|child-value|{}\ninput",
                root.path().canonicalize()?.display()
            )
            .into_bytes(),
            stderr: b"diagnostic".to_vec(),
        }
    );
    Ok(())
}

#[tokio::test]
async fn detached_spawn_preserves_exec_errors_and_executable_text_handling() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir()?;
    let program = root.path().join("script");
    let mut missing = Command::new(&program);
    missing.process_mode(ProcessMode::NewSession);
    let error = missing.spawn().err().expect("missing executable must fail");
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);

    std::fs::write(&program, "printf '%s' \"$1\"")?;
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))?;
    // Match the existing libc behavior: glibc/macOS retry executable text through
    // a shell, whereas musl returns ENOEXEC.
    let mut baseline = tokio::process::Command::new(&program);
    baseline.env_clear().arg("shell fallback");
    // SAFETY: The legacy route only performs async-signal-safe session setup.
    unsafe {
        baseline.pre_exec(crate::process_group::detach_from_tty);
    }
    let expected = baseline.output().await;
    let mut script = Command::new(&program);
    script
        .arg("shell fallback")
        .process_mode(ProcessMode::NewSession);
    let actual = match script.spawn() {
        Ok(child) => child.wait_with_output().await,
        Err(error) => Err(error),
    };
    let error_details = |error: std::io::Error| (error.kind(), error.raw_os_error());
    assert_eq!(
        actual.map_err(error_details),
        expected.map_err(error_details)
    );
    Ok(())
}
