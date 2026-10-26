//! Exercises cross-process serialization and retrying an interrupted baseline reset.

use super::baseline_control;
use super::diff_since_latest_init;
use super::ensure_git_baseline_repository;
use super::reset_git_repository;
use super::tests::git_stdout;
use super::with_baseline_lock;
use anyhow::Result;
use pretty_assertions::assert_eq;
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

#[tokio::test]
async fn concurrent_operations_share_the_workspace_lock() -> Result<()> {
    const CHILD_ROOT: &str = "CODEX_TEST_BASELINE_LOCK_ROOT";
    const READY: &str = "holding workspace lock";
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        return with_baseline_lock(Path::new(&root), |_| {
            println!("{READY}");
            io::stdout().flush()?;
            io::stdin().read_exact(&mut [0])?;
            Ok(())
        })
        .await;
    }

    let workspace = tempfile::tempdir()?;
    let root = workspace.path();
    fs::write(root.join("MEMORY.md"), "original\n")?;
    reset_git_repository(root).await?;
    fs::write(root.join("MEMORY.md"), "updated\n")?;

    let mut child = codex_utils_process::background_command(std::env::current_exe()?)
        .args([
            "--exact",
            "baseline::recovery_tests::concurrent_operations_share_the_workspace_lock",
            "--nocapture",
        ])
        .env(CHILD_ROOT, root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let stdout = io::BufReader::new(child.stdout.as_mut().expect("piped stdout"));
    assert!(
        stdout
            .lines()
            .any(|line| line.is_ok_and(|line| line.contains(READY))),
        "child exited before acquiring the lock"
    );
    let lock = fs::File::options()
        .read(true)
        .write(true)
        .open(baseline_control(root).join("codex-baseline.lock"))?;
    let contended = matches!(lock.try_lock(), Err(fs::TryLockError::WouldBlock));
    drop(lock);

    let reset = reset_git_repository(root);
    let ensure = ensure_git_baseline_repository(root);
    let diff = diff_since_latest_init(root);
    tokio::pin!(reset, ensure, diff);
    // Any one operation completing before the holder releases the lock is a regression.
    let completed_while_locked = tokio::select! {
        _ = &mut reset => Some("reset"),
        _ = &mut ensure => Some("ensure"),
        _ = &mut diff => Some("diff"),
        () = tokio::time::sleep(Duration::from_millis(500)) => None,
    };
    child.stdin.take().expect("piped stdin").write_all(b"x")?;
    assert!(child.wait()?.success(), "lock holder failed");
    assert!(contended, "another process must own the lock");
    assert_eq!(completed_while_locked, None);

    let ((), (), diff) = tokio::try_join!(reset, ensure, diff)?;
    if diff.has_changes() {
        assert!(diff.unified_diff.contains("-original\n+updated"));
    }
    assert_eq!(git_stdout(root, &["show", "HEAD:MEMORY.md"]), "updated\n");
    assert_eq!(git_stdout(root, &["show", ":MEMORY.md"]), "updated\n");
    Ok(())
}

#[tokio::test]
async fn interrupted_reset_replays_working_files() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let root = workspace.path();
    let memory = root.join("MEMORY.md");
    fs::write(&memory, "original\n")?;
    reset_git_repository(root).await?;
    fs::write(&memory, "pending edit\n")?;

    // Simulate exiting after committing the tree but before writing the index and version.
    fs::remove_file(root.join(".git").join(super::BASELINE_VERSION_FILE))?;
    fs::remove_file(root.join(".git/index"))?;
    ensure_git_baseline_repository(root).await?;
    ensure_git_baseline_repository(root).await?;
    assert_eq!(
        git_stdout(root, &["ls-tree", "-r", "--name-only", "HEAD"]),
        ""
    );
    assert_eq!(git_stdout(root, &["ls-files"]), "");
    assert_eq!(fs::read_to_string(&memory)?, "pending edit\n");
    let diff = diff_since_latest_init(root).await?;
    assert_eq!(
        diff.changes,
        vec![super::GitBaselineChange {
            status: super::GitBaselineChangeStatus::Added,
            path: "MEMORY.md".to_string(),
        }]
    );
    assert!(diff.unified_diff.contains("+pending edit"));
    Ok(())
}
