use super::*;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

/// Compute the Git SHA-1 blob object ID for the given content (string).
/// This delegates to the bytes version to avoid UTF-8 lossy conversions here.
fn git_blob_sha1_hex(data: &str) -> String {
    format!("{:x}", git_blob_sha1_hex_bytes(data.as_bytes()))
}

fn normalize_diff_for_test(input: &str, root: &Path) -> String {
    let root_str = root.display().to_string().replace('\\', "/");
    let replaced = input.replace(&root_str, "<TMP>");
    // Split into blocks on lines starting with "diff --git ", sort blocks for determinism, and rejoin
    let mut blocks: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in replaced.lines() {
        if line.starts_with("diff --git ") && !current.is_empty() {
            blocks.push(current);
            current = String::new();
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks.sort();
    let mut out = blocks.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[test]
fn accumulates_add_and_update() {
    let mut acc = TurnDiffTracker::new();

    let dir = tempdir().unwrap();
    let file = dir.path().join("a.txt");

    // First patch: add file (baseline should be /dev/null).
    let add_changes = HashMap::from([(
        file.clone(),
        FileChange::Add {
            content: "foo\n".to_string(),
        },
    )]);
    acc.on_patch_begin(&add_changes);

    // Simulate apply: create the file on disk.
    fs::write(&file, "foo\n").unwrap();
    let first = acc.get_unified_diff().unwrap().unwrap();
    let first = normalize_diff_for_test(&first, dir.path());
    let expected_first = {
        let mode = file_mode_for_path(&file).unwrap_or(FileMode::Regular);
        let right_oid = git_blob_sha1_hex("foo\n");
        format!(
            r#"diff --git a/<TMP>/a.txt b/<TMP>/a.txt
new file mode {mode}
index {ZERO_OID}..{right_oid}
--- {DEV_NULL}
+++ b/<TMP>/a.txt
@@ -0,0 +1 @@
+foo
"#,
        )
    };
    assert_eq!(first, expected_first);

    // Second patch: update the file on disk.
    let update_changes = HashMap::from([(
        file.clone(),
        FileChange::Update {
            unified_diff: "".to_owned(),
            move_path: None,
        },
    )]);
    acc.on_patch_begin(&update_changes);

    // Simulate apply: append a new line.
    fs::write(&file, "foo\nbar\n").unwrap();
    let combined = acc.get_unified_diff().unwrap().unwrap();
    let combined = normalize_diff_for_test(&combined, dir.path());
    let expected_combined = {
        let mode = file_mode_for_path(&file).unwrap_or(FileMode::Regular);
        let right_oid = git_blob_sha1_hex("foo\nbar\n");
        format!(
            r#"diff --git a/<TMP>/a.txt b/<TMP>/a.txt
new file mode {mode}
index {ZERO_OID}..{right_oid}
--- {DEV_NULL}
+++ b/<TMP>/a.txt
@@ -0,0 +1,2 @@
+foo
+bar
"#,
        )
    };
    assert_eq!(combined, expected_combined);
}

#[test]
fn accumulates_delete() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("b.txt");
    fs::write(&file, "x\n").unwrap();

    let mut acc = TurnDiffTracker::new();
    let del_changes = HashMap::from([(
        file.clone(),
        FileChange::Delete {
            content: "x\n".to_string(),
        },
    )]);
    acc.on_patch_begin(&del_changes);

    // Simulate apply: delete the file from disk.
    let baseline_mode = file_mode_for_path(&file).unwrap_or(FileMode::Regular);
    fs::remove_file(&file).unwrap();
    let diff = acc.get_unified_diff().unwrap().unwrap();
    let diff = normalize_diff_for_test(&diff, dir.path());
    let expected = {
        let left_oid = git_blob_sha1_hex("x\n");
        format!(
            r#"diff --git a/<TMP>/b.txt b/<TMP>/b.txt
deleted file mode {baseline_mode}
index {left_oid}..{ZERO_OID}
--- a/<TMP>/b.txt
+++ {DEV_NULL}
@@ -1 +0,0 @@
-x
"#,
        )
    };
    assert_eq!(diff, expected);
}

#[test]
fn accumulates_move_and_update() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("src.txt");
    let dest = dir.path().join("dst.txt");
    fs::write(&src, "line\n").unwrap();

    let mut acc = TurnDiffTracker::new();
    let mv_changes = HashMap::from([(
        src.clone(),
        FileChange::Update {
            unified_diff: "".to_owned(),
            move_path: Some(dest.clone()),
        },
    )]);
    acc.on_patch_begin(&mv_changes);

    // Simulate apply: move and update content.
    fs::rename(&src, &dest).unwrap();
    fs::write(&dest, "line2\n").unwrap();

    let out = acc.get_unified_diff().unwrap().unwrap();
    let out = normalize_diff_for_test(&out, dir.path());
    let expected = {
        let left_oid = git_blob_sha1_hex("line\n");
        let right_oid = git_blob_sha1_hex("line2\n");
        format!(
            r#"diff --git a/<TMP>/src.txt b/<TMP>/dst.txt
index {left_oid}..{right_oid}
--- a/<TMP>/src.txt
+++ b/<TMP>/dst.txt
@@ -1 +1 @@
-line
+line2
"#
        )
    };
    assert_eq!(out, expected);
}

#[test]
fn move_without_1change_yields_no_diff() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("moved.txt");
    let dest = dir.path().join("renamed.txt");
    fs::write(&src, "same\n").unwrap();

    let mut acc = TurnDiffTracker::new();
    let mv_changes = HashMap::from([(
        src.clone(),
        FileChange::Update {
            unified_diff: "".to_owned(),
            move_path: Some(dest.clone()),
        },
    )]);
    acc.on_patch_begin(&mv_changes);

    // Simulate apply: move only, no content change.
    fs::rename(&src, &dest).unwrap();

    let diff = acc.get_unified_diff().unwrap();
    assert_eq!(diff, None);
}

#[test]
fn move_declared_but_file_only_appears_at_dest_is_add() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("src.txt");
    let dest = dir.path().join("dest.txt");
    let mut acc = TurnDiffTracker::new();
    let mv = HashMap::from([(
        src,
        FileChange::Update {
            unified_diff: "".into(),
            move_path: Some(dest.clone()),
        },
    )]);
    acc.on_patch_begin(&mv);
    // No file existed initially; create only dest
    fs::write(&dest, "hello\n").unwrap();
    let diff = acc.get_unified_diff().unwrap().unwrap();
    let diff = normalize_diff_for_test(&diff, dir.path());
    let expected = {
        let mode = file_mode_for_path(&dest).unwrap_or(FileMode::Regular);
        let right_oid = git_blob_sha1_hex("hello\n");
        format!(
            r#"diff --git a/<TMP>/src.txt b/<TMP>/dest.txt
new file mode {mode}
index {ZERO_OID}..{right_oid}
--- {DEV_NULL}
+++ b/<TMP>/dest.txt
@@ -0,0 +1 @@
+hello
"#,
        )
    };
    assert_eq!(diff, expected);
}

#[test]
fn update_persists_across_new_baseline_for_new_file() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, "foo\n").unwrap();
    fs::write(&b, "z\n").unwrap();

    let mut acc = TurnDiffTracker::new();

    // First: update existing a.txt (baseline snapshot is created for a).
    let update_a = HashMap::from([(
        a.clone(),
        FileChange::Update {
            unified_diff: "".to_owned(),
            move_path: None,
        },
    )]);
    acc.on_patch_begin(&update_a);
    // Simulate apply: modify a.txt on disk.
    fs::write(&a, "foo\nbar\n").unwrap();
    let first = acc.get_unified_diff().unwrap().unwrap();
    let first = normalize_diff_for_test(&first, dir.path());
    let expected_first = {
        let left_oid = git_blob_sha1_hex("foo\n");
        let right_oid = git_blob_sha1_hex("foo\nbar\n");
        format!(
            r#"diff --git a/<TMP>/a.txt b/<TMP>/a.txt
index {left_oid}..{right_oid}
--- a/<TMP>/a.txt
+++ b/<TMP>/a.txt
@@ -1 +1,2 @@
 foo
+bar
"#
        )
    };
    assert_eq!(first, expected_first);

    // Next: introduce a brand-new path b.txt into baseline snapshots via a delete change.
    let del_b = HashMap::from([(
        b.clone(),
        FileChange::Delete {
            content: "z\n".to_string(),
        },
    )]);
    acc.on_patch_begin(&del_b);
    // Simulate apply: delete b.txt.
    let baseline_mode = file_mode_for_path(&b).unwrap_or(FileMode::Regular);
    fs::remove_file(&b).unwrap();

    let combined = acc.get_unified_diff().unwrap().unwrap();
    let combined = normalize_diff_for_test(&combined, dir.path());
    let expected = {
        let left_oid_a = git_blob_sha1_hex("foo\n");
        let right_oid_a = git_blob_sha1_hex("foo\nbar\n");
        let left_oid_b = git_blob_sha1_hex("z\n");
        format!(
            r#"diff --git a/<TMP>/a.txt b/<TMP>/a.txt
index {left_oid_a}..{right_oid_a}
--- a/<TMP>/a.txt
+++ b/<TMP>/a.txt
@@ -1 +1,2 @@
 foo
+bar
diff --git a/<TMP>/b.txt b/<TMP>/b.txt
deleted file mode {baseline_mode}
index {left_oid_b}..{ZERO_OID}
--- a/<TMP>/b.txt
+++ {DEV_NULL}
@@ -1 +0,0 @@
-z
"#,
        )
    };
    assert_eq!(combined, expected);
}

#[test]
fn binary_files_differ_update() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("bin.dat");

    // Initial non-UTF8 bytes
    let left_bytes: Vec<u8> = vec![0xff, 0xfe, 0xfd, 0x00];
    // Updated non-UTF8 bytes
    let right_bytes: Vec<u8> = vec![0x01, 0x02, 0x03, 0x00];

    fs::write(&file, &left_bytes).unwrap();

    let mut acc = TurnDiffTracker::new();
    let update_changes = HashMap::from([(
        file.clone(),
        FileChange::Update {
            unified_diff: "".to_owned(),
            move_path: None,
        },
    )]);
    acc.on_patch_begin(&update_changes);

    // Apply update on disk
    fs::write(&file, &right_bytes).unwrap();

    let diff = acc.get_unified_diff().unwrap().unwrap();
    let diff = normalize_diff_for_test(&diff, dir.path());
    let expected = {
        let left_oid = format!("{:x}", git_blob_sha1_hex_bytes(&left_bytes));
        let right_oid = format!("{:x}", git_blob_sha1_hex_bytes(&right_bytes));
        format!(
            r#"diff --git a/<TMP>/bin.dat b/<TMP>/bin.dat
index {left_oid}..{right_oid}
--- a/<TMP>/bin.dat
+++ b/<TMP>/bin.dat
Binary files differ
"#
        )
    };
    assert_eq!(diff, expected);
}

#[test]
fn filenames_with_spaces_add_and_update() {
    let mut acc = TurnDiffTracker::new();

    let dir = tempdir().unwrap();
    let file = dir.path().join("name with spaces.txt");

    // First patch: add file (baseline should be /dev/null).
    let add_changes = HashMap::from([(
        file.clone(),
        FileChange::Add {
            content: "foo\n".to_string(),
        },
    )]);
    acc.on_patch_begin(&add_changes);

    // Simulate apply: create the file on disk.
    fs::write(&file, "foo\n").unwrap();
    let first = acc.get_unified_diff().unwrap().unwrap();
    let first = normalize_diff_for_test(&first, dir.path());
    let expected_first = {
        let mode = file_mode_for_path(&file).unwrap_or(FileMode::Regular);
        let right_oid = git_blob_sha1_hex("foo\n");
        format!(
            r#"diff --git a/<TMP>/name with spaces.txt b/<TMP>/name with spaces.txt
new file mode {mode}
index {ZERO_OID}..{right_oid}
--- {DEV_NULL}
+++ b/<TMP>/name with spaces.txt
@@ -0,0 +1 @@
+foo
"#,
        )
    };
    assert_eq!(first, expected_first);

    // Second patch: update the file on disk.
    let update_changes = HashMap::from([(
        file.clone(),
        FileChange::Update {
            unified_diff: "".to_owned(),
            move_path: None,
        },
    )]);
    acc.on_patch_begin(&update_changes);

    // Simulate apply: append a new line with a space.
    fs::write(&file, "foo\nbar baz\n").unwrap();
    let combined = acc.get_unified_diff().unwrap().unwrap();
    let combined = normalize_diff_for_test(&combined, dir.path());
    let expected_combined = {
        let mode = file_mode_for_path(&file).unwrap_or(FileMode::Regular);
        let right_oid = git_blob_sha1_hex("foo\nbar baz\n");
        format!(
            r#"diff --git a/<TMP>/name with spaces.txt b/<TMP>/name with spaces.txt
new file mode {mode}
index {ZERO_OID}..{right_oid}
--- {DEV_NULL}
+++ b/<TMP>/name with spaces.txt
@@ -0,0 +1,2 @@
+foo
+bar baz
"#,
        )
    };
    assert_eq!(combined, expected_combined);
}
