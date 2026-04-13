use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

pub(super) fn clone_git_source(
    url: &str,
    ref_name: Option<&str>,
    sparse_paths: &[String],
    destination: &Path,
) -> Result<()> {
    let destination = destination.to_string_lossy().to_string();
    if sparse_paths.is_empty() {
        run_git(&["clone", url, destination.as_str()], /*cwd*/ None)?;
        if let Some(ref_name) = ref_name {
            run_git(&["checkout", ref_name], Some(Path::new(&destination)))?;
        }
        return Ok(());
    }

    run_git(
        &[
            "clone",
            "--filter=blob:none",
            "--no-checkout",
            url,
            destination.as_str(),
        ],
        /*cwd*/ None,
    )?;
    let mut sparse_args = vec!["sparse-checkout", "set"];
    sparse_args.extend(sparse_paths.iter().map(String::as_str));
    let destination = Path::new(&destination);
    run_git(&sparse_args, Some(destination))?;
    run_git(&["checkout", ref_name.unwrap_or("HEAD")], Some(destination))?;
    Ok(())
}

fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<()> {
    let mut command = Command::new("git");
    command.args(args);
    command.env("GIT_TERMINAL_PROMPT", "0");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }

    let output = command
        .output()
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    bail!(
        "git {} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        output.status,
        stdout.trim(),
        stderr.trim()
    );
}

pub(super) fn replace_marketplace_root(staged_root: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    if destination.exists() {
        bail!(
            "marketplace destination already exists: {}",
            destination.display()
        );
    }

    fs::rename(staged_root, destination).map_err(Into::into)
}

pub(super) fn remove_marketplace_root(root: &Path) -> Result<bool> {
    if !root.exists() {
        return Ok(false);
    }

    fs::remove_dir_all(root)?;
    Ok(true)
}

pub(super) fn marketplace_staging_root(install_root: &Path) -> PathBuf {
    install_root.join(".staging")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn replace_marketplace_root_rejects_existing_destination() {
        let temp_dir = TempDir::new().unwrap();
        let staged_root = temp_dir.path().join("staged");
        let destination = temp_dir.path().join("destination");
        fs::create_dir_all(&staged_root).unwrap();
        fs::write(staged_root.join("marker.txt"), "staged").unwrap();
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("marker.txt"), "installed").unwrap();

        let err = replace_marketplace_root(&staged_root, &destination).unwrap_err();

        assert!(
            err.to_string()
                .contains("marketplace destination already exists"),
            "unexpected error: {err}"
        );
        assert_eq!(
            fs::read_to_string(staged_root.join("marker.txt")).unwrap(),
            "staged"
        );
        assert_eq!(
            fs::read_to_string(destination.join("marker.txt")).unwrap(),
            "installed"
        );
    }

    #[test]
    fn remove_marketplace_root_returns_false_when_missing() {
        let temp_dir = TempDir::new().unwrap();

        let removed = remove_marketplace_root(&temp_dir.path().join("missing")).unwrap();

        assert!(!removed);
    }
}
