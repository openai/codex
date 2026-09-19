//! Exercise native filesystem-helper spawning with real sandbox and fd-transfer operations.
//! Atfork registration stays in an isolated process because its hooks cannot be removed.

#![cfg(target_os = "macos")]

mod common;
#[path = "file_system/support.rs"]
#[allow(dead_code, clippy::expect_used)]
mod support;

use std::io;
use std::os::unix::fs::symlink;
use std::os::unix::process::CommandExt;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_utils_path_uri::PathUri;
use futures::TryStreamExt;
use pretty_assertions::assert_eq;
use tokio::process::Command;
use tokio::time::timeout;

use crate::support::FileSystemImplementation;
use crate::support::create_file_system_context;
use crate::support::workspace_write_sandbox;

const CHILD_FLAG: &str = "CODEX_EXEC_SERVER_MACOS_FS_SPAWN_TEST_CHILD";
const CHILD_COMPLETED: &str = "filesystem spawn assertions completed";
static PARENT_FORKS: AtomicUsize = AtomicUsize::new(0);

extern "C" fn record_parent_fork() {
    PARENT_FORKS.fetch_add(1, Ordering::Relaxed);
}

#[tokio::test]
async fn sandboxed_filesystem_operations_avoid_fork() -> Result<()> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args([
            "--exact",
            "sandboxed_filesystem_operations_avoid_fork_child",
            "--ignored",
            "--test-threads=1",
            "--nocapture",
        ])
        .env(CHILD_FLAG, "1")
        .kill_on_drop(true);
    let output = timeout(Duration::from_secs(/*secs*/ 60), command.output()).await??;
    assert!(
        output.status.success()
            && String::from_utf8_lossy(&output.stdout).contains(CHILD_COMPLETED),
        "filesystem spawn test failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated child for sandboxed_filesystem_operations_avoid_fork"]
async fn sandboxed_filesystem_operations_avoid_fork_child() -> Result<()> {
    assert_eq!(std::env::var(CHILD_FLAG)?, "1");
    let context = create_file_system_context(FileSystemImplementation::Local).await?;
    let file_system = &context.file_system;
    let directory = tempfile::tempdir()?;
    let root = directory.path().canonicalize()?;
    let allowed = root.join("allowed");
    let outside = root.join("outside");
    std::fs::create_dir(&allowed)?;
    std::fs::create_dir(&outside)?;
    let secret = outside.join("secret.txt");
    std::fs::write(&secret, b"secret")?;
    symlink(&outside, allowed.join("escape"))?;
    let sandbox = workspace_write_sandbox(allowed.clone());
    let path = allowed.join("file.txt");
    let uri = PathUri::from_host_native_path(&path)?;

    // SAFETY: This isolated process runs one test, and the parent hook only updates a
    // lock-free atomic. The hook remains valid until this process exits.
    let registered = unsafe {
        libc::pthread_atfork(
            /*prepare*/ None,
            Some(record_parent_fork),
            /*child*/ None,
        )
    };
    assert_eq!(registered, 0);
    let mut legacy = std::process::Command::new("/usr/bin/true");
    // SAFETY: A no-op callback forces the legacy fork route for this positive control.
    unsafe {
        legacy.pre_exec(|| Ok(()));
    }
    assert!(legacy.status()?.success());
    let baseline = PARENT_FORKS.load(Ordering::Relaxed);
    assert_eq!(baseline, 1, "the positive control must detect fork");

    file_system
        .write_file(
            &uri,
            b"allowed".to_vec(),
            Default::default(),
            Some(&sandbox),
        )
        .await?;
    assert_eq!(
        file_system
            .read_file(&uri, Default::default(), Some(&sandbox))
            .await?,
        b"allowed",
    );
    // The transferred fd must retain its file after the path is replaced.
    let stream = file_system.read_file_stream(&uri, Some(&sandbox)).await?;
    std::fs::rename(&path, allowed.join("opened.txt"))?;
    std::fs::write(&path, b"replacement")?;
    assert_eq!(stream.try_collect::<Vec<_>>().await?.concat(), b"allowed",);

    for denied in [secret.clone(), allowed.join("escape/secret.txt")] {
        assert_eq!(std::fs::read(&denied)?, b"secret");
        let denied = PathUri::from_host_native_path(denied)?;
        let read_error = file_system
            .read_file(&denied, Default::default(), Some(&sandbox))
            .await
            .expect_err("sandboxed read must reject an outside file");
        assert_sandbox_denied(&read_error);
        let stream_error = file_system
            .read_file_stream(&denied, Some(&sandbox))
            .await
            .err()
            .context("sandboxed stream must reject an outside file")?;
        assert_sandbox_denied(&stream_error);
        let write_error = file_system
            .write_file(
                &denied,
                b"changed".to_vec(),
                Default::default(),
                Some(&sandbox),
            )
            .await
            .expect_err("sandboxed write must reject an outside file");
        assert_sandbox_denied(&write_error);
    }
    assert_eq!(std::fs::read(&secret)?, b"secret");
    assert_eq!(
        PARENT_FORKS.load(Ordering::Relaxed),
        baseline,
        "sandboxed reads, writes, and fd transfers must avoid fork",
    );
    println!("{CHILD_COMPLETED}");
    Ok(())
}

fn assert_sandbox_denied(error: &io::Error) {
    assert!(
        matches!(
            error.kind(),
            io::ErrorKind::PermissionDenied | io::ErrorKind::InvalidInput
        ),
        "expected a sandbox denial, got {error:?}",
    );
    let message = error.to_string();
    assert!(
        message.contains("is not permitted")
            || message.contains("Operation not permitted")
            || message.contains("Permission denied"),
        "expected a permission denial, got {message}",
    );
}
