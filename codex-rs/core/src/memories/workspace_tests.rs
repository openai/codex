use super::*;
use pretty_assertions::assert_eq;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn prepare_creates_repo_gitignore_and_initial_commit() {
    let home = TempDir::new().expect("tempdir");
    let memory_root = home.path().join("memories");
    fs::create_dir_all(&memory_root).expect("create memories");
    fs::write(memory_root.join("MEMORY.md"), "baseline").expect("write memory");

    prepare_git_repo(&memory_root).await.expect("prepare repo");

    assert!(memory_root.join(".git").is_dir());
    assert_eq!(
        fs::read_to_string(memory_root.join(GITIGNORE_FILENAME)).expect("read gitignore"),
        format!("{WORKSPACE_DIFF_FILENAME}\n")
    );
    assert!(!has_changes(&memory_root).await.expect("has changes"));
}

#[tokio::test]
async fn prepare_commits_gitignore_only_change_in_existing_repo() {
    let home = TempDir::new().expect("tempdir");
    let memory_root = home.path().join("memories");
    fs::create_dir_all(&memory_root).expect("create memories");
    fs::write(memory_root.join("MEMORY.md"), "baseline").expect("write memory");
    let repo = gix::init(&memory_root).expect("init repo");
    commit_current_tree(&repo, INITIAL_COMMIT_MESSAGE).expect("commit baseline");

    prepare_git_repo(&memory_root).await.expect("prepare repo");

    assert!(!has_changes(&memory_root).await.expect("has changes"));
    let repo = gix::open(&memory_root).expect("open repo");
    assert!(
        head_file_entries(&repo)
            .expect("head entries")
            .contains_key(GITIGNORE_FILENAME)
    );
}

#[tokio::test]
async fn writes_diff_and_commits_workspace_changes() {
    let home = TempDir::new().expect("tempdir");
    let memory_root = home.path().join("memories");
    fs::create_dir_all(&memory_root).expect("create memories");
    fs::write(memory_root.join("MEMORY.md"), "old").expect("write memory");
    prepare_git_repo(&memory_root).await.expect("prepare repo");
    fs::write(memory_root.join("MEMORY.md"), "new").expect("update memory");
    fs::write(memory_root.join("memory_summary.md"), "summary").expect("write summary");

    assert!(has_changes(&memory_root).await.expect("has changes"));

    write_workspace_diff(&memory_root)
        .await
        .expect("write workspace diff file");
    let workspace_diff = fs::read_to_string(memory_root.join(WORKSPACE_DIFF_FILENAME))
        .expect("read workspace diff file");
    assert!(workspace_diff.contains("- M MEMORY.md"));
    assert!(workspace_diff.contains("- A memory_summary.md"));
    assert!(workspace_diff.contains("diff --git a/MEMORY.md b/MEMORY.md"));
    assert!(workspace_diff.contains("-old"));
    assert!(workspace_diff.contains("+new"));
    assert!(workspace_diff.contains("diff --git a/memory_summary.md b/memory_summary.md"));
    assert!(workspace_diff.contains("+summary"));

    assert!(
        has_changes(&memory_root).await.expect("has changes"),
        "generated diff file should not affect workspace status"
    );

    commit_all(&memory_root).await.expect("commit workspace");
    assert!(!has_changes(&memory_root).await.expect("has changes"));
    assert!(
        memory_root.join(WORKSPACE_DIFF_FILENAME).is_file(),
        "generated diff file remains available but ignored after commit"
    );
}

#[tokio::test]
async fn remove_workspace_diff_ignores_missing_file() {
    let home = TempDir::new().expect("tempdir");
    let memory_root = home.path().join("memories");
    fs::create_dir_all(&memory_root).expect("create memories");

    remove_workspace_diff(&memory_root)
        .await
        .expect("remove missing workspace diff");
}

#[tokio::test]
async fn status_scan_does_not_write_added_file_blobs() {
    let home = TempDir::new().expect("tempdir");
    let memory_root = home.path().join("memories");
    prepare_git_repo(&memory_root).await.expect("prepare repo");
    let added_content = b"new uncommitted memory";
    fs::write(memory_root.join("MEMORY.md"), added_content).expect("write memory");

    assert!(has_changes(&memory_root).await.expect("has changes"));

    let repo = gix::open(&memory_root).expect("open repo");
    let added_oid = blob_oid(&repo, added_content).expect("compute added oid");
    assert!(
        repo.find_blob(added_oid).is_err(),
        "status scans should hash current files without writing loose git objects"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn reports_executable_bit_changes_as_modified() {
    use std::os::unix::fs::PermissionsExt;

    let home = TempDir::new().expect("tempdir");
    let memory_root = home.path().join("memories");
    fs::create_dir_all(&memory_root).expect("create memories");
    let path = memory_root.join("MEMORY.md");
    fs::write(&path, "same content").expect("write memory");
    prepare_git_repo(&memory_root).await.expect("prepare repo");
    let mut permissions = fs::metadata(&path).expect("stat memory").permissions();
    permissions.set_mode(permissions.mode() | 0o111);
    fs::set_permissions(&path, permissions).expect("chmod memory");

    assert!(has_changes(&memory_root).await.expect("has changes"));

    write_workspace_diff(&memory_root)
        .await
        .expect("write workspace diff file");
    let workspace_diff = fs::read_to_string(memory_root.join(WORKSPACE_DIFF_FILENAME))
        .expect("read workspace diff file");
    assert!(workspace_diff.contains("- M MEMORY.md"));
    assert!(workspace_diff.contains("old mode 100644"));
    assert!(workspace_diff.contains("new mode 100755"));
}

#[tokio::test]
async fn commit_all_creates_normal_parented_history() {
    let home = TempDir::new().expect("tempdir");
    let memory_root = home.path().join("memories");
    fs::create_dir_all(&memory_root).expect("create memories");
    fs::write(memory_root.join("MEMORY.md"), "old").expect("write memory");
    prepare_git_repo(&memory_root).await.expect("prepare repo");
    let first_head = gix::open(&memory_root)
        .expect("open repo")
        .head_id()
        .expect("first head")
        .detach();

    fs::write(memory_root.join("MEMORY.md"), "new").expect("update memory");
    commit_all(&memory_root).await.expect("commit workspace");
    let repo = gix::open(&memory_root).expect("open repo");
    let second_head = repo.head_id().expect("second head").detach();
    assert_ne!(first_head, second_head);
    let second_commit = repo.find_commit(second_head).expect("find second commit");

    assert_eq!(
        second_commit.parent_ids().collect::<Vec<_>>(),
        vec![first_head]
    );
}

#[tokio::test]
async fn workspace_diff_file_includes_deleted_head_content() {
    let home = TempDir::new().expect("tempdir");
    let memory_root = home.path().join("memories");
    fs::create_dir_all(memory_root.join("rollout_summaries")).expect("create rollout summaries");
    let summary_path = memory_root.join("rollout_summaries/deleted.md");
    fs::write(
        &summary_path,
        "thread_id: 00000000-0000-4000-8000-000000000001\nimportant stale evidence\n",
    )
    .expect("write rollout summary");
    prepare_git_repo(&memory_root).await.expect("prepare repo");
    fs::remove_file(&summary_path).expect("delete rollout summary");

    write_workspace_diff(&memory_root)
        .await
        .expect("write workspace diff file");

    let workspace_diff = fs::read_to_string(memory_root.join(WORKSPACE_DIFF_FILENAME))
        .expect("read workspace diff file");
    assert!(workspace_diff.contains("- D rollout_summaries/deleted.md"));
    assert!(workspace_diff.contains("deleted file mode 100644"));
    assert!(workspace_diff.contains("-thread_id: 00000000-0000-4000-8000-000000000001"));
    assert!(workspace_diff.contains("-important stale evidence"));
}
