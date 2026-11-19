use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use futures::future::WeakShared;
use futures::future::join_all;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::GitWorktreeStatus;
use super::canonical_worktree_root;
use super::change_probe_runs;
use super::get_has_changes_in_repo;
use super::get_worktree_status_in_repo;
use super::git_status_runs;
use super::parse_worktree_status;
use super::probe_worktree_changes;
use super::share_git_run;
use super::share_git_status_run;
use crate::GitSha;

const CLEAN_STATUS: Option<GitWorktreeStatus> = Some(GitWorktreeStatus {
    head_commit_hash: None,
    has_changes: false,
});
const DIRTY_STATUS: Option<GitWorktreeStatus> = Some(GitWorktreeStatus {
    head_commit_hash: None,
    has_changes: true,
});

#[tokio::test]
async fn head_probe_distinguishes_missing_commit_and_unavailable_repository() {
    let repository = TempDir::new().expect("create repository");
    assert_eq!(
        crate::get_head_state(repository.path()).await,
        crate::GitHeadState::Unavailable
    );
    gix::init(repository.path()).expect("initialize repository");
    std::fs::write(repository.path().join("HEAD"), "untracked").expect("write HEAD file");
    assert_eq!(
        crate::get_head_state(repository.path()).await,
        crate::GitHeadState::Missing
    );
    crate::ensure_git_baseline_repository(repository.path())
        .await
        .expect("create commit");
    let hash = crate::get_head_commit_hash(repository.path())
        .await
        .expect("read commit");
    assert_eq!(
        crate::get_head_state(repository.path()).await,
        crate::GitHeadState::Commit(hash)
    );
}

#[tokio::test]
async fn metadata_status_does_not_execute_submodule_clean_filter() {
    use std::path::Path;
    use std::process::Command;
    use std::time::SystemTime;

    fn git(cwd: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=Status Test",
                "-c",
                "user.email=status@example.com",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.autocrlf=false",
                "-c",
                if cfg!(windows) {
                    "core.hooksPath=NUL"
                } else {
                    "core.hooksPath=/dev/null"
                },
            ])
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let fixture = TempDir::new().expect("create fixture");
    let source = fixture.path().join("source");
    let repository = fixture.path().join("repository");
    gix::init(&source).expect("initialize submodule source");
    std::fs::write(source.join("tracked.txt"), "BASELINE\n").expect("write baseline");
    git(&source, &["add", "tracked.txt"]);
    git(&source, &["commit", "-m", "baseline"]);
    gix::init(&repository).expect("initialize parent repository");
    git(
        &repository,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "--",
            source.to_str().expect("source path"),
            "module",
        ],
    );
    git(&repository, &["commit", "-m", "add submodule"]);

    // Replace the submodule's .git pointer and configure an attacker-written filter.
    let module = repository.join("module");
    std::fs::remove_file(module.join(".git")).expect("remove git pointer");
    gix::init(&module).expect("initialize task-controlled git directory");
    git(&module, &["add", "tracked.txt"]);
    git(&module, &["commit", "-m", "task-created repository"]);
    let marker = fixture.path().join("filter-ran");
    git(
        &module,
        &[
            "config",
            "filter.task.clean",
            "printf ran > ../../filter-ran; cat",
        ],
    );
    std::fs::write(module.join(".gitattributes"), "tracked.txt filter=task\n")
        .expect("select task filter");
    let tracked_file = module.join("tracked.txt");
    std::fs::write(&tracked_file, "MODIFIED\n").expect("modify tracked file");
    // Windows requires a writable handle to change the modification time.
    std::fs::File::options()
        .write(true)
        .open(&tracked_file)
        .expect("open tracked file")
        .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000))
        .expect("invalidate cached file metadata");

    assert_eq!(
        get_has_changes_in_repo(&repository, &repository).await,
        Some(true)
    );
    assert!(!marker.exists(), "executed submodule clean filter");

    // Check that the same payload runs without the protective option.
    let output = Command::new("git")
        .args(["-c", "core.fsmonitor=false", "status", "--porcelain"])
        .env("GIT_OPTIONAL_LOCKS", "0")
        .current_dir(&repository)
        .output()
        .expect("run status without the fix");
    assert!(output.status.success());
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "ran");
}

#[tokio::test]
async fn status_requests_from_sibling_directories_use_the_known_worktree_root() {
    let repository = TempDir::new().expect("create repository");
    gix::init(repository.path()).expect("initialize repository");
    let first_directory = repository.path().join("first");
    let second_directory = repository.path().join("second");
    std::fs::create_dir(&first_directory).expect("create first directory");
    std::fs::create_dir(&second_directory).expect("create second directory");

    let results = tokio::join!(
        get_worktree_status_in_repo(&first_directory, repository.path()),
        get_has_changes_in_repo(&second_directory, repository.path()),
    );

    assert_eq!(results, (CLEAN_STATUS, Some(false)));
}

#[tokio::test]
async fn status_reports_the_commit_for_clean_and_dirty_worktrees() {
    let repository = TempDir::new().expect("create repository");
    let root = repository.path();
    crate::ensure_git_baseline_repository(root)
        .await
        .expect("create initial commit");
    let head = crate::get_head_commit_hash(root).await.expect("read HEAD");
    let mut expected = GitWorktreeStatus {
        head_commit_hash: Some(head),
        has_changes: false,
    };
    assert_eq!(
        get_worktree_status_in_repo(root, root).await,
        Some(expected.clone())
    );

    std::fs::write(root.join("untracked"), "worktree change").expect("create untracked file");
    expected.has_changes = true;
    assert_eq!(
        get_worktree_status_in_repo(root, root).await,
        Some(expected)
    );

    assert!(
        std::process::Command::new("git")
            .args(["config", "status.renames", "invalid"])
            .current_dir(root)
            .status()
            .expect("configure invalid status setting")
            .success()
    );
    assert_eq!(get_worktree_status_in_repo(root, root).await, None);
}

#[tokio::test]
async fn change_probe_only_returns_git_verified_positive_status() {
    let repository = TempDir::new().expect("create repository");
    let root = repository.path();
    crate::ensure_git_baseline_repository(root)
        .await
        .expect("create initial commit");
    let cwd = root.join("nested");
    std::fs::create_dir(&cwd).expect("create nested cwd");
    let head = crate::get_head_commit_hash(root).await.expect("read HEAD");
    assert_eq!(probe_worktree_changes(&cwd, root).await, None);

    // The pathspec must treat names containing Git pathspec syntax literally.
    let candidate = root.join("[recent]");
    std::fs::create_dir(&candidate).expect("create candidate directory");
    std::fs::write(candidate.join("leaf"), "new file").expect("create untracked leaf");
    assert_eq!(
        probe_worktree_changes(&cwd, root).await,
        Some((
            root.to_path_buf(),
            GitWorktreeStatus {
                head_commit_hash: Some(head),
                has_changes: true,
            }
        ))
    );

    assert!(
        std::process::Command::new("git")
            .args(["config", "status.showUntrackedFiles", "no"])
            .current_dir(root)
            .status()
            .expect("configure hidden untracked files")
            .success()
    );
    assert_eq!(probe_worktree_changes(&cwd, root).await, None);
}

#[tokio::test]
async fn concurrent_change_probes_share_one_scan() {
    let repository = TempDir::new().expect("create repository");
    let key = repository.path().to_path_buf();
    let scans = AtomicUsize::new(0);
    let (release, gate) = tokio::sync::oneshot::channel();
    let first = share_git_run(change_probe_runs(), key.clone(), || async move {
        gate.await.ok()?;
        DIRTY_STATUS
    });
    futures::pin_mut!(first);
    assert!(futures::poll!(&mut first).is_pending());
    let second = share_git_run(change_probe_runs(), key, || {
        scans.fetch_add(1, Ordering::Relaxed);
        async { CLEAN_STATUS }
    });
    futures::pin_mut!(second);
    assert!(futures::poll!(&mut second).is_pending());
    release.send(()).expect("release shared probe");
    assert_eq!(tokio::join!(first, second), (DIRTY_STATUS, DIRTY_STATUS));
    assert_eq!(scans.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn change_probes_do_not_share_sibling_cwds_or_full_status() {
    let repository = TempDir::new().expect("create repository");
    let root = repository.path().to_path_buf();
    let nested = root.join("nested");
    let (first, second, full) = tokio::join!(
        share_git_run(change_probe_runs(), root.clone(), || async {
            tokio::task::yield_now().await;
            CLEAN_STATUS
        }),
        share_git_run(change_probe_runs(), nested, || async {
            tokio::task::yield_now().await;
            DIRTY_STATUS
        }),
        share_git_status_run(root, || async {
            tokio::task::yield_now().await;
            DIRTY_STATUS
        }),
    );
    assert_eq!(
        (first, second, full),
        (CLEAN_STATUS, DIRTY_STATUS, DIRTY_STATUS)
    );
}

#[test]
fn porcelain_status_accepts_sha256_and_non_utf8_path_bytes() {
    let hash = "ab".repeat(32);
    let mut output = format!("# branch.head (detached)\n# branch.oid {hash}\n").into_bytes();
    output.extend_from_slice(b"? file-\xff\n");
    assert_eq!(
        parse_worktree_status(&output),
        Some(GitWorktreeStatus {
            head_commit_hash: Some(GitSha::new(&hash)),
            has_changes: true
        })
    );
}

#[test]
fn porcelain_status_requires_one_valid_head_observation() {
    for output in [
        b"# branch.head main\n".as_slice(),
        b"# branch.oid abc123\n",
        b"# branch.oid xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n",
        b"# branch.oid (initial)\n# branch.oid (initial)\n",
    ] {
        assert_eq!(parse_worktree_status(output), None);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn status_keys_coalesce_symlink_aliases_of_the_same_worktree() {
    let temp_dir = TempDir::new().expect("create temp directory");
    let repository = temp_dir.path().join("repository");
    let repository_alias = temp_dir.path().join("repository-alias");
    std::fs::create_dir(&repository).expect("create repository");
    std::os::unix::fs::symlink(&repository, &repository_alias).expect("create repository alias");

    let key = canonical_worktree_root(&repository).await;
    let alias_key = canonical_worktree_root(&repository_alias).await;

    assert_eq!(key, alias_key);
}

#[tokio::test]
async fn concurrent_status_requests_share_one_repository_scan() {
    let repository = TempDir::new().expect("create repository");
    let key = repository.path().to_path_buf();
    let scan_count = Arc::new(AtomicUsize::new(0));

    let results = join_all((0..32).map(|_| {
        let key = key.clone();
        let scan_count = Arc::clone(&scan_count);
        share_git_status_run(key, move || async move {
            scan_count.fetch_add(1, Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(25)).await;
            DIRTY_STATUS
        })
    }))
    .await;

    assert_eq!(results, vec![DIRTY_STATUS; 32]);
    assert_eq!(scan_count.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn status_requests_do_not_share_different_repositories() {
    let first_repository = TempDir::new().expect("create first repository");
    let second_repository = TempDir::new().expect("create second repository");
    let scan_count = Arc::new(AtomicUsize::new(0));
    let first_key = canonical_worktree_root(first_repository.path()).await;
    let second_key = canonical_worktree_root(second_repository.path()).await;

    let first_scan_count = Arc::clone(&scan_count);
    let second_scan_count = Arc::clone(&scan_count);
    let results = tokio::join!(
        share_git_status_run(first_key, move || async move {
            first_scan_count.fetch_add(1, Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(25)).await;
            CLEAN_STATUS
        }),
        share_git_status_run(second_key, move || async move {
            second_scan_count.fetch_add(1, Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(25)).await;
            DIRTY_STATUS
        }),
    );

    assert_eq!(results, (CLEAN_STATUS, DIRTY_STATUS));
    assert_eq!(scan_count.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn completed_status_requests_start_fresh_scans_and_prune_expired_entries() {
    let first_repository = TempDir::new().expect("create first repository");
    let first_key = first_repository.path().to_path_buf();
    let scan_count = Arc::new(AtomicUsize::new(0));

    let first_scan_count = Arc::clone(&scan_count);
    assert_eq!(
        share_git_status_run(first_key.clone(), move || async move {
            first_scan_count.fetch_add(1, Ordering::Relaxed);
            DIRTY_STATUS
        })
        .await,
        DIRTY_STATUS
    );

    let second_scan_count = Arc::clone(&scan_count);
    assert_eq!(
        share_git_status_run(first_key.clone(), move || async move {
            second_scan_count.fetch_add(1, Ordering::Relaxed);
            CLEAN_STATUS
        })
        .await,
        CLEAN_STATUS
    );
    assert_eq!(scan_count.load(Ordering::Relaxed), 2);

    let second_repository = TempDir::new().expect("create second repository");
    let third_scan_count = Arc::clone(&scan_count);
    assert_eq!(
        share_git_status_run(second_repository.path().to_path_buf(), move || async move {
            third_scan_count.fetch_add(1, Ordering::Relaxed);
            DIRTY_STATUS
        })
        .await,
        DIRTY_STATUS
    );
    assert_eq!(scan_count.load(Ordering::Relaxed), 3);
    assert!(
        !git_status_runs()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains_key(&first_key)
    );
}

#[tokio::test]
async fn completed_status_requests_are_not_reused_while_a_consumer_retains_them() {
    let repository = TempDir::new().expect("create repository");
    let key = repository.path().to_path_buf();
    let scan_count = Arc::new(AtomicUsize::new(0));
    let (release_sender, release_receiver) = tokio::sync::oneshot::channel();

    let first_key = key.clone();
    let first_scan_count = Arc::clone(&scan_count);
    let first = tokio::spawn(async move {
        share_git_status_run(first_key, move || async move {
            first_scan_count.fetch_add(1, Ordering::Relaxed);
            release_receiver.await.ok()?;
            DIRTY_STATUS
        })
        .await
    });

    tokio::task::yield_now().await;
    let retained_result = git_status_runs()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&key)
        .and_then(WeakShared::upgrade)
        .expect("retain the pending shared status scan");

    release_sender.send(()).expect("release shared status scan");
    assert_eq!(first.await.expect("status task completed"), DIRTY_STATUS);
    assert_eq!(retained_result.peek(), Some(&DIRTY_STATUS));

    let second_scan_count = Arc::clone(&scan_count);
    let result = share_git_status_run(key, move || async move {
        second_scan_count.fetch_add(1, Ordering::Relaxed);
        CLEAN_STATUS
    })
    .await;

    assert_eq!(result, CLEAN_STATUS);
    assert_eq!(scan_count.load(Ordering::Relaxed), 2);
    assert_eq!(retained_result.peek(), Some(&DIRTY_STATUS));
}

#[tokio::test]
async fn canceling_one_consumer_preserves_the_shared_status_scan() {
    let repository = TempDir::new().expect("create repository");
    let key = repository.path().to_path_buf();
    let scan_count = Arc::new(AtomicUsize::new(0));
    let (release_sender, release_receiver) = tokio::sync::oneshot::channel();

    let first_key = key.clone();
    let first_scan_count = Arc::clone(&scan_count);
    let first = tokio::spawn(async move {
        share_git_status_run(first_key, move || async move {
            first_scan_count.fetch_add(1, Ordering::Relaxed);
            release_receiver.await.ok()?;
            DIRTY_STATUS
        })
        .await
    });

    tokio::task::yield_now().await;

    let second_key = key.clone();
    let second_scan_count = Arc::clone(&scan_count);
    let second = tokio::spawn(async move {
        share_git_status_run(second_key, move || async move {
            second_scan_count.fetch_add(1, Ordering::Relaxed);
            CLEAN_STATUS
        })
        .await
    });

    tokio::task::yield_now().await;
    second.abort();
    assert!(
        second
            .await
            .expect_err("consumer was canceled")
            .is_cancelled()
    );
    assert_eq!(scan_count.load(Ordering::Relaxed), 1);

    release_sender.send(()).expect("release shared status scan");
    assert_eq!(first.await.expect("status task completed"), DIRTY_STATUS);
    assert!(
        git_status_runs()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&key)
            .and_then(WeakShared::upgrade)
            .is_none()
    );
}

#[tokio::test]
async fn canceling_the_last_consumer_allows_a_fresh_status_scan() {
    let repository = TempDir::new().expect("create repository");
    let key = repository.path().to_path_buf();
    let scan_count = Arc::new(AtomicUsize::new(0));
    let first_key = key.clone();
    let first_scan_count = Arc::clone(&scan_count);

    let first = tokio::spawn(async move {
        share_git_status_run(first_key, move || async move {
            first_scan_count.fetch_add(1, Ordering::Relaxed);
            std::future::pending().await
        })
        .await
    });

    tokio::task::yield_now().await;
    assert_eq!(scan_count.load(Ordering::Relaxed), 1);
    first.abort();
    assert!(
        first
            .await
            .expect_err("consumer was canceled")
            .is_cancelled()
    );

    let second_scan_count = Arc::clone(&scan_count);
    let result = share_git_status_run(key.clone(), move || async move {
        second_scan_count.fetch_add(1, Ordering::Relaxed);
        CLEAN_STATUS
    })
    .await;

    assert_eq!(result, CLEAN_STATUS);
    assert_eq!(scan_count.load(Ordering::Relaxed), 2);
    assert!(
        git_status_runs()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&key)
            .and_then(WeakShared::upgrade)
            .is_none()
    );
}
