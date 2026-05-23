use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStackOrdering;
use codex_core::config::Config;
use codex_git_utils::get_git_repo_root;
use unicode_segmentation::UnicodeSegmentation;

use super::CheckStatus;
use super::DoctorCheck;

const DEFAULT_TERMINAL_TITLE_ITEMS: &[&str] = &["activity", "project-name"];
const PROJECT_TITLE_MAX_CHARS: usize = 24;

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerminalTitleInputs {
    configured_items: Option<Vec<String>>,
    cwd: PathBuf,
    project_root: Option<ProjectTitleRoot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectTitleRoot {
    source: &'static str,
    path: PathBuf,
}

pub(super) fn terminal_title_check(config: &Config) -> DoctorCheck {
    terminal_title_check_from_inputs(TerminalTitleInputs {
        configured_items: config.tui_terminal_title.clone(),
        cwd: config.cwd.to_path_buf(),
        project_root: terminal_title_project_root(config, &config.cwd),
    })
}

fn terminal_title_check_from_inputs(inputs: TerminalTitleInputs) -> DoctorCheck {
    let (source, items, invalid_items) = match inputs.configured_items {
        Some(items) if items.is_empty() => ("disabled", Vec::new(), Vec::new()),
        Some(items) => {
            let (items, invalid_items) = parse_terminal_title_items(items);
            ("configured", items, invalid_items)
        }
        None => (
            "default",
            DEFAULT_TERMINAL_TITLE_ITEMS
                .iter()
                .map(ToString::to_string)
                .collect(),
            Vec::new(),
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
    if !invalid_items.is_empty() {
        details.push(format!(
            "terminal title invalid items: {}",
            invalid_items.join(", ")
        ));
    }

    if project_title_selected(&items) {
        let (project_source, project_value) =
            project_title_candidate(inputs.project_root, &inputs.cwd);
        details.push(format!("terminal title project source: {project_source}"));
        if let Some(project_value) = project_value {
            details.push(format!("terminal title project value: {project_value}"));
        }
    }

    let status = if invalid_items.is_empty() {
        CheckStatus::Ok
    } else {
        CheckStatus::Warning
    };
    let summary = if invalid_items.is_empty() {
        format!("terminal title {source}")
    } else {
        format!("terminal title {source} with invalid items")
    };
    let mut check = DoctorCheck::new("terminal.title", "title", status, summary).details(details);
    if !invalid_items.is_empty() {
        check = check.issue(
            super::DoctorIssue::new(
                CheckStatus::Warning,
                "terminal title configuration contains unknown item identifiers",
            )
            .measured(invalid_items.join(", "))
            .expected("known terminal title item identifiers")
            .remedy("Remove or replace the unknown entries in [tui].terminal_title.")
            .field("terminal title invalid items"),
        );
    }
    check
}

fn parse_terminal_title_items(items: Vec<String>) -> (Vec<String>, Vec<String>) {
    let mut invalid = Vec::new();
    let mut invalid_seen = HashSet::new();
    let mut parsed = Vec::new();
    for item in items {
        match terminal_title_item_id(&item) {
            Some(id) => parsed.push(id.to_string()),
            None => {
                if invalid_seen.insert(item.clone()) {
                    invalid.push(format!(r#""{item}""#));
                }
            }
        }
    }
    (parsed, invalid)
}

fn terminal_title_item_id(item: &str) -> Option<&'static str> {
    match item {
        "app-name" => Some("app-name"),
        "project-name" | "project" => Some("project-name"),
        "current-dir" => Some("current-dir"),
        "activity" | "spinner" => Some("activity"),
        "run-state" | "status" => Some("run-state"),
        "thread-title" | "thread" => Some("thread-title"),
        "git-branch" => Some("git-branch"),
        "context-remaining" => Some("context-remaining"),
        "context-used" | "context-usage" => Some("context-used"),
        "five-hour-limit" => Some("five-hour-limit"),
        "weekly-limit" => Some("weekly-limit"),
        "codex-version" => Some("codex-version"),
        "used-tokens" => Some("used-tokens"),
        "total-input-tokens" => Some("total-input-tokens"),
        "total-output-tokens" => Some("total-output-tokens"),
        "thread-id" | "session-id" => Some("thread-id"),
        "fast-mode" => Some("fast-mode"),
        "model" | "model-name" => Some("model"),
        "model-with-reasoning" => Some("model-with-reasoning"),
        "task-progress" => Some("task-progress"),
        _ => None,
    }
}

fn activity_enabled(items: &[String]) -> bool {
    items
        .iter()
        .any(|item| item == "activity" || item == "spinner")
}

fn project_title_selected(items: &[String]) -> bool {
    items
        .iter()
        .any(|item| item == "project-name" || item == "project")
}

fn terminal_title_project_root(config: &Config, cwd: &Path) -> Option<ProjectTitleRoot> {
    if let Some(repo_root) = get_git_repo_root(cwd) {
        return Some(ProjectTitleRoot {
            source: "git repo root",
            path: repo_root,
        });
    }

    config
        .config_layer_stack
        .get_layers(
            ConfigLayerStackOrdering::LowestPrecedenceFirst,
            /*include_disabled*/ true,
        )
        .iter()
        .find_map(|layer| match &layer.name {
            ConfigLayerSource::Project { dot_codex_folder } => dot_codex_folder
                .as_path()
                .parent()
                .map(|root| ProjectTitleRoot {
                    source: "project config",
                    path: root.to_path_buf(),
                }),
            _ => None,
        })
}

fn project_title_candidate(
    project_root: Option<ProjectTitleRoot>,
    cwd: &Path,
) -> (&'static str, Option<String>) {
    if let Some(project_root) = project_root {
        return (
            project_root.source,
            Some(truncate_title_part(path_display_name(&project_root.path))),
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
    let mut graphemes = value.graphemes(true);
    let head = graphemes
        .by_ref()
        .take(PROJECT_TITLE_MAX_CHARS)
        .collect::<String>();
    if graphemes.next().is_none() || PROJECT_TITLE_MAX_CHARS <= 3 {
        return head;
    }

    let mut truncated = head
        .graphemes(true)
        .take(PROJECT_TITLE_MAX_CHARS - 3)
        .collect::<String>();
    truncated.push_str("...");
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
            project_root: Some(ProjectTitleRoot {
                source: "git repo root",
                path: PathBuf::from("/repo"),
            }),
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
            project_root: None,
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
        assert!(
            !check
                .details
                .iter()
                .any(|detail| detail.starts_with("terminal title project "))
        );
    }

    #[test]
    fn terminal_title_reports_project_config_fallback() {
        let check = terminal_title_check_from_inputs(TerminalTitleInputs {
            configured_items: Some(vec!["project".to_string()]),
            cwd: PathBuf::from("/workspace/project/subdir"),
            project_root: Some(ProjectTitleRoot {
                source: "project config",
                path: PathBuf::from("/workspace/project"),
            }),
        });

        assert_eq!(check.summary, "terminal title configured");
        assert!(
            check
                .details
                .contains(&"terminal title project source: project config".to_string())
        );
        assert!(
            check
                .details
                .contains(&"terminal title project value: project".to_string())
        );
    }

    #[test]
    fn terminal_title_omits_project_when_project_item_is_not_selected() {
        let check = terminal_title_check_from_inputs(TerminalTitleInputs {
            configured_items: Some(vec!["model".to_string()]),
            cwd: PathBuf::from("/workspace/project"),
            project_root: Some(ProjectTitleRoot {
                source: "project config",
                path: PathBuf::from("/workspace/project"),
            }),
        });

        assert_eq!(check.summary, "terminal title configured");
        assert!(
            !check
                .details
                .iter()
                .any(|detail| detail.starts_with("terminal title project "))
        );
    }

    #[test]
    fn terminal_title_warns_for_invalid_configured_items() {
        let check = terminal_title_check_from_inputs(TerminalTitleInputs {
            configured_items: Some(vec![
                "project".to_string(),
                "bogus".to_string(),
                "activity".to_string(),
                "bogus".to_string(),
            ]),
            cwd: PathBuf::from("/workspace/project"),
            project_root: Some(ProjectTitleRoot {
                source: "project config",
                path: PathBuf::from("/workspace/project"),
            }),
        });

        assert_eq!(check.status, CheckStatus::Warning);
        assert_eq!(
            check.summary,
            "terminal title configured with invalid items"
        );
        assert!(
            check
                .details
                .contains(&"terminal title items: project-name, activity".to_string())
        );
        assert!(
            check
                .details
                .contains(&r#"terminal title invalid items: "bogus""#.to_string())
        );
        assert_eq!(check.issues.len(), 1);
    }

    #[test]
    fn terminal_title_warns_when_all_configured_items_are_invalid() {
        let check = terminal_title_check_from_inputs(TerminalTitleInputs {
            configured_items: Some(vec!["bogus".to_string()]),
            cwd: PathBuf::from("/workspace/project"),
            project_root: None,
        });

        assert_eq!(check.status, CheckStatus::Warning);
        assert!(
            check
                .details
                .contains(&"terminal title items: none".to_string())
        );
        assert!(
            check
                .details
                .contains(&r#"terminal title invalid items: "bogus""#.to_string())
        );
    }

    #[test]
    fn terminal_title_project_value_uses_tui_truncation_shape() {
        let check = terminal_title_check_from_inputs(TerminalTitleInputs {
            configured_items: Some(vec!["project".to_string()]),
            cwd: PathBuf::from("/workspace/abcdefghijklmnopqrstuvwxyz"),
            project_root: Some(ProjectTitleRoot {
                source: "project config",
                path: PathBuf::from("/workspace/abcdefghijklmnopqrstuvwxyz"),
            }),
        });

        assert!(
            check
                .details
                .contains(&"terminal title project value: abcdefghijklmnopqrstu...".to_string())
        );
    }
}
