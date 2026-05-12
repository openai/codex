use std::path::Path;
use std::process::Command;
use std::process::Stdio;

use anyhow::Context;
use anyhow::Result;

pub fn stdout(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = output(cwd, args)?;
    Ok(String::from_utf8(output)?.trim_end().to_string())
}

pub fn bytes(cwd: &Path, args: &[&str]) -> Result<Vec<u8>> {
    output(cwd, args)
}

pub fn status(cwd: &Path, args: &[&str]) -> Result<()> {
    output(cwd, args).map(|_| ())
}

pub fn status_with_stdin(cwd: &Path, args: &[&str], stdin: &[u8]) -> Result<()> {
    let mut child = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn git {}", args.join(" ")))?;
    use std::io::Write as _;
    child
        .stdin
        .as_mut()
        .context("git stdin unavailable")?
        .write_all(stdin)?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        anyhow::bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

pub fn output(cwd: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git").args(args).current_dir(cwd).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}
