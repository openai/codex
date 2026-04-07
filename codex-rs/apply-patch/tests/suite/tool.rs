use assert_cmd::Command;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use tempfile::tempdir;

fn run_apply_patch_in_dir(dir: &Path, patch: &str) -> anyhow::Result<assert_cmd::assert::Assert> {
    let mut cmd = Command::new(codex_utils_cargo_bin::cargo_bin("apply_patch")?);
    cmd.current_dir(dir);
    Ok(cmd.arg(patch).assert())
}

fn apply_patch_command(dir: &Path) -> anyhow::Result<Command> {
    let mut cmd = Command::new(codex_utils_cargo_bin::cargo_bin("apply_patch")?);
    cmd.current_dir(dir);
    Ok(cmd)
}

fn resolved_under(root: &Path, path: &str) -> anyhow::Result<PathBuf> {
    Ok(root.canonicalize()?.join(path))
}

#[test]
fn test_apply_patch_cli_applies_multiple_operations() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let add_path = tmp.path().join("nested/new.txt");
    let modify_path = tmp.path().join("modify.txt");
    let delete_path = tmp.path().join("delete.txt");
    let expected_add_path = resolved_under(tmp.path(), "nested/new.txt")?;
    let expected_modify_path = resolved_under(tmp.path(), "modify.txt")?;
    let expected_delete_path = resolved_under(tmp.path(), "delete.txt")?;

    fs::write(&modify_path, "line1\nline2\n")?;
    fs::write(&delete_path, "obsolete\n")?;

    let patch = "*** Begin Patch\n*** Add File: nested/new.txt\n+created\n*** Delete File: delete.txt\n*** Update File: modify.txt\n@@\n-line2\n+changed\n*** End Patch";

    run_apply_patch_in_dir(tmp.path(), patch)?
        .success()
        .stdout(format!(
            "Success. Updated the following files:\nA {}\nM {}\nD {}\n",
            expected_add_path.display(),
            expected_modify_path.display(),
            expected_delete_path.display()
        ));

    assert_eq!(fs::read_to_string(add_path)?, "created\n");
    assert_eq!(fs::read_to_string(&modify_path)?, "line1\nchanged\n");
    assert!(!delete_path.exists());

    Ok(())
}

#[test]
fn test_apply_patch_cli_applies_multiple_chunks() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let target_path = tmp.path().join("multi.txt");
    let expected_target_path = resolved_under(tmp.path(), "multi.txt")?;
    fs::write(&target_path, "line1\nline2\nline3\nline4\n")?;

    let patch = "*** Begin Patch\n*** Update File: multi.txt\n@@\n-line2\n+changed2\n@@\n-line4\n+changed4\n*** End Patch";

    run_apply_patch_in_dir(tmp.path(), patch)?
        .success()
        .stdout(format!(
            "Success. Updated the following files:\nM {}\n",
            expected_target_path.display()
        ));

    assert_eq!(
        fs::read_to_string(&target_path)?,
        "line1\nchanged2\nline3\nchanged4\n"
    );

    Ok(())
}

#[test]
fn test_apply_patch_cli_moves_file_to_new_directory() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let original_path = tmp.path().join("old/name.txt");
    let new_path = tmp.path().join("renamed/dir/name.txt");
    let expected_new_path = resolved_under(tmp.path(), "renamed/dir/name.txt")?;
    fs::create_dir_all(original_path.parent().expect("parent should exist"))?;
    fs::write(&original_path, "old content\n")?;

    let patch = "*** Begin Patch\n*** Update File: old/name.txt\n*** Move to: renamed/dir/name.txt\n@@\n-old content\n+new content\n*** End Patch";

    run_apply_patch_in_dir(tmp.path(), patch)?
        .success()
        .stdout(format!(
            "Success. Updated the following files:\nM {}\n",
            expected_new_path.display()
        ));

    assert!(!original_path.exists());
    assert_eq!(fs::read_to_string(&new_path)?, "new content\n");

    Ok(())
}

#[test]
fn test_apply_patch_cli_rejects_empty_patch() -> anyhow::Result<()> {
    let tmp = tempdir()?;

    apply_patch_command(tmp.path())?
        .arg("*** Begin Patch\n*** End Patch")
        .assert()
        .failure()
        .stderr("No files were modified.\n");

    Ok(())
}

#[test]
fn test_apply_patch_cli_reports_missing_context() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let target_path = tmp.path().join("modify.txt");
    let expected_target_path = resolved_under(tmp.path(), "modify.txt")?;
    fs::write(&target_path, "line1\nline2\n")?;

    apply_patch_command(tmp.path())?
        .arg("*** Begin Patch\n*** Update File: modify.txt\n@@\n-missing\n+changed\n*** End Patch")
        .assert()
        .failure()
        .stderr(format!(
            "Failed to find expected lines in {}:\nmissing\n",
            expected_target_path.display()
        ));
    assert_eq!(fs::read_to_string(&target_path)?, "line1\nline2\n");

    Ok(())
}

#[test]
fn test_apply_patch_cli_rejects_missing_file_delete() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let missing_path = resolved_under(tmp.path(), "missing.txt")?;

    apply_patch_command(tmp.path())?
        .arg("*** Begin Patch\n*** Delete File: missing.txt\n*** End Patch")
        .assert()
        .failure()
        .stderr(format!(
            "Failed to delete file {}\n",
            missing_path.display()
        ));

    Ok(())
}

#[test]
fn test_apply_patch_cli_rejects_empty_update_hunk() -> anyhow::Result<()> {
    let tmp = tempdir()?;

    apply_patch_command(tmp.path())?
        .arg("*** Begin Patch\n*** Update File: foo.txt\n*** End Patch")
        .assert()
        .failure()
        .stderr("Invalid patch hunk on line 2: Update file hunk for path 'foo.txt' is empty\n");

    Ok(())
}

#[test]
fn test_apply_patch_cli_requires_existing_file_for_update() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let missing_path = resolved_under(tmp.path(), "missing.txt")?;

    apply_patch_command(tmp.path())?
        .arg("*** Begin Patch\n*** Update File: missing.txt\n@@\n-old\n+new\n*** End Patch")
        .assert()
        .failure()
        .stderr(format!(
            "Failed to read file to update {}: No such file or directory (os error 2)\n",
            missing_path.display()
        ));

    Ok(())
}

#[test]
fn test_apply_patch_cli_move_overwrites_existing_destination() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let original_path = tmp.path().join("old/name.txt");
    let destination = tmp.path().join("renamed/dir/name.txt");
    let expected_destination = resolved_under(tmp.path(), "renamed/dir/name.txt")?;
    fs::create_dir_all(original_path.parent().expect("parent should exist"))?;
    fs::create_dir_all(destination.parent().expect("parent should exist"))?;
    fs::write(&original_path, "from\n")?;
    fs::write(&destination, "existing\n")?;

    run_apply_patch_in_dir(
        tmp.path(),
        "*** Begin Patch\n*** Update File: old/name.txt\n*** Move to: renamed/dir/name.txt\n@@\n-from\n+new\n*** End Patch",
    )?
    .success()
    .stdout(format!(
        "Success. Updated the following files:\nM {}\n",
        expected_destination.display()
    ));

    assert!(!original_path.exists());
    assert_eq!(fs::read_to_string(&destination)?, "new\n");

    Ok(())
}

#[test]
fn test_apply_patch_cli_add_overwrites_existing_file() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let path = tmp.path().join("duplicate.txt");
    let expected_path = resolved_under(tmp.path(), "duplicate.txt")?;
    fs::write(&path, "old content\n")?;

    run_apply_patch_in_dir(
        tmp.path(),
        "*** Begin Patch\n*** Add File: duplicate.txt\n+new content\n*** End Patch",
    )?
    .success()
    .stdout(format!(
        "Success. Updated the following files:\nA {}\n",
        expected_path.display()
    ));

    assert_eq!(fs::read_to_string(&path)?, "new content\n");

    Ok(())
}

#[test]
fn test_apply_patch_cli_delete_directory_fails() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let dir = tmp.path().join("dir");
    let expected_dir = resolved_under(tmp.path(), "dir")?;
    fs::create_dir(&dir)?;

    apply_patch_command(tmp.path())?
        .arg("*** Begin Patch\n*** Delete File: dir\n*** End Patch")
        .assert()
        .failure()
        .stderr(format!(
            "Failed to delete file {}\n",
            expected_dir.display()
        ));

    Ok(())
}

#[test]
fn test_apply_patch_cli_rejects_invalid_hunk_header() -> anyhow::Result<()> {
    let tmp = tempdir()?;

    apply_patch_command(tmp.path())?
        .arg("*** Begin Patch\n*** Frobnicate File: foo\n*** End Patch")
        .assert()
        .failure()
        .stderr("Invalid patch hunk on line 2: '*** Frobnicate File: foo' is not a valid hunk header. Valid hunk headers: '*** Add File: {path}', '*** Delete File: {path}', '*** Update File: {path}'\n");

    Ok(())
}

#[test]
fn test_apply_patch_cli_updates_file_appends_trailing_newline() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let target_path = tmp.path().join("no_newline.txt");
    let expected_target_path = resolved_under(tmp.path(), "no_newline.txt")?;
    fs::write(&target_path, "no newline at end")?;

    run_apply_patch_in_dir(
        tmp.path(),
        "*** Begin Patch\n*** Update File: no_newline.txt\n@@\n-no newline at end\n+first line\n+second line\n*** End Patch",
    )?
    .success()
    .stdout(format!(
        "Success. Updated the following files:\nM {}\n",
        expected_target_path.display()
    ));

    let contents = fs::read_to_string(&target_path)?;
    assert!(contents.ends_with('\n'));
    assert_eq!(contents, "first line\nsecond line\n");

    Ok(())
}

#[test]
fn test_apply_patch_cli_failure_after_partial_success_leaves_changes() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let new_file = tmp.path().join("created.txt");
    let missing_file = resolved_under(tmp.path(), "missing.txt")?;

    apply_patch_command(tmp.path())?
        .arg("*** Begin Patch\n*** Add File: created.txt\n+hello\n*** Update File: missing.txt\n@@\n-old\n+new\n*** End Patch")
        .assert()
        .failure()
        .stdout("")
        .stderr(format!(
            "Failed to read file to update {}: No such file or directory (os error 2)\n",
            missing_file.display()
        ));

    assert_eq!(fs::read_to_string(&new_file)?, "hello\n");

    Ok(())
}
