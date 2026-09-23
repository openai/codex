//! Exercise the public pipe adapter with native spawning and explicit inheritance.
//! This binary isolates atfork registration from the rest of the test suite.

#![cfg(target_os = "macos")]

use std::collections::HashMap;
use std::io::Read;
use std::io::Seek;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::fs::symlink;
use std::os::unix::process::CommandExt;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_utils_pty::spawn_pipe_process;
use codex_utils_pty::spawn_pipe_process_no_stdin;
use pretty_assertions::assert_eq;

static PARENT_FORKS: AtomicUsize = AtomicUsize::new(0);

extern "C" fn record_parent_fork() {
    PARENT_FORKS.fetch_add(1, Ordering::Relaxed);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pipe_commands_avoid_fork() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let cwd = directory.path().canonicalize()?;
    let executable = std::env::current_exe()?;
    symlink(&executable, cwd.join("probe"))?;
    let mut file = std::fs::File::create(cwd.join("preserved"))?;
    let preserved = inheritable_fd(&file, /*minimum*/ 100)?;
    let excluded = inheritable_fd(&file, /*minimum*/ 200)?;
    let args = ["--exact", "pipe_child", "--ignored", "--nocapture"].map(str::to_owned);
    let mut env = HashMap::from([
        ("PATH".to_string(), ".".to_string()),
        ("PRESERVED".to_string(), preserved.as_raw_fd().to_string()),
        ("EXCLUDED".to_string(), excluded.as_raw_fd().to_string()),
        (
            "EXPECTED_CWD".to_string(),
            cwd.to_string_lossy().into_owned(),
        ),
    ]);
    // SAFETY: This is the binary's only active test; the permanent parent hook
    // only updates a lock-free atomic and remains valid until process exit.
    assert_eq!(
        unsafe {
            libc::pthread_atfork(
                /*prepare*/ None,
                Some(record_parent_fork),
                /*child*/ None,
            )
        },
        0
    );
    let mut control = std::process::Command::new("/usr/bin/true");
    // SAFETY: A no-op pre-exec hook forces fork for the positive control.
    unsafe {
        control.pre_exec(|| Ok(()));
    }
    assert!(control.status()?.success());
    assert_eq!(PARENT_FORKS.load(Ordering::Relaxed), 1);

    for program in [executable.as_os_str(), "./probe".as_ref(), "probe".as_ref()] {
        for input in ["hello", ""] {
            env.insert("EXPECTED_INPUT".to_string(), input.to_string());
            file.set_len(0)?;
            file.rewind()?;
            let inherited = [preserved.as_raw_fd()];
            let arg0 = Some("pipe-probe".to_string());
            let mut child = if input.is_empty() {
                spawn_pipe_process_no_stdin(program, &args, &cwd, &env, &arg0, &inherited).await?
            } else {
                spawn_pipe_process(program, &args, &cwd, &env, &arg0, &inherited).await?
            };
            if !input.is_empty() {
                child
                    .session
                    .writer_sender()
                    .send(input.as_bytes().to_vec())
                    .await?;
            }
            child.session.close_stdin();
            let (status, stdout, stderr) = tokio::time::timeout(Duration::from_secs(10), async {
                let stdout = async {
                    let mut output = Vec::new();
                    while let Some(bytes) = child.stdout_rx.recv().await {
                        output.extend(bytes);
                    }
                    output
                };
                let stderr = async {
                    let mut output = Vec::new();
                    while let Some(bytes) = child.stderr_rx.recv().await {
                        output.extend(bytes);
                    }
                    output
                };
                tokio::join!(child.exit_rx, stdout, stderr)
            })
            .await?;
            assert_eq!(
                status?,
                0,
                "{}\n{}",
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
            assert!(String::from_utf8_lossy(&stdout).contains("pipe child assertions completed"));
            assert!(String::from_utf8_lossy(&stderr).contains("pipe diagnostic"));
            assert_eq!(std::fs::read(cwd.join("preserved"))?, b"fd");
            assert_eq!(PARENT_FORKS.load(Ordering::Relaxed), 1);
        }
    }
    Ok(())
}

#[test]
#[ignore = "child process for pipe_commands_avoid_fork"]
fn pipe_child() -> anyhow::Result<()> {
    assert_eq!(std::env::args().next().as_deref(), Some("pipe-probe"));
    assert_eq!(
        std::env::current_dir()?,
        std::path::PathBuf::from(std::env::var("EXPECTED_CWD")?)
    );
    assert!(std::env::var_os("HOME").is_none());
    // SAFETY: These calls inspect the current process's group and session.
    assert_eq!(
        unsafe {
            (libc::getpgrp(), libc::getsid(/*pid*/ 0))
        },
        unsafe { (libc::getpid(), libc::getpid()) }
    );
    let preserved = std::env::var("PRESERVED")?.parse::<i32>()?;
    let excluded = std::env::var("EXCLUDED")?.parse::<i32>()?;
    // SAFETY: The buffer is valid; the syscall reports an error if the fd was not inherited.
    assert_eq!(
        unsafe { libc::write(preserved, b"fd".as_ptr().cast(), 2) },
        2
    );
    // SAFETY: fcntl safely probes whether the unrelated descriptor survived exec.
    assert_eq!(unsafe { libc::fcntl(excluded, libc::F_GETFD) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    assert_eq!(input, std::env::var("EXPECTED_INPUT")?);
    eprintln!("pipe diagnostic");
    println!("pipe child assertions completed");
    Ok(())
}

fn inheritable_fd(file: &std::fs::File, minimum: i32) -> anyhow::Result<OwnedFd> {
    // SAFETY: The live file is duplicated into a new descriptor without CLOEXEC.
    let raw = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD, minimum) };
    if raw == -1 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: fcntl returned a new owned descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(raw) })
}
