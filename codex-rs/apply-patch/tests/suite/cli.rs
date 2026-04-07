use assert_cmd::Command;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn apply_patch_command() -> anyhow::Result<Command> {
    Ok(Command::new(codex_utils_cargo_bin::cargo_bin(
        "apply_patch",
    )?))
}

fn resolved_temp_path(tmp: &tempfile::TempDir, path: &str) -> anyhow::Result<PathBuf> {
    Ok(tmp.path().canonicalize()?.join(path))
}

#[test]
fn test_apply_patch_cli_add_and_update() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let file = "cli_test.txt";
    let absolute_path = tmp.path().join(file);
    let expected_path = resolved_temp_path(&tmp, file)?;

    // 1) Add a file
    let add_patch = format!(
        r#"*** Begin Patch
*** Add File: {file}
+hello
*** End Patch"#
    );
    apply_patch_command()?
        .arg(add_patch)
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(format!(
            "Success. Updated the following files:\nA {}\n",
            expected_path.display()
        ));
    assert_eq!(fs::read_to_string(&absolute_path)?, "hello\n");

    // 2) Update the file
    let update_patch = format!(
        r#"*** Begin Patch
*** Update File: {file}
@@
-hello
+world
*** End Patch"#
    );
    apply_patch_command()?
        .arg(update_patch)
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(format!(
            "Success. Updated the following files:\nM {}\n",
            expected_path.display()
        ));
    assert_eq!(fs::read_to_string(&absolute_path)?, "world\n");

    Ok(())
}

#[test]
fn test_apply_patch_cli_stdin_add_and_update() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let file = "cli_test_stdin.txt";
    let absolute_path = tmp.path().join(file);
    let expected_path = resolved_temp_path(&tmp, file)?;

    // 1) Add a file via stdin
    let add_patch = format!(
        r#"*** Begin Patch
*** Add File: {file}
+hello
*** End Patch"#
    );
    apply_patch_command()?
        .current_dir(tmp.path())
        .write_stdin(add_patch)
        .assert()
        .success()
        .stdout(format!(
            "Success. Updated the following files:\nA {}\n",
            expected_path.display()
        ));
    assert_eq!(fs::read_to_string(&absolute_path)?, "hello\n");

    // 2) Update the file via stdin
    let update_patch = format!(
        r#"*** Begin Patch
*** Update File: {file}
@@
-hello
+world
*** End Patch"#
    );
    apply_patch_command()?
        .current_dir(tmp.path())
        .write_stdin(update_patch)
        .assert()
        .success()
        .stdout(format!(
            "Success. Updated the following files:\nM {}\n",
            expected_path.display()
        ));
    assert_eq!(fs::read_to_string(&absolute_path)?, "world\n");

    Ok(())
}
