use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::thread;
use std::time::Duration;
use std::time::Instant;

const OPENAI_PLUGINS_REPO_URL: &str = "https://github.com/openai/plugins.git";
const CURATED_PLUGINS_RELATIVE_DIR: &str = ".tmp/plugins";
const CURATED_PLUGINS_SHA_FILE: &str = ".tmp/plugins.sha";
const CURATED_PLUGINS_GIT_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) fn curated_plugins_repo_path(codex_home: &Path) -> PathBuf {
    codex_home.join(CURATED_PLUGINS_RELATIVE_DIR)
}

pub(crate) fn sync_openai_plugins_repo(codex_home: &Path) -> Result<(), String> {
    let repo_path = curated_plugins_repo_path(codex_home);
    let sha_path = codex_home.join(CURATED_PLUGINS_SHA_FILE);
    let remote_sha = git_ls_remote_head_sha()?;
    let local_sha = read_local_sha(&repo_path, &sha_path);

    if local_sha.as_deref() == Some(remote_sha.as_str()) && repo_path.join(".git").is_dir() {
        return Ok(());
    }

    let Some(parent) = repo_path.parent() else {
        return Err(format!(
            "failed to determine curated plugins parent directory for {}",
            repo_path.display()
        ));
    };
    fs::create_dir_all(parent).map_err(|err| {
        format!(
            "failed to create curated plugins parent directory {}: {err}",
            parent.display()
        )
    })?;

    let clone_dir = tempfile::Builder::new()
        .prefix("plugins-clone-")
        .tempdir_in(parent)
        .map_err(|err| {
            format!(
                "failed to create temporary curated plugins directory in {}: {err}",
                parent.display()
            )
        })?;
    let cloned_repo_path = clone_dir.path().join("repo");
    let clone_output = run_git_command_with_timeout(
        Command::new("git")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .arg("clone")
            .arg("--depth")
            .arg("1")
            .arg(OPENAI_PLUGINS_REPO_URL)
            .arg(&cloned_repo_path),
        "git clone curated plugins repo",
        CURATED_PLUGINS_GIT_TIMEOUT,
    )?;
    ensure_git_success(&clone_output, "git clone curated plugins repo")?;

    let cloned_sha = git_head_sha(&cloned_repo_path)?;
    if cloned_sha != remote_sha {
        return Err(format!(
            "curated plugins clone HEAD mismatch: expected {remote_sha}, got {cloned_sha}"
        ));
    }

    if repo_path.exists() {
        let backup_dir = tempfile::Builder::new()
            .prefix("plugins-backup-")
            .tempdir_in(parent)
            .map_err(|err| {
                format!(
                    "failed to create curated plugins backup directory in {}: {err}",
                    parent.display()
                )
            })?;
        let backup_repo_path = backup_dir.path().join("repo");

        fs::rename(&repo_path, &backup_repo_path).map_err(|err| {
            format!(
                "failed to move previous curated plugins repo out of the way at {}: {err}",
                repo_path.display()
            )
        })?;

        if let Err(err) = fs::rename(&cloned_repo_path, &repo_path) {
            let rollback_result = fs::rename(&backup_repo_path, &repo_path);
            return match rollback_result {
                Ok(()) => Err(format!(
                    "failed to activate new curated plugins repo at {}: {err}",
                    repo_path.display()
                )),
                Err(rollback_err) => {
                    let backup_path = backup_dir.keep().join("repo");
                    Err(format!(
                        "failed to activate new curated plugins repo at {}: {err}; failed to restore previous repo (left at {}): {rollback_err}",
                        repo_path.display(),
                        backup_path.display()
                    ))
                }
            };
        }
    } else {
        fs::rename(&cloned_repo_path, &repo_path).map_err(|err| {
            format!(
                "failed to activate curated plugins repo at {}: {err}",
                repo_path.display()
            )
        })?;
    }

    if let Some(parent) = sha_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create curated plugins sha directory {}: {err}",
                parent.display()
            )
        })?;
    }
    fs::write(&sha_path, format!("{cloned_sha}\n")).map_err(|err| {
        format!(
            "failed to write curated plugins sha file {}: {err}",
            sha_path.display()
        )
    })?;

    Ok(())
}

fn read_local_sha(repo_path: &Path, sha_path: &Path) -> Option<String> {
    if repo_path.join(".git").is_dir()
        && let Ok(sha) = git_head_sha(repo_path)
    {
        return Some(sha);
    }

    fs::read_to_string(sha_path)
        .ok()
        .map(|sha| sha.trim().to_string())
        .filter(|sha| !sha.is_empty())
}

fn git_ls_remote_head_sha() -> Result<String, String> {
    let output = run_git_command_with_timeout(
        Command::new("git")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .arg("ls-remote")
            .arg(OPENAI_PLUGINS_REPO_URL)
            .arg("HEAD"),
        "git ls-remote curated plugins repo",
        CURATED_PLUGINS_GIT_TIMEOUT,
    )?;
    ensure_git_success(&output, "git ls-remote curated plugins repo")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(first_line) = stdout.lines().next() else {
        return Err("git ls-remote returned empty output for curated plugins repo".to_string());
    };
    let Some((sha, _)) = first_line.split_once('\t') else {
        return Err(format!(
            "unexpected git ls-remote output for curated plugins repo: {first_line}"
        ));
    };
    if sha.is_empty() {
        return Err("git ls-remote returned empty sha for curated plugins repo".to_string());
    }
    Ok(sha.to_string())
}

fn git_head_sha(repo_path: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .arg("-C")
        .arg(repo_path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map_err(|err| {
            format!(
                "failed to run git rev-parse HEAD in {}: {err}",
                repo_path.display()
            )
        })?;
    ensure_git_success(&output, "git rev-parse HEAD")?;

    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sha.is_empty() {
        return Err(format!(
            "git rev-parse HEAD returned empty output in {}",
            repo_path.display()
        ));
    }
    Ok(sha)
}

fn run_git_command_with_timeout(
    command: &mut Command,
    context: &str,
    timeout: Duration,
) -> Result<Output, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to run {context}: {err}"))?;

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|err| format!("failed to wait for {context}: {err}"));
            }
            Ok(None) => {}
            Err(err) => return Err(format!("failed to poll {context}: {err}")),
        }

        if start.elapsed() >= timeout {
            match child.try_wait() {
                Ok(Some(_)) => {
                    return child
                        .wait_with_output()
                        .map_err(|err| format!("failed to wait for {context}: {err}"));
                }
                Ok(None) => {}
                Err(err) => return Err(format!("failed to poll {context}: {err}")),
            }

            let _ = child.kill();
            let output = child
                .wait_with_output()
                .map_err(|err| format!("failed to wait for {context} after timeout: {err}"))?;
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return if stderr.is_empty() {
                Err(format!("{context} timed out after {}s", timeout.as_secs()))
            } else {
                Err(format!(
                    "{context} timed out after {}s: {stderr}",
                    timeout.as_secs()
                ))
            };
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn ensure_git_success(output: &Output, context: &str) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        Err(format!("{context} failed with status {}", output.status))
    } else {
        Err(format!(
            "{context} failed with status {}: {stderr}",
            output.status
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    fn curated_plugins_repo_path_uses_codex_home_tmp_dir() {
        let tmp = tempdir().expect("tempdir");
        assert_eq!(
            curated_plugins_repo_path(tmp.path()),
            tmp.path().join(".tmp/plugins")
        );
    }

    #[test]
    fn read_local_sha_prefers_repo_head_when_available() {
        let tmp = tempdir().expect("tempdir");
        let repo_path = tmp.path().join("repo");
        let sha_path = tmp.path().join("plugins.sha");

        fs::create_dir_all(&repo_path).expect("create repo dir");
        fs::write(&sha_path, "abc123\n").expect("write sha");
        let init_output = Command::new("git")
            .arg("init")
            .arg(&repo_path)
            .output()
            .expect("git init should run");
        ensure_git_success(&init_output, "git init").expect("git init should succeed");
        let config_name_output = Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .arg("config")
            .arg("user.name")
            .arg("Codex")
            .output()
            .expect("git config user.name should run");
        ensure_git_success(&config_name_output, "git config user.name")
            .expect("git config user.name should succeed");
        let config_email_output = Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .arg("config")
            .arg("user.email")
            .arg("codex@example.com")
            .output()
            .expect("git config user.email should run");
        ensure_git_success(&config_email_output, "git config user.email")
            .expect("git config user.email should succeed");
        fs::write(repo_path.join("README.md"), "demo\n").expect("write file");
        let add_output = Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .arg("add")
            .arg(".")
            .output()
            .expect("git add should run");
        ensure_git_success(&add_output, "git add").expect("git add should succeed");
        let commit_output = Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .arg("commit")
            .arg("-m")
            .arg("init")
            .output()
            .expect("git commit should run");
        ensure_git_success(&commit_output, "git commit").expect("git commit should succeed");

        let sha = read_local_sha(&repo_path, &sha_path);
        assert_eq!(sha, Some(git_head_sha(&repo_path).expect("repo head sha")));
    }

    #[test]
    fn read_local_sha_falls_back_to_sha_file() {
        let tmp = tempdir().expect("tempdir");
        let repo_path = tmp.path().join("repo");
        let sha_path = tmp.path().join("plugins.sha");
        fs::write(&sha_path, "abc123\n").expect("write sha");

        let sha = read_local_sha(&repo_path, &sha_path);
        assert_eq!(sha.as_deref(), Some("abc123"));
    }
}
