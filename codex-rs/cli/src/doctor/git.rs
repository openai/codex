use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;
use std::process::Output;
use std::time::Duration;

use codex_git_utils::get_git_repo_root;
use tokio::process::Command;
use tokio::time::timeout;

use super::CheckStatus;
use super::DoctorCheck;
use super::DoctorIssue;

const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 2);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GitCheckInputs {
    selected_git: Option<PathBuf>,
    git_candidates: Vec<PathBuf>,
    git_version: Option<String>,
    git_exec_path: Option<String>,
    git_build_options: Option<String>,
    repo_root: Option<PathBuf>,
    git_entry: Option<String>,
    branch: Option<String>,
    core_fsmonitor: Option<String>,
}

pub(super) async fn git_check(cwd: &Path) -> DoctorCheck {
    let selected_git = which::which("git").ok();
    let git_candidates = git_candidates();
    let repo_root = get_git_repo_root(cwd);

    let (git_version, git_exec_path, git_build_options, branch, core_fsmonitor) =
        if let Some(git_path) = selected_git.as_deref() {
            let (version, exec_path, build_options, branch, fsmonitor) = tokio::join!(
                git_output(git_path, cwd, &["--version"]),
                git_output(git_path, cwd, &["--exec-path"]),
                git_output(git_path, cwd, &["version", "--build-options"]),
                git_output(git_path, cwd, &["rev-parse", "--abbrev-ref", "HEAD"]),
                git_output(git_path, cwd, &["config", "--get", "core.fsmonitor"]),
            );
            (version, exec_path, build_options, branch, fsmonitor)
        } else {
            (None, None, None, None, None)
        };

    git_check_from_inputs(GitCheckInputs {
        selected_git,
        git_candidates,
        git_version,
        git_exec_path,
        git_build_options,
        git_entry: repo_root.as_deref().map(git_entry_summary),
        repo_root,
        branch,
        core_fsmonitor,
    })
}

fn git_check_from_inputs(inputs: GitCheckInputs) -> DoctorCheck {
    let mut details = Vec::new();
    match inputs.selected_git.as_deref() {
        Some(path) => details.push(format!("selected git: {}", path.display())),
        None => details.push("selected git: not found".to_string()),
    }
    details.push(format!("PATH git entries: {}", inputs.git_candidates.len()));
    for (index, path) in inputs.git_candidates.iter().enumerate() {
        details.push(format!("PATH git #{}: {}", index + 1, path.display()));
    }
    push_optional_detail(&mut details, "git version", inputs.git_version.as_deref());
    push_optional_detail(
        &mut details,
        "git exec path",
        inputs.git_exec_path.as_deref(),
    );
    push_optional_detail(
        &mut details,
        "git build options",
        inputs.git_build_options.as_deref(),
    );
    match inputs.repo_root.as_deref() {
        Some(root) => {
            details.push("repo detected: true".to_string());
            details.push(format!("repo root: {}", root.display()));
        }
        None => details.push("repo detected: false".to_string()),
    }
    push_optional_detail(&mut details, ".git entry", inputs.git_entry.as_deref());
    push_optional_detail(
        &mut details,
        "git branch",
        normalized_branch(inputs.branch.as_deref()),
    );
    push_optional_detail(
        &mut details,
        "core.fsmonitor",
        inputs
            .core_fsmonitor
            .as_deref()
            .filter(|value| !value.is_empty()),
    );

    let mut check = DoctorCheck::new(
        "git.environment",
        "git",
        CheckStatus::Ok,
        git_summary(&inputs),
    )
    .details(details);

    if inputs.selected_git.is_some() && inputs.git_version.is_none() {
        check.status = CheckStatus::Warning;
        check.summary = "Git executable found but could not be run".to_string();
        check = check.issue(
            DoctorIssue::new(
                CheckStatus::Warning,
                "Git executable was found on PATH but did not return a version",
            )
            .expected("git --version succeeds")
            .remedy("Fix the selected Git executable or PATH so Codex can inspect Git metadata.")
            .field("git version")
            .field("selected git"),
        );
    } else if inputs.selected_git.is_none() && inputs.repo_root.is_some() {
        check.status = CheckStatus::Warning;
        check.summary = "Git repository detected but git executable was not found".to_string();
        check = check.issue(
            DoctorIssue::new(
                CheckStatus::Warning,
                "Git repository detected but git executable was not found",
            )
            .expected("git available on PATH")
            .remedy("Install Git or fix PATH so Codex can inspect repository metadata.")
            .field("selected git"),
        );
    } else if let Some(cause) =
        old_windows_git_warning(inputs.git_version.as_deref(), cfg!(windows))
    {
        check.status = CheckStatus::Warning;
        check.summary = cause.clone();
        check = check.issue(
            DoctorIssue::new(CheckStatus::Warning, cause)
                .measured(inputs.git_version.unwrap_or_else(|| "unknown".to_string()))
                .expected("current Git for Windows")
                .remedy(
                    "Update Git for Windows or the bundled Git executable Codex resolves first.",
                )
                .field("git version")
                .field("selected git"),
        );
    }

    check
}

fn git_summary(inputs: &GitCheckInputs) -> String {
    match inputs.git_version.as_deref() {
        Some(version) => version.to_string(),
        None if inputs.selected_git.is_some() => {
            "git executable found; version unavailable".to_string()
        }
        None => "git executable not found".to_string(),
    }
}

fn push_optional_detail(details: &mut Vec<String>, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        details.push(format!("{label}: {value}"));
    }
}

fn normalized_branch(branch: Option<&str>) -> Option<&str> {
    match branch {
        Some("HEAD") => Some("detached HEAD"),
        Some(value) if !value.is_empty() => Some(value),
        _ => None,
    }
}

fn git_candidates() -> Vec<PathBuf> {
    let Ok(candidates) = which::which_all("git") else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    candidates
        .filter(|candidate| seen.insert(candidate.clone()))
        .collect()
}

async fn git_output(git_path: &Path, cwd: &Path, args: &[&str]) -> Option<String> {
    let mut command = Command::new(git_path);
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(args)
        .current_dir(cwd)
        .kill_on_drop(true);
    let output = timeout(GIT_COMMAND_TIMEOUT, command.output())
        .await
        .ok()?
        .ok()?;
    command_output_text(output)
}

fn command_output_text(output: Output) -> Option<String> {
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let normalized = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn git_entry_summary(repo_root: &Path) -> String {
    let entry = repo_root.join(".git");
    match std::fs::metadata(&entry) {
        Ok(metadata) if metadata.is_dir() => "directory".to_string(),
        Ok(metadata) if metadata.is_file() => std::fs::read_to_string(&entry)
            .ok()
            .and_then(|contents| {
                contents
                    .strip_prefix("gitdir:")
                    .map(str::trim)
                    .map(|path| format!("file -> {path}"))
            })
            .unwrap_or_else(|| "file".to_string()),
        Ok(_) => "other".to_string(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => "missing".to_string(),
        Err(err) => format!("unreadable ({err})"),
    }
}

fn old_windows_git_warning(version: Option<&str>, is_windows: bool) -> Option<String> {
    if !is_windows {
        return None;
    }
    let version = version?;
    if version.to_ascii_lowercase().contains("msysgit") {
        return Some("old msysgit installation may corrupt Windows TUI rendering".to_string());
    }
    let parsed = parse_git_version(version)?;
    if parsed.major < 2 || (parsed.major == 2 && parsed.minor <= 34) {
        return Some("old Git for Windows may corrupt Windows TUI rendering".to_string());
    }
    None
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ParsedGitVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

fn parse_git_version(version: &str) -> Option<ParsedGitVersion> {
    let version = version.strip_prefix("git version ")?;
    let numeric = version
        .split_whitespace()
        .next()?
        .split(".windows.")
        .next()
        .unwrap_or(version);
    let mut parts = numeric.split('.');
    Some(ParsedGitVersion {
        major: parts.next()?.parse().ok()?,
        minor: parts.next()?.parse().ok()?,
        patch: parts.next().unwrap_or("0").parse().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn parses_git_for_windows_version() {
        assert_eq!(
            parse_git_version("git version 2.34.1.windows.1"),
            Some(ParsedGitVersion {
                major: 2,
                minor: 34,
                patch: 1,
            })
        );
        assert_eq!(
            parse_git_version("git version 2.54.0.windows.1"),
            Some(ParsedGitVersion {
                major: 2,
                minor: 54,
                patch: 0,
            })
        );
    }

    #[test]
    fn classifies_old_windows_git() {
        assert_eq!(
            old_windows_git_warning(
                Some("git version 2.34.1.windows.1"),
                /*is_windows*/ true
            )
            .as_deref(),
            Some("old Git for Windows may corrupt Windows TUI rendering")
        );
        assert_eq!(
            old_windows_git_warning(
                Some("git version 2.54.0.windows.1"),
                /*is_windows*/ true
            ),
            None
        );
        assert_eq!(
            old_windows_git_warning(
                Some("git version 2.34.1.windows.1"),
                /*is_windows*/ false
            ),
            None
        );
    }

    #[test]
    fn warns_when_git_repo_has_no_git_executable() {
        let check = git_check_from_inputs(GitCheckInputs {
            repo_root: Some(PathBuf::from("/repo")),
            ..GitCheckInputs::default()
        });

        assert_eq!(check.status, CheckStatus::Warning);
        assert_eq!(
            check.summary,
            "Git repository detected but git executable was not found"
        );
    }

    #[test]
    fn warns_when_selected_git_cannot_report_version() {
        let check = git_check_from_inputs(GitCheckInputs {
            selected_git: Some(PathBuf::from("/usr/bin/git")),
            repo_root: Some(PathBuf::from("/repo")),
            ..GitCheckInputs::default()
        });

        assert_eq!(check.status, CheckStatus::Warning);
        assert_eq!(check.summary, "Git executable found but could not be run");
    }

    #[test]
    fn reports_git_candidates_and_repo_metadata() {
        let check = git_check_from_inputs(GitCheckInputs {
            selected_git: Some(PathBuf::from("/usr/bin/git")),
            git_candidates: vec![PathBuf::from("/usr/bin/git"), PathBuf::from("/opt/bin/git")],
            git_version: Some("git version 2.54.0".to_string()),
            git_exec_path: Some("/usr/libexec/git-core".to_string()),
            repo_root: Some(PathBuf::from("/repo")),
            git_entry: Some("directory".to_string()),
            branch: Some("main".to_string()),
            core_fsmonitor: Some("false".to_string()),
            ..GitCheckInputs::default()
        });

        assert_eq!(check.status, CheckStatus::Ok);
        assert!(check.details.contains(&"PATH git entries: 2".to_string()));
        assert!(check.details.contains(&"git branch: main".to_string()));
        assert!(check.details.contains(&"core.fsmonitor: false".to_string()));
    }
}
