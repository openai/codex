use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use futures::future::WeakShared;
use futures::future::join_all;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::GitStatusKey;
use super::get_has_changes_in_repo;
use super::git_status_key;
use super::git_status_runs;
use super::share_git_status_run;

#[tokio::test]
async fn status_requests_from_sibling_directories_use_the_known_worktree_root() {
    let repository = TempDir::new().expect("create repository");
    gix::init(repository.path()).expect("initialize repository");
    let first_directory = repository.path().join("first");
    let second_directory = repository.path().join("second");
    std::fs::create_dir(&first_directory).expect("create first directory");
    std::fs::create_dir(&second_directory).expect("create second directory");

    let results = tokio::join!(
        get_has_changes_in_repo(&first_directory, repository.path()),
        get_has_changes_in_repo(&second_directory, repository.path()),
    );

    assert_eq!(results, (Some(false), Some(false)));
}

#[cfg(unix)]
#[tokio::test]
async fn status_keys_coalesce_symlink_aliases_of_the_same_worktree() {
    let temp_dir = TempDir::new().expect("create temp directory");
    let repository = temp_dir.path().join("repository");
    let repository_alias = temp_dir.path().join("repository-alias");
    std::fs::create_dir(&repository).expect("create repository");
    std::os::unix::fs::symlink(&repository, &repository_alias).expect("create repository alias");

    let key = git_status_key(PathBuf::from("git"), &repository).await;
    let alias_key = git_status_key(PathBuf::from("git"), &repository_alias).await;

    assert_eq!(key, alias_key);
}

#[tokio::test]
async fn concurrent_status_requests_share_one_repository_scan() {
    let repository = TempDir::new().expect("create repository");
    let key = GitStatusKey {
        git: PathBuf::from("git"),
        repo_root: repository.path().to_path_buf(),
    };
    let scan_count = Arc::new(AtomicUsize::new(0));

    let results = join_all((0..32).map(|_| {
        let key = key.clone();
        let scan_count = Arc::clone(&scan_count);
        share_git_status_run(key, move || async move {
            scan_count.fetch_add(1, Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(25)).await;
            Some(true)
        })
    }))
    .await;

    assert_eq!(results, vec![Some(true); 32]);
    assert_eq!(scan_count.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn status_requests_do_not_share_different_repositories() {
    let first_repository = TempDir::new().expect("create first repository");
    let second_repository = TempDir::new().expect("create second repository");
    let scan_count = Arc::new(AtomicUsize::new(0));
    let first_key = git_status_key(PathBuf::from("git"), first_repository.path()).await;
    let second_key = git_status_key(PathBuf::from("git"), second_repository.path()).await;

    let first_scan_count = Arc::clone(&scan_count);
    let second_scan_count = Arc::clone(&scan_count);
    let results = tokio::join!(
        share_git_status_run(first_key, move || async move {
            first_scan_count.fetch_add(1, Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(25)).await;
            Some(false)
        }),
        share_git_status_run(second_key, move || async move {
            second_scan_count.fetch_add(1, Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(25)).await;
            Some(true)
        }),
    );

    assert_eq!(results, (Some(false), Some(true)));
    assert_eq!(scan_count.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn completed_status_requests_start_fresh_scans_and_prune_expired_entries() {
    let first_repository = TempDir::new().expect("create first repository");
    let first_key = GitStatusKey {
        git: PathBuf::from("git"),
        repo_root: first_repository.path().to_path_buf(),
    };
    let scan_count = Arc::new(AtomicUsize::new(0));

    let first_scan_count = Arc::clone(&scan_count);
    assert_eq!(
        share_git_status_run(first_key.clone(), move || async move {
            first_scan_count.fetch_add(1, Ordering::Relaxed);
            Some(true)
        })
        .await,
        Some(true)
    );

    let second_scan_count = Arc::clone(&scan_count);
    assert_eq!(
        share_git_status_run(first_key.clone(), move || async move {
            second_scan_count.fetch_add(1, Ordering::Relaxed);
            Some(false)
        })
        .await,
        Some(false)
    );
    assert_eq!(scan_count.load(Ordering::Relaxed), 2);

    let second_repository = TempDir::new().expect("create second repository");
    let third_scan_count = Arc::clone(&scan_count);
    assert_eq!(
        share_git_status_run(
            GitStatusKey {
                git: PathBuf::from("git"),
                repo_root: second_repository.path().to_path_buf(),
            },
            move || async move {
                third_scan_count.fetch_add(1, Ordering::Relaxed);
                Some(true)
            },
        )
        .await,
        Some(true)
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
    let key = GitStatusKey {
        git: PathBuf::from("git"),
        repo_root: repository.path().to_path_buf(),
    };
    let scan_count = Arc::new(AtomicUsize::new(0));
    let (release_sender, release_receiver) = tokio::sync::oneshot::channel();

    let first_key = key.clone();
    let first_scan_count = Arc::clone(&scan_count);
    let first = tokio::spawn(async move {
        share_git_status_run(first_key, move || async move {
            first_scan_count.fetch_add(1, Ordering::Relaxed);
            release_receiver.await.ok()?;
            Some(true)
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
    assert_eq!(first.await.expect("status task completed"), Some(true));
    assert_eq!(retained_result.peek(), Some(&Some(true)));

    let second_scan_count = Arc::clone(&scan_count);
    let result = share_git_status_run(key, move || async move {
        second_scan_count.fetch_add(1, Ordering::Relaxed);
        Some(false)
    })
    .await;

    assert_eq!(result, Some(false));
    assert_eq!(scan_count.load(Ordering::Relaxed), 2);
    assert_eq!(retained_result.peek(), Some(&Some(true)));
}

#[tokio::test]
async fn canceling_one_consumer_preserves_the_shared_status_scan() {
    let repository = TempDir::new().expect("create repository");
    let key = GitStatusKey {
        git: PathBuf::from("git"),
        repo_root: repository.path().to_path_buf(),
    };
    let scan_count = Arc::new(AtomicUsize::new(0));
    let (release_sender, release_receiver) = tokio::sync::oneshot::channel();

    let first_key = key.clone();
    let first_scan_count = Arc::clone(&scan_count);
    let first = tokio::spawn(async move {
        share_git_status_run(first_key, move || async move {
            first_scan_count.fetch_add(1, Ordering::Relaxed);
            release_receiver.await.ok()?;
            Some(true)
        })
        .await
    });

    tokio::task::yield_now().await;

    let second_key = key.clone();
    let second_scan_count = Arc::clone(&scan_count);
    let second = tokio::spawn(async move {
        share_git_status_run(second_key, move || async move {
            second_scan_count.fetch_add(1, Ordering::Relaxed);
            Some(false)
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
    assert_eq!(first.await.expect("status task completed"), Some(true));
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
    let key = GitStatusKey {
        git: PathBuf::from("git"),
        repo_root: repository.path().to_path_buf(),
    };
    let scan_count = Arc::new(AtomicUsize::new(0));
    let first_key = key.clone();
    let first_scan_count = Arc::clone(&scan_count);

    let first = tokio::spawn(async move {
        share_git_status_run(first_key, move || async move {
            first_scan_count.fetch_add(1, Ordering::Relaxed);
            std::future::pending::<Option<bool>>().await
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
        Some(false)
    })
    .await;

    assert_eq!(result, Some(false));
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
