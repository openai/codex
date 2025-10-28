use assert_cmd::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

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
    Command::cargo_bin("apply_patch")
        .expect("should find apply_patch binary")
        .arg(add_patch)
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(format!("Success. Updated the following files:\nA {file}\n"));
    {
        let s = fs::read_to_string(&absolute_path)?;
        assert_eq!(s.replace("\r\n", "\n"), "hello\n");
    }

    // 2) Update the file
    let update_patch = format!(
        r#"*** Begin Patch
*** Update File: {file}
@@
-hello
+world
*** End Patch"#
    );
    Command::cargo_bin("apply_patch")
        .expect("should find apply_patch binary")
        .arg(update_patch)
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(format!("Success. Updated the following files:\nM {file}\n"));
    {
        let s = fs::read_to_string(&absolute_path)?;
        assert_eq!(s.replace("\r\n", "\n"), "world\n");
    }

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
    let mut cmd =
        assert_cmd::Command::cargo_bin("apply_patch").expect("should find apply_patch binary");
    cmd.current_dir(tmp.path());
    cmd.write_stdin(add_patch)
        .assert()
        .success()
        .stdout(format!("Success. Updated the following files:\nA {file}\n"));
    {
        let s = fs::read_to_string(&absolute_path)?;
        assert_eq!(s.replace("\r\n", "\n"), "hello\n");
    }

    // 2) Update the file via stdin
    let update_patch = format!(
        r#"*** Begin Patch
*** Update File: {file}
@@
-hello
+world
*** End Patch"#
    );
    let mut cmd =
        assert_cmd::Command::cargo_bin("apply_patch").expect("should find apply_patch binary");
    cmd.current_dir(tmp.path());
    cmd.write_stdin(update_patch)
        .assert()
        .success()
        .stdout(format!("Success. Updated the following files:\nM {file}\n"));
    {
        let s = fs::read_to_string(&absolute_path)?;
        assert_eq!(s.replace("\r\n", "\n"), "world\n");
    }

    Ok(())
}

#[test]
fn test_detect_overrides_repo_policy() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    // Initialize a repo with CRLF policy via .gitattributes
    std::process::Command::new("git")
        .arg("init")
        .arg("-q")
        .current_dir(tmp.path())
        .status()?;
    std::fs::write(tmp.path().join(".gitattributes"), "*.txt text eol=crlf\n")?;

    let file = "detect_overrides.txt";
    let absolute_path = tmp.path().join(file);
    // LF patch content
    let add_patch = format!(
        r#"*** Begin Patch
*** Add File: {file}
+hello
*** End Patch"#
    );
    // CLI Detect should override repo policy and keep LF
    assert_cmd::Command::cargo_bin("apply_patch")?
        .current_dir(tmp.path())
        .arg("--assume-eol=detect")
        .arg(add_patch)
        .assert()
        .success();
    let s = std::fs::read_to_string(&absolute_path)?;
    assert_eq!(s.replace("\r\n", "\n"), "hello\n");
    Ok(())
}

#[test]
fn test_cli_overrides_env_assume_eol() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let file = "env_cli_precedence.txt";
    let absolute_path = tmp.path().join(file);

    // Env says CRLF, CLI says LF. CLI should win.
    let add_patch = format!(
        r#"*** Begin Patch
*** Add File: {file}
+hello
*** End Patch"#
    );
    Command::cargo_bin("apply_patch")
        .expect("should find apply_patch binary")
        .current_dir(tmp.path())
        .env("APPLY_PATCH_ASSUME_EOL", "crlf")
        .arg("--assume-eol=lf")
        .arg(add_patch)
        .assert()
        .success();
    let s = fs::read_to_string(&absolute_path)?;
    assert_eq!(s.replace("\r\n", "\n"), "hello\n");
    Ok(())
}
