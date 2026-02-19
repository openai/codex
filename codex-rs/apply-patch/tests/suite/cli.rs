use assert_cmd::Command;
use codex_apply_patch::PRESERVE_CRLF_FLAG;
use std::fs;
use tempfile::tempdir;

fn apply_patch_command() -> anyhow::Result<Command> {
    Ok(Command::new(codex_utils_cargo_bin::cargo_bin(
        "apply_patch",
    )?))
}

#[test]
fn test_apply_patch_cli_add_and_update() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let file = "cli_test.txt";
    let absolute_path = tmp.path().join(file);

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
        .stdout(format!("Success. Updated the following files:\nA {file}\n"));
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
        .stdout(format!("Success. Updated the following files:\nM {file}\n"));
    assert_eq!(fs::read_to_string(&absolute_path)?, "world\n");

    Ok(())
}

#[test]
fn test_apply_patch_cli_stdin_add_and_update() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let file = "cli_test_stdin.txt";
    let absolute_path = tmp.path().join(file);

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
        .stdout(format!("Success. Updated the following files:\nA {file}\n"));
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
        .stdout(format!("Success. Updated the following files:\nM {file}\n"));
    assert_eq!(fs::read_to_string(&absolute_path)?, "world\n");

    Ok(())
}

#[test]
fn test_apply_patch_cli_normalizes_crlf_without_flag() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let file = "crlf_default.txt";
    let absolute_path = tmp.path().join(file);
    fs::write(&absolute_path, b"one\r\ntwo\r\n")?;

    let patch = format!(
        "*** Begin Patch\r\n*** Update File: {file}\r\n@@\r\n-one\r\n+uno\r\n two\r\n*** End Patch\r\n"
    );

    apply_patch_command()?
        .arg(patch)
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(format!("Success. Updated the following files:\nM {file}\n"));

    assert_eq!(fs::read(absolute_path)?, b"uno\ntwo\n");
    Ok(())
}

#[test]
fn test_apply_patch_cli_preserves_crlf_with_flag() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let file = "crlf_flag.txt";
    let absolute_path = tmp.path().join(file);
    fs::write(&absolute_path, b"one\r\ntwo\r\n")?;

    let patch = format!(
        "*** Begin Patch\r\n*** Update File: {file}\r\n@@\r\n-one\r\n+uno\r\n two\r\n*** End Patch\r\n"
    );

    apply_patch_command()?
        .arg(PRESERVE_CRLF_FLAG)
        .arg(patch)
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(format!("Success. Updated the following files:\nM {file}\n"));

    assert_eq!(fs::read(absolute_path)?, b"uno\r\ntwo\r\n");
    Ok(())
}
