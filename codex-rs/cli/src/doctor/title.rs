use std::path::Path;
use std::path::PathBuf;

use codex_core::config::Config;
use codex_git_utils::get_git_repo_root;

use super::CheckStatus;
use super::DoctorCheck;

const DEFAULT_TERMINAL_TITLE_ITEMS: &[&str] = &["activity", "project-name"];
const PROJECT_TITLE_MAX_CHARS: usize = 24;

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerminalTitleInputs {
    configured_items: Option<Vec<String>>,
    cwd: PathBuf,
    repo_root: Option<PathBuf>,
}

pub(super) fn terminal_title_check(config: &Config) -> DoctorCheck {
    terminal_title_check_from_inputs(TerminalTitleInputs {
        configured_items: config.tui_terminal_title.clone(),
        cwd: config.cwd.to_path_buf(),
        repo_root: get_git_repo_root(&config.cwd),
    })
}

fn terminal_title_check_from_inputs(inputs: TerminalTitleInputs) -> DoctorCheck {
    let (source, items) = match inputs.configured_items {
        Some(items) if items.is_empty() => ("disabled", Vec::new()),
        Some(items) => ("configured", items),
        None => (
            "default",
            DEFAULT_TERMINAL_TITLE_ITEMS
                .iter()
                .map(ToString::to_string)
                .collect(),
        ),
    };
    let mut details = vec![
        format!("terminal title source: {source}"),
        format!(
            "terminal title items: {}",
            if items.is_empty() {
                "none".to_string()
            } else {
                items.join(", ")
            }
        ),
        format!("terminal title activity: {}", activity_enabled(&items)),
    ];

    let (project_source, project_value) = project_title_candidate(inputs.repo_root, &inputs.cwd);
    details.push(format!("terminal title project source: {project_source}"));
    if let Some(project_value) = project_value {
        details.push(format!("terminal title project value: {project_value}"));
    }

    DoctorCheck::new(
        "terminal.title",
        "title",
        CheckStatus::Ok,
        format!("terminal title {source}"),
    )
    .details(details)
}

fn activity_enabled(items: &[String]) -> bool {
    items
        .iter()
        .any(|item| item == "activity" || item == "spinner")
}

fn project_title_candidate(
    repo_root: Option<PathBuf>,
    cwd: &Path,
) -> (&'static str, Option<String>) {
    if let Some(repo_root) = repo_root {
        return (
            "git repo root",
            Some(truncate_title_part(path_display_name(&repo_root))),
        );
    }
    ("cwd", Some(truncate_title_part(path_display_name(cwd))))
}

fn path_display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

fn truncate_title_part(value: String) -> String {
    let mut truncated = value
        .chars()
        .take(PROJECT_TITLE_MAX_CHARS)
        .collect::<String>();
    if value.chars().count() > PROJECT_TITLE_MAX_CHARS {
        truncated.push_str("...");
    }
    truncated
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn terminal_title_reports_default_items_and_git_project_name() {
        let check = terminal_title_check_from_inputs(TerminalTitleInputs {
            configured_items: None,
            cwd: PathBuf::from("/repo/subdir"),
            repo_root: Some(PathBuf::from("/repo")),
        });

        assert_eq!(check.summary, "terminal title default");
        assert!(
            check
                .details
                .contains(&"terminal title items: activity, project-name".to_string())
        );
        assert!(
            check
                .details
                .contains(&"terminal title project source: git repo root".to_string())
        );
        assert!(
            check
                .details
                .contains(&"terminal title project value: repo".to_string())
        );
    }

    #[test]
    fn terminal_title_reports_disabled_configuration() {
        let check = terminal_title_check_from_inputs(TerminalTitleInputs {
            configured_items: Some(Vec::new()),
            cwd: PathBuf::from("/workspace"),
            repo_root: None,
        });

        assert_eq!(check.summary, "terminal title disabled");
        assert!(
            check
                .details
                .contains(&"terminal title items: none".to_string())
        );
        assert!(
            check
                .details
                .contains(&"terminal title activity: false".to_string())
        );
    }
}
