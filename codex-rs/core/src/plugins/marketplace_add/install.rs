use super::MarketplaceAddError;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

pub(super) fn clone_git_source(
    url: &str,
    ref_name: Option<&str>,
    sparse_paths: &[String],
    destination: &Path,
) -> Result<(), MarketplaceAddError> {
    let destination_string = destination.to_string_lossy().to_string();
    if sparse_paths.is_empty() {
        run_git(&["clone", url, destination_string.as_str()], None)?;
        if let Some(ref_name) = ref_name {
            run_git(
                &["checkout", ref_name],
                Some(Path::new(&destination_string)),
            )?;
        }
        return Ok(());
    }

    run_git(
        &[
            "clone",
            "--filter=blob:none",
            "--no-checkout",
            url,
            destination_string.as_str(),
        ],
        None,
    )?;
    let mut sparse_args = vec!["sparse-checkout", "set"];
    sparse_args.extend(sparse_paths.iter().map(String::as_str));
    run_git(&sparse_args, Some(destination))?;
    run_git(&["checkout", ref_name.unwrap_or("HEAD")], Some(destination))?;
    Ok(())
}

pub(super) fn safe_marketplace_dir_name(
    marketplace_name: &str,
) -> Result<String, MarketplaceAddError> {
    let safe = marketplace_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let safe = safe.trim_matches('.').to_string();
    if safe.is_empty() || safe == ".." {
        return Err(MarketplaceAddError::InvalidRequest(format!(
            "marketplace name '{marketplace_name}' cannot be used as an install directory"
        )));
    }
    Ok(safe)
}

pub(super) fn ensure_marketplace_destination_is_inside_install_root(
    install_root: &Path,
    destination: &Path,
) -> Result<(), MarketplaceAddError> {
    let install_root = install_root.canonicalize().map_err(|err| {
        MarketplaceAddError::Internal(format!(
            "failed to resolve marketplace install root {}: {err}",
            install_root.display()
        ))
    })?;
    let destination_parent = destination
        .parent()
        .ok_or_else(|| {
            MarketplaceAddError::Internal("marketplace destination has no parent".to_string())
        })?
        .canonicalize()
        .map_err(|err| {
            MarketplaceAddError::Internal(format!(
                "failed to resolve marketplace destination parent {}: {err}",
                destination.display()
            ))
        })?;
    if !destination_parent.starts_with(&install_root) {
        return Err(MarketplaceAddError::InvalidRequest(format!(
            "marketplace destination {} is outside install root {}",
            destination.display(),
            install_root.display()
        )));
    }
    Ok(())
}

pub(super) fn replace_marketplace_root(
    staged_root: &Path,
    destination: &Path,
) -> std::io::Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(staged_root, destination)
}

pub(super) fn marketplace_staging_root(install_root: &Path) -> PathBuf {
    install_root.join(".staging")
}

fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<(), MarketplaceAddError> {
    let mut command = Command::new("git");
    command.args(args);
    command.env("GIT_TERMINAL_PROMPT", "0");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }

    let output = command.output().map_err(|err| {
        MarketplaceAddError::Internal(format!("failed to run git {}: {err}", args.join(" ")))
    })?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(MarketplaceAddError::Internal(format!(
        "git {} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        output.status,
        stdout.trim(),
        stderr.trim()
    )))
}
