use super::*;
use codex_apply_patch::AppliedPatchDelta;
use codex_apply_patch::MaybeApplyPatchVerified;
use codex_exec_server::LOCAL_FS;
use codex_git_utils::ApplyGitRequest;
use codex_git_utils::apply_git_patch;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;
use tempfile::tempdir;

fn git_blob_sha1_hex(data: &str) -> String {
    format!("{:x}", git_blob_sha1_hex_bytes(data.as_bytes()))
}

async fn apply_verified_patch(root: &Path, patch: &str) -> AppliedPatchDelta {
    let cwd = AbsolutePathBuf::from_absolute_path(root).expect("absolute tempdir path");
    let argv = vec!["apply_patch".to_string(), patch.to_string()];
    match codex_apply_patch::maybe_parse_apply_patch_verified(
        &argv,
        &cwd,
        LOCAL_FS.as_ref(),
        /*sandbox*/ None,
    )
    .await
    {
        MaybeApplyPatchVerified::Body(_) => {}
        other => panic!("expected verified patch action, got {other:?}"),
    }

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    codex_apply_patch::apply_patch(
        patch,
        &cwd,
        &mut stdout,
        &mut stderr,
        LOCAL_FS.as_ref(),
        /*sandbox*/ None,
    )
    .await
    .expect("patch should apply")
}

#[tokio::test]
async fn accumulates_add_then_update_as_single_add() {
    let dir = tempdir().expect("tempdir");
    let mut tracker = TurnDiffTracker::with_display_root(dir.path().to_path_buf());

    let add = apply_verified_patch(
        dir.path(),
        "*** Begin Patch\n*** Add File: a.txt\n+foo\n*** End Patch",
    )
    .await;
    tracker.track_delta(&add);

    let update = apply_verified_patch(
        dir.path(),
        "*** Begin Patch\n*** Update File: a.txt\n@@\n foo\n+bar\n*** End Patch",
    )
    .await;
    tracker.track_delta(&update);

    let right_oid = git_blob_sha1_hex("foo\nbar\n");
    let expected = format!(
        r#"diff --git a/a.txt b/a.txt
new file mode {REGULAR_FILE_MODE}
index {ZERO_OID}..{right_oid}
--- {DEV_NULL}
+++ b/a.txt
@@ -0,0 +1,2 @@
+foo
+bar
"#,
    );
    assert_eq!(tracker.get_unified_diff(), Some(expected));
}

#[tokio::test]
async fn invalidated_tracker_suppresses_existing_diff() {
    let dir = tempdir().expect("tempdir");
    let mut tracker = TurnDiffTracker::with_display_root(dir.path().to_path_buf());

    let add = apply_verified_patch(
        dir.path(),
        "*** Begin Patch\n*** Add File: a.txt\n+foo\n*** End Patch",
    )
    .await;
    tracker.track_delta(&add);

    tracker.invalidate();

    assert_eq!(tracker.get_unified_diff(), None);
}

#[tokio::test]
async fn accumulates_delete() {
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("b.txt"), "x\n").expect("seed file");

    let mut tracker = TurnDiffTracker::with_display_root(dir.path().to_path_buf());
    let delete = apply_verified_patch(
        dir.path(),
        "*** Begin Patch\n*** Delete File: b.txt\n*** End Patch",
    )
    .await;
    tracker.track_delta(&delete);

    let left_oid = git_blob_sha1_hex("x\n");
    let expected = format!(
        r#"diff --git a/b.txt b/b.txt
deleted file mode {REGULAR_FILE_MODE}
index {left_oid}..{ZERO_OID}
--- a/b.txt
+++ {DEV_NULL}
@@ -1 +0,0 @@
-x
"#,
    );
    assert_eq!(tracker.get_unified_diff(), Some(expected));
}

#[tokio::test]
async fn accumulates_move_and_update() {
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("src.txt"), "line\n").expect("seed file");

    let mut tracker = TurnDiffTracker::with_display_root(dir.path().to_path_buf());
    let update = apply_verified_patch(
        dir.path(),
        "*** Begin Patch\n*** Update File: src.txt\n*** Move to: dst.txt\n@@\n-line\n+line2\n*** End Patch",
    )
    .await;
    tracker.track_delta(&update);

    let left_oid = git_blob_sha1_hex("line\n");
    let right_oid = git_blob_sha1_hex("line2\n");
    let expected = format!(
        r#"diff --git a/src.txt b/dst.txt
index {left_oid}..{right_oid}
--- a/src.txt
+++ b/dst.txt
@@ -1 +1 @@
-line
+line2
"#,
    );
    assert_eq!(tracker.get_unified_diff(), Some(expected));
}

#[tokio::test]
async fn pure_rename_yields_no_diff() {
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("old.txt"), "same\n").expect("seed file");

    let mut tracker = TurnDiffTracker::with_display_root(dir.path().to_path_buf());
    let rename = apply_verified_patch(
        dir.path(),
        "*** Begin Patch\n*** Update File: old.txt\n*** Move to: new.txt\n@@\n same\n*** End Patch",
    )
    .await;
    tracker.track_delta(&rename);

    assert_eq!(tracker.get_unified_diff(), None);
}

#[tokio::test]
async fn add_over_existing_file_becomes_update() {
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("dup.txt"), "before\n").expect("seed file");

    let mut tracker = TurnDiffTracker::with_display_root(dir.path().to_path_buf());
    let add = apply_verified_patch(
        dir.path(),
        "*** Begin Patch\n*** Add File: dup.txt\n+after\n*** End Patch",
    )
    .await;
    tracker.track_delta(&add);

    let left_oid = git_blob_sha1_hex("before\n");
    let right_oid = git_blob_sha1_hex("after\n");
    let expected = format!(
        r#"diff --git a/dup.txt b/dup.txt
index {left_oid}..{right_oid}
--- a/dup.txt
+++ b/dup.txt
@@ -1 +1 @@
-before
+after
"#,
    );
    assert_eq!(tracker.get_unified_diff(), Some(expected));
}

#[tokio::test]
async fn delete_then_readd_same_path_becomes_update() {
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("cycle.txt"), "before\n").expect("seed file");

    let mut tracker = TurnDiffTracker::with_display_root(dir.path().to_path_buf());
    let delete = apply_verified_patch(
        dir.path(),
        "*** Begin Patch\n*** Delete File: cycle.txt\n*** End Patch",
    )
    .await;
    tracker.track_delta(&delete);

    let add = apply_verified_patch(
        dir.path(),
        "*** Begin Patch\n*** Add File: cycle.txt\n+after\n*** End Patch",
    )
    .await;
    tracker.track_delta(&add);

    let left_oid = git_blob_sha1_hex("before\n");
    let right_oid = git_blob_sha1_hex("after\n");
    let expected = format!(
        r#"diff --git a/cycle.txt b/cycle.txt
index {left_oid}..{right_oid}
--- a/cycle.txt
+++ b/cycle.txt
@@ -1 +1 @@
-before
+after
"#,
    );
    assert_eq!(tracker.get_unified_diff(), Some(expected));
}

#[tokio::test]
async fn move_over_existing_destination_without_content_change_deletes_source_only() {
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("a.txt"), "same\n").expect("seed source");
    fs::write(dir.path().join("b.txt"), "same\n").expect("seed destination");

    let mut tracker = TurnDiffTracker::with_display_root(dir.path().to_path_buf());
    let move_overwrite = apply_verified_patch(
        dir.path(),
        "*** Begin Patch\n*** Update File: a.txt\n*** Move to: b.txt\n@@\n same\n*** End Patch",
    )
    .await;
    tracker.track_delta(&move_overwrite);

    let left_oid = git_blob_sha1_hex("same\n");
    let expected = format!(
        r#"diff --git a/a.txt b/a.txt
deleted file mode {REGULAR_FILE_MODE}
index {left_oid}..{ZERO_OID}
--- a/a.txt
+++ {DEV_NULL}
@@ -1 +0,0 @@
-same
"#,
    );
    assert_eq!(tracker.get_unified_diff(), Some(expected));
}

#[tokio::test]
async fn move_over_existing_destination_with_content_change_deletes_source_and_updates_destination()
{
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("a.txt"), "from\n").expect("seed source");
    fs::write(dir.path().join("b.txt"), "existing\n").expect("seed destination");

    let mut tracker = TurnDiffTracker::with_display_root(dir.path().to_path_buf());
    let move_overwrite = apply_verified_patch(
        dir.path(),
        "*** Begin Patch\n*** Update File: a.txt\n*** Move to: b.txt\n@@\n-from\n+new\n*** End Patch",
    )
    .await;
    tracker.track_delta(&move_overwrite);

    let left_oid_a = git_blob_sha1_hex("from\n");
    let left_oid_b = git_blob_sha1_hex("existing\n");
    let right_oid_b = git_blob_sha1_hex("new\n");
    let expected = format!(
        r#"diff --git a/a.txt b/a.txt
deleted file mode {REGULAR_FILE_MODE}
index {left_oid_a}..{ZERO_OID}
--- a/a.txt
+++ {DEV_NULL}
@@ -1 +0,0 @@
-from
diff --git a/b.txt b/b.txt
index {left_oid_b}..{right_oid_b}
--- a/b.txt
+++ b/b.txt
@@ -1 +1 @@
-existing
+new
"#,
    );
    assert_eq!(tracker.get_unified_diff(), Some(expected));
}

#[tokio::test]
async fn preserves_committed_change_order_with_delete_then_move_overwrite() {
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("a.txt"), "from\n").expect("seed source");
    fs::write(dir.path().join("b.txt"), "existing\n").expect("seed destination");

    let mut tracker = TurnDiffTracker::with_display_root(dir.path().to_path_buf());
    let ordered_patch = apply_verified_patch(
        dir.path(),
        "*** Begin Patch\n*** Delete File: b.txt\n*** Update File: a.txt\n*** Move to: b.txt\n@@\n-from\n+new\n*** End Patch",
    )
    .await;
    tracker.track_delta(&ordered_patch);

    let left_oid_a = git_blob_sha1_hex("from\n");
    let left_oid_b = git_blob_sha1_hex("existing\n");
    let right_oid_b = git_blob_sha1_hex("new\n");
    let expected = format!(
        r#"diff --git a/a.txt b/a.txt
deleted file mode {REGULAR_FILE_MODE}
index {left_oid_a}..{ZERO_OID}
--- a/a.txt
+++ {DEV_NULL}
@@ -1 +0,0 @@
-from
diff --git a/b.txt b/b.txt
index {left_oid_b}..{right_oid_b}
--- a/b.txt
+++ b/b.txt
@@ -1 +1 @@
-existing
+new
"#,
    );
    assert_eq!(tracker.get_unified_diff(), Some(expected));
}

#[tokio::test]
async fn reuses_rendered_diffs_for_unchanged_paths() {
    let dir = tempdir().expect("tempdir");
    let mut tracker = TurnDiffTracker::with_display_root(dir.path().to_path_buf());

    let add_a = apply_verified_patch(
        dir.path(),
        "*** Begin Patch\n*** Add File: a.txt\n+one\n*** End Patch",
    )
    .await;
    tracker.track_delta(&add_a);
    assert_eq!(tracker.rendered_diff_count(), 1);

    let add_b = apply_verified_patch(
        dir.path(),
        "*** Begin Patch\n*** Add File: b.txt\n+two\n*** End Patch",
    )
    .await;
    tracker.track_delta(&add_b);

    assert_eq!(tracker.rendered_diff_count(), 2);
    assert_eq!(
        tracker.get_unified_diff(),
        tracker.get_unified_diff(),
        "reading the cached aggregate must not render file diffs",
    );
    assert_eq!(tracker.rendered_diff_count(), 2);
}

#[tokio::test]
async fn repeated_updates_only_rerender_the_touched_path() {
    let dir = tempdir().expect("tempdir");
    let mut tracker = TurnDiffTracker::with_display_root(dir.path().to_path_buf());

    for patch in [
        "*** Begin Patch\n*** Add File: stable.txt\n+stable\n*** End Patch".to_string(),
        "*** Begin Patch\n*** Add File: hot.txt\n+value 0\n*** End Patch".to_string(),
    ] {
        tracker.track_delta(&apply_verified_patch(dir.path(), &patch).await);
    }

    for value in 1..=40 {
        let patch = format!(
            "*** Begin Patch\n*** Update File: hot.txt\n@@\n-value {}\n+value {value}\n*** End Patch",
            value - 1,
        );
        tracker.track_delta(&apply_verified_patch(dir.path(), &patch).await);
    }

    assert_eq!(tracker.rendered_diff_count(), 42);
}

#[test]
fn large_rewrite_returns_promptly_and_preserves_exact_content() {
    let dir = tempdir().expect("tempdir");
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(dir.path())
            .status()
            .expect("run git init")
            .success()
    );
    let old_content = (0..48_000)
        .map(|line| format!("old line {line:05}\n"))
        .collect::<String>();
    let new_content = (0..48_000)
        .map(|line| format!("new line {line:05}\n"))
        .collect::<String>();
    let path = dir.path().join("large.txt");
    fs::write(&path, &old_content).expect("seed large file");
    assert!(
        Command::new("git")
            .args(["add", "large.txt"])
            .current_dir(dir.path())
            .status()
            .expect("run git add")
            .success()
    );
    let tracker = TurnDiffTracker::with_display_root(dir.path().to_path_buf());

    let started = Instant::now();
    let diff = tracker
        .render_diff(&path, Some(&old_content), &path, Some(&new_content))
        .expect("complete rewrite should produce a diff");

    assert!(
        started.elapsed() < Duration::from_secs(2),
        "large rewrite took {:?}",
        started.elapsed(),
    );
    let result = apply_git_patch(&ApplyGitRequest {
        cwd: dir.path().to_path_buf(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect("apply generated diff");
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert_eq!(
        fs::read_to_string(path).expect("read large file"),
        new_content
    );
}
