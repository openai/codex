use super::snapshot_script;
use crate::shell_detect::ShellType;
use anyhow::Result;
#[cfg(target_os = "macos")]
use pretty_assertions::assert_eq;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn bash_snapshot_filters_invalid_exports() -> Result<()> {
    let output = Command::new("/bin/bash")
        .arg("-c")
        .arg(snapshot_script(ShellType::Bash).expect("bash supports snapshots"))
        .env("BASH_ENV", "/dev/null")
        .env("VALID_NAME", "ok")
        .env("PWD", "/tmp/stale")
        .env("NEXTEST_BIN_EXE_codex-write-config-schema", "/path/to/bin")
        .env("BAD-NAME", "broken")
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("VALID_NAME"));
    assert!(!stdout.contains("PWD=/tmp/stale"));
    assert!(!stdout.contains("NEXTEST_BIN_EXE_codex-write-config-schema"));
    assert!(!stdout.contains("BAD-NAME"));

    Ok(())
}

#[test]
fn bash_snapshot_preserves_multiline_exports() -> Result<()> {
    let multiline_cert = "-----BEGIN CERTIFICATE-----\nabc\n-----END CERTIFICATE-----";
    let output = Command::new("/bin/bash")
        .arg("-c")
        .arg(snapshot_script(ShellType::Bash).expect("bash supports snapshots"))
        .env("BASH_ENV", "/dev/null")
        .env("MULTILINE_CERT", multiline_cert)
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("MULTILINE_CERT=") || stdout.contains("MULTILINE_CERT"),
        "snapshot should include the multiline export name"
    );

    let dir = tempdir()?;
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(&snapshot_path, stdout.as_bytes())?;

    let validate = Command::new("/bin/bash")
        .arg("-c")
        .arg("set -e; . \"$1\"")
        .arg("bash")
        .arg(&snapshot_path)
        .env("BASH_ENV", "/dev/null")
        .output()?;

    assert!(
        validate.status.success(),
        "snapshot validation failed: {}",
        String::from_utf8_lossy(&validate.stderr)
    );

    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn zsh_snapshot_restores_tied_path() -> Result<()> {
    let dir = tempdir()?;
    let path_with_spaces = dir.path().join("path with spaces").join("bin");
    let plain_path = dir.path().join("plain-path").join("bin");
    let expected_path = format!(
        "{}:{}:/usr/bin:/bin",
        path_with_spaces.display(),
        plain_path.display()
    );
    let zshrc = format!(
        "export -UT PATH path=('{}' '{}' '{}' /usr/bin /bin)\n",
        path_with_spaces.display(),
        plain_path.display(),
        plain_path.display()
    );
    std::fs::write(dir.path().join(".zshrc"), zshrc)?;

    let snapshot = Command::new("/bin/zsh")
        .arg("-f")
        .arg("-c")
        .arg(snapshot_script(ShellType::Zsh).expect("zsh supports snapshots"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("ZDOTDIR", dir.path())
        .output()?;
    assert!(snapshot.status.success());

    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(&snapshot_path, &snapshot.stdout)?;

    let restored = Command::new("/bin/zsh")
        .arg("-f")
        .arg("-c")
        .arg("set -e; . \"$1\"; print -r -- \"$PATH\"")
        .arg("zsh")
        .arg(&snapshot_path)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .output()?;
    assert!(restored.status.success());
    assert_eq!(
        String::from_utf8(restored.stdout)?.trim_end(),
        expected_path
    );

    let snapshot = String::from_utf8(snapshot.stdout)?;
    assert!(
        snapshot
            .lines()
            .any(|line| line.starts_with("export -UT PATH path=")),
        "snapshot should capture the tied PATH export"
    );

    std::fs::write(dir.path().join(".zshrc"), "readonly PATH\n")?;
    let readonly_snapshot = Command::new("/bin/zsh")
        .arg("-f")
        .arg("-c")
        .arg(snapshot_script(ShellType::Zsh).expect("zsh supports snapshots"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("ZDOTDIR", dir.path())
        .output()?;
    assert!(readonly_snapshot.status.success());
    std::fs::write(&snapshot_path, &readonly_snapshot.stdout)?;

    let readonly_restored = Command::new("/bin/zsh")
        .arg("-f")
        .arg("-c")
        .arg("set -e; . \"$1\"; export PATH='/codex-path':\"$PATH\"; print -r -- \"$PATH\"")
        .arg("zsh")
        .arg(&snapshot_path)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .output()?;
    assert!(readonly_restored.status.success());
    assert_eq!(
        String::from_utf8(readonly_restored.stdout)?.trim_end(),
        "/codex-path:/usr/bin:/bin"
    );

    let readonly_snapshot = String::from_utf8(readonly_snapshot.stdout)?;
    assert!(
        !readonly_snapshot
            .lines()
            .any(|line| line.starts_with("export -rT PATH path=")),
        "snapshot should not capture the readonly tied PATH export"
    );

    Ok(())
}
