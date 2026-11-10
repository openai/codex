use super::MARKETPLACE_UPGRADE_GIT_TIMEOUT;
use crate::PluginGitMode;
use crate::marketplace_staging::MarketplaceStagingDir;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::time::Duration;

pub(super) fn git_remote_revision(
    codex_home: &Path,
    source: &str,
    ref_name: Option<&str>,
    timeout: Duration,
    mode: PluginGitMode,
) -> Result<String, String> {
    if let Some(ref_name) = ref_name
        && is_full_git_sha(ref_name)
    {
        return Ok(ref_name.to_string());
    }

    let ref_name = ref_name.unwrap_or("HEAD");
    let mut command = git_command(mode);
    let _trusted_repository = matches!(mode, PluginGitMode::Automatic)
        .then(|| crate::configure_trusted_git_repository(&mut command, codex_home))
        .transpose()?;
    let output = run_git_command_with_timeout(
        command.arg("ls-remote").arg(source).arg(ref_name),
        "git ls-remote marketplace source",
        timeout,
    )?;
    ensure_git_success(&output, "git ls-remote marketplace source")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(first_line) = stdout.lines().next() else {
        return Err("git ls-remote returned empty output for marketplace source".to_string());
    };
    let Some((revision, _)) = first_line.split_once('\t') else {
        return Err(format!(
            "unexpected git ls-remote output for marketplace source: {first_line}"
        ));
    };
    let revision = revision.trim();
    if revision.is_empty() {
        return Err("git ls-remote returned empty revision for marketplace source".to_string());
    }
    Ok(revision.to_string())
}

/// Git before 2.20 silently ignores the setting that limits which alternate refs it advertises.
fn git_supports_alternate_refs_command(mode: PluginGitMode) -> bool {
    let Ok(output) = run_git_command_with_timeout(
        git_command(mode).arg("--version"),
        "git --version",
        MARKETPLACE_UPGRADE_GIT_TIMEOUT,
    ) else {
        return false;
    };
    output.status.success()
        && git_release(&output.stdout).is_some_and(|version| version >= (2, 20, 0))
}

/// Extracts the release number from `git --version`, ignoring the Apple or Windows build suffix.
/// An unrecognized version (such as `2.20.0.rc1`) disables reference cloning.
fn git_release(stdout: &[u8]) -> Option<(u32, u32, u32)> {
    let version = std::str::from_utf8(stdout)
        .ok()?
        .strip_prefix("git version ")?
        .split_ascii_whitespace()
        .next()?;
    let (major, rest) = version.split_once('.')?;
    let (minor, patch) = rest.split_once('.')?;
    let patch = match patch.split_once(".windows.") {
        Some((patch, build)) => {
            build.parse::<u32>().ok()?;
            patch
        }
        None => patch,
    };
    Some((
        major.parse().ok()?,
        minor.parse().ok()?,
        patch.parse().ok()?,
    ))
}

pub(super) struct CloneReference<'a> {
    pub(super) git_dir: PathBuf,
    pub(super) installed_revision: &'a str,
}

/// Stages a clone for checkout. If the reference clone fails (for example, because another
/// upgrade replaced the installed object store), retries a plain clone in a fresh directory.
pub(super) fn stage_git_clone(
    codex_home: &Path,
    install_root: &Path,
    source: &str,
    sparse_paths: &[String],
    reference: Option<CloneReference<'_>>,
    mode: PluginGitMode,
) -> Result<MarketplaceStagingDir, String> {
    let run_git_clone = |reference| -> Result<_, String> {
        let staged_dir = MarketplaceStagingDir::new(install_root).map_err(|err| {
            format!(
                "failed to create temporary marketplace upgrade directory in {}: {err}",
                install_root.display()
            )
        })?;
        clone_git_source(
            codex_home,
            source,
            sparse_paths,
            staged_dir.path(),
            reference,
            mode,
        )?;
        Ok(staged_dir)
    };

    if let Some(reference) = reference
        && git_supports_alternate_refs_command(mode)
        && let Ok(staged_dir) = run_git_clone(Some(reference))
    {
        return Ok(staged_dir);
    }
    run_git_clone(/*reference*/ None)
}

/// Clones the marketplace source, optionally borrowing objects from the installed copy.
/// Git uses the recorded installation commit for alternate-ref negotiation and copies the
/// borrowed objects it needs before the upgrade can replace the installed directory.
fn clone_git_source(
    codex_home: &Path,
    source: &str,
    sparse_paths: &[String],
    destination: &Path,
    reference: Option<CloneReference<'_>>,
    mode: PluginGitMode,
) -> Result<(), String> {
    let timeout = MARKETPLACE_UPGRADE_GIT_TIMEOUT;
    let destination = git_path_arg(destination);
    let mut command = git_command(mode);
    let _trusted_repository = matches!(mode, PluginGitMode::Automatic)
        .then(|| crate::configure_trusted_git_repository(&mut command, codex_home))
        .transpose()?;
    if let Some(reference) = reference {
        // Git normally enumerates all refs in the alternate, including private local branches.
        // Seed negotiation from the installed commit. The shell builtin `:` consumes the
        // repository path that Git appends to the command.
        command
            .env("CODEX_MARKETPLACE_HAVE", reference.installed_revision)
            .args([
                "-c",
                r#"core.alternateRefsCommand=echo "$CODEX_MARKETPLACE_HAVE" && :"#,
                "clone",
                "--reference",
            ])
            .arg(git_path_arg(&reference.git_dir))
            .arg("--dissociate");
    } else {
        command.arg("clone");
    }
    if !sparse_paths.is_empty() {
        command.args(["--filter=blob:none", "--no-checkout"]);
    }
    let output = run_git_command_with_timeout(
        command.arg("--").arg(source).arg(&destination),
        "git clone marketplace source",
        timeout,
    )?;
    ensure_git_success(&output, "git clone marketplace source")
}

/// Checks out the requested ref and returns the revision that will be validated for activation.
pub(super) fn checkout_git_source(
    destination: &Path,
    ref_name: Option<&str>,
    sparse_paths: &[String],
    mode: PluginGitMode,
) -> Result<String, String> {
    let destination = git_path_arg(destination);
    let timeout = MARKETPLACE_UPGRADE_GIT_TIMEOUT;
    let checkout_ref = if sparse_paths.is_empty() {
        ref_name
    } else {
        let output = run_git_command_with_timeout(
            git_command(mode)
                .arg("-C")
                .arg(&destination)
                .args(["sparse-checkout", "set"])
                .args(sparse_paths),
            "git sparse-checkout marketplace source",
            timeout,
        )?;
        ensure_git_success(&output, "git sparse-checkout marketplace source")?;
        Some(ref_name.unwrap_or("HEAD"))
    };
    if let Some(checkout_ref) = checkout_ref {
        let output = run_git_command_with_timeout(
            git_command(mode)
                .arg("-C")
                .arg(&destination)
                .arg("checkout")
                .arg(checkout_ref),
            "git checkout marketplace ref",
            timeout,
        )?;
        ensure_git_success(&output, "git checkout marketplace ref")?;
    }
    git_worktree_revision(&destination, timeout, mode)
}

fn git_worktree_revision(
    destination: &Path,
    timeout: Duration,
    mode: PluginGitMode,
) -> Result<String, String> {
    let output = run_git_command_with_timeout(
        git_command(mode)
            .arg("-C")
            .arg(destination)
            .arg("rev-parse")
            .arg("HEAD"),
        "git rev-parse marketplace revision",
        timeout,
    )?;
    ensure_git_success(&output, "git rev-parse marketplace revision")?;

    let revision = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if revision.is_empty() {
        Err("git rev-parse returned empty revision for marketplace source".to_string())
    } else {
        Ok(revision)
    }
}

fn is_full_git_sha(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn git_command(mode: PluginGitMode) -> Command {
    let mut command = mode.command(Path::new("git"));
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0");
    command
}

#[cfg(windows)]
fn git_path_arg(path: &Path) -> PathBuf {
    strip_windows_verbatim_path_prefix(&path.to_string_lossy())
        .map(PathBuf::from)
        .unwrap_or_else(|| path.to_path_buf())
}

#[cfg(not(windows))]
fn git_path_arg(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[cfg(any(windows, test))]
fn strip_windows_verbatim_path_prefix(path: &str) -> Option<String> {
    let stripped = path.strip_prefix(r"\\?\")?;
    let stripped = stripped
        .strip_prefix(r"UNC\")
        .map(|unc_path| format!(r"\\{unc_path}"))
        .unwrap_or_else(|| stripped.to_string());
    Some(stripped)
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
    let start = std::time::Instant::now();
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

        std::thread::sleep(Duration::from_millis(100));
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
    use super::git_command;
    use super::git_release;
    use super::is_full_git_sha;
    use super::strip_windows_verbatim_path_prefix;
    use pretty_assertions::assert_eq;
    use std::ffi::OsStr;

    #[test]
    fn full_git_sha_ref_is_already_a_remote_revision() {
        assert!(is_full_git_sha("0123456789abcdef0123456789abcdef01234567"));
        assert!(!is_full_git_sha("main"));
        assert!(!is_full_git_sha("0123456"));
    }

    #[test]
    fn recognizes_released_git_versions() {
        for (output, release) in [
            ("git version 2.19.6\n", Some((2, 19, 6))),
            ("git version 2.20.0\n", Some((2, 20, 0))),
            ("git version 2.48.1.windows.1\r\n", Some((2, 48, 1))),
            ("git version 2.39.5 (Apple Git-154)\n", Some((2, 39, 5))),
            ("git version 2.20.0.rc1\n", None),
            ("git version 2.20\n", None),
            ("git version unknown\n", None),
        ] {
            assert_eq!(git_release(output.as_bytes()), release, "{output}");
        }
    }

    #[test]
    fn git_command_uses_path_lookup_with_stable_noninteractive_env() {
        let command = git_command(crate::PluginGitMode::Automatic);

        assert_eq!(command.get_program(), OsStr::new("git"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                OsStr::new("-c"),
                OsStr::new(codex_git_utils::SAFE_BARE_REPOSITORY_CONFIG),
            ]
        );
        assert_eq!(
            command_env(&command, "GIT_OPTIONAL_LOCKS"),
            Some(Some(OsStr::new("0")))
        );
        assert_eq!(
            command_env(&command, "GIT_TERMINAL_PROMPT"),
            Some(Some(OsStr::new("0")))
        );
        assert_eq!(command_env(&command, "PATH"), None);
    }

    #[test]
    fn strips_windows_verbatim_disk_prefix_for_git() {
        assert_eq!(
            strip_windows_verbatim_path_prefix(r"\\?\C:\Users\alice\marketplace"),
            Some(r"C:\Users\alice\marketplace".to_string())
        );
    }

    #[test]
    fn strips_windows_verbatim_unc_prefix_for_git() {
        assert_eq!(
            strip_windows_verbatim_path_prefix(r"\\?\UNC\server\share\marketplace"),
            Some(r"\\server\share\marketplace".to_string())
        );
    }

    #[test]
    fn leaves_non_verbatim_path_without_rewrite() {
        assert_eq!(strip_windows_verbatim_path_prefix(r"C:\Users\alice"), None);
    }

    fn command_env<'a>(
        command: &'a std::process::Command,
        name: &str,
    ) -> Option<Option<&'a OsStr>> {
        command
            .get_envs()
            .find(|(key, _)| key == &OsStr::new(name))
            .map(|(_, value)| value)
    }
}
