//! Linux executable lookup must match the compatibility launcher's PATH semantics.

use std::os::unix::fs::PermissionsExt;

use pretty_assertions::assert_eq;

use crate::Command;
use crate::ProcessMode;

#[tokio::test]
async fn path_search_stops_at_invalid_candidates() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    std::fs::create_dir(root.path().join("bin"))?;
    std::os::unix::fs::symlink("loop", root.path().join("loop"))?;
    let script = root.path().join("bin/server");
    std::fs::write(&script, "#!/bin/sh\nprintf later\n")?;
    std::fs::set_permissions(script, std::fs::Permissions::from_mode(/*mode*/ 0o755))?;
    for candidate in ["loop".to_owned(), "x".repeat(256)] {
        let mut command = Command::new("server");
        command
            .current_dir(root.path())
            .env("PATH", format!("{candidate}:bin"));
        let expected = command
            .inner
            .spawn()
            .expect_err("invalid first candidate must stop lookup");
        command.process_mode(ProcessMode::NewSession);
        let actual = command.spawn().err().expect("native lookup must stop too");
        assert_eq!(actual.raw_os_error(), expected.raw_os_error());
    }
    Ok(())
}

#[tokio::test]
async fn empty_path_entries_preserve_script_argv0() -> anyhow::Result<()> {
    // A sibling test can fork while the script is open for writing, retaining
    // the writable descriptor and making exec fail with ETXTBSY.
    if std::env::var_os("CODEX_TEST_PATH_ARGV0").is_none() {
        let output = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "child_command::linux_tests::empty_path_entries_preserve_script_argv0",
                "--nocapture",
            ])
            .env("CODEX_TEST_PATH_ARGV0", "1")
            .output()?;
        assert!(output.status.success(), "{output:?}");
        return Ok(());
    }
    let root = tempfile::tempdir()?;
    let script = root.path().join("server");
    std::fs::write(&script, "#!/bin/sh\nprintf '%s' \"$0\"\n")?;
    std::fs::set_permissions(script, std::fs::Permissions::from_mode(/*mode*/ 0o755))?;
    for path in ["", ":missing", "missing:", ".", "./"] {
        let mut command = Command::new("server");
        command.current_dir(root.path()).env("PATH", path);
        let expected = command.inner.output().await?;
        command.process_mode(ProcessMode::NewSession);
        let actual = command.spawn()?.wait_with_output().await?;
        assert_eq!(actual, expected);
    }
    Ok(())
}
